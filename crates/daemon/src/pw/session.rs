//! One connection to PipeWire: tracks the graph, owns our nodes and links,
//! executes BackendCommands and reports BackendEvents.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mixlink_core::model::{ChannelId, Device, channel_node_name};
use pipewire as pw;
use pw::properties::properties;
use pw::registry::GlobalObject;
use pw::spa::utils::dict::DictRef;
use pw::spa::utils::result::AsyncSeq;
use pw::types::ObjectType;
use tokio::sync::mpsc::UnboundedSender;

use super::graph::{
    self, ChannelLink, Graph, LinkPlan, MIXER_NODE, Removed, STREAM_MIX_NODE, SourceKind,
};
use super::identity::{app_identity, read_proc_comm};
use super::mixer_node::MixerNode;
use super::nodes::create_null_sink;
use crate::backend::{BackendCommand, BackendEvent, ChannelSource, StreamInfo};
use crate::mixer::shared::SharedParams;

pub enum SessionEnd {
    Shutdown,
    Disconnected,
    ConnectFailed(String),
}

const DEFAULT_SINK_KEY: &str = "default.configured.audio.sink";
const METER_INTERVAL: Duration = Duration::from_millis(33);
/// How long startup waits for WirePlumber to publish the `default` metadata.
const METADATA_TIMEOUT: Duration = Duration::from_secs(3);
const METADATA_POLL: Duration = Duration::from_millis(50);
/// Linux `EPIPE`: the only core error that means the connection is gone.
const EPIPE: i32 = 32;

struct ChannelEntry {
    slot: usize,
    description: String,
    source: ChannelSource,
    node: Option<pw::node::Node>,
}

struct StreamEntry {
    _proxy: pw::node::Node,
    _listener: pw::node::NodeListener,
    info: Option<StreamInfo>,
}

struct Session {
    mainloop: pw::main_loop::MainLoopRc,
    core: pw::core::CoreRc,
    registry: pw::registry::RegistryRc,
    events: UnboundedSender<BackendEvent>,
    shared: Arc<SharedParams>,
    graph: Graph,
    channels: HashMap<ChannelId, ChannelEntry>,
    monitor_output: Option<String>,
    stream_output: Option<String>,
    links: HashMap<(u32, u32), pw::link::Link>,
    /// Global id of the bound `default` metadata.
    metadata_id: Option<u32>,
    metadata: Option<pw::metadata::Metadata>,
    metadata_listener: Option<pw::metadata::MetadataListener>,
    configured_default_sink: Option<String>,
    /// Last SetDefaultSink request; (re)applied whenever the metadata gets bound.
    desired_default_sink: Option<Option<String>>,
    streams: HashMap<u32, StreamEntry>,
    pending_moves: PendingMoves,
    last_moves: HashMap<u32, String>,
    shutdown_seq: Option<AsyncSeq>,
    end: Option<SessionEnd>,
}

