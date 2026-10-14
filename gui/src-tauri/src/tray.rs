//! StatusNotifierItem tray icon via ksni (no libappindicator needed).
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ksni::TrayMethods;
use tauri::AppHandle;

use crate::window::show_main;

struct MixLinkTray {
    app: AppHandle,
}

fn icon() -> ksni::Icon {
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
}

impl ksni::Tray for MixLinkTray {
    fn id(&self) -> String {
        "mixlink".into()
    }
    fn title(&self) -> String {
        "MixLink".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![icon()]
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
}

/// Registers the tray icon; keeps it alive for the process lifetime.
pub async fn spawn(app: AppHandle, available: Arc<AtomicBool>) {
    match (MixLinkTray { app }).spawn().await {
        Ok(handle) => {
            available.store(true, Ordering::Relaxed);
            tracing::info!("tray icon registered");
            std::mem::forget(handle);
        }
        Err(e) => tracing::warn!("no tray available ({e}); closing the window will quit MixLink"),
    }
}
