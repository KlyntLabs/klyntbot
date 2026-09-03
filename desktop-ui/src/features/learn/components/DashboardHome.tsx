import { useQuery } from "@shared/hooks/useQuery";
import {
  Activity,
  AlertTriangle,
  Focus,
  GraduationCap,
  Network,
  Play,
  Plus,
  RefreshCw,
  TrendingUp,
} from "lucide-react";
import { lazy, Suspense } from "react";
import { Link } from "react-router";
import { useLearnDashboard } from "../hooks/useLearnDashboard";
import { useRetentionHistory } from "../hooks/useRetentionHistory";
import { useReviewStats } from "../hooks/useReviewStats";
import { AtomGraph } from "./AtomGraph";
import { CollapsibleSection } from "./CollapsibleSection";
import { DeckList } from "./DeckList";
import { QuickGenerate } from "./QuickGenerate";

const RetentionChart = lazy(() =>
  import("./RetentionChart").then((m) => ({ default: m.RetentionChart })),
);
const StatsBar = lazy(() => import("./StatsBar").then((m) => ({ default: m.StatsBar })));

interface StrugglingCard {
  id: string;
  front: string;
  back: string;
  deck: string;
  lapses: number;
  reviewCount: number;
  sourceNoteId: string | null;
}

interface DashboardHomeProps {
  onStartReview: (deck?: string) => void;
  onQuickAdd: () => void;
  onGenerateFromNote: (noteId: string) => void;
  onGenerateFromText: (text: string) => void;
  generating: boolean;
}

