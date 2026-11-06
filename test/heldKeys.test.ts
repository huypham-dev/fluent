import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createHeldKeys } from '../src/heldKeys';

const CTRL = 1;
const ALT = 2;
const OTHER = 3;

describe('createHeldKeys', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('resolves true at once when no tracked key is held', async () => {
    const keys = createHeldKeys([CTRL, ALT]);
    await expect(keys.whenReleased(1000)).resolves.toBe(true);
  });

  it('ignores keys it does not track', async () => {
    const keys = createHeldKeys([CTRL, ALT]);
    keys.keyDown(OTHER);
    await expect(keys.whenReleased(1000)).resolves.toBe(true);
  });

  it('waits until every tracked key is released', async () => {
    const keys = createHeldKeys([CTRL, ALT]);
    keys.keyDown(CTRL);
    keys.keyDown(ALT);
    const settled = vi.fn();
    void keys.whenReleased(1000).then(settled);

    keys.keyUp(CTRL);
    await vi.advanceTimersByTimeAsync(0);
    expect(settled).not.toHaveBeenCalled();

    keys.keyUp(ALT);
    await vi.advanceTimersByTimeAsync(0);
    expect(settled).toHaveBeenCalledWith(true);
  });

  it('resolves false when a key is still held at the timeout', async () => {
    const keys = createHeldKeys([CTRL]);
    keys.keyDown(CTRL);
    const released = keys.whenReleased(1000);
    await vi.advanceTimersByTimeAsync(1000);
    await expect(released).resolves.toBe(false);
  });

  it('forgets held keys after a timeout, since their release may have been missed', async () => {
    const keys = createHeldKeys([CTRL]);
    keys.keyDown(CTRL);
    const first = keys.whenReleased(1000);
    await vi.advanceTimersByTimeAsync(1000);
    await first;
    await expect(keys.whenReleased(1000)).resolves.toBe(true);
  });
});
