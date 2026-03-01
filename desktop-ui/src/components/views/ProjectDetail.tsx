import { useState, useMemo } from 'react';
import { useParams, useNavigate } from 'react-router';
import { ArrowLeft, ChevronDown, ChevronRight, Target } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Badge } from '../ui/Badge';
import { Checkbox } from '../ui/Checkbox';
import { Progress } from '../ui/Progress';
import { useSetToggle } from '../../hooks/useSetToggle';
import { mockProjects, mockTasks, mockObjectives } from '../../data/mockData';
import type { SidebarItem } from '../../lib/types';

export function ProjectDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [expandedOkrs, toggleOkr] = useSetToggle();
  const [completedTasks, toggleTask] = useSetToggle(
    mockTasks.filter(t => t.completed).map(t => t.id)
  );

  const project = useMemo(() => mockProjects.find(p => p.id === id), [id]);
  const tasks = useMemo(() => mockTasks.filter(t => t.project === id), [id]);
  const objectives = useMemo(
    () => mockObjectives.filter(o => project?.objectiveIds?.includes(o.id)),
    [id, project],
  );

  if (!project) {
    return (
      <div className="h-screen w-screen bg-[#0E0E0D] text-[#E6EDF3] flex items-center justify-center">
        <p className="text-[#8B949E] text-sm font-light">Project not found</p>
      </div>
    );
  }

  const completedCount = tasks.filter(t => completedTasks.has(t.id)).length;
  const doingCount = tasks.filter(t => t.status === 'Doing').length;
  const avgProgress = objectives.length > 0
    ? Math.round(objectives.reduce((sum, o) => sum + o.progress, 0) / objectives.length)
    : 0;

  const stats = [
    { label: 'Tasks', value: `${tasks.length}`, sub: `${completedCount} done` },
    { label: 'In Progress', value: `${doingCount}`, sub: 'active' },
    { label: 'OKR Progress', value: `${avgProgress}%`, sub: `${objectives.length} objectives` },
    { label: 'Completion', value: `${tasks.length > 0 ? Math.round((completedCount / tasks.length) * 100) : 0}%`, sub: 'of tasks' },
  ];

  return (
    <div className="h-screen w-screen bg-[#0E0E0D] text-[#E6EDF3] flex overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item === 'Tasks') navigate('/');
          if (item === 'Chat') navigate('/chat');
        }}
      />

      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Breadcrumb Header */}
        <div className="h-14 flex items-center px-6 gap-3 border-b border-[rgba(255,255,255,0.08)]">
          <button
            onClick={() => navigate('/')}
            className="text-[#8B949E] hover:text-[#C9D1D9] transition-colors"
          >
            <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
          </button>
          <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: project.color }} />
          <span className="text-[14px] font-light text-[#E6EDF3]">{project.name}</span>
          <span className="text-[12px] text-[#8B949E] font-light">{tasks.length} tasks</span>
        </div>

        {/* Scrollable Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {/* Stats Row */}
          <div className="grid grid-cols-4 gap-3">
            {stats.map(stat => (
              <div key={stat.label} className="bg-[rgba(255,255,255,0.03)] rounded-xl p-4">
                <p className="text-[11px] text-[#8B949E] font-light mb-1">{stat.label}</p>
                <p className="text-[22px] font-light text-[#E6EDF3]">{stat.value}</p>
                <p className="text-[11px] text-[#8B949E] font-light mt-0.5">{stat.sub}</p>
              </div>
            ))}
          </div>

          {/* OKR Section */}
          {objectives.length > 0 && (
            <div>
              <h3 className="text-[12px] font-light text-[#8B949E] uppercase tracking-wider mb-3">Objectives & Key Results</h3>
              <div className="space-y-2">
                {objectives.map(objective => {
                  const isExpanded = expandedOkrs.has(objective.id);
                  return (
                    <div key={objective.id} className="bg-[rgba(255,255,255,0.03)] rounded-xl overflow-hidden">
                      <button
                        onClick={() => toggleOkr(objective.id)}
                        className="w-full flex items-center gap-3 px-4 py-3.5 hover:bg-[rgba(255,255,255,0.02)] transition-colors text-left"
                      >
                        {isExpanded ? (
                          <ChevronDown className="w-3.5 h-3.5 text-[#8B949E] flex-shrink-0" strokeWidth={1.5} />
                        ) : (
                          <ChevronRight className="w-3.5 h-3.5 text-[#8B949E] flex-shrink-0" strokeWidth={1.5} />
                        )}
                        <Target className="w-3.5 h-3.5 text-[#F97316] flex-shrink-0" strokeWidth={1.5} />
                        <span className="text-[13px] font-light text-[#C9D1D9] flex-1">{objective.title}</span>
                        <span className="text-[12px] text-[#8B949E] font-light mr-3">{objective.progress}%</span>
                        <div className="w-24">
                          <Progress value={objective.progress} />
                        </div>
                      </button>
                      {isExpanded && objective.keyResults && (
                        <div className="px-4 pb-3 space-y-2 ml-10">
                          {objective.keyResults.map(kr => (
                            <div key={kr.id} className="flex items-center gap-3">
                              <span className="text-[12px] font-light text-[#8B949E] flex-1">{kr.title}</span>
                              <span className="text-[11px] text-[#6E7681] font-light">
                                {kr.current}{kr.unit === '$' ? '' : ` ${kr.unit}`} / {kr.target}{kr.unit === '$' ? '' : ` ${kr.unit}`}
                              </span>
                              <div className="w-20">
                                <Progress value={kr.progress} />
                              </div>
                              <span className="text-[11px] text-[#8B949E] font-light w-8 text-right">{kr.progress}%</span>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Task Table */}
          <div>
            <h3 className="text-[12px] font-light text-[#8B949E] uppercase tracking-wider mb-3">Tasks</h3>
            <div className="bg-[rgba(255,255,255,0.03)] rounded-xl overflow-hidden">
              {/* Table Header */}
              <div className="grid grid-cols-[40px_1fr_80px_100px_120px_140px] gap-4 border-b border-[rgba(255,255,255,0.08)] text-[11px] text-[#8B949E] font-light px-6 py-3">
                <div></div>
                <div>Task</div>
                <div>Priority</div>
                <div>Status</div>
                <div>Due Date</div>
                <div>Tags</div>
              </div>

              {/* Task Rows */}
              {tasks.map(task => {
                const isCompleted = completedTasks.has(task.id);
                return (
                  <div
                    key={task.id}
                    className="grid grid-cols-[40px_1fr_80px_100px_120px_140px] gap-4 px-6 py-3 hover:bg-[rgba(255,255,255,0.04)] transition-colors border-b border-[rgba(255,255,255,0.04)] last:border-b-0"
                  >
                    <div className="flex items-center">
                      <Checkbox checked={isCompleted} onCheckedChange={() => toggleTask(task.id)} />
                    </div>
                    <div className="flex items-center gap-1.5">
                      {task.objectiveId && (
                        <Target className="w-[10px] h-[10px] text-[#F97316] flex-shrink-0" strokeWidth={1.5} />
                      )}
                      <span className={`text-[13px] font-light truncate ${isCompleted ? 'text-[#8B949E] line-through' : 'text-[#C9D1D9]'}`}>
                        {task.title}
                      </span>
                    </div>
                    <div className="flex items-center">
                      <Badge variant="priority" value={task.priority} />
                    </div>
                    <div className="flex items-center">
                      <Badge variant="status" value={task.status} />
                    </div>
                    <div className="flex items-center">
                      <span className="text-[12px] text-[#8B949E] font-light">{task.dueDate}</span>
                    </div>
                    <div className="flex items-center gap-1.5">
                      {task.tags.map(tag => (
                        <Badge key={tag} variant="tag" value={tag} />
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
