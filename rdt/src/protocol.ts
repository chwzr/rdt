import { writeVarString, createEncoder, toUint8Array } from "lib0/encoding";
import { readVarString, createDecoder } from "lib0/decoding";
import { ClientMessageUnion, ServerMessageUnion } from "./types";

/**
 * Encode a client message using lib0 format
 */
export function encodeClientMessage(message: ClientMessageUnion): Uint8Array {
  const encoder = createEncoder();
  const json = JSON.stringify(message);
  writeVarString(encoder, json);
  return toUint8Array(encoder);
}

/**
 * Decode a server message from lib0 format
 */
export function decodeServerMessage(data: Uint8Array): ServerMessageUnion {
  const decoder = createDecoder(data);
  const json = readVarString(decoder);
  return JSON.parse(json) as ServerMessageUnion;
}

/**
 * Create a subscribe message
 */
export function createSubscribeMessage(
  documentId: string,
  mapKey: string,
): ClientMessageUnion {
  return {
    type: "Subscribe",
    document_id: documentId,
    map_key: mapKey,
  };
}

/**
 * Create an unsubscribe message
 */
export function createUnsubscribeMessage(
  documentId: string,
  mapKey: string,
): ClientMessageUnion {
  return {
    type: "Unsubscribe",
    document_id: documentId,
    map_key: mapKey,
  };
}

/**
 * Create a get full state message
 */
export function createGetFullStateMessage(
  documentId: string,
  mapKey: string,
): ClientMessageUnion {
  return {
    type: "GetFullState",
    document_id: documentId,
    map_key: mapKey,
  };
}
