import { Dialog } from "@shared/composites/Dialog/Dialog";

interface ShortcutGroup {
  title: string;
  shortcuts: { keys: string; description: string }[];
}

const SHORTCUT_GROUPS: ShortcutGroup[] = [
  {
    title: "General",
    shortcuts: [
      { keys: "?", description: "Show this help" },
      { keys: "⌘ N", description: "New note dialog" },
      { keys: "⌘ ⇧ N", description: "Create blank note" },
      { keys: "⌘ S", description: "Force save" },
      { keys: "⌘ F", description: "Find notes" },
      { keys: "⌘ ⌫", description: "Delete selected note" },
      { keys: "Esc", description: "Close panel / dialog" },
    ],
  },
  {
    title: "View & Layout",
    shortcuts: [
      { keys: "⌘ ⇧ ⏎", description: "Toggle focus mode" },
      { keys: "⌘ ⇧ G", description: "Toggle editor / graph" },
      { keys: "⌘ ⇧ H", description: "Toggle version history" },
      { keys: "⌘ ⇧ I", description: "Toggle Insight Review" },
    ],
  },
  {
    title: "Selection Actions",
    shortcuts: [
      { keys: "⌥ A", description: "Annotate selection" },
      { keys: "⌥ F", description: "Create flashcard" },
      { keys: "⌥ L", description: "Linked view" },
      { keys: "⌘ L", description: "Insert AI-suggested link" },
    ],
  },
  {
    title: "Editor",
    shortcuts: [
      { keys: "/", description: "Slash commands menu" },
      { keys: "[  [", description: "Wiki-link autocomplete" },
      { keys: "@", description: "Entity mention" },
      { keys: "Enter / Tab", description: "Confirm title → focus body" },
    ],
  },
  {
    title: "Autocomplete Menus",
    shortcuts: [
      { keys: "↑ ↓", description: "Navigate items" },
      { keys: "Enter / Tab", description: "Select item" },
      { keys: "Esc", description: "Close menu" },
    ],
  },
];

interface ShortcutHelpDialogProps {
  open: boolean;
  onClose: () => void;
}

export function ShortcutHelpDialog({ open, onClose }: ShortcutHelpDialogProps) {
  return (
    <Dialog open={open} onClose={onClose} title="Keyboard Shortcuts" size="lg">
      <div className="grid grid-cols-2 gap-x-8 gap-y-5 max-h-[60vh] overflow-y-auto pr-1">
        {SHORTCUT_GROUPS.map((group) => (
          <div key={group.title}>
            <h4 className="text-ui-xs font-medium text-brand uppercase tracking-wider mb-2">
              {group.title}
            </h4>
            <div className="space-y-1">
              {group.shortcuts.map((s) => (
                <div key={s.keys} className="flex items-center justify-between gap-3">
                  <span className="text-ui-sm text-fg-secondary">{s.description}</span>
                  <kbd className="shrink-0 text-ui-xs font-mono px-1.5 py-0.5 rounded bg-control-hover/50 border border-separator text-fg/70">
                    {s.keys}
                  </kbd>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </Dialog>
  );
}
