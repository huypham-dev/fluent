/** The word "Hii" as pixel art. `X` is a filled cell. */
export const HII_PATTERN = [
  'XX..XX.XX.XX',
  'XX..XX.XX.XX',
  'XX..XX......',
  'XX..XX.XX.XX',
  'XXXXXX.XX.XX',
  'XXXXXX.XX.XX',
  'XX..XX.XX.XX',
  'XX..XX.XX.XX',
  'XX..XX.XX.XX',
  'XX..XX.XX.XX',
];

/** Draws a pattern centered on a square canvas as a BGRA bitmap, with no blurred edges. */
export function renderPattern(
  pattern: string[],
  options: { canvas: number; scale: number; color: [number, number, number] },
): { bitmap: Buffer; size: number } {
  const { canvas, scale, color } = options;
  const [red, green, blue] = color;
  const size = canvas * scale;
  const bitmap = Buffer.alloc(size * size * 4);
  const left = Math.floor((canvas - (pattern[0]?.length ?? 0)) / 2);
  const top = Math.floor((canvas - pattern.length) / 2);

  pattern.forEach((row, rowIndex) => {
    [...row].forEach((cell, columnIndex) => {
      if (cell !== 'X') return;
      for (let y = 0; y < scale; y += 1) {
        for (let x = 0; x < scale; x += 1) {
          const pixelY = (top + rowIndex) * scale + y;
          const pixelX = (left + columnIndex) * scale + x;
          bitmap.set([blue, green, red, 255], (pixelY * size + pixelX) * 4);
        }
      }
    });
  });
  return { bitmap, size };
}
