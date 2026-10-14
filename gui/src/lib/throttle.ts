/** Calls `fn` at most once per `ms`, always with the latest value; `flush` sends a pending value now. */
export function throttle<T>(fn: (v: T) => void, ms: number): { push(v: T): void; flush(): void; cancel(): void } {
  let last = -Infinity;
  let pending: { v: T } | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const fire = () => {
    timer = null;
    if (!pending) return;
    last = Date.now();
    const { v } = pending;
    pending = null;
    fn(v);
  };
  return {
    push(v: T) {
      pending = { v };
      const wait = ms - (Date.now() - last);
      if (wait <= 0) {
        if (timer) clearTimeout(timer);
        fire();
      } else if (!timer) {
        timer = setTimeout(fire, wait);
      }
    },
    flush() {
      if (timer) clearTimeout(timer);
      fire();
    },
    cancel() {
      if (timer) clearTimeout(timer);
      timer = null;
      pending = null;
    },
  };
}
