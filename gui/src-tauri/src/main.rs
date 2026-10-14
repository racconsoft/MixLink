mod bridge;
mod tray;
mod window;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mixlink_core::paths;
use mixlink_desktop::autostart;
use mixlink_desktop::link::DaemonLink;
use tauri::{Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;
use tracing_subscriber::EnvFilter;

const RETRY: Duration = Duration::from_secs(1);

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let hidden = std::env::args().any(|a| a == "--hidden");
    let tray_available = Arc::new(AtomicBool::new(false));
    let close_flag = tray_available.clone();

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
                match autostart::first_run_init(&autostart::config_home(), &exec) {
                    Ok(true) => tracing::info!("first start: enabled autostart"),
                    Ok(false) => {}
                    Err(e) => tracing::warn!("could not set up autostart: {e}"),
                }
            }
            tauri::async_runtime::spawn(tray::spawn(app.handle().clone(), tray_available.clone()));
            if !hidden {
                window::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(move |window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window::close_action(close_flag.load(Ordering::Relaxed))
                    == window::CloseAction::Hide
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