pub fn run_session(
    commands: pw::channel::Receiver<BackendCommand>,
    events: &UnboundedSender<BackendEvent>,
    shared: &Arc<SharedParams>,
) -> (pw::channel::Receiver<BackendCommand>, SessionEnd) {
    macro_rules! try_connect {
        ($expr:expr) => {
            match $expr {
                Ok(value) => value,
                Err(e) => return (commands, SessionEnd::ConnectFailed(e.to_string())),
            }
        };
    }
    let mainloop = try_connect!(pw::main_loop::MainLoopRc::new(None));
    let context = try_connect!(pw::context::ContextRc::new(&mainloop, None));
    let core = try_connect!(context.connect_rc(None));
    let registry = try_connect!(core.get_registry_rc());

    let session = Rc::new(RefCell::new(Session {
        mainloop: mainloop.clone(),
        core: core.clone(),
        registry: registry.clone(),
        events: events.clone(),
        shared: shared.clone(),
        graph: Graph::default(),
        channels: HashMap::new(),
        monitor_output: None,
        stream_output: None,
        links: HashMap::new(),
        metadata_id: None,
        metadata: None,
        metadata_listener: None,
        configured_default_sink: None,
        desired_default_sink: None,
        streams: HashMap::new(),
        pending_moves: PendingMoves::default(),
        last_moves: HashMap::new(),
        shutdown_seq: None,
        end: None,
    }));

    let _core_listener = core
        .add_listener_local()
        .error({
            let weak = Rc::downgrade(&session);
            move |id, _seq, res, message| {
                tracing::warn!("PipeWire error on object {id}: {res} {message}");
                if ends_session(id, res)
                    && let Some(s) = weak.upgrade()
                {
                    let mut s = s.borrow_mut();
                    s.end.get_or_insert(SessionEnd::Disconnected);
                    s.mainloop.quit();
                }
            }
        })
        .done({
            let weak = Rc::downgrade(&session);
            move |id, seq| {
                let Some(s) = weak.upgrade() else { return };
                let mut s = s.borrow_mut();
                if id == pw::core::PW_ID_CORE && s.shutdown_seq == Some(seq) {
                    s.end = Some(SessionEnd::Shutdown);
                    s.mainloop.quit();
                }
            }
        })
        .register();

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let weak = Rc::downgrade(&session);
            move |global| {
                if let Some(s) = weak.upgrade() {
                    Session::on_global(&s, global);
                }
            }
        })
        .global_remove({
            let weak = Rc::downgrade(&session);
            move |id| {
                if let Some(s) = weak.upgrade() {
                    s.borrow_mut().on_global_remove(id);
                }
            }
        })
        .register();

    let ended = |session: &Rc<RefCell<Session>>| {
        session
            .borrow_mut()
            .end
            .take()
            .unwrap_or(SessionEnd::Disconnected)
    };
    // 1st roundtrip: all globals (metadata gets bound); 2nd: metadata property values.
    for _ in 0..2 {
        if !roundtrip(&mainloop, &core, &session) {
            return (commands, ended(&session));
        }
    }
    // Right after login (or a PipeWire restart) WirePlumber may not have published the
    // `default` metadata yet; without it the previous default sink is unknown.
    if !wait_for_metadata(&mainloop, &core, &session) {
        return (commands, ended(&session));
    }

    let _mixer = match MixerNode::new(&core, MIXER_NODE, shared.clone()) {
        Ok(mixer) => mixer,
        Err(e) => return (commands, SessionEnd::ConnectFailed(e)),
    };
    let stream_mix = match create_null_sink(
        &core,
        STREAM_MIX_NODE,
        "MixLink Stream Mix",
        "Audio/Source/Virtual",
    ) {
        Ok(node) => node,
        Err(e) => return (commands, SessionEnd::ConnectFailed(e.to_string())),
    };
    let previous = session.borrow().configured_default_sink.clone();
    let _ = events.send(BackendEvent::Ready {
        previous_default_sink: previous,
    });

    let attached = commands.attach(mainloop.loop_(), {
        let weak = Rc::downgrade(&session);
        move |cmd| {
            if let Some(s) = weak.upgrade() {
                s.borrow_mut().on_command(cmd);
            }
        }
    });
    let meter_timer = mainloop.loop_().add_timer({
        let shared = shared.clone();
        let events = events.clone();
        move |_| {
            let _ = events.send(BackendEvent::Meters(shared.take_peaks()));
        }
    });
    let _ = meter_timer.update_timer(Some(METER_INTERVAL), Some(METER_INTERVAL));

    mainloop.run();

    drop(meter_timer);
    let commands = attached.deattach();
    let end = {
        let mut s = session.borrow_mut();
        s.teardown();
        drop(stream_mix);
        s.end.take().unwrap_or(SessionEnd::Disconnected)
    };
    (commands, end)
}

/// Runs the loop until the server has processed everything sent so far.
/// Returns false if the session ended meanwhile.
fn roundtrip(
    mainloop: &pw::main_loop::MainLoopRc,
    core: &pw::core::CoreRc,
    session: &Rc<RefCell<Session>>,
) -> bool {
    let done = Rc::new(Cell::new(false));
    let Ok(pending) = core.sync(0) else {
        return false;
    };
    let _listener = core
        .add_listener_local()
        .done({
            let done = done.clone();
            let mainloop = mainloop.clone();
            move |id, seq| {
                if id == pw::core::PW_ID_CORE && seq == pending {
                    done.set(true);
                    mainloop.quit();
                }
            }
        })
        .register();
    while !done.get() {
        mainloop.run();
        if session.borrow().end.is_some() {
            return false;
        }
    }
    true
}

#[derive(Debug, PartialEq, Eq)]
enum StartupWait {
    Bound,
    Waiting,
    TimedOut,
}

fn startup_wait(metadata_bound: bool, elapsed: Duration) -> StartupWait {
    if metadata_bound {
        StartupWait::Bound
    } else if elapsed >= METADATA_TIMEOUT {
        StartupWait::TimedOut
    } else {
        StartupWait::Waiting
    }
}

