import { activeBackend } from "./stores";

const cache = new Map<string, Promise<string | null>>();

export function getAppIcon(app: string, iconName: string | null): Promise<string | null> {
  let hit = cache.get(app);
  if (!hit) {
    hit = activeBackend().appIcon(app, iconName).catch(() => null);
    cache.set(app, hit);
  }
  return hit;
}
