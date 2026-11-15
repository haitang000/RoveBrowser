import { describe, expect, it } from "vitest";
import { titlebarDragRegions } from "../src/lib/window-chrome";

describe("native titlebar dragging", () => {
  it("keeps the title and toolbar gaps draggable while excluding every control", () => {
    const header = { x: 0, y: 0, width: 1280, height: 38 };
    const controls = [
      { x: 246, y: 9, width: 84, height: 26 },
      { x: 450, y: 9, width: 480, height: 26 },
      { x: 1142, y: 0, width: 138, height: 38 },
    ];
    const regions = titlebarDragRegions(header, controls);
    const contains = (x: number, y: number) => regions.some(r => x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height);
    expect(contains(100, 20)).toBe(true);
    expect(contains(350, 20)).toBe(true);
    expect(contains(600, 5)).toBe(true);
    for (const control of controls) expect(contains(control.x + 1, control.y + 1)).toBe(false);
    const area = regions.reduce((sum, r) => sum + r.width * r.height, 0);
    expect(area).toBe(header.width * header.height - controls.reduce((sum, r) => sum + r.width * r.height, 0));
  });
  it("handles overlapping and clipped controls in a narrow titlebar", () => {
    const regions = titlebarDragRegions({ x: 0, y: 0, width: 480, height: 32 }, [
      { x: 64, y: 0, width: 220, height: 32 },
      { x: 260, y: 0, width: 100, height: 32 },
      { x: 342, y: -6, width: 138, height: 38 },
    ]);
    expect(regions).toEqual([{ x: 0, y: 0, width: 64, height: 32 }]);
  });
});
