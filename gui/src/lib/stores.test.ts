import { get } from "svelte/store";
import type { Backend } from "./api";
import { createMock, initialMockState } from "./mock";
import type { Snapshot, State } from "./types";
import { connected, daemonState, formatError, initBackend, send, toasts } from "./stores";

describe("stores", () => {
  it("initBackend loads the snapshot and applies changes", async () => {
    const mock = createMock();
    const off = initBackend(mock);
    await vi.waitFor(() => expect(get(daemonState)?.config.channels.length).toBe(6));
    expect(get(connected)).toBe(true);
    await send({ cmd: "set_volume", args: { channel: "music", mix: "stream", value: 10 } });
    expect(get(daemonState)?.config.channels.find((c) => c.id === "music")?.stream.volume).toBe(10);
    off();
  });

  it("failed commands become toasts with options", async () => {
    initBackend(createMock());
    const ok = await send({ cmd: "set_mute", args: { channel: "xyz", mix: "both", muted: true } });
    expect(ok).toBe(false);
    expect(get(toasts).at(-1)?.text).toContain("options: Wave XLR, System");
  });

  it("formats plain errors", () => {
    expect(formatError("boom")).toBe("boom");
    expect(formatError({ code: "internal", message: "down" })).toBe("down");
  });

  it("connection state is unknown (null) until something reports it", async () => {
    vi.resetModules();
    const fresh = await import("./stores");
    expect(get(fresh.connected)).toBeNull();
  });

  /** A mock whose snapshot resolves only when `resolve` is called, with events fired by hand. */
  function controlled(): { backend: Backend; resolve: (s: Snapshot) => void; state: (s: State) => void; connection: (c: boolean) => void } {
    const base = createMock();
    let resolve!: (s: Snapshot) => void;
    let stateCb: (s: State) => void = () => {};
    let connCb: (c: boolean) => void = () => {};
    const backend: Backend = {
      ...base,
      snapshot: () => new Promise<Snapshot>((r) => (resolve = r)),
      onState: (cb) => { stateCb = cb; return () => {}; },
      onConnection: (cb) => { connCb = cb; return () => {}; },
    };
    return { backend, resolve: (s) => resolve(s), state: (s) => stateCb(s), connection: (c) => connCb(c) };
  }

  it("a late snapshot does not overwrite state or connection delivered by events", async () => {
    const c = controlled();
    const off = initBackend(c.backend);
    const fromEvent = initialMockState();
    fromEvent.config.channels = fromEvent.config.channels.slice(0, 2);
    c.state(fromEvent);
    c.connection(false);
    c.resolve({ state: initialMockState(), connected: true });
    await Promise.resolve();
    await Promise.resolve();
    expect(get(connected)).toBe(false);
    expect(get(daemonState)?.config.channels.length).toBe(2);
    off();
  });

  it("the snapshot still fills in fields no event has delivered", async () => {
    const c = controlled();
    const off = initBackend(c.backend);
    connected.set(null);
    const fromEvent = initialMockState();
    fromEvent.config.channels = fromEvent.config.channels.slice(0, 3);
    c.state(fromEvent);
    c.resolve({ state: initialMockState(), connected: true });
    await vi.waitFor(() => expect(get(connected)).toBe(true));
    expect(get(daemonState)?.config.channels.length).toBe(3);
    off();
  });
});
