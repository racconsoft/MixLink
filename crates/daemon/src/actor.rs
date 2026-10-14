//! The single task that owns the App: serializes commands, backend events and saves.
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{sleep_until, Instant};
use wavelink_core::model::Config;
use wavelink_core::protocol::{to_line, Command, ErrorBody};

use crate::app::App;
use crate::backend::{AudioBackend, BackendEvent};

pub enum ActorMsg {
    Command { command: Command, reply: oneshot::Sender<Result<Value, ErrorBody>> },
    Shutdown,
}

/// Pre-serialized JSON lines fanned out to subscribed clients.
#[derive(Debug, Clone)]
pub enum Broadcast {
    State(Arc<str>),
    Meters(Arc<str>),
}

pub struct ActorChannels {
    pub msgs: mpsc::Receiver<ActorMsg>,
    pub backend_events: mpsc::UnboundedReceiver<BackendEvent>,
    pub broadcast: broadcast::Sender<Broadcast>,
}

pub async fn run_actor<B: AudioBackend>(
    mut app: App<B>,
    mut channels: ActorChannels,
    mut save: impl FnMut(&Config),
    save_delay: Duration,
) {
    let mut save_at: Option<Instant> = None;
    if app.take_config_dirty() {
        save_at = Some(Instant::now() + save_delay);
    }
    loop {
        tokio::select! {
            msg = channels.msgs.recv() => match msg {
                Some(ActorMsg::Command { command, reply }) => {
                    let _ = reply.send(app.handle(command));
                }
                Some(ActorMsg::Shutdown) | None => break,
            },
            Some(event) = channels.backend_events.recv() => {
                if let Some(meters) = app.on_backend_event(event) {
                    let _ = channels.broadcast.send(Broadcast::Meters(to_line(&meters).into()));
                }
            }
            _ = sleep_until(save_at.unwrap_or_else(Instant::now)), if save_at.is_some() => {
                save(&app.state().config);
                save_at = None;
            }
        }
        if app.take_state_dirty() {
            let _ = channels.broadcast.send(Broadcast::State(to_line(&app.state_event()).into()));
        }
        if app.take_config_dirty() {
            save_at = Some(Instant::now() + save_delay);
        }
    }
    if save_at.is_some() || app.take_config_dirty() {
        save(&app.state().config);
    }
    app.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use wavelink_core::model::{Channel, MixKind, MixTarget};

    use crate::backend::fake::FakeBackend;
    use crate::backend::BackendCommand;

    fn config() -> Config {
        Config {
            version: 1,
            fallback_channel: "system".into(),
            listen: MixKind::Monitor,
            settings: Default::default(),
            mixes: Default::default(),
            channels: vec![Channel::new("system", "System"), Channel::new("music", "Music")],
        }
    }

    async fn call(tx: &mpsc::Sender<ActorMsg>, command: Command) -> Result<Value, ErrorBody> {
        let (reply, rx) = oneshot::channel();
        tx.send(ActorMsg::Command { command, reply }).await.unwrap();
        rx.await.unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn rapid_changes_are_saved_once_after_the_delay() {
        let fake = FakeBackend::new();
        let saves = Arc::new(Mutex::new(Vec::<u8>::new()));
        let (tx, msgs) = mpsc::channel(16);
        let (_ev_tx, backend_events) = mpsc::unbounded_channel();
        let (broadcast, mut bc_rx) = broadcast::channel(64);
        let recorded = saves.clone();
        let actor = tokio::spawn(run_actor(
            App::new(config(), fake.clone(), false),
            ActorChannels { msgs, backend_events, broadcast },
            move |c: &Config| recorded.lock().unwrap().push(c.channels[1].stream.volume),
            Duration::from_millis(500),
        ));

        for v in 0..20u8 {
            let cmd = Command::SetVolume { channel: "music".into(), mix: MixTarget::Stream, value: Some(v), delta: None };
            call(&tx, cmd).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(saves.lock().unwrap().is_empty(), "no save while changes keep coming");
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(*saves.lock().unwrap(), vec![19]);
        assert!(matches!(bc_rx.recv().await.unwrap(), Broadcast::State(_)));

        tx.send(ActorMsg::Shutdown).await.unwrap();
        actor.await.unwrap();
        assert_eq!(fake.take().last(), Some(&BackendCommand::Shutdown));
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_flushes_pending_save() {
        let saves = Arc::new(Mutex::new(0u32));
        let (tx, msgs) = mpsc::channel(16);
        let (_ev_tx, backend_events) = mpsc::unbounded_channel();
        let (broadcast, _) = broadcast::channel(64);
        let recorded = saves.clone();
        let actor = tokio::spawn(run_actor(
            App::new(config(), FakeBackend::new(), false),
            ActorChannels { msgs, backend_events, broadcast },
            move |_: &Config| *recorded.lock().unwrap() += 1,
            Duration::from_millis(500),
        ));
        call(&tx, Command::SetListen { mix: MixKind::Stream }).await.unwrap();
        tx.send(ActorMsg::Shutdown).await.unwrap();
        actor.await.unwrap();
        assert_eq!(*saves.lock().unwrap(), 1);
    }
}
