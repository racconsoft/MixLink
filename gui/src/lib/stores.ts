import { writable } from "svelte/store";
import { backend as defaultBackend, type Backend } from "./api";
import { t } from "./i18n";
import type { Command, ErrorBody, Meters, State } from "./types";

export const daemonState = writable<State | null>(null);
export const meters = writable<Meters | null>(null);
export const connected = writable(false);
export interface Toast { id: number; text: string }
export const toasts = writable<Toast[]>([]);
export const TOAST_MS = 5000;

let active: Backend = defaultBackend;
let nextToast = 1;

export function activeBackend(): Backend {
  return active;
}

export function pushToast(text: string): void {
  const id = nextToast++;
  toasts.update((list) => [...list, { id, text }]);
  setTimeout(() => toasts.update((list) => list.filter((x) => x.id !== id)), TOAST_MS);
}

export function formatError(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    const body = e as ErrorBody;
    return body.options?.length ? t("error.options", { message: body.message, options: body.options.join(", ") }) : body.message;
  }
  return String(e);
}

/** Sends a command; failures become a toast. Resolves to whether it succeeded. */
export async function send(cmd: Command): Promise<boolean> {
  try {
    await active.send(cmd);
    return true;
  } catch (e) {
    pushToast(formatError(e));
    return false;
  }
}

/** Wires the backend into the stores; returns a cleanup function. */
export function initBackend(b: Backend = defaultBackend): () => void {
  active = b;
  const offs = [
    b.onState((s) => daemonState.set(s)),
    b.onMeters((m) => meters.set(m)),
    b.onConnection((c) => {
      connected.set(c);
      if (!c) meters.set(null);
    }),
  ];
  let reported = false;
  const offFirst = daemonState.subscribe((s) => {
    if (s && !reported) {
      reported = true;
      b.log("info", "ui: first state rendered");
    }
  });
  void b.snapshot().then((snap) => {
    if (snap.state) daemonState.set(snap.state);
    connected.set(snap.connected);
  });
  b.log("info", "ui: started");
  return () => {
    offs.forEach((off) => off());
    offFirst();
  };
}
