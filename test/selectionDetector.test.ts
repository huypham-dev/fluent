import { describe, expect, it } from 'vitest';
import { createSelectionDetector } from '../src/selectionDetector';

const down = (x = 0, y = 0, button = 1, clicks = 1) => ({ x, y, button, clicks });

describe('createSelectionDetector', () => {
  it('shows the bar after a left drag of more than 10 px', () => {
    const detector = createSelectionDetector();
    detector.mouseDown(down(0, 0));
    expect(detector.mouseUp(down(11, 0))).toBe(true);
  });

  it('does not show the bar for a drag of exactly 10 px', () => {
    const detector = createSelectionDetector();
    detector.mouseDown(down(0, 0));
    expect(detector.mouseUp(down(10, 0))).toBe(false);
  });

  it('measures diagonal drags by distance', () => {
    const detector = createSelectionDetector();
    detector.mouseDown(down(0, 0));
    expect(detector.mouseUp(down(8, 8))).toBe(true);
  });

  it('shows the bar after a left double-click', () => {
    const detector = createSelectionDetector();
    detector.mouseDown(down(5, 5, 1, 2));
    expect(detector.mouseUp(down(5, 5, 1, 2))).toBe(true);
  });

  it('does not show the bar after a single click', () => {
    const detector = createSelectionDetector();
    detector.mouseDown(down(5, 5));
    expect(detector.mouseUp(down(5, 5))).toBe(false);
  });

  it('ignores a right-button drag', () => {
    const detector = createSelectionDetector();
    detector.mouseDown(down(0, 0, 2));
    expect(detector.mouseUp(down(100, 0, 2))).toBe(false);
  });

  it('ignores a mouse-up with no mouse-down', () => {
    const detector = createSelectionDetector();
    expect(detector.mouseUp(down(100, 0))).toBe(false);
  });

  it('does not reuse a finished press', () => {
    const detector = createSelectionDetector();
    detector.mouseDown(down(0, 0));
    detector.mouseUp(down(100, 0));
    expect(detector.mouseUp(down(200, 0))).toBe(false);
  });

  it('ignores a drag ending over an own window', () => {
    const detector = createSelectionDetector({ isOwnWindow: (p) => p.x > 50 });
    detector.mouseDown(down(0, 0));
    expect(detector.mouseUp(down(100, 0))).toBe(false);
  });
});
