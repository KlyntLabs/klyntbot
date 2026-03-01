import { useState, useMemo, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router';
import { ArrowLeft, ChevronDown, ChevronRight, Target } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Badge } from '../ui/Badge';
import { Checkbox } from '../ui/Checkbox';
import { Progress } from '../ui/Progress';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useEvent } from '../../hooks/useEvent';
import { mockProjects, mockTasks, mockObjectives } from '../../data/mockData';
import type { Task, Project, Objective, SidebarItem } from '../../lib/types';

export function ProjectDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [expandedOkrs, toggleOkr] = useSetToggle();

  const mockProjectTasks = useMemo(() => mockTasks.filter(t => t.projectId === id), [id]);
  const mockProjectObjectives = useMemo(
    () => {
      const proj = mockProjects.find(p => p.id === id);
      return mockObjectives.filter(o => proj?.objectiveIds?.includes(o.id));
    },
    [id],
  );

  const { data: allProjects } = useQuery<Project[]>('project_list', undefined, mockProjects);
  const { data: tasks, refetch: refetchTasks } = useQuery<Task[]>(
    'task_list',
    id ? { project_id: id } : undefined,
    mockProjectTasks,
  );
  const { data: objectives } = useQuery<Objective[]>(
    'objective_list',
    id ? { project_id: id } : undefined,
    mockProjectObjectives,
  );

  const toggleComplete = useMutation<Task, { id: string }>('task_toggle_complete');

  const [completedTasks, toggleTask] = useSetToggle(
    tasks.filter(t => t.completed).map(t => t.id)
  );

  const handleToggleTask = useCallback(async (taskId: string) => {
    toggleTask(taskId);
    await toggleComplete.mutate({ id: taskId });
  }, [toggleTask, toggleComplete]);

  useEvent<{ entityKind: string; id: string }>('entity:updated', () => {
    refetchTasks();
  });

  const project = useMemo(() => allProjects.find(p => p.id === id), [id, allProjects]);

  if (!project) {
    return (
      <div className="h-screen w-screen bg-background text-primary flex items-center justify-center">
        <p className="text-muted text-sm font-light">Project not found</p>
      </div>
    );
  }

  const completedCount = tasks.filter(t => completedTasks.has(t.id)).length;
  const doingCount = tasks.filter(t => t.status === 'doing').length;
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
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
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
        <div className="h-14 flex items-center px-6 gap-3 border-b border-border">
          <button
            onClick={() => navigate('/')}
            className="text-muted hover:text-secondary transition-colors"
          >
            <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
          </button>
          <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: project.color }} />
          <span className="text-[14px] font-light text-primary">{project.name}</span>
          <span className="text-[12px] text-muted font-light">{tasks.length} tasks</span>
        </div>

        {/* Scrollable Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {/* Stats Row */}
          <div className="grid grid-cols-4 gap-3">
            {stats.map(stat => (
              <div key={stat.label} className="bg-surface-low rounded-xl p-4">
                <p className="text-[11px] text-muted font-light mb-1">{stat.label}</p>
                <p className="text-[22px] font-light text-primary">{stat.value}</p>
                <p className="text-[11px] text-muted font-light mt-0.5">{stat.sub}</p>
              </div>
            ))}
          </div>

          {/* OKR Section */}
          {objectives.length > 0 && (
            <div>
              <h3 className="text-[12px] font-light text-muted uppercase tracking-wider mb-3">Objectives & Key Results</h3>
              <div className="space-y-2">
                {objectives.map(objective => {
                  const isExpanded = expandedOkrs.has(objective.id);
                  return (
                    <div key={objective.id} className="bg-surface-low rounded-xl overflow-hidden">
                      <button
                        onClick={() => toggleOkr(objective.id)}
                        className="w-full flex items-center gap-3 px-4 py-3.5 hover:bg-surface-lowest transition-colors text-left"
                      >
                        {isExpanded ? (
                          <ChevronDown className="w-3.5 h-3.5 text-muted flex-shrink-0" strokeWidth={1.5} />
                        ) : (
                          <ChevronRight className="w-3.5 h-3.5 text-muted flex-shrink-0" strokeWidth={1.5} />
                        )}
                        <Target className="w-3.5 h-3.5 text-brand flex-shrink-0" strokeWidth={1.5} />
                        <span className="text-[13px] font-light text-secondary flex-1">{objective.title}</span>
                        <span className="text-[12px] text-muted font-light mr-3">{objective.progress}%</span>
                        <div className="w-24">
                          <Progress value={objective.progress} />
                        </div>
                      </button>
                      {isExpanded && objective.keyResults && (
                        <div className="px-4 pb-3 space-y-2 ml-10">
                          {objective.keyResults.map(kr => (
                            <div key={kr.id} className="flex items-center gap-3">
                              <span className="text-[12px] font-light text-muted flex-1">{kr.title}</span>
                              <span className="text-[11px] text-dim font-light">
                                {kr.current}{kr.unit === '$' ? '' : ` ${kr.unit}`} / {kr.target}{kr.unit === '$' ? '' : ` ${kr.unit}`}
                              </span>
                              <div className="w-20">
                                <Progress value={kr.progress} />
                              </div>
                              <span className="text-[11px] text-muted font-light w-8 text-right">{kr.progress}%</span>
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
            <h3 className="text-[12px] font-light text-muted uppercase tracking-wider mb-3">Tasks</h3>
            <div className="bg-surface-low rounded-xl overflow-hidden">
              {/* Table Header */}
              <div className="grid grid-cols-[40px_1fr_80px_100px_120px_140px] gap-4 border-b border-border text-[11px] text-muted font-light px-6 py-3">
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
                    className="grid grid-cols-[40px_1fr_80px_100px_120px_140px] gap-4 px-6 py-3 hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0"
                  >
                    <div className="flex items-center">
                      <Checkbox checked={isCompleted} onCheckedChange={() => handleToggleTask(task.id)} />
                    </div>
                    <div className="flex items-center gap-1.5">
                      {task.objectiveId && (
                        <Target className="w-[10px] h-[10px] text-brand flex-shrink-0" strokeWidth={1.5} />
                      )}
                      <span className={`text-[13px] font-light truncate ${isCompleted ? 'text-muted line-through' : 'text-secondary'}`}>
                        {task.title}
                      </span>
                    </div>
                    <div className="flex items-center">
                      <Badge variant="priority" value={task.priority ?? ''} />
                    </div>
                    <div className="flex items-center">
                      <Badge variant="status" value={task.status} />
                    </div>
                    <div className="flex items-center">
                      <span className="text-[12px] text-muted font-light">{task.dueDate}</span>
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
