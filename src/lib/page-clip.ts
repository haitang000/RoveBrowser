import type { Rect } from "./types";

/** Match the native child to the page card's inner curve. Split headers cover
 * the top corners, while the seam between panes must remain square. */
export function pageClip(
  slot: DOMRect,
  card: DOMRect,
  style: CSSStyleDeclaration,
): Rect {
  const borders = [
    style.borderTopWidth,
    style.borderRightWidth,
    style.borderBottomWidth,
    style.borderLeftWidth,
  ].map((n) => parseFloat(n) || 0);
  const radii = [
    style.borderTopLeftRadius,
    style.borderTopRightRadius,
    style.borderBottomRightRadius,
    style.borderBottomLeftRadius,
  ].map((n) => parseFloat(n) || 0);
  const [top, right, bottom, left] = borders;
  const close = (a: number, b: number) => Math.abs(a - b) < 0.5;
  const atLeft = close(slot.left, card.left + left),
    atRight = close(slot.right, card.right - right);
  const atTop = close(slot.top, card.top + top),
    atBottom = close(slot.bottom, card.bottom - bottom);
  const joins = [
    atTop && atLeft,
    atTop && atRight,
    atBottom && atRight,
    atBottom && atLeft,
  ];
  const insets = [
    Math.max(top, left),
    Math.max(top, right),
    Math.max(bottom, right),
    Math.max(bottom, left),
  ];
  return {
    x: slot.x,
    y: slot.y,
    width: slot.width,
    height: slot.height,
    corners: radii.map((r, i) =>
      joins[i] ? Math.max(0, r - insets[i]) : 0,
    ) as [number, number, number, number],
  };
}
