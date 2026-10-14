<script lang="ts">
  import type { Channel } from "../lib/types";
  import { COLORS, ICON_IDS, channelIcon } from "../lib/icons";
  import { send } from "../lib/stores";
  import { t } from "../lib/i18n";

  type Mode = { kind: "create" } | { kind: "edit"; channel: Channel };
  let { mode, onclose }: { mode: Mode; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  const original = mode.kind === "edit" ? mode.channel : null;
  let name = $state(original?.name ?? "");
  let icon = $state(original?.icon ?? "default");
  let color = $state(original?.color ?? COLORS[0]);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    let ok = true;
    if (!original) {
      ok = await send({ cmd: "create_channel", args: { name: trimmed, icon, color } });
    } else {
      if (trimmed !== original.name) ok = await send({ cmd: "rename_channel", args: { channel: original.id, name: trimmed } });
      if (ok && (icon !== original.icon || color !== original.color)) {
        ok = await send({ cmd: "update_channel", args: { channel: original.id, icon, color } });
      }
    }
    if (ok) onclose();
  }
</script>

<div class="dialog-backdrop" role="presentation" onclick={onclose}>
  <form class="dialog" onsubmit={submit} onclick={(e) => e.stopPropagation()}>
    <h2>{original ? t("dialog.editTitle") : t("dialog.createTitle")}</h2>
    <label class="field">{t("dialog.name")}<input type="text" bind:value={name} autofocus /></label>
    <div class="field">
      <span>{t("dialog.icon")}</span>
      <div class="grid">
        {#each ICON_IDS as id (id)}
          {@const Icon = channelIcon(id)}
          <button type="button" class="choice" class:selected={icon === id} title={id} onclick={() => (icon = id)}><Icon size={16} /></button>
        {/each}
      </div>
    </div>
    <div class="field">
      <span>{t("dialog.color")}</span>
      <div class="grid">
        {#each COLORS as c (c)}
          <button type="button" class="swatch" class:selected={color === c} title={c} style:background={c} onclick={() => (color = c)}></button>
        {/each}
      </div>
    </div>
    <div class="actions">
      <button type="button" class="btn" onclick={onclose}>{t("dialog.cancel")}</button>
      <button type="submit" class="btn primary" disabled={!name.trim()}>{original ? t("dialog.save") : t("dialog.create")}</button>
    </div>
  </form>
</div>

<style>
  .field { display: flex; flex-direction: column; gap: 6px; color: var(--text-dim); }
  .grid { display: flex; flex-wrap: wrap; gap: 6px; max-width: 300px; }
  .choice { width: 30px; height: 30px; background: var(--input); border: 1px solid var(--border); border-radius: var(--radius);
    display: flex; align-items: center; justify-content: center; cursor: pointer; }
  .swatch { width: 22px; height: 22px; border-radius: 50%; border: 2px solid transparent; cursor: pointer; }
  .selected { border-color: var(--accent); outline: 1px solid var(--accent); }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
</style>
