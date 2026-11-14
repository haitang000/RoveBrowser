import { describe, expect, it } from "vitest";
import {
  SPACE_EMOJIS,
  cleanIcon,
  clearsEmoji,
  emojiIndex,
  fieldState,
  lastEmoji,
  searchEmoji,
  toggled,
  type EmojiEntry,
} from "../src/lib/space-icon";

function entry(emoji: string, name: string, keywords: string[] = []): EmojiEntry {
  return { emoji, name, words: name.split(/[\s-]+/).filter(Boolean), keywords };
}

describe("space icons", () => {
  const sample = [
    entry("🐯", "tiger face"),
    entry("😺", "smiling cat face with open mouth"),
    entry("🐱", "cat face"),
    entry("🐈", "cat"),
    entry("🛒", "shopping trolley", ["cart"]),
    entry("🦆", "duck"),
  ];
  const found = (query: string) => searchEmoji(query, sample).map((item) => item.emoji);

  it("ranks a name, then a prefix, then a word, then a keyword", () => {
    expect(found("cat")).toEqual(["🐈", "🐱", "😺"]);
    expect(found("ca")).toEqual(["🐈", "🐱", "😺", "🛒"]);
    expect(found("  Face   CAT ")).toEqual(["🐱", "😺"]);
    expect(found("smil")).toEqual(["😺"]);
    expect(found("olley")).toEqual(["🛒"]);
    expect(found("uck")).toEqual([]);
    expect(found("zebra")).toEqual([]);
    expect(found("   ")).toEqual([]);
  });

  it("finds everyday emoji by Unicode name", () => {
    const index = emojiIndex();
    const first = (query: string) => searchEmoji(query, index)[0]?.emoji;
    expect(index.length).toBeGreaterThan(1000);
    expect(index.find((item) => item.emoji === "🚀")?.name).toBe("rocket");
    expect(index.find((item) => item.emoji === "🐈")?.name).toBe("cat");
    expect(index.find((item) => item.emoji === "😀")?.name).toBe("grinning face");
    expect(index.find((item) => item.emoji === "❤️")).toBeTruthy();
    expect(index.find((item) => item.emoji === "❤")).toBeUndefined();
    expect(first("rocket")).toBe("🚀");
    expect(first("cat")).toBe("🐈");
    expect(first("thumbs up")).toBe("👍");
    expect(first("pizza")).toBe("🍕");
    expect(searchEmoji("love", index).some((item) => item.emoji === "❤️")).toBe(true);
    expect(searchEmoji("a", index).length).toBeGreaterThan(0);
    expect(searchEmoji("qqzzx", index)).toEqual([]);
    expect(searchEmoji("cat", index).some((item) => item.emoji === "🪪")).toBe(false);
  });

  it("treats a typed emoji as the icon and letters as a search", () => {
    expect(fieldState("", undefined)).toEqual({ type: "empty" });
    expect(fieldState("", "🍕")).toEqual({ type: "empty" });
    expect(fieldState("a", undefined)).toEqual({ type: "search", text: "a" });
    expect(fieldState("red heart", undefined)).toEqual({ type: "search", text: "red heart" });
    expect(fieldState("1", undefined)).toEqual({ type: "search", text: "1" });
    expect(fieldState("🚀", undefined)).toEqual({ type: "emoji", emoji: "🚀" });
    expect(fieldState("I love 🍕", undefined)).toEqual({ type: "emoji", emoji: "🍕" });
    expect(fieldState("🍕🚀", "🍕")).toEqual({ type: "emoji", emoji: "🚀" });
    expect(fieldState("❤️", undefined)).toEqual({ type: "emoji", emoji: "❤️" });
    expect(fieldState("🍕d", "🍕")).toEqual({ type: "search", text: "d" });
    expect(fieldState("c🍕", "🍕")).toEqual({ type: "search", text: "c" });
    expect(fieldState("🍕", "🍕")).toEqual({ type: "emoji", emoji: "🍕" });
    expect(lastEmoji("1")).toBeUndefined();
    expect(lastEmoji("#")).toBeUndefined();
    expect(lastEmoji("1️⃣")).toBe("1️⃣");
  });

  it("stores one emoji, and clicking it again or emptying the field clears it", () => {
    expect(SPACE_EMOJIS.every((item) => cleanIcon(item.emoji) === item.emoji)).toBe(true);
    expect(toggled("⭐", "⭐")).toBeUndefined();
    expect(toggled("⭐", "❤️")).toBe("⭐");
    expect(clearsEmoji("", "🚀")).toBe(true);
    expect(clearsEmoji("", "star.fill")).toBe(false);
    expect(clearsEmoji("a", "🚀")).toBe(false);
    expect(cleanIcon("star.fill")).toBeUndefined();
    expect(cleanIcon(" 🚀 ")).toBe("🚀");
    expect(cleanIcon("rocket")).toBeUndefined();
    expect(cleanIcon("a\nb")).toBeUndefined();
  });
});
