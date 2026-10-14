import { fireEvent, render, screen } from "@testing-library/svelte";
import OutputRow from "./OutputRow.svelte";
import { daemonState, initBackend } from "../lib/stores";
import { createMock, initialMockState } from "../lib/mock";

function setup(mutate?: (s: ReturnType<typeof initialMockState>) => void) {
  const mock = createMock();
  const sent = vi.spyOn(mock, "send");
  initBackend(mock);
  const state = initialMockState();
  mutate?.(state);
  daemonState.set(state);
  return sent;
}

describe("OutputRow", () => {
  it("lists sinks and switches the monitor device", async () => {
    const sent = setup();
    render(OutputRow, { kind: "monitor" });
    const select = screen.getByRole("combobox");
    expect(select).toHaveDisplayValue("USB Audio Speakers");
    expect(screen.queryByText("Wave XLR")).toBeNull();
    await fireEvent.change(select, { target: { value: "alsa_output.hdmi" } });
    expect(sent).toHaveBeenCalledWith({ cmd: "set_mix", args: { mix: "monitor", output_device: "alsa_output.hdmi" } });
  });

  it("shows an unplugged device as disconnected instead of another device", () => {
    setup((s) => (s.config.mixes.monitor.output_device = "alsa_output.gone"));
    render(OutputRow, { kind: "monitor" });
    expect(screen.getByRole("combobox")).toHaveDisplayValue("alsa_output.gone (disconnected)");
  });

  it("stream row has a fixed target, mute and listen", async () => {
    const sent = setup();
    render(OutputRow, { kind: "stream" });
    expect(screen.getByText("MixLink Stream Mix (OBS)")).toBeInTheDocument();
    await fireEvent.click(screen.getByTitle("Mute Stream Mix"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_mix", args: { mix: "stream", muted: true } });
    await fireEvent.click(screen.getByTitle("Listen to this mix"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_listen", args: { mix: "stream" } });
  });
});
