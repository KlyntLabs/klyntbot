import { useState } from 'react';
import { Plus, ChevronDown, ChevronRight } from 'lucide-react';
import { motion } from 'motion/react';

type SkillSource = 'built-in' | 'workspace';
type SkillStatus = 'available' | 'missing';

type Skill = {
  id: string;
  name: string;
  description: string;
  version: string;
  source: SkillSource;
  status: SkillStatus;
  statusMessage?: string;
  enabled: boolean;
  icon: string;
  color: string;
  triggers?: string[];
  requirements?: string;
  alwaysLoaded?: boolean;
};

export default function Skills() {
  const [expandedSkill, setExpandedSkill] = useState<string | null>('todo');
  const [sessionOpen, setSessionOpen] = useState(true);
  const [tokenOpen, setTokenOpen] = useState(true);
  const [skillStatsOpen, setSkillStatsOpen] = useState(true);

  const [skills, setSkills] = useState<Skill[]>([
    {
      id: 'todo',
      name: 'todo',
      description: 'Task creation with ask-first workflow and enrichment',
      version: 'v1.0',
      source: 'built-in',
      status: 'available',
      enabled: true,
      icon: '\u2713',
      color: '#10a37f',
      triggers: ['todo', 'task', 'focus'],
      requirements: 'None',
      alwaysLoaded: true
    },
    {
      id: 'daily-planning',
      name: 'daily-planning',
      description: 'Morning planning and focus prioritization',
      version: 'v1.0',
      source: 'built-in',
      status: 'available',
      enabled: true,
      icon: '\u2600\uFE0F',
      color: '#fbbf24'
    },
    {
      id: 'finance',
      name: 'finance',
      description: 'Personal finance tracking, budgeting, investments',
      version: 'v1.0',
      source: 'built-in',
      status: 'available',
      enabled: true,
      icon: '$',
      color: '#10b981'
    },
    {
      id: 'cron',
      name: 'cron',
      description: 'Schedule reminders and recurring tasks',
      version: 'v1.0',
      source: 'built-in',
      status: 'available',
      enabled: true,
      icon: '\u23F0',
      color: '#3b82f6'
    },
    {
      id: 'skill-creator',
      name: 'skill-creator',
      description: 'Create and manage custom skills',
      version: 'v1.0',
      source: 'built-in',
      status: 'available',
      enabled: true,
      icon: '\u26A1',
      color: '#8b5cf6'
    },
    {
      id: 'summarize',
      name: 'summarize',
      description: 'Summarize URLs, files, and videos',
      version: 'v1.0',
      source: 'built-in',
      status: 'missing',
      statusMessage: 'Missing: summarize binary',
      enabled: false,
      icon: '\uD83D\uDCC4',
      color: '#6b7280'
    },
    {
      id: 'todo-party',
      name: 'todo-party',
      description: 'Interactive brainstorming for task creation',
      version: 'v1.0',
      source: 'built-in',
      status: 'available',
      enabled: true,
      icon: '\uD83C\uDF89',
      color: '#ec4899'
    },
    {
      id: 'weather',
      name: 'weather',
      description: 'Get current weather and forecasts',
      version: 'v1.0',
      source: 'built-in',
      status: 'missing',
      statusMessage: 'Missing: curl',
      enabled: false,
      icon: '\uD83C\uDF24\uFE0F',
      color: '#06b6d4'
    }
  ]);

  const [workspaceSkills, setWorkspaceSkills] = useState<Skill[]>([
    {
      id: 'weekly-report',
      name: 'weekly-report',
      description: 'Weekly summary generation',
      version: 'v1.0',
      source: 'workspace',
      status: 'available',
      enabled: true,
      icon: '\uD83D\uDCCA',
      color: '#f97316'
    }
  ]);

  const toggleSkill = (skillId: string, isWorkspace: boolean = false) => {
    if (isWorkspace) {
      setWorkspaceSkills(workspaceSkills.map(s =>
        s.id === skillId ? { ...s, enabled: !s.enabled } : s
      ));
    } else {
      setSkills(skills.map(s =>
        s.id === skillId ? { ...s, enabled: !s.enabled } : s
      ));
    }
  };

  const allSkills = [...skills, ...workspaceSkills];
  const totalSkills = allSkills.length;
  const activeSkills = allSkills.filter(s => s.enabled).length;
  const unavailableSkills = allSkills.filter(s => s.status === 'missing').length;

  const renderSkillCard = (skill: Skill, isWorkspace: boolean = false) => {
    const isExpanded = expandedSkill === skill.id;
    const canToggle = skill.status === 'available';

    return (
      <motion.div
        key={skill.id}
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
              style={{ backgroundColor: skill.color + '20', color: skill.color }}
            >
              {skill.icon}
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
                        backgroundColor: skill.status === 'available' ? '#10b981' : '#ef4444'
                      }} />
                      <span style={{
                        color: skill.status === 'available' ? '#10b981' : '#ef4444'
                      }}>
                        {skill.status === 'available' ? 'Available' : skill.statusMessage}
                      </span>
                    </div>
                  </div>
                </div>

                <button
                  onClick={() => canToggle && toggleSkill(skill.id, isWorkspace)}
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

              {skill.status === 'available' && (
                <button
                  onClick={() => setExpandedSkill(isExpanded ? null : skill.id)}
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

              {isExpanded && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: 'auto' }}
                  exit={{ opacity: 0, height: 0 }}
                  className="mt-4 pt-4 border-t space-y-3"
                  style={{ borderColor: 'var(--codex-border)' }}
                >
                  {skill.triggers && (
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

                  {skill.requirements && (
                    <div>
                      <div className="text-[11px] mb-2" style={{
                        color: 'var(--codex-fg-subtle)',
                        fontWeight: 500,
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em'
                      }}>
                        Requirements
                      </div>
                      <div className="text-[12px]" style={{
                        color: skill.requirements === 'None' ? '#10b981' : 'var(--codex-fg-muted)'
                      }}>
                        {skill.requirements}
                      </div>
                    </div>
                  )}

                  {skill.alwaysLoaded && (
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
                      className="text-[12px] transition-colors"
                      style={{ color: 'var(--codex-accent)' }}
                      onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-accent-hover)'}
                      onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-accent)'}
                    >
                      View Content
                    </button>
                  </div>
                </motion.div>
              )}
            </div>
          </div>
        </div>
      </motion.div>
    );
  };

  return (
    <>
      {/* Main Content Area */}
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
              <p className="text-[13px]" style={{ color: '#666' }}>
                Manage built-in and custom skills
              </p>
            </div>

            <button
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
          <div className="max-w-4xl space-y-3">
            {skills.map((skill) => renderSkillCard(skill))}

            <div className="pt-6">
              <h2 className="text-[10px] uppercase tracking-wider mb-3" style={{
                color: 'var(--codex-fg-subtle)',
                fontWeight: 500
              }}>
                Workspace Skills
              </h2>
              <div className="space-y-3">
                {workspaceSkills.map((skill) => renderSkillCard(skill, true))}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Right Sidebar */}
      <aside className="w-[260px] border-l overflow-y-auto" style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)'
      }}>
        <div className="border-b" style={{ borderColor: 'var(--codex-border-subtle)' }}>
          <button
            onClick={() => setSessionOpen(!sessionOpen)}
            className="w-full px-4 py-3 flex items-center justify-between transition-colors"
            style={{
              backgroundColor: 'transparent',
              color: 'var(--codex-fg-subtle)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
          >
            <span className="text-[10px] uppercase tracking-wider" style={{ fontWeight: 500 }}>
              Session Info
            </span>
            {sessionOpen ?
              <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} /> :
              <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
            }
          </button>
          {sessionOpen && (
            <div className="px-4 pb-4 space-y-3 text-[13px]">
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Session ID</span>
                <span style={{
                  color: 'var(--codex-fg)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '12px'
                }}>
                  #a8f32e
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Duration</span>
                <span style={{ color: 'var(--codex-fg)' }}>1h 24m</span>
              </div>
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Messages</span>
                <span style={{ color: 'var(--codex-fg)' }}>47</span>
              </div>
            </div>
          )}
        </div>

        <div className="border-b" style={{ borderColor: 'var(--codex-border-subtle)' }}>
          <button
            onClick={() => setTokenOpen(!tokenOpen)}
            className="w-full px-4 py-3 flex items-center justify-between transition-colors"
            style={{
              backgroundColor: 'transparent',
              color: 'var(--codex-fg-subtle)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
          >
            <span className="text-[10px] uppercase tracking-wider" style={{ fontWeight: 500 }}>
              Token Usage
            </span>
            {tokenOpen ?
              <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} /> :
              <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
            }
          </button>
          {tokenOpen && (
            <div className="px-4 pb-4 space-y-3">
              <div className="space-y-2">
                <div className="flex justify-between text-[13px] items-center">
                  <span style={{ color: 'var(--codex-fg)' }}>12.4K tokens</span>
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>8%</span>
                </div>
                <div className="h-1 rounded-full overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
                  <div className="h-full rounded-full transition-all" style={{
                    width: '8%',
                    backgroundColor: 'var(--codex-accent)'
                  }} />
                </div>
              </div>
              <div className="text-[13px] flex justify-between items-center pt-1">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Cost</span>
                <span style={{
                  color: 'var(--codex-accent)',
                  fontFamily: 'var(--font-mono)'
                }}>
                  $0.05
                </span>
              </div>
            </div>
          )}
        </div>

        <div>
          <button
            onClick={() => setSkillStatsOpen(!skillStatsOpen)}
            className="w-full px-4 py-3 flex items-center justify-between transition-colors"
            style={{
              backgroundColor: 'transparent',
              color: 'var(--codex-fg-subtle)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
          >
            <span className="text-[10px] uppercase tracking-wider" style={{ fontWeight: 500 }}>
              Skill Stats
            </span>
            {skillStatsOpen ?
              <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} /> :
              <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
            }
          </button>
          {skillStatsOpen && (
            <div className="px-4 pb-4 space-y-3 text-[13px]">
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Total skills</span>
                <span style={{ color: 'var(--codex-fg)' }}>{totalSkills}</span>
              </div>
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Active</span>
                <span style={{ color: 'var(--codex-accent)' }}>{activeSkills}</span>
              </div>
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Unavailable</span>
                <span style={{ color: '#ef4444' }}>{unavailableSkills}</span>
              </div>
              <div className="pt-2 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                <div className="text-[11px] mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Sources
                </div>
                <div className="space-y-1 text-[12px]">
                  <div className="flex justify-between">
                    <span style={{ color: 'var(--codex-fg-muted)' }}>Built-in</span>
                    <span style={{ color: 'var(--codex-fg)' }}>{skills.length}</span>
                  </div>
                  <div className="flex justify-between">
                    <span style={{ color: 'var(--codex-fg-muted)' }}>Workspace</span>
                    <span style={{ color: 'var(--codex-fg)' }}>{workspaceSkills.length}</span>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </aside>
    </>
  );
}
