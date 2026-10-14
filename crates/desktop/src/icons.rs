//! Finds application icons via .desktop files and the freedesktop icon theme.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine;

/// Application directories in XDG order, plus flatpak exports.
pub fn desktop_dirs() -> Vec<PathBuf> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let home = PathBuf::from(env("HOME").unwrap_or_default());
    let data_home = env("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let mut dirs = vec![data_home.join("applications")];
    let data_dirs = env("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(
        data_dirs
            .split(':')
            .filter(|d| !d.is_empty())
            .map(|d| Path::new(d).join("applications")),
    );
    dirs.push(data_home.join("flatpak/exports/share/applications"));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs
}

/// Values of the `[Desktop Entry]` group only.
fn entry_fields(text: &str) -> HashMap<String, String> {
    let mut in_entry = false;
    let mut fields = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry && let Some((k, v)) = line.split_once('=') {
            fields
                .entry(k.trim().to_owned())
                .or_insert_with(|| v.trim().to_owned());
        }
    }
    fields
}

/// Splits an Exec line into tokens, honoring desktop-entry quoting: a token starting with `"`
/// runs to the next `"`, so a quoted path may contain spaces.
fn exec_tokens(exec: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = exec.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut token = String::new();
        if c == '"' {
            chars.next();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                token.push(c);
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                token.push(c);
                chars.next();
            }
        }
        tokens.push(token);
    }
    tokens
}

/// Basename of the executable in an Exec line, skipping `env VAR=… ` prefixes.
fn exec_binary(exec: &str) -> Option<String> {
    let mut tokens = exec_tokens(exec).into_iter();
    let mut first = tokens.next()?;
    if first == "env" {
        first = tokens.find(|p| !p.contains('='))?;
    }
    Path::new(&first)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
}

pub fn icon_name_from_desktop(app: &str, dirs: &[PathBuf]) -> Option<String> {
    let app = app.to_lowercase();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "desktop"))
            .collect();
        files.sort();
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let fields = entry_fields(&text);
            let Some(icon) = fields.get("Icon") else {
                continue;
            };
            let stem = file
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let matches = stem == app
                || stem.rsplit('.').next() == Some(app.as_str())
                || fields
                    .get("StartupWMClass")
                    .is_some_and(|c| c.to_lowercase() == app)
                || fields
                    .get("Exec")
                    .and_then(|e| exec_binary(e))
                    .is_some_and(|b| b == app);
            if matches {
                return Some(icon.clone());
            }
        }
    }
    None
}

pub fn resolve_icon(
    app: &str,
    icon_name: Option<&str>,
    dirs: &[PathBuf],
    lookup: impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(found) = icon_name.and_then(&lookup) {
        return Some(found);
    }
    if let Some(name) = icon_name_from_desktop(app, dirs) {
        let path = Path::new(&name);
        if path.is_absolute() {
            if path.exists() {
                return Some(path.to_path_buf());
            }
        } else if let Some(found) = lookup(&name) {
            return Some(found);
        }
    }
    lookup(app)
}

pub fn data_url(path: &Path) -> Option<String> {
    let mime = match path.extension()?.to_str()? {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn kde_icon_theme(config_home: &Path) -> Option<String> {
    let text = std::fs::read_to_string(config_home.join("kdeglobals")).ok()?;
    let mut in_icons = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_icons = line == "[Icons]";
        } else if in_icons && let Some(theme) = line.strip_prefix("Theme=") {
            return Some(theme.trim().to_owned());
        }
    }
    None
}

/// Caches resolved icons per app (as data URLs) for the session.
pub struct IconCache {
    theme: String,
    dirs: Vec<PathBuf>,
    cache: Mutex<HashMap<String, Option<String>>>,
}

