mod format;

use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use wavelink_core::client::Client;
use wavelink_core::model::{Channel, Device, Mix, MixKind, MixTarget, State};
use wavelink_core::paths;
use wavelink_core::protocol::Command;

use format::{LevelArg, MixLevelArg};

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
    Listen { mix: MixKindArg },
    /// List playback devices, or play the monitor mix on <device>
    Output { device: Option<String> },
    /// Set a mix master: 0–100, +N, -N, mute, unmute or toggle
    Mix {
        mix: MixKindArg,
        #[arg(allow_hyphen_values = true)]
        level: String,
    },
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
enum MixKindArg {
    Monitor,
    Stream,
}

impl From<MixKindArg> for MixKind {
    fn from(m: MixKindArg) -> Self {
        match m {
            MixKindArg::Monitor => MixKind::Monitor,
            MixKindArg::Stream => MixKind::Stream,
        }
    }
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

/// What `run` has to do for a command line.
#[derive(Debug, PartialEq)]
enum Plan {
    /// Send one command.
    Call(Command),
    /// Read the mix's mute state first, then send the inverse.
    ToggleMix(MixKind),
    /// List the sink devices and mark the current monitor output.
    ListOutputs,
}

fn run(cli: &Cli) -> Result<(), String> {
    let plan = plan(&cli.command)?;
    let mut client = Client::connect(&paths::socket_path()).map_err(|e| e.to_string())?;
    let mut call = |command| client.call(command).map_err(|e| e.to_string());
    let command = match plan {
        Plan::Call(command) => command,
        Plan::ToggleMix(mix) => {
            let state: State = from_value(call(Command::GetState)?)?;
            toggle_mix_command(mix, &state)
        }
        Plan::ListOutputs => {
            let devices = call(Command::ListDevices)?;
            if cli.json {
                return print_json(&devices);
            }
            let devices: Vec<Device> = from_value(devices)?;
            let state: State = from_value(call(Command::GetState)?)?;
            let current = state.config.mixes.monitor.output_device.as_deref();
            print!("{}", format::format_outputs(&devices, current));
            return Ok(());
        }
    };
    let result = call(command)?;
    if cli.json {
        return print_json(&result);
    }
    match &cli.command {
        Cmd::Status => {
            let state: State = from_value(result)?;
            print!("{}", format::format_status(&state));
        }
        Cmd::Output { .. } => println!(
            "{}",
            format::format_mix(MixKind::Monitor, &from_value::<Mix>(result)?)
        ),
        Cmd::Mix { mix, .. } => println!(
            "{}",
            format::format_mix((*mix).into(), &from_value::<Mix>(result)?)
        ),
        _ => match serde_json::from_value::<Channel>(result) {
            Ok(channel) => println!("{}", format::format_channel(&channel)),
            Err(_) => println!("ok"),
        },
    }
    Ok(())
}

fn from_value<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("unexpected answer from wavelinkd: {e}"))
}

