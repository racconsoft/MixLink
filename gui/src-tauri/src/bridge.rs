//! Connects the daemon link to the webview: events out, commands in.
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use mixlink_core::model::State as DaemonState;
use mixlink_core::protocol::{Command, ErrorBody, Event};
use mixlink_desktop::link::{DaemonLink, LinkSink};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

pub struct TauriSink {
    app: AppHandle,
    last_state: Mutex<Option<DaemonState>>,
    connected: AtomicBool,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            last_state: Mutex::new(None),
            connected: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: self.last_state.lock().unwrap().clone(),
            connected: self.connected.load(Ordering::Relaxed),
        }
    }
}

impl LinkSink for TauriSink {
    fn state(&self, state: &DaemonState) {
        *self.last_state.lock().unwrap() = Some(state.clone());
        let _ = self.app.emit("mixlink://state", state);
    }
    fn meters(&self, meters: &Event) {
        let _ = self.app.emit("mixlink://meters", meters);
    }
    fn connection(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
        let _ = self.app.emit("mixlink://connection", connected);
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    state: Option<DaemonState>,
    connected: bool,
}

#[tauri::command]
pub async fn send(
    link: tauri::State<'_, DaemonLink>,
    command: Command,
) -> Result<Value, ErrorBody> {
    link.send(command).await
}

#[tauri::command]
pub fn snapshot(sink: tauri::State<'_, std::sync::Arc<TauriSink>>) -> Snapshot {
    sink.snapshot()
}

#[tauri::command]
pub async fn start_service() -> Result<(), String> {
    let status = tokio::process::Command::new("systemctl")
        .args(["--user", "start", "mixlinkd"])
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("systemctl exited with {status}"))
    }
}

#[tauri::command]
pub async fn app_icon(
    icons: tauri::State<'_, std::sync::Arc<mixlink_desktop::icons::IconCache>>,
    app: String,
    icon_name: Option<String>,
) -> Result<Option<String>, ()> {
    let icons = icons.inner().clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || icons.get(&app, icon_name.as_deref()))
            .await
            .ok()
            .flatten(),
    )
}

#[tauri::command]
pub fn get_autostart() -> bool {
    mixlink_desktop::autostart::is_enabled(&mixlink_desktop::autostart::config_home())
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let exec = std::env::current_exe().map_err(|e| e.to_string())?;
    mixlink_desktop::autostart::set_enabled(
        &mixlink_desktop::autostart::config_home(),
        enabled,
        &exec,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "ui", "{message}"),
        "warn" => tracing::warn!(target: "ui", "{message}"),
        _ => tracing::info!(target: "ui", "{message}"),
    }
}
