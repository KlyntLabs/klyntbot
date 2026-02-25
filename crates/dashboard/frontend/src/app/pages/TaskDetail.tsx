// TODO: Wire write operations:
//   - Status changes: PATCH /api/tasks/:id { status }
//   - Priority changes: PATCH /api/tasks/:id { priority }
//   - Title edit: PATCH /api/tasks/:id { title }
//   - Description edit: PATCH /api/tasks/:id { description }
//   - Focus/unfocus: POST/DELETE /api/tasks/:id/focus
//   - Timer start/stop: POST /api/tasks/:id/time-entries
//   - Tag add/remove: PATCH /api/tasks/:id { tags }
import { useNavigate, useParams } from 'react-router';
import {
  ArrowLeft,
  Calendar as CalendarIcon,
  Plus,
  Play,
  Pause,
  Zap,
  Circle,
  Bold,
  Italic,
  Strikethrough,
  Code,
  Heading2,
  List,
  ListOrdered,
  CheckSquare,
  Link2,
  FileCode,
  FileText,
  Link,
  StickyNote,
  Loader2,
  AlertTriangle
} from 'lucide-react';
import { useState, useMemo } from 'react';
import { useApi } from '../../lib/hooks/useApi';
import type { Task, TaskAttachment, TaskTimeEntry, Project } from '../../lib/types';

