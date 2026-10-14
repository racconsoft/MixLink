<script lang="ts">
  import { activeBackend, connected, formatError, pushToast } from "../lib/stores";
  import { t } from "../lib/i18n";

  let starting = $state(false);
  async function start() {
    starting = true;
    try {
      await activeBackend().startService();
    } catch (e) {
      pushToast(t("connection.startFailed", { error: formatError(e) }));
    } finally {
      starting = false;
    }
  }
</script>

{#if $connected === false}
  <div class="overlay" role="alertdialog" aria-label={t("connection.title")}>
    <div class="box surface">
      <h2>{t("connection.title")}</h2>
      <p>{t("connection.hint")}</p>
      <button class="btn primary" disabled={starting} onclick={start}>{t("connection.start")}</button>
    </div>
  </div>
{/if}

<style>
  .overlay { position: fixed; inset: 41px 0 0 0; background: rgba(27, 30, 32, 0.82); display: flex;
    align-items: center; justify-content: center; z-index: 25; }
  .box { padding: 20px 24px; text-align: center; display: flex; flex-direction: column; gap: 10px; align-items: center; }
  h2 { margin: 0; font-size: 14px; }
  p { margin: 0; color: var(--text-dim); }
</style>
