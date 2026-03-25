import { useQuery } from "@shared/hooks/useQuery";
import { Eye } from "lucide-react";
import type { MetaRule } from "./components/MetaRulesSection";
import { MetaRulesSection } from "./components/MetaRulesSection";
import { MirrorInput } from "./components/MirrorInput";
import type { TrendNarrative } from "./components/NarrativeCard";
import { NarrativeCard } from "./components/NarrativeCard";
import type { RoutingSnapshot } from "./components/RoutingDonut";
import { RoutingDonut } from "./components/RoutingDonut";
import type { NarrativeSnippet } from "./components/SnippetFeed";
import { SnippetFeed } from "./components/SnippetFeed";

interface MirrorState {
  lastRoutingSnapshot: RoutingSnapshot | null;
  latestTrendNarrative: TrendNarrative | null;
  pendingSnippets: NarrativeSnippet[];
  activeMetaRules: MetaRule[];
  pendingMetaRules: MetaRule[];
}

const DEFAULT_MIRROR_STATE: MirrorState = {
  lastRoutingSnapshot: null,
  latestTrendNarrative: null,
  pendingSnippets: [],
  activeMetaRules: [],
  pendingMetaRules: [],
};

export function MirrorPage() {
  const { data: mirrorState } = useQuery<MirrorState>(
    "get_mirror_state",
    undefined,
    DEFAULT_MIRROR_STATE,
  );

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0 overflow-y-auto">
      <div className="flex flex-col gap-6 p-6 max-w-2xl w-full mx-auto">
        {/* Header */}
        <div className="flex items-center gap-2.5">
          <Eye className="size-5 text-muted-foreground" strokeWidth={1.5} />
          <h1 className="text-[15px] font-semibold text-foreground">The Mirror</h1>
        </div>

        {/* Weekly Reflection */}
        <NarrativeCard narrative={mirrorState?.latestTrendNarrative} />

        {/* Recent Insights */}
        <SnippetFeed snippets={mirrorState?.pendingSnippets ?? []} />

        {/* Meta-Rules */}
        <MetaRulesSection
          activeRules={mirrorState?.activeMetaRules ?? []}
          pendingRules={mirrorState?.pendingMetaRules ?? []}
        />

        {/* Skill Routing */}
        <RoutingDonut snapshot={mirrorState?.lastRoutingSnapshot} />

        {/* Conversational Input */}
        <MirrorInput />
      </div>
    </div>
  );
}
