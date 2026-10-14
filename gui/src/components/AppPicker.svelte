<script lang="ts">
  import type { ActiveApp, Channel } from "../lib/types";
  import { runningApps } from "../lib/apps";
  import { send } from "../lib/stores";
  import { anchored } from "../lib/popover";
  import { t } from "../lib/i18n";
  import AppIcon from "./AppIcon.svelte";

  interface Props { channel: Channel; allApps: ActiveApp[]; trigger?: HTMLElement; onclose: () => void }
  let { channel, allApps, trigger, onclose }: Props = $props();
  const entries = $derived(runningApps(allApps));

  async function toggle(e: Event, app: string, assigned: boolean) {
    const box = e.currentTarget as HTMLInputElement;
    const ok = await send(assigned
      ? { cmd: "unassign_app", args: { app } }
      : { cmd: "assign_app", args: { app, channel: channel.id } });
    // On failure no state update arrives; put the checkbox back.
    if (!ok) box.checked = assigned;
  }
</script>

<svelte:window onclick={onclose} />
<div class="popover picker" role="dialog" tabindex="-1" aria-label={t("apps.assign")} use:anchored={{ trigger, align: "left" }}
  onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === "Escape") onclose(); }}>
  {#if entries.length === 0}
    <div class="empty">{t("apps.none")}</div>
  {:else}
    {#each entries as entry (entry.app)}
      {@const assigned = channel.apps.includes(entry.app)}
      <label class="item">
        <input type="checkbox" checked={assigned} onchange={(e) => toggle(e, entry.app, assigned)} />
        <AppIcon {entry} />
        <span>{entry.name}</span>
      </label>
    {/each}
  {/if}
</div>

<style>
  .picker { max-height: min(260px, var(--popover-room, 260px)); overflow: auto; }
  .item { display: flex; align-items: center; gap: 8px; padding: 5px 12px; cursor: pointer; }
  .item:hover { background: var(--input); }
  .empty { padding: 8px 12px; color: var(--hint); }
</style>
