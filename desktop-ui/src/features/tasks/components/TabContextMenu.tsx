import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@shared/ui/RadixContextMenu";
import type React from "react";
import { useTabStore } from "../store/tab-store";

interface TabContextMenuProps {
  tabId: string;
  children: React.ReactNode;
}

export function TabContextMenu({ tabId, children }: TabContextMenuProps) {
  const closeTab = useTabStore((s) => s.closeTab);
  const closeOthers = useTabStore((s) => s.closeOthers);
  const closeToRight = useTabStore((s) => s.closeToRight);

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => closeTab(tabId)}>Close</ContextMenuItem>
        <ContextMenuItem onSelect={() => closeOthers(tabId)}>Close Others</ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => closeToRight(tabId)}>
          Close Tabs to the Right
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
