//! the atproto session, held by the daemon

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::daemon;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub const STATE_EVENT: &str = "atproto://state";

const AVATAR_MAX_BYTES: u64 = 2 * 1024 * 1024;

const AVATAR_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/webp", "image/gif"];

const PROFILE_TIMEOUT: Duration = Duration::from_secs(10);

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
        #[serde(rename = "displayName")]
        display_name: Option<String>,
        /// a data uri
        avatar: Option<String>,
        pds: Option<String>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Profile {
    display_name: Option<String>,
    avatar: Option<String>,
    pds: Option<String>,
}

#[derive(Default)]
struct Profiles {
    /// settled reads, keyed by did
    loaded: Option<(String, Profile)>,
    /// the did a task already has out
    reading: Option<String>,
}

struct Atproto {
    state: Mutex<State>,
    /// one login at a time; the daemon binds one callback port
    in_flight: AtomicBool,
    profiles: Mutex<Profiles>,
}

pub fn init(app: &AppHandle) {
    app.manage(Atproto {
        state: Mutex::new(State::Unknown),
        in_flight: AtomicBool::new(false),
        profiles: Mutex::new(Profiles::default()),
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

    let next = dress(app, fetch(client).await);
    set(app, next);
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
            display_name: None,
            avatar: None,
            pds: None,
        },
        _ => State::Unknown,
    }
}

fn dress(app: &AppHandle, state: State) -> State {
    let State::Session { handle, did, .. } = state else {
        return state;
    };

    let profile = {
        let atproto = app.state::<Atproto>();
        let mut profiles = atproto.profiles.lock().unwrap();
        match &profiles.loaded {
            Some((seen, profile)) if *seen == did => profile.clone(),
            _ => {
                if profiles.reading.as_deref() != Some(did.as_str()) {
                    profiles.reading = Some(did.clone());
                    spawn_read(app.clone(), did.clone());
                }
                Profile::default()
            }
        }
    };

    State::Session {
        handle,
        did,
        display_name: profile.display_name,
        avatar: profile.avatar,
        pds: profile.pds,
    }
}

fn spawn_read(app: AppHandle, did: String) {
    tauri::async_runtime::spawn(async move {
        let profile = read_profile(&did).await;

        {
            let atproto = app.state::<Atproto>();
            let mut profiles = atproto.profiles.lock().unwrap();
            profiles.reading = None;
            if let Some(profile) = profile.clone() {
                profiles.loaded = Some((did.clone(), profile));
            }
        }

        let Some(profile) = profile else {
            return;
        };

        let State::Session {
            handle, did: at, ..
        } = current(&app)
        else {
            return;
        };
        if at != did {
            return;
        }

        set(
            &app,
            State::Session {
                handle,
                did: at,
                display_name: profile.display_name,
                avatar: profile.avatar,
                pds: profile.pds,
            },
        );
    });
}

/// `None` is the pds saying nothing, not an empty profile
async fn read_profile(did: &str) -> Option<Profile> {
    let client = reqwest::Client::builder()
        .timeout(PROFILE_TIMEOUT)
        .build()
        .ok()?;

    let pds = resolve_pds(&client, did).await?;
    let res = client
        .get(format!(
            "{pds}/xrpc/com.atproto.repo.getRecord?repo={did}&collection=app.bsky.actor.profile&rkey=self"
        ))
        .send()
        .await
        .ok()?;

    // a record never written answers 400, not 404
    if !res.status().is_success() {
        return Some(Profile {
            pds: Some(pds),
            ..Profile::default()
        });
    }

    let (display_name, blob) = parse_profile(&res.text().await.ok()?);
    let avatar = match blob {
        Some((cid, mime)) => blob_uri(&client, &pds, did, &cid, &mime).await,
        None => None,
    };

    Some(Profile {
        display_name,
        avatar,
        pds: Some(pds),
    })
}

async fn resolve_pds(client: &reqwest::Client, did: &str) -> Option<String> {
    let res = client.get(did_doc_url(did)?).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }

    pds_from_doc(&res.text().await.ok()?)
}

fn did_doc_url(did: &str) -> Option<String> {
    if !is_token(did) {
        return None;
    }

    if did.starts_with("did:plc:") {
        return Some(format!("https://plc.directory/{did}"));
    }

    let host = did.strip_prefix("did:web:")?;
    if host.is_empty() || host.contains(':') {
        return None;
    }

    Some(format!("https://{host}/.well-known/did.json"))
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'.' | b'_' | b'-'))
}

fn pds_from_doc(body: &str) -> Option<String> {
    let doc = serde_json::from_str::<serde_json::Value>(body).ok()?;
    let endpoint = doc
        .get("service")?
        .as_array()?
        .iter()
        .find(|entry| {
            entry.get("type").and_then(|v| v.as_str()) == Some("AtprotoPersonalDataServer")
        })?
        .get("serviceEndpoint")?
        .as_str()?;

    if !endpoint.starts_with("https://") {
        return None;
    }

    Some(endpoint.trim_end_matches('/').to_owned())
}

