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
export { RdtProvider, createRdtProvider, type RdtStore } from "./provider";

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
import { RdtConnectionOptions, RdtProviderConfig } from "./types";

// Main class for convenience
export class RdtClient {
  private connection: RdtConnection;
  private providers: Map<string, RdtProvider> = new Map();

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
    for (const provider of this.providers.values()) {
      provider.destroy();
    }
    this.providers.clear();

    this.connection.disconnect();
  }

  /**
   * Create a provider for a document map
   */
  createProvider(config: RdtProviderConfig): RdtProvider {
    const key = `${config.documentId}:${config.mapKey}`;

    if (this.providers.has(key)) {
      return this.providers.get(key)!;
    }

    const provider = createRdtProvider(this.connection, config);
    this.providers.set(key, provider);

    return provider;
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
