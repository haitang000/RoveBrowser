import { expect, it } from "vitest";
import { collapsedPeekTransform, PEEK_MOTION } from "../src/lib/peek-motion";
const card = { x: 320, y: 120, width: 800, height: 600 },
  viewport = { x: 240, y: 32, width: 1000, height: 780 };
it("lands the scaled card center on the hyperlink and uses the same path for exit", () => {
  const x = 450,
    y = 250,
    s = PEEK_MOTION.scale;
  expect(collapsedPeekTransform(card, { x, y }, viewport)).toBe(
    `translate(${x - card.x - (card.width * s) / 2}px, ${y - card.y - (card.height * s) / 2}px) scale(${s})`,
  );
});
it("clamps stale click points after resizing and safely handles absent geometry", () => {
  expect(collapsedPeekTransform(card, { x: -10, y: 9000 }, viewport)).toBe(
    "translate(-152px, 638px) scale(0.18)",
  );
  expect(collapsedPeekTransform(card, null, viewport)).toContain("scale(0.96)");
  expect(
    collapsedPeekTransform(card, { x: NaN, y: 2 }, viewport),
  ).not.toContain("NaN");
});
