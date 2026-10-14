import { throttle } from "./throttle";

describe("throttle", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("fires immediately, then at most once per interval with the latest value", () => {
    const seen: number[] = [];
    const th = throttle((v: number) => seen.push(v), 33);
    th.push(1);
    th.push(2);
    th.push(3);
    expect(seen).toEqual([1]);
    vi.advanceTimersByTime(33);
    expect(seen).toEqual([1, 3]);
  });

  it("flush delivers the pending value right away", () => {
    const seen: number[] = [];
    const th = throttle((v: number) => seen.push(v), 33);
    th.push(1);
    th.push(9);
    th.flush();
    expect(seen).toEqual([1, 9]);
    vi.advanceTimersByTime(100);
    expect(seen).toEqual([1, 9]);
  });
});
