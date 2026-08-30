mod panel;
mod tray;

use tauri::{ActivationPolicy, Manager, WindowEvent};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![
            panel::hide_panel,
            panel::panel_ready
        ])
        .setup(|app| {
            // LSUIElement covers the launch window before this runs
            app.set_activation_policy(ActivationPolicy::Accessory);

            panel::init(app.handle())?;
            tray::init(app.handle())?;

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
