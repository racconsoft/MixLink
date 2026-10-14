import { fireEvent, render, screen } from "@testing-library/svelte";
import ChannelDialog from "./ChannelDialog.svelte";
import { initBackend } from "../lib/stores";
import { createMock, initialMockState } from "../lib/mock";

describe("ChannelDialog", () => {
  it("creates a channel with name, icon and color", async () => {
    const mock = createMock();
    const sent = vi.spyOn(mock, "send");
    initBackend(mock);
    const onclose = vi.fn();
    render(ChannelDialog, { mode: { kind: "create" }, onclose });
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "Podcast" } });
    await fireEvent.click(screen.getByTitle("podcast"));
    await fireEvent.click(screen.getByTitle("#22c55e"));
    await fireEvent.click(screen.getByText("Create"));
    expect(sent).toHaveBeenCalledWith({ cmd: "create_channel", args: { name: "Podcast", icon: "podcast", color: "#22c55e" } });
    await vi.waitFor(() => expect(onclose).toHaveBeenCalled());
  });

  it("edit sends only what changed", async () => {
    const mock = createMock();
    const sent = vi.spyOn(mock, "send");
    initBackend(mock);
    const channel = initialMockState().config.channels.find((c) => c.id === "music")!;
    render(ChannelDialog, { mode: { kind: "edit", channel }, onclose: vi.fn() });
    await fireEvent.input(screen.getByLabelText("Name"), { target: { value: "Tunes" } });
    await fireEvent.click(screen.getByText("Save"));
    expect(sent).toHaveBeenCalledWith({ cmd: "rename_channel", args: { channel: "music", name: "Tunes" } });
    expect(sent).not.toHaveBeenCalledWith(expect.objectContaining({ cmd: "update_channel" }));
  });
});
