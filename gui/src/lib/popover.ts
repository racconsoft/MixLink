import { writable } from "svelte/store";

/**
 * Id of the single open popover (a channel menu or an app picker), if any.
 * Ids look like `menu:<channelId>` or `apps:<channelId>`. Setting this to a new id
 * closes whichever popover was open before, since every popover's `open` state is
 * derived from `$openPopover === <its own id>`.
 */
export const openPopover = writable<string | null>(null);

export interface Anchor {
  /** The element the popover opens from. */
  trigger: HTMLElement | undefined;
  /** Which trigger edge the popover lines up with. */
  align: "left" | "right";
}

const GAP = 4;
const MARGIN = 8;

/**
 * Svelte action: positions a popover with `position: fixed` just below its trigger, so it is not
 * clipped by scrolling containers such as the channel strip row. Follows scrolling and resizing
 * and stays inside the viewport.
 */
export function anchored(node: HTMLElement, anchor: Anchor) {
  let current = anchor;
  const place = () => {
    const trigger = current.trigger;
    if (!trigger) return;
    const r = trigger.getBoundingClientRect();
    const vw = document.documentElement.clientWidth || window.innerWidth;
    const vh = document.documentElement.clientHeight || window.innerHeight;
    const width = node.offsetWidth;
    let left = current.align === "left" ? r.left : r.right - width;
    left = Math.max(MARGIN, Math.min(left, vw - MARGIN - width));
    const top = r.bottom + GAP;
    node.style.position = "fixed";
    node.style.top = `${top}px`;
    node.style.left = `${left}px`;
    node.style.right = "auto";
    // Room left below the trigger; popovers cap their height with `min(…, var(--popover-room))`.
    node.style.setProperty("--popover-room", `${Math.max(120, vh - top - MARGIN)}px`);
  };
  place();
  window.addEventListener("scroll", place, true);
  window.addEventListener("resize", place);
  return {
    update(next: Anchor) {
      current = next;
      place();
    },
    destroy() {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
    },
  };
}
