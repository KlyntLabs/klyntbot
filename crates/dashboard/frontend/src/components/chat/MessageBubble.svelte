<script lang="ts">
  export let role: 'user' | 'assistant';
  export let content: string;
  export let timestamp: Date;

  $: timeLabel = timestamp.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  $: ariaLabel = `${role === 'user' ? 'User' : 'Assistant'} message at ${timeLabel}`;
</script>

<article
  class="message-bubble"
  class:user={role === 'user'}
  class:assistant={role === 'assistant'}
  aria-label={ariaLabel}
  role="article"
>
  <div class="bubble-content">{content}</div>
  <time class="timestamp">{timeLabel}</time>
</article>

<style>
  .message-bubble {
    display: flex;
    flex-direction: column;
    max-width: 75%;
    margin: 4px 0;
  }

  .user {
    align-self: flex-end;
    align-items: flex-end;
  }

  .assistant {
    align-self: flex-start;
    align-items: flex-start;
  }

  .bubble-content {
    padding: 10px 14px;
    border-radius: 12px;
    font-size: 14px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .user .bubble-content {
    background: #FF8C00;
    color: #0D1117;
    border-bottom-right-radius: 4px;
  }

  .assistant .bubble-content {
    background: #21262D;
    color: #E6EDF3;
    border-bottom-left-radius: 4px;
  }

  .timestamp {
    font-size: 11px;
    color: #6E7681;
    margin-top: 2px;
    padding: 0 4px;
  }
</style>
