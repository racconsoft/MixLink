//! App assignment rules and channel id generation.
use crate::model::Channel;

pub fn normalize_app(app: &str) -> String {
    app.trim().to_lowercase()
}

pub fn channel_for_app<'a>(channels: &'a [Channel], app: &str) -> Option<&'a Channel> {
    let app = normalize_app(app);
    channels.iter().find(|c| c.apps.contains(&app))
}

/// Assigns `app` to `channel_id`. An app belongs to at most one channel.
pub fn assign_app(channels: &mut [Channel], app: &str, channel_id: &str) {
    let app = normalize_app(app);
    for c in channels.iter_mut() {
        c.apps.retain(|a| *a != app);
        if c.id == channel_id {
            c.apps.push(app.clone());
        }
    }
}

pub fn unassign_app(channels: &mut [Channel], app: &str) -> bool {
    let app = normalize_app(app);
    let mut removed = false;
    for c in channels.iter_mut() {
        let before = c.apps.len();
        c.apps.retain(|a| *a != app);
        removed |= c.apps.len() != before;
    }
    removed
}

pub fn assigned_apps(channels: &[Channel]) -> Vec<String> {
    channels
        .iter()
        .flat_map(|c| c.apps.iter().cloned())
        .collect()
}

/// Lowercase ASCII id safe for PipeWire node names ("Voice Chat" → "voice-chat").
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().to_lowercase().chars() {
        match ch {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            c if c.is_ascii_alphanumeric() => out.push(c),
            _ => out.push('-'),
        }
    }
    let slug = out
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "channel".into()
    } else {
        slug
    }
}

pub fn unique_id(name: &str, existing: &[&str]) -> String {
    let base = slugify(name);
    if !existing.contains(&base.as_str()) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !existing.contains(&candidate.as_str()))
        .expect("an unused suffix always exists")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channels() -> Vec<Channel> {
        let mut music = Channel::new("music", "Music");
        music.apps = vec!["spotify".into()];
        vec![
            Channel::new("system", "System"),
            music,
            Channel::new("voice-chat", "Voice Chat"),
        ]
    }

    #[test]
    fn finds_channel_for_app_case_insensitively() {
        let cs = channels();
        assert_eq!(
            channel_for_app(&cs, " Spotify ").map(|c| c.id.as_str()),
            Some("music")
        );
        assert!(channel_for_app(&cs, "discord").is_none());
    }

    #[test]
    fn assign_moves_app_between_channels() {
        let mut cs = channels();
        assign_app(&mut cs, "Spotify", "voice-chat");
        assert!(cs[1].apps.is_empty());
        assert_eq!(cs[2].apps, vec!["spotify"]);
    }

    #[test]
    fn assign_twice_does_not_duplicate() {
        let mut cs = channels();
        assign_app(&mut cs, "spotify", "music");
        assert_eq!(cs[1].apps, vec!["spotify"]);
    }

    #[test]
    fn unassign_reports_whether_anything_changed() {
        let mut cs = channels();
        assert!(unassign_app(&mut cs, "SPOTIFY"));
        assert!(!unassign_app(&mut cs, "spotify"));
        assert!(assigned_apps(&cs).is_empty());
    }

    #[test]
    fn slugify_produces_node_safe_ids() {
        assert_eq!(slugify("Voice Chat"), "voice-chat");
        assert_eq!(slugify("  Größe / Ton!! "), "groesse-ton");
        assert_eq!(slugify("???"), "channel");
    }

    #[test]
    fn unique_id_appends_counter() {
        assert_eq!(unique_id("Music", &["system"]), "music");
        assert_eq!(unique_id("Music", &["music", "music-2"]), "music-3");
    }
}
