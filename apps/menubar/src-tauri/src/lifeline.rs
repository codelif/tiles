//! dying with the daemon
//!
//! the daemon hands us a stdin pipe it never writes to and holds the other end
//! for as long as it lives. reading it blocks forever, and the read only ever
//! returns when that end is gone, which covers a kill the daemon never saw
//! coming as much as an orderly shutdown

use std::io::Read;

use tauri::AppHandle;

/// set by the daemon, absent when the app was launched by hand
const SUPERVISED: &str = "TILES_MENUBAR_SUPERVISED";

pub fn init(app: &AppHandle) {
    if std::env::var_os(SUPERVISED).is_none() {
        return;
    }

    let app = app.clone();
    std::thread::spawn(move || {
        let mut byte = [0u8; 1];
        // the daemon writes nothing, so anything but a blocking wait is a bug
        // upstream and still means the pipe is no good to us
        let _ = std::io::stdin().read(&mut byte);
        app.exit(0);
    });
}
