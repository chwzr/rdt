import { RdtConnection } from "../connection";
import WS from "jest-websocket-mock";

describe("RdtConnection", () => {
  let server: WS;
  let connection: RdtConnection;

  beforeEach(() => {
    // Create a new WebSocket server for each test
    server = new WS("ws://localhost:1234");
    connection = new RdtConnection({
      url: "ws://localhost:1234",
    });
  });

  afterEach(() => {
    // Clean up after each test
    WS.clean();
    connection.disconnect();
  });

  describe("connect()", () => {
    it("should successfully connect to WebSocket server", async () => {
      // Start the connection
      const connectPromise = connection.connect();

      // Wait for the connection to be established
      await server.connected;

      // Wait for the connect promise to resolve
      await connectPromise;

      // Verify the connection state
      expect(connection.getState()).toBe("connected");
    });

    it("should emit stateChange event when connecting", async () => {
      const stateChangeSpy = jest.fn();
      connection.on("stateChange", stateChangeSpy);

      // Start the connection
      const connectPromise = connection.connect();

      // Wait for the connection to be established
      await server.connected;
      await connectPromise;

      // Verify state change events were emitted
      expect(stateChangeSpy).toHaveBeenCalledWith("connecting");
      expect(stateChangeSpy).toHaveBeenCalledWith("connected");
    });

    it("should not connect if already connected", async () => {
      // First connection
      const connectPromise1 = connection.connect();
      await server.connected;
      await connectPromise1;

      expect(connection.getState()).toBe("connected");

      // Attempt second connection (should not create new WebSocket)
      await connection.connect();

      // State should still be connected
      expect(connection.getState()).toBe("connected");
    });

    it("should not connect if already connecting", async () => {
      // Start first connection
      const connectPromise1 = connection.connect();

      // Start second connection while first is still connecting
      const connectPromise2 = connection.connect();

      // Both should resolve when connection is established
      await server.connected;
      await Promise.all([connectPromise1, connectPromise2]);

      expect(connection.getState()).toBe("connected");
    });
  });
});
