//! XDG autostart entry for the GUI and the first-run marker.
use std::io;
use std::path::{Path, PathBuf};

pub const DESKTOP_FILE: &str = "mixlink-gui.desktop";
const MARKER: &str = "mixlink/gui-initialized";

pub fn config_home() -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env("HOME").unwrap_or_else(|| ".".into())).join(".config"))
}

pub fn autostart_path(config_home: &Path) -> PathBuf {
    config_home.join("autostart").join(DESKTOP_FILE)
}

pub fn is_enabled(config_home: &Path) -> bool {
    autostart_path(config_home).exists()
}

fn entry(exec: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=MixLink\nComment=Audio mixer for PipeWire\nExec=\"{}\" --hidden\nIcon=mixlink\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec.display()
    )
}

pub fn set_enabled(config_home: &Path, enabled: bool, exec: &Path) -> io::Result<()> {
    let path = autostart_path(config_home);
    if enabled {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, entry(exec))
    } else {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

/// On the very first GUI start: enable autostart and create the marker. Returns true then.
pub fn first_run_init(config_home: &Path, exec: &Path) -> io::Result<bool> {
    let marker = config_home.join(MARKER);
    if marker.exists() {
        return Ok(false);
    }
    set_enabled(config_home, true, exec)?;
    if let Some(dir) = marker.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(marker, "")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_disable_roundtrip() {
        let home = tempfile::tempdir().unwrap();
        let exec = Path::new("/home/u/.local/bin/mixlink-gui");
        assert!(!is_enabled(home.path()));
        set_enabled(home.path(), true, exec).unwrap();
        assert!(is_enabled(home.path()));
        let text = std::fs::read_to_string(autostart_path(home.path())).unwrap();
        assert!(text.contains("Exec=\"/home/u/.local/bin/mixlink-gui\" --hidden"));
        assert!(text.contains("Icon=mixlink"));
        set_enabled(home.path(), false, exec).unwrap();
        assert!(!is_enabled(home.path()));
        set_enabled(home.path(), false, exec).unwrap(); // idempotent
    }

    #[test]
    fn first_run_enables_once() {
        let home = tempfile::tempdir().unwrap();
        let exec = Path::new("/bin/mixlink-gui");
        assert!(first_run_init(home.path(), exec).unwrap());
        assert!(is_enabled(home.path()));
        set_enabled(home.path(), false, exec).unwrap();
        assert!(
            !first_run_init(home.path(), exec).unwrap(),
            "second start does not re-enable"
        );
        assert!(!is_enabled(home.path()));
    }
}
