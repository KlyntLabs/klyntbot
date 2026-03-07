export enum CharClass {
  Word = "word",
  Space = "space",
  Punct = "punct",
}

export function classifyChar(ch: string): CharClass {
  if (/[\w]/.test(ch)) return CharClass.Word;
  if (/\s/.test(ch)) return CharClass.Space;
  return CharClass.Punct;
}

/**
 * Find a word boundary in flat text.
 * For `w` (forward, start): skip current word/punct class, skip whitespace, land on next word/punct start.
 * For `b` (backward, start): skip whitespace backward, skip prev word/punct backward, land on its start.
 * For `e` (forward, end): skip whitespace, skip word/punct, land on last char of word/punct.
 */
export function findWordBoundary(
  text: string,
  pos: number,
  direction: "forward" | "backward",
  boundary: "start" | "end",
): number {
  const len = text.length;
  if (len === 0) return pos;

  if (direction === "forward" && boundary === "start") {
    let i = pos;
    if (i >= len) return len;
    const startClass = classifyChar(text[i]);
    while (i < len && classifyChar(text[i]) === startClass) i++;
    while (i < len && classifyChar(text[i]) === CharClass.Space) i++;
    return i;
  }

  if (direction === "forward" && boundary === "end") {
    let i = pos + 1;
    if (i >= len) return Math.min(pos, len - 1);
    while (i < len && classifyChar(text[i]) === CharClass.Space) i++;
    if (i >= len) return len - 1;
    const cls = classifyChar(text[i]);
    while (i + 1 < len && classifyChar(text[i + 1]) === cls) i++;
    return i;
  }

  if (direction === "backward" && boundary === "start") {
    let i = pos - 1;
    if (i < 0) return 0;
    while (i > 0 && classifyChar(text[i]) === CharClass.Space) i--;
    if (i <= 0) return 0;
    const cls = classifyChar(text[i]);
    while (i > 0 && classifyChar(text[i - 1]) === cls) i--;
    return i;
  }

  return pos;
}

/**
 * WORD boundary (W/B/E) — only whitespace is a boundary.
 */
export function findWORDBoundary(
  text: string,
  pos: number,
  direction: "forward" | "backward",
  boundary: "start" | "end",
): number {
  const isSpace = (i: number) => /\s/.test(text[i]);

  if (direction === "forward" && boundary === "start") {
    let i = pos;
    if (i >= text.length) return text.length;
    while (i < text.length && !isSpace(i)) i++;
    while (i < text.length && isSpace(i)) i++;
    return i;
  }

  if (direction === "forward" && boundary === "end") {
    let i = pos + 1;
    if (i >= text.length) return Math.min(pos, text.length - 1);
    while (i < text.length && isSpace(i)) i++;
    while (i + 1 < text.length && !isSpace(i + 1)) i++;
    return i;
  }

  if (direction === "backward" && boundary === "start") {
    let i = pos - 1;
    if (i < 0) return 0;
    while (i > 0 && isSpace(i)) i--;
    while (i > 0 && !isSpace(i - 1)) i--;
    return i;
  }

  return pos;
}
