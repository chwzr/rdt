import { RdtClient } from "../src";

// Custom types for demonstration
interface User {
  id: number;
  name: string;
  email: string;
  isActive: boolean;
}

interface Product {
  id: string;
  name: string;
  price: number;
  inStock: boolean;
}

// Example usage with custom types using RdtClient
async function exampleUsage() {
  // Create and connect the RDT client
  const client = new RdtClient({ url: "ws://localhost:8080" });
  await client.connect();

  // Create a provider for User objects
  const userProvider = client.createProvider<User>({
    documentId: "my-app",
    mapKey: "users",
  });

  // Create a provider for Product objects
  const productProvider = client.createProvider<Product>({
    documentId: "my-app",
    mapKey: "products",
  });

  // Get the stores with proper typing
  const userStore = userProvider.getStore();
  const productStore = productProvider.getStore();

  // Subscribe to changes with type-safe callbacks
  userStore.subscribe((state) => {
    console.log("User data updated:", state.data);
    // state.data is typed as DocumentMap<User>
  });

  // Subscribe to specific key changes
  userProvider.subscribeToKey("user-123", (user) => {
    if (user) {
      // user is typed as User
      console.log(`User ${user.name} updated:`, user.email);
    }
  });

  // Type-safe access to data
  const specificUser = userStore.get("user-123"); // Returns User | undefined
  const allUsers = userStore.values(); // Returns User[]
  const userEntries = userStore.entries(); // Returns [string, User][]

  // Similarly for products
  const specificProduct = productStore.get("product-456"); // Returns Product | undefined
  const allProducts = productStore.values(); // Returns Product[]

  console.log("User store size:", userStore.size());
  console.log("Product store size:", productStore.size());

  // Cleanup when done
  client.disconnect();
}

// Example with JsonValue (backward compatibility)
async function legacyUsage() {
  // Create and connect the RDT client
  const client = new RdtClient({ url: "ws://localhost:8080" });
  await client.connect();

  // No generic parameter defaults to JsonValue
  const legacyProvider = client.createProvider({
    documentId: "legacy-app",
    mapKey: "data",
  });

  const store = legacyProvider.getStore();

  // Works with any JsonValue
  const value = store.get("key"); // Returns JsonValue | undefined
  const allValues = store.values(); // Returns JsonValue[]

  // Cleanup when done
  client.disconnect();
}

// Example with multiple document types
async function multiDocumentUsage() {
  const client = new RdtClient({
    url: "ws://localhost:8080",
    reconnectInterval: 3000,
    maxReconnectAttempts: 5,
  });

  await client.connect();

  // Different document types in the same client
  const gameUsersProvider = client.createProvider<User>({
    documentId: "game-session-1",
    mapKey: "players",
  });

  const chatMessagesProvider = client.createProvider<{
    message: string;
    userId: number;
    timestamp: number;
  }>({
    documentId: "game-session-1",
    mapKey: "chat",
  });

  const inventoryProvider = client.createProvider<Product>({
    documentId: "game-session-1",
    mapKey: "inventory",
  });

  // All providers share the same connection but manage different data
  console.log("Connection state:", client.getConnectionState());

  // Subscribe to all providers
  gameUsersProvider.subscribe((state) => {
    console.log(
      "Players updated:",
      Object.keys(state.data).length,
      "players online",
    );
  });

  chatMessagesProvider.subscribe((state) => {
    console.log(
      "New chat messages:",
      Object.keys(state.data).length,
      "messages",
    );
  });

  inventoryProvider.subscribe((state) => {
    console.log("Inventory updated:", Object.keys(state.data).length, "items");
  });

  // Cleanup - this will clean up all providers automatically
  client.disconnect();
}

export { exampleUsage, legacyUsage, multiDocumentUsage };
