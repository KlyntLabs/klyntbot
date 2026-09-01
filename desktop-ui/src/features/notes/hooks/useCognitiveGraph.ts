import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useCallback, useMemo, useState } from "react";
import type { ForceLink, ForceNode } from "./useGraphElements";

// ── Backend response types (camelCase from Rust serde) ───────────────────

interface TopicNode {
  id: string;
  subject: string;
  domain: string;
  factCount: number;
  avgConvergence: number;
  maxConfidence: number;
  totalAccessCount: number;
  lastAccessed: string | null;
  communityId: string | null;
}

interface TopicEdge {
  sourceTopicId: string;
  targetTopicId: string;
  strength: number;
}

interface CognitiveCommunity {
  id: string;
  name: string;
  color: string;
  memberTopicIds: string[];
}

interface RuleNode {
  id: string;
  ruleText: string;
  domain: string;
  signalCount: number;
  confidence: number;
}

interface GraphStats {
  totalFacts: number;
  totalTopics: number;
  totalEdges: number;
  totalCommunities: number;
  avgConvergence: number;
}

interface CognitiveGraphData {
  topics: TopicNode[];
  edges: TopicEdge[];
  communities: CognitiveCommunity[];
  rules: RuleNode[];
  stats: GraphStats;
}

interface FactNode {
  id: string;
  predicate: string;
  object: string;
  confidence: number;
  convergenceScore: number;
  stability: number;
  accessCount: number;
  source: string;
  lastAccessed: string | null;
}

interface TopicDetail {
  topicId: string;
  facts: FactNode[];
}

// ── Transform functions ──────────────────────────────────────────────────

function topicToForceNode(topic: TopicNode): ForceNode {
  return {
    id: `topic:${topic.id}`,
    label: topic.subject,
    color: "",
    size: Math.min(18 + topic.factCount * 4, 60),
    linkCount: topic.factCount,
    tags: [topic.domain],
    bodyPreview: `${topic.domain} -- ${topic.factCount} facts, convergence ${(topic.avgConvergence * 100).toFixed(0)}%`,
    notebookId: null,
    clusterId: topic.communityId ? `community:${topic.communityId}` : `domain:${topic.domain}`,
    nodeType: "topic",
    expandable: true,
    expanded: false,
  };
}

function ruleToForceNode(rule: RuleNode): ForceNode {
  const label = rule.ruleText.length > 60 ? `${rule.ruleText.slice(0, 57)}...` : rule.ruleText;
  return {
    id: `rule:${rule.id}`,
    label,
    color: "",
    size: 12,
    linkCount: rule.signalCount,
    tags: [rule.domain],
    bodyPreview: rule.ruleText,
    notebookId: null,
    clusterId: `domain:${rule.domain}`,
    nodeType: "rule",
  };
}

function edgeToForceLink(edge: TopicEdge): ForceLink {
  return {
    source: `topic:${edge.sourceTopicId}`,
    target: `topic:${edge.targetTopicId}`,
    weight: edge.strength,
    color: "",
  };
}

function factToSatellite(fact: FactNode, parentTopicId: string): ForceNode {
  return {
    id: `fact:${fact.id}`,
    label: `${fact.predicate} ${fact.object}`,
    color: "",
    size: 6 + fact.confidence * 8,
    linkCount: fact.accessCount,
    tags: [fact.source],
    bodyPreview: `${fact.predicate} ${fact.object} (confidence ${(fact.confidence * 100).toFixed(0)}%)`,
    notebookId: null,
    clusterId: parentTopicId,
    nodeType: "fact",
  };
}

// ── Hook ─────────────────────────────────────────────────────────────────

export interface UseCognitiveGraphResult {
  nodes: ForceNode[];
  links: ForceLink[];
  communities: CognitiveCommunity[];
  stats: GraphStats | undefined;
  expandTopic: (topicId: string) => Promise<void>;
  loading: boolean;
}

const EMPTY_STATS: GraphStats = {
  totalFacts: 0,
  totalTopics: 0,
  totalEdges: 0,
  totalCommunities: 0,
  avgConvergence: 0,
};

export function useCognitiveGraph(enabled: boolean): UseCognitiveGraphResult {
  // Always fetch so the data is ready when the user toggles the layer.
  // The `enabled` flag gates the returned nodes, not the query.
  const { data, loading } = useQuery<CognitiveGraphData | undefined>("cognitive_graph_data");

  const [expandedTopics, setExpandedTopics] = useState<Map<string, TopicDetail>>(new Map());
  const [expanding, setExpanding] = useState(false);

  const expandTopic = useCallback(async (topicId: string) => {
    setExpanding(true);
    try {
      // Parse topic ID format "subject::domain" to extract subject and domain
      const sepIdx = topicId.lastIndexOf("::");
      if (sepIdx === -1) return;
      const subject = topicId.slice(0, sepIdx);
      const domain = topicId.slice(sepIdx + 2);
      const detail = await ipc<TopicDetail>("cognitive_graph_expand_topic", {
        subject,
        domain,
      });
      setExpandedTopics((prev) => {
        const next = new Map(prev);
        if (next.has(topicId)) {
          next.delete(topicId);
        } else {
          next.set(topicId, detail);
        }
        return next;
      });
    } finally {
      setExpanding(false);
    }
  }, []);

  const { nodes, links } = useMemo(() => {
    if (!enabled || !data) return { nodes: [] as ForceNode[], links: [] as ForceLink[] };

    const topicNodes = data.topics.map(topicToForceNode);

    // Mark expanded topics
    for (const node of topicNodes) {
      const rawId = node.id.replace("topic:", "");
      if (expandedTopics.has(rawId)) {
        node.expanded = true;
      }
    }

    const ruleNodes = data.rules.map(ruleToForceNode);
    const edgeLinks = data.edges.map(edgeToForceLink);

    // Satellite fact nodes + links from expanded topics
    const factNodes: ForceNode[] = [];
    const factLinks: ForceLink[] = [];
    for (const [topicId, detail] of expandedTopics) {
      for (const fact of detail.facts) {
        factNodes.push(factToSatellite(fact, topicId));
        factLinks.push({
          source: `fact:${fact.id}`,
          target: `topic:${topicId}`,
          weight: fact.confidence,
          color: "",
        });
      }
    }

    return {
      nodes: [...topicNodes, ...ruleNodes, ...factNodes],
      links: [...edgeLinks, ...factLinks],
    };
  }, [enabled, data, expandedTopics]);

  return {
    nodes,
    links,
    communities: data?.communities ?? [],
    stats: data?.stats ?? (enabled ? undefined : EMPTY_STATS),
    expandTopic,
    loading: loading || expanding,
  };
}
