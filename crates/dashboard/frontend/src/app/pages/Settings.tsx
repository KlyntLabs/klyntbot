import { useState } from 'react';
import { 
  Sliders,
  Cpu,
  MessageCircle,
  Bot,
  Wrench,
  CheckSquare,
  Calendar as CalendarIcon,
  MessageSquare,
  Brain,
  DollarSign,
  Folder,
  Package,
  Eye,
  EyeOff,
  ChevronDown,
  ChevronRight,
  Plus,
  X,
  Plug
} from 'lucide-react';

type SettingsSection = {
  id: string;
  label: string;
  icon: typeof Sliders;
};

export default function Settings() {
  const [activeSection, setActiveSection] = useState('providers');
  const [activeProvider, setActiveProvider] = useState('anthropic');
  const [showApiKey, setShowApiKey] = useState(false);
  const [sessionOpen, setSessionOpen] = useState(true);
  const [tokenOpen, setTokenOpen] = useState(true);
  const [configOpen, setConfigOpen] = useState(true);

  const sections: SettingsSection[] = [
    { id: 'general', label: 'General', icon: Sliders },
    { id: 'providers', label: 'Providers', icon: Cpu },
    { id: 'channels', label: 'Channels', icon: MessageCircle },
    { id: 'agent-defaults', label: 'Agent Defaults', icon: Bot },
    { id: 'tools', label: 'Tools', icon: Wrench },
    { id: 'tasks-todo', label: 'Tasks & Todo', icon: CheckSquare },
    { id: 'calendar', label: 'Calendar', icon: CalendarIcon },
    { id: 'conversation', label: 'Conversation', icon: MessageSquare },
    { id: 'learning', label: 'Learning', icon: Brain },
    { id: 'confidence', label: 'Confidence', icon: Brain },
    { id: 'finance', label: 'Finance', icon: DollarSign },
    { id: 'projects', label: 'Projects', icon: Folder },
    { id: 'packs-skills', label: 'Packs & Skills', icon: Package },
    { id: 'plugins', label: 'Plugins', icon: Plug },
  ];

  const providers = ['Anthropic', 'OpenAI', 'OpenRouter', 'DeepSeek', 'Gemini', 'Groq', 'vLLM', 'Zhipu', 'DashScope', 'Moonshot', 'MiniMax', 'AIHubMix'];
  const channels = ['Telegram', 'Discord', 'WhatsApp', 'Slack', 'Email', 'QQ', 'Feishu', 'DingTalk', 'Mochat'];

  const [activeChannel, setActiveChannel] = useState('telegram');
  const [extendedThinking, setExtendedThinking] = useState(false);
  const [temperature, setTemperature] = useState(0.7);
  const [confidenceThreshold, setConfidenceThreshold] = useState(0.7);

  const FormCard = ({ children, title, description }: any) => (
    <div className="p-5 rounded-lg mb-4" style={{
      backgroundColor: '#141414',
      border: '1px solid var(--codex-border)'
    }}>
      {title && (
        <>
          <label className="text-[13px] block mb-1" style={{ 
            color: 'var(--codex-fg)',
            fontWeight: 500
          }}>
            {title}
          </label>
          {description && (
            <p className="text-[12px] mb-3" style={{ color: '#666' }}>
              {description}
            </p>
          )}
        </>
      )}
      {children}
    </div>
  );

  const TextInput = ({ value, placeholder, secret = false }: any) => (
    <input
      type={secret ? 'password' : 'text'}
      defaultValue={value}
      placeholder={placeholder}
      className="w-full px-3 py-2 rounded border outline-none text-[13px]"
      style={{
        backgroundColor: 'var(--codex-bg)',
        borderColor: 'var(--codex-border)',
        color: 'var(--codex-fg)',
        fontFamily: secret ? 'var(--font-mono)' : 'var(--font-ui)'
      }}
    />
  );

  const Toggle = ({ checked, onChange }: any) => (
    <button
      onClick={onChange}
      className="w-11 h-6 rounded-full relative transition-all flex-shrink-0"
      style={{
        backgroundColor: checked ? 'var(--codex-accent)' : '#333'
      }}
    >
      <div className="w-5 h-5 bg-white rounded-full absolute top-0.5 transition-all" style={{
        left: checked ? '22px' : '2px'
      }} />
    </button>
  );

  return (
    <>
      {/* Main Content Area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Settings Navigation */}
        <nav className="w-[220px] border-r overflow-y-auto" style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderColor: 'var(--codex-border-subtle)'
        }}>
          <div className="p-4">
            <h2 className="text-[10px] uppercase tracking-wider mb-3 px-3" style={{ 
              color: 'var(--codex-fg-subtle)',
              fontWeight: 500
            }}>
              Settings
            </h2>
            <div className="space-y-0.5">
              {sections.map((section) => {
                const isActive = activeSection === section.id;
                return (
                  <button
                    key={section.id}
                    onClick={() => setActiveSection(section.id)}
                    className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-[13px] transition-all relative"
                    style={{
                      color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
                      backgroundColor: isActive ? 'var(--codex-accent-dim)' : 'transparent'
                    }}
                    onMouseEnter={(e) => {
                      if (!isActive) e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                    }}
                    onMouseLeave={(e) => {
                      if (!isActive) e.currentTarget.style.backgroundColor = 'transparent';
                    }}
                  >
                    {isActive && (
                      <div className="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 rounded-r" style={{
                        backgroundColor: 'var(--codex-accent)'
                      }} />
                    )}
                    <section.icon className="w-4 h-4" strokeWidth={1.5} />
                    {section.label}
                  </button>
                );
              })}
            </div>
          </div>
        </nav>

        {/* Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Header */}
          <div className="border-b px-8 py-6" style={{ 
            borderColor: 'var(--codex-border-subtle)',
            backgroundColor: 'var(--codex-bg)'
          }}>
            <h1 className="text-xl mb-1" style={{ 
              color: 'var(--codex-fg)',
              fontWeight: 400
            }}>
              {sections.find(s => s.id === activeSection)?.label}
            </h1>
            <p className="text-[13px]" style={{ color: '#666' }}>
              {activeSection === 'providers' && 'Configure AI provider connections'}
              {activeSection === 'general' && 'General application settings'}
              {activeSection === 'channels' && 'Configure chat channel integrations'}
              {activeSection === 'agent-defaults' && 'Default agent behavior settings'}
              {activeSection === 'tools' && 'Tool permissions and configuration'}
              {activeSection === 'tasks-todo' && 'Task management preferences'}
              {activeSection === 'calendar' && 'Calendar synchronization settings'}
              {activeSection === 'conversation' && 'Conversation history and memory'}
              {activeSection === 'learning' && 'Agent learning configuration'}
              {activeSection === 'confidence' && 'Confidence threshold settings'}
              {activeSection === 'finance' && 'Financial tracking configuration'}
              {activeSection === 'projects' && 'Project management settings'}
              {activeSection === 'packs-skills' && 'Feature packs and skills'}
              {activeSection === 'plugins' && 'Plugin system configuration'}
            </p>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto px-8 py-6" style={{ backgroundColor: 'var(--codex-bg)' }}>
            <div className="max-w-3xl">
              
              {/* GENERAL */}
              {activeSection === 'general' && (
                <>
                  <FormCard title="Timezone" description="Default timezone for the application">
                    <TextInput value="UTC" placeholder="UTC" />
                  </FormCard>
                  <FormCard title="Data Directory" description="Where klyntbot stores its data">
                    <TextInput value="~/.klyntbot" placeholder="~/.klyntbot" />
                  </FormCard>
                  <FormCard title="Gateway Host" description="API gateway host address">
                    <TextInput value="0.0.0.0" placeholder="0.0.0.0" />
                  </FormCard>
                  <FormCard title="Gateway Port" description="API gateway port number">
                    <TextInput value="18790" placeholder="18790" />
                  </FormCard>
                </>
              )}

              {/* PROVIDERS */}
              {activeSection === 'providers' && (
                <>
                  {/* Provider Tabs */}
                  <div className="mb-6 overflow-x-auto">
                    <div className="flex gap-4 pb-2 min-w-max">
                      {providers.map((provider) => {
                        const isActive = provider.toLowerCase() === activeProvider;
                        return (
                          <button
                            key={provider}
                            onClick={() => setActiveProvider(provider.toLowerCase())}
                            className="px-3 py-2 text-[13px] relative transition-colors whitespace-nowrap"
                            style={{
                              color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)'
                            }}
                            onMouseEnter={(e) => {
                              if (!isActive) e.currentTarget.style.color = 'var(--codex-fg-muted)';
                            }}
                            onMouseLeave={(e) => {
                              if (!isActive) e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                            }}
                          >
                            {provider}
                            {isActive && (
                              <div className="absolute bottom-0 left-0 right-0 h-[2px]" style={{
                                backgroundColor: 'var(--codex-accent)'
                              }} />
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <FormCard>
                    <div className="flex items-start justify-between mb-3">
                      <div>
                        <label className="text-[13px] block mb-1" style={{ 
                          color: 'var(--codex-fg)',
                          fontWeight: 500
                        }}>
                          API Key
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Your {activeProvider} API key for authentication
                        </p>
                      </div>
                      <button
                        className="px-3 py-1.5 rounded text-[12px] transition-colors"
                        style={{
                          backgroundColor: 'var(--codex-accent)',
                          color: 'white'
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
                      >
                        Save
                      </button>
                    </div>
                    <div className="relative">
                      <input
                        type={showApiKey ? 'text' : 'password'}
                        defaultValue="••••••••••••••••••••••••••••••••"
                        className="w-full px-3 py-2 rounded border outline-none text-[13px] pr-10"
                        style={{
                          backgroundColor: 'var(--codex-bg)',
                          borderColor: 'var(--codex-border)',
                          color: 'var(--codex-fg)',
                          fontFamily: 'var(--font-mono)'
                        }}
                      />
                      <button
                        onClick={() => setShowApiKey(!showApiKey)}
                        className="absolute right-2 top-1/2 -translate-y-1/2 p-1"
                        style={{ color: 'var(--codex-fg-subtle)' }}
                      >
                        {showApiKey ? (
                          <EyeOff className="w-4 h-4" strokeWidth={1.5} />
                        ) : (
                          <Eye className="w-4 h-4" strokeWidth={1.5} />
                        )}
                      </button>
                    </div>
                  </FormCard>

                  <FormCard title="API Base URL" description="Custom API endpoint">
                    <TextInput value={`https://api.${activeProvider}.com`} />
                  </FormCard>

                  <FormCard title="Extra Headers" description="Additional HTTP headers for API requests">
                    <div className="space-y-2">
                      <div className="flex gap-2">
                        <TextInput placeholder="Header name" />
                        <TextInput placeholder="Header value" />
                        <button className="px-3 py-2 rounded border" style={{ borderColor: 'var(--codex-border)', color: 'var(--codex-fg-subtle)' }}>
                          <X className="w-4 h-4" strokeWidth={1.5} />
                        </button>
                      </div>
                      <button className="flex items-center gap-2 px-3 py-2 rounded text-[12px]" style={{ color: 'var(--codex-accent)' }}>
                        <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                        Add Header
                      </button>
                    </div>
                  </FormCard>

                  <FormCard>
                    <div className="flex items-start justify-between">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ 
                          color: 'var(--codex-fg)',
                          fontWeight: 500
                        }}>
                          Native Mode
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Use provider's native API format
                        </p>
                      </div>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <FormCard>
                    <div className="flex items-start justify-between mb-3">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ 
                          color: 'var(--codex-fg)',
                          fontWeight: 500
                        }}>
                          Extended Thinking
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Allow the model to think longer for complex tasks
                        </p>
                      </div>
                      <Toggle checked={extendedThinking} onChange={() => setExtendedThinking(!extendedThinking)} />
                    </div>
                    {extendedThinking && (
                      <div className="space-y-3 pt-3 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                        <div>
                          <label className="text-[12px] block mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                            Budget Tokens
                          </label>
                          <TextInput value="10000" placeholder="10000" />
                        </div>
                        <div>
                          <label className="text-[12px] block mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                            Use For
                          </label>
                          <TextInput placeholder="Complex reasoning tasks" />
                        </div>
                      </div>
                    )}
                  </FormCard>

                  <FormCard title="API Version">
                    <TextInput value="2023-06-01" placeholder="2023-06-01" />
                  </FormCard>

                  {/* Routing */}
                  <div className="mt-8 mb-4">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                      Routing
                    </h3>
                  </div>

                  <FormCard title="Primary Provider">
                    <select className="w-full px-3 py-2 rounded border text-[13px] appearance-none" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>Anthropic</option>
                      <option>OpenAI</option>
                      <option>OpenRouter</option>
                    </select>
                  </FormCard>

                  <FormCard title="Fallback Provider">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>OpenAI</option>
                      <option>Anthropic</option>
                      <option>Groq</option>
                    </select>
                  </FormCard>

                  <FormCard title="Classifier Model">
                    <TextInput value="gpt-4o-mini" placeholder="gpt-4o-mini" />
                  </FormCard>
                </>
              )}

              {/* CHANNELS */}
              {activeSection === 'channels' && (
                <>
                  <div className="mb-6 overflow-x-auto">
                    <div className="flex gap-4 pb-2 min-w-max">
                      {channels.map((channel) => {
                        const isActive = channel.toLowerCase() === activeChannel;
                        return (
                          <button
                            key={channel}
                            onClick={() => setActiveChannel(channel.toLowerCase())}
                            className="px-3 py-2 text-[13px] relative transition-colors whitespace-nowrap"
                            style={{
                              color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)'
                            }}
                          >
                            {channel}
                            {isActive && (
                              <div className="absolute bottom-0 left-0 right-0 h-[2px]" style={{
                                backgroundColor: 'var(--codex-accent)'
                              }} />
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <FormCard>
                    <div className="flex items-center justify-between mb-4">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                        Enabled
                      </label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <FormCard title="Token" description="Bot authentication token">
                    <TextInput value="••••••••••••••••" secret />
                  </FormCard>

                  <FormCard title="Allow From" description="Allowed user IDs (comma-separated)">
                    <TextInput placeholder="user1, user2, user3" />
                  </FormCard>

                  <FormCard title="Proxy" description="Proxy URL for connections">
                    <TextInput placeholder="socks5://127.0.0.1:1080" />
                  </FormCard>
                </>
              )}

              {/* AGENT DEFAULTS */}
              {activeSection === 'agent-defaults' && (
                <>
                  <FormCard title="Model" description="Default model identifier">
                    <TextInput value="anthropic/claude-opus-4-5" />
                  </FormCard>
                  <FormCard title="Workspace" description="Agent workspace directory">
                    <TextInput value="~/.klyntbot/workspace" />
                  </FormCard>
                  <FormCard title="Provider" description="Override default provider (optional)">
                    <TextInput placeholder="anthropic" />
                  </FormCard>
                  <FormCard title="Max Tokens">
                    <TextInput value="8192" />
                  </FormCard>
                  <FormCard title="Temperature" description="Model creativity (0-1)">
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.1"
                      value={temperature}
                      onChange={(e) => setTemperature(parseFloat(e.target.value))}
                      className="w-full"
                    />
                    <div className="flex justify-between text-[12px] mt-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                      <span>0</span>
                      <span style={{ color: 'var(--codex-accent)' }}>{temperature}</span>
                      <span>1</span>
                    </div>
                  </FormCard>
                  <FormCard title="Max Tool Iterations">
                    <TextInput value="20" />
                  </FormCard>
                  <FormCard title="Max Concurrent Subagents">
                    <TextInput value="3" />
                  </FormCard>
                </>
              )}

              {/* TOOLS */}
              {activeSection === 'tools' && (
                <>
                  <div className="mb-4">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Web</h3>
                  </div>
                  <FormCard title="Brave API Key">
                    <TextInput secret value="••••••••••••" />
                  </FormCard>
                  <FormCard title="Max Results">
                    <TextInput value="5" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Browser</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-4">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Trust Level">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>strict</option>
                      <option>autonomous</option>
                      <option>full</option>
                    </select>
                  </FormCard>
                  <FormCard title="Session Timeout (seconds)">
                    <TextInput value="300" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Permissions</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                          Restrict to Workspace
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Limit file operations to workspace directory
                        </p>
                      </div>
                      <Toggle checked={false} onChange={() => {}} />
                    </div>
                  </FormCard>
                </>
              )}

              {/* TASKS & TODO */}
              {activeSection === 'tasks-todo' && (
                <>
                  <FormCard title="Creation Mode">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>ask-first</option>
                      <option>yolo</option>
                      <option>party</option>
                    </select>
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enrichment</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Auto Apply Threshold">
                    <TextInput value="0.85" />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Use LLM</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Search</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Semantic Threshold">
                    <TextInput value="0.5" />
                  </FormCard>
                  <FormCard title="Embedding Model">
                    <TextInput value="text-embedding-3-small" />
                  </FormCard>
                  <FormCard title="RRF K">
                    <TextInput value="60" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Notifications</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Focus Reminders</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Daily Digest</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Digest Time">
                    <TextInput value="09:00" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Focus</h3>
                  </div>
                  <FormCard title="Max Slots">
                    <TextInput value="3" />
                  </FormCard>
                  <FormCard title="Deadline Hours">
                    <TextInput value="18" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Daily Planning</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Planning Time">
                    <TextInput value="08:00" />
                  </FormCard>
                </>
              )}

              {/* CALENDAR */}
              {activeSection === 'calendar' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                          Bidirectional Sync
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Sync changes both ways
                        </p>
                      </div>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <FormCard title="Conflict Resolution">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>manual</option>
                      <option>local-wins</option>
                      <option>remote-wins</option>
                      <option>newest-wins</option>
                    </select>
                  </FormCard>

                  <div className="flex justify-between items-center my-6">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Providers</h3>
                    <button className="flex items-center gap-2 px-3 py-1.5 rounded text-[12px]" style={{
                      backgroundColor: 'var(--codex-accent)',
                      color: 'white'
                    }}>
                      <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                      Add Provider
                    </button>
                  </div>

                  <div className="p-4 rounded-lg mb-4" style={{
                    backgroundColor: '#141414',
                    border: '1px solid var(--codex-border)'
                  }}>
                    <div className="flex items-center justify-between mb-4">
                      <div>
                        <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Google Calendar</div>
                        <div className="text-[11px]" style={{ color: '#666' }}>CalDAV</div>
                      </div>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                    <div className="space-y-3">
                      <div>
                        <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>CalDAV URL</label>
                        <TextInput placeholder="https://calendar.google.com/..." />
                      </div>
                      <div className="grid grid-cols-2 gap-3">
                        <div>
                          <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Username</label>
                          <TextInput placeholder="user@gmail.com" />
                        </div>
                        <div>
                          <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Password</label>
                          <TextInput secret />
                        </div>
                      </div>
                      <div>
                        <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Calendar Name</label>
                        <TextInput value="Primary" />
                      </div>
                      <div className="grid grid-cols-2 gap-3">
                        <div>
                          <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Sync Interval (min)</label>
                          <TextInput value="15" />
                        </div>
                        <div className="flex items-end">
                          <label className="flex items-center gap-2 text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                            <input type="checkbox" defaultChecked />
                            Auto sync due dates
                          </label>
                        </div>
                      </div>
                    </div>
                  </div>
                </>
              )}

              {/* CONVERSATION */}
              {activeSection === 'conversation' && (
                <>
                  <div className="mb-4">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Embedding</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Exclude Channels">
                    <TextInput placeholder="channel1, channel2" />
                  </FormCard>
                  <FormCard title="Exclude Roles">
                    <TextInput placeholder="role1, role2" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Search</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Semantic Threshold">
                    <TextInput value="0.5" />
                  </FormCard>
                  <FormCard title="Max Results">
                    <TextInput value="20" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Session</h3>
                  </div>
                  <FormCard title="History Limit">
                    <TextInput value="50" />
                  </FormCard>
                  <FormCard title="TTL (days)">
                    <TextInput value="30" />
                  </FormCard>
                  <FormCard title="Cleanup Interval (hours)">
                    <TextInput value="1" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Memory</h3>
                  </div>
                  <FormCard title="Decay Half Life (days)">
                    <TextInput value="138" />
                  </FormCard>
                  <FormCard title="Max Age (days)">
                    <TextInput value="90" />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Consolidation Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Maintenance Interval (hours)">
                    <TextInput value="24" />
                  </FormCard>
                </>
              )}

              {/* LEARNING */}
              {activeSection === 'learning' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Analysis Interval (seconds)">
                    <TextInput value="3600" />
                  </FormCard>
                  <FormCard title="Min Threshold">
                    <TextInput value="0.4" />
                  </FormCard>
                  <FormCard title="Max Threshold">
                    <TextInput value="0.9" />
                  </FormCard>
                  <FormCard title="Min Outcomes for Adaptation">
                    <TextInput value="50" />
                  </FormCard>
                </>
              )}

              {/* CONFIDENCE */}
              {activeSection === 'confidence' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Threshold" description="Minimum confidence level (0-1)">
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.1"
                      value={confidenceThreshold}
                      onChange={(e) => setConfidenceThreshold(parseFloat(e.target.value))}
                      className="w-full"
                    />
                    <div className="flex justify-between text-[12px] mt-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                      <span>0</span>
                      <span style={{ color: 'var(--codex-accent)' }}>{confidenceThreshold}</span>
                      <span>1</span>
                    </div>
                  </FormCard>
                  <FormCard title="Tool Overrides" description="Per-tool confidence settings">
                    <div className="space-y-2">
                      <div className="flex gap-2">
                        <TextInput placeholder="Tool name" />
                        <TextInput placeholder="0.8" />
                        <button className="px-3 py-2 rounded border" style={{ borderColor: 'var(--codex-border)', color: 'var(--codex-fg-subtle)' }}>
                          <X className="w-4 h-4" strokeWidth={1.5} />
                        </button>
                      </div>
                      <button className="flex items-center gap-2 px-3 py-2 rounded text-[12px]" style={{ color: 'var(--codex-accent)' }}>
                        <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                        Add Override
                      </button>
                    </div>
                  </FormCard>
                </>
              )}

              {/* FINANCE */}
              {activeSection === 'finance' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Default Currency">
                    <TextInput value="USD" />
                  </FormCard>
                  <FormCard title="Proactivity Level">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>low</option>
                      <option>medium</option>
                      <option>high</option>
                    </select>
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Inflation</h3>
                  </div>
                  <FormCard title="Rate (%)">
                    <TextInput value="3.3" />
                  </FormCard>
                  <FormCard title="Source">
                    <TextInput value="CPI" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Expected Returns</h3>
                  </div>
                  <FormCard title="Stocks (%)">
                    <TextInput value="10" />
                  </FormCard>
                  <FormCard title="Crypto (%)">
                    <TextInput value="15" />
                  </FormCard>
                  <FormCard title="Real Estate (%)">
                    <TextInput value="8" />
                  </FormCard>
                  <FormCard title="Bonds (%)">
                    <TextInput value="5" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Budgeting</h3>
                  </div>
                  <FormCard title="Default Method">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>six-jar</option>
                      <option>50-30-20</option>
                      <option>zero-based</option>
                    </select>
                  </FormCard>
                  <FormCard title="Alert Threshold (%)">
                    <TextInput value="80" />
                  </FormCard>
                  <FormCard title="Six Jar Ratios" description="Essentials/Savings/Investment/Education/Entertainment/Charity">
                    <TextInput value="55/10/10/10/10/5" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Price Refresh</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Interval (hours)">
                    <TextInput value="4" />
                  </FormCard>
                  <FormCard title="Cache TTL (minutes)">
                    <TextInput value="15" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Scheduling</h3>
                  </div>
                  <FormCard title="Daily Review Time">
                    <TextInput value="21:00" />
                  </FormCard>
                  <FormCard title="Weekly Report Day">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}>
                      <option>monday</option>
                      <option>tuesday</option>
                      <option>wednesday</option>
                      <option>thursday</option>
                      <option>friday</option>
                      <option>saturday</option>
                      <option>sunday</option>
                    </select>
                  </FormCard>
                  <FormCard title="Budget Check Time">
                    <TextInput value="09:00" />
                  </FormCard>
                </>
              )}

              {/* PROJECTS */}
              {activeSection === 'projects' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                          Enabled
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Enable project management features
                        </p>
                      </div>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                </>
              )}

              {/* PACKS & SKILLS */}
              {activeSection === 'packs-skills' && (
                <>
                  <div className="mb-4">
                    <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Feature Packs</h3>
                    <p className="text-[12px] mb-4" style={{ color: '#666' }}>Select feature packs to enable</p>
                    <div className="flex flex-wrap gap-2">
                      {['task-management', 'productivity', 'ai-intelligence', 'developer-tools'].map((pack) => (
                        <button
                          key={pack}
                          className="px-3 py-1.5 rounded text-[12px] border transition-all"
                          style={{
                            backgroundColor: 'var(--codex-accent-dim)',
                            borderColor: 'var(--codex-accent)',
                            color: 'var(--codex-accent)'
                          }}
                        >
                          {pack}
                        </button>
                      ))}
                    </div>
                  </div>

                  <FormCard title="Enabled Skills" description="Comma-separated list of enabled skills">
                    <TextInput value="todo, daily-planning, finance, cron, skill-creator" />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Plugins</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Registry URL">
                    <TextInput value="https://plugins.klyntbot.com" />
                  </FormCard>
                  <FormCard title="Sandbox Memory (MB)">
                    <TextInput value="64" />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Allow Network by Default</label>
                      <Toggle checked={false} onChange={() => {}} />
                    </div>
                  </FormCard>
                </>
              )}

              {/* PLUGINS */}
              {activeSection === 'plugins' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      <Toggle checked={true} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Registry URL">
                    <TextInput value="https://plugins.klyntbot.com" />
                  </FormCard>
                  <FormCard title="Sandbox Memory (MB)">
                    <TextInput value="64" />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Allow Network by Default</label>
                      <Toggle checked={false} onChange={() => {}} />
                    </div>
                  </FormCard>
                </>
              )}

              {/* Bottom Actions */}
              <div className="flex items-center justify-between pt-6 border-t mt-8" style={{ borderColor: 'var(--codex-border)' }}>
                <button
                  className="text-[13px] transition-colors"
                  style={{ color: 'var(--codex-fg-subtle)' }}
                  onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
                  onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                >
                  Reset to Defaults
                </button>
                <button
                  className="px-6 py-2 rounded-lg text-[14px] transition-colors"
                  style={{
                    backgroundColor: 'var(--codex-accent)',
                    color: 'white'
                  }}
                  onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                  onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
                >
                  Save Changes
                </button>
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
        {/* Session Info */}
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

        {/* Token Usage */}
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

        {/* Config Status */}
        <div>
          <button 
            onClick={() => setConfigOpen(!configOpen)}
            className="w-full px-4 py-3 flex items-center justify-between transition-colors"
            style={{ 
              backgroundColor: 'transparent',
              color: 'var(--codex-fg-subtle)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
          >
            <span className="text-[10px] uppercase tracking-wider" style={{ fontWeight: 500 }}>
              Config Status
            </span>
            {configOpen ? 
              <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} /> : 
              <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
            }
          </button>
          {configOpen && (
            <div className="px-4 pb-4 space-y-3 text-[13px]">
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Config file
                </div>
                <div style={{ 
                  color: 'var(--codex-fg-muted)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '11px'
                }}>
                  ~/.klyntbot/config.json
                </div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Last modified
                </div>
                <div style={{ color: 'var(--codex-fg)' }}>
                  2 hours ago
                </div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Data directory
                </div>
                <div style={{ 
                  color: 'var(--codex-fg-muted)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '11px'
                }}>
                  ~/.klyntbot
                </div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Database size
                </div>
                <div style={{ color: 'var(--codex-fg)' }}>
                  12.4 MB
                </div>
              </div>
            </div>
          )}
        </div>
      </aside>
    </>
  );
}
