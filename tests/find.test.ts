import { describe, expect, it } from "vitest";
import {
  FIND_QUERY_MAX,
  findMatchLabel,
  normalizeFindQuery,
} from "../src/lib/find";

describe("find bar formatting", () => {
  it("keeps the query on one line and within the host cap", () => {
    expect(normalizeFindQuery("a\nb\r\nc")).toBe("a b c");
    expect(normalizeFindQuery("x".repeat(FIND_QUERY_MAX + 20))).toHaveLength(
      FIND_QUERY_MAX,
    );
    expect(normalizeFindQuery("  中文  ")).toBe("  中文  ");
  });

  it("shows the place and total, hiding an empty query", () => {
    expect(findMatchLabel(0, 0, false)).toBe("");
    expect(findMatchLabel(0, 0, true)).toBe("无结果");
    // ActiveMatchIndex is 1-based: 2 is the second of twelve.
    expect(findMatchLabel(12, 2, true)).toBe("2 / 12");
    expect(findMatchLabel(1, 1, true)).toBe("1 / 1");
    expect(findMatchLabel(12, 0, true)).toBe("1 / 12");
    expect(findMatchLabel(3, 9, true)).toBe("3 / 3");
  });
});
