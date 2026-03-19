// desktop-ui/src/features/projects/components/GlassTabBar.tsx

import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  arrayMove,
  horizontalListSortingStrategy,
  SortableContext,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useNavigate } from "react-router";

export interface TabDef {
  id: string;
  label: string;
  badge?: string | number;
  indicatorColor?: string;
}

interface GlassTabBarProps {
  tabs: TabDef[];
  activeTab: string;
  basePath: string;
  onReorder: (newOrder: string[]) => void;
}

function SortableTab({
  tab,
  isActive,
  basePath,
}: {
  tab: TabDef;
  isActive: boolean;
  basePath: string;
}) {
  const navigate = useNavigate();
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id: tab.id });
  const style = { transform: CSS.Transform.toString(transform), transition };

  const path = tab.id === "overview" ? basePath : `${basePath}/${tab.id}`;

  return (
    <button
      ref={setNodeRef}
      style={style}
      type="button"
      onClick={() => navigate(path)}
      className={`flex items-center gap-1.5 px-4 py-2.5 text-xs font-medium transition-colors border-b-2 -mb-px ${
        isActive
          ? "border-brand text-foreground"
          : "border-transparent text-muted-foreground hover:text-foreground"
      }`}
      {...attributes}
      {...listeners}
    >
      {tab.indicatorColor && (
        <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: tab.indicatorColor }} />
      )}
      {tab.label}
      {tab.badge != null && (
        <span className="glass-badge px-1.5 py-0.5 text-[10px] text-muted-foreground font-light">
          {tab.badge}
        </span>
      )}
    </button>
  );
}

export function GlassTabBar({ tabs, activeTab, basePath, onReorder }: GlassTabBarProps) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 8 } }));

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = tabs.findIndex((t) => t.id === active.id);
    const newIndex = tabs.findIndex((t) => t.id === over.id);
    const newOrder = arrayMove(
      tabs.map((t) => t.id),
      oldIndex,
      newIndex,
    );
    onReorder(newOrder);
  }

  return (
    <div className="flex gap-0.5 px-6 glass-toolbar border-b border-border">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={tabs.map((t) => t.id)} strategy={horizontalListSortingStrategy}>
          {tabs.map((tab) => (
            <SortableTab
              key={tab.id}
              tab={tab}
              isActive={tab.id === activeTab}
              basePath={basePath}
            />
          ))}
        </SortableContext>
      </DndContext>
    </div>
  );
}
