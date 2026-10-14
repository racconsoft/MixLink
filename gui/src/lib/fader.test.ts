import { FaderModel, IGNORE_AFTER_RELEASE_MS, clamp, valueFromPointer, valueFromPointerX, wheelStep } from "./fader";

describe("fader math", () => {
  it("clamps and rounds", () => {
    expect(clamp(-3)).toBe(0);
    expect(clamp(101.2)).toBe(100);
    expect(clamp(41.6)).toBe(42);
  });

  it("maps pointer positions", () => {
    expect(valueFromPointer(100, { top: 100, height: 200 })).toBe(100);
    expect(valueFromPointer(300, { top: 100, height: 200 })).toBe(0);
    expect(valueFromPointer(200, { top: 100, height: 200 })).toBe(50);
    expect(valueFromPointerX(150, { left: 100, width: 100 })).toBe(50);
  });

  it("wheel steps by 5", () => {
    expect(wheelStep(50, -1)).toBe(55);
    expect(wheelStep(2, 1)).toBe(0);
  });
});

describe("FaderModel", () => {
  it("ignores external values while dragging and shortly after release", () => {
    const m = new FaderModel(50);
    m.begin();
    m.drag(80);
    expect(m.external(60, 0)).toBe(false);
    expect(m.value).toBe(80);
    m.end(1000);
    expect(m.external(70, 1000 + IGNORE_AFTER_RELEASE_MS - 1)).toBe(false);
    expect(m.external(70, 1000 + IGNORE_AFTER_RELEASE_MS)).toBe(true);
    expect(m.value).toBe(70);
  });

  it("set() applies a local value and opens the ignore window", () => {
    const m = new FaderModel(10);
    expect(m.set(100, 5)).toBe(100);
    expect(m.external(10, 6)).toBe(false);
  });
});
