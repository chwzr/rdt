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
  private eventListeners: Map<
    keyof RdtConnectionEvents,
    Set<(...args: any[]) => void>
  > = new Map();
  private reconnectAttempts = 0;
  private reconnectTimer: number | null = null;
  private subscriptions = new Set<string>();

  constructor(options: RdtConnectionOptions) {
    this.options = {
      reconnectInterval: 5000,
      maxReconnectAttempts: 10,
      debug: false,
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

          // Re-subscribe to all stored subscriptions on reconnect
          this.resubscribeAll();

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
          console.error("WebSocket connection error:", error);
          const err = new Error(
            `WebSocket connection failed - attempting to reconnect...`,
          );
          this.emit("error", err);
          this.scheduleReconnect();
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
    if (this.options.debug) {
      console.log("subscribing to", documentId, mapKey);
    }
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
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, new Set());
    }
    this.eventListeners.get(event)!.add(listener as (...args: any[]) => void);
  }

  /**
   * Remove event listener
   */
  off<K extends keyof RdtConnectionEvents>(
    event: K,
    listener?: RdtConnectionEvents[K],
  ): void {
    const listeners = this.eventListeners.get(event);
    if (!listeners) {
      return;
    }

    if (listener) {
      listeners.delete(listener as (...args: any[]) => void);
    } else {
      // If no specific listener provided, remove all listeners for this event
      listeners.clear();
    }
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
    const listeners = this.eventListeners.get(event);
    if (listeners && listeners.size > 0) {
      listeners.forEach((listener) => {
        try {
          listener(...args);
        } catch (error) {
          console.error(`Error in event listener for ${event}:`, error);
        }
      });
    }
  }

  private resubscribeAll(): void {
    if (this.options.debug) {
      console.log(
        "Resubscribing to",
        this.subscriptions.size,
        "subscriptions after reconnect",
      );
    }

    this.subscriptions.forEach((subscriptionKey) => {
      const [documentId, mapKey] = subscriptionKey.split(":");
      if (this.options.debug) {
        console.log("Resubscribing to", documentId, mapKey);
      }
      try {
        this.sendMessage({
          type: "Subscribe",
          document_id: documentId,
          map_key: mapKey,
        });
      } catch (error) {
        console.warn(`Failed to resubscribe to ${subscriptionKey}:`, error);
      }
    });

    // Request full state for all subscriptions after a small delay to ensure subscriptions are processed
    setTimeout(() => {
      if (this.options.debug) {
        console.log(
          "Requesting full state for",
          this.subscriptions.size,
          "subscriptions",
        );
      }
      this.subscriptions.forEach((subscriptionKey) => {
        const [documentId, mapKey] = subscriptionKey.split(":");
        if (this.options.debug) {
          console.log("Requesting full state for", documentId, mapKey);
        }
        try {
          this.sendMessage({
            type: "GetFullState",
            document_id: documentId,
            map_key: mapKey,
          });
        } catch (error) {
          console.warn(
            `Failed to request full state for ${subscriptionKey}:`,
            error,
          );
        }
      });
    }, 100); // Small delay to ensure subscriptions are processed first
  }

  private scheduleReconnect(): void {
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
