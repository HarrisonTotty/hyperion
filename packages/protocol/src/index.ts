// The types in ./generated are produced by ts-rs from crates/hyperion-protocol
// (run `just gen-protocol`). Re-export each new message type here.
import type { ClientMessage } from "./generated/ClientMessage";
import type { ServerMessage } from "./generated/ServerMessage";

export type { ClientMessage, ServerMessage };

/** Serializes `message` into the JSON text frame the server expects. */
export function encodeClientMessage(message: ClientMessage): string {
  return JSON.stringify(message);
}

/**
 * Parses a JSON text frame received from the server.
 *
 * @throws SyntaxError if `data` is not valid JSON.
 */
export function decodeServerMessage(data: string): ServerMessage {
  // The server is the source of truth for these types — they are generated
  // from it — so its payloads are trusted rather than re-validated here.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return JSON.parse(data) as ServerMessage;
}
