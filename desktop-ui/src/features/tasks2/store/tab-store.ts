import { arrayMove } from "@dnd-kit/sortable";
import { create } from "zustand";

export interface NavEntry {
  type: "my-issues" | "all-issues" | "area" | "project" | "issue";
  targetId: string;
  label: string;
}

export interface Tab {
  id: string;
  navStack: NavEntry[];
}

interface TabState {
  tabs: Tab[];
  activeTabId: string;
  initialized: boolean;

  initFromAreas: (areas: { id: string; name: string }[]) => void;
  openTab: (type: NavEntry["type"], targetId: string, label: string) => void;
  closeTab: (tabId: string) => void;
  closeOthers: (tabId: string) => void;
  closeToRight: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;
  navigateInPlace: (type: NavEntry["type"], targetId: string, label: string) => void;
  navigateToStackIndex: (index: number) => void;
  reorderTabs: (fromIndex: number, toIndex: number) => void;
}

let idCounter = 0;
function nextId() {
  return `tab-${++idCounter}`;
}

const defaultMyIssuesTab: Tab = {
  id: nextId(),
  navStack: [{ type: "my-issues", targetId: "my-issues", label: "My Issues" }],
};

export const useTabStore = create<TabState>((set, get) => ({
  tabs: [defaultMyIssuesTab],
  activeTabId: defaultMyIssuesTab.id,
  initialized: false,

  initFromAreas: (areas) => {
    if (get().initialized) return;

    const myIssuesTab: Tab = {
      id: nextId(),
      navStack: [{ type: "my-issues", targetId: "my-issues", label: "My Issues" }],
    };

    const areaTabs: Tab[] = areas.map((area) => ({
      id: nextId(),
      navStack: [{ type: "area" as const, targetId: area.id, label: area.name }],
    }));

    const tabs = [myIssuesTab, ...areaTabs];
    set({ tabs, activeTabId: tabs[0].id, initialized: true });
  },

  openTab: (type, targetId, label) => {
    const { tabs, activeTabId } = get();

    // Deduplicate: if tab with same root type+targetId exists, switch to it
    const existing = tabs.find(
      (t) =>
        t.navStack.length > 0 && t.navStack[0].type === type && t.navStack[0].targetId === targetId,
    );
    if (existing) {
      set({ activeTabId: existing.id });
      return;
    }

    const newTab: Tab = {
      id: nextId(),
      navStack: [{ type, targetId, label }],
    };

    // Insert after active tab
    const activeIndex = tabs.findIndex((t) => t.id === activeTabId);
    const insertIndex = activeIndex >= 0 ? activeIndex + 1 : tabs.length;
    const newTabs = [...tabs.slice(0, insertIndex), newTab, ...tabs.slice(insertIndex)];

    set({ tabs: newTabs, activeTabId: newTab.id });
  },

  closeTab: (tabId) => {
    const { tabs, activeTabId } = get();
    const index = tabs.findIndex((t) => t.id === tabId);
    if (index === -1) return;

    const newTabs = tabs.filter((t) => t.id !== tabId);
    if (newTabs.length === 0) {
      set({ tabs: [], activeTabId: "" });
      return;
    }

    let newActiveId = activeTabId;
    if (activeTabId === tabId) {
      const newIndex = Math.min(index, newTabs.length - 1);
      newActiveId = newTabs[newIndex].id;
    }

    set({ tabs: newTabs, activeTabId: newActiveId });
  },

  closeOthers: (tabId) => {
    const { tabs } = get();
    const kept = tabs.find((t) => t.id === tabId);
    if (!kept) return;
    set({ tabs: [kept], activeTabId: tabId });
  },

  closeToRight: (tabId) => {
    const { tabs, activeTabId } = get();
    const index = tabs.findIndex((t) => t.id === tabId);
    if (index === -1) return;
    const newTabs = tabs.slice(0, index + 1);
    if (newTabs.length === 0) {
      set({ tabs: [], activeTabId: "" });
      return;
    }
    const newActive = newTabs.find((t) => t.id === activeTabId)
      ? activeTabId
      : newTabs[newTabs.length - 1].id;
    set({ tabs: newTabs, activeTabId: newActive });
  },

  setActiveTab: (tabId) => {
    set({ activeTabId: tabId });
  },

  navigateInPlace: (type, targetId, label) => {
    const { tabs, activeTabId } = get();
    const idx = tabs.findIndex((t) => t.id === activeTabId);
    if (idx === -1) return;
    const updated = [...tabs];
    updated[idx] = { ...tabs[idx], navStack: [...tabs[idx].navStack, { type, targetId, label }] };
    set({ tabs: updated });
  },

  navigateToStackIndex: (index) => {
    const { tabs, activeTabId } = get();
    const idx = tabs.findIndex((t) => t.id === activeTabId);
    if (idx === -1) return;
    if (index < 0 || index >= tabs[idx].navStack.length) return;
    const updated = [...tabs];
    updated[idx] = { ...tabs[idx], navStack: tabs[idx].navStack.slice(0, index + 1) };
    set({ tabs: updated });
  },

  reorderTabs: (fromIndex, toIndex) => {
    const { tabs } = get();
    if (fromIndex < 0 || fromIndex >= tabs.length || toIndex < 0 || toIndex >= tabs.length) return;
    set({ tabs: arrayMove(tabs, fromIndex, toIndex) });
  },
}));
