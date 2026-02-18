/**
 * WebSocket chat client for streaming agent responses.
 * Connects to /ws/chat — separate protocol from GraphQL subscriptions.
 */

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  toolCalls?: ToolCall[];
}

export interface ToolCall {
  id: string;
  name: string;
  inputs: object;
  outputs: object;
  status: 'running' | 'completed' | 'failed';
}

export type AgentEvent =
  | { type: 'chunk'; content: string; messageId: string }
  | { type: 'tool_start'; toolId: string; name: string; inputs: object }
  | { type: 'tool_end'; toolId: string; outputs: object; success: boolean }
  | { type: 'done'; messageId: string }
  | { type: 'error'; error: string };

export type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error';

export interface ChatClientOptions {
  url?: string;
  onEvent?: (event: AgentEvent) => void;
  onConnectionChange?: (state: ConnectionState) => void;
  maxReconnectDelay?: number;
}

export class ChatWebSocketClient {
  private ws: WebSocket | null = null;
  private reconnectDelay = 1000;
  private maxReconnectDelay: number;
  private url: string;
  private onEvent: (event: AgentEvent) => void;
  private onConnectionChange: (state: ConnectionState) => void;
  private destroyed = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(options: ChatClientOptions = {}) {
    this.url = options.url ?? `ws://${window.location.host}/ws/chat`;
    this.onEvent = options.onEvent ?? (() => {});
    this.onConnectionChange = options.onConnectionChange ?? (() => {});
    this.maxReconnectDelay = options.maxReconnectDelay ?? 30000;
  }

  connect(): void {
    if (this.destroyed) return;
    this.onConnectionChange('connecting');
    try {
      this.ws = new WebSocket(this.url);
      this.ws.onopen = () => {
        this.reconnectDelay = 1000;
        this.onConnectionChange('connected');
      };
      this.ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data) as AgentEvent;
          this.onEvent(data);
        } catch (e) {
          console.error('[ChatWS] Failed to parse message:', e);
        }
      };
      this.ws.onclose = () => {
        if (!this.destroyed) this.scheduleReconnect();
      };
      this.ws.onerror = () => {
        this.onConnectionChange('error');
      };
    } catch (e) {
      this.scheduleReconnect();
    }
  }

  sendMessage(sessionId: string, content: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: 'message', sessionId, content }));
    } else {
      console.warn('[ChatWS] Cannot send — not connected');
    }
  }

  private scheduleReconnect(): void {
    this.onConnectionChange('disconnected');
    if (this.destroyed) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectDelay = Math.min(this.reconnectDelay * 2, this.maxReconnectDelay);
      this.connect();
    }, this.reconnectDelay);
  }

  destroy(): void {
    this.destroyed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.ws?.close();
    this.ws = null;
  }
}
