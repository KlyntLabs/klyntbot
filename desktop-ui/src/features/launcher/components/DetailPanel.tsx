import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem, LauncherItemKind } from "../types";

export function DetailPanel() {
  const item = useLauncherStore((s) => s.detailItem);

  if (!item) return null;

  return (
    <div className="max-h-[500px] overflow-y-auto">
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border">
        <button
          type="button"
          className="text-xs text-muted-foreground hover:text-foreground transition-colors px-1.5 py-0.5 rounded hover:bg-muted"
          onClick={() => {
            useLauncherStore.getState().setDetailItem(null);
            useLauncherStore.getState().setMode("search");
          }}
        >
          &larr; Back
          <span className="ml-1 text-muted-foreground/60">Tab</span>
        </button>
        <span className="text-sm text-foreground truncate">{item.title}</span>
      </div>
      <div className="p-4">
        <DetailContent item={item} />
      </div>
    </div>
  );
}

function DetailContent({ item }: { item: LauncherItem }) {
  const { kind } = item;

  switch (kind.type) {
    case "application":
      return <ApplicationDetail item={item} kind={kind} />;
    case "file":
      return <FileDetail item={item} kind={kind} />;
    case "contentMatch":
      return <ContentMatchDetail item={item} kind={kind} />;
    case "task":
      return <TaskDetail item={item} kind={kind} />;
    case "note":
      return <NoteDetail item={item} kind={kind} />;
    case "calculator":
      return <CalculatorDetail item={item} kind={kind} />;
    case "bookmark":
      return <BookmarkDetail item={item} kind={kind} />;
    case "browserHistory":
      return <BrowserHistoryDetail item={item} kind={kind} />;
    case "contact":
      return <ContactDetail item={item} kind={kind} />;
    case "gitRepo":
      return <GitRepoDetail item={item} kind={kind} />;
    case "sshHost":
      return <SshHostDetail item={item} kind={kind} />;
    case "systemCommand":
      return <SystemCommandDetail item={item} kind={kind} />;
    default:
      return <DefaultDetail item={item} />;
  }
}

function DetailField({ label, value }: { label: string; value: string | null | undefined }) {
  if (!value) return null;
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-2xs text-muted-foreground uppercase tracking-wider">{label}</span>
      <span className="text-sm text-foreground break-all">{value}</span>
    </div>
  );
}

function KindTag({ label }: { label: string }) {
  return (
    <span className="text-2xs text-muted-foreground px-1.5 py-0.5 rounded bg-accent inline-block">
      {label}
    </span>
  );
}

function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    doing: "text-info bg-info/10",
    todo: "text-muted-foreground bg-accent",
    blocked: "text-destructive bg-destructive/10",
    done: "text-success bg-success/10",
  };
  const colorClass = colors[status] || colors.todo;
  return <span className={`text-xs px-2 py-0.5 rounded-full ${colorClass}`}>{status}</span>;
}

type KindOf<T extends LauncherItemKind["type"]> = Extract<LauncherItemKind, { type: T }>;

function ApplicationDetail({ item, kind }: { item: LauncherItem; kind: KindOf<"application"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        {item.icon?.startsWith("data:") && (
          <img src={item.icon} alt="" className="size-12 rounded-lg" />
        )}
        <div>
          <div className="text-base text-foreground font-medium">{item.title}</div>
          {kind.running && <span className="text-xs text-success">Running</span>}
        </div>
      </div>
      <DetailField label="Path" value={kind.path} />
      <KindTag label="Application" />
    </div>
  );
}

function FileDetail({ item, kind }: { item: LauncherItem; kind: KindOf<"file"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      <DetailField label="Path" value={kind.path} />
      <div className="flex items-center gap-2">
        <KindTag label={kind.kind.charAt(0).toUpperCase() + kind.kind.slice(1)} />
        {kind.kind === "code" && (
          <span className="text-2xs text-muted-foreground">Open in Editor &mdash; Enter</span>
        )}
      </div>
      {item.subtitle && <DetailField label="Info" value={item.subtitle} />}
    </div>
  );
}

