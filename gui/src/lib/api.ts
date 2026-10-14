import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Command, Meters, Snapshot, State } from "./types";
import { createMock } from "./mock";

export interface Backend {
  send(cmd: Command): Promise<unknown>;
  snapshot(): Promise<Snapshot>;
  onState(cb: (s: State) => void): () => void;
  onMeters(cb: (m: Meters) => void): () => void;
  onConnection(cb: (connected: boolean) => void): () => void;
  startService(): Promise<void>;
  appIcon(app: string, iconName: string | null): Promise<string | null>;
  getAutostart(): Promise<boolean>;
  setAutostart(enabled: boolean): Promise<void>;
  log(level: "info" | "warn" | "error", message: string): void;
}

function on<T>(event: string, cb: (payload: T) => void): () => void {
  let off: (() => void) | null = null;
  let cancelled = false;
  void listen<T>(event, (e) => cb(e.payload)).then((f) => {
    if (cancelled) f();
    else off = f;
  });
  return () => {
    cancelled = true;
    off?.();
  };
}

export function tauriBackend(): Backend {
  return {
    send: (command) => invoke("send", { command }),
    snapshot: () => invoke<Snapshot>("snapshot"),
    onState: (cb) => on<State>("mixlink://state", cb),
    onMeters: (cb) => on<Meters>("mixlink://meters", cb),
    onConnection: (cb) => on<boolean>("mixlink://connection", cb),
    startService: () => invoke("start_service"),
    appIcon: (app, iconName) => invoke<string | null>("app_icon", { app, iconName }),
    getAutostart: () => invoke<boolean>("get_autostart"),
    setAutostart: (enabled) => invoke("set_autostart", { enabled }),
    log: (level, message) => {
      void invoke("log_frontend", { level, message });
    },
  };
}

function mockFromUrl(): Backend {
  const params = new URLSearchParams(window.location.search);
  return createMock({
    connected: !params.has("disconnected"),
    warnings: params.has("warning") ? ["The config was invalid; defaults were loaded"] : [],
  });
}

export const backend: Backend = import.meta.env.VITE_MIXLINK_MOCK ? mockFromUrl() : tauriBackend();
