import { RdtConnection, createRdtProvider } from "../src";

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

// Example usage with custom types
async function exampleUsage() {
  const connection = new RdtConnection({ url: "ws://localhost:8080" });
  await connection.connect();

  // Create a provider for User objects
  const userProvider = createRdtProvider<User>(connection, {
    documentId: "my-app",
    mapKey: "users",
  });

  // Create a provider for Product objects
  const productProvider = createRdtProvider<Product>(connection, {
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
}

// Example with JsonValue (backward compatibility)
async function legacyUsage() {
  const connection = new RdtConnection({ url: "ws://localhost:8080" });
  await connection.connect();

  // No generic parameter defaults to JsonValue
  const legacyProvider = createRdtProvider(connection, {
    documentId: "legacy-app",
    mapKey: "data",
  });

  const store = legacyProvider.getStore();

  // Works with any JsonValue
  const value = store.get("key"); // Returns JsonValue | undefined
  const allValues = store.values(); // Returns JsonValue[]
}

export { exampleUsage, legacyUsage };