function ContentMatchDetail({ item, kind }: { item: LauncherItem; kind: KindOf<"contentMatch"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      <DetailField label="File" value={kind.path} />
      <DetailField label="Line" value={String(kind.line)} />
      <div className="bg-accent rounded p-3 text-xs text-foreground font-mono whitespace-pre-wrap">
        {kind.preview}
      </div>
      <KindTag label="Content Match" />
    </div>
  );
}

function TaskDetail({ item, kind }: { item: LauncherItem; kind: KindOf<"task"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      <div className="flex items-center gap-2">
        <StatusBadge status={kind.status} />
        {item.subtitle && <span className="text-xs text-muted-foreground">{item.subtitle}</span>}
      </div>
      {item.subtitle && <DetailField label="Description" value={item.subtitle} />}
      <KindTag label="Task" />
    </div>
  );
}

function NoteDetail({ item, kind }: { item: LauncherItem; kind: KindOf<"note"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      {kind.preview && (
        <div className="text-sm text-muted-foreground leading-relaxed whitespace-pre-wrap">
          {kind.preview}
        </div>
      )}
      <KindTag label="Note" />
    </div>
  );
}

function CalculatorDetail({ kind }: { item: LauncherItem; kind: KindOf<"calculator"> }) {
  return (
    <div className="flex flex-col gap-4">
      <div className="text-sm text-muted-foreground font-mono">{kind.expression}</div>
      <div className="text-3xl text-foreground font-mono font-medium">{kind.result}</div>
      <span className="text-2xs text-muted-foreground">Copied to clipboard on Enter</span>
      <KindTag label="Calculator" />
    </div>
  );
}

function BookmarkDetail({ item, kind }: { item: LauncherItem; kind: KindOf<"bookmark"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      <DetailField label="URL" value={kind.url} />
      <div className="flex items-center gap-2">
        <KindTag label="Bookmark" />
        <KindTag label={kind.browser} />
      </div>
    </div>
  );
}

function BrowserHistoryDetail({
  item,
  kind,
}: {
  item: LauncherItem;
  kind: KindOf<"browserHistory">;
}) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      <DetailField label="URL" value={kind.url} />
      <DetailField label="Visited" value={new Date(kind.visitedAt).toLocaleString()} />
      <KindTag label="Browser History" />
    </div>
  );
}

function ContactDetail({ kind }: { item: LauncherItem; kind: KindOf<"contact"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{kind.name}</div>
      <DetailField label="Email" value={kind.email} />
      <DetailField label="Phone" value={kind.phone} />
      <KindTag label="Contact" />
    </div>
  );
}

function GitRepoDetail({ item, kind }: { item: LauncherItem; kind: KindOf<"gitRepo"> }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      <DetailField label="Path" value={kind.path} />
      <KindTag label="Git Repository" />
    </div>
  );
}

function SshHostDetail({ kind }: { item: LauncherItem; kind: KindOf<"sshHost"> }) {
  const sshCmd = kind.user ? `ssh ${kind.user}@${kind.host}` : `ssh ${kind.host}`;
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{kind.host}</div>
      {kind.user && <DetailField label="User" value={kind.user} />}
      <DetailField label="Command" value={sshCmd} />
      <KindTag label="SSH Host" />
    </div>
  );
}

function SystemCommandDetail({
  item,
  kind,
}: {
  item: LauncherItem;
  kind: KindOf<"systemCommand">;
}) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      {item.subtitle && <DetailField label="Description" value={item.subtitle} />}
      <DetailField label="Action" value={kind.action} />
      <KindTag label="System Command" />
    </div>
  );
}

function DefaultDetail({ item }: { item: LauncherItem }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="text-base text-foreground font-medium">{item.title}</div>
      {item.subtitle && <DetailField label="Info" value={item.subtitle} />}
      <KindTag label={item.kind.type} />
    </div>
  );
}
