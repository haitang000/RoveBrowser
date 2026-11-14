// Space icons follow Zigzag's icon picker, using emoji throughout: a short
// grid, or any emoji found by Unicode's name. Copyright 2026 Zigao Wang,
// Apache-2.0. Names come from Unicode emoji-test.
import rawEmoji from "./emoji-index.json" with { type: "json" };

export interface SpaceEmoji {
  emoji: string;
  label: string;
}
export interface EmojiEntry {
  emoji: string;
  name: string;
  words: string[];
  keywords: string[];
}
export type FieldState =
  | { type: "empty" }
  | { type: "emoji"; emoji: string }
  | { type: "search"; text: string };

/** Enough to tell work, study, money and the rest apart at a glance. */
export const SPACE_EMOJIS: SpaceEmoji[] = [
  { emoji: "⭐️", label: "星" },
  { emoji: "❤️", label: "心" },
  { emoji: "⚡️", label: "闪电" },
  { emoji: "🔥", label: "火焰" },
  { emoji: "🍃", label: "叶子" },
  { emoji: "🌙", label: "月亮" },
  { emoji: "☀️", label: "太阳" },
  { emoji: "✨", label: "闪光" },
  { emoji: "🏠", label: "家" },
  { emoji: "💼", label: "公文包" },
  { emoji: "🏢", label: "建筑" },
  { emoji: "🎓", label: "学业" },
  { emoji: "📖", label: "书" },
  { emoji: "🔖", label: "书签" },
  { emoji: "📰", label: "报纸" },
  { emoji: "💡", label: "点子" },
  { emoji: "💻", label: "代码" },
  { emoji: "⌨️", label: "终端" },
  { emoji: "🔨", label: "工具" },
  { emoji: "🔧", label: "维修" },
  { emoji: "🖌️", label: "画笔" },
  { emoji: "🎨", label: "调色" },
  { emoji: "📷", label: "相机" },
  { emoji: "🎬", label: "影片" },
  { emoji: "🎵", label: "音乐" },
  { emoji: "🎮", label: "游戏" },
  { emoji: "🏃", label: "运动" },
  { emoji: "✈️", label: "飞机" },
  { emoji: "🚗", label: "汽车" },
  { emoji: "🗺️", label: "地图" },
  { emoji: "🌍", label: "地球" },
  { emoji: "☁️", label: "云" },
  { emoji: "🛒", label: "购物" },
  { emoji: "👜", label: "袋子" },
  { emoji: "💳", label: "银行卡" },
  { emoji: "📊", label: "图表" },
  { emoji: "✉️", label: "邮件" },
  { emoji: "💬", label: "聊天" },
  { emoji: "👥", label: "朋友" },
  { emoji: "📅", label: "日历" },
  { emoji: "✅", label: "完成" },
  { emoji: "🧠", label: "思考" },
  { emoji: "☕", label: "咖啡" },
  { emoji: "🍴", label: "餐饮" },
  { emoji: "🐾", label: "宠物" },
  { emoji: "🚩", label: "旗帜" },
  { emoji: "🔒", label: "锁定" },
];

/** Everyday words whose Unicode names don't say them. From Zigzag's EmojiSearch. */
const KEYWORDS: Record<string, string[]> = {
  "😀": ["happy", "smile"],
  "😃": ["happy", "smile"],
  "😄": ["happy", "smile"],
  "😊": ["happy", "smile", "blush"],
  "🙂": ["smile"],
  "😂": ["lol", "laugh", "funny"],
  "🤣": ["lol", "laugh", "rofl"],
  "😍": ["love", "crush"],
  "🥰": ["love"],
  "😘": ["kiss", "love"],
  "😢": ["sad", "tear"],
  "😭": ["sad", "sob"],
  "😡": ["angry", "mad"],
  "😠": ["angry", "mad"],
  "🤔": ["think", "hmm"],
  "😎": ["cool"],
  "😴": ["sleep", "tired"],
  "🤯": ["mind blown"],
  "❤️": ["love", "red heart"],
  "💕": ["love"],
  "💖": ["love"],
  "👍": ["yes", "like", "ok", "good"],
  "👎": ["no", "dislike", "bad"],
  "👌": ["ok", "okay", "perfect"],
  "👋": ["hi", "hello", "bye", "wave"],
  "🙏": ["please", "thanks", "pray"],
  "👏": ["applause", "bravo"],
  "💪": ["strong", "gym", "muscle"],
  "🔥": ["hot", "lit", "fire"],
  "✨": ["magic", "shiny"],
  "🎉": ["party", "celebrate"],
  "🎊": ["party", "celebrate"],
  "💯": ["hundred", "perfect"],
  "✅": ["done", "check", "yes"],
  "❌": ["no", "wrong"],
  "⭐": ["favorite"],
  "💰": ["money", "cash"],
  "💵": ["money", "cash", "dollar"],
  "💸": ["money", "spend"],
  "📈": ["growth", "stocks"],
  "💼": ["work", "job", "office"],
  "🏠": ["home"],
  "🏡": ["home"],
  "📚": ["study", "read", "school"],
  "🎓": ["school", "study", "university"],
  "💻": ["computer", "code", "work"],
  "⌨️": ["code", "type"],
  "🐛": ["bug"],
  "🚀": ["launch", "ship", "fast"],
  "🎮": ["game", "gaming"],
  "🎵": ["music", "song"],
  "🎶": ["music", "song"],
  "🍕": ["food"],
  "🍔": ["food", "burger"],
  "☕": ["coffee", "cafe"],
  "✈️": ["travel", "flight", "plane"],
  "🌍": ["world", "globe", "earth", "travel"],
  "🛒": ["shopping", "cart"],
  "📝": ["note", "write", "todo"],
  "📅": ["calendar", "date", "schedule"],
  "🔒": ["lock", "private", "secure"],
  "💡": ["idea"],
  "⚽": ["football", "sport"],
  "🏃": ["run", "sport"],
  "🐶": ["dog", "puppy"],
  "🐱": ["cat", "kitty"],
  "🌸": ["flower", "spring"],
  "🌙": ["night", "moon"],
  "☀️": ["sun", "weather"],
  "🎂": ["birthday", "cake"],
};

