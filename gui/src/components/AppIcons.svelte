<script lang="ts">
  import { Mic } from "@lucide/svelte";
  import type { ActiveApp, Channel } from "../lib/types";
  import { channelApps } from "../lib/apps";
  import { t } from "../lib/i18n";
  import AppIcon from "./AppIcon.svelte";
  import AppPicker from "./AppPicker.svelte";

  let { channel, allApps }: { channel: Channel; allApps: ActiveApp[] } = $props();
  let picking = $state(false);
  const entries = $derived(channelApps(channel, allApps));
</script>

{#if channel.kind === "input"}
  <div class="apps input" title={t("channel.inputHint")}><Mic size={16} /></div>
{:else}
  <div class="wrap">
    <button class="apps" title={t("apps.assign")} onclick={(e) => { e.stopPropagation(); picking = !picking; }}>
      {#each entries as entry (entry.app)}<AppIcon {entry} />{/each}
    </button>
    {#if picking}<AppPicker {channel} {allApps} onclose={() => (picking = false)} />{/if}
  </div>
{/if}

<style>
  .wrap { position: relative; }
  .apps { width: 100%; min-height: 28px; background: var(--input); border: 1px solid transparent; border-radius: var(--radius);
    display: flex; flex-wrap: wrap; align-items: center; justify-content: center; gap: 4px; padding: 3px; cursor: pointer; }
  .apps:hover { border-color: var(--accent); }
  .apps.input { cursor: default; color: var(--text-dim); }
  .apps.input:hover { border-color: transparent; }
</style>
