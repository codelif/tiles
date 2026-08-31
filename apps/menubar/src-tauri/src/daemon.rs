//! finding the daemon, starting it, and watching it

use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};


const PORT: u16 = 1729;

const PING_TIMEOUT: Duration = Duration::from_secs(1);
const POLL_UP: Duration = Duration::from_secs(5);
const POLL_STARTING: Duration = Duration::from_millis(500);

/// past this a daemon that never answered is called down
const START_TIMEOUT: Duration = Duration::from_secs(20);

pub const HEALTH_EVENT: &str = "daemon://health";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Health {
    Down { reason: String },
    Starting,
    Up { version: String },
}

pub struct Daemon {
    health: Mutex<Health>,
    /// some only while we own the process, an adopted daemon is not ours to kill
    child: Mutex<Option<Child>>,
}

pub fn init(app: &AppHandle) {
    app.manage(Daemon {
        health: Mutex::new(Health::Starting),
        child: Mutex::new(None),
    });

    let app = app.clone();
    tauri::async_runtime::spawn(supervise(app));
}

/// `env!` resolves where we were built, so this is only good for debug builds
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("src-tauri sits three levels under the repo root")
        .to_path_buf()
}

/// a GUI has no shell PATH, so the binary has to be looked for by hand
fn resolve() -> Option<PathBuf> {
    if let Some(from_env) = std::env::var_os("TILES_BIN") {
        let path = PathBuf::from(from_env);
        if path.is_file() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();

    // in dev, the daemon built alongside us, which keeps its data in .tiles_dev
    if cfg!(debug_assertions) {
        candidates.push(repo_root().join("target/debug/tiles"));
    }

    // a sidecar shipped in the bundle, once there is one
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("tiles"));
    }

    candidates.push(PathBuf::from("/usr/local/bin/tiles"));
    candidates.push(PathBuf::from("/opt/homebrew/bin/tiles"));
    if let Some(home) = std::env::home_dir() {
        candidates.push(home.join(".cargo/bin/tiles"));
    }

    candidates.into_iter().find(|path| path.is_file())
}

/// the debug daemon resolves .tiles_dev off the cwd, and a GUI inherits `/`
fn work_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        repo_root()
    } else {
        std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    }
}

fn logs(app: &AppHandle) -> (Stdio, Stdio) {
    let Ok(dir) = app.path().app_log_dir() else {
        return (Stdio::null(), Stdio::null());
    };
    let _ = std::fs::create_dir_all(&dir);

    let open = |name: &str| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(name))
            .ok()
    };

    match (open("daemon.out.log"), open("daemon.err.log")) {
        (Some(out), Some(err)) => (out.into(), err.into()),
        _ => (Stdio::null(), Stdio::null()),
    }
}

/// a debug daemon takes the db key from the env and never reads back the one it
/// saved, so without a stable value every run mints a new key and then cannot
/// open the db the run before it wrote
fn dev_db_password() -> Option<String> {
    if let Ok(from_env) = std::env::var("TILES_DEV_DB_PASSWORD") {
        return Some(from_env);
    }

    let path = repo_root().join(".tiles_dev/db_password");
    if let Ok(saved) = std::fs::read_to_string(&path) {
        return Some(saved.trim().to_owned());
    }

    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
    let password: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();

    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, &password).ok()?;
    let _ = std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600));
    Some(password)
}

fn spawn(app: &AppHandle, bin: &Path) -> std::io::Result<Child> {
    let (out, err) = logs(app);

    let mut command = Command::new(bin);
    command
        // bare `daemon` serves in the foreground and stays our child. `daemon
        // start` detaches and resolves its own binary path off the cwd
        .arg("daemon")
        .current_dir(work_dir())
        .env("RUST_LOG", "info,iroh=error,tracing=off")
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err);

    if cfg!(debug_assertions)
        && let Some(password) = dev_db_password()
    {
        command.env("TILES_DEV_DB_PASSWORD", password);
    }

    command.spawn()
}

fn start(app: &AppHandle) {
    let Some(bin) = resolve() else {
        set(
            app,
            Health::Down {
                reason: "no tiles binary found".into(),
            },
        );
        return;
    };

    match spawn(app, &bin) {
        Ok(child) => {
            *app.state::<Daemon>().child.lock().unwrap() = Some(child);
            set(app, Health::Starting);
        }
        Err(err) => set(
            app,
            Health::Down {
                reason: err.to_string(),
            },
        ),
    }
}

/// `GET /` answers with the daemon's version, so one request covers both questions
async fn ping(client: &reqwest::Client) -> Option<String> {
    let res = client
        .get(format!("http://127.0.0.1:{PORT}/"))
        .send()
        .await
        .ok()?;
    Some(res.text().await.ok()?.trim().to_owned())
}

/// some once, when a child we spawned has exited
fn reap(app: &AppHandle) -> Option<ExitStatus> {
    let daemon = app.state::<Daemon>();
    let mut child = daemon.child.lock().unwrap();
    let status = child.as_mut()?.try_wait().ok().flatten()?;
    *child = None;
    Some(status)
}

fn current(app: &AppHandle) -> Health {
    app.state::<Daemon>().health.lock().unwrap().clone()
}

/// emits on change only, the supervisor polls far more often than state moves
fn set(app: &AppHandle, next: Health) {
    let daemon = app.state::<Daemon>();
    let mut health = daemon.health.lock().unwrap();
    if *health == next {
        return;
    }
    *health = next.clone();
    drop(health);

    let _ = app.emit(HEALTH_EVENT, next);
}

async fn supervise(app: AppHandle) {
    let client = reqwest::Client::builder()
        .timeout(PING_TIMEOUT)
        .build()
        .expect("a client with only a timeout set always builds");

    // adopt whatever is already listening, the user may have started it by hand
    let mut starting_since = match ping(&client).await {
        Some(version) => {
            set(&app, Health::Up { version });
            None
        }
        None => {
            start(&app);
            Some(Instant::now())
        }
    };

    loop {
        let starting = matches!(current(&app), Health::Starting);
        tokio::time::sleep(if starting { POLL_STARTING } else { POLL_UP }).await;

        // a child of ours exiting is definitive, so ask before paying for a request
        if let Some(status) = reap(&app) {
            set(
                &app,
                Health::Down {
                    reason: format!("daemon exited with {status}"),
                },
            );
            starting_since = None;
            continue;
        }

        match ping(&client).await {
            Some(version) => {
                set(&app, Health::Up { version });
                starting_since = None;
            }
            // a daemon started by hand while we were down gets adopted on the
            // next tick, so a dead one is only ever reported, never restarted
            None if starting_since.is_some_and(|at| at.elapsed() < START_TIMEOUT) => {}
            None => {
                set(
                    &app,
                    Health::Down {
                        reason: "not running".into(),
                    },
                );
                starting_since = None;
            }
        }
    }
}

#[tauri::command]
pub fn daemon_health(app: AppHandle) -> Health {
    current(&app)
}