export function DashboardHome({
  onStartReview,
  onQuickAdd,
  onGenerateFromNote,
  onGenerateFromText,
  generating,
}: DashboardHomeProps) {
  const { decks, totalDue, loading } = useLearnDashboard();
  const { data: stats } = useReviewStats();
  const { data: retentionData } = useRetentionHistory(30);
  const { data: struggling } = useQuery<StrugglingCard[]>(
    "flashcard_list_struggling",
    { limit: 5 },
    [],
    { invalidateOn: ["entity:updated"] },
  );

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <GraduationCap className="size-12 text-fg-secondary mx-auto mb-3" strokeWidth={1.5} />
          <h1 className="text-xl font-semibold text-fg">Learning Hub</h1>
          <p className="text-fg-secondary mt-1 text-sm">Loading dashboard...</p>
        </div>
      </div>
    );
  }

  const isEmpty = decks.length === 0;

  if (isEmpty) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center max-w-sm space-y-4">
          <GraduationCap size={40} className="mx-auto text-fg-secondary" strokeWidth={1.5} />
          <div>
            <h1 className="text-xl font-semibold text-fg">Learning Hub</h1>
            <p className="text-sm text-fg-secondary mt-1">
              No flashcards yet. Create your first card to get started with spaced repetition
              learning.
            </p>
          </div>
          <button
            type="button"
            onClick={onQuickAdd}
            className="glass-button px-4 py-2 text-sm text-fg inline-flex items-center gap-2"
          >
            <Plus size={16} strokeWidth={1.5} />
            Create Card
            <span className="text-ui-xs text-fg-secondary ml-1">{"\u2318"}N</span>
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 p-6 space-y-5 overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <GraduationCap size={22} className="text-brand" strokeWidth={1.5} />
          <h1 className="text-lg font-semibold text-fg">Learning Hub</h1>
        </div>
        <button
          type="button"
          onClick={onQuickAdd}
          className="glass-button px-3 py-1.5 text-ui-sm text-fg inline-flex items-center gap-1.5"
        >
          <Plus size={14} strokeWidth={1.5} />
          Quick Add
          <span className="text-ui-xs text-fg-secondary">{"\u2318"}N</span>
        </button>
      </div>

      {/* Stats */}
      <Suspense
        fallback={
          <div className="flex items-center justify-center h-full text-fg-secondary text-sm">
            Loading...
          </div>
        }
      >
        <StatsBar
          totalDue={totalDue}
          streak={stats.streak}
          retention={stats.retention}
          weekly={stats.weekly}
        />
      </Suspense>

      {/* Action cards */}
      <div className="grid grid-cols-2 gap-3">
        <button
          type="button"
          onClick={() => onStartReview()}
          disabled={totalDue === 0}
          className="island p-4 text-left transition-all duration-200 hover:bg-white/[0.06] disabled:opacity-40 disabled:cursor-not-allowed group"
        >
          <div className="flex items-center gap-2 mb-2">
            <div className="p-1.5 rounded-lg bg-brand/10">
              <Play size={16} className="text-brand" strokeWidth={1.5} />
            </div>
            <span className="text-sm font-medium text-fg">Start Review</span>
          </div>
          <p className="text-ui-sm text-fg-secondary">
            {totalDue > 0
              ? `${totalDue} card${totalDue !== 1 ? "s" : ""} due for review`
              : "No cards due right now"}
          </p>
        </button>

        <QuickGenerate
          onGenerateFromNote={onGenerateFromNote}
          onGenerateFromText={onGenerateFromText}
          generating={generating}
        />
      </div>

      {/* Focused review link */}
      <Link
        to="/learn/review"
        className="island p-4 flex items-center gap-3 transition-all duration-200 hover:bg-white/[0.06] group"
      >
        <div className="p-1.5 rounded-lg bg-purple-500/10">
          <Focus size={16} className="text-purple-400" strokeWidth={1.5} />
        </div>
        <div className="flex-1 min-w-0">
          <span className="text-sm font-medium text-fg">Focused Review</span>
          <p className="text-ui-sm text-fg-secondary">
            Review all due cards in a distraction-free session
          </p>
        </div>
      </Link>

      {/* Knowledge Health link */}
      <Link
        to="/learn/knowledge"
        className="island p-4 flex items-center gap-3 transition-all duration-200 hover:bg-white/[0.06] group"
      >
        <div className="p-1.5 rounded-lg bg-green-500/10">
          <Activity size={16} className="text-green-400" strokeWidth={1.5} />
        </div>
        <div className="flex-1 min-w-0">
          <span className="text-sm font-medium text-fg">Knowledge Health</span>
          <p className="text-ui-sm text-fg-secondary">Track retention across topics</p>
        </div>
      </Link>

      {/* Needs Attention — struggling cards */}
      {struggling.length > 0 && (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <AlertTriangle size={14} className="text-red-400" strokeWidth={1.5} />
            <span className="text-sm font-medium text-fg">Needs Attention</span>
            <span className="text-ui-xs text-fg-secondary">
              {struggling.length} card{struggling.length !== 1 ? "s" : ""} struggling
            </span>
          </div>
          <div className="space-y-1.5">
            {struggling.map((card) => (
              <div key={card.id} className="island p-3 flex items-center gap-3 group">
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-fg truncate">{card.front}</p>
                  <div className="flex items-center gap-2 mt-0.5">
                    <span className="text-ui-xs text-fg-secondary">{card.deck}</span>
                    <span className="text-ui-xs text-red-400 font-medium">
                      {card.lapses} lapses
                    </span>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => onGenerateFromText(`${card.front}\n\nExpected: ${card.back}`)}
                  className="glass-button px-2.5 py-1 text-ui-xs text-fg inline-flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                >
                  <RefreshCw size={12} strokeWidth={1.5} />
                  Regenerate
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Deck list */}
      <DeckList decks={decks} onReviewDeck={onStartReview} />

      {/* Charts */}
      <CollapsibleSection
        title="Retention Trend"
        icon={<TrendingUp size={14} strokeWidth={1.5} />}
        storageKey="learn-retention-open"
      >
        <div className="island p-4">
          <Suspense
            fallback={
              <div className="flex items-center justify-center h-full text-fg-secondary text-sm">
                Loading...
              </div>
            }
          >
            <RetentionChart data={retentionData.overall} height={160} />
          </Suspense>
        </div>
      </CollapsibleSection>

      <CollapsibleSection
        title="Knowledge Map"
        icon={<Network size={14} strokeWidth={1.5} />}
        storageKey="learn-graph-open"
      >
        <div className="island p-4">
          <AtomGraph />
        </div>
      </CollapsibleSection>
    </div>
  );
}
