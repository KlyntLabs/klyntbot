import { useNavigate, useParams } from 'react-router';
import {
  ArrowLeft,
  Plus,
  Circle,
  Loader2,
  AlertTriangle,
  Trash2,
  X,
} from 'lucide-react';
import { useState, useCallback, useEffect } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Project, ProjectWithStats, Task } from '../../lib/types';

const PROJECT_COLORS: Record<string, string> = {
  red: '#ef4444',
  orange: '#f97316',
  yellow: '#eab308',
  green: '#22c55e',
  blue: '#3b82f6',
  purple: '#8b5cf6',
  gray: '#6b7280',
};

const COLOR_NAMES = Object.keys(PROJECT_COLORS);

const STATUS_COLORS: Record<string, string> = {
  active: '#10a37f',
  paused: '#e5c07b',
  completed: '#3b82f6',
  archived: '#666',
};

const STATUS_OPTIONS = ['Active', 'Paused', 'Completed', 'Archived'];

export default function ProjectDetail() {
  const { id } = useParams();
  const navigate = useNavigate();

  // Editing state
  const [editingName, setEditingName] = useState(false);
  const [nameValue, setNameValue] = useState('');
  const [descValue, setDescValue] = useState('');
  const [patchingField, setPatchingField] = useState<string | null>(null);

  // Tag input state
  const [addingTag, setAddingTag] = useState(false);
  const [newTag, setNewTag] = useState('');

  // Delete state
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const {
    data: projectData,
    loading,
    error,
    setData: setProjectData,
    refetch: refetchProject,
  } = useApi<ProjectWithStats>(`/api/projects/${id}`, {
    params: { withStats: 'true' },
  });

  const { data: linkedTasks } = useApi<Task[]>('/api/tasks', {
    params: { projectId: id },
  });

  // Sync local state when project loads
  useEffect(() => {
    if (projectData) {
      setNameValue(projectData.project.name);
      setDescValue(projectData.project.description ?? '');
    }
  }, [projectData]);

  const formatDate = (dateStr: string) =>
    new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });

  const getPriorityColor = (priority: number | null) => {
    switch (priority) {
      case 1:
        return '#e55050';
      case 2:
        return '#d4a017';
      case 3:
        return '#e5c07b';
      case 4:
        return '#666';
      default:
        return '#666';
    }
  };

  const getTaskStatusColor = (status: string) => {
    switch (status.toLowerCase()) {
      case 'done':
        return '#10a37f';
      case 'doing':
        return '#e5c07b';
      default:
        return '#666';
    }
  };

  // ── Patch helper ──────────────────────────────────────────────────────────────
  const patchProject = useCallback(
    async (field: string, value: unknown) => {
      if (!id) return;
      setPatchingField(field);
      try {
        const updated = await apiFetch<Project>(
          `/api/projects/${encodeURIComponent(id)}`,
          {
            method: 'PATCH',
            body: { [field]: value },
          },
        );
        setProjectData((prev) =>
          prev ? { ...prev, project: updated } : prev,
        );
      } catch {
        refetchProject();
      } finally {
        setPatchingField(null);
      }
    },
    [id, setProjectData, refetchProject],
  );

  // ── Status change ─────────────────────────────────────────────────────────────
  const handleStatusChange = useCallback(
    async (newStatus: string) => {
      if (!projectData || newStatus === projectData.project.status) return;
      setProjectData((prev) =>
        prev
          ? { ...prev, project: { ...prev.project, status: newStatus } }
          : prev,
      );
      await patchProject('status', newStatus);
    },
    [projectData, setProjectData, patchProject],
  );

  // ── Name edit ─────────────────────────────────────────────────────────────────
  const handleNameBlur = useCallback(async () => {
    setEditingName(false);
    if (!projectData || nameValue.trim() === projectData.project.name) return;
    if (!nameValue.trim()) {
      setNameValue(projectData.project.name);
      return;
    }
    setProjectData((prev) =>
      prev
        ? { ...prev, project: { ...prev.project, name: nameValue.trim() } }
        : prev,
    );
    await patchProject('name', nameValue.trim());
  }, [projectData, nameValue, setProjectData, patchProject]);

  // ── Description edit ──────────────────────────────────────────────────────────
  const handleDescBlur = useCallback(async () => {
    if (!projectData) return;
    const newDesc = descValue.trim() || null;
    if (newDesc === (projectData.project.description ?? null)) return;
    setProjectData((prev) =>
      prev
        ? { ...prev, project: { ...prev.project, description: newDesc } }
        : prev,
    );
    await patchProject('description', newDesc);
  }, [projectData, descValue, setProjectData, patchProject]);

  // ── Color change ──────────────────────────────────────────────────────────────
  const handleColorCycle = useCallback(async () => {
    if (!projectData) return;
    const currentIdx = COLOR_NAMES.indexOf(projectData.project.color);
    const nextColor = COLOR_NAMES[(currentIdx + 1) % COLOR_NAMES.length];
    setProjectData((prev) =>
      prev
        ? { ...prev, project: { ...prev.project, color: nextColor } }
        : prev,
    );
    await patchProject('color', nextColor);
  }, [projectData, setProjectData, patchProject]);

  // ── Tag management ────────────────────────────────────────────────────────────
  const handleAddTag = useCallback(async () => {
    if (!projectData || !newTag.trim()) return;
    const tag = newTag.trim().toLowerCase();
    if (projectData.project.tags.includes(tag)) {
      setNewTag('');
      setAddingTag(false);
      return;
    }
    const newTags = [...projectData.project.tags, tag];
    setProjectData((prev) =>
      prev
        ? { ...prev, project: { ...prev.project, tags: newTags } }
        : prev,
    );
    setNewTag('');
    setAddingTag(false);
    await patchProject('tags', newTags);
  }, [projectData, newTag, setProjectData, patchProject]);

  const handleRemoveTag = useCallback(
    async (tag: string) => {
      if (!projectData) return;
      const newTags = projectData.project.tags.filter((t) => t !== tag);
      setProjectData((prev) =>
        prev
          ? { ...prev, project: { ...prev.project, tags: newTags } }
          : prev,
      );
      await patchProject('tags', newTags);
    },
    [projectData, setProjectData, patchProject],
  );

  // ── Delete project ────────────────────────────────────────────────────────────
  const handleDelete = useCallback(async () => {
    if (!id) return;
    try {
      await apiFetch<void>(`/api/projects/${encodeURIComponent(id)}`, {
        method: 'DELETE',
      });
      navigate('/projects');
    } catch {
      setConfirmingDelete(false);
    }
  }, [id, navigate]);

  if (loading && !projectData) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{ backgroundColor: 'var(--codex-bg)' }}
      >
        <Loader2
          className="w-5 h-5 animate-spin"
          style={{ color: 'var(--codex-fg-subtle)' }}
        />
      </div>
    );
  }

  if (error && !projectData) {
    return (
      <div
        className="flex-1 flex items-center justify-center gap-2"
        style={{
          backgroundColor: 'var(--codex-bg)',
          color: 'var(--codex-fg-subtle)',
        }}
      >
        <AlertTriangle className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px]">Failed to load project</span>
      </div>
    );
  }

  if (!projectData) return null;

  const project = projectData.project;
  const currentStatus = project.status.toLowerCase();
  const statusColor = STATUS_COLORS[currentStatus] ?? '#666';
  const colorHex = PROJECT_COLORS[project.color] ?? '#6b7280';
  const taskList = linkedTasks ?? [];

  // Task stats bar segments
  const total = projectData.taskCountTotal;
  const todoPct = total > 0 ? (projectData.taskCountTodo / total) * 100 : 0;
  const doingPct = total > 0 ? (projectData.taskCountDoing / total) * 100 : 0;
  const donePct = total > 0 ? (projectData.taskCountDone / total) * 100 : 0;

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header Bar */}
        <div
          className="border-b px-6 py-4"
          style={{
            borderColor: 'var(--codex-border-subtle)',
            backgroundColor: 'var(--codex-bg)',
          }}
        >
          <div className="flex items-center gap-4">
            <button
              onClick={() => navigate('/projects')}
              className="flex items-center gap-2 text-[13px] transition-colors"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) =>
                (e.currentTarget.style.color = 'var(--codex-fg)')
              }
              onMouseLeave={(e) =>
                (e.currentTarget.style.color = 'var(--codex-fg-subtle)')
              }
            >
              <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
              Projects
            </button>

            <div
              className="text-[13px]"
              style={{
                color: '#10a37f',
                fontFamily: 'var(--font-mono)',
              }}
            >
              {project.id.slice(0, 8)}
            </div>

            {/* Status select */}
            <select
              value={
                currentStatus.charAt(0).toUpperCase() + currentStatus.slice(1)
              }
              onChange={(e) =>
                handleStatusChange(e.target.value.toLowerCase())
              }
              disabled={patchingField === 'status'}
              className="px-3 py-1.5 rounded text-[12px] outline-none cursor-pointer"
              style={{
                backgroundColor:
                  currentStatus === 'active'
                    ? 'rgba(16, 163, 127, 0.2)'
                    : '#141414',
                color:
                  currentStatus === 'active'
                    ? '#10a37f'
                    : 'var(--codex-fg-subtle)',
                border: `1px solid ${currentStatus === 'active' ? '#10a37f' : 'var(--codex-border)'}`,
              }}
            >
              {STATUS_OPTIONS.map((s) => (
                <option key={s}>{s}</option>
              ))}
            </select>

            <div className="flex-1" />

            {/* Delete button */}
            <button
              onClick={() => setConfirmingDelete(!confirmingDelete)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] transition-colors"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) => {
                e.currentTarget.style.color = '#ef4444';
                e.currentTarget.style.backgroundColor =
                  'rgba(239, 68, 68, 0.1)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                e.currentTarget.style.backgroundColor = 'transparent';
              }}
            >
              <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
              Delete
            </button>
          </div>

          {/* Delete confirmation inline */}
          <AnimatePresence>
            {confirmingDelete && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: 'auto' }}
                exit={{ opacity: 0, height: 0 }}
                className="mt-3 p-3 rounded-lg border"
                style={{
                  backgroundColor: 'rgba(239, 68, 68, 0.05)',
                  borderColor: 'rgba(239, 68, 68, 0.2)',
                }}
              >
                <p
                  className="text-[12px] mb-3"
                  style={{ color: 'var(--codex-fg-muted)' }}
                >
                  Delete{' '}
                  <strong style={{ color: 'var(--codex-fg)' }}>
                    {project.name}
                  </strong>
                  ? This cannot be undone.
                </p>
                <div className="flex items-center gap-2">
                  <button
                    onClick={handleDelete}
                    className="px-3 py-1.5 rounded text-[12px] transition-colors"
                    style={{ backgroundColor: '#ef4444', color: 'white' }}
                    onMouseEnter={(e) =>
                      (e.currentTarget.style.backgroundColor = '#dc2626')
                    }
                    onMouseLeave={(e) =>
                      (e.currentTarget.style.backgroundColor = '#ef4444')
                    }
                  >
                    Delete
                  </button>
                  <button
                    onClick={() => setConfirmingDelete(false)}
                    className="px-3 py-1.5 rounded text-[12px] transition-colors"
                    style={{
                      color: 'var(--codex-fg-muted)',
                      border: '1px solid var(--codex-border)',
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* Content Area */}
        <div
          className="flex-1 overflow-y-auto p-8"
          style={{ backgroundColor: 'var(--codex-bg)' }}
        >
          <div className="max-w-4xl mx-auto">
            {/* Name — click-to-edit input */}
            {editingName ? (
              <input
                type="text"
                value={nameValue}
                onChange={(e) => setNameValue(e.target.value)}
                onBlur={handleNameBlur}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') e.currentTarget.blur();
                  if (e.key === 'Escape') {
                    setNameValue(project.name);
                    setEditingName(false);
                  }
                }}
                autoFocus
                className="w-full text-xl mb-6 bg-transparent border-none outline-none"
                style={{
                  color: 'var(--codex-fg)',
                  fontWeight: 500,
                }}
                placeholder="Project name..."
              />
            ) : (
              <div
                className="w-full text-xl mb-6 cursor-text"
                style={{
                  color: 'var(--codex-fg)',
                  fontWeight: 500,
                }}
                onClick={() => setEditingName(true)}
              >
                {project.name}
              </div>
            )}

            {/* Description — click-to-edit textarea */}
            <div className="mb-8">
              <div
                className="rounded-lg border"
                style={{
                  backgroundColor: '#141414',
                  borderColor: '#1a1a1a',
                }}
              >
                <textarea
                  value={descValue}
                  onChange={(e) => setDescValue(e.target.value)}
                  onBlur={handleDescBlur}
                  placeholder="Add a description..."
                  className="w-full p-4 min-h-[400px] bg-transparent outline-none resize-none"
                  style={{
                    color: 'var(--codex-fg)',
                    fontSize: '14px',
                    lineHeight: '1.6',
                  }}
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Right Panel */}
      <aside
        className="w-[420px] border-l overflow-y-auto"
        style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderColor: 'var(--codex-border-subtle)',
        }}
      >
        <div className="p-6 space-y-6">
          {/* Properties Card */}
          <div
            className="p-4 rounded-lg"
            style={{
              backgroundColor: '#141414',
              border: '1px solid var(--codex-border)',
            }}
          >
            <h3
              className="text-[13px] mb-4"
              style={{ color: 'var(--codex-fg)', fontWeight: 500 }}
            >
              Properties
            </h3>
            <div className="space-y-3">
              {/* Status */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Status</span>
                <div className="flex items-center gap-2">
                  <div
                    className="w-2 h-2 rounded-full"
                    style={{ backgroundColor: statusColor }}
                  />
                  <select
                    value={
                      currentStatus.charAt(0).toUpperCase() +
                      currentStatus.slice(1)
                    }
                    onChange={(e) =>
                      handleStatusChange(e.target.value.toLowerCase())
                    }
                    disabled={patchingField === 'status'}
                    className="px-2 py-1 rounded text-[11px] outline-none cursor-pointer"
                    style={{
                      backgroundColor: 'var(--codex-bg)',
                      color: statusColor,
                      border: '1px solid var(--codex-border)',
                    }}
                  >
                    {STATUS_OPTIONS.map((s) => (
                      <option key={s}>{s}</option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Color */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Color</span>
                <button
                  onClick={handleColorCycle}
                  className="flex items-center gap-2 px-2 py-1 rounded text-[11px] cursor-pointer transition-colors"
                  style={{
                    backgroundColor: 'var(--codex-bg)',
                    color: 'var(--codex-fg)',
                    border: '1px solid var(--codex-border)',
                  }}
                  onMouseEnter={(e) =>
                    (e.currentTarget.style.borderColor = colorHex)
                  }
                  onMouseLeave={(e) =>
                    (e.currentTarget.style.borderColor =
                      'var(--codex-border)')
                  }
                >
                  <div
                    className="w-3 h-3 rounded-full"
                    style={{ backgroundColor: colorHex }}
                  />
                  {project.color}
                </button>
              </div>

              {/* Tags */}
              <div>
                <div
                  className="text-[12px] mb-2"
                  style={{ color: 'var(--codex-fg-subtle)' }}
                >
                  Tags
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {project.tags.map((tag) => (
                    <span
                      key={tag}
                      className="group px-2 py-0.5 rounded text-[11px] cursor-pointer transition-colors"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg-subtle)',
                        border: '1px solid var(--codex-border)',
                      }}
                      onClick={() => handleRemoveTag(tag)}
                      title="Click to remove"
                    >
                      {tag}
                      <X
                        className="w-2.5 h-2.5 inline ml-1 opacity-50 group-hover:opacity-100"
                        strokeWidth={2}
                      />
                    </span>
                  ))}
                  {addingTag ? (
                    <input
                      value={newTag}
                      onChange={(e) => setNewTag(e.target.value)}
                      onBlur={() => {
                        if (newTag.trim()) handleAddTag();
                        else setAddingTag(false);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') handleAddTag();
                        if (e.key === 'Escape') {
                          setAddingTag(false);
                          setNewTag('');
                        }
                      }}
                      autoFocus
                      placeholder="tag..."
                      className="px-2 py-0.5 rounded text-[11px] outline-none w-20"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg)',
                        border: '1px solid var(--codex-accent)',
                      }}
                    />
                  ) : (
                    <button
                      onClick={() => setAddingTag(true)}
                      className="px-2 py-0.5 rounded text-[11px] transition-colors"
                      style={{
                        backgroundColor: 'transparent',
                        color: 'var(--codex-fg-subtle)',
                        border: '1px solid var(--codex-border)',
                      }}
                      onMouseEnter={(e) =>
                        (e.currentTarget.style.color = '#10a37f')
                      }
                      onMouseLeave={(e) =>
                        (e.currentTarget.style.color =
                          'var(--codex-fg-subtle)')
                      }
                    >
                      <Plus className="w-3 h-3" strokeWidth={1.5} />
                    </button>
                  )}
                </div>
              </div>

              <div
                className="flex justify-between text-[12px] pt-2 border-t"
                style={{ borderColor: 'var(--codex-border)' }}
              >
                <span style={{ color: 'var(--codex-fg-subtle)' }}>
                  Created
                </span>
                <span style={{ color: 'var(--codex-fg)' }}>
                  {formatDate(project.createdAt)}
                </span>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>
                  Updated
                </span>
                <span style={{ color: 'var(--codex-fg)' }}>
                  {formatDate(project.updatedAt)}
                </span>
              </div>
            </div>
          </div>

          {/* Task Stats Card */}
          <div
            className="p-4 rounded-lg"
            style={{
              backgroundColor: '#141414',
              border: '1px solid var(--codex-border)',
            }}
          >
            <h3
              className="text-[13px] mb-4"
              style={{ color: 'var(--codex-fg)', fontWeight: 500 }}
            >
              Task Stats
            </h3>
            <div className="space-y-3">
              <div className="grid grid-cols-4 gap-2 text-center">
                <div>
                  <div
                    className="text-[16px]"
                    style={{ color: '#666', fontWeight: 500 }}
                  >
                    {projectData.taskCountTodo}
                  </div>
                  <div
                    className="text-[10px]"
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    Todo
                  </div>
                </div>
                <div>
                  <div
                    className="text-[16px]"
                    style={{ color: '#e5c07b', fontWeight: 500 }}
                  >
                    {projectData.taskCountDoing}
                  </div>
                  <div
                    className="text-[10px]"
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    Doing
                  </div>
                </div>
                <div>
                  <div
                    className="text-[16px]"
                    style={{ color: '#10a37f', fontWeight: 500 }}
                  >
                    {projectData.taskCountDone}
                  </div>
                  <div
                    className="text-[10px]"
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    Done
                  </div>
                </div>
                <div>
                  <div
                    className="text-[16px]"
                    style={{ color: 'var(--codex-fg)', fontWeight: 500 }}
                  >
                    {projectData.taskCountTotal}
                  </div>
                  <div
                    className="text-[10px]"
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    Total
                  </div>
                </div>
              </div>

              {/* Stacked bar visualization */}
              {total > 0 && (
                <div
                  className="w-full h-2 rounded-full overflow-hidden flex"
                  style={{ backgroundColor: 'var(--codex-bg)' }}
                >
                  {donePct > 0 && (
                    <div
                      style={{
                        width: `${donePct}%`,
                        backgroundColor: '#10a37f',
                      }}
                    />
                  )}
                  {doingPct > 0 && (
                    <div
                      style={{
                        width: `${doingPct}%`,
                        backgroundColor: '#e5c07b',
                      }}
                    />
                  )}
                  {todoPct > 0 && (
                    <div
                      style={{
                        width: `${todoPct}%`,
                        backgroundColor: '#666',
                      }}
                    />
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Linked Tasks Card */}
          <div
            className="p-4 rounded-lg"
            style={{
              backgroundColor: '#141414',
              border: '1px solid var(--codex-border)',
            }}
          >
            <h3
              className="text-[13px] mb-4"
              style={{ color: 'var(--codex-fg)', fontWeight: 500 }}
            >
              Tasks{' '}
              <span
                className="ml-1"
                style={{ color: 'var(--codex-fg-subtle)', fontWeight: 400 }}
              >
                ({taskList.length})
              </span>
            </h3>
            <div className="space-y-2">
              {taskList.length === 0 ? (
                <div
                  className="text-[12px] py-4 text-center"
                  style={{ color: 'var(--codex-fg-subtle)' }}
                >
                  No tasks linked to this project
                </div>
              ) : (
                taskList.map((task) => (
                  <div
                    key={task.id}
                    className="flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors"
                    style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                    }}
                    onClick={() => navigate(`/tasks/${task.id}`)}
                    onMouseEnter={(e) =>
                      (e.currentTarget.style.backgroundColor = '#1a1a1a')
                    }
                    onMouseLeave={(e) =>
                      (e.currentTarget.style.backgroundColor =
                        'var(--codex-bg)')
                    }
                  >
                    <Circle
                      className="w-3.5 h-3.5 flex-shrink-0"
                      strokeWidth={1.5}
                      style={{
                        color: getTaskStatusColor(task.status),
                        fill:
                          task.status.toLowerCase() === 'done'
                            ? getTaskStatusColor(task.status)
                            : 'transparent',
                      }}
                    />
                    <div
                      className="flex-1 text-[12px] min-w-0 truncate"
                      style={{
                        color: 'var(--codex-fg)',
                        textDecoration:
                          task.status.toLowerCase() === 'done'
                            ? 'line-through'
                            : 'none',
                      }}
                    >
                      {task.title}
                    </div>
                    {task.priority != null && (
                      <div
                        className="flex items-center gap-1 text-[10px] flex-shrink-0 px-1.5 py-0.5 rounded"
                        style={{
                          backgroundColor:
                            getPriorityColor(task.priority) + '20',
                          color: getPriorityColor(task.priority),
                        }}
                      >
                        P{task.priority}
                      </div>
                    )}
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </aside>
    </div>
  );
}
