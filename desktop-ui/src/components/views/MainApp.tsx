import { useState, useMemo, useCallback, useRef } from 'react';
import { useNavigate } from 'react-router';
import { MessageSquare } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Toolbar } from '../tasks/Toolbar';
import { TaskTable } from '../tasks/TaskTable';
import { SidebarChat } from '../chat/SidebarChat';
import { OkrView } from './OkrView';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useEvent } from '../../hooks/useEvent';
import { taskGridCols } from '../../lib/utils';
import type { Task, Project, Objective, Tab, SidebarItem, ViewMode, TaskUpdateParams } from '../../lib/types';

export function MainApp() {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<Tab>('All');
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [isChatOpen, setIsChatOpen] = useState(false);
  const prevSidebarRef = useRef<SidebarItem>('Tasks');
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [collapsedProjects, toggleProject] = useSetToggle();

  const { data: tasks, refetch: refetchTasks } = useQuery<Task[]>('task_list', undefined, []);
  const { data: projects, refetch: refetchProjects } = useQuery<Project[]>('project_list', undefined, []);
  const { data: objectives } = useQuery<Objective[]>('objective_list', undefined, []);

  const toggleComplete = useMutation<Task, { id: string }>('task_toggle_complete');
  const updateTask = useMutation<Task, TaskUpdateParams>('task_update');
  const createTask = useMutation<Task, { title: string }>('task_create');

  const [completedTasks, toggleTask] = useSetToggle(
    tasks.filter(t => t.completed).map(t => t.id)
  );

  // Inline add task state
  const [addingTask, setAddingTask] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState('');

  const handleToggleTask = useCallback(async (taskId: string) => {
    toggleTask(taskId);
    await toggleComplete.mutate({ id: taskId });
  }, [toggleTask, toggleComplete]);

  const handleUpdatePriority = useCallback(async (taskId: string, priority: number | null) => {
    await updateTask.mutate({ id: taskId, priority });
  }, [updateTask]);

  const handleRenameTask = useCallback(async (taskId: string, title: string) => {
    await updateTask.mutate({ id: taskId, title });
  }, [updateTask]);

  const handleAddTask = useCallback(async () => {
    if (!newTaskTitle.trim()) return;
    await createTask.mutate({ title: newTaskTitle.trim() });
    setNewTaskTitle('');
    setAddingTask(false);
  }, [newTaskTitle, createTask]);

  // Auto-refresh when relevant entities change
  useEvent<{ entityKind: string; id: string }>('entity:updated', (payload) => {
    const kind = payload?.entityKind;
    if (!kind || kind === 'task' || kind === 'area') refetchTasks();
    if (!kind || kind === 'project' || kind === 'objective') refetchProjects();
  });

  // Listen for open-chat events from the tray
  useEvent<{ text: string }>('open-chat', () => {
    setIsChatOpen(true);
    setActiveSidebar('Chat');
  });

  const filteredTasks = useMemo(() =>
    activeTab === 'All' ? tasks : tasks.filter(task => task.areaId === activeTab.toLowerCase()),
  [activeTab, tasks]);

  const showArea = activeTab === 'All';

  // Content view is independent of chat state — if chat is open, show the view
  // that was active before chat opened.
  const contentView = activeSidebar === 'Chat' ? prevSidebarRef.current : activeSidebar;

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item === 'Finance') navigate('/finance');
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
            <div className="mb-2 bg-surface-low rounded-xl overflow-hidden">
              <div className={`grid ${taskGridCols(showArea)} gap-4 px-6 py-3`}>
                <div />
                <div className="flex items-center">
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
              </div>
            </div>
          )}

          <TaskTable
            tasks={filteredTasks}
            projects={projects}
            objectives={objectives}
            activeTab={activeTab}
            completedTasks={completedTasks}
            collapsedProjects={collapsedProjects}
            onToggleTask={handleToggleTask}
            onToggleProject={toggleProject}
            onUpdatePriority={handleUpdatePriority}
            onRenameTask={handleRenameTask}
          />
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
      <SidebarChat isOpen={isChatOpen} onClose={() => setIsChatOpen(false)} />
    </div>
  );
}
