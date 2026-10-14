//! Pure model of the PipeWire graph and the links the daemon wants.
use std::collections::HashMap;

use wavelink_core::model::{DeviceKind, NODE_PREFIX};

pub const MIXER_NODE: &str = "wavelink.mixer";
pub const STREAM_MIX_NODE: &str = "wavelink.stream_mix";
pub const MONITOR_OUT: [&str; 2] = ["monitor_FL", "monitor_FR"];
pub const STREAM_OUT: [&str; 2] = ["stream_FL", "stream_FR"];

pub fn mixer_input_port(slot: usize, channel: &str) -> String {
    format!("in_{:02}_{channel}", slot + 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeInfo {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub media_class: String,
}

impl NodeInfo {
    pub fn device_kind(&self) -> Option<DeviceKind> {
        if self.name.starts_with(NODE_PREFIX) {
            return None;
        }
        match self.media_class.as_str() {
            "Audio/Sink" => Some(DeviceKind::Sink),
            "Audio/Source" => Some(DeviceKind::Source),
            _ => None,
        }
    }

    pub fn is_app_stream(&self) -> bool {
        self.media_class == "Stream/Output/Audio" && !self.name.starts_with(NODE_PREFIX)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortInfo {
    pub id: u32,
    pub node: u32,
    pub name: String,
    pub direction: Direction,
    /// `audio.channel` (FL, FR, MONO, …).
    pub channel: String,
    pub monitor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkInfo {
    pub id: u32,
    pub output_node: u32,
    pub output_port: u32,
    pub input_node: u32,
    pub input_port: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Removed {
    Node(NodeInfo),
    Port(PortInfo),
    Link(LinkInfo),
    Unknown,
}

pub fn parse_node(id: u32, get: impl Fn(&str) -> Option<String>) -> Option<NodeInfo> {
    let name = get("node.name")?;
    let description = get("node.description").or_else(|| get("node.nick")).unwrap_or_else(|| name.clone());
    Some(NodeInfo { id, name, description, media_class: get("media.class").unwrap_or_default() })
}

pub fn parse_port(id: u32, get: impl Fn(&str) -> Option<String>) -> Option<PortInfo> {
    let node = get("node.id")?.parse().ok()?;
    let name = get("port.name")?;
    let direction = match get("port.direction")?.as_str() {
        "in" => Direction::In,
        "out" => Direction::Out,
        _ => return None,
    };
    let channel = get("audio.channel").unwrap_or_else(|| name.rsplit('_').next().unwrap_or_default().to_owned());
    let monitor = get("port.monitor").as_deref() == Some("true");
    Some(PortInfo { id, node, name, direction, channel, monitor })
}

pub fn parse_link(id: u32, get: impl Fn(&str) -> Option<String>) -> Option<LinkInfo> {
    let num = |k: &str| get(k).and_then(|v| v.parse().ok());
    Some(LinkInfo {
        id,
        output_node: num("link.output.node")?,
        output_port: num("link.output.port")?,
        input_node: num("link.input.node")?,
        input_port: num("link.input.port")?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Monitor ports of a (virtual) sink.
    SinkMonitor,
    /// Capture ports of a source device.
    Source,
}

#[derive(Debug, Clone)]
pub struct ChannelLink<'a> {
    pub slot: usize,
    pub node: &'a str,
    pub kind: SourceKind,
}

#[derive(Debug, Clone)]
pub struct LinkPlan<'a> {
    pub mixer: &'a str,
    pub stream_mix: &'a str,
    pub monitor_output: Option<&'a str>,
    pub channels: Vec<ChannelLink<'a>>,
}

#[derive(Debug, Default)]
pub struct Graph {
    nodes: HashMap<u32, NodeInfo>,
    ports: HashMap<u32, PortInfo>,
    links: HashMap<u32, LinkInfo>,
}

impl Graph {
    pub fn add_node(&mut self, node: NodeInfo) {
        self.nodes.insert(node.id, node);
    }

    pub fn add_port(&mut self, port: PortInfo) {
        self.ports.insert(port.id, port);
    }

    pub fn add_link(&mut self, link: LinkInfo) {
        self.links.insert(link.id, link);
    }

    pub fn remove(&mut self, id: u32) -> Removed {
        if let Some(node) = self.nodes.remove(&id) {
            return Removed::Node(node);
        }
        if let Some(port) = self.ports.remove(&id) {
            return Removed::Port(port);
        }
        if let Some(link) = self.links.remove(&id) {
            return Removed::Link(link);
        }
        Removed::Unknown
    }

    /// Drops a node we are destroying ourselves, before PipeWire confirms it.
    pub fn forget_node_named(&mut self, name: &str) {
        self.nodes.retain(|_, n| n.name != name);
    }

    pub fn node_by_name(&self, name: &str) -> Option<&NodeInfo> {
        self.nodes.values().find(|n| n.name == name)
    }

    pub fn port(&self, id: u32) -> Option<&PortInfo> {
        self.ports.get(&id)
    }

    pub fn has_link(&self, output_port: u32, input_port: u32) -> bool {
        self.links.values().any(|l| l.output_port == output_port && l.input_port == input_port)
    }

    /// Name of the node `output_node` is currently linked to.
    pub fn link_target(&self, output_node: u32) -> Option<&str> {
        self.links
            .values()
            .find(|l| l.output_node == output_node)
            .and_then(|l| self.nodes.get(&l.input_node))
            .map(|n| n.name.as_str())
    }

    pub fn desired_links(&self, plan: &LinkPlan) -> Vec<(u32, u32)> {
        let Some(mixer) = self.node_by_name(plan.mixer) else { return Vec::new() };
        let mut out = Vec::new();
        for channel in &plan.channels {
            let Some(source) = self.node_by_name(channel.node) else { continue };
            let sources = self.ports_where(source.id, Direction::Out, |p| match channel.kind {
                SourceKind::SinkMonitor => p.monitor,
                SourceKind::Source => !p.monitor,
            });
            let inputs: Vec<&PortInfo> =
                ["FL", "FR"].iter().filter_map(|c| self.port_named(mixer.id, &mixer_input_port(channel.slot, c))).collect();
            out.extend(pair_ports(&sources, &inputs));
        }
        if let Some(device) = plan.monitor_output.and_then(|name| self.node_by_name(name)) {
            let outs: Vec<&PortInfo> = MONITOR_OUT.iter().filter_map(|n| self.port_named(mixer.id, n)).collect();
            let ins = self.ports_where(device.id, Direction::In, |p| !p.monitor);
            out.extend(pair_ports(&outs, &ins));
        }
        if let Some(stream_mix) = self.node_by_name(plan.stream_mix) {
            let outs: Vec<&PortInfo> = STREAM_OUT.iter().filter_map(|n| self.port_named(mixer.id, n)).collect();
            let ins = self.ports_where(stream_mix.id, Direction::In, |_| true);
            out.extend(pair_ports(&outs, &ins));
        }
        out
    }

    fn port_named(&self, node: u32, name: &str) -> Option<&PortInfo> {
        self.ports.values().find(|p| p.node == node && p.name == name)
    }

    fn ports_where(&self, node: u32, direction: Direction, keep: impl Fn(&PortInfo) -> bool) -> Vec<&PortInfo> {
        let mut ports: Vec<&PortInfo> =
            self.ports.values().filter(|p| p.node == node && p.direction == direction && keep(p)).collect();
        ports.sort_by(|a, b| a.name.cmp(&b.name));
        ports
    }
}

/// Pairs output with input ports. Priority: channel-name matches only (if any
/// input matches); else a single input (mono sink) gets every output; else a
/// single output (mono source) feeds every input; else by position.
pub fn pair_ports(outs: &[&PortInfo], ins: &[&PortInfo]) -> Vec<(u32, u32)> {
    let mut outs: Vec<&PortInfo> = outs.to_vec();
    outs.sort_by(|a, b| a.name.cmp(&b.name));
    let mut ins: Vec<&PortInfo> = ins.to_vec();
    ins.sort_by(|a, b| a.name.cmp(&b.name));

    let channel_matches: Vec<(u32, u32)> = ins
        .iter()
        .filter_map(|input| outs.iter().find(|o| o.channel == input.channel).map(|output| (output.id, input.id)))
        .collect();
    if !channel_matches.is_empty() {
        return channel_matches;
    }
    if let [input] = ins[..] {
        return outs.iter().map(|output| (output.id, input.id)).collect();
    }
    if let [output] = outs[..] {
        return ins.iter().map(|input| (output.id, input.id)).collect();
    }
    ins.iter().enumerate().filter_map(|(index, input)| outs.get(index).map(|output| (output.id, input.id))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(id: u32, node: u32, name: &str, direction: Direction, monitor: bool) -> PortInfo {
        let channel = name.rsplit('_').next().unwrap().to_owned();
        PortInfo { id, node, name: name.into(), direction, channel, monitor }
    }

    fn node(id: u32, name: &str, class: &str) -> NodeInfo {
        NodeInfo { id, name: name.into(), description: name.into(), media_class: class.into() }
    }

    /// mixer(10), channel sink(20), stream mix(30), headphones(40), xlr mic(50)
    fn graph() -> Graph {
        let mut g = Graph::default();
        g.add_node(node(10, MIXER_NODE, ""));
        for slot in 0..2 {
            for (i, ch) in ["FL", "FR"].iter().enumerate() {
                g.add_port(port(100 + slot as u32 * 2 + i as u32, 10, &mixer_input_port(slot, ch), Direction::In, false));
            }
        }
        for (i, name) in MONITOR_OUT.iter().chain(STREAM_OUT.iter()).enumerate() {
            g.add_port(port(110 + i as u32, 10, name, Direction::Out, false));
        }
        g.add_node(node(20, "wavelink.channel.music", "Audio/Sink"));
        g.add_port(port(200, 20, "playback_FL", Direction::In, false));
        g.add_port(port(201, 20, "playback_FR", Direction::In, false));
        g.add_port(port(202, 20, "monitor_FL", Direction::Out, true));
        g.add_port(port(203, 20, "monitor_FR", Direction::Out, true));
        g.add_node(node(30, STREAM_MIX_NODE, "Audio/Source/Virtual"));
        g.add_port(port(300, 30, "input_FL", Direction::In, false));
        g.add_port(port(301, 30, "input_FR", Direction::In, false));
        g.add_port(port(302, 30, "capture_FL", Direction::Out, false));
        g.add_port(port(303, 30, "capture_FR", Direction::Out, false));
        g.add_node(node(40, "alsa_output.headphones", "Audio/Sink"));
        g.add_port(port(400, 40, "playback_FL", Direction::In, false));
        g.add_port(port(401, 40, "playback_FR", Direction::In, false));
        g.add_port(port(402, 40, "monitor_FL", Direction::Out, true));
        g.add_port(port(403, 40, "monitor_FR", Direction::Out, true));
        g.add_node(node(50, "alsa_input.xlr", "Audio/Source"));
        g.add_port(port(500, 50, "capture_MONO", Direction::Out, false));
        g
    }

    fn plan(monitor_output: Option<&str>) -> LinkPlan<'_> {
        LinkPlan {
            mixer: MIXER_NODE,
            stream_mix: STREAM_MIX_NODE,
            monitor_output,
            channels: vec![
                ChannelLink { slot: 0, node: "wavelink.channel.music", kind: SourceKind::SinkMonitor },
                ChannelLink { slot: 1, node: "alsa_input.xlr", kind: SourceKind::Source },
            ],
        }
    }

    #[test]
    fn desired_links_cover_channels_monitor_and_stream() {
        let mut links = graph().desired_links(&plan(Some("alsa_output.headphones")));
        links.sort();
        assert_eq!(
            links,
            vec![
                (110, 400), (111, 401),             // monitor_FL/FR → headphones
                (112, 300), (113, 301),             // stream_FL/FR → stream mix
                (202, 100), (203, 101),             // music monitor → in_01
                (500, 102), (500, 103),             // mono mic → in_02 FL+FR
            ]
        );
    }

    #[test]
    fn absent_monitor_device_only_drops_monitor_links() {
        let links = graph().desired_links(&plan(Some("alsa_output.unplugged")));
        assert!(!links.iter().any(|(o, _)| *o == 110 || *o == 111));
        assert!(links.contains(&(112, 300)));
        let links = graph().desired_links(&plan(None));
        assert_eq!(links.len(), 6);
    }

    #[test]
    fn device_appearing_later_gets_linked() {
        let mut g = graph();
        g.remove(40);
        assert!(!g.desired_links(&plan(Some("alsa_output.headphones"))).contains(&(110, 400)));
        g.add_node(node(41, "alsa_output.headphones", "Audio/Sink"));
        g.add_port(port(410, 41, "playback_FL", Direction::In, false));
        g.add_port(port(411, 41, "playback_FR", Direction::In, false));
        assert!(g.desired_links(&plan(Some("alsa_output.headphones"))).contains(&(110, 410)));
    }

    #[test]
    fn no_mixer_means_no_links() {
        let mut g = graph();
        g.remove(10);
        assert!(g.desired_links(&plan(Some("alsa_output.headphones"))).is_empty());
    }

    #[test]
    fn pair_ports_matches_channels_mono_and_index() {
        let fl = port(1, 1, "out_FL", Direction::Out, false);
        let fr = port(2, 1, "out_FR", Direction::Out, false);
        let in_fl = port(3, 2, "in_FL", Direction::In, false);
        let in_fr = port(4, 2, "in_FR", Direction::In, false);
        assert_eq!(pair_ports(&[&fr, &fl], &[&in_fl, &in_fr]), vec![(1, 3), (2, 4)]);
        let mono = port(5, 1, "capture_MONO", Direction::Out, false);
        assert_eq!(pair_ports(&[&mono], &[&in_fl, &in_fr]), vec![(5, 3), (5, 4)]);
        let a = port(6, 1, "out_AUX0", Direction::Out, false);
        let b = port(7, 1, "out_AUX1", Direction::Out, false);
        assert_eq!(pair_ports(&[&a, &b], &[&in_fl, &in_fr]), vec![(6, 3), (7, 4)]);
    }

    #[test]
    fn pair_ports_surround_sink_only_links_matching_channels() {
        let out_fl = port(1, 1, "out_FL", Direction::Out, false);
        let out_fr = port(2, 1, "out_FR", Direction::Out, false);
        let in_fc = port(10, 2, "playback_FC", Direction::In, false);
        let in_fl = port(11, 2, "playback_FL", Direction::In, false);
        let in_fr = port(12, 2, "playback_FR", Direction::In, false);
        let in_lfe = port(13, 2, "playback_LFE", Direction::In, false);
        let in_rl = port(14, 2, "playback_RL", Direction::In, false);
        let in_rr = port(15, 2, "playback_RR", Direction::In, false);
        let mut links =
            pair_ports(&[&out_fl, &out_fr], &[&in_fc, &in_fl, &in_fr, &in_lfe, &in_rl, &in_rr]);
        links.sort();
        assert_eq!(links, vec![(1, 11), (2, 12)]);
    }

    #[test]
    fn pair_ports_single_mono_input_gets_every_output() {
        let out_fl = port(1, 1, "out_FL", Direction::Out, false);
        let out_fr = port(2, 1, "out_FR", Direction::Out, false);
        let in_mono = port(20, 2, "playback_MONO", Direction::In, false);
        let mut links = pair_ports(&[&out_fl, &out_fr], &[&in_mono]);
        links.sort();
        assert_eq!(links, vec![(1, 20), (2, 20)]);
    }

    #[test]
    fn link_target_and_removal() {
        let mut g = graph();
        g.add_node(node(60, "Firefox", "Stream/Output/Audio"));
        g.add_link(LinkInfo { id: 70, output_node: 60, output_port: 600, input_node: 20, input_port: 200 });
        assert_eq!(g.link_target(60), Some("wavelink.channel.music"));
        assert!(g.has_link(600, 200));
        assert!(matches!(g.remove(70), Removed::Link(_)));
        assert_eq!(g.link_target(60), None);
        assert!(matches!(g.remove(500), Removed::Port(_)));
        assert!(matches!(g.remove(9999), Removed::Unknown));
    }

    #[test]
    fn forget_node_named_hides_it_immediately() {
        let mut g = graph();
        g.forget_node_named("wavelink.channel.music");
        assert!(g.node_by_name("wavelink.channel.music").is_none());
        assert!(matches!(g.remove(20), Removed::Unknown));
    }

    #[test]
    fn parsing_globals() {
        let get = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string())
        };
        let p = parse_port(9, get(&[("node.id", "4"), ("port.name", "monitor_FR"), ("port.direction", "out"), ("port.monitor", "true")])).unwrap();
        assert_eq!((p.node, p.channel.as_str(), p.direction, p.monitor), (4, "FR", Direction::Out, true));
        let n = parse_node(4, get(&[("node.name", "alsa_output.x"), ("node.nick", "Nick"), ("media.class", "Audio/Sink")])).unwrap();
        assert_eq!(n.description, "Nick");
        assert_eq!(n.device_kind(), Some(DeviceKind::Sink));
        let ours = parse_node(5, get(&[("node.name", "wavelink.channel.music"), ("media.class", "Audio/Sink")])).unwrap();
        assert_eq!(ours.device_kind(), None);
        let s = parse_node(6, get(&[("node.name", "Firefox"), ("media.class", "Stream/Output/Audio")])).unwrap();
        assert!(s.is_app_stream());
        let l = parse_link(7, get(&[("link.output.node", "1"), ("link.output.port", "2"), ("link.input.node", "3"), ("link.input.port", "4")])).unwrap();
        assert_eq!((l.output_port, l.input_node), (2, 3));
        assert!(parse_port(8, get(&[("port.name", "x")])).is_none());
    }
}
