import { describe, expect, it } from 'vitest';
import { barPosition } from '../src/barPosition';

const bar = { width: 120, height: 40 };
const workArea = { x: 0, y: 0, width: 1000, height: 800 };

describe('barPosition', () => {
  it('centers the bar 12 px above the pointer', () => {
    expect(barPosition({ x: 500, y: 400 }, bar, workArea)).toEqual({ x: 440, y: 348 });
  });

  it('flips below the pointer when there is no room above', () => {
    expect(barPosition({ x: 500, y: 20 }, bar, workArea)).toEqual({ x: 440, y: 32 });
  });

  it.each([
    ['top left', { x: 0, y: 0 }],
    ['top right', { x: 1000, y: 0 }],
    ['bottom left', { x: 0, y: 800 }],
    ['bottom right', { x: 1000, y: 800 }],
  ])('stays inside the work area at the %s corner', (_name, pointer) => {
    const position = barPosition(pointer, bar, workArea);
    expect(position.x).toBeGreaterThanOrEqual(0);
    expect(position.y).toBeGreaterThanOrEqual(0);
    expect(position.x + bar.width).toBeLessThanOrEqual(1000);
    expect(position.y + bar.height).toBeLessThanOrEqual(800);
  });

  it('respects a display with a non-zero origin', () => {
    const second = { x: -1200, y: 100, width: 1200, height: 700 };
    expect(barPosition({ x: -1200, y: 100 }, bar, second)).toEqual({ x: -1200, y: 112 });
  });
});
