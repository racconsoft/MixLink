import { fireEvent, render, screen } from "@testing-library/svelte";
import SettingsDialog from "./SettingsDialog.svelte";
import { daemonState, initBackend, toasts } from "../lib/stores";
import { createMock, initialMockState } from "../lib/mock";
import { get } from "svelte/store";

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

  it("snaps the checkbox back and shows a toast when setAutostart fails", async () => {
    const mock = createMock();
    mock.setAutostart = async () => {
      throw { code: "internal", message: "disk full" };
    };
    initBackend(mock);
    daemonState.set(initialMockState());
    render(SettingsDialog, { onclose: vi.fn() });
    const box = await screen.findByLabelText<HTMLInputElement>("Start at login");
    await vi.waitFor(() => expect(box).toBeChecked());
    await fireEvent.click(box);
    await vi.waitFor(() => expect(box).toBeChecked());
    expect(get(toasts).at(-1)?.text).toContain("disk full");
  });

  it("shows a toast when getAutostart fails on open", async () => {
    const mock = createMock();
    mock.getAutostart = async () => {
      throw { code: "internal", message: "no config" };
    };
    initBackend(mock);
    daemonState.set(initialMockState());
    render(SettingsDialog, { onclose: vi.fn() });
    await vi.waitFor(() => expect(get(toasts).at(-1)?.text).toContain("no config"));
  });

  it("resets the fallback select when the command fails", async () => {
    const mock = createMock();
    mock.send = async () => {
      throw { code: "internal", message: "nope" };
    };
    initBackend(mock);
    daemonState.set(initialMockState());
    render(SettingsDialog, { onclose: vi.fn() });
    const select = screen.getByLabelText("Fallback channel");
    await fireEvent.change(select, { target: { value: "music" } });
    await vi.waitFor(() => expect(select).toHaveDisplayValue("System"));
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
