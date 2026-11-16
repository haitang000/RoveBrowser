// Zigzag Web/Reader.swift preferences (Zigao Wang, Apache-2.0), adapted to Vue.
import type { Rect } from "./types";
export interface ReaderPreferences {
  size: number;
  face: "serif" | "sans";
  theme: "auto" | "light" | "sepia" | "dark";
  width: "narrow" | "medium" | "wide";
}
export interface ReaderState {
  id: string | null;
  waiting: string | null;
  available: string[];
  rect: Rect | null;
}
export function readerPreferences(value?: ReaderPreferences): ReaderPreferences {
  return value || { size: 20, face: "serif", theme: "auto", width: "medium" };
}
