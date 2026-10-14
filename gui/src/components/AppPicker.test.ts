import { fireEvent, render, screen } from "@testing-library/svelte";
import AppPicker from "./AppPicker.svelte";
import AppIcons from "./AppIcons.svelte";
import { initBackend } from "../lib/stores";
import { openPopover } from "../lib/popover";
import { createMock, initialMockState } from "../lib/mock";

function setup(channelId: string, opts: { noApps?: boolean } = {}) {
  const mock = createMock();
  const sent = vi.spyOn(mock, "send");
  initBackend(mock);
  const state = initialMockState();
  const channel = state.config.channels.find((c) => c.id === channelId)!;
  const allApps = opts.noApps ? [] : state.apps;
  render(AppPicker, { channel, allApps, onclose: vi.fn() });
  return { mock, sent };
}

const box = (name: string) => screen.getByRole("checkbox", { name: new RegExp(name) }) as HTMLInputElement;

describe("AppPicker", () => {
  it("lists running apps, checked when assigned to this channel", () => {
    setup("browser");
    expect(screen.getAllByRole("checkbox")).toHaveLength(3);
    expect(box("Firefox").checked).toBe(true);
    expect(box("Spotify").checked).toBe(false);
    expect(box("Discord").checked).toBe(false);
  });

  it("checking assigns the app, unchecking unassigns it", async () => {
    const { sent } = setup("browser");
    await fireEvent.click(box("Spotify"));
    expect(sent).toHaveBeenCalledWith({ cmd: "assign_app", args: { app: "spotify", channel: "browser" } });
    await fireEvent.click(box("Firefox"));
    expect(sent).toHaveBeenCalledWith({ cmd: "unassign_app", args: { app: "firefox" } });
  });

  it("a failed command puts the checkbox back", async () => {
    const { mock } = setup("browser");
    mock.send = async () => { throw { code: "internal", message: "MixLink service is not running" }; };
    const spotify = box("Spotify");
    // The click checks the box; without a state update it must be unchecked again.
    await fireEvent.click(spotify);
    await vi.waitFor(() => expect(spotify.checked).toBe(false));
    const firefox = box("Firefox");
    await fireEvent.click(firefox);
    await vi.waitFor(() => expect(firefox.checked).toBe(true));
  });

  it("shows a hint when no apps are playing", () => {
    setup("browser", { noApps: true });
    expect(screen.getByText("No apps are playing audio right now")).toBeInTheDocument();
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });
});

describe("AppIcons field", () => {
  beforeEach(() => openPopover.set(null));

  it("is a keyboard-operable button that is not a <button> (drags start inside it)", async () => {
    initBackend(createMock());
    const state = initialMockState();
    const channel = state.config.channels.find((c) => c.id === "browser")!;
    render(AppIcons, { channel, allApps: state.apps });
    const field = screen.getByRole("button", { name: "Assign apps" });
    expect(field.tagName).toBe("DIV");
    expect(field.querySelector("[draggable=true]")).not.toBeNull();
    await fireEvent.keyDown(field, { key: "Enter" });
    expect(screen.getByRole("dialog", { name: "Assign apps" })).toBeInTheDocument();
    await fireEvent.keyDown(field, { key: " " });
    expect(screen.queryByRole("dialog")).toBeNull();
    await fireEvent.click(field);
    expect(screen.getByRole("dialog", { name: "Assign apps" })).toBeInTheDocument();
  });
});