impl IconCache {
    pub fn new(theme: String) -> Self {
        Self {
            theme,
            dirs: desktop_dirs(),
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, app: &str, icon_name: Option<&str>) -> Option<String> {
        if let Some(hit) = self.cache.lock().unwrap().get(app) {
            return hit.clone();
        }
        let lookup = |name: &str| {
            freedesktop_icons::lookup(name)
                .with_theme(&self.theme)
                .with_size(48)
                .find()
        };
        let url = resolve_icon(app, icon_name, &self.dirs, lookup).and_then(|p| data_url(&p));
        self.cache
            .lock()
            .unwrap()
            .insert(app.to_owned(), url.clone());
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn finds_icon_by_file_name_wm_class_or_exec() {
        let d = tempfile::tempdir().unwrap();
        let apps = d.path().join("applications");
        write(
            &apps,
            "firefox.desktop",
            "[Desktop Entry]\nName=Firefox\nExec=/usr/lib/firefox/firefox %u\nIcon=firefox\n",
        );
        write(
            &apps,
            "com.discordapp.Discord.desktop",
            "[Desktop Entry]\nExec=/usr/bin/Discord\nStartupWMClass=discord\nIcon=discord\n",
        );
        write(
            &apps,
            "spotify-launcher.desktop",
            "[Desktop Entry]\nExec=env LD_PRELOAD= spotify --uri=%U\nIcon=spotify-client\n[Desktop Action New]\nIcon=wrong\n",
        );
        let dirs = vec![apps];
        assert_eq!(
            icon_name_from_desktop("firefox", &dirs).as_deref(),
            Some("firefox")
        );
        assert_eq!(
            icon_name_from_desktop("discord", &dirs).as_deref(),
            Some("discord")
        );
        assert_eq!(
            icon_name_from_desktop("spotify", &dirs).as_deref(),
            Some("spotify-client")
        );
        assert_eq!(icon_name_from_desktop("pw-play", &dirs), None);
    }

    #[test]
    fn finds_icon_for_quoted_exec_paths_with_spaces() {
        let d = tempfile::tempdir().unwrap();
        let apps = d.path().join("applications");
        write(
            &apps,
            "my-app.desktop",
            "[Desktop Entry]\nExec=\"/opt/My App/My App\" %U\nIcon=my-app\n",
        );
        write(
            &apps,
            "tool-x.desktop",
            "[Desktop Entry]\nExec=env FOO=1 \"/opt/Tool X/tool\" --x\nIcon=tool-x\n",
        );
        let dirs = vec![apps];
        assert_eq!(
            icon_name_from_desktop("my app", &dirs).as_deref(),
            Some("my-app")
        );
        assert_eq!(
            icon_name_from_desktop("tool", &dirs).as_deref(),
            Some("tool-x")
        );
    }

    #[test]
    fn resolve_prefers_stream_icon_then_desktop_then_identity() {
        let d = tempfile::tempdir().unwrap();
        let apps = d.path().join("applications");
        write(
            &apps,
            "obs.desktop",
            "[Desktop Entry]\nExec=obs\nIcon=com.obsproject.Studio\n",
        );
        let dirs = vec![apps];
        let lookup =
            |name: &str| (name != "missing").then(|| PathBuf::from(format!("/icons/{name}.svg")));
        assert_eq!(
            resolve_icon("obs", Some("custom"), &dirs, lookup),
            Some(PathBuf::from("/icons/custom.svg"))
        );
        assert_eq!(
            resolve_icon("obs", None, &dirs, lookup),
            Some(PathBuf::from("/icons/com.obsproject.Studio.svg"))
        );
        assert_eq!(
            resolve_icon("vlc", Some("missing"), &dirs, lookup),
            Some(PathBuf::from("/icons/vlc.svg"))
        );
        assert_eq!(resolve_icon("missing", None, &dirs, |_| None), None);
    }

    #[test]
    fn absolute_icon_paths_in_desktop_files_are_used_directly() {
        let d = tempfile::tempdir().unwrap();
        let png = write(d.path(), "app.png", "png-bytes");
        let apps = d.path().join("applications");
        write(
            &apps,
            "tool.desktop",
            &format!("[Desktop Entry]\nExec=tool\nIcon={}\n", png.display()),
        );
        assert_eq!(resolve_icon("tool", None, &[apps], |_| None), Some(png));
    }

    #[test]
    fn data_urls_carry_the_mime_type() {
        let d = tempfile::tempdir().unwrap();
        let svg = write(d.path(), "a.svg", "<svg/>");
        let png = write(d.path(), "a.png", "x");
        let xpm = write(d.path(), "a.xpm", "x");
        assert_eq!(
            data_url(&svg).unwrap(),
            "data:image/svg+xml;base64,PHN2Zy8+"
        );
        assert!(
            data_url(&png)
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert_eq!(data_url(&xpm), None);
    }

    #[test]
    fn reads_the_kde_icon_theme() {
        let d = tempfile::tempdir().unwrap();
        write(
            d.path(),
            "kdeglobals",
            "[General]\nColorScheme=BreezeDark\n\n[Icons]\nTheme=breeze-dark\n",
        );
        assert_eq!(kde_icon_theme(d.path()).as_deref(), Some("breeze-dark"));
        assert_eq!(kde_icon_theme(&d.path().join("nope")), None);
    }
}
