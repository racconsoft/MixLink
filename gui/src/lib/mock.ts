import type { Backend } from "./api";
import type { ActiveApp, Channel, Command, Meters, State } from "./types";

function channel(id: string, name: string, icon: string, color: string, extra: Partial<Channel> = {}): Channel {
  return {
    id, name, icon, color, kind: "virtual", linked: false, apps: [],
    monitor: { volume: 100, muted: false }, stream: { volume: 100, muted: false }, ...extra,
  };
}

export function initialMockState(): State {
  const apps: ActiveApp[] = [
    { node_id: 1, app: "firefox", name: "Firefox", icon: null, channel: "browser" },
    { node_id: 2, app: "spotify", name: "Spotify", icon: null, channel: "music" },
    { node_id: 3, app: "discord", name: "Discord", icon: null, channel: "voice-chat" },
  ];
  return {
    config: {
      version: 1, fallback_channel: "system", listen: "monitor", settings: { auto_xlr_profile: true },
      mixes: {
        monitor: { volume: 100, muted: false, output_device: "alsa_output.usb-speakers" },
        stream: { volume: 100, muted: false },
      },
      channels: [
        channel("mic", "Wave XLR", "mic", "#3b82f6", { kind: "input", source: "alsa_input.xlr", monitor: { volume: 100, muted: true } }),
        channel("system", "System", "system", "#64748b"),
        channel("game", "Game", "game", "#ef4444", { apps: ["steam"], linked: true, stream: { volume: 80, muted: false } }),
        channel("browser", "Browser", "browser", "#a855f7", { apps: ["firefox"] }),
        channel("voice-chat", "Voice Chat", "chat", "#facc15", { apps: ["discord"] }),
        channel("music", "Music", "music", "#ec4899", { apps: ["spotify"], stream: { volume: 55, muted: true } }),
      ],
    },
    apps,
    devices: [
      { name: "alsa_output.usb-speakers", description: "USB Audio Speakers", kind: "sink" },
      { name: "alsa_output.usb-headphones", description: "USB Audio Front Headphones", kind: "sink" },
      { name: "alsa_output.hdmi", description: "HDMI / DisplayPort", kind: "sink" },
      { name: "alsa_input.xlr", description: "Wave XLR", kind: "source" },
    ],
    warnings: [],
  };
}

/** Applies a command to the mock state, mimicking mixlinkd's semantics. */
export function applyMockCommand(state: State, cmd: Command): unknown {
  const ch = (id: string) => {
    const c = state.config.channels.find((x) => x.id === id);
    if (!c) throw { code: "not_found", message: `nothing matches '${id}'`, options: state.config.channels.map((x) => x.name) };
    return c;
  };
  switch (cmd.cmd) {
    case "get_state":
      return state;
    case "set_volume": {
      const c = ch(cmd.args.channel);
      const kinds = cmd.args.mix === "both" ? (["monitor", "stream"] as const) : [cmd.args.mix];
      for (const k of kinds) {
        const old = c[k].volume;
        const next = cmd.args.value ?? Math.max(0, Math.min(100, old + (cmd.args.delta ?? 0)));
        c[k].volume = next;
        if (c.linked && cmd.args.mix !== "both") {
          const other = k === "monitor" ? "stream" : "monitor";
          c[other].volume = Math.max(0, Math.min(100, c[other].volume + next - old));
        }
      }
      return c;
    }
    case "set_mute": {
      const c = ch(cmd.args.channel);
      const kinds = cmd.args.mix === "both" ? (["monitor", "stream"] as const) : [cmd.args.mix];
      const target = cmd.args.muted ?? !kinds.every((k) => c[k].muted);
      for (const k of kinds) c[k].muted = target;
      return c;
    }
    case "set_link": {
      const c = ch(cmd.args.channel);
      c.linked = cmd.args.linked;
      return c;
    }
    case "assign_app": {
      const c = ch(cmd.args.channel);
      if (c.kind === "input") throw { code: "invalid_argument", message: `${c.name} is an input channel` };
      for (const x of state.config.channels) x.apps = x.apps.filter((a) => a !== cmd.args.app);
      c.apps.push(cmd.args.app);
      for (const a of state.apps) if (a.app === cmd.args.app) a.channel = c.id;
      return c;
    }
    case "unassign_app": {
      for (const x of state.config.channels) x.apps = x.apps.filter((a) => a !== cmd.args.app);
      for (const a of state.apps) if (a.app === cmd.args.app) a.channel = state.config.fallback_channel;
      return { app: cmd.args.app, channel: state.config.fallback_channel };
    }
    case "create_channel": {
      const id = cmd.args.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "channel";
      const c = channel(id, cmd.args.name, cmd.args.icon ?? "default", cmd.args.color ?? "#6b7280");
      state.config.channels.push(c);
      return c;
    }
    case "rename_channel": {
      const c = ch(cmd.args.channel);
      c.name = cmd.args.name;
      return c;
    }
    case "update_channel": {
      const c = ch(cmd.args.channel);
      if (cmd.args.icon) c.icon = cmd.args.icon;
      if (cmd.args.color) c.color = cmd.args.color;
      return c;
    }
    case "delete_channel": {
      if (cmd.args.channel === state.config.fallback_channel) throw { code: "not_allowed", message: "cannot delete the fallback channel" };
      state.config.channels = state.config.channels.filter((x) => x.id !== cmd.args.channel);
      for (const a of state.apps) if (a.channel === cmd.args.channel) a.channel = state.config.fallback_channel;
      return { deleted: cmd.args.channel };
    }
    case "set_mix": {
      const m = state.config.mixes[cmd.args.mix];
      if (cmd.args.volume !== undefined) m.volume = cmd.args.volume;
      if (cmd.args.muted !== undefined) m.muted = cmd.args.muted;
      if (cmd.args.output_device !== undefined) {
        if (cmd.args.output_device === "") delete m.output_device;
        else m.output_device = cmd.args.output_device;
      }
      return m;
    }
    case "set_listen":
      state.config.listen = cmd.args.mix;
      return { listen: cmd.args.mix };
    case "set_fallback_channel":
      state.config.fallback_channel = ch(cmd.args.channel).id;
      return { fallback_channel: cmd.args.channel };
  }
}

