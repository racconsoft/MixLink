import { fireEvent, render, screen, within } from "@testing-library/svelte";
import ChannelStrip from "./ChannelStrip.svelte";
import { initBackend } from "../lib/stores";
import { openPopover } from "../lib/popover";
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
  beforeEach(() => openPopover.set(null));

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

  it("the menu can set the fallback", async () => {
    const sent = setup("music");
    await fireEvent.click(screen.getByTitle("Channel options"));
    await fireEvent.click(screen.getByText("Set as fallback"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_fallback_channel", args: { channel: "music" } });
  });

  it("delete opens an in-app confirmation dialog and only deletes once confirmed", async () => {
    const sent = setup("music");
    await fireEvent.click(screen.getByTitle("Channel options"));
    await fireEvent.click(screen.getByText("Delete"));
    expect(screen.getByText('Delete channel "Music"? Its apps move to the fallback channel.')).toBeInTheDocument();
    expect(sent).not.toHaveBeenCalledWith(expect.objectContaining({ cmd: "delete_channel" }));

    await fireEvent.click(screen.getByText("Delete"));
    expect(sent).toHaveBeenCalledWith({ cmd: "delete_channel", args: { channel: "music" } });
  });

  it("cancelling the delete confirmation sends nothing", async () => {
    const sent = setup("music");
    await fireEvent.click(screen.getByTitle("Channel options"));
    await fireEvent.click(screen.getByText("Delete"));
    await fireEvent.click(screen.getByText("Cancel"));
    expect(sent).not.toHaveBeenCalledWith(expect.objectContaining({ cmd: "delete_channel" }));
  });

  it("opening one channel's menu closes another channel's menu", async () => {
    const mock = createMock();
    initBackend(mock);
    const state = initialMockState();
    const music = state.config.channels.find((c) => c.id === "music")!;
    const game = state.config.channels.find((c) => c.id === "game")!;
    render(ChannelStrip, { channel: music, fallback: false, peaks: [0, 0], allApps: state.apps, onedit: vi.fn() });
    render(ChannelStrip, { channel: game, fallback: false, peaks: [0, 0], allApps: state.apps, onedit: vi.fn() });

    const musicStrip = screen.getByTestId("strip-music");
    const gameStrip = screen.getByTestId("strip-game");
    await fireEvent.click(within(musicStrip).getByTitle("Channel options"));
    expect(within(musicStrip).queryByRole("menu")).toBeInTheDocument();

    await fireEvent.click(within(gameStrip).getByTitle("Channel options"));
    expect(within(gameStrip).queryByRole("menu")).toBeInTheDocument();
    expect(within(musicStrip).queryByRole("menu")).toBeNull();
  });
});
