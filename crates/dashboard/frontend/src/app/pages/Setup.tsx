// TODO: Wire setup wizard to real API:
//   - Step 1 (Provider): POST /api/settings to save provider + API key
//   - Step 2 (Channel): POST /api/settings to save channel config
//   - Step 3 (Calendar): POST /api/settings to save CalDAV config
//   - Step 4 (Packs): POST /api/settings to save pack selection
//   - Verify button: POST /api/settings/verify-provider
//   - No read-only integration needed (wizard is write-only)
import { useState } from 'react';
import {
  Check,
  ChevronLeft,
  ChevronRight, 
  Eye, 
  EyeOff, 
  Loader2,
  X,
  CheckCircle2,
  Lock,
  Info,
  Network,
  Fish,
  Zap,
  Server,
  Brain,
  Radar,
  Moon,
  Sparkles,
  GitMerge,
  Calendar as CalendarIcon,
  Mail,
  Phone,
  Hash
} from 'lucide-react';
import { motion } from 'motion/react';

type Provider = {
  id: string;
  name: string;
  color: string;
};

type Pack = {
  id: string;
  name: string;
  tier: 'core' | 'recommended' | 'optional';
  description: string;
  skills: string[];
  enabled: boolean;
  locked?: boolean;
};

// Brand SVG Icons as Components
const AnthropicIcon = () => (
  <svg viewBox="0 0 24 24" width="24" height="24" fill="white">
    <path d="M17.304 4.49l-3.897 10.643H7.593L3.696 4.49h3.294l2.447 7.406h.126L11.993 4.49h3.294zm2.392 0h3.304L19.304 15.133h-3.304l3.696-10.643z" />
  </svg>
);

const OpenAIIcon = () => (
  <svg viewBox="0 0 24 24" width="24" height="24" fill="white">
    <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.998 5.998 0 0 0-3.998 2.9 6.042 6.042 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.872zm16.597 3.855l-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023l-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zm-12.64 4.135l-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.375-3.453l-.142.08L8.704 5.46a.795.795 0 0 0-.393.681zm1.097-2.365l2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z" />
  </svg>
);

const TelegramIcon = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="white">
    <path d="M11.944 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0a12 12 0 0 0-.056 0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 0 1 .171.325c.016.093.036.306.02.472-.18 1.898-.962 6.502-1.36 8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.479.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z" />
  </svg>
);

const DiscordIcon = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="white">
    <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03z" />
  </svg>
);

const WhatsAppIcon = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="white">
    <path d="M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458.13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198.05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118.571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 01-5.031-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 01-1.51-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988 2.898a9.825 9.825 0 012.893 6.994c-.003 5.45-4.437 9.884-9.885 9.884m8.413-18.297A11.815 11.815 0 0012.05 0C5.495 0 .16 5.335.157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a11.882 11.882 0 005.683 1.448h.005c6.554 0 11.89-5.335 11.893-11.893a11.821 11.821 0 00-3.48-8.413z" />
  </svg>
);

const AppleIcon = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="white">
    <path d="M17.05 20.28c-.98.95-2.05.88-3.08.4-1.09-.5-2.08-.48-3.24 0-1.44.62-2.2.44-3.06-.4C2.79 15.25 3.51 7.59 9.05 7.31c1.35.07 2.29.74 3.08.8 1.18-.24 2.31-.93 3.57-.84 1.51.12 2.65.72 3.4 1.8-3.12 1.87-2.38 5.98.48 7.13-.57 1.5-1.31 2.99-2.54 4.09l.01-.01zM12.03 7.25c-.15-2.23 1.66-4.07 3.74-4.25.29 2.58-2.34 4.5-3.74 4.25z" />
  </svg>
);

const GoogleIcon = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" fill="white">
    <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" />
    <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" />
    <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" />
    <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" />
  </svg>
);

