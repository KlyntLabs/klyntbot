// TODO: Replace all mock data in this file with real API calls:
//   - Task detail: useApi<Task>('/api/tasks/:id')
//   - Subtasks: useApi<Task[]>('/api/tasks/:id/subtasks')
//   - Attachments: useApi<TaskAttachment[]>('/api/tasks/:id/attachments')
//   - Time entries: useApi<TaskTimeEntry[]>('/api/tasks/:id/time-entries')
//   - Project lookup: useApi<Project[]>('/api/projects')
//   - Status/priority changes: PATCH /api/tasks/:id
//   - Focus/unfocus: POST/DELETE /api/tasks/:id/focus
//   - Timer: POST /api/tasks/:id/time-entries
//   - Description edit: PATCH /api/tasks/:id { description }
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
  StickyNote
} from 'lucide-react';
import { useState } from 'react';

export default function TaskDetail() {
  const { id } = useParams();
  const navigate = useNavigate();
  const [isTimerRunning, setIsTimerRunning] = useState(false);

  const task = {
    id: '3f2a1b8c',
    title: 'Fix auth token refresh',
    description: `<h2>Overview</h2>
<p>We need to implement an automatic token refresh mechanism for expired JWT tokens. Currently, users are being logged out unexpectedly when their tokens expire.</p>

<h3>Steps to implement:</h3>
<ul>
  <li>Add token expiry check interceptor</li>
  <li>Implement refresh token endpoint call</li>
  <li>Update token storage mechanism</li>
  <li>Add retry logic for failed requests</li>
</ul>

<h3>JWT Example:</h3>
<pre><code>const refreshToken = async () => {
  const response = await fetch('/api/auth/refresh', {
    method: 'POST',
    headers: {
      'Authorization': \`Bearer \${refreshToken}\`
    }
  });
  return response.json();
};</code></pre>

<p>This will significantly improve the user experience and reduce support tickets related to unexpected logouts.</p>`,
    status: 'Doing',
    priority: 1,
    due_date: '2026-02-24',
    estimated_minutes: 60,
    project: 'Backend',
    projectColor: '#4a9eff',
    tags: ['backend', 'security'],
    created_at: '2026-02-20',
    updated_at: '2026-02-24',
    focused_at: '2026-02-24T14:00:00',
    focus_deadline: '2026-02-24T17:00:00',
    blocked_by: [{ id: '9d7e4f2a', title: 'Review PR #142' }],
    blocks: [] as { id: string; title: string }[],
    attachments: [
      { id: 'a1', type: 'URL', title: 'JWT Spec', value: 'https://jwt.io/introduction' },
      { id: 'a2', type: 'Note', title: 'Implementation Notes', value: 'Check existing middleware patterns' }
    ],
    subtasks: [
      { id: 's1', title: 'Add interceptor', status: 'Done', priority: 1 },
      { id: 's2', title: 'Implement refresh endpoint', status: 'Doing', priority: 1 },
      { id: 's3', title: 'Add retry logic', status: 'Todo', priority: 2 }
    ],
    time_entries: [
      { id: 't1', date: '2026-02-24', startTime: '14:00', endTime: '14:45', source: 'Focus', note: 'Working on auth' }
    ],
    total_tracked_secs: 2700,
    calendar_event_uid: 'auth-work-session@calendar'
  };

  const getPriorityColor = (priority: number) => {
    switch (priority) {
      case 1: return '#e55050';
      case 2: return '#d4a017';
      case 3: return '#e5c07b';
      case 4: return '#666';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'Done':
        return <Circle className="w-4 h-4 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      case 'Doing':
        return <Circle className="w-4 h-4 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      default:
        return <Circle className="w-4 h-4" strokeWidth={1.5} style={{ color: '#666' }} />;
    }
  };

  const getAttachmentIcon = (type: string) => {
    switch (type) {
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

            <select
              defaultValue={task.status}
              className="px-3 py-1.5 rounded text-[12px] outline-none cursor-pointer"
              style={{
                backgroundColor: task.status === 'Doing' ? 'rgba(16, 163, 127, 0.2)' : '#141414',
                color: task.status === 'Doing' ? '#10a37f' : 'var(--codex-fg-subtle)',
                border: `1px solid ${task.status === 'Doing' ? '#10a37f' : 'var(--codex-border)'}`
              }}
            >
              <option>Todo</option>
              <option>Doing</option>
              <option>Done</option>
              <option>Archived</option>
            </select>

            <div className="flex items-center gap-2 px-3 py-1.5 rounded text-[12px]" style={{
              backgroundColor: getPriorityColor(task.priority) + '20',
              color: getPriorityColor(task.priority),
              border: `1px solid ${getPriorityColor(task.priority)}40`
            }}>
              <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(task.priority) }} />
              P{task.priority}
            </div>
          </div>
        </div>

        {/* Content Area */}
        <div className="flex-1 overflow-y-auto p-8" style={{ backgroundColor: 'var(--codex-bg)' }}>
          <div className="max-w-4xl mx-auto">
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

                <div
                  className="p-4 min-h-[400px] prose prose-invert max-w-none"
                  style={{
                    color: 'var(--codex-fg)',
                    fontSize: '14px',
                    lineHeight: '1.6'
                  }}
                  dangerouslySetInnerHTML={{ __html: task.description }}
                />
              </div>
            </div>

            {/* Attachments */}
            {task.attachments.length > 0 && (
              <div className="mb-8">
                <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                  Attachments
                </h3>
                <div className="grid grid-cols-2 gap-3">
                  {task.attachments.map(att => (
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
                          {getAttachmentIcon(att.type)}
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)' }}>
                            {att.title}
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
            {task.subtasks.length > 0 && (
              <div className="mb-8">
                <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                  Subtasks
                </h3>
                <div className="space-y-2">
                  {task.subtasks.map(subtask => (
                    <div
                      key={subtask.id}
                      className="flex items-center gap-3 p-3 rounded-lg border"
                      style={{
                        backgroundColor: '#141414',
                        borderColor: 'var(--codex-border)'
                      }}
                    >
                      {getStatusIcon(subtask.status)}
                      <div className="flex-1 text-[13px]" style={{
                        color: 'var(--codex-fg)',
                        textDecoration: subtask.status === 'Done' ? 'line-through' : 'none'
                      }}>
                        {subtask.title}
                      </div>
                      <div className="flex items-center gap-1.5 text-[11px]">
                        <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(subtask.priority) }} />
                        <span style={{ color: getPriorityColor(subtask.priority) }}>P{subtask.priority}</span>
                      </div>
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
                <select
                  defaultValue={task.status}
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
                <div className="flex items-center gap-1.5">
                  <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(task.priority) }} />
                  <span style={{ color: getPriorityColor(task.priority) }}>P{task.priority}</span>
                </div>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Due Date</span>
                <div className="flex items-center gap-1.5" style={{ color: 'var(--codex-fg)' }}>
                  <CalendarIcon className="w-3.5 h-3.5" strokeWidth={1.5} />
                  {formatDate(task.due_date)}
                </div>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Estimated</span>
                <span style={{ color: 'var(--codex-fg)' }}>{task.estimated_minutes} min</span>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Project</span>
                <div className="flex items-center gap-1.5">
                  <div className="w-2 h-2 rounded-full" style={{ backgroundColor: task.projectColor }} />
                  <span style={{ color: 'var(--codex-fg)' }}>{task.project}</span>
                </div>
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
                <span style={{ color: 'var(--codex-fg)' }}>{formatDate(task.created_at)}</span>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Updated</span>
                <span style={{ color: 'var(--codex-fg)' }}>{formatDate(task.updated_at)}</span>
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
              {task.blocked_by.length > 0 && (
                <div>
                  <div className="text-[11px] mb-2" style={{ color: '#e55050' }}>Blocked by</div>
                  {task.blocked_by.map(dep => (
                    <button
                      key={dep.id}
                      onClick={() => navigate(`/tasks/${dep.id}`)}
                      className="w-full text-left p-2 rounded text-[12px] transition-colors"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg)',
                        border: '1px solid var(--codex-border)'
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = '#1a1a1a';
                        e.currentTarget.style.color = '#10a37f';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                        e.currentTarget.style.color = 'var(--codex-fg)';
                      }}
                    >
                      {dep.title}
                    </button>
                  ))}
                </div>
              )}
              {task.blocks.length === 0 && task.blocked_by.length === 0 && (
                <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                  No dependencies
                </div>
              )}
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
                {Math.floor(task.total_tracked_secs / 60)}m
              </div>
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
              {task.time_entries.length > 0 && (
                <div className="pt-3 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                  <div className="text-[11px] mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>Recent Entries</div>
                  {task.time_entries.map(entry => (
                    <div key={entry.id} className="text-[11px] mb-2">
                      <div className="flex items-center gap-2 mb-1">
                        <span style={{ color: 'var(--codex-fg-muted)' }}>
                          {formatDate(entry.date)}
                        </span>
                        <span style={{
                          color: 'var(--codex-fg-subtle)',
                          fontFamily: 'var(--font-mono)'
                        }}>
                          {entry.startTime}-{entry.endTime}
                        </span>
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
          {task.focused_at && (
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
                    {formatDate(task.focused_at.split('T')[0])} {task.focused_at.split('T')[1]?.slice(0, 5)}
                  </span>
                </div>
                <div className="flex justify-between text-[12px]">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>Deadline</span>
                  <span style={{ color: 'var(--codex-fg)' }}>
                    {formatDate(task.focus_deadline.split('T')[0])} {task.focus_deadline.split('T')[1]?.slice(0, 5)}
                  </span>
                </div>
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
          {task.calendar_event_uid && (
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
                  {task.calendar_event_uid}
                </div>
              </div>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}
