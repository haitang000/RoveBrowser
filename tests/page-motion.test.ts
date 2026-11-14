import { describe, expect, it } from "vitest";
import { pageDirection, paneDirection } from "../src/lib/page-motion";

const tabs = [{ id: "a" }, { id: "b" }, { id: "c" }];
const panes = ["general", "spaces", "privacy"];

describe("page direction", () => {
  it("slides toward a later or earlier tab", () => {
    expect(pageDirection(tabs, "a", "c")).toBe(1);
    expect(pageDirection(tabs, "c", "a")).toBe(-1);
  });

  it("fades in place for the same tab, a space change, or an unknown tab", () => {
    expect(pageDirection(tabs, "b", "b")).toBe(0);
    expect(pageDirection(tabs, "a", "b", true)).toBe(0);
    expect(pageDirection(tabs, "missing", "b")).toBe(0);
    expect(pageDirection(tabs, undefined, "a")).toBe(0);
  });

  it("slides settings panes along their column order", () => {
    expect(paneDirection(panes, "general", "privacy")).toBe(1);
    expect(paneDirection(panes, "privacy", "spaces")).toBe(-1);
    expect(paneDirection(panes, "general", "general")).toBe(0);
    expect(paneDirection(panes, undefined, "spaces")).toBe(0);
  });
});
