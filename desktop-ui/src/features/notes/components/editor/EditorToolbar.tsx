import type { Editor } from "@tiptap/react";
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Bold,
  Code,
  Expand,
  GitGraph,
  Heading1,
  Heading2,
  Heading3,
  Highlighter,
  History,
  ImageIcon,
  Italic,
  Link,
  List,
  ListChecks,
  ListOrdered,
  Minus,
  Quote,
  Sparkles,
  Strikethrough,
  Table,
  Underline as UnderlineIcon,
} from "lucide-react";
import { VimStatusLine } from "./VimStatusLine";
import type { VimMode } from "./vim";

interface EditorToolbarProps {
  editor: Editor | null;
  vimEnabled: boolean;
  vimMode: VimMode;
  onToggleVim: () => void;
  onOpenLinkDialog?: () => void;
  onOpenImageDialog?: () => void;
  onGenerateCards?: (selectedText?: string) => void;
  onToggleFocusMode?: () => void;
  onToggleGraphMode?: () => void;
  onToggleVersionHistory?: () => void;
  focusModeActive?: boolean;
  graphModeActive?: boolean;
  versionHistoryActive?: boolean;
}

interface ToolbarButton {
  icon: typeof Bold;
  label: string;
  action: (editor: Editor) => void;
  isActive?: (editor: Editor) => boolean;
}

function buildGroups(callbacks?: {
  onOpenLinkDialog?: () => void;
  onOpenImageDialog?: () => void;
}): ToolbarButton[][] {
  return [
    [
      {
        icon: Bold,
        label: "Bold",
        action: (e) => e.chain().focus().toggleBold().run(),
        isActive: (e) => e.isActive("bold"),
      },
      {
        icon: Italic,
        label: "Italic",
        action: (e) => e.chain().focus().toggleItalic().run(),
        isActive: (e) => e.isActive("italic"),
      },
      {
        icon: UnderlineIcon,
        label: "Underline",
        action: (e) => e.chain().focus().toggleUnderline().run(),
        isActive: (e) => e.isActive("underline"),
      },
      {
        icon: Strikethrough,
        label: "Strikethrough",
        action: (e) => e.chain().focus().toggleStrike().run(),
        isActive: (e) => e.isActive("strike"),
      },
      {
        icon: Code,
        label: "Inline code",
        action: (e) => e.chain().focus().toggleCode().run(),
        isActive: (e) => e.isActive("code"),
      },
      {
        icon: Highlighter,
        label: "Highlight",
        action: (e) => e.chain().focus().toggleHighlight().run(),
        isActive: (e) => e.isActive("highlight"),
      },
    ],
    [
      {
        icon: Heading1,
        label: "Heading 1",
        action: (e) => e.chain().focus().toggleHeading({ level: 1 }).run(),
        isActive: (e) => e.isActive("heading", { level: 1 }),
      },
      {
        icon: Heading2,
        label: "Heading 2",
        action: (e) => e.chain().focus().toggleHeading({ level: 2 }).run(),
        isActive: (e) => e.isActive("heading", { level: 2 }),
      },
      {
        icon: Heading3,
        label: "Heading 3",
        action: (e) => e.chain().focus().toggleHeading({ level: 3 }).run(),
        isActive: (e) => e.isActive("heading", { level: 3 }),
      },
    ],
    [
      {
        icon: List,
        label: "Bullet list",
        action: (e) => e.chain().focus().toggleBulletList().run(),
        isActive: (e) => e.isActive("bulletList"),
      },
      {
        icon: ListOrdered,
        label: "Ordered list",
        action: (e) => e.chain().focus().toggleOrderedList().run(),
        isActive: (e) => e.isActive("orderedList"),
      },
      {
        icon: ListChecks,
        label: "Task list",
        action: (e) => e.chain().focus().toggleTaskList().run(),
        isActive: (e) => e.isActive("taskList"),
      },
      {
        icon: Quote,
        label: "Blockquote",
        action: (e) => e.chain().focus().toggleBlockquote().run(),
        isActive: (e) => e.isActive("blockquote"),
      },
      {
        icon: Minus,
        label: "Horizontal rule",
        action: (e) => e.chain().focus().setHorizontalRule().run(),
      },
    ],
    [
      {
        icon: Table,
        label: "Insert table",
        action: (e) =>
          e.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(),
      },
      {
        icon: Link,
        label: "Link",
        action: (e) => {
          if (e.isActive("link")) {
            e.chain().focus().unsetLink().run();
            return;
          }
          callbacks?.onOpenLinkDialog?.();
        },
        isActive: (e) => e.isActive("link"),
      },
      {
        icon: ImageIcon,
        label: "Image",
        action: () => {
          callbacks?.onOpenImageDialog?.();
        },
      },
    ],
    [
      {
        icon: AlignLeft,
        label: "Align left",
        action: (e) => e.chain().focus().setTextAlign("left").run(),
        isActive: (e) => e.isActive({ textAlign: "left" }),
      },
      {
        icon: AlignCenter,
        label: "Align center",
        action: (e) => e.chain().focus().setTextAlign("center").run(),
        isActive: (e) => e.isActive({ textAlign: "center" }),
      },
      {
        icon: AlignRight,
        label: "Align right",
        action: (e) => e.chain().focus().setTextAlign("right").run(),
        isActive: (e) => e.isActive({ textAlign: "right" }),
      },
    ],
  ];
}

