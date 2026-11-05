import { describe, expect, it } from 'vitest';
import { HII_PATTERN, renderPattern } from '../src/trayIcon';

const alphaAt = (bitmap: Buffer, size: number, x: number, y: number) => bitmap[(y * size + x) * 4 + 3];

describe('renderPattern', () => {
  it('produces four bytes per pixel at the requested scale', () => {
    const { bitmap, size } = renderPattern(HII_PATTERN, { canvas: 16, scale: 2, color: [0, 0, 0] });
    expect(size).toBe(32);
    expect(bitmap.length).toBe(32 * 32 * 4);
  });

  it('draws each pattern cell as a solid block', () => {
    const { bitmap, size } = renderPattern(['X'], { canvas: 1, scale: 2, color: [1, 2, 3] });
    for (const [x, y] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) {
      expect(alphaAt(bitmap, size, x, y)).toBe(255);
    }
    expect([...bitmap.subarray(0, 4)]).toEqual([3, 2, 1, 255]);
  });

  it('leaves empty cells transparent', () => {
    const { bitmap, size } = renderPattern(['X.'], { canvas: 2, scale: 1, color: [0, 0, 0] });
    expect(alphaAt(bitmap, size, 0, 0)).toBe(255);
    expect(alphaAt(bitmap, size, 1, 0)).toBe(0);
  });

  it('centers the pattern on the canvas', () => {
    const { bitmap, size } = renderPattern(['X'], { canvas: 3, scale: 1, color: [0, 0, 0] });
    expect(alphaAt(bitmap, size, 1, 1)).toBe(255);
    expect(alphaAt(bitmap, size, 0, 0)).toBe(0);
  });
});

describe('HII_PATTERN', () => {
  it('fits a 16-point menu bar icon', () => {
    expect(HII_PATTERN.length).toBeLessThanOrEqual(16);
    for (const row of HII_PATTERN) expect(row.length).toBeLessThanOrEqual(16);
  });

  it('has rows of equal width', () => {
    expect(new Set(HII_PATTERN.map((row) => row.length)).size).toBe(1);
  });
});
