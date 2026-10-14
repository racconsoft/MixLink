use std::time::Duration;

use anyhow::Context;
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::EnvFilter;
use wavelink_core::paths;
use wavelinkd::actor::{ActorChannels, ActorMsg, run_actor};
use wavelinkd::app::App;
use wavelinkd::config::{self, LoadOutcome};
use wavelinkd::ipc;
use wavelinkd::pw::PipeWireBackend;

const SAVE_DELAY: Duration = Duration::from_millis(500);

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let socket = paths::socket_path();
    let listener =
        ipc::bind_socket(&socket).with_context(|| format!("binding {}", socket.display()))?;

    let config_path = paths::config_path();
    let (config, first_run, warning) = match config::load(&config_path) {
        LoadOutcome::Loaded(config) => (config, false, None),
        LoadOutcome::Created(config) => (config, true, None),
        LoadOutcome::Recovered {
            config,
            backup,
            error,
        } => {
            let warning = format!(
                "The config was invalid ({error}); it was backed up to {} and defaults were loaded",
                backup.display()
            );
            (config, true, Some(warning))
        }
    };

    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let backend = PipeWireBackend::spawn(event_tx);
    let mut app = App::new(config, backend.clone(), first_run);
    if let Some(warning) = warning {
        tracing::warn!("{warning}");
        app.add_warning(warning);
    }

    let (msg_tx, msg_rx) = mpsc::channel(64);
    let (broadcast_tx, _) = broadcast::channel(256);
    let server = tokio::spawn(ipc::serve(listener, msg_tx.clone(), broadcast_tx.clone()));
    tokio::spawn(shutdown_on_signal(msg_tx));
    tracing::info!("wavelinkd listening on {}", socket.display());

    let save_path = config_path.clone();
    run_actor(
        app,
        ActorChannels {
            msgs: msg_rx,
            backend_events: event_rx,
            broadcast: broadcast_tx,
        },
        move |config| {
            if let Err(e) = config::save(&save_path, config) {
                tracing::error!("saving {} failed: {e}", save_path.display());
            }
        },
        SAVE_DELAY,
    )
    .await;

    server.abort();
    if !backend.wait_stopped(Duration::from_secs(2)) {
        tracing::warn!("the PipeWire thread did not stop in time");
    }
    let _ = std::fs::remove_file(&socket);
    Ok(())
}

async fn shutdown_on_signal(tx: mpsc::Sender<ActorMsg>) {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut term), Ok(mut int)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
    ) else {
        tracing::error!("cannot install signal handlers");
        return;
    };
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
    let _ = tx.send(ActorMsg::Shutdown).await;
}