export default function Setup() {
  const [currentStep, setCurrentStep] = useState(1);
  const [selectedProvider, setSelectedProvider] = useState<string | null>('anthropic');
  const [apiKey, setApiKey] = useState('');
  const [showApiKey, setShowApiKey] = useState(false);
  const [verifyStatus, setVerifyStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [envDetected] = useState(true);

  const [selectedChannel, setSelectedChannel] = useState<string | null>('none');
  const [channelToken, setChannelToken] = useState('');
  const [channelToken2, setChannelToken2] = useState('');
  const [showChannelToken, setShowChannelToken] = useState(false);
  const [channelTestStatus, setChannelTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');

  const [selectedCalendar, setSelectedCalendar] = useState<string | null>('none');
  const [calendarFields, setCalendarFields] = useState({
    appleId: '',
    appPassword: '',
    caldavUrl: 'https://caldav.icloud.com',
    calendarName: 'Personal',
    googleClientId: '',
    googleClientSecret: '',
    caldavUrl2: '',
    caldavUsername: '',
    caldavPassword: '',
  });
  const [calendarTestStatus, setCalendarTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');

  const [packs, setPacks] = useState<Pack[]>([
    { 
      id: 'task-management', 
      name: 'Task Management', 
      tier: 'core', 
      description: 'Tasks, focus mode, enrichment, semantic search',
      skills: ['tasks', 'focus', 'search'],
      enabled: true,
      locked: true
    },
    { 
      id: 'productivity', 
      name: 'Productivity', 
      tier: 'recommended', 
      description: 'Calendar sync, daily planning, cron, summarize',
      skills: ['calendar', 'planning', 'cron', 'summarize'],
      enabled: true
    },
    { 
      id: 'ai-intelligence', 
      name: 'AI Intelligence', 
      tier: 'recommended', 
      description: 'Conversation memory, learning system, embeddings',
      skills: ['memory', 'learning', 'embeddings'],
      enabled: true
    },
    { 
      id: 'browser-automation', 
      name: 'Browser', 
      tier: 'optional', 
      description: 'Real-world task execution: booking, shopping',
      skills: ['browser', 'booking'],
      enabled: false
    },
    { 
      id: 'finance', 
      name: 'Finance', 
      tier: 'optional', 
      description: 'Budget tracking, expenses, investment projection',
      skills: ['budget', 'expenses'],
      enabled: false
    },
    { 
      id: 'skill-creator', 
      name: 'Skill Creator', 
      tier: 'optional', 
      description: 'Create custom skills',
      skills: ['custom-skills'],
      enabled: false
    },
    { 
      id: 'weather', 
      name: 'Weather', 
      tier: 'optional', 
      description: 'Weather queries and forecasts',
      skills: ['weather', 'forecast'],
      enabled: false
    },
  ]);

  const providers: Provider[] = [
    { id: 'anthropic', name: 'Anthropic', color: '#d4876f' },
    { id: 'openai', name: 'OpenAI', color: '#10a37f' },
    { id: 'openrouter', name: 'OpenRouter', color: '#4a90e2' },
    { id: 'deepseek', name: 'DeepSeek', color: '#0084ff' },
    { id: 'gemini', name: 'Gemini', color: '#4285f4' },
    { id: 'groq', name: 'Groq', color: '#f55036' },
    { id: 'vllm', name: 'vLLM', color: '#666' },
    { id: 'zhipu', name: 'Zhipu', color: '#4a90e2' },
    { id: 'dashscope', name: 'DashScope', color: '#ff7000' },
    { id: 'moonshot', name: 'Moonshot', color: '#8b5cf6' },
    { id: 'minimax', name: 'MiniMax', color: '#dc2626' },
    { id: 'aihubmix', name: 'AIHubMix', color: '#10a37f' },
  ];

  const channels = [
    { id: 'none', name: 'None', icon: 'none' },
    { id: 'telegram', name: 'Telegram', icon: 'telegram' },
    { id: 'discord', name: 'Discord', icon: 'discord' },
    { id: 'slack', name: 'Slack', icon: 'slack' },
    { id: 'whatsapp', name: 'WhatsApp', icon: 'whatsapp' },
    { id: 'email', name: 'Email', icon: 'email' },
    { id: 'qq', name: 'QQ', icon: 'qq' },
  ];

  const calendars = [
    { id: 'none', name: 'None', icon: 'none' },
    { id: 'apple', name: 'Apple', icon: 'apple' },
    { id: 'google', name: 'Google', icon: 'google' },
    { id: 'caldav', name: 'CalDAV', icon: 'caldav' },
  ];

  const steps = [
    { number: 1, label: 'Provider', completed: currentStep > 1 },
    { number: 2, label: 'Channel', completed: currentStep > 2 },
    { number: 3, label: 'Calendar', completed: currentStep > 3 },
    { number: 4, label: 'Packs', completed: currentStep > 4 },
  ];

  const handleVerifyConnection = () => {
    setVerifyStatus('loading');
    setTimeout(() => {
      setVerifyStatus(apiKey.length > 10 ? 'success' : 'error');
    }, 1500);
  };

  const handleTestChannel = () => {
    setChannelTestStatus('loading');
    setTimeout(() => {
      setChannelTestStatus(channelToken.length > 5 ? 'success' : 'error');
    }, 1500);
  };

  const handleTestCalendar = () => {
    setCalendarTestStatus('loading');
    setTimeout(() => {
      setCalendarTestStatus('success');
    }, 1500);
  };

  const getEnabledSkills = () => {
    return packs.filter(p => p.enabled).flatMap(p => p.skills);
  };

  const getProviderIcon = (id: string) => {
    switch (id) {
      case 'anthropic':
        return <AnthropicIcon />;
      case 'openai':
        return <OpenAIIcon />;
      case 'openrouter':
        return <Network className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'deepseek':
        return <Fish className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'gemini':
        return (
          <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="white" strokeWidth="2">
            <path d="M12 2l2 7h7l-5.5 4.5L18 21l-6-4.5L6 21l2.5-7.5L3 9h7z" />
          </svg>
        );
      case 'groq':
        return <Zap className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'vllm':
        return <Server className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'zhipu':
        return <Brain className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'dashscope':
        return <Radar className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'moonshot':
        return <Moon className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'minimax':
        return <Sparkles className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      case 'aihubmix':
        return <GitMerge className="w-6 h-6" strokeWidth={2} stroke="white" fill="none" />;
      default:
        return null;
    }
  };

  const getChannelIcon = (id: string) => {
    switch (id) {
      case 'none':
        return <X className="w-3.5 h-3.5" strokeWidth={2} stroke="white" fill="none" />;
      case 'telegram':
        return <TelegramIcon />;
      case 'discord':
        return <DiscordIcon />;
      case 'slack':
        return <Hash className="w-4 h-4" strokeWidth={2} stroke="white" fill="none" />;
      case 'whatsapp':
        return <WhatsAppIcon />;
      case 'email':
        return <Mail className="w-4 h-4" strokeWidth={2} stroke="white" fill="none" />;
      case 'qq':
        return (
          <svg viewBox="0 0 24 24" width="16" height="16" fill="white">
            <path d="M12 2c2.2 0 4 1.8 4 4s-1.8 4-4 4-4-1.8-4-4 1.8-4 4-4zm0 14c-3.3 0-6 2.7-6 6h12c0-3.3-2.7-6-6-6zm-8-2c0 1.1.9 2 2 2h1.5c-.3-.6-.5-1.3-.5-2s.2-1.4.5-2H6c-1.1 0-2 .9-2 2zm12.5-2c.3.6.5 1.3.5 2s-.2 1.4-.5 2H18c1.1 0 2-.9 2-2s-.9-2-2-2h-1.5z" />
          </svg>
        );
      default:
        return null;
    }
  };

  const getCalendarIcon = (id: string) => {
    switch (id) {
      case 'none':
        return <X className="w-4 h-4" strokeWidth={2} stroke="white" fill="none" />;
      case 'apple':
        return <AppleIcon />;
      case 'google':
        return <GoogleIcon />;
      case 'caldav':
        return <Server className="w-4 h-4" strokeWidth={2} stroke="white" fill="none" />;
      default:
        return null;
    }
  };

  return (
    <div className="h-screen flex items-center justify-center" style={{
      backgroundColor: 'var(--codex-bg)',
      fontFamily: 'var(--font-ui)'
    }}>
      <div className="w-full max-w-3xl px-8">
        {/* Compact Header */}
        <div className="flex items-center gap-3 mb-8">
          <div className="w-7 h-7 rounded flex items-center justify-center" style={{
            backgroundColor: 'var(--codex-accent)',
            fontSize: '16px',
            fontWeight: 600,
            color: 'white'
          }}>
            K
          </div>
          <h1 className="text-[16px]" style={{ 
            color: 'var(--codex-fg)',
            fontWeight: 500
          }}>
            Setup Wizard
          </h1>
        </div>

        {/* Compact Stepper */}
        <div className="mb-3 flex items-center justify-center">
          {steps.map((step, index) => (
            <div key={step.number} className="flex items-center">
              <div className="flex flex-col items-center">
                <div 
                  className="w-6 h-6 rounded-full flex items-center justify-center mb-1 transition-all"
                  style={{
                    backgroundColor: step.completed ? '#10a37f' : currentStep === step.number ? 'var(--codex-accent)' : 'transparent',
                    border: `1px solid ${step.completed || currentStep === step.number ? 'var(--codex-accent)' : '#333'}`,
                    color: step.completed || currentStep === step.number ? 'white' : '#333'
                  }}
                >
                  {step.completed ? (
                    <Check className="w-3 h-3" strokeWidth={2} />
                  ) : (
                    <span className="text-[11px]">{step.number}</span>
                  )}
                </div>
                <span className="text-[11px]" style={{ 
                  color: currentStep === step.number ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)'
                }}>
                  {step.label}
                </span>
              </div>
              {index < steps.length - 1 && (
                <div className="w-16 h-[1px] mx-3 mb-4" style={{
                  backgroundColor: step.completed ? '#10a37f' : '#333'
                }} />
              )}
            </div>
          ))}
        </div>

        {/* Step Content */}
        <motion.div
          key={currentStep}
          initial={{ opacity: 0, x: 20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.15 }}
          className="p-5 rounded"
          style={{
            backgroundColor: '#141414',
            border: '1px solid #1a1a1a',
            minHeight: '320px'
          }}
        >
          {/* Step 1: Provider Selection */}
          {currentStep === 1 && (
            <div>
              <h2 className="text-[16px] mb-3" style={{ 
                color: 'var(--codex-fg)',
                fontWeight: 500
              }}>
                Choose your AI provider
              </h2>
              
              {/* Provider Grid: 6x2 */}
              <div className="grid grid-cols-6 gap-2 mb-4">
                {providers.map((provider) => (
                  <button
                    key={provider.id}
                    onClick={() => setSelectedProvider(provider.id)}
                    className="p-2 rounded border transition-all flex flex-col items-center justify-center"
                    style={{
                      backgroundColor: '#1a1a1a',
                      borderColor: selectedProvider === provider.id ? '#10a37f' : '#1a1a1a',
                      width: '80px',
                      height: '64px'
                    }}
                  >
                    <div className="mb-1">
                      {getProviderIcon(provider.id)}
                    </div>
                    <div className="text-[11px] text-center truncate" style={{ color: 'var(--codex-fg)' }}>
                      {provider.name}
                    </div>
                  </button>
                ))}
              </div>

              {/* API Key Row - Horizontal */}
              <div className="flex items-center gap-2">
                <label className="text-[13px] whitespace-nowrap" style={{ color: 'var(--codex-fg-subtle)', width: '60px' }}>
                  API Key
                </label>
                <div className="relative flex-1">
                  <input
                    type={showApiKey ? 'text' : 'password'}
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="sk-ant-..."
                    className="w-full px-3 py-1.5 rounded border outline-none text-[12px] pr-16"
                    style={{
                      backgroundColor: '#0d0d0d',
                      borderColor: '#1a1a1a',
                      color: 'var(--codex-fg)',
                      fontFamily: 'var(--font-mono)',
                      height: '32px'
                    }}
                    onFocus={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                    onBlur={(e) => e.currentTarget.style.borderColor = '#1a1a1a'}
                  />
                  <button
                    onClick={() => setShowApiKey(!showApiKey)}
                    className="absolute right-2 top-1/2 -translate-y-1/2"
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    {showApiKey ? <EyeOff className="w-3.5 h-3.5" strokeWidth={1.5} /> : <Eye className="w-3.5 h-3.5" strokeWidth={1.5} />}
                  </button>
                </div>
                <button
                  onClick={handleVerifyConnection}
                  disabled={verifyStatus === 'loading'}
                  className="px-3 py-1.5 rounded border transition-all text-[12px] flex items-center gap-1.5 whitespace-nowrap"
                  style={{
                    borderColor: 'var(--codex-accent)',
                    color: 'var(--codex-accent)',
                    backgroundColor: 'transparent',
                    height: '32px'
                  }}
                  onMouseEnter={(e) => !e.currentTarget.disabled && (e.currentTarget.style.backgroundColor = 'rgba(16, 163, 127, 0.1)')}
                  onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                >
                  {verifyStatus === 'loading' && <Loader2 className="w-3 h-3 animate-spin" strokeWidth={1.5} />}
                  Verify
                </button>
                {verifyStatus === 'success' && (
                  <CheckCircle2 className="w-4 h-4" strokeWidth={1.5} style={{ color: '#10a37f' }} />
                )}
                {verifyStatus === 'error' && (
                  <X className="w-4 h-4" strokeWidth={1.5} style={{ color: '#e55050' }} />
                )}
                {envDetected && verifyStatus === 'idle' && (
                  <span className="text-[10px] whitespace-nowrap" style={{ color: '#10a37f' }}>
                    Detected from env
                  </span>
                )}
              </div>
            </div>
          )}

          {/* Step 2: Channel Selection */}
          {currentStep === 2 && (
            <div>
              <div className="flex items-center justify-between mb-3">
                <h2 className="text-[16px]" style={{ 
                  color: 'var(--codex-fg)',
                  fontWeight: 500
                }}>
                  Connect a chat platform
                </h2>
                <button
                  onClick={() => setCurrentStep(3)}
                  className="text-[12px]"
                  style={{ color: 'var(--codex-accent)' }}
                >
                  Skip →
                </button>
              </div>

              {/* Channel Pills - Horizontal Row */}
              <div className="flex gap-2 mb-4">
                {channels.map((channel) => (
                  <button
                    key={channel.id}
                    onClick={() => setSelectedChannel(channel.id)}
                    className="px-2 py-2 rounded border transition-all flex flex-col items-center justify-center gap-1"
                    style={{
                      backgroundColor: '#1a1a1a',
                      borderColor: selectedChannel === channel.id ? '#10a37f' : '#1a1a1a',
                      width: '60px',
                      height: '48px'
                    }}
                  >
                    {getChannelIcon(channel.id)}
                    <div className="text-[9px]" style={{ color: 'var(--codex-fg)' }}>
                      {channel.name}
                    </div>
                  </button>
                ))}
              </div>

              {/* Channel-specific fields */}
              {selectedChannel === 'none' && (
                <div className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                  CLI only — no platform integration
                </div>
              )}

              {selectedChannel === 'telegram' && (
                <div className="flex items-center gap-2">
                  <label className="text-[12px] whitespace-nowrap" style={{ color: 'var(--codex-fg-subtle)', width: '70px' }}>
                    Bot Token
                  </label>
                  <div className="relative flex-1">
                    <input
                      type={showChannelToken ? 'text' : 'password'}
                      value={channelToken}
                      onChange={(e) => setChannelToken(e.target.value)}
                      placeholder="Enter token..."
                      className="w-full px-3 py-1.5 rounded border outline-none text-[12px] pr-16"
                      style={{
                        backgroundColor: '#0d0d0d',
                        borderColor: '#1a1a1a',
                        color: 'var(--codex-fg)',
                        fontFamily: 'var(--font-mono)',
                        height: '32px'
                      }}
                    />
                    <button
                      onClick={() => setShowChannelToken(!showChannelToken)}
                      className="absolute right-2 top-1/2 -translate-y-1/2"
                      style={{ color: 'var(--codex-fg-subtle)' }}
                    >
                      {showChannelToken ? <EyeOff className="w-3.5 h-3.5" strokeWidth={1.5} /> : <Eye className="w-3.5 h-3.5" strokeWidth={1.5} />}
                    </button>
                  </div>
                  <button
                    onClick={handleTestChannel}
                    disabled={channelTestStatus === 'loading'}
                    className="px-3 py-1.5 rounded border text-[12px] flex items-center gap-1.5 whitespace-nowrap"
                    style={{
                      borderColor: 'var(--codex-accent)',
                      color: 'var(--codex-accent)',
                      height: '32px'
                    }}
                  >
                    {channelTestStatus === 'loading' && <Loader2 className="w-3 h-3 animate-spin" strokeWidth={1.5} />}
                    Test
                  </button>
                  {channelTestStatus === 'success' && (
                    <CheckCircle2 className="w-4 h-4" strokeWidth={1.5} style={{ color: '#10a37f' }} />
                  )}
                  {channelTestStatus === 'error' && (
                    <X className="w-4 h-4" strokeWidth={1.5} style={{ color: '#e55050' }} />
                  )}
                </div>
              )}

              {selectedChannel === 'discord' && (
                <div className="flex items-center gap-2">
                  <label className="text-[12px] whitespace-nowrap" style={{ color: 'var(--codex-fg-subtle)', width: '70px' }}>
                    Bot Token
                  </label>
                  <input
                    type="password"
                    value={channelToken}
                    onChange={(e) => setChannelToken(e.target.value)}
                    placeholder="Enter token..."
                    className="flex-1 px-3 py-1.5 rounded border outline-none text-[12px]"
                    style={{
                      backgroundColor: '#0d0d0d',
                      borderColor: '#1a1a1a',
                      color: 'var(--codex-fg)',
                      fontFamily: 'var(--font-mono)',
                      height: '32px'
                    }}
                  />
                  <button
                    onClick={handleTestChannel}
                    className="px-3 py-1.5 rounded border text-[12px] whitespace-nowrap"
                    style={{
                      borderColor: 'var(--codex-accent)',
                      color: 'var(--codex-accent)',
                      height: '32px'
                    }}
                  >
                    Test
                  </button>
                </div>
              )}

              {selectedChannel === 'slack' && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <label className="text-[12px] whitespace-nowrap" style={{ color: 'var(--codex-fg-subtle)', width: '70px' }}>
                      Bot Token
                    </label>
                    <input
                      type="password"
                      value={channelToken}
                      onChange={(e) => setChannelToken(e.target.value)}
                      placeholder="xoxb-..."
                      className="flex-1 px-3 py-1.5 rounded border outline-none text-[12px]"
                      style={{
                        backgroundColor: '#0d0d0d',
                        borderColor: '#1a1a1a',
                        color: 'var(--codex-fg)',
                        fontFamily: 'var(--font-mono)',
                        height: '32px'
                      }}
                    />
                  </div>
                  <div className="flex items-center gap-2">
                    <label className="text-[12px] whitespace-nowrap" style={{ color: 'var(--codex-fg-subtle)', width: '70px' }}>
                      App Token
                    </label>
                    <input
                      type="password"
                      value={channelToken2}
                      onChange={(e) => setChannelToken2(e.target.value)}
                      placeholder="xapp-..."
                      className="flex-1 px-3 py-1.5 rounded border outline-none text-[12px]"
                      style={{
                        backgroundColor: '#0d0d0d',
                        borderColor: '#1a1a1a',
                        color: 'var(--codex-fg)',
                        fontFamily: 'var(--font-mono)',
                        height: '32px'
                      }}
                    />
                    <button
                      onClick={handleTestChannel}
                      className="px-3 py-1.5 rounded border text-[12px] whitespace-nowrap"
                      style={{
                        borderColor: 'var(--codex-accent)',
                        color: 'var(--codex-accent)',
                        height: '32px'
                      }}
                    >
                      Test
                    </button>
                  </div>
                </div>
              )}

              {selectedChannel === 'whatsapp' && (
                <div className="flex items-center gap-2">
                  <label className="text-[12px] whitespace-nowrap" style={{ color: 'var(--codex-fg-subtle)', width: '70px' }}>
                    Bridge URL
                  </label>
                  <input
                    type="text"
                    defaultValue="ws://localhost:3001"
                    className="flex-1 px-3 py-1.5 rounded border outline-none text-[12px]"
                    style={{
                      backgroundColor: '#0d0d0d',
                      borderColor: '#1a1a1a',
                      color: 'var(--codex-fg)',
                      fontFamily: 'var(--font-mono)',
                      height: '32px'
                    }}
                  />
                  <span className="text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                    Configure in config
                  </span>
                </div>
              )}

              {selectedChannel === 'email' && (
                <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Configure IMAP/SMTP in config file
                </div>
              )}

              {selectedChannel === 'qq' && (
                <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Configure App ID and Secret in config file
                </div>
              )}
            </div>
          )}

          {/* Step 3: Calendar Selection */}
          {currentStep === 3 && (
            <div>
              <div className="flex items-center justify-between mb-3">
                <h2 className="text-[16px]" style={{ 
                  color: 'var(--codex-fg)',
                  fontWeight: 500
                }}>
                  Calendar Sync
                </h2>
                <button
                  onClick={() => setCurrentStep(4)}
                  className="text-[12px]"
                  style={{ color: 'var(--codex-accent)' }}
                >
                  Skip →
                </button>
              </div>

              {/* Calendar Cards - Horizontal Row */}
              <div className="flex gap-2 mb-4">
                {calendars.map((cal) => (
                  <button
                    key={cal.id}
                    onClick={() => setSelectedCalendar(cal.id)}
                    className="px-3 py-2 rounded border transition-all flex flex-col items-center justify-center gap-1"
                    style={{
                      backgroundColor: '#1a1a1a',
                      borderColor: selectedCalendar === cal.id ? '#10a37f' : '#1a1a1a',
                      width: '100px',
                      height: '56px'
                    }}
                  >
                    {getCalendarIcon(cal.id)}
                    <div className="text-[11px]" style={{ color: 'var(--codex-fg)' }}>
                      {cal.name}
                    </div>
                  </button>
                ))}
              </div>

              {/* Calendar-specific fields */}
              {selectedCalendar === 'none' && (
                <div className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                  No calendar sync
                </div>
              )}

              {selectedCalendar === 'apple' && (
                <div className="space-y-2">
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Apple ID</label>
                      <input
                        type="email"
                        value={calendarFields.appleId}
                        onChange={(e) => setCalendarFields({ ...calendarFields, appleId: e.target.value })}
                        placeholder="your@icloud.com"
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          height: '32px'
                        }}
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>App Password</label>
                      <input
                        type="password"
                        value={calendarFields.appPassword}
                        onChange={(e) => setCalendarFields({ ...calendarFields, appPassword: e.target.value })}
                        placeholder="xxxx-xxxx-xxxx-xxxx"
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          fontFamily: 'var(--font-mono)',
                          height: '32px'
                        }}
                      />
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>CalDAV URL</label>
                      <input
                        type="text"
                        value={calendarFields.caldavUrl}
                        onChange={(e) => setCalendarFields({ ...calendarFields, caldavUrl: e.target.value })}
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          fontFamily: 'var(--font-mono)',
                          height: '32px'
                        }}
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Calendar Name</label>
                      <input
                        type="text"
                        value={calendarFields.calendarName}
                        onChange={(e) => setCalendarFields({ ...calendarFields, calendarName: e.target.value })}
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          height: '32px'
                        }}
                      />
                    </div>
                  </div>
                  <button
                    onClick={handleTestCalendar}
                    className="px-3 py-1.5 rounded border text-[12px]"
                    style={{
                      borderColor: 'var(--codex-accent)',
                      color: 'var(--codex-accent)',
                      height: '32px'
                    }}
                  >
                    Test Sync
                  </button>
                </div>
              )}

              {selectedCalendar === 'google' && (
                <div className="space-y-2">
                  <div className="flex gap-2">
                    <div className="flex-1">
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Client ID</label>
                      <input
                        type="text"
                        value={calendarFields.googleClientId}
                        onChange={(e) => setCalendarFields({ ...calendarFields, googleClientId: e.target.value })}
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          height: '32px'
                        }}
                      />
                    </div>
                    <div className="flex-1">
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Client Secret</label>
                      <input
                        type="password"
                        value={calendarFields.googleClientSecret}
                        onChange={(e) => setCalendarFields({ ...calendarFields, googleClientSecret: e.target.value })}
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          fontFamily: 'var(--font-mono)',
                          height: '32px'
                        }}
                      />
                    </div>
                    <button
                      onClick={handleTestCalendar}
                      className="px-3 py-1.5 rounded border text-[12px] self-end whitespace-nowrap"
                      style={{
                        borderColor: 'var(--codex-accent)',
                        color: 'var(--codex-accent)',
                        height: '32px'
                      }}
                    >
                      Test Sync
                    </button>
                  </div>
                </div>
              )}

              {selectedCalendar === 'caldav' && (
                <div className="space-y-2">
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>CalDAV URL</label>
                      <input
                        type="text"
                        value={calendarFields.caldavUrl2}
                        onChange={(e) => setCalendarFields({ ...calendarFields, caldavUrl2: e.target.value })}
                        placeholder="https://..."
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          fontFamily: 'var(--font-mono)',
                          height: '32px'
                        }}
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Username</label>
                      <input
                        type="text"
                        value={calendarFields.caldavUsername}
                        onChange={(e) => setCalendarFields({ ...calendarFields, caldavUsername: e.target.value })}
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          height: '32px'
                        }}
                      />
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Password</label>
                      <input
                        type="password"
                        value={calendarFields.caldavPassword}
                        onChange={(e) => setCalendarFields({ ...calendarFields, caldavPassword: e.target.value })}
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          fontFamily: 'var(--font-mono)',
                          height: '32px'
                        }}
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Calendar</label>
                      <input
                        type="text"
                        defaultValue="Personal"
                        className="w-full px-2 py-1.5 rounded border outline-none text-[12px]"
                        style={{
                          backgroundColor: '#0d0d0d',
                          borderColor: '#1a1a1a',
                          color: 'var(--codex-fg)',
                          height: '32px'
                        }}
                      />
                    </div>
                  </div>
                  <button
                    onClick={handleTestCalendar}
                    className="px-3 py-1.5 rounded border text-[12px]"
                    style={{
                      borderColor: 'var(--codex-accent)',
                      color: 'var(--codex-accent)',
                      height: '32px'
                    }}
                  >
                    Test Sync
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Step 4: Pack Selection */}
          {currentStep === 4 && (
            <div>
              <h2 className="text-[16px] mb-3" style={{ 
                color: 'var(--codex-fg)',
                fontWeight: 500
              }}>
                Feature Packs
              </h2>

              <div className="space-y-3">
                {/* Core Pack */}
                <div>
                  <div className="text-[10px] uppercase tracking-wide mb-1.5" style={{ color: '#10a37f', fontWeight: 500 }}>
                    CORE
                  </div>
                  <div className="flex items-center gap-2 px-3 py-2 rounded" style={{ backgroundColor: '#1a1a1a', border: '1px solid #1a1a1a' }}>
                    <Lock className="w-3.5 h-3.5" strokeWidth={1.5} style={{ color: '#10a37f' }} />
                    <span className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>Task Management</span>
                    <span className="text-[11px] ml-auto" style={{ color: 'var(--codex-fg-subtle)' }}>Always enabled</span>
                  </div>
                </div>

                {/* Recommended Packs */}
                <div>
                  <div className="text-[10px] uppercase tracking-wide mb-1.5" style={{ color: '#4a9eff', fontWeight: 500 }}>
                    RECOMMENDED
                  </div>
                  <div className="flex gap-3">
                    {packs.filter(p => p.tier === 'recommended').map(pack => (
                      <label
                        key={pack.id}
                        className="flex items-center gap-2 px-3 py-2 rounded cursor-pointer flex-1"
                        style={{ backgroundColor: '#1a1a1a', border: '1px solid #1a1a1a' }}
                      >
                        <input
                          type="checkbox"
                          checked={pack.enabled}
                          onChange={() => {
                            setPacks(packs.map(p => 
                              p.id === pack.id ? { ...p, enabled: !p.enabled } : p
                            ));
                          }}
                          className="w-3.5 h-3.5 rounded"
                          style={{ accentColor: '#10a37f' }}
                        />
                        <span className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{pack.name}</span>
                        <Info className="w-3 h-3 ml-auto" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
                      </label>
                    ))}
                  </div>
                </div>

                {/* Optional Packs */}
                <div>
                  <div className="text-[10px] uppercase tracking-wide mb-1.5" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                    OPTIONAL
                  </div>
                  <div className="grid grid-cols-2 gap-2">
                    {packs.filter(p => p.tier === 'optional').map(pack => (
                      <label
                        key={pack.id}
                        className="flex items-center gap-2 px-3 py-2 rounded cursor-pointer"
                        style={{ backgroundColor: '#1a1a1a', border: '1px solid #1a1a1a' }}
                      >
                        <input
                          type="checkbox"
                          checked={pack.enabled}
                          onChange={() => {
                            setPacks(packs.map(p => 
                              p.id === pack.id ? { ...p, enabled: !p.enabled } : p
                            ));
                          }}
                          className="w-3.5 h-3.5 rounded"
                          style={{ accentColor: '#10a37f' }}
                        />
                        <span className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{pack.name}</span>
                        <Info className="w-3 h-3 ml-auto" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
                      </label>
                    ))}
                  </div>
                </div>

                {/* Enabled Skills */}
                <div className="pt-2 border-t" style={{ borderColor: '#1a1a1a' }}>
                  <div className="text-[11px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
                    Enabled skills:
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {getEnabledSkills().map((skill, idx) => (
                      <span
                        key={idx}
                        className="px-2 py-0.5 rounded text-[10px]"
                        style={{
                          backgroundColor: '#10a37f20',
                          color: '#10a37f',
                          border: '1px solid #10a37f40'
                        }}
                      >
                        {skill}
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}
        </motion.div>

        {/* Navigation */}
        <div className="flex items-center justify-between mt-3">
          <button
            onClick={() => setCurrentStep(Math.max(1, currentStep - 1))}
            disabled={currentStep === 1}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[13px] transition-all"
            style={{
              color: currentStep === 1 ? '#333' : 'var(--codex-fg-subtle)',
              cursor: currentStep === 1 ? 'not-allowed' : 'pointer',
              height: '32px'
            }}
            onMouseEnter={(e) => {
              if (currentStep > 1) e.currentTarget.style.backgroundColor = '#141414';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = 'transparent';
            }}
          >
            <ChevronLeft className="w-3.5 h-3.5" strokeWidth={1.5} />
            Back
          </button>

          <button
            onClick={() => {
              if (currentStep === 4) {
                window.location.href = '/';
              } else {
                setCurrentStep(Math.min(4, currentStep + 1));
              }
            }}
            className="flex items-center gap-1.5 px-4 py-1.5 rounded text-[13px] transition-all"
            style={{
              backgroundColor: 'var(--codex-accent)',
              color: 'white',
              height: '32px'
            }}
            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
          >
            {currentStep === 4 ? 'Complete Setup' : 'Continue'}
            <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
          </button>
        </div>
      </div>
    </div>
  );
}
