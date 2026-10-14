mod bridge;

use std::sync::Arc;
use std::time::Duration;

use mixlink_core::paths;
use mixlink_desktop::link::DaemonLink;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

const RETRY: Duration = Duration::from_secs(1);

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let sink = Arc::new(bridge::TauriSink::new(app.handle().clone()));
            let link = DaemonLink::new();
            tauri::async_runtime::spawn(link.clone().run(
                paths::socket_path(),
                sink.clone(),
                RETRY,
            ));
            app.manage(link);
            app.manage(sink);
            let theme =
                mixlink_desktop::icons::kde_icon_theme(&mixlink_desktop::autostart::config_home())
                    .unwrap_or_else(|| "breeze-dark".into());
            app.manage(Arc::new(mixlink_desktop::icons::IconCache::new(theme)));
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
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
