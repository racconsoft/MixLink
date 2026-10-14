<script lang="ts">
  import { onMount } from "svelte";
  import { connected, daemonState, initBackend } from "./lib/stores";
  import { t } from "./lib/i18n";

  onMount(() => initBackend());
</script>

<main>
  <h1>{t("app.title")}</h1>
  <p>{$connected ? t("app.connected") : t("app.disconnected")}</p>
  <pre>{JSON.stringify($daemonState?.config.channels.map((c) => c.name) ?? [], null, 2)}</pre>
</main>
