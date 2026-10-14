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

  it("resets the select to the current device when the command fails", async () => {
    const mock = createMock();
    mock.send = async () => {
      throw { code: "internal", message: "nope" };
    };
    initBackend(mock);
    daemonState.set(initialMockState());
    render(OutputRow, { kind: "monitor" });
    const select = screen.getByRole("combobox");
    await fireEvent.change(select, { target: { value: "alsa_output.hdmi" } });
    await vi.waitFor(() => expect(select).toHaveDisplayValue("USB Audio Speakers"));
  });

  it("stream row has a device select, mute and listen", async () => {
    const sent = setup();
    render(OutputRow, { kind: "stream" });
    const select = screen.getByRole("combobox");
    expect(select).toHaveDisplayValue("MixLink Stream Mix (OBS) only");
    await fireEvent.click(screen.getByTitle("Mute Stream Mix"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_mix", args: { mix: "stream", muted: true } });
    await fireEvent.click(screen.getByTitle("Listen to this mix"));
    expect(sent).toHaveBeenCalledWith({ cmd: "set_listen", args: { mix: "stream" } });
  });

  it("lists OBS only plus the sinks for the stream device and sends the change", async () => {
    const sent = setup();
    render(OutputRow, { kind: "stream" });
    const select = screen.getByRole("combobox");
    expect(screen.getByText("HDMI / DisplayPort")).toBeInTheDocument();
    await fireEvent.change(select, { target: { value: "alsa_output.hdmi" } });
    expect(sent).toHaveBeenCalledWith({ cmd: "set_mix", args: { mix: "stream", output_device: "alsa_output.hdmi" } });
  });

  it("clears the stream device by sending an empty string", async () => {
    const sent = setup((s) => (s.config.mixes.stream.output_device = "alsa_output.hdmi"));
    render(OutputRow, { kind: "stream" });
    const select = screen.getByRole("combobox");
    expect(select).toHaveDisplayValue("HDMI / DisplayPort");
    await fireEvent.change(select, { target: { value: "" } });
    expect(sent).toHaveBeenCalledWith({ cmd: "set_mix", args: { mix: "stream", output_device: "" } });
  });

  it("shows a missing stream device as disconnected", () => {
    setup((s) => (s.config.mixes.stream.output_device = "alsa_output.gone"));
    render(OutputRow, { kind: "stream" });
    expect(screen.getByRole("combobox")).toHaveDisplayValue("alsa_output.gone (disconnected)");
  });

  it("resets the stream select to the current device when the command fails", async () => {
    const mock = createMock();
    mock.send = async () => {
      throw { code: "internal", message: "nope" };
    };
    initBackend(mock);
    daemonState.set(initialMockState());
    render(OutputRow, { kind: "stream" });
    const select = screen.getByRole("combobox");
    await fireEvent.change(select, { target: { value: "alsa_output.hdmi" } });
    await vi.waitFor(() => expect(select).toHaveDisplayValue("MixLink Stream Mix (OBS) only"));
  });
});