function VimToggleButton({
  vimEnabled,
  onToggleVim,
}: {
  vimEnabled: boolean;
  onToggleVim: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onToggleVim}
      title={vimEnabled ? "Disable Vim mode" : "Enable Vim mode"}
      className={`px-2 h-7 rounded-lg font-mono text-xs font-bold transition-all ${
        vimEnabled
          ? "bg-brand/15 text-brand shadow-[0_0_8px_rgba(249,115,22,0.12)]"
          : "text-muted-foreground hover:text-foreground hover:bg-muted"
      }`}
    >
      Vi
    </button>
  );
}

export function EditorToolbar({
  editor,
  vimEnabled,
  vimMode,
  onToggleVim,
  onOpenLinkDialog,
  onOpenImageDialog,
  onGenerateCards,
  onToggleFocusMode,
  onToggleGraphMode,
  onToggleVersionHistory,
  focusModeActive,
  graphModeActive,
  versionHistoryActive,
}: EditorToolbarProps) {
  if (!editor) return null;

  const modeButtons = (
    <div className="flex items-center gap-0.5 ml-auto">
      {onGenerateCards && (
        <button
          type="button"
          onClick={() => {
            const { from, to } = editor.state.selection;
            const selectedText =
              from !== to ? editor.state.doc.textBetween(from, to, "\n") : undefined;
            onGenerateCards(selectedText);
          }}
          title="Generate flashcards from note (or selection)"
          className="p-1.5 rounded-lg transition-all text-dim hover:text-muted-foreground hover:bg-card"
        >
          <Sparkles className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      )}
      {onToggleFocusMode && (
        <button
          type="button"
          onClick={onToggleFocusMode}
          title="Focus mode"
          className={`p-1.5 rounded-lg transition-all ${
            focusModeActive
              ? "bg-muted text-foreground"
              : "text-dim hover:text-muted-foreground hover:bg-card"
          }`}
        >
          <Expand className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      )}
      {onToggleGraphMode && (
        <button
          type="button"
          onClick={onToggleGraphMode}
          title="Graph mode"
          className={`p-1.5 rounded-lg transition-all ${
            graphModeActive
              ? "bg-muted text-foreground"
              : "text-dim hover:text-muted-foreground hover:bg-card"
          }`}
        >
          <GitGraph className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      )}
      {onToggleVersionHistory && (
        <button
          type="button"
          onClick={onToggleVersionHistory}
          title="Version history"
          className={`p-1.5 rounded-lg transition-all ${
            versionHistoryActive
              ? "bg-muted text-foreground"
              : "text-dim hover:text-muted-foreground hover:bg-card"
          }`}
        >
          <History className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      )}
    </div>
  );

  if (vimEnabled) {
    return (
      <div className="glass-toolbar rounded-lg px-2 py-1 flex items-center justify-between">
        <VimStatusLine mode={vimMode} />
        <div className="flex items-center gap-1">
          <VimToggleButton vimEnabled={vimEnabled} onToggleVim={onToggleVim} />
          {modeButtons}
        </div>
      </div>
    );
  }

  const groups = buildGroups({ onOpenLinkDialog, onOpenImageDialog });

  return (
    <div className="glass-toolbar rounded-lg px-2 py-1 flex items-center gap-0.5 flex-wrap">
      {groups.map((group, gi) => (
        <div key={gi} className="flex items-center gap-0.5">
          {gi > 0 && <div className="w-px h-4 bg-muted mx-1.5" />}
          {group.map((btn) => {
            const Icon = btn.icon;
            const active = btn.isActive?.(editor) ?? false;
            return (
              <button
                key={btn.label}
                type="button"
                onClick={() => btn.action(editor)}
                title={btn.label}
                className={`w-7 h-7 rounded-lg flex items-center justify-center transition-all ${
                  active
                    ? "bg-brand/15 text-brand shadow-[0_0_8px_rgba(249,115,22,0.12)]"
                    : "text-muted-foreground hover:text-foreground hover:bg-muted"
                }`}
              >
                <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
              </button>
            );
          })}
        </div>
      ))}
      <div className="w-px h-4 bg-muted mx-1.5" />
      <VimToggleButton vimEnabled={vimEnabled} onToggleVim={onToggleVim} />
      {modeButtons}
    </div>
  );
}
