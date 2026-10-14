<script lang="ts">
  import { meterLevel } from "../lib/meter";

  interface Props { peaks?: [number, number]; label?: string }
  let { peaks = [0, 0], label = "" }: Props = $props();
</script>

<div class="meter" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={1} aria-valuenow={Math.max(...peaks)}>
  {#each peaks as peak, i (i)}
    <div class="bar"><div class="level" style:width="{meterLevel(peak) * 100}%"></div></div>
  {/each}
</div>

<style>
  .meter { display: flex; flex-direction: column; gap: 2px; }
  .bar { height: 3px; background: var(--border); border-radius: 2px; overflow: hidden; }
  .level { height: 100%; background: var(--accent); transition: width 40ms linear; }
</style>
