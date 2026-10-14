import { createMock } from "./mock";

describe("mock", () => {
  it("delete_channel moves its apps to the fallback channel", async () => {
    const mock = createMock();
    await mock.send({ cmd: "delete_channel", args: { channel: "music" } });
    const snap = await mock.snapshot();
    expect(snap.state?.config.channels.find((c) => c.id === "music")).toBeUndefined();
    expect(snap.state?.apps.find((a) => a.app === "spotify")?.channel).toBe("system");
  });
});
