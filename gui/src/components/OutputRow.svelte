<script lang="ts">
  import { Ear, Headphones, Radio, Volume2, VolumeX } from "@lucide/svelte";
  import type { MixKind } from "../lib/types";
  import { daemonState, meters, send } from "../lib/stores";
  import { t } from "../lib/i18n";
  import Fader from "./Fader.svelte";
  import Meter from "./Meter.svelte";

  let { kind }: { kind: MixKind } = $props();
  const config = $derived($daemonState?.config);
  const mix = $derived(config?.mixes[kind]);
  const title = $derived(kind === "monitor" ? t("outputs.monitor") : t("outputs.stream"));
  const listening = $derived(config?.listen === kind);
  const sinks = $derived(($daemonState?.devices ?? []).filter((d) => d.kind === "sink"));
  const current = $derived(mix?.output_device ?? "");
  const missing = $derived(current !== "" && !sinks.some((d) => d.name === current));

  async function changeDevice(e: Event & { currentTarget: HTMLSelectElement }) {
    const select = e.currentTarget;
    const value = select.value;
    const ok = await send({ cmd: "set_mix", args: { mix: "monitor", output_device: value } });
    if (!ok) select.value = current;
  }
</script>

{#if mix}
  <div class="row surface" data-testid="output-{kind}">
    <span class="label">{title}</span>
    {#if kind === "monitor"}
      <span class="device"><Headphones size={14} />
        <select aria-label={title} value={current} onchange={changeDevice}>
          {#if missing}<option value={current}>{t("outputs.disconnected", { name: current })}</option>{/if}
          {#each sinks as d (d.name)}<option value={d.name}>{d.description}</option>{/each}
        </select>
      </span>
    {:else}
      <span class="device fixed"><Radio size={14} />{t("outputs.streamTarget")}</span>
    {/if}
    <div class="meter"><Meter peaks={$meters?.[kind] ?? [0, 0]} label={title} /></div>
    <button class="icon-btn" class:muted={mix.muted} title={t("outputs.mute", { name: title })}
      onclick={() => send({ cmd: "set_mix", args: { mix: kind, muted: !mix.muted } })}>
      {#if mix.muted}<VolumeX size={16} />{:else}<Volume2 size={16} />{/if}
    </button>
    <div class="volume">
      <Fader orientation="horizontal" value={mix.volume} label={t("outputs.volume", { name: title })} muted={mix.muted}
        onchange={(v) => send({ cmd: "set_mix", args: { mix: kind, volume: v } })} />
    </div>
    <button class="icon-btn" class:active={listening} aria-pressed={listening} title={t("outputs.listen")}
      onclick={() => !listening && send({ cmd: "set_listen", args: { mix: kind } })}><Ear size={16} /></button>
  </div>
{/if}

<style>
  .row { display: flex; align-items: center; gap: 10px; padding: 7px 10px; }
  .label { width: 90px; font-weight: 600; flex: none; }
  .device { display: flex; align-items: center; gap: 6px; width: 230px; flex: none; color: var(--text-dim); }
  .device select { flex: 1; min-width: 0; }
  .device.fixed { background: var(--input); border-radius: var(--radius); padding: 5px 8px; }
  .meter { flex: 1; min-width: 80px; }
  .volume { width: 140px; flex: none; }
</style>
