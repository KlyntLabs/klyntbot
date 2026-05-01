import { type QueryKey, useQueryClient } from "@tanstack/react-query";
import { useCallback, useRef, useState } from "react";
import { taskUpdate } from "@/api/endpoints/dashboard";
import type { TaskResponse, TaskUpdateParams, TimelineResponse } from "@/bindings";
import { useTauriMutation } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { minutesToIso } from "@/utils/dashboardDates";

const SNAP_MINUTES = 15;

interface DragState {
  taskId: string;
  mode: "move" | "resize" | "tray";
  originTopMin: number;
  originEndMin: number;
  startMouseY: number;
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

function optimisticUpdateTimeline(vars: TaskUpdateParams, prev: unknown): TimelineResponse {
  const timeline = prev as TimelineResponse;
  if (!timeline?.entries) return timeline;
  const newStart = vars.scheduledStart;
  const newEnd = vars.scheduledEnd;
  if (!newStart || !newEnd) return timeline;
  const newDuration = Math.round(
    (new Date(newEnd).getTime() - new Date(newStart).getTime()) / 1000,
  );
  return {
    ...timeline,
    entries: timeline.entries.map((entry) => {
      if (entry.entityId !== vars.id) return entry;
      return {
        ...entry,
        startedAt: newStart,
        durationSecs: newDuration,
      };
    }),
  };
}

export function useTimelineDrag(date: string, pxPerMin: number, timelineQueryKey: QueryKey) {
  const [drag, setDrag] = useState<DragState | null>(null);
  const [ghost, setGhost] = useState<GhostPosition | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const ghostRef = useRef<GhostPosition | null>(null);
  const pxPerMinRef = useRef(pxPerMin);
  pxPerMinRef.current = pxPerMin;

  const queryClient = useQueryClient();
  const { mutate: updateTask } = useTauriMutation<TaskResponse, TaskUpdateParams>({
    mutationFn: taskUpdate,
    invalidates: [qk.dashboard.all()],
    optimistic: {
      queryKey: timelineQueryKey,
      update: optimisticUpdateTimeline,
    },
  });

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

  const onMouseMove = useCallback((e: MouseEvent) => {
    const d = dragRef.current;
    if (!d) return;

    const deltaY = e.clientY - d.startMouseY;
    const deltaMins = deltaY / pxPerMinRef.current;

    let newGhost: GhostPosition;
    if (d.mode === "move") {
      const newTop = snapToGrid(d.originTopMin + deltaMins);
      const duration = d.originEndMin - d.originTopMin;
      newGhost = { topMin: clampMinutes(newTop), endMin: clampMinutes(newTop + duration) };
    } else if (d.mode === "resize") {
      const newEnd = snapToGrid(d.originEndMin + deltaMins);
      newGhost = {
        topMin: d.originTopMin,
        endMin: clampMinutes(Math.max(d.originTopMin + SNAP_MINUTES, newEnd)),
      };
    } else {
      const duration = ghostRef.current ? ghostRef.current.endMin - ghostRef.current.topMin : 30;
      const rawTop = snapToGrid(d.originTopMin + deltaMins);
      newGhost = { topMin: clampMinutes(rawTop), endMin: clampMinutes(rawTop + duration) };
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
    dragRef.current = null;
    ghostRef.current = null;
    setDrag(null);
    setGhost(null);

    if (d.mode !== "tray" && g.topMin === d.originTopMin && g.endMin === d.originEndMin) return;
    if (d.mode === "tray" && d.originTopMin === -1) return;

    await updateTask({
      id: d.taskId,
      title: null,
      description: null,
      priority: null,
      status: null,
      dueDate: null,
      projectId: null,
      areaId: null,
      tags: null,
      keyResultId: null,
      statusLabelId: null,
      position: null,
      groupId: null,
      taskType: null,
      energyLevel: null,
      estimatedMinutes: null,
      scheduledStart: minutesToIso(dateRef.current, g.topMin),
      scheduledEnd: minutesToIso(dateRef.current, g.endMin),
    });

    void queryClient.invalidateQueries({ queryKey: qk.dashboard.all() });
  }, [updateTask, queryClient]);

  return { drag, ghost, isDragging, startMove, startResize, startTrayDrag, onMouseMove, onMouseUp };
}
