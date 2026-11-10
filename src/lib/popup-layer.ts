// Geometry rules for the trusted floating layer.
//
// The popup WebView is a separate native window clipped with a Win32 region to
// the union of the visible surfaces (see native/src/clip.rs). Anything that
// paints outside its own layout box — a drop shadow — needs that region widened,
// or the clip slices the shadow into a square-cornered halo.
export interface LayerRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Surfaces whose shadow needs room around their layout box. */
const SHADOWED_SURFACES = ["tab-hover-preview", "error-toast", "find-bar"];

/** How far a surface's region extends beyond its box for a soft shadow. */
export const SHADOW_BLEED = 20;

export function shadowBleed(className: string): number {
  return SHADOWED_SURFACES.some((name) =>
    className.split(/\s+/).includes(name),
  )
    ? SHADOW_BLEED
    : 0;
}

/** The native clip region reported for one surface's layout box. */
export function surfaceRegion(bounds: LayerRect, bleed: number): LayerRect {
  return {
    x: Math.max(0, bounds.x - bleed),
    y: Math.max(0, bounds.y - bleed),
    width: bounds.width + bleed * 2,
    height: bounds.height + bleed * 2,
  };
}
