import { fireEvent, render, screen } from "@testing-library/svelte";
import Fader from "./Fader.svelte";

function rect() {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    top: 0, left: 0, height: 100, width: 10, bottom: 100, right: 10, x: 0, y: 0, toJSON: () => ({}),
  } as DOMRect);
}

describe("Fader", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); });

  it("double click sets 100", async () => {
    const onchange = vi.fn();
    render(Fader, { value: 30, label: "Music stream", onchange });
    await fireEvent.dblClick(screen.getByRole("slider"));
    expect(onchange).toHaveBeenLastCalledWith(100);
  });

  it("wheel up adds 5", async () => {
    const onchange = vi.fn();
    render(Fader, { value: 30, label: "Music stream", onchange });
    await fireEvent.wheel(screen.getByRole("slider"), { deltaY: -100 });
    expect(onchange).toHaveBeenLastCalledWith(35);
  });

  it("dragging throttles and always delivers the final value", async () => {
    rect();
    const onchange = vi.fn();
    render(Fader, { value: 0, label: "Music stream", onchange });
    const slider = screen.getByRole("slider");
    await fireEvent.pointerDown(slider, { clientY: 90, pointerId: 1 });
    for (const y of [80, 70, 60, 50, 40]) await fireEvent.pointerMove(slider, { clientY: y, pointerId: 1 });
    await fireEvent.pointerUp(slider, { clientY: 40, pointerId: 1 });
    expect(onchange.mock.calls.length).toBeLessThan(6);
    expect(onchange).toHaveBeenLastCalledWith(60);
  });

  it("does not jump when the daemon echoes older values during a drag", async () => {
    rect();
    const onchange = vi.fn();
    const { rerender } = render(Fader, { value: 0, label: "Music stream", onchange });
    const slider = screen.getByRole("slider");
    await fireEvent.pointerDown(slider, { clientY: 20, pointerId: 1 });
    await rerender({ value: 10, label: "Music stream", onchange });
    expect(slider).toHaveAttribute("aria-valuenow", "80");
  });

  it("shows the value tooltip while hovering", async () => {
    render(Fader, { value: 42, label: "Music stream", onchange: vi.fn() });
    await fireEvent.pointerEnter(screen.getByRole("slider"));
    expect(screen.getByText("42 %")).toBeInTheDocument();
  });

  it("ignores echoes for 300ms after release, then accepts external values again", async () => {
    rect();
    const onchange = vi.fn();
    const { rerender } = render(Fader, { value: 0, label: "Music stream", onchange });
    const slider = screen.getByRole("slider");
    await fireEvent.pointerDown(slider, { clientY: 100, pointerId: 1 });
    await fireEvent.pointerUp(slider, { clientY: 100, pointerId: 1 });
    expect(slider).toHaveAttribute("aria-valuenow", "0");

    await rerender({ value: 55, label: "Music stream", onchange });
    expect(slider).toHaveAttribute("aria-valuenow", "0");

    vi.advanceTimersByTime(300);
    await rerender({ value: 77, label: "Music stream", onchange });
    expect(slider).toHaveAttribute("aria-valuenow", "77");
  });
});
