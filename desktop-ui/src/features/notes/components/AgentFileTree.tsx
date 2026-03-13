import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { AgentProfileSummary } from "@shared/types";
import { Bot, ChevronDown, ChevronRight, FileText, Plus, Sparkles } from "lucide-react";
import { memo, useCallback, useState } from "react";

interface AgentFileTreeProps {
  activeAgent: string | null;
  activeFilename: string | null;
  onSelectFile: (agentName: string, filename: string) => void;
}

export const AgentFileTree = memo(function AgentFileTree({
  activeAgent,
  activeFilename,
  onSelectFile,
}: AgentFileTreeProps) {
  const { data: profiles, refetch } = useQuery<AgentProfileSummary[]>(
    "agent_list_profiles",
    undefined,
    [],
  );

  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem("agent-profiles-collapsed") !== "false";
    } catch {
      return true;
    }
  });

  const toggleCollapsed = useCallback(() => {
    setCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem("agent-profiles-collapsed", String(next));
      } catch {
        // ignore
      }
      return next;
    });
  }, []);

  const handleCreateAgent = useCallback(async () => {
    const name = prompt("Agent name:");
    if (!name?.trim()) return;
    try {
      await ipc("agent_create_profile", { name: name.trim() });
      refetch();
    } catch (e) {
      console.error("Failed to create agent profile:", e);
    }
  }, [refetch]);

  const handleCreateSkill = useCallback(
    async (agentName: string) => {
      const skillName = prompt("Skill name:");
      if (!skillName?.trim()) return;
      try {
        await ipc("agent_create_skill", { agentName, skillName: skillName.trim() });
        refetch();
      } catch (e) {
        console.error("Failed to create skill:", e);
      }
    },
    [refetch],
  );

  if (profiles.length === 0) return null;

  return (
    <div className="mt-2 pt-2 border-t border-white/[0.06]">
      <div className="flex items-center">
        <button
          type="button"
          onClick={toggleCollapsed}
          className="flex-1 flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-muted hover:text-secondary transition-colors"
        >
          {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
          <Bot size={12} />
          <span>Agent Profiles</span>
        </button>
        {!collapsed && (
          <button
            type="button"
            onClick={handleCreateAgent}
            className="mr-2 w-4 h-4 rounded flex items-center justify-center text-dim hover:text-primary hover:bg-white/[0.06] transition-colors"
            aria-label="New agent"
          >
            <Plus size={10} />
          </button>
        )}
      </div>

      {!collapsed && (
        <div className="mt-0.5">
          {profiles.map((profile) => (
            <AgentNode
              key={profile.name}
              profile={profile}
              activeAgent={activeAgent}
              activeFilename={activeFilename}
              onSelectFile={onSelectFile}
              onCreateSkill={handleCreateSkill}
            />
          ))}
        </div>
      )}
    </div>
  );
});

// ── Agent Node ──────────────────────────────────────────────────────────

interface AgentNodeProps {
  profile: AgentProfileSummary;
  activeAgent: string | null;
  activeFilename: string | null;
  onSelectFile: (agentName: string, filename: string) => void;
  onCreateSkill: (agentName: string) => void;
}

const AgentNode = memo(function AgentNode({
  profile,
  activeAgent,
  activeFilename,
  onSelectFile,
  onCreateSkill,
}: AgentNodeProps) {
  const [expanded, setExpanded] = useState(false);
  const isActive = activeAgent === profile.name;

  return (
    <div>
      <div className="flex items-center group">
        <button
          type="button"
          onClick={() => setExpanded((p) => !p)}
          className={`flex-1 flex items-center gap-1.5 px-3 py-1 text-xs transition-colors ${
            isActive && !activeFilename
              ? "bg-white/[0.08] text-primary"
              : "text-muted hover:text-secondary hover:bg-white/[0.03]"
          }`}
        >
          <ChevronRight
            size={10}
            className={`shrink-0 text-dim transition-transform ${expanded ? "rotate-90" : ""}`}
          />
          <Bot size={11} className="shrink-0 text-dim" />
          <span className="truncate font-mono">{profile.name}</span>
          {profile.isBuiltin && (
            <span className="text-[9px] text-dim bg-white/[0.06] px-1 rounded shrink-0">
              built-in
            </span>
          )}
          {profile.hasOverride && (
            <span className="text-[9px] text-brand/60 bg-brand/10 px-1 rounded shrink-0">
              override
            </span>
          )}
        </button>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onCreateSkill(profile.name);
          }}
          className="mr-2 w-4 h-4 rounded flex items-center justify-center text-dim opacity-0 group-hover:opacity-100 hover:text-primary hover:bg-white/[0.06] transition-all"
          aria-label={`New skill for ${profile.name}`}
        >
          <Plus size={9} />
        </button>
      </div>

      {expanded && (
        <div className="ml-3">
          {profile.files.map((file) => {
            const isFileActive = isActive && activeFilename === file.filename;
            const isSkill = file.filename.startsWith("skills/");

            return (
              <button
                key={file.filename}
                type="button"
                onClick={() => onSelectFile(profile.name, file.filename)}
                title={file.description}
                className={`w-full text-left flex items-center gap-1.5 px-3 py-0.5 text-[11px] transition-colors ${
                  isFileActive
                    ? "bg-white/[0.08] text-primary"
                    : "text-dim hover:text-secondary hover:bg-white/[0.03]"
                }`}
              >
                {isSkill ? (
                  <Sparkles size={10} className="shrink-0" />
                ) : (
                  <FileText size={10} className="shrink-0" />
                )}
                <span className="truncate font-mono">{file.displayName}</span>
                {file.isBuiltin && !file.hasOverride && (
                  <span className="text-[8px] text-dim/60 shrink-0">ro</span>
                )}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
});
