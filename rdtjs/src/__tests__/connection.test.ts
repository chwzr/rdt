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

  describe("retry functionality", () => {
    it("should succeed on second attempt after first failure", async () => {
      const reconnectInterval = 50;

      connection = new RdtConnection({
        url: "ws://localhost:1234",
        reconnectInterval: reconnectInterval,
      });

      const stateChangeSpy = jest.fn();
      const errorSpy = jest.fn();
      const consoleErrorSpy = jest.spyOn(console, "error").mockImplementation();
      connection.on("stateChange", stateChangeSpy);
      connection.on("error", errorSpy);

      // Mock WebSocket to fail first time, succeed second time
      const originalWebSocket = global.WebSocket;
      let attemptCount = 0;
      let wsInstance: any;

      const MockWebSocket = jest.fn().mockImplementation((url) => {
        attemptCount++;
        wsInstance = {
          url,
          binaryType: "arraybuffer",
          onopen: null,
          onclose: null,
          onerror: null,
          onmessage: null,
          close: jest.fn(),
          send: jest.fn(),
        };

        if (attemptCount === 1) {
          // First attempt: simulate failure
          setTimeout(() => {
            if (wsInstance.onerror) {
              wsInstance.onerror(new Error("Connection failed"));
            }
          }, 10);
        } else {
          // Second attempt: simulate success
          setTimeout(() => {
            if (wsInstance.onopen) {
              wsInstance.onopen();
            }
          }, 10);
        }

        return wsInstance;
      });

      // Add WebSocket constants to the mock
      (MockWebSocket as any).CONNECTING = 0;
      (MockWebSocket as any).OPEN = 1;
      (MockWebSocket as any).CLOSING = 2;
      (MockWebSocket as any).CLOSED = 3;

      global.WebSocket = MockWebSocket as any;

      try {
        // Single call to connect() - should eventually succeed after internal retry
        const connectPromise = connection.connect();

        // Wait for the first attempt to fail and retry to be scheduled
        await new Promise((resolve) =>
          setTimeout(resolve, reconnectInterval + 50),
        );

        // The connection should eventually succeed
        await connectPromise;

        // Verify the connection state
        expect(connection.getState()).toBe("connected");

        // Verify WebSocket was called twice (first attempt fails, second succeeds)
        expect(global.WebSocket).toHaveBeenCalledTimes(2);

        // Verify state transitions
        expect(stateChangeSpy).toHaveBeenCalledWith("connecting");
        expect(stateChangeSpy).toHaveBeenCalledWith("error");
        expect(stateChangeSpy).toHaveBeenCalledWith("connecting");
        expect(stateChangeSpy).toHaveBeenCalledWith("connected");

        // Verify error was emitted for the first failure
        expect(errorSpy).toHaveBeenCalledTimes(1);

        // Verify console.error was called for the connection failure
        expect(consoleErrorSpy).toHaveBeenCalledWith(
          "WebSocket connection error:",
          expect.any(Error),
        );
      } finally {
        // Restore original WebSocket and console.error
        global.WebSocket = originalWebSocket;
        consoleErrorSpy.mockRestore();
      }
    });

    it("should prevent double reconnection when both onclose and onerror fire", async () => {
      const reconnectInterval = 50;

      connection = new RdtConnection({
        url: "ws://localhost:1234",
        reconnectInterval: reconnectInterval,
        maxReconnectAttempts: 3, // Limit attempts for this test
      });

      const stateChangeSpy = jest.fn();
      const errorSpy = jest.fn();
      const consoleErrorSpy = jest.spyOn(console, "error").mockImplementation();
      connection.on("stateChange", stateChangeSpy);
      connection.on("error", errorSpy);

      // Mock WebSocket to fire both onclose and onerror on first attempt, succeed on second
      const originalWebSocket = global.WebSocket;
      let attemptCount = 0;
      let wsInstance: any;

      const MockWebSocket = jest.fn().mockImplementation((url) => {
        attemptCount++;
        wsInstance = {
          url,
          binaryType: "arraybuffer",
          onopen: null,
          onclose: null,
          onerror: null,
          onmessage: null,
          close: jest.fn(),
          send: jest.fn(),
        };

        if (attemptCount === 1) {
          // First attempt: fire both onclose and onerror
          setTimeout(() => {
            if (wsInstance.onerror) {
              wsInstance.onerror(new Error("Connection failed"));
            }
            // Fire onclose immediately after onerror
            setTimeout(() => {
              if (wsInstance.onclose) {
                wsInstance.onclose();
              }
            }, 5);
          }, 10);
        } else {
          // Second attempt: succeed
          setTimeout(() => {
            if (wsInstance.onopen) {
              wsInstance.onopen();
            }
          }, 10);
        }

        return wsInstance;
      });

      // Add WebSocket constants to the mock
      (MockWebSocket as any).CONNECTING = 0;
      (MockWebSocket as any).OPEN = 1;
      (MockWebSocket as any).CLOSING = 2;
      (MockWebSocket as any).CLOSED = 3;

      global.WebSocket = MockWebSocket as any;

      try {
        // Attempt to connect - should fail and trigger retries
        const connectPromise = connection.connect();

        // Wait for the first attempt to fail and retry to be scheduled
        await new Promise((resolve) =>
          setTimeout(resolve, reconnectInterval + 50),
        );

        // The connection should eventually succeed (second attempt)
        await connectPromise;

        // Verify the connection state
        expect(connection.getState()).toBe("connected");

        // Verify WebSocket was called exactly twice (not more due to double reconnection)
        expect(global.WebSocket).toHaveBeenCalledTimes(2);

        // Verify state transitions
        expect(stateChangeSpy).toHaveBeenCalledWith("connecting");
        expect(stateChangeSpy).toHaveBeenCalledWith("error");
        expect(stateChangeSpy).toHaveBeenCalledWith("disconnected");
        expect(stateChangeSpy).toHaveBeenCalledWith("connecting");
        expect(stateChangeSpy).toHaveBeenCalledWith("connected");

        // Verify error was emitted for the first failure
        expect(errorSpy).toHaveBeenCalledTimes(1);

        // Verify console.error was called for the connection failure
        expect(consoleErrorSpy).toHaveBeenCalledWith(
          "WebSocket connection error:",
          expect.any(Error),
        );
      } finally {
        // Restore original WebSocket and console.error
        global.WebSocket = originalWebSocket;
        consoleErrorSpy.mockRestore();
      }
    });
  });
});
