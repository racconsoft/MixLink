<script lang="ts">
  import { onMount } from "svelte";
  import { activeBackend, daemonState, formatError, pushToast, send } from "../lib/stores";
  import { t } from "../lib/i18n";

  let { onclose }: { onclose: () => void } = $props();
  let autostart = $state(false);
  const channels = $derived(($daemonState?.config.channels ?? []).filter((c) => c.kind === "virtual"));

  onMount(() => {
    void activeBackend()
      .getAutostart()
      .then((v) => (autostart = v))
      .catch((e) => pushToast(formatError(e)));
  });

  async function toggleAutostart(e: Event & { currentTarget: HTMLInputElement }) {
    const checkbox = e.currentTarget;
    const next = checkbox.checked;
    try {
      await activeBackend().setAutostart(next);
      autostart = next;
    } catch (err) {
      checkbox.checked = autostart;
      pushToast(formatError(err));
    }
  }

  async function changeFallback(e: Event & { currentTarget: HTMLSelectElement }) {
    const select = e.currentTarget;
    const value = select.value;
    const ok = await send({ cmd: "set_fallback_channel", args: { channel: value } });
    if (!ok) select.value = $daemonState?.config.fallback_channel ?? "";
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="dialog-backdrop" role="presentation" onclick={onclose}>
  <div class="dialog" role="dialog" aria-label={t("settings.title")} onclick={(e) => e.stopPropagation()}>
    <h2>{t("settings.title")}</h2>
    <label class="check"><input type="checkbox" checked={autostart} onchange={toggleAutostart} />{t("settings.autostart")}</label>
    <label class="field">{t("settings.fallback")}
      <select value={$daemonState?.config.fallback_channel} onchange={changeFallback}>
        {#each channels as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
      </select>
    </label>
    <div class="actions"><button class="btn" onclick={onclose}>{t("settings.close")}</button></div>
  </div>
</div>

<style>
  .check { display: flex; align-items: center; gap: 8px; }
  .field { display: flex; flex-direction: column; gap: 6px; color: var(--text-dim); }
  .actions { display: flex; justify-content: flex-end; }
</style>