/// Keeps the loop running until the `default` metadata is bound and its property
/// values have arrived, or until `METADATA_TIMEOUT`. Returns false if the session
/// ended meanwhile.
fn wait_for_metadata(
    mainloop: &pw::main_loop::MainLoopRc,
    core: &pw::core::CoreRc,
    session: &Rc<RefCell<Session>>,
) -> bool {
    let start = Instant::now();
    if session.borrow().metadata.is_some() {
        return true;
    }
    tracing::info!("waiting for WirePlumber's default metadata");
    let timer = mainloop.loop_().add_timer({
        let mainloop = mainloop.clone();
        move |_| mainloop.quit()
    });
    let _ = timer.update_timer(Some(METADATA_POLL), Some(METADATA_POLL));
    loop {
        match startup_wait(session.borrow().metadata.is_some(), start.elapsed()) {
            StartupWait::Bound => break,
            StartupWait::TimedOut => {
                tracing::warn!(
                    "default metadata did not appear within {METADATA_TIMEOUT:?}; \
                     the previous default sink is unknown"
                );
                return true;
            }
            StartupWait::Waiting => {}
        }
        mainloop.run();
        if session.borrow().end.is_some() {
            return false;
        }
    }
    drop(timer);
    // The bind went out just now; one more roundtrip delivers the property values.
    roundtrip(mainloop, core, session)
}

/// Other core errors (e.g. -ENOENT for a destroy racing a server-side removal) are
/// reported on PW_ID_CORE too; only a broken connection ends the session.
fn ends_session(id: u32, res: i32) -> bool {
    id == pw::core::PW_ID_CORE && res == -EPIPE
}

fn parse_default_sink(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get("name")?
        .as_str()
        .map(str::to_owned)
}

impl Session {
    fn on_global(this: &Rc<RefCell<Self>>, global: &GlobalObject<&DictRef>) {
        let Some(props) = global.props else { return };
        let get = |k: &str| props.get(k).map(str::to_owned);
        match global.type_ {
            ObjectType::Node => {
                let Some(node) = graph::parse_node(global.id, get) else {
                    return;
                };
                let is_stream = node.is_app_stream();
                {
                    let mut s = this.borrow_mut();
                    if let Some(kind) = node.device_kind() {
                        let device = Device {
                            name: node.name.clone(),
                            description: node.description.clone(),
                            kind,
                        };
                        let _ = s.events.send(BackendEvent::DeviceAdded(device));
                    }
                    s.graph.add_node(node);
                    s.flush_pending_moves();
                    s.reconcile();
                }
                if is_stream {
                    Self::watch_stream(this, global);
                }
            }
            ObjectType::Port => {
                if let Some(port) = graph::parse_port(global.id, get) {
                    let mut s = this.borrow_mut();
                    s.graph.add_port(port);
                    s.reconcile();
                }
            }
            ObjectType::Link => {
                if let Some(link) = graph::parse_link(global.id, get) {
                    let mut s = this.borrow_mut();
                    let output_node = link.output_node;
                    s.graph.add_link(link);
                    s.refresh_stream(output_node);
                }
            }
            ObjectType::Metadata if props.get("metadata.name") == Some("default") => {
                Self::watch_metadata(this, global)
            }
            _ => {}
        }
    }

    fn watch_stream(this: &Rc<RefCell<Self>>, global: &GlobalObject<&DictRef>) {
        let id = global.id;
        let proxy: pw::node::Node = match this.borrow().registry.bind(global) {
            Ok(proxy) => proxy,
            Err(e) => {
                tracing::warn!("binding stream {id} failed: {e}");
                return;
            }
        };
        let weak = Rc::downgrade(this);
        let listener = proxy
            .add_listener_local()
            .info(move |info| {
                let Some(session) = weak.upgrade() else {
                    return;
                };
                // State-only updates (e.g. idle → running) carry an empty props dict;
                // only a PROPS change has the full set.
                if !info.change_mask().contains(pw::node::NodeChangeMask::PROPS) {
                    return;
                }
                let Some(props) = info.props() else { return };
                let get = |k: &str| props.get(k).map(str::to_owned);
                if get("node.dont-move").as_deref() == Some("true") {
                    return;
                }
                let app = app_identity(get, read_proc_comm);
                let name = get("application.name").unwrap_or_else(|| app.clone());
                let icon = get("application.icon-name");
                let mut guard = session.borrow_mut();
                let s = &mut *guard;
                let target = s.graph.link_target(id).map(str::to_owned);
                let info = StreamInfo {
                    node_id: id,
                    app,
                    name,
                    icon,
                    target,
                };
                if let Some(entry) = s.streams.get_mut(&id)
                    && entry.info.as_ref() != Some(&info)
                {
                    entry.info = Some(info.clone());
                    let _ = s.events.send(BackendEvent::StreamUpdated(info));
                }
            })
            .register();
        this.borrow_mut().streams.insert(
            id,
            StreamEntry {
                _proxy: proxy,
                _listener: listener,
                info: None,
            },
        );
    }

