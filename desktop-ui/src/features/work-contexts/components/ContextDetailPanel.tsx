import { SlidePanel } from "@shared/composites/SlidePanel/SlidePanel";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { formatHumanDuration, formatTime } from "@shared/lib/dates";
import type { ActivityEvent, WorkContextDetail, WorkResource } from "@shared/types";
import {
  Archive,
  Clock,
  Edit3,
  ExternalLink,
  FileText,
  Globe,
  Layers,
  Play,
  Terminal,
} from "lucide-react";
import { useCallback, useState } from "react";
import { useContextResume } from "../hooks/useContextResume";
import { useContextMutations } from "../hooks/useWorkContexts";
import { contextColor } from "../lib/context-colors";

interface ContextDetailPanelProps {
  open: boolean;
  onClose: () => void;
  detail: WorkContextDetail | null;
}

const RESOURCE_ICONS: Record<string, typeof FileText> = {
  file: FileText,
  url: Globe,
  command: Terminal,
  app: Layers,
};

function ResourceIcon({ type }: { type: string }) {
  const Icon = RESOURCE_ICONS[type] ?? FileText;
  return <Icon className="w-3.5 h-3.5 text-muted shrink-0" strokeWidth={1.5} />;
}

export function ContextDetailPanel({ open, onClose, detail }: ContextDetailPanelProps) {
  const ctx = detail?.context;
  const { update, archive } = useContextMutations();
  const { resume } = useContextResume();
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");

  const handleRename = useCallback(async () => {
    if (!ctx || !titleDraft.trim()) return;
    await update.mutate({ id: ctx.id, title: titleDraft.trim() });
    invalidateQueries("list_work_contexts");
    invalidateQueries("get_work_context");
    setEditingTitle(false);
  }, [ctx, titleDraft, update]);

  const handleArchive = useCallback(async () => {
    if (!ctx) return;
    await archive.mutate({ id: ctx.id });
    invalidateQueries("list_work_contexts");
    onClose();
  }, [ctx, archive, onClose]);

  const handleResume = useCallback(() => {
    if (!ctx) return;
    resume(ctx.id);
    onClose();
  }, [ctx, resume, onClose]);

  const color = ctx ? contextColor(ctx.color, ctx.contextType) : "#6B7280";

  return (
    <SlidePanel open={open} onClose={onClose} title="Context Detail" width={460}>
      {!ctx ? (
        <p className="text-[13px] text-muted">No context selected</p>
      ) : (
        <div className="flex flex-col gap-5">
          {/* Header */}
          <div className="flex items-start gap-3">
            <div
              className="w-3 h-3 rounded-full mt-1 shrink-0"
              style={{ backgroundColor: color }}
            />
            <div className="flex-1 min-w-0">
              {editingTitle ? (
                <input
                  autoFocus
                  value={titleDraft}
                  onChange={(e) => setTitleDraft(e.target.value)}
                  onBlur={handleRename}
                  onKeyDown={(e) => e.key === "Enter" && handleRename()}
                  className="w-full px-2 py-1 text-[14px] font-medium bg-white/[0.06] border border-white/[0.08] rounded-lg text-primary"
                />
              ) : (
                <button
                  type="button"
                  className="text-[14px] font-medium text-primary hover:text-brand transition-colors flex items-center gap-1.5"
                  onClick={() => {
                    setTitleDraft(ctx.title);
                    setEditingTitle(true);
                  }}
                >
                  {ctx.title}
                  <Edit3 className="w-3 h-3 text-muted" />
                </button>
              )}
              <p className="text-[11px] text-muted mt-0.5">
                {ctx.contextType} · {ctx.status}
              </p>
            </div>
            <button
              type="button"
              onClick={handleResume}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-brand/20 text-brand text-[12px] font-medium hover:bg-brand/30 transition-colors shrink-0"
            >
              <Play className="w-3 h-3" />
              Resume
            </button>
          </div>

          {/* Stats */}
          <div className="grid grid-cols-3 gap-3">
            <Stat label="Duration" value={formatHumanDuration(ctx.totalDurationSecs)} />
            <Stat label="Events" value={String(ctx.eventCount)} />
            <Stat label="Confidence" value={`${Math.round(ctx.confidence * 100)}%`} />
          </div>

          {/* Key Resources */}
          {detail.resources.length > 0 && (
            <Section title="Key Resources">
              <div className="flex flex-col gap-1.5">
                {detail.resources.slice(0, 10).map((r) => (
                  <ResourceRow key={r.id} resource={r} />
                ))}
              </div>
            </Section>
          )}

          {/* Recent Events */}
          {detail.recentEvents.length > 0 && (
            <Section title="Recent Events">
              <div className="flex flex-col gap-1">
                {detail.recentEvents.slice(0, 20).map((e) => (
                  <EventRow key={e.id} event={e} />
                ))}
              </div>
            </Section>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 pt-2 border-t border-white/[0.08]">
            <button
              type="button"
              onClick={handleArchive}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[12px] text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <Archive className="w-3.5 h-3.5" />
              Archive
            </button>
          </div>
        </div>
      )}
    </SlidePanel>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-white/[0.04] rounded-lg p-2.5 text-center">
      <p className="text-[14px] font-medium text-primary">{value}</p>
      <p className="text-[10px] text-muted mt-0.5">{label}</p>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h4 className="text-[12px] font-medium text-secondary mb-2">{title}</h4>
      {children}
    </div>
  );
}

function ResourceRow({ resource }: { resource: WorkResource }) {
  return (
    <div className="flex items-center gap-2 text-[12px] text-secondary py-1 px-1.5 rounded hover:bg-white/[0.04]">
      <ResourceIcon type={resource.resourceType} />
      <span className="truncate flex-1">{resource.resourceName}</span>
      <span className="text-[10px] text-muted shrink-0">{resource.accessCount}×</span>
    </div>
  );
}

function EventRow({ event }: { event: ActivityEvent }) {
  return (
    <div className="flex items-center gap-2 text-[11px] py-1 px-1.5 rounded hover:bg-white/[0.04]">
      <span className="text-muted shrink-0 w-12 text-right">{formatTime(event.timestamp)}</span>
      <span className="text-secondary">{event.action}</span>
      {event.resourceName && (
        <span className="text-muted truncate flex-1">{event.resourceName}</span>
      )}
      {event.appName && <span className="text-dim text-[10px] shrink-0">{event.appName}</span>}
    </div>
  );
}
