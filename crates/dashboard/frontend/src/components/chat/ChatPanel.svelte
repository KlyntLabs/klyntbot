<script lang="ts">
  import MessageBubble from './MessageBubble.svelte';
  import ToolCallCard from './ToolCallCard.svelte';
  import StreamingIndicator from './StreamingIndicator.svelte';
  import type { ChatMessage } from '../../lib/types/index';

  interface Props {
    messages?: ChatMessage[];
    session?: { id: string; name: string } | null;
    streaming?: boolean;
    online?: boolean;
    onSend?: (text: string) => void;
    model?: string;
  }

  let {
    messages = [],
    session = null,
    streaming = false,
    online = true,
    onSend = () => {},
    model = 'Claude Sonnet 4.5',
  }: Props = $props();

  let inputText = $state('');
  let threadEl: HTMLElement | undefined = $state(undefined);
  // Local state — initialized once from prop; user can change independently
  let selectedModel = $state('Claude Sonnet 4.5');
  let selectedMode = $state('Normal');

  const models = ['Claude Sonnet 4.5', 'Claude Opus 4.6', 'Claude Haiku 4.5'];
  const modes = ['Normal', 'Extended', 'Concise'];

  function handleKeydown(e: KeyboardEvent) {
    // Enter sends message, Shift+Enter adds new line
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      const text = inputText.trim();
      if (text && !streaming) {
        inputText = '';
        onSend(text);
        // Reset textarea height after sending
        const target = e.target as HTMLTextAreaElement;
        target.style.height = '52px';
      }
    }
  }

  function sendMessage() {
    const text = inputText.trim();
    if (!text) return;
    inputText = '';
    onSend(text);
  }

  function handleInput(e: Event) {
    const el = e.target as HTMLTextAreaElement;
    el.style.height = 'auto';
    el.style.height = Math.min(el.scrollHeight, 150) + 'px';
  }

  $effect(() => {
    if (messages.length && threadEl) {
      setTimeout(() => threadEl?.scrollTo({ top: threadEl!.scrollHeight, behavior: 'smooth' }), 10);
    }
  });
</script>

<div class="chat-panel">
  {#if !online}
    <div class="offline-banner">Reconnecting...</div>
  {/if}

  <div
    class="message-thread"
    bind:this={threadEl}
    role="log"
    aria-live="polite"
    aria-label="Chat messages"
  >
    {#if messages.length === 0}
      <div class="empty-state">
        <p>Start a conversation</p>
      </div>
    {:else}
      {#each messages as message (message.id)}
        <MessageBubble
          role={message.role}
          content={message.content}
          timestamp={message.timestamp}
        />
        {#if message.toolCalls}
          {#each message.toolCalls as call}
            <ToolCallCard
              name={call.name}
              inputs={call.inputs}
              outputs={call.outputs}
              status={call.status}
            />
          {/each}
        {/if}
      {/each}
      <StreamingIndicator active={streaming} />
    {/if}
  </div>

  <div class="chat-footer">
    <div class="input-card">
      <!-- Row 1: Text input -->
      <textarea
        bind:value={inputText}
        onkeydown={handleKeydown}
        oninput={handleInput}
        placeholder="Ask Klyntbot anything, @ to add files, / for commands"
        aria-label="Chat message input"
      ></textarea>

      <!-- Row 2: Controls toolbar -->
      <div class="controls-row">
        <div class="controls-left">
          <!-- Model selector -->
          <select
            bind:value={selectedModel}
            class="ctrl-select"
            aria-label="Model selector"
          >
            {#each models as m}
              <option value={m}>{m}</option>
            {/each}
          </select>

          <!-- Mode selector -->
          <select
            bind:value={selectedMode}
            class="ctrl-select"
            aria-label="Response mode selector"
          >
            {#each modes as mode}
              <option value={mode}>{mode}</option>
            {/each}
          </select>
        </div>

        <div class="controls-right">
          <!-- Microphone button (placeholder) -->
          <button class="icon-btn" aria-label="Voice input" disabled title="Voice input coming soon">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z"/>
              <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
              <line x1="12" y1="19" x2="12" y2="22"/>
            </svg>
          </button>

          <!-- Send button -->
          <button
            class="send-btn"
            onclick={sendMessage}
            disabled={!inputText.trim() || streaming}
            aria-label="Send message"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <line x1="12" y1="19" x2="12" y2="5"/>
              <polyline points="5 12 12 5 19 12"/>
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .chat-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #0D1117;
    border-left: 1px solid #30363D;
  }

  .offline-banner {
    background: #FF8C00;
    color: #0D1117;
    text-align: center;
    padding: 8px;
    font-size: 13px;
    font-weight: 500;
  }

  .message-thread {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #6E7681;
    font-size: 14px;
  }

  .chat-footer {
    padding: 12px 14px 14px;
    background: #0D1117;
  }

  .input-card {
    display: flex;
    flex-direction: column;
    background: #161B22;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    transition: border-color 150ms;
    overflow: hidden;
  }

  .input-card:focus-within {
    border-color: rgba(255, 140, 0, 0.4);
    box-shadow: 0 0 0 3px rgba(255, 140, 0, 0.08);
  }

  textarea {
    background: transparent;
    border: none;
    color: #E6EDF3;
    padding: 12px 14px 8px;
    font-size: 13px;
    resize: none;
    font-family: inherit;
    line-height: 1.5;
    min-height: 52px;
    max-height: 150px;
    height: 52px;
    overflow-y: auto;
    display: block;
    width: 100%;
    box-sizing: border-box;
  }

  textarea:focus { outline: none; }

  textarea::placeholder { color: #4D5562; }

  /* Controls toolbar */
  .controls-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    gap: 8px;
  }

  .controls-left {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }

  .controls-right {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  /* Dropdown selects styled as pill buttons */
  .ctrl-select {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    color: #8B949E;
    font-size: 11px;
    font-family: inherit;
    padding: 4px 22px 4px 8px;
    cursor: pointer;
    appearance: none;
    -webkit-appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%238B949E' stroke-width='1.5' fill='none' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 7px center;
    transition: border-color 150ms, color 150ms;
    max-width: 140px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ctrl-select:hover {
    border-color: rgba(255, 255, 255, 0.15);
    color: #C9D1D9;
  }

  .ctrl-select:focus {
    outline: none;
    border-color: rgba(255, 140, 0, 0.4);
    color: #C9D1D9;
  }

  /* Microphone icon button */
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    color: #6E7681;
    cursor: pointer;
    transition: background 150ms, color 150ms;
    padding: 0;
  }

  .icon-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: #8B949E;
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* Send button — circular orange */
  .send-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: #FF8C00;
    border: none;
    border-radius: 50%;
    color: #0D1117;
    cursor: pointer;
    transition: background 150ms, transform 100ms, opacity 150ms;
    padding: 0;
    flex-shrink: 0;
  }

  .send-btn:hover:not(:disabled) {
    background: #E67E00;
    transform: scale(1.05);
  }

  .send-btn:active:not(:disabled) {
    transform: scale(0.95);
  }

  .send-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
</style>
