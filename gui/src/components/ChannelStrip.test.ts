import { fireEvent, render, screen } from "@testing-library/svelte";
import ChannelStrip from "./ChannelStrip.svelte";
import { initBackend } from "../lib/stores";
import { createMock, initialMockState } from "../lib/mock";
import { APP_MIME } from "../lib/apps";

function setup(id: string) {
  const mock = createMock();
  const sent = vi.spyOn(mock, "send");
  initBackend(mock);
  const state = initialMockState();
  const channel = state.config.channels.find((c) => c.id === id)!;
  render(ChannelStrip, { channel, fallback: id === "system", peaks: [0.5, 0.4], allApps: state.apps, onedit: vi.fn() });
  return sent;
}

describe("ChannelStrip", () => {
  it("mute and link buttons send the inverse state", async () => {
    const sent = setup("music");
    await fireEvent.click(screen.getByTitle("Mute in stream mix"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_mute", args: { channel: "music", mix: "stream", muted: false } });
    await fireEvent.click(screen.getByTitle("Mute in monitor mix"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_mute", args: { channel: "music", mix: "monitor", muted: true } });
    await fireEvent.click(screen.getByTitle("Link monitor and stream faders"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_link", args: { channel: "music", linked: true } });
  });

  it("dropping an app assigns it to this channel", async () => {
    const sent = setup("game");
    const data = new Map<string, string>([[APP_MIME, "firefox"]]);
    const dataTransfer = { types: [APP_MIME], getData: (k: string) => data.get(k) ?? "" };
    const strip = screen.getByTestId("strip-game");
    await fireEvent.dragOver(strip, { dataTransfer });
    await fireEvent.drop(strip, { dataTransfer });
    expect(sent).toHaveBeenCalledWith({ cmd: "assign_app", args: { app: "firefox", channel: "game" } });
  });

  it("input channels do not accept apps", async () => {
    const sent = setup("mic");
    const dataTransfer = { types: [APP_MIME], getData: () => "firefox" };
    await fireEvent.drop(screen.getByTestId("strip-mic"), { dataTransfer });
    expect(sent).not.toHaveBeenCalledWith(expect.objectContaining({ cmd: "assign_app" }));
    expect(screen.getByTitle("Input channel – apps cannot be assigned")).toBeInTheDocument();
  });

  it("the menu can set the fallback and delete after confirmation", async () => {
    const sent = setup("music");
    vi.spyOn(window, "confirm").mockReturnValue(true);
    await fireEvent.click(screen.getByTitle("Channel options"));
    await fireEvent.click(screen.getByText("Set as fallback"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_fallback_channel", args: { channel: "music" } });
    await fireEvent.click(screen.getByTitle("Channel options"));
    await fireEvent.click(screen.getByText("Delete"));
    expect(sent).toHaveBeenCalledWith({ cmd: "delete_channel", args: { channel: "music" } });
  });
});
