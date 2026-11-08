import type { Point } from "./types";

export const PEEK_MOTION = {
  enter: 240,
  exit: 180,
  reduced: 80,
  scale: 0.18,
  ease: "cubic-bezier(.23,1,.32,1)",
} as const;

/** Translate a scaled card so its center lands on the original hyperlink click.
 * The same transform is used for entry and exit; viewport changes clamp the origin. */
export function collapsedPeekTransform(
  card: { x: number; y: number; width: number; height: number },
  origin: Point | null,
  viewport: { x: number; y: number; width: number; height: number },
): string {
  const valid =
    origin && Number.isFinite(origin.x) && Number.isFinite(origin.y);
  const scale = valid ? PEEK_MOTION.scale : 0.96;
  const x = valid
    ? Math.max(viewport.x, Math.min(viewport.x + viewport.width, origin.x))
    : card.x + card.width / 2;
  const y = valid
    ? Math.max(viewport.y, Math.min(viewport.y + viewport.height, origin.y))
    : card.y + card.height / 2;
  return `translate(${x - card.x - (card.width * scale) / 2}px, ${y - card.y - (card.height * scale) / 2}px) scale(${scale})`;
}

/** Preserve the actual presentation value before cancelling/reversing a WAAPI effect. */
export function freezePeekAnimation(animation: Animation | undefined): void {
  if (!animation) return;
  try {
    animation.commitStyles();
  } catch {
    /* Detached/replaced cards need no presentation state. */
  }
  animation.cancel();
}
