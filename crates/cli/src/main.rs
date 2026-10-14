mod format;

use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use wavelink_core::client::Client;
use wavelink_core::model::{Channel, MixKind, MixTarget, State};
use wavelink_core::paths;
use wavelink_core::protocol::Command;

use format::LevelArg;

#[derive(Parser)]
#[command(name = "wavelink", version, about = "Control the WaveLink Linux mixer")]
struct Cli {
    /// Print the raw JSON result
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show channels, mixes and running apps
    Status,
    /// Set or change a channel's volume (40, +10, -10)
    Volume {
        channel: String,
        mix: MixArg,
        #[arg(allow_hyphen_values = true)]
        level: String,
    },
    /// Mute, unmute or toggle a channel
    Mute {
        channel: String,
        mix: MixArg,
        #[arg(default_value = "toggle")]
        action: MuteArg,
    },
    /// Assign an app to a channel
    Assign { app: String, channel: String },
    /// Remove an app's channel assignment
    Unassign { app: String },
    /// Choose which mix you hear on the monitor output
    Listen { mix: ListenArg },
}

#[derive(Clone, Copy, ValueEnum)]
enum MixArg {
    Monitor,
    Stream,
    Both,
}

#[derive(Clone, Copy, ValueEnum)]
enum MuteArg {
    On,
    Off,
    Toggle,
}

#[derive(Clone, Copy, ValueEnum)]
enum ListenArg {
    Monitor,
    Stream,
}

impl From<MixArg> for MixTarget {
    fn from(m: MixArg) -> Self {
        match m {
            MixArg::Monitor => MixTarget::Monitor,
            MixArg::Stream => MixTarget::Stream,
            MixArg::Both => MixTarget::Both,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wavelink: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), String> {
    let command = to_command(&cli.command)?;
    let mut client = Client::connect(&paths::socket_path()).map_err(|e| e.to_string())?;
    let result = client.call(command).map_err(|e| e.to_string())?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?);
        return Ok(());
    }
    if let Cmd::Status = cli.command {
        let state: State = serde_json::from_value(result).map_err(|e| e.to_string())?;
        print!("{}", format::format_status(&state));
    } else if let Ok(channel) = serde_json::from_value::<Channel>(result) {
        println!("{}", format::format_channel(&channel));
    } else {
        println!("ok");
    }
    Ok(())
}

fn to_command(cmd: &Cmd) -> Result<Command, String> {
    Ok(match cmd {
        Cmd::Status => Command::GetState,
        Cmd::Volume { channel, mix, level } => {
            let (value, delta) = match format::parse_level(level)? {
                LevelArg::Set(v) => (Some(v), None),
                LevelArg::Delta(d) => (None, Some(d)),
            };
            Command::SetVolume { channel: channel.clone(), mix: (*mix).into(), value, delta }
        }
        Cmd::Mute { channel, mix, action } => Command::SetMute {
            channel: channel.clone(),
            mix: (*mix).into(),
            muted: match action {
                MuteArg::On => Some(true),
                MuteArg::Off => Some(false),
                MuteArg::Toggle => None,
            },
        },
        Cmd::Assign { app, channel } => Command::AssignApp { app: app.clone(), channel: channel.clone() },
        Cmd::Unassign { app } => Command::UnassignApp { app: app.clone() },
        Cmd::Listen { mix } => Command::SetListen {
            mix: match mix {
                ListenArg::Monitor => MixKind::Monitor,
                ListenArg::Stream => MixKind::Stream,
            },
        },
    })
}
