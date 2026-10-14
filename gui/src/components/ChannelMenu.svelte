<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import type { Channel } from "../lib/types";
  import { send } from "../lib/stores";
  import { anchored, openPopover } from "../lib/popover";
  import { t } from "../lib/i18n";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  let { channel, fallback, onedit }: { channel: Channel; fallback: boolean; onedit: (c: Channel) => void } = $props();
  const id = $derived(`menu:${channel.id}`);
  const open = $derived($openPopover === id);
  let confirming = $state(false);
  let trigger = $state<HTMLElement>();

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    openPopover.set(open ? null : id);
  }
  function run(action: () => void) {
    openPopover.set(null);
    action();
  }
  function remove() {
    openPopover.set(null);
    confirming = true;
  }
  function confirmDelete() {
    confirming = false;
    void send({ cmd: "delete_channel", args: { channel: channel.id } });
  }
</script>

<svelte:window onclick={() => { if (open) openPopover.set(null); }} />
<div class="menu">
  <button class="icon-btn" title={t("channel.menu")} aria-label={t("channel.menu")} bind:this={trigger} onclick={toggle}><ChevronDown size={14} /></button>
  {#if open}
    <div class="popover" role="menu" use:anchored={{ trigger, align: "right" }}>
      <button class="item" onclick={() => run(() => onedit(channel))}>{t("channel.rename")}</button>
      <button class="item" onclick={() => run(() => onedit(channel))}>{t("channel.iconColor")}</button>
      <button class="item" disabled={fallback || channel.kind === "input"}
        onclick={() => run(() => void send({ cmd: "set_fallback_channel", args: { channel: channel.id } }))}>{t("channel.setFallback")}</button>
      <button class="item danger" disabled={fallback} onclick={remove}>{t("channel.delete")}</button>
    </div>
  {/if}
</div>

{#if confirming}
  <ConfirmDialog
    message={t("channel.confirmDelete", { name: channel.name })}
    confirmLabel={t("channel.delete")}
    onconfirm={confirmDelete}
    oncancel={() => (confirming = false)}
  />
{/if}

<style>
  .menu { position: relative; }
  .danger { color: var(--danger); }
</style>
