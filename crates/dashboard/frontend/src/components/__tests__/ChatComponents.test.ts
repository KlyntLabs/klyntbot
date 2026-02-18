/**
 * Chat component tests — ChatPanel, MessageBubble, ToolCallCard, StreamingIndicator.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import ChatPanel from '../chat/ChatPanel.svelte';
import MessageBubble from '../chat/MessageBubble.svelte';
import ToolCallCard from '../chat/ToolCallCard.svelte';
import StreamingIndicator from '../chat/StreamingIndicator.svelte';

const makeMessage = (overrides = {}) => ({
  id: `msg-${Math.random()}`,
  role: 'user' as const,
  content: 'Hello world',
  timestamp: new Date('2026-02-18T14:30:00Z'),
  ...overrides,
});

describe('ChatPanel', () => {
  it('AC-15.1: renders message thread', () => {
    const messages = Array.from({ length: 5 }, (_, i) =>
      makeMessage({ id: `msg-${i}`, role: i % 2 === 0 ? 'user' : 'assistant', content: `Message ${i}` })
    );
    const { container } = render(ChatPanel, { props: { messages } });
    const articles = container.querySelectorAll('[role="article"]');
    expect(articles.length).toBe(5);
  });

  it('AC-15.1: send message on Enter', async () => {
    const onSend = vi.fn();
    render(ChatPanel, { props: { messages: [], onSend } });
    const input = screen.getByRole('textbox');
    await fireEvent.input(input, { target: { value: 'Hello' } });
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: false });
    expect(onSend).toHaveBeenCalledWith('Hello');
  });

  it('Shift+Enter creates newline instead of sending', async () => {
    const onSend = vi.fn();
    render(ChatPanel, { props: { messages: [], onSend } });
    const input = screen.getByRole('textbox');
    await fireEvent.input(input, { target: { value: 'Hello' } });
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    expect(onSend).not.toHaveBeenCalled();
  });

  it('shows empty state for new conversation', () => {
    render(ChatPanel, { props: { messages: [] } });
    expect(screen.getByText('Start a conversation')).toBeTruthy();
  });

  it('CC-3.1: has log role and aria-live', () => {
    const { container } = render(ChatPanel, { props: { messages: [] } });
    const log = container.querySelector('[role="log"]');
    expect(log).toBeTruthy();
    expect(log?.getAttribute('aria-live')).toBe('polite');
  });

  it('CC-4.1: shows offline banner on disconnect', () => {
    render(ChatPanel, { props: { messages: [], online: false } });
    expect(screen.getByText('Reconnecting...')).toBeTruthy();
  });
});

describe('MessageBubble', () => {
  it('AC-15.2: user message styled differently from assistant', () => {
    const { container } = render(MessageBubble, {
      props: { role: 'user', content: 'Hi', timestamp: new Date() }
    });
    const article = container.querySelector('article');
    expect(article?.classList.contains('user')).toBe(true);
  });

  it('AC-15.2: assistant message with streaming content', () => {
    const { container } = render(MessageBubble, {
      props: { role: 'assistant', content: 'Hello there', timestamp: new Date() }
    });
    const article = container.querySelector('article');
    expect(article?.classList.contains('assistant')).toBe(true);
    expect(screen.getByText('Hello there')).toBeTruthy();
  });

  it('CC-3.1: has article role with timestamp label', () => {
    const ts = new Date('2026-02-18T14:30:00Z');
    const { container } = render(MessageBubble, {
      props: { role: 'assistant', content: 'Hi', timestamp: ts }
    });
    const article = container.querySelector('[role="article"]');
    expect(article).toBeTruthy();
    expect(article?.getAttribute('aria-label')).toContain('Assistant message at');
  });
});

describe('ToolCallCard', () => {
  it('AC-15.3: renders tool name and status', () => {
    render(ToolCallCard, {
      props: { name: 'todo', status: 'running', inputs: {}, outputs: {} }
    });
    expect(screen.getByText('todo')).toBeTruthy();
    const spinner = document.querySelector('.spinner');
    expect(spinner).toBeTruthy();
  });

  it('AC-15.3: completed tool shows result summary', () => {
    const { container } = render(ToolCallCard, {
      props: { name: 'search', status: 'completed', inputs: {}, outputs: { result: 'found 5 items' } }
    });
    expect(container.querySelector('.check')).toBeTruthy();
  });

  it('click expands to show full inputs/outputs', async () => {
    const { container } = render(ToolCallCard, {
      props: { name: 'todo', status: 'completed', inputs: { action: 'list' }, outputs: { items: [] } }
    });
    const btn = container.querySelector('button.header') as HTMLElement;
    await fireEvent.click(btn);
    expect(container.querySelector('.details')).toBeTruthy();
  });

  it('CC-3.1: has aria-expanded for collapse state', async () => {
    const { container } = render(ToolCallCard, {
      props: { name: 'todo', status: 'completed', inputs: {}, outputs: {} }
    });
    const btn = container.querySelector('button.header') as HTMLElement;
    expect(btn.getAttribute('aria-expanded')).toBe('false');
    await fireEvent.click(btn);
    expect(btn.getAttribute('aria-expanded')).toBe('true');
  });
});

describe('StreamingIndicator', () => {
  it('AC-15.2: shows typing dots when active', () => {
    const { container } = render(StreamingIndicator, { props: { active: true } });
    const dots = container.querySelectorAll('.dot');
    expect(dots.length).toBe(3);
  });

  it('hidden when not active', () => {
    const { container } = render(StreamingIndicator, { props: { active: false } });
    const indicator = container.querySelector('.streaming-indicator');
    expect(indicator).toBeNull();
  });
});
