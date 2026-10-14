<script lang="ts">
  import type { ActiveApp, Channel } from "../lib/types";
  import { runningApps } from "../lib/apps";
  import { send } from "../lib/stores";
  import { t } from "../lib/i18n";
  import AppIcon from "./AppIcon.svelte";

  let { channel, allApps, onclose }: { channel: Channel; allApps: ActiveApp[]; onclose: () => void } = $props();
  const entries = $derived(runningApps(allApps));

  function toggle(app: string, assigned: boolean) {
    void send(assigned
      ? { cmd: "unassign_app", args: { app } }
      : { cmd: "assign_app", args: { app, channel: channel.id } });
  }
</script>

<svelte:window onclick={onclose} />
<div class="popover picker" role="dialog" aria-label={t("apps.assign")} onclick={(e) => e.stopPropagation()}>
  {#if entries.length === 0}
    <div class="empty">{t("apps.none")}</div>
  {:else}
    {#each entries as entry (entry.app)}
      {@const assigned = channel.apps.includes(entry.app)}
      <label class="item">
        <input type="checkbox" checked={assigned} onchange={() => toggle(entry.app, assigned)} />
        <AppIcon {entry} />
        <span>{entry.name}</span>
      </label>
    {/each}
  {/if}
</div>

<style>
  .picker { top: 100%; left: 0; margin-top: 4px; max-height: 260px; overflow: auto; }
  .item { display: flex; align-items: center; gap: 8px; padding: 5px 12px; cursor: pointer; }
  .item:hover { background: var(--input); }
  .empty { padding: 8px 12px; color: var(--hint); }
</style>
