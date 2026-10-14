//! StatusNotifierItem tray icon via ksni (no libappindicator needed).
use std::sync::{Arc, LazyLock};

use ksni::TrayMethods;
use tauri::AppHandle;

use crate::window::{TrayState, TrayStatus, show_main};

struct MixLinkTray {
    app: AppHandle,
    status: Arc<TrayState>,
}

/// The tray pixmap, decoded and rotated RGBA → ARGB once and reused for every `icon_pixmap` call.
static ICON: LazyLock<ksni::Icon> = LazyLock::new(|| {
    let image = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-64.png"))
        .expect("tray icon is a PNG");
    let mut data = image.rgba().to_vec();
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel.rotate_right(1); // RGBA → ARGB
    }
    ksni::Icon {
        width: image.width() as i32,
        height: image.height() as i32,
        data,
    }
});

impl ksni::Tray for MixLinkTray {
    fn id(&self) -> String {
        "mixlink".into()
    }
    fn title(&self) -> String {
        "MixLink".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ICON.clone()]
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        show_main(&self.app);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Open MixLink".into(),
                activate: Box::new(|tray: &mut Self| show_main(&tray.app)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| tray.app.exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
    /// A tray host (StatusNotifierWatcher) appeared after us; ksni re-registers the icon.
    fn watcher_online(&self) {
        tracing::info!("tray host appeared; tray icon registered");
        self.status.set(TrayStatus::Available);
    }
    /// No tray host (yet): closing must quit, and the window must be reachable. Keep the
    /// service running so the icon appears once a host starts.
    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        tracing::warn!("no tray host ({reason:?}); closing the window will quit MixLink");
        self.status.set(TrayStatus::Unavailable);
        show_main(&self.app);
        true
    }
}

/// Registers the tray icon; keeps it alive for the process lifetime. Updates `status` once
/// registration settles; on failure, also shows the main window, in case it was hidden while
/// registration was still pending (see [`TrayStatus`]). With `assume_sni_available` a missing
/// tray host is reported through `watcher_offline` instead, and the icon appears when one starts.
pub async fn spawn(app: AppHandle, status: Arc<TrayState>) {
    let tray = MixLinkTray {
        app: app.clone(),
        status: status.clone(),
    };
    match tray.assume_sni_available(true).spawn().await {
        Ok(handle) => {
            status.settle_available();
            if status.get() == TrayStatus::Available {
                tracing::info!("tray icon registered");
            }
            std::mem::forget(handle);
        }
        Err(e) => {
            status.set(TrayStatus::Unavailable);
            tracing::warn!("no tray available ({e}); closing the window will quit MixLink");
            show_main(&app);
        }
    }
}
