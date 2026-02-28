import { useState, useMemo, useCallback } from 'react';
import { useNavigate } from 'react-router';
import { Search, Clock, Trash2, ArrowUpDown } from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { SessionListItem } from '../../lib/types';
import { formatDuration } from '../chat/utils';
import { formatTimeAgo } from '../../lib/utils';

type SortMode = 'recent' | 'oldest' | 'messages';

export default function Sessions() {
  const navigate = useNavigate();
  const { data: sessions, loading, refetch } = useApi<SessionListItem[]>('/api/sessions');
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<SortMode>('recent');
  const [deleting, setDeleting] = useState<string | null>(null);

  const filtered = useMemo(() => {
    if (!sessions) return [];
    let list = sessions;
    if (search) {
      const q = search.toLowerCase();
      list = list.filter((s) => s.key.toLowerCase().includes(q));
    }
    return list.slice().sort((a, b) => {
      switch (sort) {
        case 'recent':
          return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
        case 'oldest':
          return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
        case 'messages':
          return b.messageCount - a.messageCount;
      }
    });
  }, [sessions, search, sort]);

  const handleDelete = useCallback(
    async (e: React.MouseEvent, key: string) => {
      e.stopPropagation();
      setDeleting(key);
      try {
        await apiFetch(`/api/sessions/${key}`, { method: 'DELETE' });
        refetch();
      } finally {
        setDeleting(null);
      }
    },
    [refetch],
  );

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header */}
      <div
        className="px-6 py-4 border-b flex items-center justify-between"
        style={{ borderColor: 'var(--codex-border-subtle)' }}
      >
        <h1
          className="text-lg"
          style={{ color: 'var(--codex-fg)', fontWeight: 400 }}
        >
          Sessions
        </h1>
        <div className="flex items-center gap-3">
          {/* Search */}
          <div
            className="flex items-center gap-2 px-3 py-1.5 rounded-md border"
            style={{
              borderColor: 'var(--codex-border)',
              backgroundColor: 'var(--codex-bg-tertiary)',
            }}
          >
            <Search className="w-3.5 h-3.5" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search sessions..."
              className="bg-transparent outline-none text-[13px] w-48"
              style={{ color: 'var(--codex-fg)' }}
            />
          </div>
          {/* Sort */}
          <div className="flex items-center gap-1.5">
            <ArrowUpDown className="w-3.5 h-3.5" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
            {(['recent', 'oldest', 'messages'] as const).map((mode) => (
              <button
                key={mode}
                onClick={() => setSort(mode)}
                className="px-2 py-1 rounded text-[11px] transition-colors"
                style={{
                  backgroundColor: sort === mode ? 'var(--codex-bg-tertiary)' : 'transparent',
                  color: sort === mode ? 'var(--codex-fg)' : 'var(--codex-fg-subtle)',
                  border: sort === mode ? '1px solid var(--codex-border)' : '1px solid transparent',
                }}
              >
                {mode === 'recent' ? 'Recent' : mode === 'oldest' ? 'Oldest' : 'Messages'}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Session List */}
      <div className="flex-1 overflow-y-auto">
        {loading && (
          <div className="flex items-center justify-center py-12 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            Loading sessions...
          </div>
        )}
        {!loading && filtered.length === 0 && (
          <div className="flex items-center justify-center py-12 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            {search ? 'No sessions match your search' : 'No sessions yet'}
          </div>
        )}
        {filtered.map((session) => (
          <button
            key={session.key}
            onClick={() => navigate(`/chat/${session.key}`)}
            className="w-full px-6 py-3 flex items-center gap-4 border-b transition-colors text-left group"
            style={{
              borderColor: 'var(--codex-border-subtle)',
              backgroundColor: 'transparent',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'; }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
          >
            {/* Session key */}
            <span
              className="text-[12px] w-20 flex-shrink-0"
              style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}
            >
              #{session.key.slice(0, 8)}
            </span>

            {/* Message count */}
            <span
              className="text-[11px] w-16 flex-shrink-0"
              style={{ color: 'var(--codex-fg-subtle)', fontFamily: 'var(--font-mono)' }}
            >
              {session.messageCount} msgs
            </span>

            {/* Duration */}
            <div className="flex items-center gap-1 text-[11px] w-16 flex-shrink-0" style={{ color: 'var(--codex-fg-subtle)' }}>
              <Clock className="w-3 h-3" strokeWidth={1.5} />
              {formatDuration(session.createdAt, session.updatedAt)}
            </div>

            {/* Created date */}
            <span className="text-[11px] flex-1" style={{ color: 'var(--codex-fg-subtle)' }}>
              {formatTimeAgo(session.createdAt)}
            </span>

            {/* Last active */}
            <span className="text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
              Active {formatTimeAgo(session.updatedAt)}
            </span>

            {/* Delete */}
            <button
              onClick={(e) => handleDelete(e, session.key)}
              disabled={deleting === session.key}
              className="p-1.5 rounded opacity-0 group-hover:opacity-100 transition-opacity"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) => { e.currentTarget.style.color = '#ef4444'; }}
              onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--codex-fg-subtle)'; }}
              title="Delete session"
            >
              <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
          </button>
        ))}
      </div>
    </div>
  );
}
