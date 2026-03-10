import { useProjectConversations } from "@shared/hooks";
import type { Project, SessionSummary } from "@shared/types";
import { BookOpen, FileText, MessageSquare, Plus, Settings, User } from "lucide-react";
import { useState } from "react";

interface ProjectLeftPanelProps {
  project: Project;
  onOpenInstructions: () => void;
  onOpenSources: () => void;
  onOpenRole: () => void;
}

export function ProjectLeftPanel({
  project,
  onOpenInstructions,
  onOpenSources,
  onOpenRole,
}: ProjectLeftPanelProps) {
  const { data: conversations } = useProjectConversations(project.id);
  const [selectedConvo, setSelectedConvo] = useState<string | null>(null);

  const hasInstructions = project.instructions && Object.values(project.instructions).some(Boolean);

  return (
    <div className="w-[250px] glass-sidebar flex flex-col overflow-y-auto shrink-0">
      {/* AI Context cards */}
      <div className="p-3 space-y-1.5">
        <button
          type="button"
          onClick={onOpenInstructions}
          className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg hover:bg-white/[0.04] transition-colors text-left"
        >
          <Settings className="w-3.5 h-3.5 text-brand shrink-0" strokeWidth={1.5} />
          <div className="flex-1 min-w-0">
            <p className="text-[12px] font-light text-secondary">Instructions</p>
            <p className="text-[10px] text-dim truncate">
              {hasInstructions ? "Configured" : "Not set"}
            </p>
          </div>
        </button>

        <button
          type="button"
          onClick={onOpenSources}
          className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg hover:bg-white/[0.04] transition-colors text-left"
        >
          <BookOpen className="w-3.5 h-3.5 text-brand shrink-0" strokeWidth={1.5} />
          <div className="flex-1 min-w-0">
            <p className="text-[12px] font-light text-secondary">Sources</p>
            <p className="text-[10px] text-dim">Reference material</p>
          </div>
        </button>

        <button
          type="button"
          onClick={onOpenRole}
          className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg hover:bg-white/[0.04] transition-colors text-left"
        >
          <User className="w-3.5 h-3.5 text-brand shrink-0" strokeWidth={1.5} />
          <div className="flex-1 min-w-0">
            <p className="text-[12px] font-light text-secondary">My Role</p>
            <p className="text-[10px] text-dim truncate">{project.userRole || "Not set"}</p>
          </div>
        </button>
      </div>

      {/* Separator */}
      <div className="mx-3 border-t border-white/[0.06]" />

      {/* Conversations */}
      <div className="flex-1 p-3 flex flex-col gap-1">
        <div className="flex items-center justify-between px-1 mb-1">
          <span className="text-[10px] font-medium text-dim uppercase tracking-wider">
            Conversations
          </span>
          <button
            type="button"
            className="text-muted hover:text-brand transition-colors"
            title="New conversation"
          >
            <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
          </button>
        </div>

        {conversations.map((convo: SessionSummary) => (
          <button
            key={convo.key}
            type="button"
            onClick={() => setSelectedConvo(convo.key)}
            className={`w-full flex items-center gap-2 px-2.5 py-2 rounded-md text-left transition-colors ${
              selectedConvo === convo.key
                ? "bg-white/[0.06] text-primary"
                : "hover:bg-white/[0.03] text-secondary"
            }`}
          >
            {convo.conversationType === "note" ? (
              <FileText className="w-3 h-3 text-muted shrink-0" strokeWidth={1.5} />
            ) : (
              <MessageSquare className="w-3 h-3 text-muted shrink-0" strokeWidth={1.5} />
            )}
            <span className="text-[12px] font-light truncate">{convo.title || "Untitled"}</span>
          </button>
        ))}

        {conversations.length === 0 && (
          <p className="text-[11px] text-dim font-light px-2 py-4 text-center">
            No conversations yet
          </p>
        )}
      </div>
    </div>
  );
}
