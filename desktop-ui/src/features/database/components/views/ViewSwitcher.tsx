import { useReorderViews } from "@features/database/hooks/useReorderViews";
import { useCreateView, useDeleteView, useUpdateView } from "@features/database/hooks/useViews";
import { defaultViewConfig, defaultViewName } from "@features/database/lib/view-defaults";
import { useClickOutside } from "@shared/hooks/useClickOutside";
import type { DatabaseSchema, ViewDefinition, ViewType } from "@shared/types";
import { MoreHorizontal, Plus } from "lucide-react";
import { useRef, useState } from "react";
import { ViewTypeIcon } from "./ViewTypeIcon";
import { ViewTypePicker } from "./ViewTypePicker";

interface ViewSwitcherProps {
  schema: DatabaseSchema;
  views: ViewDefinition[];
  activeViewId: string | undefined;
  onViewSelect: (viewId: string) => void;
  onConfigView?: (view: ViewDefinition) => void;
}

export function ViewSwitcher({
  schema,
  views,
  activeViewId,
  onViewSelect,
  onConfigView,
}: ViewSwitcherProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const pickerRef = useRef<HTMLDivElement>(null);

  const createView = useCreateView(schema.id);
  const deleteView = useDeleteView(schema.id);
  const reorderViews = useReorderViews(schema.id);
  const updateView = useUpdateView(schema.id);

  useClickOutside(pickerRef, () => setPickerOpen(false), pickerOpen);

  const handleCreate = async (type: ViewType) => {
    const name = defaultViewName(views, type);
    const config = defaultViewConfig(schema, type);
    const result = await createView.mutate(name, type, config);
    if (result) onViewSelect(result.id);
    setPickerOpen(false);
  };

  const handleMove = async (id: string, dir: -1 | 1) => {
    const ids = views.map((v) => v.id);
    const i = ids.indexOf(id);
    const j = i + dir;
    if (j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    await reorderViews.mutate(ids);
  };

  return (
    <div className="flex items-center gap-0.5">
      {views.map((view, idx) => (
        <ViewTab
          key={view.id}
          view={view}
          isActive={view.id === activeViewId}
          isRenaming={renamingId === view.id}
          canMoveLeft={idx > 0}
          canMoveRight={idx < views.length - 1}
          canDelete={views.length > 1}
          onSelect={() => onViewSelect(view.id)}
          onStartRename={() => setRenamingId(view.id)}
          onFinishRename={(name) => {
            if (name && name !== view.name) void updateView.mutate(view.id, { name });
            setRenamingId(null);
          }}
          onDuplicate={() => createView.mutate(`${view.name} copy`, view.viewType, view.config)}
          onDelete={() => deleteView.mutate(view.id)}
          onConfig={() => onConfigView?.(view)}
          onMoveLeft={() => handleMove(view.id, -1)}
          onMoveRight={() => handleMove(view.id, 1)}
        />
      ))}
      <div ref={pickerRef} className="relative">
        <button
          type="button"
          onClick={() => setPickerOpen((o) => !o)}
          className="flex items-center justify-center rounded-md p-1.5 text-foreground/55 hover:bg-accent hover:text-foreground transition-colors"
          aria-label="Add view"
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
        {pickerOpen && (
          <div className="absolute left-0 top-full z-50 mt-1 w-64 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg">
            <ViewTypePicker onSelect={handleCreate} />
          </div>
        )}
      </div>
    </div>
  );
}

interface ViewTabProps {
  view: ViewDefinition;
  isActive: boolean;
  isRenaming: boolean;
  canMoveLeft: boolean;
  canMoveRight: boolean;
  canDelete: boolean;
  onSelect: () => void;
  onStartRename: () => void;
  onFinishRename: (newName: string) => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onConfig: () => void;
  onMoveLeft: () => void;
  onMoveRight: () => void;
}

function ViewTab({
  view,
  isActive,
  isRenaming,
  canMoveLeft,
  canMoveRight,
  canDelete,
  onSelect,
  onStartRename,
  onFinishRename,
  onDuplicate,
  onDelete,
  onConfig,
  onMoveLeft,
  onMoveRight,
}: ViewTabProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const tabRef = useRef<HTMLDivElement>(null);
  useClickOutside(tabRef, () => setMenuOpen(false), menuOpen);

  return (
    <div
      ref={tabRef}
      className={`group relative flex items-center gap-1.5 px-2.5 py-2 text-[13px] transition-colors ${
        isActive ? "text-foreground font-semibold" : "text-foreground/60 hover:text-foreground"
      }`}
    >
      <ViewTypeIcon type={view.viewType} />
      {isRenaming ? (
        <RenameInput initial={view.name} onDone={onFinishRename} />
      ) : (
        <button
          type="button"
          onClick={onSelect}
          onDoubleClick={onStartRename}
          className="cursor-pointer"
        >
          {view.name}
        </button>
      )}
      {isActive && !isRenaming && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            setMenuOpen((o) => !o);
          }}
          className="rounded p-0.5 text-foreground/60 opacity-0 transition-opacity hover:bg-foreground/10 hover:text-foreground group-hover:opacity-100"
          aria-label="View actions"
        >
          <MoreHorizontal className="h-3.5 w-3.5" />
        </button>
      )}
      {isActive && (
        <span
          className="absolute inset-x-0 -bottom-px h-[2px] rounded-full bg-foreground"
          aria-hidden="true"
        />
      )}
      {menuOpen && (
        <div className="absolute left-0 top-full z-50 mt-1 w-44 rounded-md border border-border bg-popover text-popover-foreground py-1 shadow-lg">
          <MenuItem
            label="Rename"
            onClick={() => {
              onStartRename();
              setMenuOpen(false);
            }}
          />
          <MenuItem
            label="Edit properties"
            onClick={() => {
              onConfig();
              setMenuOpen(false);
            }}
          />
          <MenuItem
            label="Duplicate"
            onClick={() => {
              onDuplicate();
              setMenuOpen(false);
            }}
          />
          <MenuItem
            label="Move left"
            disabled={!canMoveLeft}
            onClick={() => {
              onMoveLeft();
              setMenuOpen(false);
            }}
          />
          <MenuItem
            label="Move right"
            disabled={!canMoveRight}
            onClick={() => {
              onMoveRight();
              setMenuOpen(false);
            }}
          />
          <div className="my-1 border-t border-border/60" />
          <MenuItem
            label="Delete"
            danger
            disabled={!canDelete}
            onClick={() => {
              onDelete();
              setMenuOpen(false);
            }}
          />
        </div>
      )}
    </div>
  );
}

function MenuItem({
  label,
  onClick,
  disabled,
  danger,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`w-full px-2.5 py-1 text-left text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
        danger
          ? "text-red-500 hover:bg-red-500/10"
          : "text-foreground/85 hover:bg-accent hover:text-foreground"
      }`}
    >
      {label}
    </button>
  );
}

function RenameInput({ initial, onDone }: { initial: string; onDone: (newName: string) => void }) {
  const [value, setValue] = useState(initial);
  return (
    <input
      type="text"
      autoFocus
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onBlur={() => onDone(value.trim())}
      onKeyDown={(e) => {
        if (e.key === "Enter") onDone(value.trim());
        else if (e.key === "Escape") onDone(initial);
      }}
      className="rounded border border-border bg-background px-1 py-0.5 text-[13px] outline-none"
      style={{ width: `${Math.max(value.length + 1, 6)}ch` }}
    />
  );
}
