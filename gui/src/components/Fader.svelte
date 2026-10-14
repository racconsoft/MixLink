<script lang="ts">
  import { FaderModel, valueFromPointer, valueFromPointerX, wheelStep } from "../lib/fader";
  import { throttle } from "../lib/throttle";
  import { t } from "../lib/i18n";

  interface Props {
    value: number;
    label: string;
    onchange: (v: number) => void;
    orientation?: "vertical" | "horizontal";
  }
  let { value, label, onchange, orientation = "vertical" }: Props = $props();

  // svelte-ignore state_referenced_locally
  const model = new FaderModel(value);
  // svelte-ignore state_referenced_locally
  let shown = $state(value);
  let hover = $state(false);
  let dragging = $state(false);
  let el: HTMLDivElement;
  const out = throttle((v: number) => onchange(v), 33);

  $effect(() => {
    if (model.external(value, Date.now())) shown = model.value;
  });

  $effect(() => {
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      shown = model.set(wheelStep(shown, e.deltaY), Date.now());
      onchange(shown);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  function position(e: PointerEvent): number {
    const r = el.getBoundingClientRect();
    return orientation === "vertical" ? valueFromPointer(e.clientY, r) : valueFromPointerX(e.clientX, r);
  }
  function down(e: PointerEvent) {
    el.setPointerCapture?.(e.pointerId);
    model.begin();
    dragging = true;
    shown = model.drag(position(e));
    out.push(shown);
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    shown = model.drag(position(e));
    out.push(shown);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    shown = model.end(Date.now());
    out.flush();
  }
  function reset() {
    shown = model.set(100, Date.now());
    onchange(100);
  }
</script>

<div
  class="fader {orientation}"
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={shown}
  bind:this={el}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  ondblclick={reset}
  onpointerenter={() => (hover = true)}
  onpointerleave={() => (hover = false)}
>
  <div class="rail"><div class="fill" style:--level="{shown}%"></div></div>
  <div class="knob" style:--level="{shown}%"></div>
  {#if dragging || hover}<div class="tip">{t("fader.value", { value: shown })}</div>{/if}
</div>

<style>
  .fader { position: relative; touch-action: none; cursor: pointer; }
  .fader.vertical { width: 22px; height: 100%; min-height: 120px; }
  .fader.horizontal { height: 22px; width: 100%; min-width: 120px; }
  .rail { position: absolute; background: #4d4d4d; border-radius: 2px; }
  .vertical .rail { left: 9px; width: 4px; top: 7px; bottom: 7px; }
  .horizontal .rail { top: 9px; height: 4px; left: 7px; right: 7px; }
  .fill { position: absolute; background: var(--accent); border-radius: 2px; }
  .vertical .fill { left: 0; right: 0; bottom: 0; height: var(--level); }
  .horizontal .fill { top: 0; bottom: 0; left: 0; width: var(--level); }
  .knob { position: absolute; width: 14px; height: 14px; border-radius: 50%; background: var(--text);
    border: 2px solid var(--accent); }
  .vertical .knob { left: 4px; bottom: calc(var(--level) * (100% - 14px) / 100%); }
  .horizontal .knob { top: 4px; left: calc(var(--level) * (100% - 14px) / 100%); }
  .tip { position: absolute; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius);
    padding: 1px 6px; font-size: 10px; white-space: nowrap; pointer-events: none; z-index: 5; }
  .vertical .tip { left: 26px; top: 50%; transform: translateY(-50%); }
  .horizontal .tip { bottom: 24px; left: 50%; transform: translateX(-50%); }
  .fader:focus-visible { outline: 1px solid var(--accent); border-radius: var(--radius); }
</style>
