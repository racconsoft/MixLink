<script lang="ts">
  import { onMount } from "svelte";
  import { Settings } from "@lucide/svelte";
  import { connected, daemonState, initBackend, meters } from "./lib/stores";
  import { t } from "./lib/i18n";
  import type { Channel } from "./lib/types";
  import ChannelStrip from "./components/ChannelStrip.svelte";
  import AddChannelTile from "./components/AddChannelTile.svelte";
  import ChannelDialog from "./components/ChannelDialog.svelte";
  import OutputRow from "./components/OutputRow.svelte";
  import SettingsDialog from "./components/SettingsDialog.svelte";
  import ConnectionOverlay from "./components/ConnectionOverlay.svelte";
  import WarningBanner from "./components/WarningBanner.svelte";
  import Toasts from "./components/Toasts.svelte";

  let dialog = $state<null | { kind: "create" } | { kind: "edit"; channel: Channel }>(null);
  let settingsOpen = $state(false);

  onMount(() => initBackend());
</script>

<div class="app">
  <header class="titlebar">
    <span class="title">{t("app.title")}</span>
    <span class="conn" class:ok={$connected}>● {$connected ? t("app.connected") : t("app.disconnected")}</span>
    <span class="spacer"></span>
    <button class="icon-btn" title={t("app.settings")} aria-label={t("app.settings")} onclick={() => (settingsOpen = true)}><Settings size={16} /></button>
  </header>
  <WarningBanner />

  <main>
    <h2 class="section">{t("channels.title")}</h2>
    <div class="strips" data-testid="channels">
      {#if $daemonState}
        {#each $daemonState.config.channels as channel (channel.id)}
          <ChannelStrip
            {channel}
            fallback={channel.id === $daemonState.config.fallback_channel}
            peaks={$meters?.channels[channel.id] ?? [0, 0]}
            allApps={$daemonState.apps}
            onedit={(c) => (dialog = { kind: "edit", channel: c })}
          />
        {/each}
        <AddChannelTile oncreate={() => (dialog = { kind: "create" })} />
      {/if}
    </div>

    <h2 class="section">{t("outputs.title")}</h2>
    <div class="outputs" data-testid="outputs">
      <OutputRow kind="monitor" />
      <OutputRow kind="stream" />
    </div>
  </main>
  {#if dialog}<ChannelDialog mode={dialog} onclose={() => (dialog = null)} />{/if}
  {#if settingsOpen}<SettingsDialog onclose={() => (settingsOpen = false)} />{/if}
  <ConnectionOverlay />
  <Toasts />
</div>

<style>
  .app { display: flex; flex-direction: column; height: 100%; }
  .titlebar { display: flex; align-items: center; gap: 10px; padding: 8px 12px;
    background: #2a2e32; border-bottom: 1px solid var(--border); }
  .title { font-weight: 600; font-size: 13px; }
  .conn { font-size: 11px; padding: 2px 8px; border-radius: 10px; color: var(--danger);
    background: color-mix(in srgb, var(--danger) 18%, transparent); }
  .conn.ok { color: var(--ok); background: color-mix(in srgb, var(--ok) 18%, transparent); }
  .spacer { flex: 1; }
  main { flex: 1; overflow: auto; padding: 0 0 12px; }
  .section { margin: 12px 12px 6px; font-size: 10px; font-weight: 600; letter-spacing: 0.08em;
    text-transform: uppercase; color: var(--text-dim); }
  .strips { display: flex; gap: 8px; padding: 0 12px 6px; overflow-x: auto; align-items: stretch; }
  .outputs { display: flex; flex-direction: column; gap: 6px; padding: 0 12px; }
</style>
