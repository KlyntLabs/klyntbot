/**
 * WebSocket client for klyntbot agent streaming.
 *
 * Connects to /ws, auto-reconnects after 2 seconds on disconnect,
 * and buffers messages sent before the connection is open.
 */

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'reconnecting';

export interface AgentSocketEvent {
  type: string;
  [key: string]: unknown;
}

type MessageHandler = (event: AgentSocketEvent) => void;
type StatusHandler = (status: ConnectionStatus) => void;

const RECONNECT_DELAY_MS = 2000;

export class AgentSocket {
  private ws: WebSocket | null = null;
  private status: ConnectionStatus = 'disconnected';
  private readonly messageHandlers: MessageHandler[] = [];
  private readonly statusHandlers: StatusHandler[] = [];
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionalClose = false;
  private readonly pendingMessages: string[] = [];

  constructor(private readonly wsUrl?: string) {
    this.connect();
  }

  private buildWsUrl(): string {
    if (typeof window !== 'undefined' && window.location.host) {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      return `${protocol}//${window.location.host}/ws`;
    }
    return 'ws://localhost/ws';
  }

  private connect(): void {
    this.intentionalClose = false;
    const url = this.wsUrl ?? this.buildWsUrl();

    this.setStatus('connecting');
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.setStatus('connected');
      // Flush buffered messages
      const pending = this.pendingMessages.splice(0);
      for (const msg of pending) {
        this.ws?.send(msg);
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
      if (!this.intentionalClose) {
        this.setStatus('reconnecting');
        this.reconnectTimer = setTimeout(
          () => this.connect(),
          RECONNECT_DELAY_MS,
        );
      } else {
        this.setStatus('disconnected');
      }
    };

    this.ws.onerror = () => {
      // onclose fires after onerror — handled there
    };

    this.ws.onmessage = (event: MessageEvent<string>) => {
      try {
        const parsed = JSON.parse(event.data) as AgentSocketEvent;
        for (const handler of this.messageHandlers) {
          handler(parsed);
        }
      } catch {
        console.error('AgentSocket: failed to parse message', event.data);
      }
    };
  }

  private setStatus(status: ConnectionStatus): void {
    this.status = status;
    for (const handler of this.statusHandlers) {
      handler(status);
    }
  }

  getStatus(): ConnectionStatus {
    return this.status;
  }

  onMessage(handler: MessageHandler): () => void {
    this.messageHandlers.push(handler);
    return () => {
      const idx = this.messageHandlers.indexOf(handler);
      if (idx !== -1) this.messageHandlers.splice(idx, 1);
    };
  }

  onStatusChange(handler: StatusHandler): () => void {
    this.statusHandlers.push(handler);
    return () => {
      const idx = this.statusHandlers.indexOf(handler);
      if (idx !== -1) this.statusHandlers.splice(idx, 1);
    };
  }

  private sendRaw(data: unknown): void {
    const json = JSON.stringify(data);
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(json);
    } else {
      this.pendingMessages.push(json);
    }
  }

  sendChatMessage(sessionKey: string | undefined, message: string): void {
    this.sendRaw({ type: 'chat.send', sessionKey: sessionKey ?? '', message });
  }

  sendCancel(): void {
    this.sendRaw({ type: 'chat.cancel' });
  }

  sendInteractionResponse(requestId: string, response: unknown): void {
    this.sendRaw({ type: 'interaction.respond', requestId, response });
  }

  disconnect(): void {
    this.intentionalClose = true;
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setStatus('disconnected');
  }
}
