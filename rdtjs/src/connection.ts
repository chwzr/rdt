import {
  RdtConnectionOptions,
  ServerMessageUnion,
  ClientMessageUnion,
  FullStateMessage,
  MapChangeMessage,
  ErrorMessage,
  DocumentMap,
  BatchMapChangeMessage,
} from "./types";
import { encodeClientMessage, decodeServerMessage } from "./protocol";

export type ConnectionState =
  | "connecting"
  | "connected"
  | "disconnected"
  | "error";

export interface RdtConnectionEvents {
  stateChange: (state: ConnectionState) => void;
  error: (error: Error) => void;
  fullState: (message: FullStateMessage) => void;
  mapChange: (message: MapChangeMessage) => void;
  batchMapChange: (message: BatchMapChangeMessage) => void;
}

export class RdtConnection {
  private ws: WebSocket | null = null;
  private options: Required<RdtConnectionOptions>;
  private state: ConnectionState = "disconnected";
  private eventListeners: Partial<RdtConnectionEvents> = {};
  private reconnectAttempts = 0;
  private reconnectTimer: number | null = null;
  private subscriptions = new Set<string>();

  constructor(options: RdtConnectionOptions) {
    this.options = {
      reconnectInterval: 5000,
      maxReconnectAttempts: 10,
      ...options,
    };
  }

  /**
   * Connect to the WebSocket server
   */
  async connect(): Promise<void> {
    if (this.state === "connected" || this.state === "connecting") {
      return;
    }

    this.setState("connecting");

    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.options.url);
        this.ws.binaryType = "arraybuffer";

        this.ws.onopen = () => {
          this.setState("connected");
          this.reconnectAttempts = 0;
          this.clearReconnectTimer();
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const data = new Uint8Array(event.data);
            const message = decodeServerMessage(data);
            this.handleServerMessage(message);
          } catch (error) {
            this.emit("error", new Error(`Failed to decode message: ${error}`));
          }
        };

        this.ws.onclose = () => {
          this.setState("disconnected");
          this.scheduleReconnect();
        };

        this.ws.onerror = (error) => {
          this.setState("error");
          const err = new Error(`WebSocket error: ${error}`);
          this.emit("error", err);
          reject(err);
        };
      } catch (error) {
        this.setState("error");
        const err = new Error(`Failed to create WebSocket: ${error}`);
        this.emit("error", err);
        reject(err);
      }
    });
  }

  /**
   * Disconnect from the WebSocket server
   */
  disconnect(): void {
    this.clearReconnectTimer();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setState("disconnected");
  }

  /**
   * Subscribe to a document map
   */
  subscribe(documentId: string, mapKey: string): void {
    const subscriptionKey = `${documentId}:${mapKey}`;
    if (this.subscriptions.has(subscriptionKey)) {
      return;
    }
    console.log("subscribing to", documentId, mapKey);
    this.subscriptions.add(subscriptionKey);

    if (this.state === "connected") {
      this.sendMessage({
        type: "Subscribe",
        document_id: documentId,
        map_key: mapKey,
      });
    }
  }

  /**
   * Unsubscribe from a document map
   */
  unsubscribe(documentId: string, mapKey: string): void {
    const subscriptionKey = `${documentId}:${mapKey}`;
    this.subscriptions.delete(subscriptionKey);

    if (this.state === "connected") {
      this.sendMessage({
        type: "Unsubscribe",
        document_id: documentId,
        map_key: mapKey,
      });
    }
  }

  /**
   * Request full state of a document map
   */
  getFullState(documentId: string, mapKey: string): void {
    if (this.state === "connected") {
      this.sendMessage({
        type: "GetFullState",
        document_id: documentId,
        map_key: mapKey,
      });
    }
  }

  /**
   * Add event listener
   */
  on<K extends keyof RdtConnectionEvents>(
    event: K,
    listener: RdtConnectionEvents[K],
  ): void {
    this.eventListeners[event] = listener;
  }

  /**
   * Remove event listener
   */
  off<K extends keyof RdtConnectionEvents>(event: K): void {
    delete this.eventListeners[event];
  }

  /**
   * Get current connection state
   */
  getState(): ConnectionState {
    return this.state;
  }

  private sendMessage(message: ClientMessageUnion): void {
    if (!this.ws || this.state !== "connected") {
      throw new Error("WebSocket is not connected");
    }

    try {
      const encoded = encodeClientMessage(message);
      this.ws.send(encoded);
    } catch (error) {
      this.emit("error", new Error(`Failed to send message: ${error}`));
    }
  }

  private handleServerMessage(message: ServerMessageUnion): void {
    switch (message.type) {
      case "FullState":
        this.emit("fullState", message);
        break;

      case "MapChange":
        this.emit("mapChange", message);
        break;

      case "BatchMapChange":
        this.emit("batchMapChange", message);
        break;

      case "Error":
        this.emit("error", new Error(message.message));
        break;

      case "Ack":
        // Handle acknowledgment if needed
        break;
    }
  }

  private setState(newState: ConnectionState): void {
    if (this.state !== newState) {
      this.state = newState;
      this.emit("stateChange", newState);
    }
  }

  private emit<K extends keyof RdtConnectionEvents>(
    event: K,
    ...args: Parameters<RdtConnectionEvents[K]>
  ): void {
    const listener = this.eventListeners[event];
    if (listener) {
      // @ts-ignore - TypeScript can't infer the correct overload
      listener(...args);
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.options.maxReconnectAttempts) {
      return;
    }

    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectAttempts++;
      this.connect().catch(() => {
        // Connection failed, will schedule another attempt
      });
    }, this.options.reconnectInterval);
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      window.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}
