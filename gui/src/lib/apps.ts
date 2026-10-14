import type { ActiveApp, Channel } from "./types";

export const APP_MIME = "application/x-mixlink-app";

export interface AppEntry { app: string; name: string; iconName: string | null; running: boolean }

function firstByApp(allApps: ActiveApp[]): Map<string, ActiveApp> {
  const map = new Map<string, ActiveApp>();
  for (const a of allApps) if (!map.has(a.app)) map.set(a.app, a);
  return map;
}

/** Apps shown in a channel: its rules (running or not), then other apps currently playing in it. */
export function channelApps(channel: Channel, allApps: ActiveApp[]): AppEntry[] {
  const byApp = firstByApp(allApps);
  const out: AppEntry[] = channel.apps.map((rule) => {
    const a = byApp.get(rule);
    return { app: rule, name: a?.name ?? rule, iconName: a?.icon ?? null, running: a !== undefined };
  });
  for (const a of allApps) {
    if (a.channel === channel.id && !out.some((e) => e.app === a.app)) {
      out.push({ app: a.app, name: a.name, iconName: a.icon ?? null, running: true });
    }
  }
  return out;
}

export function runningApps(allApps: ActiveApp[]): AppEntry[] {
  return [...firstByApp(allApps).values()].map((a) => ({ app: a.app, name: a.name, iconName: a.icon ?? null, running: true }));
}
