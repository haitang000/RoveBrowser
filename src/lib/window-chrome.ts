import type { Rect } from "./types";

/** Subtract interactive controls from the native titlebar hit regions. */
export function titlebarDragRegions(header: Rect, controls: Rect[]): Rect[] {
  let regions = [header];
  for (const control of controls) {
    regions = regions.flatMap(region => {
      const left = Math.max(region.x, control.x);
      const top = Math.max(region.y, control.y);
      const right = Math.min(region.x + region.width, control.x + control.width);
      const bottom = Math.min(region.y + region.height, control.y + control.height);
      if (left >= right || top >= bottom) return [region];
      return [
        { x: region.x, y: region.y, width: region.width, height: top - region.y },
        { x: region.x, y: bottom, width: region.width, height: region.y + region.height - bottom },
        { x: region.x, y: top, width: left - region.x, height: bottom - top },
        { x: right, y: top, width: region.x + region.width - right, height: bottom - top },
      ].filter(rect => rect.width > 0 && rect.height > 0);
    });
  }
  return regions;
}
