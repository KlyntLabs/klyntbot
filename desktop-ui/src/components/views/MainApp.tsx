import { useState, useMemo } from 'react';
import { MessageSquare } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Toolbar } from '../tasks/Toolbar';
import { TaskTable } from '../tasks/TaskTable';
import { ChatPanel } from '../chat/ChatPanel';
import { useSetToggle } from '../../hooks/useSetToggle';
import { mockProjects, mockTasks, mockObjectives } from '../../data/mockData';
import type { Tab, SidebarItem, ViewMode } from '../../lib/types';

export function MainApp() {
  const [activeTab, setActiveTab] = useState<Tab>('All');
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [collapsedProjects, toggleProject] = useSetToggle();
  const [completedTasks, toggleTask] = useSetToggle(
    mockTasks.filter(t => t.completed).map(t => t.id)
  );

  const filteredTasks = useMemo(() =>
    activeTab === 'All' ? mockTasks : mockTasks.filter(task => task.area === activeTab),
  [activeTab]);

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
              const nextOpen = !isChatOpen;
              setIsChatOpen(nextOpen);
              setActiveSidebar(nextOpen ? 'Chat' : 'Tasks');
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
