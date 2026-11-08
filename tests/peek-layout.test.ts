import { expect, it } from "vitest";
import { peekInsets } from "../src/lib/peek-layout";
it("keeps the source 4px grid and minimum space at narrow window sizes", () => {
  expect(peekInsets(1000, 800)).toEqual({ x: 80, y: 40 });
  expect(peekInsets(320, 360)).toEqual({ x: 32, y: 24 });
});
