import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import {
  RdtProviderConfig,
  DocumentMap,
  JsonValue,
  ChangeUnion,
  FullStateMessage,
  MapChangeMessage,
  BatchMapChangeMessage,
  InsertChange,
  UpdateChange,
  RemoveChange,
} from "./types";
import { RdtConnection } from "./connection";

export interface RdtStore<T = JsonValue> {
  data: DocumentMap<T>;
  isLoading: boolean;
  error: string | null;
  get: (key: string) => T | undefined;
  set: (key: string, value: T) => void;
  delete: (key: string) => void;
  has: (key: string) => boolean;
  keys: () => string[];
  values: () => T[];
  entries: () => [string, T][];
  size: () => number;
  clear: () => void;
  getState: () => RdtStoreState<T>;
  setState: (partial: Partial<RdtStoreState<T>>) => void;
  subscribe: (listener: (state: RdtStoreState<T>) => void) => () => void;
}

interface RdtStoreState<T = JsonValue> {
  data: DocumentMap<T>;
  isLoading: boolean;
  error: string | null;
}

export class RdtProvider<T = JsonValue> {
  private connection: RdtConnection;
  private config: RdtProviderConfig;
  private store: any;
  private subscriptionKey: string;

  constructor(connection: RdtConnection, config: RdtProviderConfig) {
    this.connection = connection;
    this.config = config;
    this.subscriptionKey = `${config.documentId}:${config.mapKey}`;

    this.store = create<RdtStoreState<T>>()(
      subscribeWithSelector((set, get) => ({
        data: {},
        isLoading: true,
        error: null,
      })),
    );

    this.setupEventListeners();
    this.initialize();
  }

  /**
   * Get the Zustand store with enhanced API
   */
  getStore(): RdtStore<T> {
    const store = this.store;

    return {
      ...store,
      get: (key: string) => store.getState().data[key],
      set: (key: string, value: T) => {
        // Note: This is read-only from client perspective
        // All mutations should come from the server
        console.warn(
          "Direct mutations are not allowed. State is managed by the server.",
        );
      },
      delete: (key: string) => {
        console.warn(
          "Direct mutations are not allowed. State is managed by the server.",
        );
      },
      has: (key: string) => key in store.getState().data,
      keys: () => Object.keys(store.getState().data),
      values: () => Object.values(store.getState().data),
      entries: () => Object.entries(store.getState().data),
      size: () => Object.keys(store.getState().data).length,
      clear: () => {
        console.warn(
          "Direct mutations are not allowed. State is managed by the server.",
        );
      },
    };
  }

  /**
   * Subscribe to store changes
   */
  subscribe(listener: (state: RdtStoreState<T>) => void): () => void {
    return this.store.subscribe(listener);
  }

  /**
   * Subscribe to specific key changes
   */
  subscribeToKey(
    key: string,
    listener: (value: T | undefined) => void,
  ): () => void {
    return this.store.subscribe(
      (state: RdtStoreState<T>) => state.data[key],
      listener,
    );
  }

  /**
   * Destroy the provider and clean up resources
   */
  destroy(): void {
    this.connection.unsubscribe(this.config.documentId, this.config.mapKey);
    this.connection.off("fullState");
    this.connection.off("mapChange");
    this.connection.off("batchMapChange");
    this.connection.off("error");
  }

  private setupEventListeners(): void {
    this.connection.on("fullState", (message: FullStateMessage) => {
      if (this.isMessageForThisProvider(message.document_id, message.map_key)) {
        this.handleFullState(message as FullStateMessage<T>);
      }
    });

    this.connection.on("mapChange", (message: MapChangeMessage) => {
      if (this.isMessageForThisProvider(message.document_id, message.map_key)) {
        this.handleMapChange(message as MapChangeMessage<T>);
      }
    });

    this.connection.on("batchMapChange", (message: BatchMapChangeMessage) => {
      if (this.isMessageForThisProvider(message.document_id, message.map_key)) {
        this.handleBatchMapChange(message as BatchMapChangeMessage<T>);
      }
    });

    this.connection.on("error", (error: Error) => {
      this.store.setState({ error: error.message });
    });

    this.connection.on("stateChange", (state) => {
      if (state === "connected") {
        // Re-subscribe when connection is restored
        this.connection.subscribe(this.config.documentId, this.config.mapKey);
        if (this.config.options?.initialSync !== false) {
          this.connection.getFullState(
            this.config.documentId,
            this.config.mapKey,
          );
        }
      } else if (state === "disconnected" || state === "error") {
        this.store.setState({ isLoading: true });
      }
    });
  }

  private async initialize(): Promise<void> {
    // Subscribe to the document map
    this.connection.subscribe(this.config.documentId, this.config.mapKey);

    // Request initial state if enabled
    if (this.config.options?.initialSync !== false) {
      this.connection.getFullState(this.config.documentId, this.config.mapKey);
    } else {
      this.store.setState({ isLoading: false });
    }
  }

  private handleFullState(message: FullStateMessage<T>): void {
    this.store.setState({
      data: message.data,
      isLoading: false,
      error: null,
    });
  }

  private handleMapChange(message: MapChangeMessage<T>): void {
    const change = message.change as ChangeUnion<T>;
    const currentData = this.store.getState().data;

    switch (change.op) {
      case "Insert": {
        const insertChange = change as InsertChange<T>;
        this.store.setState({
          data: {
            ...currentData,
            [insertChange.key]: insertChange.value,
          },
        });
        break;
      }

      case "Update": {
        const updateChange = change as UpdateChange<T>;
        this.store.setState({
          data: {
            ...currentData,
            [updateChange.key]: updateChange.new_value,
          },
        });
        break;
      }

      case "Remove": {
        const removeChange = change as RemoveChange<T>;
        const newData = { ...currentData };
        delete newData[removeChange.key];
        this.store.setState({ data: newData });
        break;
      }
    }
  }

  private handleBatchMapChange(message: BatchMapChangeMessage<T>): void {
    const currentData = this.store.getState().data;
    let newData = { ...currentData };

    // Apply each change sequentially to build the final state
    for (const change of message.changes) {
      newData = this.applyChangeToData(newData, change);
    }

    // Single state update for all changes
    this.store.setState({ data: newData });
  }

  private applyChangeToData(
    data: DocumentMap<T>,
    change: ChangeUnion<T>,
  ): DocumentMap<T> {
    const result = { ...data };

    switch (change.op) {
      case "Insert": {
        const insertChange = change as InsertChange<T>;
        result[insertChange.key] = insertChange.value;
        break;
      }

      case "Update": {
        const updateChange = change as UpdateChange<T>;
        result[updateChange.key] = updateChange.new_value;
        break;
      }

      case "Remove": {
        const removeChange = change as RemoveChange<T>;
        delete result[removeChange.key];
        break;
      }
    }

    return result;
  }

  private isMessageForThisProvider(
    documentId: string,
    mapKey: string,
  ): boolean {
    return (
      documentId === this.config.documentId && mapKey === this.config.mapKey
    );
  }
}

/**
 * Create a new RDT provider
 */
export function createRdtProvider<T = JsonValue>(
  connection: RdtConnection,
  config: RdtProviderConfig,
): RdtProvider<T> {
  return new RdtProvider<T>(connection, config);
}
