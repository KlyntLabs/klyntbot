import type { BrainVersion } from "@features/mirror/components/BrainTimeline";
import { BrainTimeline } from "@features/mirror/components/BrainTimeline";
import type { TrialPreview } from "@features/mirror/components/ExperimentWatchlist";
import { ExperimentWatchlist } from "@features/mirror/components/ExperimentWatchlist";
import type { MetaRule } from "@features/mirror/components/MetaRulesSection";
import { MetaRulesSection } from "@features/mirror/components/MetaRulesSection";
import { MirrorInput } from "@features/mirror/components/MirrorInput";
import type { TrendNarrative } from "@features/mirror/components/NarrativeCard";
import { NarrativeCard } from "@features/mirror/components/NarrativeCard";
import type { RoutingSnapshot } from "@features/mirror/components/RoutingDonut";
import { RoutingDonut } from "@features/mirror/components/RoutingDonut";
import type { NarrativeSnippet } from "@features/mirror/components/SnippetFeed";
import { SnippetFeed } from "@features/mirror/components/SnippetFeed";
import { useQuery } from "@shared/hooks/useQuery";

interface MirrorState {
  lastRoutingSnapshot: RoutingSnapshot | null;
  latestTrendNarrative: TrendNarrative | null;
  pendingSnippets: NarrativeSnippet[];
  activeMetaRules: MetaRule[];
  pendingMetaRules: MetaRule[];
  latestBrainVersion: BrainVersion | null;
  recentTrialPreviews: TrialPreview[];
}

const DEFAULT_MIRROR_STATE: MirrorState = {
  lastRoutingSnapshot: null,
  latestTrendNarrative: null,
  pendingSnippets: [],
  activeMetaRules: [],
  pendingMetaRules: [],
  latestBrainVersion: null,
  recentTrialPreviews: [],
};

export function MirrorDetail() {
  const { data: mirrorState, refetch } = useQuery<MirrorState>(
    "get_mirror_state",
    undefined,
    DEFAULT_MIRROR_STATE,
  );

  return (
    <div className="flex flex-col gap-6 max-w-2xl">
      <NarrativeCard narrative={mirrorState?.latestTrendNarrative} />
      <SnippetFeed snippets={mirrorState?.pendingSnippets ?? []} />
      <ExperimentWatchlist previews={mirrorState?.recentTrialPreviews ?? []} onAction={refetch} />
      <MetaRulesSection
        activeRules={mirrorState?.activeMetaRules ?? []}
        pendingRules={mirrorState?.pendingMetaRules ?? []}
        onRuleAction={refetch}
      />
      <BrainTimeline />
      <RoutingDonut snapshot={mirrorState?.lastRoutingSnapshot} />
      <MirrorInput />
    </div>
  );
}
