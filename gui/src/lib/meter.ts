/** Peak (linear) → 0..1 on a -60..0 dB scale. */
export function meterLevel(peak: number): number {
  if (!(peak > 0)) return 0;
  const db = 20 * Math.log10(peak);
  return Math.max(0, Math.min(1, (db + 60) / 60));
}
