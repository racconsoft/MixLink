//! Derives the rule identity of an application stream.
use wavelink_core::rules::normalize_app;

/// Identity order: process binary → /proc/<pid>/comm → application.name → node.name.
pub fn app_identity(
    get: impl Fn(&str) -> Option<String>,
    read_comm: impl Fn(&str) -> Option<String>,
) -> String {
    get("application.process.binary")
        .or_else(|| get("application.process.id").and_then(|pid| read_comm(&pid)))
        .or_else(|| get("application.name"))
        .or_else(|| get("node.name"))
        .map(|s| normalize_app(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

pub fn read_proc_comm(pid: &str) -> Option<String> {
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn props(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    fn no_proc(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn prefers_process_binary() {
        let get = props(&[
            ("application.process.binary", "Discord"),
            ("application.name", "WEBRTC VoiceEngine"),
        ]);
        assert_eq!(app_identity(get, no_proc), "discord");
    }

    #[test]
    fn falls_back_to_proc_comm_for_native_clients() {
        let get = props(&[
            ("application.process.id", "4242"),
            ("application.name", "Some Name"),
        ]);
        let comm = |pid: &str| (pid == "4242").then(|| "pw-play".to_string());
        assert_eq!(app_identity(get, comm), "pw-play");
    }

    #[test]
    fn falls_back_to_application_then_node_name() {
        assert_eq!(
            app_identity(props(&[("application.name", "Firefox")]), no_proc),
            "firefox"
        );
        assert_eq!(
            app_identity(props(&[("node.name", "Game Audio")]), no_proc),
            "game audio"
        );
        assert_eq!(app_identity(props(&[]), no_proc), "unknown");
    }

    #[test]
    fn proc_comm_rejects_non_numeric_pids() {
        assert_eq!(read_proc_comm("../etc"), None);
        assert!(read_proc_comm(&std::process::id().to_string()).is_some());
    }
}
