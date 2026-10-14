import { fireEvent, render, screen } from "@testing-library/svelte";
import ConnectionOverlay from "./ConnectionOverlay.svelte";
import { connected, initBackend, toasts } from "../lib/stores";
import { createMock } from "../lib/mock";
import { get } from "svelte/store";

describe("ConnectionOverlay", () => {
  it("appears when disconnected and starts the service", async () => {
    const mock = createMock({ connected: false });
    const start = vi.spyOn(mock, "startService");
    initBackend(mock);
    connected.set(false);
    render(ConnectionOverlay);
    expect(screen.getByText("MixLink service is not running")).toBeInTheDocument();
    await fireEvent.click(screen.getByText("Start service"));
    expect(start).toHaveBeenCalled();
    connected.set(true);
    await vi.waitFor(() => expect(screen.queryByText("MixLink service is not running")).toBeNull());
  });

  it("reports a failed start as toast", async () => {
    const mock = createMock({ connected: false });
    mock.startService = async () => { throw "systemctl exited with 5"; };
    initBackend(mock);
    connected.set(false);
    render(ConnectionOverlay);
    await fireEvent.click(screen.getByText("Start service"));
    await vi.waitFor(() => expect(get(toasts).at(-1)?.text).toContain("systemctl exited with 5"));
  });

  it("stays hidden while the connection state is still unknown", () => {
    connected.set(null);
    render(ConnectionOverlay);
    expect(screen.queryByText("MixLink service is not running")).toBeNull();
  });
});
