import { useState, useCallback } from 'react';
import { Plus, ChevronDown, ChevronRight, Loader2, AlertTriangle, X, Trash2, Pencil } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Skill } from '../../lib/types';

// Icon map derived from skill name — no hardcoded metadata, just display icons
const SKILL_ICONS: Record<string, { icon: string; color: string }> = {
  'todo':            { icon: '\u2713', color: '#10a37f' },
  'daily-planning':  { icon: '\u2600\uFE0F', color: '#fbbf24' },
  'finance':         { icon: '$', color: '#10b981' },
  'cron':            { icon: '\u23F0', color: '#3b82f6' },
  'skill-creator':   { icon: '\u26A1', color: '#8b5cf6' },
  'summarize':       { icon: '\uD83D\uDCC4', color: '#6b7280' },
  'weather':         { icon: '\uD83C\uDF24\uFE0F', color: '#06b6d4' },
  'browser':         { icon: '\uD83C\uDF10', color: '#8b5cf6' },
  'weekly-report':   { icon: '\uD83D\uDCCA', color: '#f59e0b' },
};
const DEFAULT_ICON = { icon: '\uD83D\uDD27', color: '#6b7280' };

export default function Skills() {
  const [expandedSkill, setExpandedSkill] = useState<string | null>(null);
  const [contentViewing, setContentViewing] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editingSkill, setEditingSkill] = useState<string | null>(null);
  const [deletingSkill, setDeletingSkill] = useState<string | null>(null);

  // Create form state
  const [newName, setNewName] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newContent, setNewContent] = useState('');
  const [newTriggers, setNewTriggers] = useState('');
  const [newAlways, setNewAlways] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  // Edit form state
  const [editDescription, setEditDescription] = useState('');
  const [editContent, setEditContent] = useState('');
  const [editTriggers, setEditTriggers] = useState('');
  const [editAlways, setEditAlways] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);

  const { data: skills, loading, error, setData } = useApi<Skill[]>('/api/skills');

  // Normalize API response — handle both old (4-field) and new (12-field) backends
  const skillList: Skill[] = (skills ?? []).map(s => ({
    ...s,
    source: s.source ?? 'built-in',
    always: s.always ?? false,
    triggers: s.triggers ?? [],
    requiresBins: s.requiresBins ?? [],
    requiresEnv: s.requiresEnv ?? [],
    content: s.content ?? null,
    enabled: s.enabled ?? s.available,
  }));

  const builtInSkills = skillList.filter(s => s.source === 'built-in');
  const workspaceSkills = skillList.filter(s => s.source === 'workspace');

  const totalSkills = skillList.length;
  const activeSkills = skillList.filter(s => s.enabled).length;
  const unavailableSkills = skillList.filter(s => !s.available).length;

  const toggleSkill = useCallback(async (skillName: string, currentEnabled: boolean) => {
    const newEnabled = !currentEnabled;

    // Optimistic update
    setData(prev =>
      prev?.map(s => s.name === skillName ? { ...s, enabled: newEnabled } : s),
    );

    try {
      await apiFetch(`/api/skills/${encodeURIComponent(skillName)}`, {
        method: 'PATCH',
        body: { enabled: newEnabled },
      });
    } catch {
      // Revert on failure
      setData(prev =>
        prev?.map(s => s.name === skillName ? { ...s, enabled: currentEnabled } : s),
      );
    }
  }, [setData]);

  const confirmDelete = useCallback(async (skillName: string) => {
    // Optimistic update
    setData(prev => prev?.filter(s => s.name !== skillName));
    setDeletingSkill(null);

    try {
      await apiFetch<void>(`/api/skills/${encodeURIComponent(skillName)}`, {
        method: 'DELETE',
      });
    } catch {
      // Refetch on failure (can't easily revert delete)
      const fresh = await apiFetch<Skill[]>('/api/skills');
      setData(fresh);
    }
  }, [setData]);

  const startEditing = useCallback((skill: Skill) => {
    setEditingSkill(skill.name);
    setEditDescription(skill.description);
    setEditContent(skill.content ?? '');
    setEditTriggers(skill.triggers.join(', '));
    setEditAlways(skill.always);
    setEditError(null);
    setExpandedSkill(skill.name);
  }, []);

  const handleEditSave = useCallback(async () => {
    if (!editingSkill) return;
    setSaving(true);
    setEditError(null);

    try {
      const updated = await apiFetch<Skill>(`/api/skills/${encodeURIComponent(editingSkill)}`, {
        method: 'PATCH',
        body: {
          description: editDescription.trim(),
          content: editContent.trim(),
          triggers: editTriggers.split(',').map(t => t.trim()).filter(Boolean),
          always: editAlways,
        },
      });

      // Update in list
      setData(prev => prev?.map(s => s.name === editingSkill ? { ...s, ...updated } : s));
      setEditingSkill(null);
    } catch (err) {
      setEditError(err instanceof Error ? err.message : 'Failed to update skill');
    } finally {
      setSaving(false);
    }
  }, [editingSkill, editDescription, editContent, editTriggers, editAlways, setData]);

  const handleCreate = useCallback(async () => {
    if (!newName.trim()) {
      setCreateError('Name is required');
      return;
    }
    setSaving(true);
    setCreateError(null);

    try {
      const created = await apiFetch<Skill>('/api/skills', {
        body: {
          name: newName.trim(),
          description: newDescription.trim(),
          version: '1.0',
          content: newContent.trim(),
          triggers: newTriggers.split(',').map(t => t.trim()).filter(Boolean),
          always: newAlways,
        },
      });

      // Add to list optimistically
      setData(prev => [...(prev ?? []), created]);

      // Reset form
      setCreating(false);
      setNewName('');
      setNewDescription('');
      setNewContent('');
      setNewTriggers('');
      setNewAlways(false);
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Failed to create skill');
    } finally {
      setSaving(false);
    }
  }, [newName, newDescription, newContent, newTriggers, newAlways, setData]);

  // Loading state
  if (loading && !skills) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <Loader2 className="w-5 h-5 animate-spin" style={{ color: 'var(--codex-fg-subtle)' }} />
      </div>
    );
  }

  // Error state
  if (error && !skills) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2" style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg-subtle)' }}>
        <AlertTriangle className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px]">Failed to load skills</span>
      </div>
    );
  }

  const getIcon = (name: string) => SKILL_ICONS[name] ?? DEFAULT_ICON;

  const renderSkillCard = (skill: Skill) => {
    const isExpanded = expandedSkill === skill.name;
    const isViewingContent = contentViewing === skill.name;
    const canToggle = skill.available;
    const { icon, color } = getIcon(skill.name);
    const hasRequirements = skill.requiresBins.length > 0 || skill.requiresEnv.length > 0;

    return (
      <motion.div
        key={skill.name}
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        className="rounded-lg border"
        style={{
          backgroundColor: '#141414',
          borderColor: '#1e1e1e'
        }}
      >
        <div className="p-4">
          <div className="flex items-start gap-4">
            <div
              className="w-10 h-10 rounded-full flex items-center justify-center flex-shrink-0 text-lg"
              style={{ backgroundColor: color + '20', color }}
            >
              {icon}
            </div>

            <div className="flex-1 min-w-0">
              <div className="flex items-start justify-between gap-4 mb-2">
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-1">
                    <h3 className="text-[14px]" style={{
                      color: 'var(--codex-fg)',
                      fontWeight: 600,
                      fontFamily: 'var(--font-mono)'
                    }}>
                      {skill.name}
                    </h3>
                    <span className="px-2 py-0.5 rounded text-[10px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      color: 'var(--codex-fg-subtle)',
                      fontFamily: 'var(--font-mono)'
                    }}>
                      {skill.version}
                    </span>
                    <span className="px-2 py-0.5 rounded text-[10px]" style={{
                      backgroundColor: skill.source === 'workspace' ? 'transparent' : 'var(--codex-bg)',
                      color: skill.source === 'workspace' ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
                      border: skill.source === 'workspace' ? '1px solid var(--codex-accent)' : 'none'
                    }}>
                      {skill.source === 'built-in' ? 'Built-in' : 'Workspace'}
                    </span>
                  </div>
                  <p className="text-[13px] mb-2" style={{ color: '#888' }}>
                    {skill.description}
                  </p>
                  <div className="flex items-center gap-2 text-[12px]">
                    <div className="flex items-center gap-1.5">
                      <div className="w-1.5 h-1.5 rounded-full" style={{
                        backgroundColor: skill.available ? '#10b981' : '#ef4444'
                      }} />
                      <span style={{
                        color: skill.available ? '#10b981' : '#ef4444'
                      }}>
                        {skill.available ? 'Available' : 'Unavailable'}
                      </span>
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  {skill.source === 'workspace' && (
                    <>
                      <button
                        onClick={() => startEditing(skill)}
                        className="p-1.5 rounded transition-colors"
                        style={{ color: 'var(--codex-fg-subtle)' }}
                        onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-accent)'}
                        onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                        title="Edit skill"
                      >
                        <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />
                      </button>
                      <button
                        onClick={() => setDeletingSkill(skill.name)}
                        className="p-1.5 rounded transition-colors"
                        style={{ color: 'var(--codex-fg-subtle)' }}
                        onMouseEnter={(e) => e.currentTarget.style.color = '#ef4444'}
                        onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                        title="Delete skill"
                      >
                        <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
                      </button>
                    </>
                  )}
                  <button
                    onClick={() => canToggle && toggleSkill(skill.name, skill.enabled)}
                    disabled={!canToggle}
                    className="w-11 h-6 rounded-full relative transition-all flex-shrink-0"
                    style={{
                      backgroundColor: skill.enabled ? 'var(--codex-accent)' : '#333',
                      opacity: canToggle ? 1 : 0.5,
                      cursor: canToggle ? 'pointer' : 'not-allowed'
                    }}
                  >
                    <div className="w-5 h-5 bg-white rounded-full absolute top-0.5 transition-all" style={{
                      left: skill.enabled ? '22px' : '2px'
                    }} />
                  </button>
                </div>
              </div>

              {skill.available && (
                <button
                  onClick={() => setExpandedSkill(isExpanded ? null : skill.name)}
                  className="flex items-center gap-1 text-[12px] mt-2 transition-colors"
                  style={{ color: 'var(--codex-fg-subtle)' }}
                  onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
                  onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                >
                  {isExpanded ? (
                    <>
                      <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} />
                      Hide details
                    </>
                  ) : (
                    <>
                      <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
                      Show details
                    </>
                  )}
                </button>
              )}

              {/* Delete confirmation */}
              <AnimatePresence>
                {deletingSkill === skill.name && (
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
                    <p className="text-[12px] mb-3" style={{ color: 'var(--codex-fg-muted)' }}>
                      Delete <strong style={{ color: 'var(--codex-fg)' }}>{skill.name}</strong>? This removes the SKILL.md file from disk and cannot be undone.
                    </p>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => confirmDelete(skill.name)}
                        className="px-3 py-1.5 rounded text-[12px] transition-colors"
                        style={{ backgroundColor: '#ef4444', color: 'white' }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#dc2626'}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#ef4444'}
                      >
                        Delete
                      </button>
                      <button
                        onClick={() => setDeletingSkill(null)}
                        className="px-3 py-1.5 rounded text-[12px] transition-colors"
                        style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}
                      >
                        Cancel
                      </button>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              <AnimatePresence>
                {isExpanded && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: 'auto' }}
                    exit={{ opacity: 0, height: 0 }}
                    className="mt-4 pt-4 border-t space-y-3"
                    style={{ borderColor: 'var(--codex-border)' }}
                  >
                    {/* Edit form for workspace skills */}
                    {editingSkill === skill.name ? (
                      <div className="space-y-3">
                        <div>
                          <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                            color: 'var(--codex-fg-subtle)', fontWeight: 500
                          }}>
                            Description
                          </label>
                          <input
                            value={editDescription}
                            onChange={(e) => setEditDescription(e.target.value)}
                            className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                            style={{
                              backgroundColor: 'var(--codex-bg)',
                              color: 'var(--codex-fg)',
                              border: '1px solid var(--codex-border)',
                            }}
                            onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                            onBlur={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                          />
                        </div>

                        <div>
                          <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                            color: 'var(--codex-fg-subtle)', fontWeight: 500
                          }}>
                            Triggers
                          </label>
                          <input
                            value={editTriggers}
                            onChange={(e) => setEditTriggers(e.target.value)}
                            placeholder="keyword1, keyword2"
                            className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                            style={{
                              backgroundColor: 'var(--codex-bg)',
                              color: 'var(--codex-fg)',
                              border: '1px solid var(--codex-border)',
                              fontFamily: 'var(--font-mono)',
                            }}
                            onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                            onBlur={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                          />
                        </div>

                        <div>
                          <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                            color: 'var(--codex-fg-subtle)', fontWeight: 500
                          }}>
                            Content
                          </label>
                          <textarea
                            value={editContent}
                            onChange={(e) => setEditContent(e.target.value)}
                            rows={10}
                            className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors resize-y"
                            style={{
                              backgroundColor: 'var(--codex-bg)',
                              color: 'var(--codex-fg)',
                              border: '1px solid var(--codex-border)',
                              fontFamily: 'var(--font-mono)',
                            }}
                            onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                            onBlur={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                          />
                        </div>

                        <div className="flex items-center gap-3">
                          <button
                            onClick={() => setEditAlways(!editAlways)}
                            className="w-9 h-5 rounded-full relative transition-all flex-shrink-0"
                            style={{ backgroundColor: editAlways ? 'var(--codex-accent)' : '#333' }}
                          >
                            <div className="w-4 h-4 bg-white rounded-full absolute top-0.5 transition-all" style={{
                              left: editAlways ? '18px' : '2px'
                            }} />
                          </button>
                          <span className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                            Always loaded
                          </span>
                        </div>

                        {editError && (
                          <div className="text-[12px] flex items-center gap-1.5" style={{ color: '#ef4444' }}>
                            <AlertTriangle className="w-3.5 h-3.5" strokeWidth={1.5} />
                            {editError}
                          </div>
                        )}

                        <div className="flex justify-end gap-3 pt-1">
                          <button
                            onClick={() => setEditingSkill(null)}
                            className="px-3 py-1.5 rounded-lg text-[12px] transition-colors"
                            style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}
                          >
                            Cancel
                          </button>
                          <button
                            onClick={handleEditSave}
                            disabled={saving}
                            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-[12px] transition-colors"
                            style={{
                              backgroundColor: 'var(--codex-accent)',
                              color: 'white',
                              opacity: saving ? 0.7 : 1,
                            }}
                          >
                            {saving && <Loader2 className="w-3 h-3 animate-spin" />}
                            Save Changes
                          </button>
                        </div>
                      </div>
                    ) : (
                      /* Read-only details view */
                      <>
                        {skill.triggers.length > 0 && (
                          <div>
                            <div className="text-[11px] mb-2" style={{
                              color: 'var(--codex-fg-subtle)',
                              fontWeight: 500,
                              textTransform: 'uppercase',
                              letterSpacing: '0.05em'
                            }}>
                              Triggers
                            </div>
                            <div className="flex flex-wrap gap-1.5">
                              {skill.triggers.map((trigger) => (
                                <span
                                  key={trigger}
                                  className="px-2 py-1 rounded text-[11px]"
                                  style={{
                                    backgroundColor: 'var(--codex-bg)',
                                    color: 'var(--codex-fg-muted)',
                                    fontFamily: 'var(--font-mono)'
                                  }}
                                >
                                  {trigger}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}

                        <div>
                          <div className="text-[11px] mb-2" style={{
                            color: 'var(--codex-fg-subtle)',
                            fontWeight: 500,
                            textTransform: 'uppercase',
                            letterSpacing: '0.05em'
                          }}>
                            Requirements
                          </div>
                          {hasRequirements ? (
                            <div className="space-y-1">
                              {skill.requiresBins.length > 0 && (
                                <div className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                                  Binaries: {skill.requiresBins.map(b => (
                                    <span key={b} className="px-1.5 py-0.5 rounded mx-0.5" style={{
                                      backgroundColor: 'var(--codex-bg)',
                                      fontFamily: 'var(--font-mono)',
                                      fontSize: '11px',
                                    }}>
                                      {b}
                                    </span>
                                  ))}
                                </div>
                              )}
                              {skill.requiresEnv.length > 0 && (
                                <div className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                                  Env vars: {skill.requiresEnv.map(e => (
                                    <span key={e} className="px-1.5 py-0.5 rounded mx-0.5" style={{
                                      backgroundColor: 'var(--codex-bg)',
                                      fontFamily: 'var(--font-mono)',
                                      fontSize: '11px',
                                    }}>
                                      {e}
                                    </span>
                                  ))}
                                </div>
                              )}
                            </div>
                          ) : (
                            <div className="text-[12px]" style={{ color: '#10b981' }}>
                              None
                            </div>
                          )}
                        </div>

                        {skill.always && (
                          <div className="flex items-center gap-2">
                            <span className="px-2 py-1 rounded text-[11px]" style={{
                              backgroundColor: 'var(--codex-accent-dim)',
                              color: 'var(--codex-accent)',
                              border: '1px solid var(--codex-accent)'
                            }}>
                              Always loaded
                            </span>
                          </div>
                        )}

                        <div>
                          <button
                            onClick={() => setContentViewing(isViewingContent ? null : skill.name)}
                            className="text-[12px] transition-colors"
                            style={{ color: 'var(--codex-accent)' }}
                            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-accent-hover)'}
                            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-accent)'}
                          >
                            {isViewingContent ? 'Hide Content' : 'View Content'}
                          </button>
                        </div>

                        <AnimatePresence>
                          {isViewingContent && skill.content && (
                            <motion.div
                              initial={{ opacity: 0, height: 0 }}
                              animate={{ opacity: 1, height: 'auto' }}
                              exit={{ opacity: 0, height: 0 }}
                            >
                              <pre
                                className="mt-2 p-4 rounded-lg text-[12px] overflow-x-auto"
                                style={{
                                  backgroundColor: 'var(--codex-bg)',
                                  color: 'var(--codex-fg-muted)',
                                  fontFamily: 'var(--font-mono)',
                                  border: '1px solid var(--codex-border)',
                                  maxHeight: '400px',
                                  overflowY: 'auto',
                                  whiteSpace: 'pre-wrap',
                                  wordBreak: 'break-word',
                                }}
                              >
                                {skill.content}
                              </pre>
                            </motion.div>
                          )}
                        </AnimatePresence>
                      </>
                    )}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>
        </div>
      </motion.div>
    );
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="border-b px-8 py-6" style={{
        borderColor: 'var(--codex-border-subtle)',
        backgroundColor: 'var(--codex-bg)'
      }}>
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl mb-1" style={{
              color: 'var(--codex-fg)',
              fontWeight: 400
            }}>
              Skills
            </h1>
            <div className="flex items-center gap-4 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
              <span>{totalSkills} total</span>
              <span style={{ color: 'var(--codex-accent)' }}>{activeSkills} active</span>
              {unavailableSkills > 0 && (
                <span style={{ color: '#ef4444' }}>{unavailableSkills} unavailable</span>
              )}
              <span>{builtInSkills.length} built-in</span>
              {workspaceSkills.length > 0 && (
                <span>{workspaceSkills.length} workspace</span>
              )}
            </div>
          </div>

          <button
            onClick={() => setCreating(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg transition-colors text-[14px]"
            style={{
              backgroundColor: 'var(--codex-accent)',
              color: 'white'
            }}
            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
          >
            <Plus className="w-4 h-4" strokeWidth={1.5} />
            Create Skill
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-8 py-6" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="space-y-3">
          {/* Create skill form */}
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
                }}
              >
                <div className="flex items-center justify-between mb-4">
                  <h2 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                    Create Workspace Skill
                  </h2>
                  <button
                    onClick={() => { setCreating(false); setCreateError(null); }}
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    <X className="w-4 h-4" strokeWidth={1.5} />
                  </button>
                </div>

                <div className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                        color: 'var(--codex-fg-subtle)', fontWeight: 500
                      }}>
                        Name *
                      </label>
                      <input
                        value={newName}
                        onChange={(e) => setNewName(e.target.value)}
                        placeholder="my-skill"
                        className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                        style={{
                          backgroundColor: 'var(--codex-bg)',
                          color: 'var(--codex-fg)',
                          border: '1px solid var(--codex-border)',
                          fontFamily: 'var(--font-mono)',
                        }}
                        onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                        onBlur={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                        color: 'var(--codex-fg-subtle)', fontWeight: 500
                      }}>
                        Triggers
                      </label>
                      <input
                        value={newTriggers}
                        onChange={(e) => setNewTriggers(e.target.value)}
                        placeholder="keyword1, keyword2"
                        className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                        style={{
                          backgroundColor: 'var(--codex-bg)',
                          color: 'var(--codex-fg)',
                          border: '1px solid var(--codex-border)',
                          fontFamily: 'var(--font-mono)',
                        }}
                        onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                        onBlur={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                      color: 'var(--codex-fg-subtle)', fontWeight: 500
                    }}>
                      Description
                    </label>
                    <input
                      value={newDescription}
                      onChange={(e) => setNewDescription(e.target.value)}
                      placeholder="What this skill does and when to use it"
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg)',
                        border: '1px solid var(--codex-border)',
                      }}
                      onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                      onBlur={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                    />
                  </div>

                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                      color: 'var(--codex-fg-subtle)', fontWeight: 500
                    }}>
                      Content (Markdown instructions)
                    </label>
                    <textarea
                      value={newContent}
                      onChange={(e) => setNewContent(e.target.value)}
                      placeholder="# My Skill&#10;&#10;Instructions for the agent..."
                      rows={8}
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors resize-y"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg)',
                        border: '1px solid var(--codex-border)',
                        fontFamily: 'var(--font-mono)',
                      }}
                      onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                      onBlur={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                    />
                  </div>

                  <div className="flex items-center gap-3">
                    <button
                      onClick={() => setNewAlways(!newAlways)}
                      className="w-9 h-5 rounded-full relative transition-all flex-shrink-0"
                      style={{
                        backgroundColor: newAlways ? 'var(--codex-accent)' : '#333',
                      }}
                    >
                      <div className="w-4 h-4 bg-white rounded-full absolute top-0.5 transition-all" style={{
                        left: newAlways ? '18px' : '2px'
                      }} />
                    </button>
                    <span className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                      Always loaded (inject full content into every system prompt)
                    </span>
                  </div>

                  {createError && (
                    <div className="text-[12px] flex items-center gap-1.5" style={{ color: '#ef4444' }}>
                      <AlertTriangle className="w-3.5 h-3.5" strokeWidth={1.5} />
                      {createError}
                    </div>
                  )}

                  <div className="flex justify-end gap-3 pt-2">
                    <button
                      onClick={() => { setCreating(false); setCreateError(null); }}
                      className="px-4 py-2 rounded-lg text-[13px] transition-colors"
                      style={{
                        color: 'var(--codex-fg-muted)',
                        backgroundColor: 'transparent',
                        border: '1px solid var(--codex-border)',
                      }}
                      onMouseEnter={(e) => e.currentTarget.style.borderColor = 'var(--codex-fg-subtle)'}
                      onMouseLeave={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
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
                      onMouseEnter={(e) => !saving && (e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)')}
                      onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = 'var(--codex-accent)')}
                    >
                      {saving && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                      Create Skill
                    </button>
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* Empty state */}
          {skillList.length === 0 && !loading && (
            <div className="text-center py-16 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
              No skills found. Install packs or create a custom skill.
            </div>
          )}

          {builtInSkills.map((skill) => renderSkillCard(skill))}

          {workspaceSkills.length > 0 && (
            <div className="pt-6">
              <h2 className="text-[10px] uppercase tracking-wider mb-3" style={{
                color: 'var(--codex-fg-subtle)',
                fontWeight: 500
              }}>
                Workspace Skills
              </h2>
              <div className="space-y-3">
                {workspaceSkills.map((skill) => renderSkillCard(skill))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
