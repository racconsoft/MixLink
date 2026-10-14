import {
  Bell, Circle, Gamepad2, Globe, Headphones, MessageCircle, Mic, MicSignal, Monitor, Music, Phone, Radio, Sparkles, Tv, Video,
} from "@lucide/svelte";

// Deviation: this @lucide/svelte version has no `Podcast` export (the icon was renamed
// upstream to `mic-signal` / `MicSignal`); using that as the podcast icon instead.
const Podcast = MicSignal;

export const CHANNEL_ICONS = {
  mic: Mic, system: Monitor, game: Gamepad2, browser: Globe, chat: MessageCircle, music: Music, sfx: Sparkles,
  video: Video, podcast: Podcast, headphones: Headphones, radio: Radio, tv: Tv, phone: Phone, bell: Bell, default: Circle,
} as const;

export const ICON_IDS = Object.keys(CHANNEL_ICONS);

export function channelIcon(id: string) {
  return CHANNEL_ICONS[id as keyof typeof CHANNEL_ICONS] ?? Circle;
}

export const COLORS = ["#3b82f6", "#64748b", "#ef4444", "#a855f7", "#facc15", "#ec4899", "#22c55e", "#f97316", "#14b8a6", "#e5e7eb"];
