import { REGISTRY } from "./registry";
import type { SlashLeaf, SlashNode, SlashPath } from "./types";

/**
 * Walk the registry tree for the deepest matching leaf node.
 * Returns `null` if no leaf is reached (branch without terminal arg).
 */
export function resolveLeaf(input: string): SlashLeaf | null {
  if (input == null || input.length === 0) return null;
  if (input[0] !== "/") return null;
  const stripped = input.slice(1).trim();
  if (stripped.length === 0) return null;

  const tokens = stripped.split(/\s+/);
  let node: SlashNode | undefined = REGISTRY[tokens[0]];
  if (!node) return null;
  for (let i = 1; i < tokens.length; i++) {
    if (node.kind === "leaf") break;
    const next: SlashNode | undefined = node.children[tokens[i]];
    if (!next) break;
    node = next;
  }
  return node.kind === "leaf" ? node : null;
}

/**
 * Classify a raw composer input as agent-routed, direct, or null.
 *
 * Rules (spec §9.1048):
 * 1. Reject if the first non-whitespace character is not `/` or input is `null`-ish.
 * 2. Take leading `/`-prefixed token as command head.
 * 3. Walk REGISTRY tree as deep as possible.
 * 4. If deepest match is a leaf, return its `path`. Otherwise null (branch w/o terminal arg).
 * 5. Tie-break: direct wins over agent if same-named alias ever exists.
 */
export function classify(input: string): SlashPath | null {
  const leaf = resolveLeaf(input);
  return leaf?.path ?? null;
}
