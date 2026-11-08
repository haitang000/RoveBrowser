import { describe, it, expect } from "vitest";
import { pageClip } from "../src/lib/page-clip";
const rect = (x: number, y: number, width: number, height: number) =>
  ({
    x,
    y,
    width,
    height,
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
  }) as DOMRect;
const style = {
  borderTopWidth: "1px",
  borderRightWidth: "1px",
  borderBottomWidth: "1px",
  borderLeftWidth: "1px",
  borderTopLeftRadius: "8px",
  borderTopRightRadius: "8px",
  borderBottomRightRadius: "8px",
  borderBottomLeftRadius: "8px",
} as CSSStyleDeclaration;
describe("native page card geometry", () => {
  it("uses the inner radius after the card border", () => {
    expect(
      pageClip(rect(241, 33, 998, 778), rect(240, 32, 1000, 780), style)
        .corners,
    ).toEqual([7, 7, 7, 7]);
  });
  it("keeps split headers and the center seam square", () => {
    const card = rect(240, 32, 1000, 780);
    expect(pageClip(rect(241, 63, 495, 748), card, style).corners).toEqual([
      0, 0, 0, 7,
    ]);
    expect(pageClip(rect(744, 63, 495, 748), card, style).corners).toEqual([
      0, 0, 7, 0,
    ]);
  });
});
