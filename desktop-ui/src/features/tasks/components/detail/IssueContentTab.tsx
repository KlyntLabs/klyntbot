import { EditorContentWrapper, useNoteEditor } from "@features/notes/components/editor/EditorCore";
import { Bot, ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import type { SubIssue } from "../../lib/mappers";
import { renderStatusIcon } from "../../lib/status-utils";
import { useTabStore } from "../../store/tab-store";

interface IssueContentTabProps {
  detail: ReturnType<typeof useIssueDetail>;
}

export function IssueContentTab({ detail }: IssueContentTabProps) {
  return (
    <div className="space-y-6">
      <DescriptionEditor
        content={detail.task.description}
        onUpdate={(html) => detail.updateTask("description", html)}
      />
      {detail.task.acceptanceCriteria && (
        <AcceptanceCriteria text={detail.task.acceptanceCriteria} />
      )}
      <SubIssuesList issues={detail.subIssues} onDecompose={detail.openDecomposition} />
    </div>
  );
}

function DescriptionEditor({
  content,
  onUpdate,
}: {
  content: string;
  onUpdate: (html: string) => void;
}) {
  const editor = useNoteEditor({
    content,
    onUpdate: (html, _text) => onUpdate(html),
    onNavigateNote: () => {},
    onNavigateEntity: () => {},
  });
  return <EditorContentWrapper editor={editor} className="min-h-[200px]" />;
}

function AcceptanceCriteria({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const lines = text.split("\n").filter(Boolean);
  const preview = lines[0] ?? "";

  return (
    <div className="border border-border rounded-md">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-3 py-2 text-sm font-medium text-foreground hover:bg-accent/50 transition-colors"
      >
        {expanded ? (
          <ChevronDown className="size-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="size-4 text-muted-foreground" />
        )}
        Acceptance Criteria
        {!expanded && (
          <span className="text-muted-foreground font-normal truncate">— {preview}</span>
        )}
      </button>
      {expanded && (
        <div className="px-3 pb-3 text-sm text-foreground whitespace-pre-wrap font-mono">
          {text}
        </div>
      )}
    </div>
  );
}

function SubIssuesList({ issues, onDecompose }: { issues: SubIssue[]; onDecompose: () => void }) {
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);
  const completedCount = issues.filter((i) => i.completed).length;

  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-medium text-foreground">
          Sub-issues ({completedCount}/{issues.length} done)
        </h3>
        <button
          type="button"
          onClick={onDecompose}
          className="flex items-center gap-1 text-xs text-purple hover:text-purple/80 transition-colors"
        >
          <Bot className="size-3" />
          Break Down
        </button>
      </div>
      <div className="border border-border rounded-md divide-y divide-border">
        {issues.map((issue) => {
          const PriorityIcon = issue.priority.icon;
          return (
            <button
              key={issue.id}
              type="button"
              onClick={() => navigateInPlace("issue", issue.id, issue.identifier)}
              className="flex items-center gap-2 w-full px-3 py-2 text-sm hover:bg-accent/50 transition-colors text-left"
            >
              <span className="flex items-center justify-center size-4">
                {renderStatusIcon(issue.status)}
              </span>
              <PriorityIcon className="size-3.5 text-muted-foreground shrink-0" />
              <span className="text-xs text-muted-foreground shrink-0">{issue.identifier}</span>
              <span className="truncate text-foreground">{issue.title}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
