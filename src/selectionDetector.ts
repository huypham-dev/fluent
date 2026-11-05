export type Point = { x: number; y: number };
export type MouseInfo = Point & { button: number; clicks: number };

const LEFT_BUTTON = 1;
const DEFAULT_DRAG_THRESHOLD = 10;

/**
 * Decides from mouse events alone whether the user probably selected text.
 * It never reads the selection, so a wrong guess costs only a brief icon bar.
 */
export function createSelectionDetector(options: {
  dragThreshold?: number;
  isOwnWindow?: (point: Point) => boolean;
} = {}) {
  const threshold = options.dragThreshold ?? DEFAULT_DRAG_THRESHOLD;
  let press: Point | null = null;

  return {
    mouseDown(event: MouseInfo): void {
      press = event.button === LEFT_BUTTON ? { x: event.x, y: event.y } : null;
    },

    mouseUp(event: MouseInfo): boolean {
      const start = press;
      press = null;
      if (!start || event.button !== LEFT_BUTTON) return false;
      if (options.isOwnWindow?.(event)) return false;
      const dragged = Math.hypot(event.x - start.x, event.y - start.y) > threshold;
      return dragged || event.clicks >= 2;
    },
  };
}
