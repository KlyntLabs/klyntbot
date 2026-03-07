import { classifyChar } from "./charClass";

export interface TextRange {
  from: number;
  to: number;
}

/**
 * Find the range of a vim text object in flat text.
 *
 * @param text  - The full document text (flattened ProseMirror content)
 * @param pos   - Cursor position (0-based index into `text`)
 * @param scope - "i" for inner (content only) or "a" for around (includes delimiters/whitespace)
 * @param type  - The text object type: w, W, s, p, (, ), b, [, ], {, }, B, <, >, ", ', `
 * @returns A TextRange {from, to} (from inclusive, to exclusive), or null if no match
 */
export function findTextObject(
  text: string,
  pos: number,
  scope: "i" | "a",
  type: string,
): TextRange | null {
  switch (type) {
    case "w":
      return findWordObject(text, pos, scope, false);
    case "W":
      return findWordObject(text, pos, scope, true);
    case "s":
      return findSentenceObject(text, pos, scope);
    case "p":
      return findParagraphObject(text, pos, scope);
    case "(":
    case ")":
    case "b":
      return findBracketObject(text, pos, scope, "(", ")");
    case "[":
    case "]":
      return findBracketObject(text, pos, scope, "[", "]");
    case "{":
    case "}":
    case "B":
      return findBracketObject(text, pos, scope, "{", "}");
    case "<":
    case ">":
      return findBracketObject(text, pos, scope, "<", ">");
    case '"':
      return findQuoteObject(text, pos, scope, '"');
    case "'":
      return findQuoteObject(text, pos, scope, "'");
    case "`":
      return findQuoteObject(text, pos, scope, "`");
    default:
      return null;
  }
}

// ---------------------------------------------------------------------------
// Word objects (w / W)
// ---------------------------------------------------------------------------

function findWordObject(
  text: string,
  pos: number,
  scope: "i" | "a",
  bigWord: boolean,
): TextRange | null {
  if (text.length === 0 || pos < 0 || pos >= text.length) return null;

  const classify = bigWord
    ? (i: number): "word" | "space" => (/\s/.test(text[i]) ? "space" : "word")
    : (i: number): string => classifyChar(text[i]);

  const curClass = classify(pos);

  // Find start of current word/class run
  let from = pos;
  while (from > 0 && classify(from - 1) === curClass) from--;

  // Find end of current word/class run
  let to = pos;
  while (to + 1 < text.length && classify(to + 1) === curClass) to++;
  to++; // make exclusive

  if (scope === "i") {
    return { from, to };
  }

  // "a" scope: include trailing whitespace, or if none, leading whitespace
  let aTo = to;
  while (aTo < text.length && /\s/.test(text[aTo])) aTo++;

  if (aTo > to) {
    return { from, to: aTo };
  }

  // No trailing whitespace — include leading whitespace
  let aFrom = from;
  while (aFrom > 0 && /\s/.test(text[aFrom - 1])) aFrom--;

  if (aFrom < from) {
    return { from: aFrom, to };
  }

  return { from, to };
}

// ---------------------------------------------------------------------------
// Sentence objects (s)
// ---------------------------------------------------------------------------

function isSentenceEnd(text: string, i: number): boolean {
  if (i < 0 || i >= text.length) return false;
  const ch = text[i];
  if (ch !== "." && ch !== "!" && ch !== "?") return false;
  // Followed by whitespace or end of text
  return i + 1 >= text.length || /\s/.test(text[i + 1]);
}

function findSentenceObject(text: string, pos: number, scope: "i" | "a"): TextRange | null {
  if (text.length === 0 || pos < 0 || pos >= text.length) return null;

  // Find start of current sentence:
  // Walk backward to find the previous sentence-ending punctuation + whitespace,
  // or start of text.
  let from = 0;
  for (let i = pos - 1; i >= 0; i--) {
    if (isSentenceEnd(text, i)) {
      // Sentence starts after the punctuation + whitespace
      from = i + 1;
      while (from < pos && /\s/.test(text[from])) from++;
      break;
    }
  }

  // Find end of current sentence:
  // Walk forward to find sentence-ending punctuation.
  let endPunct = -1;
  for (let i = pos; i < text.length; i++) {
    if (isSentenceEnd(text, i)) {
      endPunct = i;
      break;
    }
  }

  if (endPunct === -1) {
    // No sentence end found — extend to end of text
    endPunct = text.length - 1;
  }

  if (scope === "i") {
    // Inner sentence: from start to end of punctuation (inclusive), exclusive `to`
    return { from, to: endPunct + 1 };
  }

  // Around sentence: include trailing whitespace
  let to = endPunct + 1;
  while (to < text.length && /\s/.test(text[to])) to++;

  return { from, to };
}

// ---------------------------------------------------------------------------
// Paragraph objects (p)
// ---------------------------------------------------------------------------

