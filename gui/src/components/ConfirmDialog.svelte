<script lang="ts">
  import { t } from "../lib/i18n";

  let { message, confirmLabel, onconfirm, oncancel }: {
    message: string;
    confirmLabel: string;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();
</script>

<div class="dialog-backdrop" role="presentation" onclick={oncancel}>
  <div class="dialog" role="alertdialog" aria-modal="true" aria-label={message} onclick={(e) => e.stopPropagation()}>
    <p class="message">{message}</p>
    <div class="actions">
      <button type="button" class="btn" onclick={oncancel}>{t("dialog.cancel")}</button>
      <button type="button" class="btn danger" onclick={onconfirm}>{confirmLabel}</button>
    </div>
  </div>
</div>

<style>
  .message { margin: 0; color: var(--text-dim); max-width: 320px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  .danger { background: var(--danger); border-color: var(--danger); color: #fff; }
  .danger:hover { filter: brightness(1.1); }
</style>
