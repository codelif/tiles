mod daemon;
mod panel;
mod settings;
mod tray;

use tauri::{ActivationPolicy, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

fn main() {
    tauri::Builder::default()
        // has to be registered first, so a second copy exits before it builds a
        // status item of its own. nothing to focus, so the callback is empty
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            panel::hide_panel,
            panel::panel_ready,
            daemon::daemon_health
        ])
        .setup(|app| {
            // LSUIElement covers the launch window before this runs
            app.set_activation_policy(ActivationPolicy::Accessory);

            // before the tray, its menu reads the toggles
            settings::init(app.handle());
            panel::init(app.handle())?;
            tray::init(app.handle())?;
            daemon::init(app.handle());

            panel::warm_up(app.handle());

            Ok(())
        })
        .on_window_event(|window, event| {
            // has to stay a WindowEvent, nspanel's set_event_handler replaces
            // Tauri's NSWindowDelegate instead of chaining and kills this
            if matches!(event, WindowEvent::Focused(false)) && window.label() == panel::LABEL {
                let app = window.app_handle();
                let mode = if tray::pointer_over_item(app) {
                    panel::Dismiss::Fade
                } else {
                    panel::Dismiss::Instant
                };
                panel::dismiss(app, mode);
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to start the Tiles menu bar app");
}
