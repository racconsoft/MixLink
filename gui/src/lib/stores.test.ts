import { get } from "svelte/store";
import { createMock } from "./mock";
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
});
