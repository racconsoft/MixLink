<script lang="ts">
  import { X } from "@lucide/svelte";
  import { daemonState } from "../lib/stores";
  import { t } from "../lib/i18n";

  let dismissed = $state<string[]>([]);
  const visible = $derived(($daemonState?.warnings ?? []).filter((w) => !dismissed.includes(w)));
</script>

{#each visible as warning (warning)}
  <div class="banner" role="alert">
    <span>{warning}</span>
    <button class="icon-btn" title={t("warning.dismiss")} onclick={() => (dismissed = [...dismissed, warning])}><X size={14} /></button>
  </div>
{/each}

<style>
  .banner { display: flex; align-items: center; gap: 8px; margin: 8px 12px 0; padding: 6px 10px; border-radius: var(--radius);
    background: color-mix(in srgb, #f67400 22%, var(--surface)); border: 1px solid #f67400; }
  .banner span { flex: 1; }
</style>
