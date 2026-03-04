import { useState, useMemo, useCallback, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { MessageSquare } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Toolbar } from '../tasks/Toolbar';
import { TaskTable } from '../tasks/TaskTable';
import { KanbanBoard } from '../tasks/KanbanBoard';
import { SidebarChat } from '../chat/SidebarChat';
import { OkrView } from './OkrView';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useSubtasks } from '../../hooks/useSubtasks';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useEvent } from '../../hooks/useEvent';
import type { Task, Project, Objective, Area, Tab, SidebarItem, ViewMode, TaskUpdateParams, TaskCreateParams } from '../../lib/types';

export function MainApp() {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<Tab>('All');
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [isChatOpen, setIsChatOpen] = useState(false);
  const prevSidebarRef = useRef<SidebarItem>('Tasks');
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [collapsedProjects, toggleProject] = useSetToggle();
  const { expandedTasks, childrenCache, generation, toggleExpand: toggleExpandTask, fetchChildren, invalidateCache } = useSubtasks();

  const { data: tasks, refetch: refetchTasks } = useQuery<Task[]>('task_list', undefined, []);
  const { data: projects, refetch: refetchProjects } = useQuery<Project[]>('project_list', undefined, []);
  const { data: objectives } = useQuery<Objective[]>('objective_list', undefined, []);
  const { data: areas, refetch: refetchAreas } = useQuery<Area[]>('area_list', undefined, []);

  const areaMap = useMemo(() => new Map(areas.map(a => [a.id, a])), [areas]);
  const projectMap = useMemo(() => new Map(projects.map(p => [p.id, p])), [projects]);

  const toggleComplete = useMutation<Task, { id: string }>('task_toggle_complete');
  const updateTask = useMutation<Task, TaskUpdateParams>('task_update', 'params');
  const createTask = useMutation<Task, TaskCreateParams>('task_create', 'params');

  const [completedTasks, toggleTask] = useSetToggle(
    tasks.filter(t => t.completed).map(t => t.id)
  );

  // Auto-fetch children for expanded tasks.
  // Uses `generation` (bumped on invalidation) instead of `childrenCache`
  // to avoid O(N²) re-fetches — each cache write would otherwise re-trigger
  // the loop for all expanded tasks.
  useEffect(() => {
    for (const taskId of expandedTasks) {
      fetchChildren(taskId);
    }
  }, [expandedTasks, generation, fetchChildren]);

  // Inline add task state
  const [addingTask, setAddingTask] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState('');

  const handleToggleTask = useCallback(async (taskId: string) => {
    toggleTask(taskId);
    await toggleComplete.mutate({ id: taskId });
  }, [toggleTask, toggleComplete]);

  const handleUpdateTask = useCallback(async (params: TaskUpdateParams) => {
    await updateTask.mutate(params);
  }, [updateTask]);

  const handleAddTask = useCallback(async () => {
    if (!newTaskTitle.trim()) return;
    await createTask.mutate({ title: newTaskTitle.trim() });
    setNewTaskTitle('');
    setAddingTask(false);
  }, [newTaskTitle, createTask]);

  const handleCreateSubtask = useCallback(async (parentId: string, title: string) => {
    await createTask.mutate({ title, parentId });
  }, [createTask]);

  // Auto-refresh when relevant entities change
  useEvent<{ entityKind: string; id: string }>('entity:updated', (payload) => {
    const kind = payload?.entityKind;
    if (!kind) { refetchTasks(); refetchProjects(); refetchAreas(); invalidateCache(); return; }
    if (kind === 'task' || kind === 'area') { refetchTasks(); invalidateCache(); }
    if (kind === 'project' || kind === 'objective') refetchProjects();
    if (kind === 'area') refetchAreas();
  });

  // Listen for open-chat events from the tray
  useEvent<{ text: string }>('open-chat', () => {
    setIsChatOpen(true);
    setActiveSidebar('Chat');
  });

  const filteredTasks = useMemo(() => {
    if (activeTab === 'All') return tasks;
    const tabLower = activeTab.toLowerCase();
    return tasks.filter(task => {
      const area = areaMap.get(task.areaId);
      return area?.name.toLowerCase() === tabLower;
    });
  }, [activeTab, tasks, areaMap]);

  // Content view is independent of chat state — if chat is open, show the view
  // that was active before chat opened.
  const contentView = activeSidebar === 'Chat' ? prevSidebarRef.current : activeSidebar;

  // Derive sidebar chat context from the current view + tab
  const sidebarViewContext = useMemo(() => {
    const section = contentView.toLowerCase();
    if (activeTab === 'All') return { entityKind: section };
    return { entityKind: `${section}.${activeTab.toLowerCase()}` };
  }, [contentView, activeTab]);

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item !== 'Chat') setIsChatOpen(false);
        }}
      />

      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header Tabs */}
        <div className="h-14 bg-background flex items-center px-2">
          <div className="flex-1 flex items-center gap-2">
            {(['All', 'Work', 'Personal'] as Tab[]).map(tab => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={`flex-1 py-2 rounded-md text-[13px] font-light transition-colors ${
                  activeTab === tab
                    ? 'bg-surface-highest text-white'
                    : 'bg-surface-low text-muted hover:bg-surface-base hover:text-secondary'
                }`}
              >
                {tab}
              </button>
            ))}
          </div>
          <button
            onClick={() => {
              const nextOpen = !isChatOpen;
              if (nextOpen) {
                prevSidebarRef.current = activeSidebar;
              }
              setIsChatOpen(nextOpen);
              setActiveSidebar(nextOpen ? 'Chat' : prevSidebarRef.current);
            }}
            className="w-9 h-9 rounded-md flex items-center justify-center transition-colors bg-surface-low text-muted hover:bg-surface-base hover:text-secondary ml-2"
          >
            <MessageSquare className="w-[18px] h-[18px]" strokeWidth={1.5} />
          </button>
        </div>

        {/* Scrollable Content */}
        <div className="flex-1 overflow-y-auto p-2">
          {contentView === 'OKR' ? (
            <OkrView />
          ) : (
          <>
          <Toolbar viewMode={viewMode} onViewModeChange={setViewMode} onAddTask={() => setAddingTask(true)} />

          {/* Inline Add Task Row */}
          {addingTask && (
            <div className="mb-2 bg-surface-low rounded-xl px-5 py-3">
              <input
                autoFocus
                value={newTaskTitle}
                onChange={e => setNewTaskTitle(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Enter') handleAddTask();
                  if (e.key === 'Escape') { setAddingTask(false); setNewTaskTitle(''); }
                }}
                onBlur={() => { if (!newTaskTitle.trim()) { setAddingTask(false); setNewTaskTitle(''); } }}
                placeholder="Task title... (Enter to save, Esc to cancel)"
                className="w-full bg-transparent text-[13px] font-light text-primary outline-none placeholder:text-dim"
              />
            </div>
          )}

          {viewMode === 'board' ? (
            <KanbanBoard
              tasks={filteredTasks}
              projectMap={projectMap}
              areaMap={areaMap}
              completedTasks={completedTasks}
            />
          ) : (
            <TaskTable
              tasks={filteredTasks}
              projectMap={projectMap}
              objectives={objectives}
              areaMap={areaMap}
              activeTab={activeTab}
              completedTasks={completedTasks}
              collapsedProjects={collapsedProjects}
              expandedTasks={expandedTasks}
              childrenCache={childrenCache}
              onToggleTask={handleToggleTask}
              onToggleProject={toggleProject}
              onToggleExpandTask={toggleExpandTask}
              onUpdate={handleUpdateTask}
              onCreateSubtask={handleCreateSubtask}
            />
          )}
          {filteredTasks.length === 0 && !addingTask && (
            <div className="flex flex-col items-center justify-center py-20 text-center">
              <p className="text-muted text-sm font-light">No tasks yet</p>
              <p className="text-dim text-xs font-light mt-1">Create a task to get started</p>
            </div>
          )}
          </>
          )}
        </div>
      </div>

      {/* Sidebar Chat */}
      <SidebarChat isOpen={isChatOpen} onClose={() => setIsChatOpen(false)} viewContext={sidebarViewContext} />
    </div>
  );
}
