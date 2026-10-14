<script lang="ts">
  import { toasts } from "../lib/stores";
</script>

<div class="toasts" aria-live="polite">
  {#each $toasts as toast (toast.id)}<div class="toast surface">{toast.text}</div>{/each}
</div>

<style>
  .toasts { position: fixed; right: 12px; bottom: 12px; display: flex; flex-direction: column; gap: 6px; z-index: 40; max-width: 420px; }
  .toast { padding: 8px 12px; border-left: 3px solid var(--danger); box-shadow: 0 6px 18px rgba(0, 0, 0, 0.45); }
</style>
