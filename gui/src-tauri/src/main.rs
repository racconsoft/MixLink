mod bridge;
mod tray;
mod window;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use mixlink_core::paths;
use mixlink_desktop::autostart;
use mixlink_desktop::link::DaemonLink;
use tauri::{Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;
use tracing_subscriber::EnvFilter;
use window::TrayState;

const RETRY: Duration = Duration::from_secs(1);

/// Cargo build output always lives under a `target/` directory. Autostart should only ever point
/// at an installed binary; wiring it up for a throwaway build-tree artifact would create a
/// broken or stale autostart entry once that build directory is cleaned or rebuilt elsewhere.
fn is_build_tree(exe: &Path) -> bool {
    exe.components().any(|c| c.as_os_str() == "target")
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let hidden = std::env::args().any(|a| a == "--hidden");
    let tray_state = Arc::new(TrayState::new());
    let close_state = tray_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("second instance started; showing the window");
            window::show_main(app);
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .setup(move |app| {
            let sink = Arc::new(bridge::TauriSink::new(app.handle().clone()));
            let link = DaemonLink::new();
            tauri::async_runtime::spawn(link.clone().run(
                paths::socket_path(),
                sink.clone(),
                RETRY,
            ));
            app.manage(link);
            app.manage(sink);
            let theme = mixlink_desktop::icons::kde_icon_theme(&autostart::config_home())
                .unwrap_or_else(|| "breeze-dark".into());
            app.manage(Arc::new(mixlink_desktop::icons::IconCache::new(theme)));

            if let Ok(exec) = std::env::current_exe() {
                if is_build_tree(&exec) {
                    tracing::info!("running from a build tree; skipping first-run autostart");
                } else {
                    match autostart::first_run_init(&autostart::config_home(), &exec) {
                        Ok(true) => tracing::info!("first start: enabled autostart"),
                        Ok(false) => {}
                        Err(e) => tracing::warn!("could not set up autostart: {e}"),
                    }
                }
            }
            tauri::async_runtime::spawn(tray::spawn(app.handle().clone(), tray_state.clone()));
            if !hidden {
                window::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(move |window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window::close_action(close_state.get()) == window::CloseAction::Hide
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            bridge::send,
            bridge::snapshot,
            bridge::start_service,
            bridge::log_frontend,
            bridge::app_icon,
            bridge::get_autostart,
            bridge::set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("running the MixLink GUI");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tree_executables_are_recognized_by_a_target_component() {
        assert!(is_build_tree(Path::new(
            "/home/u/dev/x/target/release/mixlink-gui"
        )));
        assert!(!is_build_tree(Path::new("/home/u/.local/bin/mixlink-gui")));
    }
}