function findParagraphObject(text: string, pos: number, scope: "i" | "a"): TextRange | null {
  if (text.length === 0 || pos < 0 || pos >= text.length) return null;

  // Split into lines, find which line the cursor is on
  const lines = text.split("\n");
  let charOffset = 0;
  let cursorLine = 0;
  for (let i = 0; i < lines.length; i++) {
    const lineEnd = charOffset + lines[i].length; // position of the \n (or end)
    if (pos <= lineEnd) {
      cursorLine = i;
      break;
    }
    charOffset = lineEnd + 1; // +1 for the \n
  }

  const isBlank = (lineIdx: number): boolean =>
    lineIdx >= 0 && lineIdx < lines.length && lines[lineIdx].trim() === "";

  // If cursor is on a blank line, the "paragraph" is the run of blank lines
  if (isBlank(cursorLine)) {
    let startLine = cursorLine;
    while (startLine > 0 && isBlank(startLine - 1)) startLine--;
    let endLine = cursorLine;
    while (endLine + 1 < lines.length && isBlank(endLine + 1)) endLine++;

    const from = lines.slice(0, startLine).join("\n").length + (startLine > 0 ? 1 : 0);
    const to = lines.slice(0, endLine + 1).join("\n").length;

    if (scope === "a") {
      // Include the next non-blank paragraph if any
      let eLine = endLine + 1;
      while (eLine < lines.length && !isBlank(eLine)) eLine++;
      const aTo = lines.slice(0, eLine).join("\n").length;
      return { from, to: aTo };
    }
    return { from, to };
  }

  // Cursor is on a non-blank line — find paragraph boundaries
  let startLine = cursorLine;
  while (startLine > 0 && !isBlank(startLine - 1)) startLine--;
  let endLine = cursorLine;
  while (endLine + 1 < lines.length && !isBlank(endLine + 1)) endLine++;

  const from = lines.slice(0, startLine).join("\n").length + (startLine > 0 ? 1 : 0);
  const to = lines.slice(0, endLine + 1).join("\n").length;

  if (scope === "i") {
    return { from, to };
  }

  // Around paragraph: include trailing blank lines
  let eLine = endLine + 1;
  while (eLine < lines.length && isBlank(eLine)) eLine++;
  // Include the trailing \n separators
  let aTo: number;
  if (eLine > endLine + 1) {
    aTo = lines.slice(0, eLine).join("\n").length;
  } else {
    aTo = to;
  }

  // If no trailing blanks, include leading blank lines
  if (aTo === to) {
    let sLine = startLine - 1;
    while (sLine >= 0 && isBlank(sLine)) sLine--;
    sLine++;
    if (sLine < startLine) {
      const aFrom = lines.slice(0, sLine).join("\n").length + (sLine > 0 ? 1 : 0);
      return { from: aFrom, to };
    }
  }

  return { from, to: aTo };
}

// ---------------------------------------------------------------------------
// Bracket/paren objects
// ---------------------------------------------------------------------------

function findBracketObject(
  text: string,
  pos: number,
  scope: "i" | "a",
  open: string,
  close: string,
): TextRange | null {
  if (text.length === 0 || pos < 0 || pos >= text.length) return null;

  // Find the opening bracket by scanning backward, respecting nesting
  let openPos = -1;
  let depth = 0;

  // If cursor is on the open bracket, use that position
  // If cursor is on the close bracket, find its matching open bracket
  let searchStart = pos;
  if (text[pos] === close && open !== close) {
    // Scan backward from before the close bracket to find its matching open
    searchStart = pos - 1;
    depth = 0;
  } else if (text[pos] === open) {
    // If on the opening bracket, this is our match
    openPos = pos;
  }

  if (openPos === -1) {
    // Scan backward to find matching open bracket
    for (let i = searchStart; i >= 0; i--) {
      if (text[i] === close) {
        depth++;
      } else if (text[i] === open) {
        if (depth === 0) {
          openPos = i;
          break;
        }
        depth--;
      }
    }
  }

  if (openPos === -1) return null;

  // Find the matching close bracket by scanning forward from after the open
  let closePos = -1;
  depth = 0;
  for (let i = openPos + 1; i < text.length; i++) {
    if (text[i] === open) {
      depth++;
    } else if (text[i] === close) {
      if (depth === 0) {
        closePos = i;
        break;
      }
      depth--;
    }
  }

  if (closePos === -1) return null;

  // Verify cursor is within this bracket pair
  if (pos < openPos || pos > closePos) return null;

  if (scope === "i") {
    return { from: openPos + 1, to: closePos };
  }
  return { from: openPos, to: closePos + 1 };
}

// ---------------------------------------------------------------------------
// Quote objects
// ---------------------------------------------------------------------------

function findQuoteObject(
  text: string,
  pos: number,
  scope: "i" | "a",
  quote: string,
): TextRange | null {
  if (text.length === 0 || pos < 0 || pos >= text.length) return null;

  // Collect positions of all unescaped quote characters
  const quotePositions: number[] = [];
  for (let i = 0; i < text.length; i++) {
    if (text[i] === quote) {
      // Check if escaped
      let backslashes = 0;
      let j = i - 1;
      while (j >= 0 && text[j] === "\\") {
        backslashes++;
        j--;
      }
      if (backslashes % 2 === 0) {
        quotePositions.push(i);
      }
    }
  }

  if (quotePositions.length < 2) return null;

  // Find the pair that surrounds the cursor
  // Quotes form pairs: (0,1), (2,3), (4,5), ...
  // But we also handle the case where cursor is ON a quote
  for (let i = 0; i < quotePositions.length - 1; i += 2) {
    const openIdx = quotePositions[i];
    const closeIdx = quotePositions[i + 1];

    if (pos >= openIdx && pos <= closeIdx) {
      if (scope === "i") {
        return { from: openIdx + 1, to: closeIdx };
      }
      return { from: openIdx, to: closeIdx + 1 };
    }
  }

  // If cursor is between quote pairs, try to find any surrounding pair
  // This handles cases where the cursor is after the last quote pair
  // Try a different strategy: find the nearest opening quote before cursor
  // and its matching close quote after cursor
  for (let i = 0; i < quotePositions.length; i++) {
    if (quotePositions[i] > pos) {
      // Found first quote after cursor — check if there's a quote before
      if (i > 0) {
        const openIdx = quotePositions[i - 1];
        const closeIdx = quotePositions[i];
        if (pos >= openIdx && pos <= closeIdx) {
          if (scope === "i") {
            return { from: openIdx + 1, to: closeIdx };
          }
          return { from: openIdx, to: closeIdx + 1 };
        }
      }
      break;
    }
  }

  return null;
}