export default function TaskDetail() {
  const { id } = useParams();
  const navigate = useNavigate();
  const [isTimerRunning, setIsTimerRunning] = useState(false);

  const { data: task, loading, error } = useApi<Task>(`/api/tasks/${id}`);
  const { data: subtasks } = useApi<Task[]>(`/api/tasks/${id}/subtasks`);
  const { data: attachments } = useApi<TaskAttachment[]>(`/api/tasks/${id}/attachments`);
  const { data: timeEntries } = useApi<TaskTimeEntry[]>(`/api/tasks/${id}/time-entries`);
  const { data: projects } = useApi<Project[]>('/api/projects');

  // Build project lookup map
  const projectMap = useMemo(() => {
    const map = new Map<string, Project>();
    if (projects) {
      for (const p of projects) map.set(p.id, p);
    }
    return map;
  }, [projects]);

  const project = task?.projectId ? projectMap.get(task.projectId) : undefined;

  const getPriorityColor = (priority: number | null) => {
    switch (priority) {
      case 1: return '#e55050';
      case 2: return '#d4a017';
      case 3: return '#e5c07b';
      case 4: return '#666';
      default: return '#666';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status.toLowerCase()) {
      case 'done':
        return <Circle className="w-4 h-4 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      case 'doing':
        return <Circle className="w-4 h-4 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      default:
        return <Circle className="w-4 h-4" strokeWidth={1.5} style={{ color: '#666' }} />;
    }
  };

  const getAttachmentIcon = (attachmentType: string) => {
    switch (attachmentType) {
      case 'File': return <FileText className="w-4 h-4" strokeWidth={1.5} />;
      case 'URL': return <Link className="w-4 h-4" strokeWidth={1.5} />;
      case 'Note': return <StickyNote className="w-4 h-4" strokeWidth={1.5} />;
      default: return <FileText className="w-4 h-4" strokeWidth={1.5} />;
    }
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
  };

  const formatDateTime = (isoStr: string) => {
    const date = new Date(isoStr);
    const datePart = date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    const timePart = date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
    return `${datePart} ${timePart}`;
  };

  const formatDuration = (secs: number) => {
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    const remainMins = mins % 60;
    return remainMins > 0 ? `${hours}h ${remainMins}m` : `${hours}h`;
  };

  const statusDisplayValue = (status: string) => {
    // Capitalize first letter for display in select
    return status.charAt(0).toUpperCase() + status.slice(1).toLowerCase();
  };

  if (loading && !task) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <Loader2 className="w-5 h-5 animate-spin" style={{ color: 'var(--codex-fg-subtle)' }} />
      </div>
    );
  }

  if (error && !task) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2" style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg-subtle)' }}>
        <AlertTriangle className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px]">Failed to load task</span>
      </div>
    );
  }

  if (!task) return null;

  const subtaskList = subtasks ?? [];
  const attachmentList = attachments ?? [];
  const timeEntryList = timeEntries ?? [];
  const statusDisplay = statusDisplayValue(task.status);
  const isStatusDoing = task.status.toLowerCase() === 'doing';

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header Bar */}
        <div className="border-b px-6 py-4" style={{
          borderColor: 'var(--codex-border-subtle)',
          backgroundColor: 'var(--codex-bg)'
        }}>
          <div className="flex items-center gap-4">
            <button
              onClick={() => navigate('/tasks')}
              className="flex items-center gap-2 text-[13px] transition-colors"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg)'}
              onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
            >
              <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
              Tasks
            </button>

            <div className="text-[13px]" style={{
              color: '#10a37f',
              fontFamily: 'var(--font-mono)'
            }}>
              {task.id}
            </div>

            {/* TODO: wire onChange to PATCH /api/tasks/:id { status } */}
            <select
              defaultValue={statusDisplay}
              className="px-3 py-1.5 rounded text-[12px] outline-none cursor-pointer"
              style={{
                backgroundColor: isStatusDoing ? 'rgba(16, 163, 127, 0.2)' : '#141414',
                color: isStatusDoing ? '#10a37f' : 'var(--codex-fg-subtle)',
                border: `1px solid ${isStatusDoing ? '#10a37f' : 'var(--codex-border)'}`
              }}
            >
              <option>Todo</option>
              <option>Doing</option>
              <option>Done</option>
              <option>Archived</option>
            </select>

            {task.priority != null && (
              <div className="flex items-center gap-2 px-3 py-1.5 rounded text-[12px]" style={{
                backgroundColor: getPriorityColor(task.priority) + '20',
                color: getPriorityColor(task.priority),
                border: `1px solid ${getPriorityColor(task.priority)}40`
              }}>
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(task.priority) }} />
                P{task.priority}
              </div>
            )}
          </div>
        </div>

        {/* Content Area */}
        <div className="flex-1 overflow-y-auto p-8" style={{ backgroundColor: 'var(--codex-bg)' }}>
          <div className="max-w-4xl mx-auto">
            {/* TODO: wire onBlur to PATCH /api/tasks/:id { title } */}
            <input
              type="text"
              defaultValue={task.title}
              className="w-full text-2xl font-bold mb-6 bg-transparent border-none outline-none"
              style={{ color: 'var(--codex-fg)' }}
              placeholder="Task title..."
            />

            {/* WYSIWYG Editor */}
            <div className="mb-8">
              <div className="rounded-lg border" style={{
                backgroundColor: '#141414',
                borderColor: '#1a1a1a'
              }}>
                <div className="flex items-center gap-1 p-2 border-b" style={{ borderColor: '#1a1a1a' }}>
                  {[
                    { icon: Bold, title: 'Bold' },
                    { icon: Italic, title: 'Italic' },
                    { icon: Strikethrough, title: 'Strikethrough' },
                    { icon: Code, title: 'Code' },
                  ].map(({ icon: Icon, title }) => (
                    <button
                      key={title}
                      className="p-2 rounded transition-colors"
                      style={{ color: 'var(--codex-fg-subtle)' }}
                      onMouseEnter={(e) => e.currentTarget.style.color = '#10a37f'}
                      onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                      title={title}
                    >
                      <Icon className="w-4 h-4" strokeWidth={1.5} />
                    </button>
                  ))}
                  <div className="w-px h-6 mx-1" style={{ backgroundColor: '#1a1a1a' }} />
                  {[
                    { icon: Heading2, title: 'Heading' },
                    { icon: List, title: 'Bullet List' },
                    { icon: ListOrdered, title: 'Numbered List' },
                    { icon: CheckSquare, title: 'Checklist' },
                  ].map(({ icon: Icon, title }) => (
                    <button
                      key={title}
                      className="p-2 rounded transition-colors"
                      style={{ color: 'var(--codex-fg-subtle)' }}
                      onMouseEnter={(e) => e.currentTarget.style.color = '#10a37f'}
                      onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                      title={title}
                    >
                      <Icon className="w-4 h-4" strokeWidth={1.5} />
                    </button>
                  ))}
                  <div className="w-px h-6 mx-1" style={{ backgroundColor: '#1a1a1a' }} />
                  {[
                    { icon: Link2, title: 'Link' },
                    { icon: FileCode, title: 'Code Block' },
                  ].map(({ icon: Icon, title }) => (
                    <button
                      key={title}
                      className="p-2 rounded transition-colors"
                      style={{ color: 'var(--codex-fg-subtle)' }}
                      onMouseEnter={(e) => e.currentTarget.style.color = '#10a37f'}
                      onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                      title={title}
                    >
                      <Icon className="w-4 h-4" strokeWidth={1.5} />
                    </button>
                  ))}
                </div>

                {/* TODO: wire contentEditable onBlur to PATCH /api/tasks/:id { description } */}
                <div
                  className="p-4 min-h-[400px] prose prose-invert max-w-none"
                  style={{
                    color: 'var(--codex-fg)',
                    fontSize: '14px',
                    lineHeight: '1.6'
                  }}
                  dangerouslySetInnerHTML={{ __html: task.description ?? '' }}
                />
              </div>
            </div>

            {/* Attachments */}
            {attachmentList.length > 0 && (
              <div className="mb-8">
                <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                  Attachments
                </h3>
                <div className="grid grid-cols-2 gap-3">
                  {attachmentList.map(att => (
                    <div
                      key={att.id}
                      className="p-3 rounded-lg border cursor-pointer transition-colors"
                      style={{
                        backgroundColor: '#141414',
                        borderColor: 'var(--codex-border)'
                      }}
                      onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
                      onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#141414'}
                    >
                      <div className="flex items-start gap-3">
                        <div style={{ color: 'var(--codex-fg-subtle)' }}>
                          {getAttachmentIcon(att.attachmentType)}
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)' }}>
                            {att.title ?? att.attachmentType}
                          </div>
                          <div className="text-[11px] truncate" style={{
                            color: 'var(--codex-fg-subtle)',
                            fontFamily: 'var(--font-mono)'
                          }}>
                            {att.value}
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Subtasks */}
            {subtaskList.length > 0 && (
              <div className="mb-8">
                <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                  Subtasks
                </h3>
                <div className="space-y-2">
                  {subtaskList.map(subtask => (
                    <div
                      key={subtask.id}
                      className="flex items-center gap-3 p-3 rounded-lg border cursor-pointer"
                      style={{
                        backgroundColor: '#141414',
                        borderColor: 'var(--codex-border)'
                      }}
                      onClick={() => navigate(`/tasks/${subtask.id}`)}
                    >
                      {getStatusIcon(subtask.status)}
                      <div className="flex-1 text-[13px]" style={{
                        color: 'var(--codex-fg)',
                        textDecoration: subtask.status.toLowerCase() === 'done' ? 'line-through' : 'none'
                      }}>
                        {subtask.title}
                      </div>
                      {subtask.priority != null && (
                        <div className="flex items-center gap-1.5 text-[11px]">
                          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(subtask.priority) }} />
                          <span style={{ color: getPriorityColor(subtask.priority) }}>P{subtask.priority}</span>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Right Panel */}
      <aside className="w-[420px] border-l overflow-y-auto" style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)'
      }}>
        <div className="p-6 space-y-6">
          {/* Properties Card */}
          <div className="p-4 rounded-lg" style={{
            backgroundColor: '#141414',
            border: '1px solid var(--codex-border)'
          }}>
            <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
              Properties
            </h3>
            <div className="space-y-3">
              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Status</span>
                {/* TODO: wire onChange to PATCH /api/tasks/:id { status } */}
                <select
                  defaultValue={statusDisplay}
                  className="px-2 py-1 rounded text-[11px] outline-none cursor-pointer"
                  style={{
                    backgroundColor: 'var(--codex-bg)',
                    borderColor: 'var(--codex-border)',
                    color: 'var(--codex-fg)',
                    border: '1px solid var(--codex-border)'
                  }}
                >
                  <option>Todo</option>
                  <option>Doing</option>
                  <option>Done</option>
                  <option>Archived</option>
                </select>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Priority</span>
                {/* TODO: wire onClick to PATCH /api/tasks/:id { priority } */}
                <div className="flex items-center gap-1.5">
                  {task.priority != null ? (
                    <>
                      <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(task.priority) }} />
                      <span style={{ color: getPriorityColor(task.priority) }}>P{task.priority}</span>
                    </>
                  ) : (
                    <span style={{ color: 'var(--codex-fg-subtle)' }}>None</span>
                  )}
                </div>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Due Date</span>
                {task.dueDate ? (
                  <div className="flex items-center gap-1.5" style={{ color: 'var(--codex-fg)' }}>
                    <CalendarIcon className="w-3.5 h-3.5" strokeWidth={1.5} />
                    {formatDate(task.dueDate)}
                  </div>
                ) : (
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>None</span>
                )}
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Estimated</span>
                <span style={{ color: 'var(--codex-fg)' }}>
                  {task.estimatedMinutes != null ? `${task.estimatedMinutes} min` : 'None'}
                </span>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Project</span>
                {project ? (
                  <div className="flex items-center gap-1.5">
                    <div className="w-2 h-2 rounded-full" style={{ backgroundColor: project.color }} />
                    <span style={{ color: 'var(--codex-fg)' }}>{project.name}</span>
                  </div>
                ) : (
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>None</span>
                )}
              </div>

              <div>
                <div className="text-[12px] mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>Tags</div>
                <div className="flex flex-wrap gap-1.5">
                  {task.tags.map(tag => (
                    <span
                      key={tag}
                      className="px-2 py-0.5 rounded text-[11px]"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg-subtle)',
                        border: '1px solid var(--codex-border)'
                      }}
                    >
                      {tag}
                    </span>
                  ))}
                  {/* TODO: wire to PATCH /api/tasks/:id { tags } */}
                  <button
                    className="px-2 py-0.5 rounded text-[11px] transition-colors"
                    style={{
                      backgroundColor: 'transparent',
                      color: 'var(--codex-fg-subtle)',
                      border: '1px solid var(--codex-border)'
                    }}
                    onMouseEnter={(e) => e.currentTarget.style.color = '#10a37f'}
                    onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                  >
                    <Plus className="w-3 h-3" strokeWidth={1.5} />
                  </button>
                </div>
              </div>

              <div className="flex justify-between text-[12px] pt-2 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Created</span>
                <span style={{ color: 'var(--codex-fg)' }}>{formatDate(task.createdAt)}</span>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Updated</span>
                <span style={{ color: 'var(--codex-fg)' }}>{formatDate(task.updatedAt)}</span>
              </div>
            </div>
          </div>

          {/* Dependencies Card */}
          <div className="p-4 rounded-lg" style={{
            backgroundColor: '#141414',
            border: '1px solid var(--codex-border)'
          }}>
            <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
              Dependencies
            </h3>
            <div className="space-y-3">
              <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                No dependencies
              </div>
            </div>
          </div>

          {/* Time Tracking Card */}
          <div className="p-4 rounded-lg" style={{
            backgroundColor: '#141414',
            border: '1px solid var(--codex-border)'
          }}>
            <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
              Time Tracking
            </h3>
            <div className="space-y-3">
              <div className="text-[18px]" style={{ color: '#10a37f', fontWeight: 500 }}>
                {formatDuration(task.totalTrackedSecs)}
              </div>
              {/* TODO: wire to POST /api/tasks/:id/time-entries (start) and PATCH to stop */}
              <button
                onClick={() => setIsTimerRunning(!isTimerRunning)}
                className="w-full flex items-center justify-center gap-2 py-2 rounded text-[13px] transition-colors"
                style={{
                  backgroundColor: isTimerRunning ? 'rgba(229, 80, 80, 0.2)' : 'rgba(16, 163, 127, 0.2)',
                  color: isTimerRunning ? '#e55050' : '#10a37f',
                  border: `1px solid ${isTimerRunning ? '#e5505040' : '#10a37f40'}`
                }}
              >
                {isTimerRunning ? (
                  <>
                    <Pause className="w-4 h-4" strokeWidth={1.5} />
                    Stop Timer
                  </>
                ) : (
                  <>
                    <Play className="w-4 h-4" strokeWidth={1.5} />
                    Start Timer
                  </>
                )}
              </button>
              {timeEntryList.length > 0 && (
                <div className="pt-3 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                  <div className="text-[11px] mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>Recent Entries</div>
                  {timeEntryList.map(entry => (
                    <div key={entry.id} className="text-[11px] mb-2">
                      <div className="flex items-center gap-2 mb-1">
                        <span style={{ color: 'var(--codex-fg-muted)' }}>
                          {formatDateTime(entry.startedAt)}
                        </span>
                        {entry.durationSecs != null && (
                          <span style={{
                            color: 'var(--codex-fg-subtle)',
                            fontFamily: 'var(--font-mono)'
                          }}>
                            {formatDuration(entry.durationSecs)}
                          </span>
                        )}
                        <span className="px-1.5 py-0.5 rounded text-[9px]" style={{
                          backgroundColor: entry.source === 'Focus' ? 'rgba(229, 192, 123, 0.2)' : 'var(--codex-bg)',
                          color: entry.source === 'Focus' ? '#e5c07b' : 'var(--codex-fg-subtle)',
                          border: `1px solid ${entry.source === 'Focus' ? '#e5c07b40' : 'var(--codex-border)'}`
                        }}>
                          {entry.source}
                        </span>
                      </div>
                      {entry.note && (
                        <div style={{ color: 'var(--codex-fg-subtle)' }}>
                          {entry.note}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Focus Card */}
          {task.focusedAt && (
            <div className="p-4 rounded-lg" style={{
              backgroundColor: '#141414',
              border: '1px solid var(--codex-border)'
            }}>
              <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                Focus
              </h3>
              <div className="space-y-3">
                <div className="flex justify-between text-[12px]">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>Focused since</span>
                  <span style={{ color: '#e5c07b' }}>
                    {formatDateTime(task.focusedAt)}
                  </span>
                </div>
                {task.focusDeadline && (
                  <div className="flex justify-between text-[12px]">
                    <span style={{ color: 'var(--codex-fg-subtle)' }}>Deadline</span>
                    <span style={{ color: 'var(--codex-fg)' }}>
                      {formatDateTime(task.focusDeadline)}
                    </span>
                  </div>
                )}
                {/* TODO: wire to DELETE /api/tasks/:id/focus */}
                <button
                  className="w-full flex items-center justify-center gap-2 py-2 rounded text-[13px] transition-colors"
                  style={{
                    backgroundColor: 'rgba(229, 192, 123, 0.2)',
                    color: '#e5c07b',
                    border: '1px solid #e5c07b40'
                  }}
                >
                  <Zap className="w-4 h-4" strokeWidth={1.5} />
                  Unfocus
                </button>
              </div>
            </div>
          )}

          {/* Calendar Card */}
          {task.calendarEventUid && (
            <div className="p-4 rounded-lg" style={{
              backgroundColor: '#141414',
              border: '1px solid var(--codex-border)'
            }}>
              <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                Calendar
              </h3>
              <div className="space-y-2">
                <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Linked to CalDAV event
                </div>
                <div className="text-[10px] p-2 rounded" style={{
                  backgroundColor: 'var(--codex-bg)',
                  color: 'var(--codex-fg-subtle)',
                  fontFamily: 'var(--font-mono)',
                  wordBreak: 'break-all'
                }}>
                  {task.calendarEventUid}
                </div>
              </div>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}
