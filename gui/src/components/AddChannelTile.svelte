<script lang="ts">
  import { Plus } from "@lucide/svelte";
  import { t } from "../lib/i18n";

  let { oncreate }: { oncreate: () => void } = $props();
</script>

<button class="tile" title={t("channel.add")} aria-label={t("channel.add")} onclick={oncreate}><Plus size={22} /></button>

<style>
  .tile { width: 60px; flex: none; border: 1px dashed #4d4d4d; border-radius: var(--radius); background: none;
    color: var(--hint); display: flex; align-items: center; justify-content: center; cursor: pointer; }
  .tile:hover { border-color: var(--accent); color: var(--accent); }
</style>
