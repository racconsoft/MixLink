<script lang="ts">
  import { Headphones, Link, Radio, Star } from "@lucide/svelte";
  import type { ActiveApp, Channel, MixKind } from "../lib/types";
  import { APP_MIME } from "../lib/apps";
  import { channelIcon } from "../lib/icons";
  import { send } from "../lib/stores";
  import { t } from "../lib/i18n";
  import AppIcons from "./AppIcons.svelte";
  import ChannelMenu from "./ChannelMenu.svelte";
  import Fader from "./Fader.svelte";
  import Meter from "./Meter.svelte";

  interface Props {
    channel: Channel;
    fallback: boolean;
    peaks: [number, number];
    allApps: ActiveApp[];
    onedit: (c: Channel) => void;
  }
  let { channel, fallback, peaks, allApps, onedit }: Props = $props();
  const Icon = $derived(channelIcon(channel.icon));
  const isInput = $derived(channel.kind === "input");
  let dropping = $state(false);

  function volume(mix: MixKind, value: number) {
    void send({ cmd: "set_volume", args: { channel: channel.id, mix, value } });
  }
  function mute(mix: MixKind) {
    void send({ cmd: "set_mute", args: { channel: channel.id, mix, muted: !channel[mix].muted } });
  }
  function link() {
    void send({ cmd: "set_link", args: { channel: channel.id, linked: !channel.linked } });
  }
  function dragover(e: DragEvent) {
    if (!isInput && e.dataTransfer?.types.includes(APP_MIME)) {
      e.preventDefault();
      dropping = true;
    }
  }
  function drop(e: DragEvent) {
    dropping = false;
    const app = e.dataTransfer?.getData(APP_MIME);
    if (!isInput && app) {
      e.preventDefault();
      void send({ cmd: "assign_app", args: { app, channel: channel.id } });
    }
  }
</script>

<section class="strip surface" class:dropping data-testid="strip-{channel.id}"
  ondragover={dragover} ondragleave={() => (dropping = false)} ondrop={drop}>
  <header>
    <span class="ico" style:background={channel.color}><Icon size={13} /></span>
    <span class="name" title={channel.name}>{channel.name}</span>
    {#if fallback}<span class="fallback" title={t("channel.fallback")}><Star size={11} /></span>{/if}
    <ChannelMenu {channel} {fallback} {onedit} />
  </header>
  <Meter {peaks} label={t("channel.meter", { name: channel.name })} />
  <AppIcons {channel} {allApps} />
  <div class="faders">
    <Fader value={channel.monitor.volume} label={t("channel.monitorVolume", { name: channel.name })} onchange={(v) => volume("monitor", v)} />
    <Fader value={channel.stream.volume} label={t("channel.streamVolume", { name: channel.name })} onchange={(v) => volume("stream", v)} />
  </div>
  <div class="buttons">
    <button class="icon-btn" class:muted={channel.monitor.muted} class:active={!channel.monitor.muted}
      aria-pressed={channel.monitor.muted} title={t("channel.muteMonitor")} onclick={() => mute("monitor")}><Headphones size={16} /></button>
    <button class="icon-btn" class:active={channel.linked} aria-pressed={channel.linked}
      title={t("channel.link")} onclick={link}><Link size={14} /></button>
    <button class="icon-btn" class:muted={channel.stream.muted} class:active={!channel.stream.muted}
      aria-pressed={channel.stream.muted} title={t("channel.muteStream")} onclick={() => mute("stream")}><Radio size={16} /></button>
  </div>
</section>

<style>
  .strip { width: 108px; flex: none; padding: 8px; display: flex; flex-direction: column; gap: 8px; }
  .strip.dropping { border-color: var(--accent); }
  header { display: flex; align-items: center; gap: 5px; }
  .ico { width: 18px; height: 18px; border-radius: 3px; display: inline-flex; align-items: center; justify-content: center; color: #fff; flex: none; }
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .fallback { color: var(--accent); display: inline-flex; }
  .faders { display: flex; justify-content: space-around; height: 180px; }
  .buttons { display: flex; justify-content: space-around; }
</style>
