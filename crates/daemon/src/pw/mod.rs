//! PipeWire integration: a dedicated thread that (re)connects and runs a session.
pub mod graph;
pub mod identity;
pub mod mixer_node;
pub mod nodes;
mod session;

use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::backend::{AudioBackend, BackendCommand, BackendEvent};
use crate::mixer::shared::SharedParams;

const BACKOFF_START: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(10);
const STABLE_SESSION: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct PipeWireBackend {
    commands: pipewire::channel::Sender<BackendCommand>,
    shared: Arc<SharedParams>,
    stopped: Arc<Mutex<Option<std_mpsc::Receiver<()>>>>,
}

impl PipeWireBackend {
    pub fn spawn(events: UnboundedSender<BackendEvent>) -> Self {
        let (commands, receiver) = pipewire::channel::channel();
        let shared = Arc::new(SharedParams::new());
        let (stop_tx, stop_rx) = std_mpsc::channel();
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("pipewire".into())
            .spawn(move || {
                thread_main(receiver, events, thread_shared);
                let _ = stop_tx.send(());
            })
            .expect("spawning the PipeWire thread");
        Self {
            commands,
            shared,
            stopped: Arc::new(Mutex::new(Some(stop_rx))),
        }
    }

    /// Waits for the PipeWire thread to finish after a `Shutdown` command.
    pub fn wait_stopped(&self, timeout: Duration) -> bool {
        match self.stopped.lock().unwrap().take() {
            Some(rx) => rx.recv_timeout(timeout).is_ok(),
            None => true,
        }
    }
}

impl AudioBackend for PipeWireBackend {
    fn send(&self, cmd: BackendCommand) {
        if let BackendCommand::SetTargets(targets) = &cmd {
            // Gains go straight to the RT thread; no hop through the PipeWire loop.
            self.shared.store_targets(targets);
            return;
        }
        if self.commands.send(cmd).is_err() {
            tracing::warn!("PipeWire thread is gone; command dropped");
        }
    }
}

fn thread_main(
    mut commands: pipewire::channel::Receiver<BackendCommand>,
    events: UnboundedSender<BackendEvent>,
    shared: Arc<SharedParams>,
) {
    pipewire::init();
    let mut backoff = BACKOFF_START;
    loop {
        let started = Instant::now();
        let (receiver, end) = session::run_session(commands, &events, &shared);
        commands = receiver;
        match end {
            session::SessionEnd::Shutdown => break,
            session::SessionEnd::Disconnected => {
                tracing::warn!("lost the connection to PipeWire; reconnecting");
                let _ = events.send(BackendEvent::Disconnected);
            }
            session::SessionEnd::ConnectFailed(e) => {
                tracing::warn!("cannot connect to PipeWire: {e}")
            }
        }
        if started.elapsed() >= STABLE_SESSION {
            backoff = BACKOFF_START;
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(BACKOFF_MAX);
    }
}