fn print_json(value: &serde_json::Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn plan(cmd: &Cmd) -> Result<Plan, String> {
    Ok(Plan::Call(match cmd {
        Cmd::Output { device: None } => return Ok(Plan::ListOutputs),
        Cmd::Output {
            device: Some(device),
        } => set_mix(MixKind::Monitor, None, None, None, Some(device.clone())),
        Cmd::Mix { mix, level } => {
            let mix = MixKind::from(*mix);
            match format::parse_mix_level(level)? {
                MixLevelArg::Level(LevelArg::Set(v)) => set_mix(mix, Some(v), None, None, None),
                MixLevelArg::Level(LevelArg::Delta(d)) => set_mix(mix, None, Some(d), None, None),
                MixLevelArg::Mute => set_mix(mix, None, None, Some(true), None),
                MixLevelArg::Unmute => set_mix(mix, None, None, Some(false), None),
                MixLevelArg::Toggle => return Ok(Plan::ToggleMix(mix)),
            }
        }
        other => to_command(other)?,
    }))
}

fn set_mix(
    mix: MixKind,
    volume: Option<u8>,
    delta: Option<i32>,
    muted: Option<bool>,
    output_device: Option<String>,
) -> Command {
    Command::SetMix {
        mix,
        volume,
        delta,
        muted,
        output_device,
    }
}

fn toggle_mix_command(mix: MixKind, state: &State) -> Command {
    let muted = state.config.mixes.get(mix).muted;
    set_mix(mix, None, None, Some(!muted), None)
}

fn to_command(cmd: &Cmd) -> Result<Command, String> {
    Ok(match cmd {
        Cmd::Status => Command::GetState,
        Cmd::Volume {
            channel,
            mix,
            level,
        } => {
            let (value, delta) = match format::parse_level(level)? {
                LevelArg::Set(v) => (Some(v), None),
                LevelArg::Delta(d) => (None, Some(d)),
            };
            Command::SetVolume {
                channel: channel.clone(),
                mix: (*mix).into(),
                value,
                delta,
            }
        }
        Cmd::Mute {
            channel,
            mix,
            action,
        } => Command::SetMute {
            channel: channel.clone(),
            mix: (*mix).into(),
            muted: match action {
                MuteArg::On => Some(true),
                MuteArg::Off => Some(false),
                MuteArg::Toggle => None,
            },
        },
        Cmd::Assign { app, channel } => Command::AssignApp {
            app: app.clone(),
            channel: channel.clone(),
        },
        Cmd::Unassign { app } => Command::UnassignApp { app: app.clone() },
        Cmd::Listen { mix } => Command::SetListen { mix: (*mix).into() },
        Cmd::Output { .. } | Cmd::Mix { .. } => {
            unreachable!("handled by plan")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wavelink_core::model::Config;

    fn parse(args: &[&str]) -> Result<Plan, String> {
        let cli = Cli::try_parse_from(std::iter::once("wavelink").chain(args.iter().copied()))
            .map_err(|e| e.to_string())?;
        plan(&cli.command)
    }

    fn mix_call(mix: MixKind, volume: Option<u8>, delta: Option<i32>, muted: Option<bool>) -> Plan {
        Plan::Call(set_mix(mix, volume, delta, muted, None))
    }

    #[test]
    fn output_lists_or_sets_the_monitor_device() {
        assert_eq!(parse(&["output"]), Ok(Plan::ListOutputs));
        assert_eq!(
            parse(&["output", "hdmi"]),
            Ok(Plan::Call(set_mix(
                MixKind::Monitor,
                None,
                None,
                None,
                Some("hdmi".into())
            )))
        );
    }

    #[test]
    fn mix_levels_map_to_set_mix() {
        use MixKind::{Monitor, Stream};
        assert_eq!(
            parse(&["mix", "monitor", "80"]),
            Ok(mix_call(Monitor, Some(80), None, None))
        );
        assert_eq!(
            parse(&["mix", "stream", "+5"]),
            Ok(mix_call(Stream, None, Some(5), None))
        );
        assert_eq!(
            parse(&["mix", "stream", "-10"]),
            Ok(mix_call(Stream, None, Some(-10), None))
        );
        assert_eq!(
            parse(&["mix", "monitor", "mute"]),
            Ok(mix_call(Monitor, None, None, Some(true)))
        );
        assert_eq!(
            parse(&["mix", "monitor", "unmute"]),
            Ok(mix_call(Monitor, None, None, Some(false)))
        );
        assert_eq!(
            parse(&["mix", "stream", "toggle"]),
            Ok(Plan::ToggleMix(Stream))
        );
        assert!(parse(&["mix", "stream", "150"]).is_err());
        assert!(parse(&["mix", "stream", "loud"]).is_err());
        assert!(
            parse(&["mix", "both", "50"]).is_err(),
            "no 'both' for mixes"
        );
    }

    #[test]
    fn toggle_sends_the_inverse_mute_state() {
        let mut state = State {
            config: Config {
                version: 1,
                fallback_channel: "system".into(),
                listen: MixKind::Monitor,
                settings: Default::default(),
                mixes: Default::default(),
                channels: vec![Channel::new("system", "System")],
            },
            apps: vec![],
            devices: vec![],
            warnings: vec![],
        };
        state.config.mixes.stream.muted = true;
        assert_eq!(
            toggle_mix_command(MixKind::Stream, &state),
            set_mix(MixKind::Stream, None, None, Some(false), None)
        );
        assert_eq!(
            toggle_mix_command(MixKind::Monitor, &state),
            set_mix(MixKind::Monitor, None, None, Some(true), None)
        );
    }

    #[test]
    fn other_commands_still_map_directly() {
        assert_eq!(
            parse(&["listen", "stream"]),
            Ok(Plan::Call(Command::SetListen {
                mix: MixKind::Stream
            }))
        );
        assert_eq!(parse(&["status"]), Ok(Plan::Call(Command::GetState)));
    }
}
