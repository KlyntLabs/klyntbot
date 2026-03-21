import { Activity, GraduationCap, Play, Plus } from "lucide-react";
import { Link } from "react-router";
import { useLearnDashboard } from "../hooks/useLearnDashboard";
import { DeckList } from "./DeckList";
import { QuickGenerate } from "./QuickGenerate";
import { StatsBar } from "./StatsBar";

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

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <GraduationCap
            className="w-12 h-12 text-muted-foreground mx-auto mb-3"
            strokeWidth={1.5}
          />
          <h1 className="text-xl font-semibold text-foreground">Learning Hub</h1>
          <p className="text-muted-foreground mt-1 text-sm">Loading dashboard...</p>
        </div>
      </div>
    );
  }

  const isEmpty = decks.length === 0;

  if (isEmpty) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center max-w-sm space-y-4">
          <GraduationCap size={40} className="mx-auto text-muted-foreground" strokeWidth={1.5} />
          <div>
            <h1 className="text-xl font-semibold text-foreground">Learning Hub</h1>
            <p className="text-sm text-muted-foreground mt-1">
              No flashcards yet. Create your first card to get started with spaced repetition
              learning.
            </p>
          </div>
          <button
            type="button"
            onClick={onQuickAdd}
            className="glass-button px-4 py-2 text-sm text-foreground inline-flex items-center gap-2"
          >
            <Plus size={16} strokeWidth={1.5} />
            Create Card
            <span className="text-[10px] text-muted-foreground ml-1">{"\u2318"}N</span>
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
          <h1 className="text-lg font-semibold text-foreground">Learning Hub</h1>
        </div>
        <button
          type="button"
          onClick={onQuickAdd}
          className="glass-button px-3 py-1.5 text-[12px] text-foreground inline-flex items-center gap-1.5"
        >
          <Plus size={14} strokeWidth={1.5} />
          Quick Add
          <span className="text-[10px] text-muted-foreground">{"\u2318"}N</span>
        </button>
      </div>

      {/* Stats */}
      <StatsBar totalDue={totalDue} />

      {/* Action cards */}
      <div className="grid grid-cols-2 gap-3">
        <button
          type="button"
          onClick={() => onStartReview()}
          disabled={totalDue === 0}
          className="glass-card p-4 text-left transition-all duration-200 hover:bg-white/[0.06] disabled:opacity-40 disabled:cursor-not-allowed group"
        >
          <div className="flex items-center gap-2 mb-2">
            <div className="p-1.5 rounded-lg bg-brand/10">
              <Play size={16} className="text-brand" strokeWidth={1.5} />
            </div>
            <span className="text-sm font-medium text-foreground">Start Review</span>
          </div>
          <p className="text-[12px] text-muted-foreground">
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

      {/* Knowledge Health link */}
      <Link
        to="/learn/knowledge"
        className="glass-card p-4 flex items-center gap-3 transition-all duration-200 hover:bg-white/[0.06] group"
      >
        <div className="p-1.5 rounded-lg bg-green-500/10">
          <Activity size={16} className="text-green-400" strokeWidth={1.5} />
        </div>
        <div className="flex-1 min-w-0">
          <span className="text-sm font-medium text-foreground">Knowledge Health</span>
          <p className="text-[12px] text-muted-foreground">Track retention across topics</p>
        </div>
      </Link>

      {/* Deck list */}
      <DeckList decks={decks} onReviewDeck={onStartReview} />
    </div>
  );
}
