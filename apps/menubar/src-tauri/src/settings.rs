//! the toggles that outlive a run

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
// a file written by an older build is missing the newer keys
#[serde(default)]
pub struct Settings {
    pub autostart: bool,
    /// quitting the menu bar app stops the daemon too, read by the quit sequence
    pub daemon_tied: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            autostart: false,
            daemon_tied: true,
        }
    }
}

struct Store {
    path: PathBuf,
    settings: Mutex<Settings>,
}

pub fn init(app: &AppHandle) {
    let path = app
        .path()
        .app_config_dir()
        .expect("macOS always has an app config dir")
        .join("settings.json");

    // anything unreadable or malformed falls back to defaults rather than
    // blocking launch, the next write repairs the file
    let settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    app.manage(Store {
        path,
        settings: Mutex::new(settings),
    });

    reconcile_autostart(app);
}

/// the login item can be removed in System Settings without us hearing about
/// it, so the system wins and the file follows
fn reconcile_autostart(app: &AppHandle) {
    let Ok(enabled) = app.autolaunch().is_enabled() else {
        return;
    };
    if enabled != get(app).autostart {
        update(app, |s| s.autostart = enabled);
    }
}

pub fn set_autostart(app: &AppHandle, on: bool) -> bool {
    let launcher = app.autolaunch();
    let result = if on {
        launcher.enable()
    } else {
        launcher.disable()
    };
    if let Err(err) = &result {
        eprintln!("[settings] autostart {on} failed: {err}");
    }
    result.is_ok()
}

pub fn get(app: &AppHandle) -> Settings {
    *app.state::<Store>().settings.lock().unwrap()
}

pub fn update(app: &AppHandle, edit: impl FnOnce(&mut Settings)) {
    let store = app.state::<Store>();

    let mut settings = store.settings.lock().unwrap();
    edit(&mut settings);
    let snapshot = *settings;
    drop(settings);

    save(&store.path, &snapshot);
}

fn save(path: &Path, settings: &Settings) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(raw) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, raw);
    }
}
