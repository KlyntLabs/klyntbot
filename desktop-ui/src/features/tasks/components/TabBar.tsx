import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { horizontalListSortingStrategy, SortableContext, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Area, Project } from "@shared/types/tasks";
import type React from "react";
import { useTabStore } from "../store/tab-store";
import { AddTabMenu } from "./AddTabMenu";
import { TabContextMenu } from "./TabContextMenu";
import { TabPill } from "./TabPill";

function SortableTab({ tabId, children }: { tabId: string; children: React.ReactNode }) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id: tabId });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div ref={setNodeRef} style={style} {...attributes} {...listeners}>
      {children}
    </div>
  );
}

interface TabBarProps {
  areas: Area[];
  projects: Project[];
}

export function TabBar({ areas, projects }: TabBarProps) {
  const tabs = useTabStore((s) => s.tabs);
  const activeTabId = useTabStore((s) => s.activeTabId);
  const reorderTabs = useTabStore((s) => s.reorderTabs);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 8 } }));

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const fromIndex = tabs.findIndex((t) => t.id === active.id);
    const toIndex = tabs.findIndex((t) => t.id === over.id);
    if (fromIndex !== -1 && toIndex !== -1) {
      reorderTabs(fromIndex, toIndex);
    }
  };

  return (
    <div className="flex items-end gap-0.5 px-2 pt-1.5 border-b border-[hsl(var(--border))] bg-[hsl(var(--background))] overflow-x-auto">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={tabs.map((t) => t.id)} strategy={horizontalListSortingStrategy}>
          {tabs.map((tab) => (
            <SortableTab key={tab.id} tabId={tab.id}>
              <TabContextMenu tabId={tab.id}>
                <TabPill tab={tab} isActive={tab.id === activeTabId} />
              </TabContextMenu>
            </SortableTab>
          ))}
        </SortableContext>
      </DndContext>
      <AddTabMenu areas={areas} projects={projects} />
    </div>
  );
}
