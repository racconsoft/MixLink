import { meterLevel } from "./meter";

describe("meterLevel", () => {
  it("maps -60..0 dB linearly", () => {
    expect(meterLevel(0)).toBe(0);
    expect(meterLevel(1)).toBe(1);
    expect(meterLevel(2)).toBe(1);
    expect(meterLevel(0.001)).toBeCloseTo(0, 5);
    expect(meterLevel(Math.pow(10, -30 / 20))).toBeCloseTo(0.5, 5);
  });
});
