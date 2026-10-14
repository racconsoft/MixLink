import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { MESSAGES, t } from "./i18n";

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return sources(p);
    return /\.(svelte|ts)$/.test(name) && !name.endsWith(".test.ts") ? [p] : [];
  });
}

describe("i18n", () => {
  it("replaces placeholders", () => {
    expect(t("fader.value", { value: 42 })).toBe("42 %");
  });

  it("returns the key for unknown keys", () => {
    expect(t("nope.missing")).toBe("nope.missing");
  });

  it("every key used in the sources exists in en.json", () => {
    const used = new Set<string>();
    for (const file of sources(join(__dirname, ".."))) {
      for (const m of readFileSync(file, "utf8").matchAll(/\bt\(\s*"([a-zA-Z0-9_.]+)"/g)) used.add(m[1]);
    }
    const missing = [...used].filter((k) => !(k in MESSAGES));
    expect(missing).toEqual([]);
  });
});
