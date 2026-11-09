import { describe, expect, it } from 'vitest';
import { POPUP_MARGIN, POPUP_WINDOW, popupPlacement, selectionAnchor } from '../src/popupPosition';

const workArea = { x: 0, y: 0, width: 1600, height: 1000 };

describe('selectionAnchor', () => {
  it('centers under a selection dragged along one line', () => {
    expect(selectionAnchor({ x: 200, y: 300 }, { x: 600, y: 304 })).toEqual({ x: 400, y: 304 });
  });

  it('uses the release point for a selection over several lines', () => {
    expect(selectionAnchor({ x: 200, y: 300 }, { x: 600, y: 380 })).toEqual({ x: 600, y: 380 });
  });

  it('uses the release point when the press is unknown', () => {
    expect(selectionAnchor(null, { x: 600, y: 380 })).toEqual({ x: 600, y: 380 });
  });
});

describe('popupPlacement', () => {
  it('opens below the selection with the arrow under the anchor', () => {
    const placement = popupPlacement({ x: 800, y: 300 }, workArea);
    expect(placement.side).toBe('below');
    // The arrow tip is the top of the visible content, just under the selected line.
    expect(placement.y + POPUP_MARGIN).toBe(312);
    expect(placement.x + placement.arrowX).toBe(800);
  });

  it('opens above the selection when there is no room below', () => {
    const placement = popupPlacement({ x: 800, y: 900 }, workArea);
    expect(placement.side).toBe('above');
    expect(placement.y + POPUP_WINDOW.height - POPUP_MARGIN).toBe(888);
  });

  it('stays inside the work area near an edge and keeps the arrow on the card', () => {
    const placement = popupPlacement({ x: 1590, y: 300 }, workArea);
    expect(placement.x + POPUP_WINDOW.width).toBeLessThanOrEqual(1600);
    expect(placement.arrowX).toBeLessThan(POPUP_WINDOW.width - 30);
  });

  it('respects a display with a non-zero origin', () => {
    const second = { x: -1200, y: 100, width: 1200, height: 900 };
    const placement = popupPlacement({ x: -1190, y: 120 }, second);
    expect(placement.x).toBeGreaterThanOrEqual(-1200);
    expect(placement.side).toBe('below');
  });
});
