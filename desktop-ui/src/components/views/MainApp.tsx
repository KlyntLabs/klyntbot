import { useState, useMemo, useCallback } from 'react';
import { MessageSquare } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Toolbar } from '../tasks/Toolbar';
import { TaskTable } from '../tasks/TaskTable';
import { ChatPanel } from '../chat/ChatPanel';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useEvent } from '../../hooks/useEvent';
import { mockProjects, mockTasks, mockObjectives } from '../../data/mockData';
import type { Task, Project, Objective, Tab, SidebarItem, ViewMode } from '../../lib/types';

export function MainApp() {
  const [activeTab, setActiveTab] = useState<Tab>('All');
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [collapsedProjects, toggleProject] = useSetToggle();

  const { data: tasks, refetch: refetchTasks } = useQuery<Task[]>('task_list', undefined, mockTasks);
  const { data: projects, refetch: refetchProjects } = useQuery<Project[]>('project_list', undefined, mockProjects);
  const { data: objectives } = useQuery<Objective[]>('objective_list', undefined, mockObjectives);

  const toggleComplete = useMutation<Task, { id: string }>('task_toggle_complete');

  const [completedTasks, toggleTask] = useSetToggle(
    tasks.filter(t => t.completed).map(t => t.id)
  );

  const handleToggleTask = useCallback(async (taskId: string) => {
    toggleTask(taskId);
    await toggleComplete.mutate({ id: taskId });
  }, [toggleTask, toggleComplete]);

  // Auto-refresh when entities change (e.g. after task_toggle_complete emits event)
  useEvent<{ entityKind: string; id: string }>('entity:updated', () => {
    refetchTasks();
    refetchProjects();
  });

  const filteredTasks = useMemo(() =>
    activeTab === 'All' ? tasks : tasks.filter(task => task.areaId === activeTab.toLowerCase()),
  [activeTab, tasks]);

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
              setIsChatOpen(nextOpen);
              setActiveSidebar(nextOpen ? 'Chat' : 'Tasks');
            }}
            className="w-9 h-9 rounded-md flex items-center justify-center transition-colors bg-surface-low text-muted hover:bg-surface-base hover:text-secondary ml-2"
          >
            <MessageSquare className="w-[18px] h-[18px]" strokeWidth={1.5} />
          </button>
        </div>

        {/* Scrollable Content */}
        <div className="flex-1 overflow-y-auto p-2">
          <Toolbar viewMode={viewMode} onViewModeChange={setViewMode} />
          <TaskTable
            tasks={filteredTasks}
            projects={projects}
            objectives={objectives}
            activeTab={activeTab}
            completedTasks={completedTasks}
            collapsedProjects={collapsedProjects}
            onToggleTask={handleToggleTask}
            onToggleProject={toggleProject}
          />
        </div>
      </div>

      {/* Chat Panel */}
      <ChatPanel isOpen={isChatOpen} onClose={() => setIsChatOpen(false)} />
    </div>
  );
}
