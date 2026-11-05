import type { Point } from './selectionDetector';

export type Rect = Point & { width: number; height: number };

const POINTER_GAP = 12;

/** Places the bar above the pointer, or below it near the top, inside the work area. */
export function barPosition(
  pointer: Point,
  bar: { width: number; height: number },
  workArea: Rect,
): Point {
  const above = pointer.y - POINTER_GAP - bar.height;
  const y = above >= workArea.y ? above : pointer.y + POINTER_GAP;
  return {
    x: clamp(pointer.x - bar.width / 2, workArea.x, workArea.x + workArea.width - bar.width),
    y: clamp(y, workArea.y, workArea.y + workArea.height - bar.height),
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
