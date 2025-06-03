import { useEffect, useState, useCallback, useRef } from "react";
import { RdtProvider } from "./provider";
import { RdtConnection, ConnectionState } from "./connection";
import { RdtConnectionOptions, RdtProviderConfig, JsonValue } from "./types";

/**
 * React hook to manage RDT connection
 */
export function useRdtConnection(options: RdtConnectionOptions) {
  const connectionRef = useRef<RdtConnection | null>(null);
  const [connectionState, setConnectionState] =
    useState<ConnectionState>("disconnected");
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    const connection = new RdtConnection(options);
    connectionRef.current = connection;

    connection.on("stateChange", setConnectionState);
    connection.on("error", setError);

    // Auto-connect
    connection.connect().catch(setError);

    return () => {
      connection.disconnect();
    };
  }, [options.url]);

  const connect = useCallback(() => {
    return connectionRef.current?.connect() || Promise.resolve();
  }, []);

  const disconnect = useCallback(() => {
    connectionRef.current?.disconnect();
  }, []);

  return {
    connection: connectionRef.current,
    connectionState,
    error,
    connect,
    disconnect,
  };
}

/**
 * React hook to use an RDT provider with Zustand store
 */
export function useRdtProvider(
  connection: RdtConnection | null,
  config: RdtProviderConfig,
) {
  const providerRef = useRef<RdtProvider | null>(null);
  const [data, setData] = useState<Record<string, JsonValue>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!connection) return;

    const provider = new RdtProvider(connection, config);
    providerRef.current = provider;

    const store = provider.getStore();

    // Subscribe to store changes
    const unsubscribe = provider.subscribe((state) => {
      setData(state.data);
      setIsLoading(state.isLoading);
      setError(state.error);
    });

    return () => {
      unsubscribe();
      provider.destroy();
    };
  }, [connection, config.documentId, config.mapKey]);

  const get = useCallback((key: string) => data[key], [data]);
  const has = useCallback((key: string) => key in data, [data]);
  const keys = useCallback(() => Object.keys(data), [data]);
  const values = useCallback(() => Object.values(data), [data]);
  const entries = useCallback(() => Object.entries(data), [data]);
  const size = useCallback(() => Object.keys(data).length, [data]);

  return {
    data,
    isLoading,
    error,
    get,
    has,
    keys,
    values,
    entries,
    size,
    provider: providerRef.current,
  };
}

/**
 * React hook to subscribe to a specific key in an RDT map
 */
export function useRdtValue<T extends JsonValue = JsonValue>(
  connection: RdtConnection | null,
  config: RdtProviderConfig,
  key: string,
): {
  value: T | undefined;
  isLoading: boolean;
  error: string | null;
} {
  const [value, setValue] = useState<T | undefined>(undefined);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!connection) return;

    const provider = new RdtProvider(connection, config);

    const unsubscribe = provider.subscribeToKey(key, (newValue) => {
      setValue(newValue as T | undefined);
    });

    // Subscribe to loading and error states
    const unsubscribeState = provider.subscribe((state) => {
      setIsLoading(state.isLoading);
      setError(state.error);
    });

    return () => {
      unsubscribe();
      unsubscribeState();
      provider.destroy();
    };
  }, [connection, config.documentId, config.mapKey, key]);

  return { value, isLoading, error };
}