/** Extra running apps for `?manyapps=1` (they play in the fallback channel). */
const MANY_APPS = ["OBS Studio", "VLC", "Steam", "Telegram", "Thunderbird", "mpv", "Chromium", "Signal"];

export function createMock(opts: { connected?: boolean; warnings?: string[]; manyApps?: boolean } = {}): Backend {
  const state = initialMockState();
  state.warnings = opts.warnings ?? [];
  if (opts.manyApps) {
    MANY_APPS.forEach((name, i) => state.apps.push({
      node_id: 10 + i, app: name.toLowerCase().replace(/\s+/g, "-"), name, icon: null, channel: state.config.fallback_channel,
    }));
  }
  const connected = opts.connected ?? true;
  const stateCbs = new Set<(s: State) => void>();
  const meterCbs = new Set<(m: Meters) => void>();
  const connCbs = new Set<(c: boolean) => void>();
  let autostart = true;
  const clone = () => structuredClone(state);
  let tick = 0;
  if (connected) {
    setInterval(() => {
      tick += 1;
      const wave = (phase: number) => Math.abs(Math.sin(tick / 9 + phase)) * 0.8;
      const m: Meters = { channels: {}, monitor: [wave(0), wave(0.3)], stream: [wave(1), wave(1.2)] };
      state.config.channels.forEach((c, i) => {
        const playing = c.id === "mic" || state.apps.some((a) => a.channel === c.id);
        m.channels[c.id] = playing ? [wave(i), wave(i + 0.2)] : [0, 0];
      });
      meterCbs.forEach((cb) => cb(m));
    }, 33);
  }
  return {
    async send(cmd) {
      if (!connected) throw { code: "internal", message: "MixLink service is not running" };
      const result = applyMockCommand(state, cmd);
      const snapshot = clone();
      stateCbs.forEach((cb) => cb(snapshot));
      return structuredClone(result);
    },
    async snapshot() {
      return { state: connected ? clone() : null, connected };
    },
    onState(cb) { stateCbs.add(cb); return () => stateCbs.delete(cb); },
    onMeters(cb) { meterCbs.add(cb); return () => meterCbs.delete(cb); },
    onConnection(cb) { connCbs.add(cb); return () => connCbs.delete(cb); },
    async startService() {},
    async appIcon() { return null; },
    async getAutostart() { return autostart; },
    async setAutostart(enabled) { autostart = enabled; },
    log() {},
  };
}
