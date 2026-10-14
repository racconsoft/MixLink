import { createMock } from "./mock";

describe("mock", () => {
  it("delete_channel moves its apps to the fallback channel", async () => {
    const mock = createMock();
    await mock.send({ cmd: "delete_channel", args: { channel: "music" } });
    const snap = await mock.snapshot();
    expect(snap.state?.config.channels.find((c) => c.id === "music")).toBeUndefined();
    expect(snap.state?.apps.find((a) => a.app === "spotify")?.channel).toBe("system");
  });

  it("manyApps adds running apps without changing the default", async () => {
    expect((await createMock().snapshot()).state?.apps).toHaveLength(3);
    const many = (await createMock({ manyApps: true }).snapshot()).state!.apps;
    expect(many.length).toBeGreaterThanOrEqual(8);
    expect(new Set(many.map((a) => a.app)).size).toBe(many.length);
  });
});
