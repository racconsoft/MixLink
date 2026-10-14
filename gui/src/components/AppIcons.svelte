<script lang="ts">
  import { Mic } from "@lucide/svelte";
  import type { ActiveApp, Channel } from "../lib/types";
  import { channelApps } from "../lib/apps";
  import { openPopover } from "../lib/popover";
  import { t } from "../lib/i18n";
  import AppIcon from "./AppIcon.svelte";
  import AppPicker from "./AppPicker.svelte";

  let { channel, allApps }: { channel: Channel; allApps: ActiveApp[] } = $props();
  const id = $derived(`apps:${channel.id}`);
  const picking = $derived($openPopover === id);
  const entries = $derived(channelApps(channel, allApps));

  let trigger = $state<HTMLElement>();

  function toggle(e: Event) {
    e.stopPropagation();
    openPopover.set(picking ? null : id);
  }
  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle(e);
    }
  }
</script>

{#if channel.kind === "input"}
  <div class="apps input" title={t("channel.inputHint")}><Mic size={16} /></div>
{:else}
  <div class="wrap">
    <!-- Not a <button>: draggable icons inside a button do not start drags in WebKitGTK. -->
    <div class="apps" role="button" tabindex="0" title={t("apps.assign")} aria-label={t("apps.assign")}
      aria-expanded={picking} bind:this={trigger} onclick={toggle} onkeydown={keydown}>
      {#each entries as entry (entry.app)}<AppIcon {entry} />{/each}
    </div>
    {#if picking}<AppPicker {channel} {allApps} {trigger} onclose={() => openPopover.set(null)} />{/if}
  </div>
{/if}

<style>
  .wrap { position: relative; }
  .apps { width: 100%; min-height: 28px; background: var(--input); border: 1px solid transparent; border-radius: var(--radius);
    display: flex; flex-wrap: wrap; align-items: center; justify-content: center; gap: 4px; padding: 3px; cursor: pointer; }
  .apps:hover, .apps:focus-visible { border-color: var(--accent); outline: none; }
  .apps.input { cursor: default; color: var(--text-dim); }
  .apps.input:hover { border-color: transparent; }
</style>
