<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import type { Channel } from "../lib/types";
  import { send } from "../lib/stores";
  import { t } from "../lib/i18n";

  let { channel, fallback, onedit }: { channel: Channel; fallback: boolean; onedit: (c: Channel) => void } = $props();
  let open = $state(false);

  function run(action: () => void) {
    open = false;
    action();
  }
  function remove() {
    if (window.confirm(t("channel.confirmDelete", { name: channel.name }))) {
      void send({ cmd: "delete_channel", args: { channel: channel.id } });
    }
  }
</script>

<svelte:window onclick={() => (open = false)} />
<div class="menu">
  <button class="icon-btn" title={t("channel.menu")} aria-label={t("channel.menu")}
    onclick={(e) => { e.stopPropagation(); open = !open; }}><ChevronDown size={14} /></button>
  {#if open}
    <div class="popover" role="menu">
      <button class="item" onclick={() => run(() => onedit(channel))}>{t("channel.rename")}</button>
      <button class="item" onclick={() => run(() => onedit(channel))}>{t("channel.iconColor")}</button>
      <button class="item" disabled={fallback || channel.kind === "input"}
        onclick={() => run(() => void send({ cmd: "set_fallback_channel", args: { channel: channel.id } }))}>{t("channel.setFallback")}</button>
      <button class="item danger" disabled={fallback} onclick={() => run(remove)}>{t("channel.delete")}</button>
    </div>
  {/if}
</div>

<style>
  .menu { position: relative; }
  .popover { right: 0; top: 100%; }
  .danger { color: var(--danger); }
</style>
