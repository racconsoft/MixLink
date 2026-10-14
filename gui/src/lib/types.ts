export type MixKind = "monitor" | "stream";
export type MixTarget = MixKind | "both";
export interface Level { volume: number; muted: boolean }
export interface Channel {
  id: string; name: string; kind: "virtual" | "input"; source?: string;
  icon: string; color: string; linked: boolean; apps: string[]; monitor: Level; stream: Level;
}
export interface Mix { volume: number; muted: boolean; output_device?: string }
export interface Config {
  version: number; fallback_channel: string; listen: MixKind;
  settings: { auto_xlr_profile: boolean };
  mixes: { monitor: Mix; stream: Mix };
  channels: Channel[];
}
export interface ActiveApp { node_id: number; app: string; name: string; icon?: string | null; channel?: string | null }
export interface Device { name: string; description: string; kind: "sink" | "source" }
export interface State { config: Config; apps: ActiveApp[]; devices: Device[]; warnings: string[] }
export interface Meters { channels: Record<string, [number, number]>; monitor: [number, number]; stream: [number, number] }
export interface ErrorBody { code: string; message: string; options?: string[] }
export interface Snapshot { state: State | null; connected: boolean }
export type Command =
  | { cmd: "get_state" }
  | { cmd: "set_volume"; args: { channel: string; mix: MixTarget; value?: number; delta?: number } }
  | { cmd: "set_mute"; args: { channel: string; mix: MixTarget; muted?: boolean } }
  | { cmd: "set_link"; args: { channel: string; linked: boolean } }
  | { cmd: "assign_app"; args: { app: string; channel: string } }
  | { cmd: "unassign_app"; args: { app: string } }
  | { cmd: "create_channel"; args: { name: string; icon?: string; color?: string } }
  | { cmd: "rename_channel"; args: { channel: string; name: string } }
  | { cmd: "update_channel"; args: { channel: string; icon?: string; color?: string } }
  | { cmd: "delete_channel"; args: { channel: string } }
  | { cmd: "set_mix"; args: { mix: MixKind; volume?: number; muted?: boolean; output_device?: string } }
  | { cmd: "set_listen"; args: { mix: MixKind } }
  | { cmd: "set_fallback_channel"; args: { channel: string } };
