import { formatDuration, formatTokens, groupBy, qualifiedToolName } from "@shared/lib/utils";
import type { DelegationInfo, TransparencyData } from "@shared/types";
import { Bot, Brain, ChevronDown, Cpu, Database, FileText, Package } from "lucide-react";
import { useMemo, useState } from "react";

interface CollapsibleBoxProps {
  title: string;
  icon: React.ElementType;
  children: React.ReactNode;
  defaultOpen?: boolean;
}

function CollapsibleBox({ title, icon: Icon, children, defaultOpen = true }: CollapsibleBoxProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2 rounded-t-[var(--radius-2xl)] bg-surface-base"
      >
        <Icon className="w-3 h-3 text-muted" strokeWidth={1.5} />
        <span className="flex-1 text-left text-[11px] font-medium text-secondary">{title}</span>
        <ChevronDown
          className={`w-3 h-3 text-muted transition-transform ${open ? "rotate-0" : "-rotate-90"}`}
          strokeWidth={1.5}
        />
      </button>
      {open && <div className="px-3 py-2 space-y-1 text-[10px] font-light">{children}</div>}
    </div>
  );
}

function Row({
  icon: Icon,
  label,
  detail,
  active,
}: {
  icon: React.ElementType;
  label: string;
  detail?: string;
  active?: boolean;
}) {
  return (
    <div className={`flex items-center gap-1.5 ${active ? "text-secondary" : "text-muted"}`}>
      <Icon className={`w-3 h-3 shrink-0 ${active ? "text-brand" : ""}`} strokeWidth={1.5} />
      <span className={active ? "text-primary font-medium" : "text-secondary"}>{label}</span>
      {detail && (
        <span
          className={`ml-auto ${active ? "text-brand text-[9px] font-medium uppercase" : "text-dim"}`}
        >
          {detail}
        </span>
      )}
    </div>
  );
}

function AgentGroupLabel({ name }: { name: string }) {
  return (
    <div className="pt-1 first:pt-0 text-dim text-[9px] font-medium uppercase tracking-wider">
      {name}
    </div>
  );
}

