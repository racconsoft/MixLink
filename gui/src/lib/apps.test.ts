import { channelApps, runningApps } from "./apps";
import type { ActiveApp, Channel } from "./types";

const music: Channel = {
  id: "music", name: "Music", kind: "virtual", icon: "music", color: "#000", linked: false,
  apps: ["spotify", "vlc"], monitor: { volume: 100, muted: false }, stream: { volume: 100, muted: false },
};
const apps: ActiveApp[] = [
  { node_id: 1, app: "spotify", name: "Spotify", icon: "spotify-client", channel: "music" },
  { node_id: 2, app: "firefox", name: "Firefox", icon: null, channel: "music" },
  { node_id: 3, app: "firefox", name: "Firefox", icon: null, channel: "music" },
  { node_id: 4, app: "discord", name: "Discord", icon: null, channel: "voice-chat" },
];

describe("apps", () => {
  it("lists rules first, then other apps playing here, without duplicates", () => {
    expect(channelApps(music, apps)).toEqual([
      { app: "spotify", name: "Spotify", iconName: "spotify-client", running: true },
      { app: "vlc", name: "vlc", iconName: null, running: false },
      { app: "firefox", name: "Firefox", iconName: null, running: true },
    ]);
  });

  it("running apps are unique by identity", () => {
    expect(runningApps(apps).map((a) => a.app)).toEqual(["spotify", "firefox", "discord"]);
  });

  it("duplicate rules from a hand-edited config are shown once", () => {
    const dup: Channel = { ...music, apps: ["spotify", "vlc", "spotify", "vlc"] };
    expect(channelApps(dup, apps).map((e) => e.app)).toEqual(["spotify", "vlc", "firefox"]);
  });
});
