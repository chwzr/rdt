// Types
export type {
  JsonValue,
  ClientMessageUnion,
  ServerMessageUnion,
  ChangeUnion,
  DocumentMap,
  RdtConnectionOptions,
  SubscriptionOptions,
  RdtProviderConfig,
  FullStateMessage,
  MapChangeMessage,
  BatchMapChangeMessage,
  ErrorMessage,
  AckMessage,
  InsertChange,
  UpdateChange,
  RemoveChange,
} from "./types";

// Connection
export {
  RdtConnection,
  type ConnectionState,
  type RdtConnectionEvents,
} from "./connection";

// Provider
export {
  RdtProvider,
  createRdtProvider,
  type RdtStore,
  type RdtStoreState,
} from "./provider";

// Protocol utilities
export {
  encodeClientMessage,
  decodeServerMessage,
  createSubscribeMessage,
  createUnsubscribeMessage,
  createGetFullStateMessage,
} from "./protocol";

// Import types for internal use
import { RdtConnection, ConnectionState } from "./connection";
import { RdtProvider, createRdtProvider } from "./provider";
import { RdtConnectionOptions, RdtProviderConfig, JsonValue } from "./types";

// Main class for convenience
export class RdtClient {
  private connection: RdtConnection;
  private providers: Map<string, RdtProvider<any>> = new Map();

  constructor(options: RdtConnectionOptions) {
    this.connection = new RdtConnection(options);
  }

  /**
   * Connect to the WebSocket server
   */
  async connect(): Promise<void> {
    return this.connection.connect();
  }

  /**
   * Disconnect from the WebSocket server
   */
  disconnect(): void {
    // Clean up all providers
    for (const provider of Array.from(this.providers.values())) {
      provider.destroy();
    }
    this.providers.clear();

    this.connection.disconnect();
  }

  /**
   * Create a provider for a document map
   */
  createProvider<T = JsonValue>(config: RdtProviderConfig): RdtProvider<T> {
    const key = `${config.documentId}:${config.mapKey}`;

    if (this.providers.has(key)) {
      return this.providers.get(key)! as RdtProvider<T>;
    }

    const provider = createRdtProvider<T>(this.connection, config);
    this.providers.set(key, provider);

    return provider;
  }

  /**
   * Get a provider for a document map
   */
  getProvider<T = JsonValue>(key: string): RdtProvider<T> {
    return this.providers.get(key)! as RdtProvider<T>;
  }

  /**
   * Get the underlying connection
   */
  getConnection(): RdtConnection {
    return this.connection;
  }

  /**
   * Get connection state
   */
  getConnectionState(): ConnectionState {
    return this.connection.getState();
  }
}

// Default export
export default RdtClient;