    fn watch_metadata(this: &Rc<RefCell<Self>>, global: &GlobalObject<&DictRef>) {
        let metadata: pw::metadata::Metadata = match this.borrow().registry.bind(global) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("binding default metadata failed: {e}");
                return;
            }
        };
        let weak = Rc::downgrade(this);
        let listener = metadata
            .add_listener_local()
            .property(move |subject, key, _type, value| {
                if subject == 0
                    && key == Some(DEFAULT_SINK_KEY)
                    && let Some(s) = weak.upgrade()
                {
                    s.borrow_mut().configured_default_sink = value.and_then(parse_default_sink);
                }
                0
            })
            .register();
        let mut s = this.borrow_mut();
        s.metadata_id = Some(global.id);
        s.metadata = Some(metadata);
        s.metadata_listener = Some(listener);
        // Requests that arrived while no metadata was bound (late WirePlumber start,
        // WirePlumber restart).
        s.apply_default_sink();
        s.flush_pending_moves();
    }

    fn on_global_remove(&mut self, id: u32) {
        if self.metadata_id == Some(id) {
            // WirePlumber went away; defer requests until it publishes the metadata again.
            self.metadata_id = None;
            self.metadata_listener = None;
            self.metadata = None;
            return;
        }
        match self.graph.remove(id) {
            Removed::Node(node) => {
                if node.device_kind().is_some() {
                    let _ = self.events.send(BackendEvent::DeviceRemoved(node.name));
                }
                if self.streams.remove(&id).is_some() {
                    let _ = self.events.send(BackendEvent::StreamRemoved(id));
                }
                self.pending_moves.cancel(id);
                self.last_moves.remove(&id);
            }
            Removed::Port(port) => self
                .links
                .retain(|(out, inp), _| *out != port.id && *inp != port.id),
            Removed::Link(link) => {
                // A link of ours vanished (removed externally or never fully created):
                // forget it so reconcile can recreate it if it is still desired.
                self.links.remove(&(link.output_port, link.input_port));
                self.refresh_stream(link.output_node);
                self.reconcile();
            }
            Removed::Unknown => {}
        }
    }

    fn refresh_stream(&mut self, node_id: u32) {
        let target = self.graph.link_target(node_id).map(str::to_owned);
        if let Some(info) = self.streams.get_mut(&node_id).and_then(|e| e.info.as_mut())
            && info.target != target
        {
            info.target = target;
            let _ = self.events.send(BackendEvent::StreamUpdated(info.clone()));
        }
    }

    fn on_command(&mut self, cmd: BackendCommand) {
        match cmd {
            BackendCommand::EnsureChannel {
                id,
                slot,
                description,
                source,
            } => self.ensure_channel(id, slot, description, source),
            BackendCommand::RemoveChannel { id } => {
                self.destroy_channel(&id);
                self.reconcile();
            }
            BackendCommand::SetTargets(targets) => self.shared.store_targets(&targets),
            BackendCommand::SetMonitorOutput(output) => {
                self.monitor_output = output;
                self.reconcile();
            }
            BackendCommand::SetStreamOutput(output) => {
                self.stream_output = output;
                self.reconcile();
            }
            BackendCommand::SetDefaultSink(name) => {
                self.desired_default_sink = Some(name);
                self.apply_default_sink();
            }
            BackendCommand::MoveStream { node_id, target } => match target {
                Some(target)
                    if self.metadata.is_none() || self.graph.node_by_name(&target).is_none() =>
                {
                    self.pending_moves.defer(node_id, target);
                }
                target => {
                    // An older deferred move must not override this one later.
                    self.pending_moves.cancel(node_id);
                    self.apply_move(node_id, target.as_deref());
                }
            },
            BackendCommand::Shutdown => match self.core.sync(0) {
                Ok(seq) => self.shutdown_seq = Some(seq),
                Err(_) => {
                    self.end = Some(SessionEnd::Shutdown);
                    self.mainloop.quit();
                }
            },
        }
    }

    fn ensure_channel(
        &mut self,
        id: ChannelId,
        slot: usize,
        description: String,
        source: ChannelSource,
    ) {
        if let Some(existing) = self.channels.get(&id)
            && existing.slot == slot
            && existing.description == description
            && existing.source == source
        {
            return;
        }
        self.destroy_channel(&id);
        let node = match &source {
            ChannelSource::Virtual => {
                match create_null_sink(
                    &self.core,
                    &channel_node_name(&id),
                    &description,
                    "Audio/Sink",
                ) {
                    Ok(node) => Some(node),
                    Err(e) => {
                        tracing::error!("creating channel {id} failed: {e}");
                        None
                    }
                }
            }
            ChannelSource::Input { .. } => None,
        };
        self.channels.insert(
            id,
            ChannelEntry {
                slot,
                description,
                source,
                node,
            },
        );
        self.reconcile();
    }

    /// Proxies of our objects are destroyed by dropping them: `pw_proxy_destroy`
    /// sends exactly one `pw_core_destroy`. (`Core::destroy_object` sends one too and
    /// the dropped proxy a second, which the server answers with "unknown resource".)
    fn destroy_channel(&mut self, id: &str) {
        if let Some(entry) = self.channels.remove(id)
            && let Some(node) = entry.node
        {
            self.graph.forget_node_named(&channel_node_name(id));
            // Unlink before the node goes: destroying the node removes its links on the
            // server, and a later destroy of those links would hit unknown resources.
            self.reconcile();
            drop(node);
        }
    }

    /// Writes the desired default sink, if there is one and the metadata is bound
    /// (otherwise `watch_metadata` does it once the metadata appears).
    fn apply_default_sink(&self) {
        let (Some(metadata), Some(name)) = (&self.metadata, &self.desired_default_sink) else {
            if self.desired_default_sink.is_some() {
                tracing::info!(
                    "default metadata not available yet; setting the default sink later"
                );
            }
            return;
        };
        let value = name
            .as_deref()
            .map(|n| serde_json::json!({ "name": n }).to_string());
        metadata.set_property(
            0,
            DEFAULT_SINK_KEY,
            value.as_ref().map(|_| "Spa:String:JSON"),
            value.as_deref(),
        );
    }

    /// Sets (or, for `None`, clears) the stream's `target.object`. Callers defer
    /// `Some` moves while no metadata is bound; a clear without metadata is moot.
    fn apply_move(&mut self, node_id: u32, target: Option<&str>) {
        let Some(metadata) = &self.metadata else {
            return;
        };
        let Some(target) = target else {
            metadata.set_property(node_id, "target.object", None, None);
            self.last_moves.remove(&node_id);
            return;
        };
        if self.last_moves.get(&node_id).map(String::as_str) == Some(target) {
            // Same value again (e.g. after the target sink was recreated): clear first so
            // WirePlumber sees a change.
            metadata.set_property(node_id, "target.object", None, None);
        }
        metadata.set_property(node_id, "target.object", None, Some(target));
        self.last_moves.insert(node_id, target.to_owned());
    }

    /// Applies the deferred moves whose target node exists now. Without metadata
    /// nothing can be applied, so everything stays queued.
    fn flush_pending_moves(&mut self) {
        if self.metadata.is_none() {
            return;
        }
        let graph = &self.graph;
        let ready = self
            .pending_moves
            .take_ready(|name| graph.node_by_name(name).is_some());
        for (node_id, target) in ready {
            self.apply_move(node_id, Some(&target));
        }
    }

    fn reconcile(&mut self) {
        let channel_nodes: Vec<(usize, String, SourceKind)> = self
            .channels
            .iter()
            .map(|(id, entry)| match &entry.source {
                ChannelSource::Virtual => {
                    (entry.slot, channel_node_name(id), SourceKind::SinkMonitor)
                }
                ChannelSource::Input { source } => (entry.slot, source.clone(), SourceKind::Source),
            })
            .collect();
        let desired: HashSet<(u32, u32)> = {
            let plan = LinkPlan {
                mixer: MIXER_NODE,
                stream_mix: STREAM_MIX_NODE,
                monitor_output: self.monitor_output.as_deref(),
                stream_output: self.stream_output.as_deref(),
                channels: channel_nodes
                    .iter()
                    .map(|(slot, node, kind)| ChannelLink {
                        slot: *slot,
                        node,
                        kind: *kind,
                    })
                    .collect(),
            };
            self.graph.desired_links(&plan).into_iter().collect()
        };
        let stale: Vec<(u32, u32)> = self
            .links
            .keys()
            .filter(|k| !desired.contains(k))
            .copied()
            .collect();
        for key in stale {
            drop(self.links.remove(&key));
        }
        for (out_port, in_port) in desired {
            if self.links.contains_key(&(out_port, in_port))
                || self.graph.has_link(out_port, in_port)
            {
                continue;
            }
            let (Some(out), Some(inp)) = (self.graph.port(out_port), self.graph.port(in_port))
            else {
                continue;
            };
            let props = properties! {
                "link.output.node" => out.node.to_string(),
                "link.output.port" => out_port.to_string(),
                "link.input.node" => inp.node.to_string(),
                "link.input.port" => in_port.to_string(),
                "object.linger" => "false",
            };
            match self
                .core
                .create_object::<pw::link::Link>("link-factory", &props)
            {
                Ok(link) => {
                    self.links.insert((out_port, in_port), link);
                }
                Err(e) => tracing::warn!("creating link {out_port}->{in_port} failed: {e}"),
            }
        }
    }

    fn teardown(&mut self) {
        // Links first, then the nodes they hang off (see `destroy_channel`). No
        // reconcile here: it would recreate links for the channels still present.
        self.links.clear();
        self.channels.clear();
        self.streams.clear();
        self.metadata_listener = None;
        self.metadata = None;
    }
}

