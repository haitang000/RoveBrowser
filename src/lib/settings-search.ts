const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });
const normalize = (text: string) => text.normalize("NFKD").toLocaleLowerCase();

export function settingsSearchWords(query: string): string[] {
  return [...new Set(normalize(query).trim().split(/\s+/).filter(Boolean))];
}

export function matchesSettingsSearch(words: readonly string[], ...text: string[]) {
  const joined = normalize(text.join(" "));
  return words.every((word) => joined.includes(word));
}

export interface SearchPart {
  text: string;
  match: boolean;
}

export function settingsSearchParts(text: string, words: readonly string[]): SearchPart[] {
  if (!words.length) return [{ text, match: false }];

  // Normalization can expand a character (e.g. ﬁ → fi). Map matches back to
  // whole original graphemes so accents and emoji are never split or rewritten.
  const normalized = normalize(text);
  const positions: { start: number; end: number }[] = [];
  for (const { segment, index } of graphemes.segment(text)) {
    const folded = normalize(segment);
    for (let i = 0; i < folded.length; i++)
      positions.push({ start: index, end: index + segment.length });
  }

  const ranges: { start: number; end: number }[] = [];
  for (const word of words) {
    if (!word) continue;
    let at = normalized.indexOf(word);
    while (at !== -1) {
      ranges.push({ start: positions[at]!.start, end: positions[at + word.length - 1]!.end });
      at = normalized.indexOf(word, at + 1);
    }
  }
  ranges.sort((a, b) => a.start - b.start || a.end - b.end);
  const merged: typeof ranges = [];
  for (const range of ranges) {
    const previous = merged.at(-1);
    if (previous && range.start <= previous.end)
      previous.end = Math.max(previous.end, range.end);
    else merged.push({ ...range });
  }

  const parts: SearchPart[] = [];
  let offset = 0;
  for (const { start, end } of merged) {
    if (start > offset) parts.push({ text: text.slice(offset, start), match: false });
    parts.push({ text: text.slice(start, end), match: true });
    offset = end;
  }
  if (offset < text.length) parts.push({ text: text.slice(offset), match: false });
  return parts;
}
