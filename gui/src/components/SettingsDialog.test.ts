import { fireEvent, render, screen } from "@testing-library/svelte";
import SettingsDialog from "./SettingsDialog.svelte";
import { daemonState, initBackend } from "../lib/stores";
import { createMock, initialMockState } from "../lib/mock";

describe("SettingsDialog", () => {
  it("toggles autostart and changes the fallback channel (virtual channels only)", async () => {
    const mock = createMock();
    const sent = vi.spyOn(mock, "send");
    const setAutostart = vi.spyOn(mock, "setAutostart");
    initBackend(mock);
    daemonState.set(initialMockState());
    render(SettingsDialog, { onclose: vi.fn() });
    const box = await screen.findByLabelText("Start at login");
    await vi.waitFor(() => expect(box).toBeChecked());
    await fireEvent.click(box);
    expect(setAutostart).toHaveBeenCalledWith(false);
    const select = screen.getByLabelText("Fallback channel");
    expect(screen.queryByRole("option", { name: "Wave XLR" })).toBeNull();
    await fireEvent.change(select, { target: { value: "music" } });
    expect(sent).toHaveBeenCalledWith({ cmd: "set_fallback_channel", args: { channel: "music" } });
  });

  it("Escape calls onclose", async () => {
    const mock = createMock();
    initBackend(mock);
    daemonState.set(initialMockState());
    const onclose = vi.fn();
    render(SettingsDialog, { onclose });
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
  });
});
