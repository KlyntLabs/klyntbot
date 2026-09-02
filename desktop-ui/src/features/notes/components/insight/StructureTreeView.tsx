import type { TreePathRef } from "@shared/types/tree-path";
import { useState } from "react";

// ── Tree building ──────────────────────────────────────────────────────

interface TreeNode {
  nodeId: string;
  title: string;
  level: number;
  matched: boolean;
  similarity: number;
  children: TreeNode[];
}

/** Build a tree structure from flat TreePathRef[] paths. */
function buildTree(paths: TreePathRef[]): TreeNode[] {
  const roots: TreeNode[] = [];

  for (const ref of paths) {
    let siblings = roots;
    for (let i = 0; i < ref.path.length; i++) {
      const segment = ref.path[i];
      const isLeaf = i === ref.path.length - 1;

      let existing = siblings.find((n) => n.nodeId === segment.nodeId);
      if (!existing) {
        existing = {
          nodeId: segment.nodeId,
          title: segment.title,
          level: segment.level,
          matched: isLeaf,
          similarity: isLeaf ? ref.similarity : 0,
          children: [],
        };
        siblings.push(existing);
      } else if (isLeaf && ref.similarity > existing.similarity) {
        existing.matched = true;
        existing.similarity = ref.similarity;
      }

      siblings = existing.children;
    }
  }

  return roots;
}

// ── Tree node component ────────────────────────────────────────────────

interface TreeNodeRowProps {
  node: TreeNode;
  depth: number;
}

function TreeNodeRow({ node, depth }: TreeNodeRowProps) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;

  return (
    <div>
      <div
        className="flex items-center gap-1 py-0.5 text-ui-sm text-fg-secondary hover:text-fg
          transition-colors"
        style={{ paddingLeft: `${depth * 12}px` }}
      >
        {hasChildren ? (
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="shrink-0 w-3.5 text-[10px] text-fg-dim hover:text-fg-secondary
              cursor-pointer select-none"
          >
            {expanded ? "▼" : "▶"}
          </button>
        ) : (
          <span className="shrink-0 w-3.5" />
        )}

        <span className="shrink-0 text-[10px]">
          {node.matched ? (
            <span className="text-brand">●</span>
          ) : (
            <span className="text-fg-dim">○</span>
          )}
        </span>

        <span className="truncate flex-1">{node.title}</span>

        {node.matched && (
          <span className="shrink-0 text-[10px] text-brand tabular-nums">
            {Math.round(node.similarity * 100)}%
          </span>
        )}
      </div>

      {expanded &&
        hasChildren &&
        node.children.map((child) => (
          <TreeNodeRow key={child.nodeId} node={child} depth={depth + 1} />
        ))}
    </div>
  );
}

// ── Main component ─────────────────────────────────────────────────────

interface StructureTreeViewProps {
  treePaths: TreePathRef[];
}

export function StructureTreeView({ treePaths }: StructureTreeViewProps) {
  if (treePaths.length === 0) return null;

  const tree = buildTree(treePaths);

  return (
    <div className="rounded-lg border border-separator/50 bg-bg-elevated/50 p-2 mt-3">
      <p className="text-[10px] font-medium text-fg-secondary uppercase tracking-wider mb-1.5">
        Matched structure
      </p>
      {tree.map((node) => (
        <TreeNodeRow key={node.nodeId} node={node} depth={0} />
      ))}
    </div>
  );
}
