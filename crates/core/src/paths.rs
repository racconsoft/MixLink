//! Locations of the daemon socket and the config file.
use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    socket_path_from(|k| std::env::var(k).ok())
}

pub fn config_path() -> PathBuf {
    config_path_from(|k| std::env::var(k).ok())
}

pub fn socket_path_from(env: impl Fn(&str) -> Option<String>) -> PathBuf {
    let env = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(path) = env("WAVELINK_SOCKET") {
        return path.into();
    }
    if let Some(dir) = env("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("wavelink.sock");
    }
    PathBuf::from(format!(
        "/tmp/wavelink-{}.sock",
        env("USER").unwrap_or_else(|| "user".into())
    ))
}

pub fn config_path_from(env: impl Fn(&str) -> Option<String>) -> PathBuf {
    let env = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(path) = env("WAVELINK_CONFIG") {
        return path.into();
    }
    let base = env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env("HOME").unwrap_or_else(|| ".".into())).join(".config")
        });
    base.join("wavelink-linux").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn socket_prefers_override_then_runtime_dir() {
        assert_eq!(
            socket_path_from(env(&[("WAVELINK_SOCKET", "/x.sock")])),
            PathBuf::from("/x.sock")
        );
        assert_eq!(
            socket_path_from(env(&[("XDG_RUNTIME_DIR", "/run/user/1000")])),
            PathBuf::from("/run/user/1000/wavelink.sock")
        );
        assert_eq!(
            socket_path_from(env(&[("USER", "bob")])),
            PathBuf::from("/tmp/wavelink-bob.sock")
        );
    }

    #[test]
    fn config_prefers_override_then_xdg_then_home() {
        assert_eq!(
            config_path_from(env(&[("WAVELINK_CONFIG", "/c.toml")])),
            PathBuf::from("/c.toml")
        );
        assert_eq!(
            config_path_from(env(&[("XDG_CONFIG_HOME", "/cfg"), ("HOME", "/home/a")])),
            PathBuf::from("/cfg/wavelink-linux/config.toml")
        );
        assert_eq!(
            config_path_from(env(&[("HOME", "/home/a"), ("XDG_CONFIG_HOME", "")])),
            PathBuf::from("/home/a/.config/wavelink-linux/config.toml")
        );
    }
}
