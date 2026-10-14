export const IGNORE_AFTER_RELEASE_MS = 300;

export function clamp(v: number): number {
  return Math.max(0, Math.min(100, Math.round(v)));
}

export function valueFromPointer(clientY: number, rect: { top: number; height: number }): number {
  return clamp(((rect.top + rect.height - clientY) / rect.height) * 100);
}

export function valueFromPointerX(clientX: number, rect: { left: number; width: number }): number {
  return clamp(((clientX - rect.left) / rect.width) * 100);
}

export function wheelStep(current: number, deltaY: number): number {
  return clamp(current + (deltaY < 0 ? 5 : -5));
}

/** Local fader value vs. values echoed by the daemon. */
export class FaderModel {
  value: number;
  private dragging = false;
  private ignoreUntil = 0;

  constructor(value: number) {
    this.value = value;
  }

  get isDragging(): boolean {
    return this.dragging;
  }

  /** Applies a value from the daemon unless the user is (or just was) dragging. */
  external(v: number, now: number): boolean {
    if (this.dragging || now < this.ignoreUntil) return false;
    this.value = v;
    return true;
  }

  begin(): void {
    this.dragging = true;
  }

  drag(v: number): number {
    this.value = clamp(v);
    return this.value;
  }

  end(now: number): number {
    this.dragging = false;
    this.ignoreUntil = now + IGNORE_AFTER_RELEASE_MS;
    return this.value;
  }

  /** A discrete local change (wheel, double click). */
  set(v: number, now: number): number {
    this.value = clamp(v);
    this.ignoreUntil = now + IGNORE_AFTER_RELEASE_MS;
    return this.value;
  }
}
