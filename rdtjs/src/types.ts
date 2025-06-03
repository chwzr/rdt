export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface ClientMessage {
  type: "Subscribe" | "Unsubscribe" | "GetFullState";
}

export interface SubscribeMessage extends ClientMessage {
  type: "Subscribe";
  document_id: string;
  map_key: string;
}

export interface UnsubscribeMessage extends ClientMessage {
  type: "Unsubscribe";
  document_id: string;
  map_key: string;
}

export interface GetFullStateMessage extends ClientMessage {
  type: "GetFullState";
  document_id: string;
  map_key: string;
}

export type ClientMessageUnion =
  | SubscribeMessage
  | UnsubscribeMessage
  | GetFullStateMessage;

export interface ServerMessage {
  type: "FullState" | "MapChange" | "BatchMapChange" | "Error" | "Ack";
}

export interface FullStateMessage<T = JsonValue> extends ServerMessage {
  type: "FullState";
  document_id: string;
  map_key: string;
  data: Record<string, T>;
}

export interface MapChangeMessage<T = JsonValue> extends ServerMessage {
  type: "MapChange";
  document_id: string;
  map_key: string;
  change: Change<T>;
}

export interface BatchMapChangeMessage<T = JsonValue> extends ServerMessage {
  type: "BatchMapChange";
  document_id: string;
  map_key: string;
  changes: ChangeUnion<T>[];
}

export interface ErrorMessage extends ServerMessage {
  type: "Error";
  message: string;
}

export interface AckMessage extends ServerMessage {
  type: "Ack";
  request_id?: string;
}

export type ServerMessageUnion<T = JsonValue> =
  | FullStateMessage<T>
  | MapChangeMessage<T>
  | BatchMapChangeMessage<T>
  | ErrorMessage
  | AckMessage;

export interface Change<T = JsonValue> {
  op: "Insert" | "Update" | "Remove";
}

export interface InsertChange<T = JsonValue> extends Change<T> {
  op: "Insert";
  key: string;
  value: T;
}

export interface UpdateChange<T = JsonValue> extends Change<T> {
  op: "Update";
  key: string;
  old_value: T;
  new_value: T;
}

export interface RemoveChange<T = JsonValue> extends Change<T> {
  op: "Remove";
  key: string;
  old_value: T;
}

export type ChangeUnion<T = JsonValue> =
  | InsertChange<T>
  | UpdateChange<T>
  | RemoveChange<T>;

export interface DocumentMap<T = JsonValue> {
  [key: string]: T;
}

export interface RdtConnectionOptions {
  url: string;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

export interface SubscriptionOptions {
  initialSync?: boolean;
}

export interface RdtProviderConfig {
  documentId: string;
  mapKey: string;
  options?: SubscriptionOptions;
}
