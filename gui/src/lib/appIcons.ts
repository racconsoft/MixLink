import { activeBackend } from "./stores";

const cache = new Map<string, Promise<string | null>>();

export function getAppIcon(app: string, iconName: string | null): Promise<string | null> {
  const key = `${app}|${iconName ?? ""}`;
  let hit = cache.get(key);
  if (!hit) {
    hit = activeBackend().appIcon(app, iconName).catch(() => null);
    cache.set(key, hit);
  }
  return hit;
}
