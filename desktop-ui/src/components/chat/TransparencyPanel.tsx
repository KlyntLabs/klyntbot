import { useState } from 'react';
import {
  ChevronDown, FileText, Package, Cpu, Brain, Database, Bot, BookOpen,
} from 'lucide-react';
import { formatDuration } from '../../lib/utils';
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

function Row({ icon: Icon, label, detail }: { icon: React.ElementType; label: string; detail?: string }) {
  return (
    <div className="flex items-center gap-1.5 text-muted">
      <Icon className="w-3 h-3 shrink-0" strokeWidth={1.5} />
      <span className="text-secondary">{label}</span>
      {detail && <span className="text-dim ml-auto">{detail}</span>}
    </div>
  );
}

interface TransparencyPanelProps {
  transparency: TransparencyData;
}

export function TransparencyPanel({ transparency }: TransparencyPanelProps) {
  const { memoryAccesses, skills, execution, classification, subagents, learning, tools } = transparency;
  const hasKlyntbot = (memoryAccesses && memoryAccesses.length > 0) || (tools && tools.length > 0);
  const hasContext = skills && skills.length > 0;
  const hasExecution = execution || classification || (subagents && subagents.length > 0) || (learning && learning.length > 0);

  if (!hasKlyntbot && !hasContext && !hasExecution) return null;

  return (
    <div className="mt-2 space-y-1.5">
      {/* Klyntbot Box */}
      {hasKlyntbot && (
        <CollapsibleBox title="klyntbot" icon={Brain}>
          {memoryAccesses?.map((ma, i) => (
            <Row
              key={`mem-${i}`}
              icon={FileText}
              label={`memory: ${ma.query ?? ma.action}`}
              detail={ma.resultsCount > 0 ? `${ma.resultsCount} hits` : undefined}
            />
          ))}
          {tools?.map((tool, i) => (
            <Row
              key={`tool-${i}`}
              icon={Cpu}
              label={tool.name}
              detail={formatDuration(tool.durationMs)}
            />
          ))}
        </CollapsibleBox>
      )}

      {/* Context Box */}
      {hasContext && (
        <CollapsibleBox title="Context" icon={BookOpen}>
          {skills?.map((skill, i) => (
            <Row
              key={`skill-${i}`}
              icon={Package}
              label={`skill: ${skill.name}`}
              detail={skill.trigger}
            />
          ))}
        </CollapsibleBox>
      )}

      {/* Execution Detail */}
      {hasExecution && (
        <CollapsibleBox title="Execution" icon={Cpu} defaultOpen={false}>
          {execution && (
            <Row
              icon={Cpu}
              label={`Engine: ${execution.engine}`}
              detail={`${execution.iterations}/${execution.maxIterations} iterations`}
            />
          )}
          {classification && (
            <Row
              icon={Brain}
              label={`Strategy: ${classification.strategy}`}
              detail={`${Math.round(classification.confidence * 100)}%`}
            />
          )}
          {subagents?.map((sa, i) => (
            <Row key={`sa-${i}`} icon={Bot} label={`Agent: ${sa.label}`} detail={sa.profile} />
          ))}
          {learning?.map((le, i) => (
            <Row key={`le-${i}`} icon={Database} label={le.eventType} detail={le.detail} />
          ))}
          {(!subagents || subagents.length === 0) && (
            <Row icon={Bot} label="Agents: none" />
          )}
          {(!learning || learning.length === 0) && (
            <Row icon={Database} label="Learning: none" />
          )}
        </CollapsibleBox>
      )}
    </div>
  );
}
