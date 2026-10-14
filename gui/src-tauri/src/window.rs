//! Window visibility rules.
use std::sync::atomic::{AtomicU8, Ordering};

use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    Hide,
    Quit,
}

/// Tray registration status. `ksni::Tray::spawn` negotiates with the StatusNotifierWatcher
/// asynchronously, so there is a window on startup where we don't yet know whether a tray will
/// be available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    /// Registration hasn't finished yet; the tray may still appear.
    Pending,
    /// The tray registered successfully.
    Available,
    /// Registration failed; no tray will ever bring a hidden window back.
    Unavailable,
}

impl TrayStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => TrayStatus::Available,
            2 => TrayStatus::Unavailable,
            _ => TrayStatus::Pending,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            TrayStatus::Pending => 0,
            TrayStatus::Available => 1,
            TrayStatus::Unavailable => 2,
        }
    }
}

/// Shared, thread-safe cell holding the current [`TrayStatus`].
pub struct TrayState(AtomicU8);

impl TrayState {
    pub fn new() -> Self {
        Self(AtomicU8::new(TrayStatus::Pending.as_u8()))
    }

    pub fn set(&self, status: TrayStatus) {
        self.0.store(status.as_u8(), Ordering::Relaxed);
    }

    pub fn get(&self) -> TrayStatus {
        TrayStatus::from_u8(self.0.load(Ordering::Relaxed))
    }
}

impl Default for TrayState {
    fn default() -> Self {
        Self::new()
    }
}

/// While registration is pending we optimistically hide (the tray may still appear and bring the
/// window back); once we know there is no tray, closing must quit, since a hidden window could
/// never be reopened.
pub fn close_action(tray: TrayStatus) -> CloseAction {
    match tray {
        TrayStatus::Pending | TrayStatus::Available => CloseAction::Hide,
        TrayStatus::Unavailable => CloseAction::Quit,
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_hides_while_pending_or_available_and_quits_once_unavailable() {
        assert_eq!(close_action(TrayStatus::Pending), CloseAction::Hide);
        assert_eq!(close_action(TrayStatus::Available), CloseAction::Hide);
        assert_eq!(close_action(TrayStatus::Unavailable), CloseAction::Quit);
    }
}
