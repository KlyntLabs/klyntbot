import { useState } from 'react';
import { MessageSquare } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Toolbar } from '../tasks/Toolbar';
import { TaskTable } from '../tasks/TaskTable';
import { ChatPanel } from '../chat/ChatPanel';
import { mockProjects, mockTasks, mockObjectives } from '../../data/mockData';
import type { Tab, SidebarItem, ViewMode } from '../../lib/types';

export function MainApp() {
  const [activeTab, setActiveTab] = useState<Tab>('All');
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [collapsedProjects, setCollapsedProjects] = useState<Set<string>>(new Set());
  const [completedTasks, setCompletedTasks] = useState<Set<string>>(
    new Set(mockTasks.filter(t => t.completed).map(t => t.id))
  );

  const filteredTasks = activeTab === 'All'
    ? mockTasks
    : mockTasks.filter(task => task.area === activeTab);

  const toggleTask = (taskId: string) => {
    setCompletedTasks(prev => {
      const next = new Set(prev);
      if (next.has(taskId)) next.delete(taskId);
      else next.add(taskId);
      return next;
    });
  };

  const toggleProject = (projectId: string) => {
    setCollapsedProjects(prev => {
      const next = new Set(prev);
      if (next.has(projectId)) next.delete(projectId);
      else next.add(projectId);
      return next;
    });
  };

  return (
    <div className="h-screen w-screen bg-[#0E0E0D] text-[#E6EDF3] flex overflow-hidden">
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
        <div className="h-14 bg-[#0E0E0D] flex items-center px-2">
          <div className="flex-1 flex items-center gap-2">
            {(['All', 'Work', 'Personal'] as Tab[]).map(tab => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={`flex-1 py-2 rounded-md text-[13px] font-light transition-colors ${
                  activeTab === tab
                    ? 'bg-[rgba(255,255,255,0.08)] text-white'
                    : 'bg-[rgba(255,255,255,0.03)] text-[#8B949E] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#C9D1D9]'
                }`}
              >
                {tab}
              </button>
            ))}
          </div>
          <button
            onClick={() => {
              setIsChatOpen(prev => !prev);
              setActiveSidebar(isChatOpen ? 'Tasks' : 'Chat');
            }}
            className="w-9 h-9 rounded-md flex items-center justify-center transition-colors bg-[rgba(255,255,255,0.03)] text-[#8B949E] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#C9D1D9] ml-2"
          >
            <MessageSquare className="w-[18px] h-[18px]" strokeWidth={1.5} />
          </button>
        </div>

        {/* Scrollable Content */}
        <div className="flex-1 overflow-y-auto p-2">
          <Toolbar viewMode={viewMode} onViewModeChange={setViewMode} />
          <TaskTable
            tasks={filteredTasks}
            projects={mockProjects}
            objectives={mockObjectives}
            activeTab={activeTab}
            completedTasks={completedTasks}
            collapsedProjects={collapsedProjects}
            onToggleTask={toggleTask}
            onToggleProject={toggleProject}
          />
        </div>
      </div>

      {/* Chat Panel */}
      <ChatPanel isOpen={isChatOpen} onClose={() => setIsChatOpen(false)} />
    </div>
  );
}
