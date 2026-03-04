import { useState } from 'react';
import {
  ChevronDown, FileText, Package, Cpu, Brain, Database, Bot, BookOpen,
} from 'lucide-react';
import { formatDuration, formatTokens, qualifiedToolName } from '../../lib/utils';
import type { TransparencyData } from '../../lib/types';

interface CollapsibleBoxProps {
  title: string;
  icon: React.ElementType;
  children: React.ReactNode;
  defaultOpen?: boolean;
}

function CollapsibleBox({ title, icon: Icon, children, defaultOpen = true }: CollapsibleBoxProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2 bg-surface-raised hover:bg-surface-highest transition-colors"
      >
        <Icon className="w-3 h-3 text-muted" strokeWidth={1.5} />
        <span className="flex-1 text-left text-[11px] font-medium text-secondary">{title}</span>
        <ChevronDown
          className={`w-3 h-3 text-muted transition-transform ${open ? 'rotate-0' : '-rotate-90'}`}
          strokeWidth={1.5}
        />
      </button>
      {open && <div className="px-3 py-2 space-y-1 text-[10px] font-light">{children}</div>}
    </div>
  );
}

function Row({ icon: Icon, label, detail, active }: { icon: React.ElementType; label: string; detail?: string; active?: boolean }) {
  return (
    <div className={`flex items-center gap-1.5 ${active ? 'text-secondary' : 'text-muted'}`}>
      <Icon className={`w-3 h-3 shrink-0 ${active ? 'text-brand' : ''}`} strokeWidth={1.5} />
      <span className={active ? 'text-primary font-medium' : 'text-secondary'}>{label}</span>
      {detail && <span className={`ml-auto ${active ? 'text-brand text-[9px] font-medium uppercase' : 'text-dim'}`}>{detail}</span>}
    </div>
  );
}

interface TransparencyPanelProps {
  transparency: TransparencyData;
}

export function TransparencyPanel({ transparency }: TransparencyPanelProps) {
  const { memoryAccesses, skills, execution, classification, agentSelected, subagents, learning, tools, toolTokensTotal } = transparency;

  const hasMemory = memoryAccesses && memoryAccesses.length > 0;
  const hasSkills = skills && skills.length > 0;
  const hasExecution = execution || classification;
  const hasAgent = !!agentSelected;
  const hasSubagents = subagents && subagents.length > 0;
  const hasLearning = learning && learning.length > 0;
  const hasTools = tools && tools.length > 0;

  if (!hasMemory && !hasExecution && !hasTools) return null;

  return (
    <div className="w-64 border-l border-border bg-background overflow-y-auto p-3 space-y-2 shrink-0">
      <div className="text-[10px] font-medium text-dim uppercase tracking-wider px-1">Transparency</div>

      {/* Tools summary */}
      {hasTools && (
        <CollapsibleBox title="Tools" icon={Cpu}>
          {tools!.map((tool, i) => {
            const qName = qualifiedToolName(tool.name, tool.action);
            const parts = [formatDuration(tool.durationMs)];
            if (tool.estimatedTokens) parts.push(`~${formatTokens(tool.estimatedTokens)} tok`);
            return <Row key={`tool-${i}`} icon={Cpu} label={qName} detail={parts.join(' · ')} />;
          })}
          {toolTokensTotal && toolTokensTotal > 0 && (
            <div className="pt-1 mt-1 border-t border-border flex justify-between text-dim">
              <span>Total I/O (est.)</span>
              <span>~{formatTokens(toolTokensTotal)}</span>
            </div>
          )}
        </CollapsibleBox>
      )}

      {/* Memory */}
      {hasMemory && (
        <CollapsibleBox title="Memory" icon={Brain}>
          {memoryAccesses!.map((ma, i) => (
            <Row
              key={`mem-${i}`}
              icon={FileText}
              label={ma.query ?? ma.action}
              detail={ma.resultsCount > 0 ? `${ma.resultsCount} hits` : undefined}
            />
          ))}
        </CollapsibleBox>
      )}

      {/* Skills — always visible when execution data exists */}
      {hasExecution && (
        <CollapsibleBox title="Skills" icon={BookOpen} defaultOpen={hasSkills}>
          {hasSkills ? (
            skills!.map((skill, i) => {
              const isActive = skill.trigger === 'always' || skill.trigger === 'activated';
              return (
                <Row
                  key={`skill-${i}`}
                  icon={Package}
                  label={skill.name}
                  detail={isActive ? 'active' : skill.trigger}
                  active={isActive}
                />
              );
            })
          ) : (
            <span className="text-dim">none</span>
          )}
        </CollapsibleBox>
      )}

      {/* Execution */}
      {hasExecution && (
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
      )}

      {/* Agents — show selected agent + any delegated sub-agents */}
      {hasExecution && (
        <CollapsibleBox title="Agents" icon={Bot} defaultOpen={hasAgent || hasSubagents}>
          {hasAgent ? (
            <>
              <div className="flex items-start gap-1.5 text-muted">
                <Bot className="w-3 h-3 shrink-0 mt-0.5 text-brand" strokeWidth={1.5} />
                <div>
                  <span className="text-secondary font-medium">{agentSelected!.name}</span>
                  <div className="text-dim">{agentSelected!.description}</div>
                </div>
              </div>
              {hasSubagents && (
                <>
                  <div className="pt-1 mt-1 border-t border-border text-dim text-[9px] uppercase tracking-wider">Delegated</div>
                  {subagents!.map((sa, i) => (
                    <Row key={`sa-${i}`} icon={Bot} label={sa.label} detail={sa.profile} />
                  ))}
                </>
              )}
            </>
          ) : (
            <span className="text-dim">none</span>
          )}
        </CollapsibleBox>
      )}

      {/* Learning — always visible when execution data exists */}
      {hasExecution && (
        <CollapsibleBox title="Learning" icon={Database} defaultOpen={hasLearning}>
          {hasLearning ? (
            learning!.map((le, i) => (
              <Row key={`le-${i}`} icon={Database} label={le.eventType} detail={le.detail} />
            ))
          ) : (
            <span className="text-dim">none</span>
          )}
        </CollapsibleBox>
      )}
    </div>
  );
}
