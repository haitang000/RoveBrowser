import { describe, it, expect } from "vitest";
import { address } from "../src/lib/address";
import { initialState, reduce } from "../src/lib/preview";
describe("address boundary", () => {
  it("normalizes URL and search terms", () => {
    expect(address("example.com")).toBe("https://example.com/");
    expect(address("字体 design")).toBe(
      "https://www.google.com/search?q=%E5%AD%97%E4%BD%93%20design",
    );
  });
  it("rejects privileged schemes", () => {
    for (const u of ["file:///C:/Windows", "javascript:alert(1)", "era://ui"])
      expect(() => address(u)).toThrow();
  });
});
describe("preview state invariants", () => {
  it("restores a closed tab and changes its workspace", () => {
    let s = initialState();
    s = reduce(s, { type: "close", id: "wiki" });
    expect(s.tabs.some((t) => t.id === "wiki")).toBe(false);
    s = reduce(s, { type: "restore" });
    expect(s.activeTab).toBe("wiki");
    s = reduce(s, { type: "move", id: "wiki", spaceId: "work" });
    expect(s.activeSpace).toBe("work");
  });
  it("clears a split after closing its companion", () => {
    let s = reduce(initialState(), {
      type: "split",
      id: "github",
      other: "wiki",
    });
    s = reduce(s, { type: "close", id: "wiki" });
    expect(s.tabs.find((t) => t.id === "github")?.splitWith).toBeUndefined();
  });
});
