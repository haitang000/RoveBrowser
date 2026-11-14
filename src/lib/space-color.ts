// SpaceColor.paint and OKLab, from Zigzag/Design/SpaceColor.swift.
// Copyright 2026 Zigao Wang, Apache-2.0. Modified for Rove: sRGB hex output.

const decode = (x: number) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4);
const encode = (x: number) => (x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055);

export function clamp01(n: number): number {
  if (!Number.isFinite(n)) return 0;
  return Math.min(1, Math.max(0, n));
}

/** Wheel hue (red at 0) to the OKLCH angle SpaceColor.okHue uses. */
export function okHue(hue: number): number {
  const h = ((hue % 1) + 1) % 1;
  const c = h * 6;
  const x = 1 - Math.abs((c % 2) - 1);
  const [r, g, b] = (c < 1 ? [1, x, 0] : c < 2 ? [x, 1, 0] : c < 3 ? [0, 1, x] : c < 4 ? [0, x, 1] : c < 5 ? [x, 0, 1] : [1, 0, x]).map(decode);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return Math.atan2(
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
  );
}

export function hueDegrees(hue: number): number {
  return ((okHue(hue) * 180) / Math.PI + 360) % 360;
}

function oklabRgb(L: number, a: number, b: number): [number, number, number] {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

function inGamut(rgb: number[]): boolean {
  return rgb.every((v) => v >= -0.0005 && v <= 1.0005);
}

function hex(rgb: number[]): string {
  return (
    "#" +
    rgb
      .map((v) =>
        Math.round(encode(Math.min(1, Math.max(0, v))) * 255)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}

/**
 * A space color: hue around the wheel, depth from white (0) to black (1),
 * tint from none to all of the hue. Mixed in OKLCH, matching SpaceColor.paint.
 */
export function paint(hue: number, depth: number, tint = 1): string {
  const d = clamp01(depth);
  const lightness =
    d < 0.04 ? 1 - ((1 - 0.9474) * d) / 0.04 : d > 0.9 ? (0.354 * (1 - d)) / 0.1 : 0.975 - 0.69 * d;
  const rise = Math.min(d / 0.5, 1);
  const most = 0.02 + 0.13 * rise * rise * (3 - 2 * rise) - (0.03 * Math.max(0, d - 0.65)) / 0.35;
  let chroma = most * clamp01(tint) * Math.min(1, lightness / 0.12) * Math.min(1, (1 - lightness) / 0.04);
  const angle = okHue(hue);
  let rgb = oklabRgb(lightness, chroma * Math.cos(angle), chroma * Math.sin(angle));
  while (!inGamut(rgb) && chroma > 0.002) {
    chroma *= 0.94;
    rgb = oklabRgb(lightness, chroma * Math.cos(angle), chroma * Math.sin(angle));
  }
  return hex(rgb);
}

export function luminance(color: string): number {
  const n = Number.parseInt(color.slice(1), 16);
  const lin = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const r = lin(((n >> 16) & 255) / 255);
  const g = lin(((n >> 8) & 255) / 255);
  const b = lin((n & 255) / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Light or dark, whichever text reads better on the hardest of these colors. */
export function readableTheme(colors: string[]): "light" | "dark" {
  const lums = colors.map(luminance);
  const black = Math.min(...lums.map((l) => (l + 0.05) / 0.05));
  const white = Math.min(...lums.map((l) => 1.05 / (l + 0.05)));
  return black >= white ? "light" : "dark";
}
