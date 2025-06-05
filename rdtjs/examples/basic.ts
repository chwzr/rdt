import RdtClient from "../src/index";

async function basicExample() {
  // Create RDT client
  const client = new RdtClient({
    url: "ws://localhost:8080/rdt",
    reconnectInterval: 5000,
    maxReconnectAttempts: 10,
  });

  try {
    // Connect to server
    console.log("Connecting to RDT server...");
    await client.connect();
    console.log("Connected!");

    // Create a provider for users data
    const usersProvider = client.createProvider({
      documentId: "app-state",
      mapKey: "users",
      options: {
        initialSync: true,
      },
    });

    // Get the store
    const usersStore = usersProvider.getStore();

    // Subscribe to changes
    const unsubscribe = usersProvider.subscribe((state) => {
      console.log("Users data updated:", state.data);
      console.log("Loading:", state.isLoading);
      console.log("Error:", state.error);
    });

    // Subscribe to a specific user
    const unsubscribeUser = usersProvider.subscribeToKey("user-123", (user) => {
      console.log("User 123 updated:", user);
    });

    // Use the store API
    console.log("Current users:", usersStore.getState().data);
    console.log("User count:", usersStore.size());
    console.log("User IDs:", usersStore.keys());

    // Simulate running for 30 seconds
    await new Promise((resolve) => setTimeout(resolve, 30000));

    // Cleanup
    unsubscribe();
    unsubscribeUser();
    client.disconnect();
  } catch (error) {
    console.error("Error:", error);
  }
}

// Run the example
basicExample().catch(console.error);