/// Moves to nodes that cannot be applied yet: the target node does not exist yet or
/// the `default` metadata is not bound. At most one per stream; the newest wins.
#[derive(Default)]
struct PendingMoves(HashMap<u32, String>);

impl PendingMoves {
    fn defer(&mut self, node_id: u32, target: String) {
        self.0.insert(node_id, target);
    }

    fn cancel(&mut self, node_id: u32) {
        self.0.remove(&node_id);
    }

    /// Removes and returns the moves whose target exists; the rest stay queued.
    fn take_ready(&mut self, exists: impl Fn(&str) -> bool) -> Vec<(u32, String)> {
        let ready: Vec<u32> = self
            .0
            .iter()
            .filter(|(_, target)| exists(target))
            .map(|(id, _)| *id)
            .collect();
        let mut moves: Vec<(u32, String)> = ready
            .into_iter()
            .filter_map(|id| self.0.remove(&id).map(|target| (id, target)))
            .collect();
        moves.sort_unstable();
        moves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_broken_core_connection_ends_the_session() {
        assert!(ends_session(pw::core::PW_ID_CORE, -EPIPE));
        assert!(
            !ends_session(pw::core::PW_ID_CORE, -2),
            "unknown resource (-ENOENT)"
        );
        assert!(
            !ends_session(pw::core::PW_ID_CORE, -22),
            "invalid argument (-EINVAL)"
        );
        assert!(!ends_session(42, -EPIPE), "error on another object");
    }

    #[test]
    fn startup_waits_for_metadata_until_the_timeout() {
        assert_eq!(startup_wait(true, Duration::ZERO), StartupWait::Bound);
        assert_eq!(startup_wait(true, METADATA_TIMEOUT * 2), StartupWait::Bound);
        assert_eq!(
            startup_wait(false, Duration::from_millis(100)),
            StartupWait::Waiting
        );
        assert_eq!(startup_wait(false, METADATA_TIMEOUT), StartupWait::TimedOut);
    }

    #[test]
    fn pending_moves_keep_what_cannot_be_applied() {
        let mut pending = PendingMoves::default();
        pending.defer(1, "a".into());
        pending.defer(2, "b".into());
        pending.defer(3, "a".into());
        assert!(pending.take_ready(|_| false).is_empty());
        assert_eq!(
            pending.take_ready(|n| n == "a"),
            vec![(1, "a".into()), (3, "a".into())]
        );
        assert!(pending.take_ready(|n| n == "a").is_empty(), "applied once");
        assert_eq!(pending.take_ready(|_| true), vec![(2, "b".into())]);
    }

    #[test]
    fn newest_pending_move_wins_and_cancel_drops_it() {
        let mut pending = PendingMoves::default();
        pending.defer(1, "a".into());
        pending.defer(1, "b".into());
        assert_eq!(pending.take_ready(|_| true), vec![(1, "b".into())]);
        pending.defer(1, "c".into());
        pending.cancel(1);
        assert!(pending.take_ready(|_| true).is_empty());
    }
}
