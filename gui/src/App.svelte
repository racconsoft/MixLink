<script lang="ts">
  import { onMount } from "svelte";
  import { Settings } from "@lucide/svelte";
  import { connected, daemonState, initBackend } from "./lib/stores";
  import { t } from "./lib/i18n";

  onMount(() => initBackend());
</script>

<div class="app">
  <header class="titlebar">
    <span class="title">{t("app.title")}</span>
    <span class="conn" class:ok={$connected}>● {$connected ? t("app.connected") : t("app.disconnected")}</span>
    <span class="spacer"></span>
    <button class="icon-btn" title={t("app.settings")} aria-label={t("app.settings")}><Settings size={16} /></button>
  </header>

  <main>
    <h2 class="section">{t("channels.title")}</h2>
    <div class="strips" data-testid="channels">
      {#each $daemonState?.config.channels ?? [] as channel (channel.id)}
        <div class="surface placeholder" data-testid="strip-{channel.id}">{channel.name}</div>
      {/each}
    </div>

    <h2 class="section">{t("outputs.title")}</h2>
    <div class="outputs" data-testid="outputs">
      <div class="surface row">{t("outputs.monitor")}</div>
      <div class="surface row">{t("outputs.stream")}</div>
    </div>
  </main>
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
  .placeholder { width: 108px; height: 300px; padding: 8px; flex: none; }
  .outputs { display: flex; flex-direction: column; gap: 6px; padding: 0 12px; }
  .row { padding: 8px 10px; }
</style>