const EMOJI = (rawEmoji as [string, string][]).map(([emoji, name]): EmojiEntry => ({
  emoji,
  name,
  words: name.split(/[\s-]+/).filter(Boolean),
  keywords: KEYWORDS[emoji] ?? [],
}));

const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** Lowercased, trimmed, one space between words. Hyphens count as spaces. */
export function normalized(text: string): string {
  return text
    .toLowerCase()
    .split(/[ \-\n\r]+/)
    .filter(Boolean)
    .join(" ");
}

function rank(entry: EmojiEntry, query: string, words: string[]): number | undefined {
  if (entry.name === query) return 0;
  if (entry.name.startsWith(query)) return 1;
  if (words.every((word) => entry.words.some((part) => part.startsWith(word)))) return 2;
  if (query.length >= 4 && entry.name.includes(query)) return 3;
  if (entry.keywords.some((word) => word === query || word.startsWith(query))) return 4;
  return undefined;
}

/** Matches, best first. Shorter names win ties, then Unicode order. */
export function searchEmoji(text: string, entries: EmojiEntry[] = EMOJI): EmojiEntry[] {
  const query = normalized(text);
  if (!query) return [];
  const words = query.split(" ");
  return entries
    .map((entry, offset) => {
      const score = rank(entry, query, words);
      return score === undefined ? undefined : { score, offset, entry };
    })
    .filter((item): item is { score: number; offset: number; entry: EmojiEntry } => item !== undefined)
    .sort((a, b) => a.score - b.score || a.entry.name.length - b.entry.name.length || a.offset - b.offset)
    .map((item) => item.entry);
}

export function emojiIndex(): EmojiEntry[] {
  return EMOJI;
}

function isEmojiPresentation(char: string): boolean {
  return /^\p{Emoji_Presentation}$/u.test(char);
}
function isEmojiScalar(char: string): boolean {
  return /^\p{Emoji}$/u.test(char);
}

/** The last emoji grapheme: a presentation emoji, or one asked to show (VS16 or a keycap). */
export function lastEmoji(text: string): string | undefined {
  const parts = [...segmenter.segment(text)].map((part) => part.segment);
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    const cluster = parts[index]!;
    const chars = [...cluster];
    if (chars.some(isEmojiPresentation)) return cluster;
    if (chars.some(isEmojiScalar) && chars.some((char) => char === "\uFE0F" || char === "\u20E3")) return cluster;
  }
  return undefined;
}

/** Nothing, an emoji to use as the icon, or a name to search. */
export function fieldState(text: string, shown?: string): FieldState {
  if (!text) return { type: "empty" };
  const typed = shown ? text.replaceAll(shown, "") : text;
  const emoji = lastEmoji(typed);
  if (emoji) return { type: "emoji", emoji };
  if (!typed && shown) return { type: "emoji", emoji: shown };
  return { type: "search", text: typed };
}

/** Clicking the chosen emoji again clears it. */
export function toggled(emoji: string, selected?: string): string | undefined {
  return emoji === selected ? undefined : emoji;
}

/** Deleting the emoji in the field clears the icon. */
export function clearsEmoji(text: string, selected?: string): boolean {
  return text === "" && cleanIcon(selected) !== undefined;
}

/** A stored icon is one emoji grapheme. */
export function cleanIcon(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const text = value.trim();
  if (!text || [...text].length > 48) return undefined;
  if ([...text].some((char) => {
    const code = char.codePointAt(0)!;
    return code < 32 || code === 127;
  })) return undefined;
  if (lastEmoji(text) === text) return text;
  return undefined;
}
