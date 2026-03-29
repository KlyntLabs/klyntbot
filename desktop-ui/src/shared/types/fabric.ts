// ── Fabric Graph Types ────────────────────────────────────────────────────
// Mirrors crates/desktop-shared/src/commands/fabric.rs (camelCase)

export interface FabricNote {
  id: string;
  title: string;
  notebookId: string | null;
  tags: string[];
  bodyPreview: string;
  treeSectionCount: number;
  entityCount: number;
}

export interface FabricLink {
  sourceId: string;
  targetId: string;
  linkType: string;
}

export interface FabricCommunity {
  id: string;
  name: string;
  color: string;
  stability: number;
  memberCount: number;
  memberNoteIds: string[];
}

export interface FabricGraphBase {
  notes: FabricNote[];
  links: FabricLink[];
  communities: FabricCommunity[];
  suggestedPreset: string | null;
  lastActivityTimestamp: string;
  livePulseActive: boolean;
}

export interface FabricEntity {
  id: string;
  name: string;
  entityType: string;
  mentionCount: number;
}

export interface FabricEntityEdge {
  entityId: string;
  noteId: string;
  weight: number;
}

export interface FabricTreeNode {
  id: string;
  parentId: string | null;
  nodeType: string;
  title: string | null;
  contentPreview: string;
  level: number;
}

export interface FabricMember {
  noteId: string;
  treeNodeId: string;
  membershipScore: number;
}

export interface FabricCommunityDetail {
  communityId: string;
  representativePaths: string[];
  topEntities: string[];
  stabilityHistory: number[];
  members: FabricMember[];
}

export interface FabricGraphEvent {
  type: string;
  nodeType: string;
  id: string;
  data?: unknown;
  animationHint: string;
  intensity: number;
}

export type FabricLayer = "communities" | "entities" | "tree" | "community_detail";
