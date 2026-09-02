import { useQuery } from "@shared/hooks/useQuery";
import { retentionBarColor, retentionTextColor } from "@shared/lib/retention";
import { Activity, BookOpen, Brain, Flame } from "lucide-react";
import { useNavigate } from "react-router";

interface FadingAtom {
  id: string;
  subject: string;
  retentionPct: number;
  domain: string;
  sourceNoteId?: string;
}

interface TopicStat {
  name: string;
  avgRetention: number;
  atomCount: number;
}

interface MorningBriefingSummary {
  streakDays: number;
  dueCards: number;
  atomsReviewedThisWeek: number;
  atomsCreatedThisWeek: number;
  fadingAtoms: FadingAtom[];
  strongestTopic: TopicStat | null;
  weakestTopic: TopicStat | null;
}

const DEFAULT_SUMMARY: MorningBriefingSummary = {
  streakDays: 0,
  dueCards: 0,
  atomsReviewedThisWeek: 0,
  atomsCreatedThisWeek: 0,
  fadingAtoms: [],
  strongestTopic: null,
  weakestTopic: null,
};

function TopicBar({ topic }: { topic: TopicStat }) {
  const pct = Math.round(topic.avgRetention * 100);
  return (
    <div className="flex items-center gap-2">
      <span className="text-ui-xs text-fg-secondary w-24 truncate shrink-0">{topic.name}</span>
      <div className="flex-1 h-1.5 rounded-full bg-white/5 overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-500 ${retentionBarColor(topic.avgRetention)}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span
        className={`text-ui-xs font-medium tabular-nums w-8 text-right shrink-0 ${retentionTextColor(topic.avgRetention)}`}
      >
        {pct}%
      </span>
    </div>
  );
}

export function MorningBriefing() {
  const navigate = useNavigate();
  const { data: summary } = useQuery<MorningBriefingSummary>(
    "morning_briefing_summary",
    undefined,
    DEFAULT_SUMMARY,
  );

  const isEmpty =
    summary.dueCards === 0 && summary.streakDays === 0 && summary.fadingAtoms.length === 0;

  if (isEmpty) {
    return (
      <div className="island p-5">
        <div className="flex items-center gap-2 mb-2">
          <Brain size={16} className="text-fg-secondary" strokeWidth={1.5} />
          <h2 className="text-ui font-medium text-fg-secondary">Knowledge Health</h2>
        </div>
        <p className="text-ui-xs text-fg-secondary">
          No learning activity yet. Start reviewing flashcards or accept knowledge atoms from your
          notes to track retention here.
        </p>
      </div>
    );
  }

  return (
    <div className="island p-5">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Brain size={16} className="text-brand" strokeWidth={1.5} />
          <h2 className="text-ui font-medium text-fg-secondary">Knowledge Health</h2>
        </div>
        {summary.streakDays > 0 && (
          <div className="flex items-center gap-1">
            <Flame size={14} className="text-amber-400" strokeWidth={1.5} />
            <span className="text-ui-sm font-medium text-amber-400 tabular-nums">
              {summary.streakDays}-day streak
            </span>
          </div>
        )}
      </div>

      {/* Stats row */}
      <div className="flex items-center gap-4 mb-4">
        <div className="flex items-center gap-1.5">
          <BookOpen size={12} className="text-brand" strokeWidth={1.5} />
          <span className="text-ui-xs text-fg-secondary">
            <span className="font-medium text-fg tabular-nums">{summary.dueCards}</span> due
          </span>
        </div>
        <div className="text-ui-xs text-fg-secondary">
          <span className="font-medium text-fg tabular-nums">
            {summary.atomsReviewedThisWeek}
          </span>{" "}
          reviewed this week
        </div>
        <div className="text-ui-xs text-fg-secondary">
          <span className="font-medium text-fg tabular-nums">
            {summary.atomsCreatedThisWeek}
          </span>{" "}
          created this week
        </div>
      </div>

      {/* Two-column: Fading atoms + Topics */}
      <div className="grid grid-cols-2 gap-4">
        {/* Fading atoms */}
        {summary.fadingAtoms.length > 0 && (
          <div>
            <h3 className="text-ui-xs font-medium text-fg-secondary uppercase tracking-wider mb-2">
              Fading Atoms
            </h3>
            <div className="flex flex-col gap-1">
              {summary.fadingAtoms.slice(0, 5).map((atom) => {
                const pct = Math.round(atom.retentionPct * 100);
                return (
                  <button
                    type="button"
                    key={atom.id}
                    className="flex items-center gap-2 w-full text-left hover:bg-white/5 rounded px-1 -mx-1 transition-colors"
                    onClick={() => {
                      if (atom.sourceNoteId) {
                        navigate(`/notes?noteId=${atom.sourceNoteId}&atomId=${atom.id}`);
                      }
                    }}
                  >
                    <span className="text-ui-xs text-fg truncate flex-1">
                      {atom.subject}
                    </span>
                    <span
                      className={`text-ui-xs font-medium tabular-nums shrink-0 ${retentionTextColor(atom.retentionPct)}`}
                    >
                      {pct}%
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* Strongest + Weakest topics */}
        <div className="flex flex-col gap-3">
          {summary.strongestTopic && (
            <div>
              <h3 className="text-ui-xs font-medium text-fg-secondary uppercase tracking-wider mb-1.5">
                Strongest
              </h3>
              <TopicBar topic={summary.strongestTopic} />
            </div>
          )}
          {summary.weakestTopic && (
            <div>
              <h3 className="text-ui-xs font-medium text-fg-secondary uppercase tracking-wider mb-1.5">
                Weakest
              </h3>
              <TopicBar topic={summary.weakestTopic} />
            </div>
          )}
        </div>
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-2 mt-4 pt-3 border-t border-separator/30">
        <button
          type="button"
          onClick={() => navigate("/learn")}
          className="glass-button px-3 py-1.5 text-ui-xs text-fg inline-flex items-center gap-1.5"
        >
          <BookOpen size={12} strokeWidth={1.5} />
          Start Review
        </button>
        <button
          type="button"
          onClick={() => navigate("/learn/knowledge")}
          className="glass-button px-3 py-1.5 text-ui-xs text-fg inline-flex items-center gap-1.5"
        >
          <Activity size={12} strokeWidth={1.5} />
          See Health
        </button>
      </div>
    </div>
  );
}
