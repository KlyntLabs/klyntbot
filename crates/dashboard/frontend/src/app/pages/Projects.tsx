import { useState, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router';
import {
  Plus,
  Search,
  Loader2,
  AlertTriangle,
  X,
  Trash2,
  ChevronDown,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Project } from '../../lib/types';
import {
  PROJECT_COLORS,
  resolveProjectColor,
  formatTimeAgo,
  inputStyle,
  focusHandler,
  blurHandler,
} from '../../lib/utils';

const STATUS_OPTIONS = ['active', 'paused', 'completed', 'archived'];

function statusLabel(status: string): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

function statusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'active':
      return '#10a37f';
    case 'paused':
      return '#e5c07b';
    case 'completed':
      return '#3b82f6';
    case 'archived':
      return '#666';
    default:
      return 'var(--codex-fg-subtle)';
  }
}

export default function Projects() {
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');

  // Create form state
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [newName, setNewName] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newColor, setNewColor] = useState('blue');
  const [newTags, setNewTags] = useState('');
  const [newStatus, setNewStatus] = useState('active');

  // Delete state
  const [deletingProject, setDeletingProject] = useState<string | null>(null);

  const { data: projects, loading, error, setData } = useApi<Project[]>('/api/projects');

  // Filter projects client-side
  const filteredProjects = useMemo(() => {
    let list = projects ?? [];
    // Status filter
    if (statusFilter !== 'all') {
      list = list.filter((p) => p.status.toLowerCase() === statusFilter);
    }
    // Search
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter((p) => p.name.toLowerCase().includes(q));
    }
    return list;
  }, [projects, statusFilter, searchQuery]);

  const resetCreateForm = useCallback(() => {
    setCreating(false);
    setCreateError(null);
    setNewName('');
    setNewDescription('');
    setNewColor('blue');
    setNewTags('');
    setNewStatus('active');
  }, []);

  // Create project
  const handleCreate = useCallback(async () => {
    if (!newName.trim()) {
      setCreateError('Name is required');
      return;
    }

    setSaving(true);
    setCreateError(null);

    try {
      const body: Record<string, unknown> = { name: newName.trim() };
      if (newDescription.trim()) body.description = newDescription.trim();
      body.color = newColor;
      if (newTags.trim())
        body.tags = newTags
          .split(',')
          .map((t) => t.trim())
          .filter(Boolean);
      body.status = newStatus;

      const created = await apiFetch<Project>('/api/projects', { body });
      setData((prev) => [created, ...(prev ?? [])]);

      // Reset form
      resetCreateForm();
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Failed to create project');
    } finally {
      setSaving(false);
    }
  }, [newName, newDescription, newColor, newTags, newStatus, setData, resetCreateForm]);

  // Delete project
  const confirmDelete = useCallback(
    async (projectId: string) => {
      const projectBefore = projects?.find((p) => p.id === projectId);
      setData((prev) => prev?.filter((p) => p.id !== projectId));
      setDeletingProject(null);

      try {
        await apiFetch<void>(`/api/projects/${encodeURIComponent(projectId)}`, {
          method: 'DELETE',
        });
      } catch {
        // Revert -- re-add the project
        if (projectBefore) {
          setData((prev) => [...(prev ?? []), projectBefore]);
        }
      }
    },
    [projects, setData],
  );

  if (loading && !projects) {
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

  if (error && !projects) {
    return (
      <div
        className="flex-1 flex items-center justify-center gap-2"
        style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg-subtle)' }}
      >
        <AlertTriangle className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px]">Failed to load projects</span>
      </div>
    );
  }

  return (
    <>
      {/* Center Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <div
          className="border-b px-6 py-4"
          style={{
            borderColor: 'var(--codex-border-subtle)',
            backgroundColor: 'var(--codex-bg)',
          }}
        >
          <div className="flex items-center justify-between mb-4">
            <h1 className="text-xl" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
              Projects
            </h1>
            <button
              onClick={() => setCreating(true)}
              className="flex items-center gap-2 px-4 py-2 rounded-lg text-[13px] transition-colors"
              style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
              onMouseEnter={(e) =>
                (e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)')
              }
              onMouseLeave={(e) =>
                (e.currentTarget.style.backgroundColor = 'var(--codex-accent)')
              }
            >
              <Plus className="w-4 h-4" strokeWidth={1.5} />
              New Project
            </button>
          </div>

          {/* Search and Status Filter */}
          <div className="flex items-center gap-3">
            <div className="flex-1 relative">
              <Search
                className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2"
                strokeWidth={1.5}
                style={{ color: 'var(--codex-fg-subtle)' }}
              />
              <input
                type="text"
                placeholder="Search projects..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2 rounded-lg border outline-none text-[13px]"
                style={{
                  backgroundColor: 'var(--codex-bg-tertiary)',
                  borderColor: 'var(--codex-border)',
                  color: 'var(--codex-fg)',
                }}
                onFocus={focusHandler}
                onBlur={blurHandler}
              />
            </div>
            <div className="relative">
              <select
                value={statusFilter}
                onChange={(e) => setStatusFilter(e.target.value)}
                className="appearance-none pl-4 pr-9 py-2 rounded-lg border text-[13px] outline-none transition-colors cursor-pointer"
                style={{
                  backgroundColor: 'var(--codex-bg-tertiary)',
                  borderColor: 'var(--codex-border)',
                  color: 'var(--codex-fg-subtle)',
                }}
                onFocus={focusHandler}
                onBlur={blurHandler}
              >
                <option value="all">All</option>
                {STATUS_OPTIONS.map((s) => (
                  <option key={s} value={s}>
                    {statusLabel(s)}
                  </option>
                ))}
              </select>
              <ChevronDown
                className="w-3.5 h-3.5 absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
                strokeWidth={1.5}
                style={{ color: 'var(--codex-fg-subtle)' }}
              />
            </div>
          </div>
        </div>

        {/* Project List */}
        <div className="flex-1 overflow-y-auto p-6" style={{ backgroundColor: 'var(--codex-bg)' }}>
          <div className="max-w-4xl mx-auto space-y-3">
            {/* Create Form */}
            <AnimatePresence>
              {creating && (
                <motion.div
                  initial={{ opacity: 0, y: -10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  className="rounded-lg border p-6 mb-4"
                  style={{
                    backgroundColor: '#141414',
                    borderColor: 'var(--codex-accent)',
                    borderLeft: '3px solid var(--codex-accent)',
                  }}
                >
                  <div className="flex items-center justify-between mb-5">
                    <h2
                      className="text-[14px]"
                      style={{ color: 'var(--codex-fg)', fontWeight: 600 }}
                    >
                      Create Project
                    </h2>
                    <button
                      onClick={resetCreateForm}
                      style={{ color: 'var(--codex-fg-subtle)' }}
                    >
                      <X className="w-4 h-4" strokeWidth={1.5} />
                    </button>
                  </div>

                  <div className="space-y-4">
                    {/* Name */}
                    <div>
                      <label
                        className="block text-[11px] mb-1.5 uppercase tracking-wider"
                        style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}
                      >
                        Name *
                      </label>
                      <input
                        value={newName}
                        onChange={(e) => setNewName(e.target.value)}
                        placeholder="Project name..."
                        className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                        style={inputStyle}
                        onFocus={focusHandler}
                        onBlur={blurHandler}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') handleCreate();
                        }}
                        autoFocus
                      />
                    </div>

                    {/* Description */}
                    <div>
                      <label
                        className="block text-[11px] mb-1.5 uppercase tracking-wider"
                        style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}
                      >
                        Description
                      </label>
                      <textarea
                        value={newDescription}
                        onChange={(e) => setNewDescription(e.target.value)}
                        placeholder="Optional description..."
                        rows={2}
                        className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors resize-none"
                        style={inputStyle}
                        onFocus={focusHandler}
                        onBlur={blurHandler}
                      />
                    </div>

                    {/* Color Picker */}
                    <div>
                      <label
                        className="block text-[11px] mb-1.5 uppercase tracking-wider"
                        style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}
                      >
                        Color
                      </label>
                      <div className="flex items-center gap-2">
                        {Object.entries(PROJECT_COLORS).map(([name, hex]) => (
                          <button
                            key={name}
                            type="button"
                            onClick={() => setNewColor(name)}
                            className="rounded-full transition-transform"
                            style={{
                              width: 28,
                              height: 28,
                              backgroundColor: hex,
                              border:
                                newColor === name
                                  ? '2px solid var(--codex-fg)'
                                  : '2px solid transparent',
                              transform: newColor === name ? 'scale(1.15)' : 'scale(1)',
                            }}
                            title={name}
                          />
                        ))}
                      </div>
                    </div>

                    {/* Row: Tags, Status */}
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label
                          className="block text-[11px] mb-1.5 uppercase tracking-wider"
                          style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}
                        >
                          Tags (comma-separated)
                        </label>
                        <input
                          value={newTags}
                          onChange={(e) => setNewTags(e.target.value)}
                          placeholder="backend, rust, v2"
                          className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={inputStyle}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        />
                      </div>
                      <div>
                        <label
                          className="block text-[11px] mb-1.5 uppercase tracking-wider"
                          style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}
                        >
                          Status
                        </label>
                        <select
                          value={newStatus}
                          onChange={(e) => setNewStatus(e.target.value)}
                          className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={inputStyle}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        >
                          {STATUS_OPTIONS.map((s) => (
                            <option key={s} value={s}>
                              {statusLabel(s)}
                            </option>
                          ))}
                        </select>
                      </div>
                    </div>

                    {createError && (
                      <div
                        className="text-[12px] flex items-center gap-1.5"
                        style={{ color: '#ef4444' }}
                      >
                        <AlertTriangle className="w-3.5 h-3.5" strokeWidth={1.5} />
                        {createError}
                      </div>
                    )}

                    <div className="flex justify-end gap-3 pt-2">
                      <button
                        onClick={resetCreateForm}
                        className="px-4 py-2 rounded-lg text-[13px] transition-colors"
                        style={{
                          color: 'var(--codex-fg-muted)',
                          border: '1px solid var(--codex-border)',
                        }}
                        onMouseEnter={(e) =>
                          (e.currentTarget.style.borderColor = 'var(--codex-fg-subtle)')
                        }
                        onMouseLeave={(e) =>
                          (e.currentTarget.style.borderColor = 'var(--codex-border)')
                        }
                      >
                        Cancel
                      </button>
                      <button
                        onClick={handleCreate}
                        disabled={saving}
                        className="flex items-center gap-2 px-4 py-2 rounded-lg text-[13px] transition-colors"
                        style={{
                          backgroundColor: 'var(--codex-accent)',
                          color: 'white',
                          opacity: saving ? 0.7 : 1,
                        }}
                        onMouseEnter={(e) =>
                          !saving &&
                          (e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)')
                        }
                        onMouseLeave={(e) =>
                          (e.currentTarget.style.backgroundColor = 'var(--codex-accent)')
                        }
                      >
                        {saving && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                        Create Project
                      </button>
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {filteredProjects.length === 0 && !loading && !searchQuery && (
              <div
                className="text-center py-16 text-[13px]"
                style={{ color: 'var(--codex-fg-subtle)' }}
              >
                No projects yet. Create one to get started.
              </div>
            )}

            {filteredProjects.length === 0 && searchQuery && (
              <div
                className="text-center py-16 text-[13px]"
                style={{ color: 'var(--codex-fg-subtle)' }}
              >
                No projects matching "{searchQuery}"
              </div>
            )}

            {filteredProjects.map((project) => (
              <div key={project.id}>
                <div
                  className="group p-4 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: 'var(--codex-border)' }}
                  onClick={() => navigate(`/projects/${project.id}`)}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = '#1a1a1a';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = '#141414';
                  }}
                >
                  <div className="flex items-start gap-3">
                    {/* Color dot */}
                    <div
                      className="flex-shrink-0 mt-1.5 rounded-full"
                      style={{
                        width: 12,
                        height: 12,
                        backgroundColor: resolveProjectColor(project.color),
                      }}
                    />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-3 mb-1">
                        {/* Name */}
                        <div
                          className="text-[14px]"
                          style={{ color: 'var(--codex-fg)', fontWeight: 500 }}
                        >
                          {project.name}
                        </div>
                        {/* Status badge */}
                        <span
                          className="px-2 py-0.5 rounded-full text-[11px]"
                          style={{
                            color: statusColor(project.status),
                            backgroundColor: `${statusColor(project.status)}15`,
                            border: `1px solid ${statusColor(project.status)}30`,
                            fontWeight: 500,
                          }}
                        >
                          {statusLabel(project.status)}
                        </span>
                      </div>

                      {/* Description */}
                      {project.description && (
                        <div
                          className="text-[13px] truncate mb-2"
                          style={{ color: 'var(--codex-fg-muted)' }}
                        >
                          {project.description}
                        </div>
                      )}

                      {/* Tags */}
                      {project.tags.length > 0 && (
                        <div className="flex items-center gap-1.5 mb-2 flex-wrap">
                          {project.tags.map((tag) => (
                            <span
                              key={tag}
                              className="px-2 py-0.5 rounded text-[11px]"
                              style={{
                                backgroundColor: 'var(--codex-bg)',
                                color: 'var(--codex-fg-subtle)',
                                border: '1px solid var(--codex-border)',
                              }}
                            >
                              {tag}
                            </span>
                          ))}
                        </div>
                      )}

                      {/* Updated date */}
                      <div
                        className="text-[11px]"
                        style={{ color: 'var(--codex-fg-subtle)' }}
                      >
                        Updated {formatTimeAgo(project.updatedAt)}
                      </div>
                    </div>

                    {/* Delete button */}
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setDeletingProject(
                          deletingProject === project.id ? null : project.id,
                        );
                      }}
                      title="Delete project"
                      className="flex-shrink-0 p-1.5 rounded transition-colors opacity-0 group-hover:opacity-100"
                      style={{ color: 'var(--codex-fg-subtle)' }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.color = '#ef4444';
                        e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                        e.currentTarget.style.backgroundColor = 'transparent';
                      }}
                    >
                      <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
                    </button>
                  </div>
                </div>

                {/* Delete confirmation */}
                <AnimatePresence>
                  {deletingProject === project.id && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className="mt-1 p-3 rounded-lg border"
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
                        <strong style={{ color: 'var(--codex-fg)' }}>{project.name}</strong>?
                        This cannot be undone.
                      </p>
                      <div className="flex items-center gap-2">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            confirmDelete(project.id);
                          }}
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
                          onClick={(e) => {
                            e.stopPropagation();
                            setDeletingProject(null);
                          }}
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
            ))}
          </div>
        </div>
      </div>
    </>
  );
}
