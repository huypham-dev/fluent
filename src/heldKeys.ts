/** Tracks which of the given keys are physically held, from global key events. */
export function createHeldKeys(tracked: readonly number[]) {
  const held = new Set<number>();
  let onAllReleased: (() => void)[] = [];

  return {
    keyDown(key: number): void {
      if (tracked.includes(key)) held.add(key);
    },

    keyUp(key: number): void {
      held.delete(key);
      if (held.size > 0) return;
      const callbacks = onAllReleased;
      onAllReleased = [];
      for (const callback of callbacks) callback();
    },

    /** Resolves false if a key is still held at the timeout. */
    whenReleased(timeoutMs: number): Promise<boolean> {
      if (held.size === 0) return Promise.resolve(true);
      return new Promise((resolve) => {
        const callback = () => {
          clearTimeout(timer);
          resolve(true);
        };
        const timer = setTimeout(() => {
          onAllReleased = onAllReleased.filter((other) => other !== callback);
          // A release can go unseen, for example during secure input, so don't keep waiting on it.
          held.clear();
          resolve(false);
        }, timeoutMs);
        onAllReleased.push(callback);
      });
    },
  };
}
