import { describe, expect, it } from "vitest";
import {
  matchesSettingsSearch,
  settingsSearchParts,
  settingsSearchWords,
} from "../src/lib/settings-search";

const highlights = (text: string, query: string) =>
  settingsSearchParts(text, settingsSearchWords(query))
    .filter((part) => part.match)
    .map((part) => part.text);

describe("settings search highlighting", () => {
  it("uses the same normalization and whitespace-separated words as filtering", () => {
    const words = settingsSearchWords("  Ｃｔｒｌ\t标签\nCTRL  ");
    expect(words).toEqual(["ctrl", "标签"]);
    expect(matchesSettingsSearch(words, "新建标签页", "Ctrl + T")).toBe(true);
    expect(matchesSettingsSearch(words, "新建标签页")).toBe(false);
    expect(highlights("Ctrl + T 标签页", "ＣＴＲＬ 标签")).toEqual(["Ctrl", "标签"]);
    expect(matchesSettingsSearch(settingsSearchWords("ΟΣ"), "ος")).toBe(true);
    expect(highlights("ΟΣ", "ος")).toEqual(["ΟΣ"]);
  });

  it("highlights every occurrence in Chinese titles and descriptions", () => {
    expect(highlights("搜索引擎：使用此引擎搜索。", "搜索")).toEqual(["搜索", "搜索"]);
  });

  it("merges overlapping matches regardless of query order", () => {
    for (const query of ["自动 自动填充", "自动填充 自动", "aba bab"])
      expect(highlights(query.includes("自动") ? "自动填充表单" : "ababa", query))
        .toEqual([query.includes("自动") ? "自动填充" : "ababa"]);
  });

  it("maps normalized characters back to whole original graphemes", () => {
    expect(highlights("Café Cafe\u0301 Ｒｏｖｅ ﬁle 👩‍💻", "CAFE rove fi 👩"))
      .toEqual(["Café", "Cafe\u0301", "Ｒｏｖｅ", "ﬁ", "👩‍💻"]);
  });

  it("treats regex metacharacters and HTML as literal text", () => {
    const text = '<b>[.*+?]</b> & "quoted"';
    const parts = settingsSearchParts(text, settingsSearchWords("<b> [.*+?]"));
    expect(parts.filter((part) => part.match).map((part) => part.text))
      .toEqual(["<b>[.*+?]"]);
    expect(parts.map((part) => part.text).join("")).toBe(text);
  });

  it("leaves unmatched and blank searches unchanged", () => {
    for (const query of ["", " \t\n", "no-match"])
      expect(settingsSearchParts("搜索引擎", settingsSearchWords(query)))
        .toEqual([{ text: "搜索引擎", match: false }]);
    expect(settingsSearchParts("", settingsSearchWords("搜索"))).toEqual([]);
  });
});
