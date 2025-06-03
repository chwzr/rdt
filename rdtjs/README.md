# @rdt/client

Client library for RDT (Realtime Document Synchronization) framework. This library provides WebSocket-based real-time synchronization between your frontend application and RDT server with seamless Zustand store integration.

## Features

- 🔄 Real-time bidirectional synchronization
- 🏪 Built-in Zustand store integration
- 🔌 WebSocket connection management with auto-reconnection
- 📦 lib0 encoding/decoding for efficient messaging
- 🎯 TypeScript support with full type safety
- ⚛️ React hooks for easy integration
- 🔍 Document and map-level subscriptions
- 🎨 DashMap-like API for familiar usage

## Installation

```bash
npm install @rdt/client zustand lib0
```

## Quick Start

### Basic Usage

```typescript
import RdtClient from "@rdt/client";

// Create and connect to RDT server
const client = new RdtClient({
  url: "ws://localhost:8080/ws",
});

await client.connect();

// Create a provider for a document map
const userProvider = client.createProvider({
  documentId: "app-state",
  mapKey: "users",
});

// Get the Zustand store
const userStore = userProvider.getStore();

// Use the store
console.log(userStore.getState().data); // Current users data
console.log(userStore.get("user-123")); // Get specific user
console.log(userStore.keys()); // Get all user IDs
```

### With React Hooks

```tsx
import { useRdtConnection, useRdtProvider } from "@rdt/client/react";

function App() {
  const { connection, connectionState } = useRdtConnection({
    url: "ws://localhost:8080/ws",
  });

  const { data, isLoading, error, get } = useRdtProvider(connection, {
    documentId: "app-state",
    mapKey: "users",
  });

  if (isLoading) return <div>Loading...</div>;
  if (error) return <div>Error: {error}</div>;

  return (
    <div>
      <h1>Users ({Object.keys(data).length})</h1>
      {Object.entries(data).map(([id, user]) => (
        <div key={id}>{JSON.stringify(user)}</div>
      ))}
    </div>
  );
}
```

### Subscribe to Specific Values

```tsx
import { useRdtValue } from "@rdt/client/react";

function UserProfile({ userId }: { userId: string }) {
  const { value: user, isLoading } = useRdtValue(
    connection,
    { documentId: "app-state", mapKey: "users" },
    userId,
  );

  if (isLoading) return <div>Loading user...</div>;
  if (!user) return <div>User not found</div>;

  return <div>Welcome, {user.name}!</div>;
}
```

## API Reference

### RdtClient

Main client class for managing connections and providers.

```typescript
const client = new RdtClient(options: RdtConnectionOptions);

// Methods
await client.connect(): Promise<void>
client.disconnect(): void
client.createProvider(config: RdtProviderConfig): RdtProvider
client.getConnection(): RdtConnection
client.getConnectionState(): ConnectionState
```

### RdtConnection

Low-level WebSocket connection management.

```typescript
const connection = new RdtConnection(options: RdtConnectionOptions);

// Methods
await connection.connect(): Promise<void>
connection.disconnect(): void
connection.subscribe(documentId: string, mapKey: string): void
connection.unsubscribe(documentId: string, mapKey: string): void
connection.getFullState(documentId: string, mapKey: string): void
connection.on(event: string, listener: Function): void
connection.off(event: string): void
connection.getState(): ConnectionState
```

### RdtProvider

Document map provider with Zustand store integration.

```typescript
const provider = new RdtProvider(connection: RdtConnection, config: RdtProviderConfig);

// Methods
provider.getStore(): RdtStore
provider.subscribe(listener: Function): () => void
provider.subscribeToKey(key: string, listener: Function): () => void
provider.destroy(): void
```

### RdtStore

Enhanced Zustand store with DashMap-like API.

```typescript
const store = provider.getStore();

// Properties
store.data: DocumentMap
store.isLoading: boolean
store.error: string | null

// Methods
store.get(key: string): JsonValue | undefined
store.has(key: string): boolean
store.keys(): string[]
store.values(): JsonValue[]
store.entries(): [string, JsonValue][]
store.size(): number

// Zustand methods
store.getState(): StoreState
store.setState(partial: Partial<StoreState>): void
store.subscribe(listener: Function): () => void
```

## Configuration

### RdtConnectionOptions

```typescript
interface RdtConnectionOptions {
  url: string; // WebSocket server URL
  reconnectInterval?: number; // Reconnection interval in ms (default: 5000)
  maxReconnectAttempts?: number; // Max reconnection attempts (default: 10)
}
```

### RdtProviderConfig

```typescript
interface RdtProviderConfig {
  documentId: string; // Document identifier
  mapKey: string; // Map key within the document
  options?: {
    initialSync?: boolean; // Request initial state (default: true)
  };
}
```

## Events

### Connection Events

```typescript
connection.on("stateChange", (state: ConnectionState) => {
  console.log("Connection state:", state);
});

connection.on("error", (error: Error) => {
  console.error("Connection error:", error);
});

connection.on("fullState", (message: FullStateMessage) => {
  console.log("Received full state:", message);
});

connection.on("mapChange", (message: MapChangeMessage) => {
  console.log("Map changed:", message);
});
```

## Message Protocol

The library uses lib0 encoding for efficient WebSocket communication:

### Client Messages

- `Subscribe` - Subscribe to map updates
- `Unsubscribe` - Unsubscribe from map updates
- `GetFullState` - Request current map state

### Server Messages

- `FullState` - Complete map data
- `MapChange` - Incremental change (Insert/Update/Remove)
- `Error` - Error message
- `Ack` - Acknowledgment

## Advanced Usage

### Multiple Documents

```typescript
const client = new RdtClient({ url: "ws://localhost:8080/ws" });
await client.connect();

// Different document maps
const usersProvider = client.createProvider({
  documentId: "users-doc",
  mapKey: "profiles",
});

const settingsProvider = client.createProvider({
  documentId: "app-config",
  mapKey: "settings",
});
```

### Error Handling

```typescript
const { connection, error } = useRdtConnection({
  url: "ws://localhost:8080/ws",
});

useEffect(() => {
  if (error) {
    console.error("RDT Connection Error:", error);
    // Handle connection errors
  }
}, [error]);
```

### Manual Connection Management

```typescript
const connection = new RdtConnection({ url: "ws://localhost:8080/ws" });

connection.on("stateChange", (state) => {
  if (state === "connected") {
    console.log("Connected to RDT server");
  } else if (state === "disconnected") {
    console.log("Disconnected from RDT server");
  }
});

await connection.connect();
```

## Contributing

Please read our contributing guidelines and submit pull requests to our repository.

## License

MIT
