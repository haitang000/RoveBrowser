/** Zigzag PeekController.panelFrame: snap to the 4pt grid, 8% horizontal
 * inset, 4.8% vertical inset; minimum 32/24. Modified for CSS logical pixels. */
export function peekInsets(width: number, height: number) {
  const snap = (n: number) => Math.round(n / 4) * 4;
  return {
    x: Math.max(32, snap(width * 0.08)),
    y: Math.max(24, snap(height * 0.08 * 0.6)),
  };
}
