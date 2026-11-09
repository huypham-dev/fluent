import type { Rect } from './barPosition';
import type { Point } from './selectionDetector';

/** The visible card, and the transparent room around it for the arrow and the shadow. */
export const POPUP_CARD = { width: 500, height: 340 };
export const POPUP_MARGIN = 18;
export const POPUP_ARROW = 10;
export const POPUP_WINDOW = {
  width: POPUP_CARD.width + POPUP_MARGIN * 2,
  height: POPUP_CARD.height + POPUP_MARGIN * 2 + POPUP_ARROW,
};

/** The pointer sits mid-line, so the arrow tip moves this far to clear the selected line. */
const LINE_GAP = 12;
const SAME_LINE = 10;
/** Keeps the arrow off the card's rounded corners. */
const ARROW_INSET = POPUP_MARGIN + 24;

export type PopupSide = 'below' | 'above';
export type PopupPlacement = Point & { side: PopupSide; arrowX: number };

/** Where the selection probably is, from the drag that made it. */
export function selectionAnchor(press: Point | null, release: Point): Point {
  if (press && Math.abs(release.y - press.y) <= SAME_LINE) {
    return { x: Math.round((press.x + release.x) / 2), y: release.y };
  }
  return release;
}

/** Places the popup window under the selection, or above it near the bottom of the work area. */
export function popupPlacement(anchor: Point, workArea: Rect): PopupPlacement {
  const bottom = workArea.y + workArea.height;
  const belowY = anchor.y + LINE_GAP - POPUP_MARGIN;
  const aboveY = anchor.y - LINE_GAP + POPUP_MARGIN - POPUP_WINDOW.height;
  const fitsBelow = belowY + POPUP_WINDOW.height <= bottom;
  const moreRoomAbove = anchor.y - workArea.y > bottom - anchor.y;
  const side: PopupSide = !fitsBelow && moreRoomAbove ? 'above' : 'below';
  const y = clamp(side === 'below' ? belowY : aboveY, workArea.y, bottom - POPUP_WINDOW.height);
  const x = clamp(anchor.x - POPUP_WINDOW.width / 2, workArea.x, workArea.x + workArea.width - POPUP_WINDOW.width);
  const arrowX = clamp(anchor.x - x, ARROW_INSET, POPUP_WINDOW.width - ARROW_INSET);
  return { x: Math.round(x), y: Math.round(y), side, arrowX: Math.round(arrowX) };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
