import { useState, useMemo, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router';
import { ArrowLeft, ChevronDown, ChevronRight, Target, Plus, Archive } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { Badge } from '../ui/Badge';
import { Checkbox } from '../ui/Checkbox';
import { Progress } from '../ui/Progress';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useEvent } from '../../hooks/useEvent';
import type { Task, Project, Objective, SidebarItem, ProjectUpdateParams, ObjectiveCreateParams } from '../../lib/types';

const PROJECT_COLORS = ['#3b82f6', '#ef4444', '#f97316', '#eab308', '#22c55e', '#a855f7', '#6b7280'];

export function ProjectDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Tasks');
  const [expandedOkrs, toggleOkr] = useSetToggle();

  const { data: allProjects, refetch: refetchProjects } = useQuery<Project[]>('project_list', undefined, []);
  const { data: tasks, refetch: refetchTasks } = useQuery<Task[]>(
    'task_list',
    id ? { project_id: id } : undefined,
    [],
  );
  const { data: objectives, refetch: refetchObjectives } = useQuery<Objective[]>(
    'objective_list',
    id ? { project_id: id } : undefined,
    [],
  );

  const toggleComplete = useMutation<Task, { id: string }>('task_toggle_complete');
  const updateProject = useMutation<Project, ProjectUpdateParams>('project_update');
  const archiveProject = useMutation<Project, { id: string }>('project_archive');
  const createTask = useMutation<Task, { title: string; projectId: string }>('task_create');
  const createObjective = useMutation<Objective, ObjectiveCreateParams>('objective_create');

  const [completedTasks, toggleTask] = useSetToggle(
    tasks.filter(t => t.completed).map(t => t.id)
  );

  // Inline editing state
  const [editingName, setEditingName] = useState(false);
  const [nameDraft, setNameDraft] = useState('');
  const [showColorPicker, setShowColorPicker] = useState(false);
  const [addingTask, setAddingTask] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState('');
  const [addingObjective, setAddingObjective] = useState(false);
  const [newObjTitle, setNewObjTitle] = useState('');
  const [confirmArchive, setConfirmArchive] = useState(false);

  const handleToggleTask = useCallback(async (taskId: string) => {
    toggleTask(taskId);
    await toggleComplete.mutate({ id: taskId });
  }, [toggleTask, toggleComplete]);

  useEvent<{ entityKind: string; id: string }>('entity:updated', (payload) => {
    const kind = payload?.entityKind;
    if (!kind || kind === 'task') refetchTasks();
    if (!kind || kind === 'project') refetchProjects();
    if (!kind || kind === 'objective' || kind === 'keyResult') refetchObjectives();
  });

  const project = useMemo(() => allProjects.find(p => p.id === id), [id, allProjects]);

  const handleUpdateProject = useCallback(async (params: Partial<ProjectUpdateParams>) => {
    if (!id) return;
    await updateProject.mutate({ id, ...params });
  }, [id, updateProject]);

  const handleArchive = useCallback(async () => {
    if (!id) return;
    if (!confirmArchive) {
      setConfirmArchive(true);
      return;
    }
    await archiveProject.mutate({ id });
    navigate('/');
  }, [id, confirmArchive, archiveProject, navigate]);

  const handleAddTask = useCallback(async () => {
    if (!id || !newTaskTitle.trim()) return;
    await createTask.mutate({ title: newTaskTitle.trim(), projectId: id });
    setNewTaskTitle('');
    setAddingTask(false);
  }, [id, newTaskTitle, createTask]);

  const handleAddObjective = useCallback(async () => {
    if (!id || !newObjTitle.trim()) return;
    await createObjective.mutate({ title: newObjTitle.trim(), projectId: id });
    setNewObjTitle('');
    setAddingObjective(false);
  }, [id, newObjTitle, createObjective]);

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

          {/* Color dot — clickable to open picker */}
          <div className="relative">
            <button
              onClick={() => setShowColorPicker(!showColorPicker)}
              className="w-2.5 h-2.5 rounded-full cursor-pointer hover:ring-2 hover:ring-brand/30 transition-all"
              style={{ backgroundColor: project.color }}
            />
            {showColorPicker && (
              <div className="absolute top-6 left-0 z-50 bg-surface-floating rounded-lg p-2 shadow-lg border border-border-subtle flex gap-1.5">
                {PROJECT_COLORS.map(c => (
                  <button
                    key={c}
                    onClick={() => { handleUpdateProject({ color: c }); setShowColorPicker(false); }}
                    className={`w-5 h-5 rounded-full hover:ring-2 hover:ring-brand/30 transition-all ${project.color === c ? 'ring-2 ring-brand' : ''}`}
                    style={{ backgroundColor: c }}
                  />
                ))}
              </div>
            )}
          </div>

          {/* Project name — click to edit */}
          {editingName ? (
            <input
              autoFocus
              value={nameDraft}
              onChange={e => setNameDraft(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter') {
                  handleUpdateProject({ name: nameDraft });
                  setEditingName(false);
                }
                if (e.key === 'Escape') setEditingName(false);
              }}
              onBlur={() => {
                if (nameDraft !== project.name) handleUpdateProject({ name: nameDraft });
                setEditingName(false);
              }}
              className="text-[14px] font-light text-primary bg-transparent border-b border-brand outline-none"
            />
          ) : (
            <span
              onClick={() => { setNameDraft(project.name); setEditingName(true); }}
              className="text-[14px] font-light text-primary cursor-text hover:text-secondary transition-colors"
            >
              {project.name}
            </span>
          )}

          <span className="text-[12px] text-muted font-light">{tasks.length} tasks</span>

          <div className="flex-1" />

          {/* Archive button */}
          <button
            onClick={handleArchive}
            onBlur={() => setConfirmArchive(false)}
            className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[11px] font-light transition-colors ${
              confirmArchive
                ? 'bg-destructive text-white'
                : 'text-muted hover:text-secondary hover:bg-surface-low'
            }`}
          >
            <Archive className="w-3.5 h-3.5" strokeWidth={1.5} />
            {confirmArchive ? 'Click again' : 'Archive'}
          </button>
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
          <div>
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-[12px] font-light text-muted uppercase tracking-wider">Objectives & Key Results</h3>
              <button
                onClick={() => setAddingObjective(true)}
                className="flex items-center gap-1 text-[12px] text-brand hover:text-brand-hover transition-colors font-light"
              >
                <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                New Objective
              </button>
            </div>

            <div className="space-y-2">
              {objectives.map(objective => {
                const isExpanded = expandedOkrs.has(objective.id);
                return (
                  <div key={objective.id} className="bg-surface-low rounded-xl overflow-hidden">
                    <div className="flex items-center">
                      <button
                        onClick={() => toggleOkr(objective.id)}
                        className="flex items-center gap-3 px-4 py-3.5 hover:bg-surface-lowest transition-colors text-left flex-1"
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
                      <button
                        onClick={() => navigate(`/objective/${objective.id}`)}
                        className="px-3 py-3.5 text-[11px] text-muted hover:text-brand transition-colors font-light"
                      >
                        Open
                      </button>
                    </div>
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

              {objectives.length === 0 && !addingObjective && (
                <div className="text-center py-6">
                  <p className="text-[13px] text-muted font-light">No objectives yet</p>
                </div>
              )}

              {/* Add Objective inline row */}
              {addingObjective && (
                <div className="bg-surface-low rounded-xl px-4 py-3">
                  <input
                    autoFocus
                    value={newObjTitle}
                    onChange={e => setNewObjTitle(e.target.value)}
                    onKeyDown={e => {
                      if (e.key === 'Enter') handleAddObjective();
                      if (e.key === 'Escape') { setAddingObjective(false); setNewObjTitle(''); }
                    }}
                    onBlur={() => { if (!newObjTitle.trim()) { setAddingObjective(false); setNewObjTitle(''); } }}
                    placeholder="Objective title..."
                    className="w-full bg-transparent text-[13px] font-light text-primary outline-none placeholder:text-dim"
                  />
                </div>
              )}
            </div>
          </div>

          {/* Task Table */}
          <div>
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-[12px] font-light text-muted uppercase tracking-wider">Tasks</h3>
              <button
                onClick={() => setAddingTask(true)}
                className="flex items-center gap-1 text-[12px] text-brand hover:text-brand-hover transition-colors font-light"
              >
                <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                New Task
              </button>
            </div>
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

              {/* Add Task inline row */}
              {addingTask && (
                <div className="grid grid-cols-[40px_1fr_80px_100px_120px_140px] gap-4 px-6 py-3 border-b border-border-subtle">
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
                      placeholder="Task title..."
                      className="w-full bg-transparent text-[13px] font-light text-primary outline-none placeholder:text-dim"
                    />
                  </div>
                  <div /><div /><div /><div />
                </div>
              )}

              {/* Task Rows */}
              {tasks.map(task => {
                const isCompleted = completedTasks.has(task.id);
                return (
                  <div
                    key={task.id}
                    onClick={() => navigate(`/task/${task.id}`)}
                    className="grid grid-cols-[40px_1fr_80px_100px_120px_140px] gap-4 px-6 py-3 hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0 cursor-pointer"
                  >
                    <div className="flex items-center" onClick={e => e.stopPropagation()}>
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

              {tasks.length === 0 && !addingTask && (
                <div className="px-6 py-8 text-center">
                  <p className="text-[13px] text-muted font-light">No tasks yet</p>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
