<script lang="ts">
  import { APP_MIME, type AppEntry } from "../lib/apps";
  import { getAppIcon } from "../lib/appIcons";
  import { t } from "../lib/i18n";

  let { entry }: { entry: AppEntry } = $props();
  let src = $state<string | null>(null);

  $effect(() => {
    let alive = true;
    void getAppIcon(entry.app, entry.iconName).then((url) => {
      if (alive) src = url;
    });
    return () => (alive = false);
  });

  function dragstart(e: DragEvent) {
    e.dataTransfer?.setData(APP_MIME, entry.app);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }
</script>

<span
  class="app"
  class:dim={!entry.running}
  draggable="true"
  ondragstart={dragstart}
  title={entry.running ? entry.name : t("apps.notRunning", { name: entry.name })}
>
  {#if src}
    <img {src} alt={entry.name} />
  {:else}
    <span class="letter">{entry.name.charAt(0).toUpperCase()}</span>
  {/if}
</span>

<style>
  .app { width: 20px; height: 20px; display: inline-flex; align-items: center; justify-content: center; cursor: grab; }
  .app.dim { opacity: 0.35; }
  img { width: 20px; height: 20px; object-fit: contain; pointer-events: none; }
  .letter { width: 18px; height: 18px; border-radius: 50%; background: var(--border); color: var(--text);
    font-size: 10px; font-weight: 600; display: flex; align-items: center; justify-content: center; }
</style>
