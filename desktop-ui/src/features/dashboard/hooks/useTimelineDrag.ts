import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { minutesToIso } from "@shared/lib/dates";
import type { TaskUpdateParams } from "@shared/types";
import { useCallback, useRef, useState } from "react";

const SNAP_MINUTES = 15;

interface DragState {
  taskId: string;
  mode: "move" | "resize" | "tray";
  /** Pre-drag top in minutes (for rollback) */
  originTopMin: number;
  /** Pre-drag end in minutes (for rollback) */
  originEndMin: number;
  /** Mouse Y at drag start relative to timeline container */
  startMouseY: number;
  /** Offset from top of block to mouse position (for move mode) */
  offsetMin: number;
}

interface GhostPosition {
  topMin: number;
  endMin: number;
}

function snapToGrid(minutes: number): number {
  return Math.round(minutes / SNAP_MINUTES) * SNAP_MINUTES;
}

function clampMinutes(min: number): number {
  return Math.max(0, Math.min(1440, min));
}

export function useTimelineDrag(date: string, pxPerMin: number) {
  const [drag, setDrag] = useState<DragState | null>(null);
  const [ghost, setGhost] = useState<GhostPosition | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const ghostRef = useRef<GhostPosition | null>(null);
  const pxPerMinRef = useRef(pxPerMin);
  pxPerMinRef.current = pxPerMin;

  const { mutate: updateTask } = useMutation<void, TaskUpdateParams>("task_update", "params");

  const isDragging = drag !== null;

  const startMove = (e: React.MouseEvent, taskId: string, topMin: number, endMin: number) => {
    e.preventDefault();
    e.stopPropagation();
    const mouseMin = e.nativeEvent.offsetY / pxPerMinRef.current + topMin;
    const offsetMin = mouseMin - topMin;
    const state: DragState = {
      taskId,
      mode: "move",
      originTopMin: topMin,
      originEndMin: endMin,
      startMouseY: e.clientY,
      offsetMin,
    };
    dragRef.current = state;
    ghostRef.current = { topMin, endMin };
    setDrag(state);
    setGhost({ topMin, endMin });
  };

  const startResize = (e: React.MouseEvent, taskId: string, topMin: number, endMin: number) => {
    e.preventDefault();
    e.stopPropagation();
    const state: DragState = {
      taskId,
      mode: "resize",
      originTopMin: topMin,
      originEndMin: endMin,
      startMouseY: e.clientY,
      offsetMin: 0,
    };
    dragRef.current = state;
    ghostRef.current = { topMin, endMin };
    setDrag(state);
    setGhost({ topMin, endMin });
  };

  const startTrayDrag = (e: React.MouseEvent, taskId: string, estimatedMinutes: number) => {
    e.preventDefault();
    e.stopPropagation();
    const duration = estimatedMinutes ?? 30;
    const state: DragState = {
      taskId,
      mode: "tray",
      originTopMin: -1,
      originEndMin: -1,
      startMouseY: e.clientY,
      offsetMin: 0,
    };
    dragRef.current = state;
    ghostRef.current = { topMin: 0, endMin: duration };
    setDrag(state);
    setGhost({ topMin: 0, endMin: duration });
  };

  const dateRef = useRef(date);
  dateRef.current = date;

  // Stable identity — reads only from refs, no state closures
  const onMouseMove = useCallback((e: MouseEvent) => {
    const d = dragRef.current;
    if (!d) return;

    const deltaY = e.clientY - d.startMouseY;
    const deltaMins = deltaY / pxPerMinRef.current;

    let newGhost: GhostPosition;

    if (d.mode === "move") {
      const newTop = snapToGrid(d.originTopMin + deltaMins);
      const duration = d.originEndMin - d.originTopMin;
      newGhost = {
        topMin: clampMinutes(newTop),
        endMin: clampMinutes(newTop + duration),
      };
    } else if (d.mode === "resize") {
      const newEnd = snapToGrid(d.originEndMin + deltaMins);
      newGhost = {
        topMin: d.originTopMin,
        endMin: clampMinutes(Math.max(d.originTopMin + SNAP_MINUTES, newEnd)),
      };
    } else {
      // tray mode: position ghost based on mouse position
      const duration = ghostRef.current ? ghostRef.current.endMin - ghostRef.current.topMin : 30;
      const rawTop = snapToGrid(d.originTopMin + deltaMins);
      newGhost = {
        topMin: clampMinutes(rawTop),
        endMin: clampMinutes(rawTop + duration),
      };
    }

    ghostRef.current = newGhost;
    setGhost(newGhost);
  }, []);

  const onMouseUp = useCallback(async () => {
    const d = dragRef.current;
    const g = ghostRef.current;
    if (!d || !g) {
      dragRef.current = null;
      ghostRef.current = null;
      setDrag(null);
      setGhost(null);
      return;
    }

    // Clear drag state immediately for responsive UI
    dragRef.current = null;
    ghostRef.current = null;
    setDrag(null);
    setGhost(null);

    // Skip if nothing changed (move/resize back to original position)
    if (d.mode !== "tray" && g.topMin === d.originTopMin && g.endMin === d.originEndMin) {
      return;
    }

    // Skip tray drags that never moved
    if (d.mode === "tray" && d.originTopMin === -1) {
      return;
    }

    await updateTask({
      id: d.taskId,
      scheduledStart: minutesToIso(dateRef.current, g.topMin),
      scheduledEnd: minutesToIso(dateRef.current, g.endMin),
    });

    invalidateQueries("timeline_");
    invalidateQueries("task");
  }, [updateTask]);

  return {
    drag,
    ghost,
    isDragging,
    startMove,
    startResize,
    startTrayDrag,
    onMouseMove,
    onMouseUp,
  };
}
