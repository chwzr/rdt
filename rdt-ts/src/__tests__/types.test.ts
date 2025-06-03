import {
  JsonValue,
  ClientMessageUnion,
  ServerMessageUnion,
  ChangeUnion,
  RdtConnectionOptions,
  RdtProviderConfig,
} from "../types";

describe("Types", () => {
  test("JsonValue should accept valid JSON values", () => {
    const string: JsonValue = "hello";
    const number: JsonValue = 42;
    const boolean: JsonValue = true;
    const nullValue: JsonValue = null;
    const array: JsonValue = [1, 2, 3];
    const object: JsonValue = { key: "value" };

    expect(typeof string).toBe("string");
    expect(typeof number).toBe("number");
    expect(typeof boolean).toBe("boolean");
    expect(nullValue).toBeNull();
    expect(Array.isArray(array)).toBe(true);
    expect(typeof object).toBe("object");
  });

  test("ClientMessageUnion should have proper structure", () => {
    const subscribeMessage: ClientMessageUnion = {
      type: "Subscribe",
      document_id: "doc-1",
      map_key: "map-1",
    };

    const unsubscribeMessage: ClientMessageUnion = {
      type: "Unsubscribe",
      document_id: "doc-1",
      map_key: "map-1",
    };

    const getFullStateMessage: ClientMessageUnion = {
      type: "GetFullState",
      document_id: "doc-1",
      map_key: "map-1",
    };

    expect(subscribeMessage.type).toBe("Subscribe");
    expect(unsubscribeMessage.type).toBe("Unsubscribe");
    expect(getFullStateMessage.type).toBe("GetFullState");
  });

  test("RdtConnectionOptions should have proper structure", () => {
    const options: RdtConnectionOptions = {
      url: "ws://localhost:8080/ws",
      reconnectInterval: 5000,
      maxReconnectAttempts: 10,
    };

    expect(options.url).toBe("ws://localhost:8080/ws");
    expect(options.reconnectInterval).toBe(5000);
    expect(options.maxReconnectAttempts).toBe(10);
  });

  test("RdtProviderConfig should have proper structure", () => {
    const config: RdtProviderConfig = {
      documentId: "doc-1",
      mapKey: "map-1",
      options: {
        initialSync: true,
      },
    };

    expect(config.documentId).toBe("doc-1");
    expect(config.mapKey).toBe("map-1");
    expect(config.options?.initialSync).toBe(true);
  });
});