/** Skills popup shown on hover over an agent name in the Agents section. */
function AgentSkillsPopup({ skills }: { skills: { name: string; trigger: string }[] }) {
  return (
    <div className="absolute left-0 top-full mt-1 z-50 w-48 rounded-xl p-2.5 space-y-0.5 text-[10px] font-light bg-surface-floating border border-border">
      <div className="text-dim text-[9px] font-medium uppercase tracking-wider mb-1">Skills</div>
      {skills.map((skill) => {
        const isActive = skill.trigger === "always" || skill.trigger === "activated";
        return (
          <div key={`sp-${skill.name}`} className="flex items-center gap-1.5">
            <Package
              className={`w-2.5 h-2.5 shrink-0 ${isActive ? "text-brand" : "text-muted"}`}
              strokeWidth={1.5}
            />
            <span className={isActive ? "text-primary font-medium" : "text-secondary"}>
              {skill.name}
            </span>
            <span
              className={`ml-auto ${isActive ? "text-brand text-[9px] font-medium uppercase" : "text-dim"}`}
            >
              {isActive ? "active" : skill.trigger}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/** Agent name with hover-to-show skills popup and optional delegation status. */
function AgentWithSkills({
  name,
  skills,
  isMain,
  delegation,
}: {
  name: string;
  skills?: { name: string; trigger: string }[];
  isMain?: boolean;
  delegation?: DelegationInfo;
}) {
  const [hovered, setHovered] = useState(false);
  const hasSkills = skills && skills.length > 0;
  const statusIcon = delegation
    ? delegation.status === "active"
      ? "⟳"
      : delegation.status === "completed"
        ? "✓"
        : "✗"
    : null;
  const detail = delegation?.durationMs != null ? formatDuration(delegation.durationMs) : undefined;

  return (
    <fieldset
      className="relative border-none p-0 m-0"
      onMouseEnter={() => hasSkills && setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div className={`flex items-center gap-1.5 ${isMain ? "text-muted" : "text-muted pl-3"}`}>
        {isMain ? (
          <Bot className="w-3 h-3 shrink-0 text-brand" strokeWidth={1.5} />
        ) : (
          <span className="text-[9px]">{statusIcon ?? "◇"}</span>
        )}
        <span
          className={`text-secondary ${isMain ? "font-medium" : ""} ${hasSkills ? "underline decoration-dotted decoration-border underline-offset-2 cursor-default" : ""}`}
        >
          {name}
        </span>
        {detail && <span className="ml-auto text-dim">{detail}</span>}
      </div>
      {hovered && hasSkills && skills && <AgentSkillsPopup skills={skills} />}
    </fieldset>
  );
}

type TransparencyTool = NonNullable<TransparencyData["tools"]>[number];

function ToolRow({ tool }: { tool: TransparencyTool }) {
  const qName = qualifiedToolName(tool.name, tool.action);
  const parts = [formatDuration(tool.durationMs)];
  if (tool.estimatedTokens) parts.push(`~${formatTokens(tool.estimatedTokens)} tok`);
  return <Row icon={Cpu} label={qName} detail={parts.join(" · ")} />;
}

interface TransparencyPanelProps {
  transparency: TransparencyData;
}

export function TransparencyPanel({ transparency }: TransparencyPanelProps) {
  const {
    memoryAccesses,
    skills,
    execution,
    classification,
    agentSelected,
    delegations,
    learning,
    tools,
    toolTokensTotal,
  } = transparency;

  const hasMemory = memoryAccesses && memoryAccesses.length > 0;
  const hasSkills = skills && skills.length > 0;
  const hasExecution = execution || classification;
  const hasAgent = !!agentSelected;
  const hasDelegations = delegations && delegations.length > 0;
  const hasLearning = learning && learning.length > 0;
  const hasTools = tools && tools.length > 0;

  // Group tools by agent when delegations exist
  const toolsByAgent = useMemo(
    () =>
      hasDelegations && hasTools && tools
        ? groupBy(tools, (t) => t.agent ?? agentSelected?.name ?? "unknown")
        : null,
    [hasDelegations, hasTools, tools, agentSelected?.name],
  );

  // Group skills by agent for hover popups
  const skillsByAgent = useMemo(
    () =>
      hasSkills && skills
        ? groupBy(skills, (s) => s.agent ?? agentSelected?.name ?? "unknown")
        : {},
    [hasSkills, skills, agentSelected?.name],
  );

  if (!hasMemory && !hasExecution && !hasTools) return null;

  return (
    <div className="w-56 space-y-2">
      {/* Agents */}
      {hasExecution && (
        <div className="glass-bubble relative z-10">
          <CollapsibleBox title="Agents" icon={Bot} defaultOpen={hasAgent || hasDelegations}>
            {hasAgent ? (
              <>
                <AgentWithSkills
                  name={agentSelected?.name}
                  skills={skillsByAgent[agentSelected?.name]}
                  isMain
                />
                {hasDelegations &&
                  delegations?.map((d) => (
                    <AgentWithSkills
                      key={`del-${d.toAgent}`}
                      name={d.toAgent}
                      skills={skillsByAgent[d.toAgent]}
                      delegation={d}
                    />
                  ))}
              </>
            ) : (
              <span className="text-dim">none</span>
            )}
          </CollapsibleBox>
        </div>
      )}

      {/* Tools */}
      {hasTools && (
        <div className="glass-bubble overflow-hidden">
          <CollapsibleBox title="Tools" icon={Cpu}>
            {toolsByAgent
              ? Object.entries(toolsByAgent).map(([agent, agentTools]) => (
                  <div key={`tg-${agent}`}>
                    <AgentGroupLabel name={agent} />
                    {agentTools.map((tool) => (
                      <ToolRow
                        key={`tool-${agent}-${tool.name}-${tool.action ?? ""}`}
                        tool={tool}
                      />
                    ))}
                  </div>
                ))
              : tools?.map((tool) => (
                  <ToolRow
                    key={`tool-${tool.name}-${tool.action ?? ""}-${tool.durationMs}`}
                    tool={tool}
                  />
                ))}
            {toolTokensTotal && toolTokensTotal > 0 && (
              <div className="pt-1 mt-1 border-t border-border flex justify-between text-dim">
                <span>Total I/O (est.)</span>
                <span>~{formatTokens(toolTokensTotal)}</span>
              </div>
            )}
          </CollapsibleBox>
        </div>
      )}

      {/* Memory */}
      {hasMemory && (
        <div className="glass-bubble overflow-hidden">
          <CollapsibleBox title="Memory" icon={Brain}>
            {memoryAccesses?.map((ma) => (
              <Row
                key={`mem-${ma.action}-${ma.query ?? ""}`}
                icon={FileText}
                label={ma.query ?? ma.action}
                detail={ma.resultsCount > 0 ? `${ma.resultsCount} hits` : undefined}
              />
            ))}
          </CollapsibleBox>
        </div>
      )}

      {/* Execution */}
      {hasExecution && (
        <div className="glass-bubble overflow-hidden">
          <CollapsibleBox title="Execution" icon={Cpu}>
            {execution && (
              <Row
                icon={Cpu}
                label={`Engine: ${execution.engine}`}
                detail={`${execution.iterations}/${execution.maxIterations} itr`}
              />
            )}
            {classification && (
              <Row
                icon={Brain}
                label={`Strategy: ${classification.strategy}`}
                detail={`${Math.round(classification.confidence * 100)}%`}
              />
            )}
          </CollapsibleBox>
        </div>
      )}

      {/* Learning */}
      {hasExecution && (
        <div className="glass-bubble overflow-hidden">
          <CollapsibleBox title="Learning" icon={Database} defaultOpen={hasLearning}>
            {hasLearning ? (
              learning?.map((le) => (
                <Row
                  key={`le-${le.eventType}-${le.detail ?? ""}`}
                  icon={Database}
                  label={le.eventType}
                  detail={le.detail}
                />
              ))
            ) : (
              <span className="text-dim">none</span>
            )}
          </CollapsibleBox>
        </div>
      )}
    </div>
  );
}
