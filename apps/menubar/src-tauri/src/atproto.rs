//! the atproto session, held by the daemon

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::daemon;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub const STATE_EVENT: &str = "atproto://state";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum State {
    /// no daemon to ask through
    Unknown,
    /// the daemon answered, nobody is signed in
    None,
    Pending {
        handle: String,
    },
    Session {
        handle: String,
        did: String,
    },
}

struct Atproto {
    state: Mutex<State>,
    /// one login at a time; the daemon binds one callback port
    in_flight: AtomicBool,
}

pub fn init(app: &AppHandle) {
    app.manage(Atproto {
        state: Mutex::new(State::Unknown),
        in_flight: AtomicBool::new(false),
    });
}

fn current(app: &AppHandle) -> State {
    app.state::<Atproto>().state.lock().unwrap().clone()
}

fn set(app: &AppHandle, next: State) {
    let atproto = app.state::<Atproto>();
    let mut state = atproto.state.lock().unwrap();
    if *state == next {
        return;
    }
    *state = next.clone();
    drop(state);

    let _ = app.emit(STATE_EVENT, next);
}

pub fn unknown(app: &AppHandle) {
    set(app, State::Unknown);
}

pub async fn poll(app: &AppHandle, client: &reqwest::Client) {
    // the daemon reports signed out for the whole browser round trip
    if app.state::<Atproto>().in_flight.load(Ordering::SeqCst) {
        return;
    }

    set(app, fetch(client).await);
}

async fn fetch(client: &reqwest::Client) -> State {
    let Ok(res) = client
        .get(daemon::url("/v1/tilekit/atproto/status"))
        .send()
        .await
    else {
        return State::Unknown;
    };

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return State::None;
    }
    if !res.status().is_success() {
        return State::Unknown;
    }

    match res.text().await {
        Ok(body) => parse(&body),
        Err(_) => State::Unknown,
    }
}

fn parse(body: &str) -> State {
    // reqwest has no json feature here
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(body) else {
        return State::Unknown;
    };

    let data = payload.get("data");
    let handle = data.and_then(|d| d.get("handle")).and_then(|v| v.as_str());
    let did = data.and_then(|d| d.get("did")).and_then(|v| v.as_str());

    match (handle, did) {
        (Some(handle), Some(did)) if !handle.is_empty() && !did.is_empty() => State::Session {
            handle: handle.to_owned(),
            did: did.to_owned(),
        },
        _ => State::Unknown,
    }
}

#[tauri::command]
pub fn atproto_state(app: AppHandle) -> State {
    current(&app)
}

#[tauri::command]
pub async fn atproto_login(app: AppHandle, handle: String) -> Result<(), String> {
    let handle = handle.trim().trim_start_matches('@').to_lowercase();
    if handle.is_empty() {
        return Err("A handle is needed".into());
    }

    if app
        .state::<Atproto>()
        .in_flight
        .swap(true, Ordering::SeqCst)
    {
        return Err("A sign-in is already waiting".into());
    }

    set(
        &app,
        State::Pending {
            handle: handle.clone(),
        },
    );

    let outcome = request(&handle).await;
    app.state::<Atproto>()
        .in_flight
        .store(false, Ordering::SeqCst);

    if let Err(err) = outcome {
        set(&app, State::Unknown);
        return Err(err);
    }

    Ok(())
}

async fn request(handle: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "user_handle": handle }).to_string();

    let res = client
        .post(daemon::url("/v1/tilekit/atproto/login"))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|err| err.to_string())?;

    let status = res.status();
    if status.is_success() {
        return Ok(());
    }

    Err(reason(&res.text().await.unwrap_or_default(), status))
}

fn reason(body: &str, status: reqwest::StatusCode) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .as_ref()
        .and_then(|payload| payload.get("reason"))
        .and_then(|v| v.as_str())
        .map(|reason| reason.to_owned())
        .unwrap_or_else(|| format!("The daemon answered {status}"))
}

#[cfg(test)]
mod tests {
    use super::{State, parse};

    const SUCCESS: &str = r#"{"status":"success","data":{"handle":"codelif.in","did":"did:plc:7iza6de2dwap2sbkpav7c6c6"}}"#;

    #[test]
    fn reads_the_daemons_success_body() {
        assert_eq!(
            parse(SUCCESS),
            State::Session {
                handle: "codelif.in".to_owned(),
                did: "did:plc:7iza6de2dwap2sbkpav7c6c6".to_owned(),
            }
        );
    }

    #[test]
    fn a_body_without_an_identity_is_not_a_sign_out() {
        assert_eq!(parse(r#"{"status":"success","data":{}}"#), State::Unknown);
        assert_eq!(parse("not json"), State::Unknown);
    }
}