fn parse_profile(body: &str) -> (Option<String>, Option<(String, String)>) {
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(body) else {
        return (None, None);
    };

    let value = payload.get("value");
    let display_name = value
        .and_then(|v| v.get("displayName"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned);

    let avatar = value.and_then(|v| v.get("avatar"));
    let cid = avatar
        .and_then(|a| a.get("ref"))
        .and_then(|r| r.get("$link"))
        .and_then(|v| v.as_str());
    let mime = avatar
        .and_then(|a| a.get("mimeType"))
        .and_then(|v| v.as_str());

    let blob = match (cid, mime) {
        (Some(cid), Some(mime)) if AVATAR_TYPES.contains(&mime) && is_token(cid) => {
            Some((cid.to_owned(), mime.to_owned()))
        }
        _ => None,
    };

    (display_name, blob)
}

async fn blob_uri(
    client: &reqwest::Client,
    pds: &str,
    did: &str,
    cid: &str,
    mime: &str,
) -> Option<String> {
    let res = client
        .get(format!(
            "{pds}/xrpc/com.atproto.sync.getBlob?did={did}&cid={cid}"
        ))
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return None;
    }

    if res
        .content_length()
        .is_some_and(|len| len > AVATAR_MAX_BYTES)
    {
        return None;
    }

    let bytes = res.bytes().await.ok()?;
    if bytes.len() as u64 > AVATAR_MAX_BYTES {
        return None;
    }

    Some(format!(
        "data:{mime};base64,{}",
        data_encoding::BASE64.encode(&bytes)
    ))
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
    use super::{State, did_doc_url, parse, parse_profile, pds_from_doc};

    const SUCCESS: &str = r#"{"status":"success","data":{"handle":"codelif.in","did":"did:plc:7iza6de2dwap2sbkpav7c6c6"}}"#;

    const RECORD: &str = r#"{"uri":"at://did:plc:x/app.bsky.actor.profile/self","value":{"$type":"app.bsky.actor.profile","displayName":"Harsh Sharma","avatar":{"$type":"blob","ref":{"$link":"bafkreiabc123"},"mimeType":"image/jpeg","size":91234}}}"#;

    #[test]
    fn reads_the_daemons_success_body() {
        assert_eq!(
            parse(SUCCESS),
            State::Session {
                handle: "codelif.in".to_owned(),
                did: "did:plc:7iza6de2dwap2sbkpav7c6c6".to_owned(),
                display_name: None,
                avatar: None,
                pds: None,
            }
        );
    }

    #[test]
    fn a_body_without_an_identity_is_not_a_sign_out() {
        assert_eq!(parse(r#"{"status":"success","data":{}}"#), State::Unknown);
        assert_eq!(parse("not json"), State::Unknown);
    }

    #[test]
    fn finds_the_pds_in_a_did_document() {
        let doc = r##"{"service":[{"id":"#atproto_pds","type":"AtprotoPersonalDataServer","serviceEndpoint":"https://shimeji.us-east.host.bsky.network/"}]}"##;
        assert_eq!(
            pds_from_doc(doc).as_deref(),
            Some("https://shimeji.us-east.host.bsky.network")
        );

        assert_eq!(pds_from_doc(r#"{"service":[]}"#), None);
        let plain = r#"{"service":[{"type":"AtprotoPersonalDataServer","serviceEndpoint":"http://pds.example"}]}"#;
        assert_eq!(pds_from_doc(plain), None);
    }

    #[test]
    fn reads_the_profile_record() {
        let (name, blob) = parse_profile(RECORD);
        assert_eq!(name.as_deref(), Some("Harsh Sharma"));
        assert_eq!(
            blob,
            Some(("bafkreiabc123".to_owned(), "image/jpeg".to_owned()))
        );
    }

    #[test]
    fn a_record_without_a_picture_is_not_a_failure() {
        let (name, blob) = parse_profile(r#"{"value":{"displayName":"Harsh"}}"#);
        assert_eq!(name.as_deref(), Some("Harsh"));
        assert_eq!(blob, None);

        let odd = r#"{"value":{"avatar":{"ref":{"$link":"bafkrei1"},"mimeType":"video/mp4"}}}"#;
        assert_eq!(parse_profile(odd), (None, None));
    }

    #[test]
    fn only_resolvable_dids_become_urls() {
        assert_eq!(
            did_doc_url("did:plc:abc").as_deref(),
            Some("https://plc.directory/did:plc:abc")
        );
        assert_eq!(
            did_doc_url("did:web:example.com").as_deref(),
            Some("https://example.com/.well-known/did.json")
        );
        assert_eq!(did_doc_url("did:web:example.com:u:alice"), None);
        assert_eq!(did_doc_url("did:plc:a/../../evil"), None);
    }
}
