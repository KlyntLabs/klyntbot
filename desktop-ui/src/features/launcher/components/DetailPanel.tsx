import { useLauncherApi, useLauncherState } from "../store";
import type { LauncherItem, LauncherItemKind } from "../types";

export function DetailPanel() {
	const item = useLauncherState((s) => s.detailItem);
	const { setDetailItem, setMode } = useLauncherApi();

	if (!item) return null;

	return (
		<div className="lc-detail">
			<div className="lc-detail-header">
				<button
					type="button"
					className="lc-detail-back"
					onClick={() => {
						setDetailItem(null);
						setMode("search");
					}}
				>
					← Back <span className="lc-dim-xs">Tab</span>
				</button>
				<span className="lc-detail-title">{item.title}</span>
			</div>
			<div className="lc-detail-body">
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
			return <CalculatorDetail kind={kind} />;
		case "bookmark":
			return <BookmarkDetail item={item} kind={kind} />;
		case "browserHistory":
			return <BrowserHistoryDetail item={item} kind={kind} />;
		case "contact":
			return <ContactDetail kind={kind} />;
		case "gitRepo":
			return <GitRepoDetail item={item} kind={kind} />;
		case "sshHost":
			return <SshHostDetail kind={kind} />;
		case "systemCommand":
			return <SystemCommandDetail item={item} kind={kind} />;
		default:
			return <DefaultDetail item={item} />;
	}
}

function DetailField({
	label,
	value,
}: {
	label: string;
	value: string | null | undefined;
}) {
	if (!value) return null;
	return (
		<div className="lc-field">
			<span className="lc-field-label">{label}</span>
			<span className="lc-field-value">{value}</span>
		</div>
	);
}

function KindTag({ label }: { label: string }) {
	return <span className="lc-detail-tag">{label}</span>;
}

function StatusBadge({ status }: { status: string }) {
	return <span className={`lc-status-badge status-${status}`}>{status}</span>;
}

type KindOf<T extends LauncherItemKind["type"]> = Extract<
	LauncherItemKind,
	{ type: T }
>;

function ApplicationDetail({
	item,
	kind,
}: {
	item: LauncherItem;
	kind: KindOf<"application">;
}) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-app-row">
				{item.icon?.startsWith("data:") && (
					<img src={item.icon} alt="" className="lc-detail-app-icon" />
				)}
				<div>
					<div className="lc-detail-name">{item.title}</div>
					{kind.running && <span className="lc-detail-running">Running</span>}
				</div>
			</div>
			<DetailField label="Path" value={kind.path} />
			<KindTag label="Application" />
		</div>
	);
}

function FileDetail({
	item,
	kind,
}: {
	item: LauncherItem;
	kind: KindOf<"file">;
}) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			<DetailField label="Path" value={kind.path} />
			<KindTag label={kind.kind.charAt(0).toUpperCase() + kind.kind.slice(1)} />
			{item.subtitle && <DetailField label="Info" value={item.subtitle} />}
		</div>
	);
}

function ContentMatchDetail({
	item,
	kind,
}: {
	item: LauncherItem;
	kind: KindOf<"contentMatch">;
}) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			<DetailField label="File" value={kind.path} />
			<DetailField label="Line" value={String(kind.line)} />
			<div className="lc-detail-preview">{kind.preview}</div>
			<KindTag label="Content Match" />
		</div>
	);
}

function TaskDetail({
	item,
	kind,
}: {
	item: LauncherItem;
	kind: KindOf<"task">;
}) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			<StatusBadge status={kind.status} />
			{item.subtitle && (
				<DetailField label="Description" value={item.subtitle} />
			)}
			<KindTag label="Task" />
		</div>
	);
}

function NoteDetail({
	item,
	kind,
}: {
	item: LauncherItem;
	kind: KindOf<"note">;
}) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			{kind.preview && (
				<div className="lc-detail-note-body">{kind.preview}</div>
			)}
			<KindTag label="Note" />
		</div>
	);
}

function CalculatorDetail({ kind }: { kind: KindOf<"calculator"> }) {
	return (
		<div className="lc-detail-block">
			<div className="lc-calc-expr">{kind.expression}</div>
			<div className="lc-calc-result">{kind.result}</div>
			<span className="lc-dim-xs">Copied to clipboard on Enter</span>
			<KindTag label="Calculator" />
		</div>
	);
}

function BookmarkDetail({
	item,
	kind,
}: {
	item: LauncherItem;
	kind: KindOf<"bookmark">;
}) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			<DetailField label="URL" value={kind.url} />
			<KindTag label="Bookmark" />
			<KindTag label={kind.browser} />
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
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			<DetailField label="URL" value={kind.url} />
			<DetailField
				label="Visited"
				value={new Date(kind.visitedAt).toLocaleString()}
			/>
			<KindTag label="Browser History" />
		</div>
	);
}

function ContactDetail({ kind }: { kind: KindOf<"contact"> }) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{kind.name}</div>
			<DetailField label="Email" value={kind.email} />
			<DetailField label="Phone" value={kind.phone} />
			<KindTag label="Contact" />
		</div>
	);
}

function GitRepoDetail({
	item,
	kind,
}: {
	item: LauncherItem;
	kind: KindOf<"gitRepo">;
}) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			<DetailField label="Path" value={kind.path} />
			<KindTag label="Git Repository" />
		</div>
	);
}

function SshHostDetail({ kind }: { kind: KindOf<"sshHost"> }) {
	const sshCmd = kind.user
		? `ssh ${kind.user}@${kind.host}`
		: `ssh ${kind.host}`;
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{kind.host}</div>
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
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			{item.subtitle && (
				<DetailField label="Description" value={item.subtitle} />
			)}
			<DetailField label="Action" value={kind.action} />
			<KindTag label="System Command" />
		</div>
	);
}

function DefaultDetail({ item }: { item: LauncherItem }) {
	return (
		<div className="lc-detail-block">
			<div className="lc-detail-name">{item.title}</div>
			{item.subtitle && <DetailField label="Info" value={item.subtitle} />}
			<KindTag label={item.kind.type} />
		</div>
	);
}
