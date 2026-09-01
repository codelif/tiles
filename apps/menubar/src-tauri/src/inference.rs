//! the inference server, reachable only through the daemon

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::{daemon, tray};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const START_TIMEOUT: Duration = Duration::from_secs(20);
const STOP_TIMEOUT: Duration = Duration::from_secs(15);

/// start answers in ~40ms and the port opens ~5s later, past this one that
/// never opened it is called off
const READY_TIMEOUT: Duration = Duration::from_secs(15);

/// how long the user's ask outlives the request meant to satisfy it
const RECONCILE_TIMEOUT: Duration = Duration::from_secs(45);

pub const STATE_EVENT: &str = "inference://state";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Power {
    /// no daemon to ask through
    Unknown,
    Off,
    Starting,
    On,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub power: Power,
    /// `[model] current`, what the daemon is configured for
    pub model: Option<String>,
}

struct Inference {
    state: Mutex<State>,
    /// set when we ask for a start, the only thing that separates one still
    /// coming up from a server that is simply down
    starting_since: Mutex<Option<Instant>>,
    /// what the user last asked for, held until the server agrees, see [`reconcile`]
    desired: Mutex<Option<(bool, Instant)>>,
    /// a second start inside the boot window spawns a second server, the daemon
    /// only checks the port
    in_flight: AtomicBool,
}

pub fn init(app: &AppHandle) {
    app.manage(Inference {
        state: Mutex::new(State {
            power: Power::Unknown,
            model: None,
        }),
        starting_since: Mutex::new(None),
        desired: Mutex::new(None),
        in_flight: AtomicBool::new(false),
    });
}

fn current(app: &AppHandle) -> State {
    app.state::<Inference>().state.lock().unwrap().clone()
}

/// emits on change only, same as the daemon's health
fn set(app: &AppHandle, next: State) {
    let inference = app.state::<Inference>();
    let mut state = inference.state.lock().unwrap();
    if *state == next {
        return;
    }
    let power = next.power;
    *state = next.clone();
    drop(state);

    let _ = app.emit(STATE_EVENT, next);

    // the status item dims with the panel's mark, and AppKit wants the main thread
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || tray::set_live(&handle, power == Power::On));
}

fn set_power(app: &AppHandle, power: Power) {
    let mut next = current(app);
    next.power = power;
    set(app, next);
}

fn settle(app: &AppHandle, power: Power) {
    *app.state::<Inference>().starting_since.lock().unwrap() = None;
    set_power(app, power);
}

/// a start in flight resolves in seconds, so the supervisor watches it closely
pub fn is_settling(app: &AppHandle) -> bool {
    app.try_state::<Inference>()
        .is_some_and(|inference| inference.state.lock().unwrap().power == Power::Starting)
}

/// the daemon stopped answering, and it is the only way in
pub fn unknown(app: &AppHandle) {
    *app.state::<Inference>().desired.lock().unwrap() = None;
    settle(app, Power::Unknown);
}

/// one supervisor tick, only while the daemon answers
pub async fn poll(app: &AppHandle, client: &reqwest::Client) {
    let listening = matches!(
        client
            .get(daemon::url("/v1/tilekit/server/ping"))
            .send()
            .await,
        Ok(res) if res.status().is_success()
    );

    let started_at = *app.state::<Inference>().starting_since.lock().unwrap();
    let power = match (listening, started_at) {
        (true, _) => Power::On,
        (false, Some(at)) if at.elapsed() < READY_TIMEOUT => Power::Starting,
        (false, _) => Power::Off,
    };

    if power != Power::Starting {
        *app.state::<Inference>().starting_since.lock().unwrap() = None;
    }

    let model = match current(app).model {
        known @ Some(_) => known,
        // read once, the spec only changes when someone runs the cli
        None => model_name(client).await,
    };

    set(app, State { power, model });
    reconcile(app, client, power).await;
}

/// the daemon decides "is it running" by pinging rather than by pid, so a stop
/// inside a start's boot window reports success and kills nothing, and a start
/// inside a stop's reports "already up" and spawns nothing. either way the
/// switch would settle opposite to the tap, so the ask is re-issued until the
/// server agrees with it
async fn reconcile(app: &AppHandle, client: &reqwest::Client, power: Power) {
    let Some((on, asked_at)) = *app.state::<Inference>().desired.lock().unwrap() else {
        return;
    };

    let settled = match power {
        Power::On => true,
        Power::Off => false,
        // still moving, nothing to disagree with yet
        Power::Starting | Power::Unknown => return,
    };

    if settled == on || asked_at.elapsed() > RECONCILE_TIMEOUT {
        *app.state::<Inference>().desired.lock().unwrap() = None;
        return;
    }

    let _ = request(app, client, on).await;
}

/// the config blob carries the user's did, so only this field crosses into the
/// webview
async fn model_name(client: &reqwest::Client) -> Option<String> {
    let res = client.get(daemon::url("/config")).send().await.ok()?;
    // reqwest is built without its json feature, serde_json is already here
    let config: serde_json::Value = serde_json::from_str(&res.text().await.ok()?).ok()?;
    let spec = config.get("model")?.get("current")?.as_str()?;

    (!spec.is_empty()).then(|| spec.to_owned())
}

async fn request(app: &AppHandle, client: &reqwest::Client, on: bool) -> Result<(), String> {
    if app
        .state::<Inference>()
        .in_flight
        .swap(true, Ordering::SeqCst)
    {
        return Err("a start or stop is already in flight".into());
    }

    let (path, timeout) = if on {
        ("/v1/tilekit/server/start", START_TIMEOUT)
    } else {
        ("/v1/tilekit/server/stop", STOP_TIMEOUT)
    };

    if on {
        *app.state::<Inference>().starting_since.lock().unwrap() = Some(Instant::now());
        set_power(app, Power::Starting);
    }

    let outcome = match client.get(daemon::url(path)).timeout(timeout).send().await {
        Ok(res) if res.status().is_success() => Ok(()),
        Ok(res) => Err(format!("{path} answered {}", res.status())),
        Err(err) => Err(err.to_string()),
    };

    match (on, &outcome) {
        // a start that was refused never comes up
        (true, Err(_)) => settle(app, Power::Off),
        // stop closes the port before the next tick, no reason to wait to say so
        (false, Ok(())) => settle(app, Power::Off),
        // a start that landed is the poll's to confirm
        _ => {}
    }

    app.state::<Inference>()
        .in_flight
        .store(false, Ordering::SeqCst);

    outcome
}

#[tauri::command]
pub fn inference_state(app: AppHandle) -> State {
    current(&app)
}

#[tauri::command]
pub async fn inference_set(app: AppHandle, on: bool) -> Result<(), String> {
    // the ask outlives this request, the daemon may quietly ignore it
    *app.state::<Inference>().desired.lock().unwrap() = Some((on, Instant::now()));

    request(&app, &reqwest::Client::new(), on).await
}
