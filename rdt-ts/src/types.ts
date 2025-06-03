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

export interface FullStateMessage extends ServerMessage {
  type: "FullState";
  document_id: string;
  map_key: string;
  data: Record<string, JsonValue>;
}

export interface MapChangeMessage extends ServerMessage {
  type: "MapChange";
  document_id: string;
  map_key: string;
  change: Change;
}

export interface BatchMapChangeMessage extends ServerMessage {
  type: "BatchMapChange";
  document_id: string;
  map_key: string;
  changes: ChangeUnion[];
}

export interface ErrorMessage extends ServerMessage {
  type: "Error";
  message: string;
}

export interface AckMessage extends ServerMessage {
  type: "Ack";
  request_id?: string;
}

export type ServerMessageUnion =
  | FullStateMessage
  | MapChangeMessage
  | BatchMapChangeMessage
  | ErrorMessage
  | AckMessage;

export interface Change {
  op: "Insert" | "Update" | "Remove";
}

export interface InsertChange extends Change {
  op: "Insert";
  key: string;
  value: JsonValue;
}

export interface UpdateChange extends Change {
  op: "Update";
  key: string;
  old_value: JsonValue;
  new_value: JsonValue;
}

export interface RemoveChange extends Change {
  op: "Remove";
  key: string;
  old_value: JsonValue;
}

export type ChangeUnion = InsertChange | UpdateChange | RemoveChange;

export interface DocumentMap {
  [key: string]: JsonValue;
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
