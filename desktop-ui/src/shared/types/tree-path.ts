export interface TreePathRef {
  noteId: string;
  noteName: string;
  path: PathSegment[];
  nodeId: string;
  similarity: number;
}

export interface PathSegment {
  nodeId: string;
  title: string;
  level: number;
}

export interface CommunityCardData {
  communityId: string;
  name: string;
  sourceNoteCount: number;
  representativePaths: string[];
  stabilityTrend: number;
  topEntities: string[];
}
