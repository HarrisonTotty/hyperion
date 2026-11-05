/**
 * A relay between a client and the server that holds the server's side of each connection open
 * once the client has left (the Mac batch's sky warm-up, `scripts/macProfile.mjs`).
 *
 * @remarks
 * The server cancels every request of a connection that closes (`ws.rs`), a sky's census among
 * them, and keeps a sky's reply for its next asking only once the reply is final
 * (`requests/sky/kept.rs`). An inspection launch ends long before its camera's cold sky does on the
 * Mac (about an hour of its four workers), so each launch would start the census again beside its
 * own windows. The batch's warm-up launch connects through this relay instead: the relay passes
 * the client's handshake and frames through byte for byte, but not its WebSocket close frame, and
 * when the client goes it keeps the server's socket open, reading and dropping what the server
 * sends, until {@link HoldingRelay.close}. The sky then runs to its final reply and is kept, and
 * every later launch asking it is answered at once. It reads the client's bytes only to find the
 * frames' bounds; nothing is decoded.
 */

import { createConnection, createServer, type Socket } from "node:net";

/** A WebSocket frame's opcode for a close (RFC 6455 §5.5.1). */
const CLOSE_OPCODE = 0x8;

/** The end of an HTTP request's head. */
const HEAD_END = Buffer.from("\r\n\r\n", "latin1");

/**
 * The whole length of the WebSocket frame at the start of `bytes`, its header and payload, and its
 * opcode; or `null` where its header is not all there yet (RFC 6455 §5.2).
 */
export function frameAt(
  bytes: Buffer,
): { readonly opcode: number; readonly length: number } | null {
  const [first, second] = bytes;
  if (first === undefined || second === undefined) {
    return null;
  }
  const masked = (second & 0x80) !== 0;
  const short = second & 0x7f;
  const extended = short === 126 ? 2 : short === 127 ? 8 : 0;
  const header = 2 + extended + (masked ? 4 : 0);
  if (bytes.length < header) {
    return null;
  }
  const payload =
    short === 126
      ? bytes.readUInt16BE(2)
      : short === 127
        ? Number(bytes.readBigUInt64BE(2))
        : short;
  return { opcode: first & 0x0f, length: header + payload };
}

/**
 * The client's side of a connection, split into what the relay passes on: its HTTP upgrade head as
 * it came, then each whole frame but a close.
 */
export class ClientBytes {
  #pending: Buffer = Buffer.alloc(0);
  #upgraded = false;
  #closed = false;

  /** Whether the client's close frame has come, after which nothing more is passed on. */
  get closed(): boolean {
    return this.#closed;
  }

  /** The bytes of `chunk`, with those held from before, that may go on to the server now. */
  take(chunk: Buffer): ReadonlyArray<Buffer> {
    if (this.#closed) {
      return [];
    }
    this.#pending = Buffer.concat([this.#pending, chunk]);
    const out: Buffer[] = [];
    if (!this.#upgraded) {
      const end = this.#pending.indexOf(HEAD_END);
      if (end < 0) {
        return out;
      }
      out.push(this.#pending.subarray(0, end + HEAD_END.length));
      this.#pending = this.#pending.subarray(end + HEAD_END.length);
      this.#upgraded = true;
    }
    for (;;) {
      const frame = frameAt(this.#pending);
      if (frame === null || this.#pending.length < frame.length) {
        return out;
      }
      if (frame.opcode === CLOSE_OPCODE) {
        this.#closed = true;
        this.#pending = Buffer.alloc(0);
        return out;
      }
      out.push(this.#pending.subarray(0, frame.length));
      this.#pending = this.#pending.subarray(frame.length);
    }
  }
}

/** The part of a socket the relay uses, so that a test can stand in for one. */
export interface RelaySocket {
  on(event: "data", listener: (chunk: Buffer) => void): unknown;
  on(event: "end" | "close", listener: () => void): unknown;
  on(event: "error", listener: (error: Error) => void): unknown;
  write(chunk: Buffer): unknown;
  destroy(): unknown;
}

/** Where a relayed connection stands. */
export type RelayState = "open" | "held" | "ended";

/** One relayed connection: a client's socket and the server's. */
export interface RelayedConnection {
  readonly state: () => RelayState;
  /** Ends the connection, closing the server's socket and so cancelling its requests. */
  readonly end: () => void;
}

/**
 * Relays `client` to `upstream`, the server's socket, until the client leaves (its close frame, its
 * end or an error), and then holds `upstream` open, its bytes read and dropped, until
 * {@link RelayedConnection.end} or the server closes it.
 */
export function relayConnection(client: RelaySocket, upstream: RelaySocket): RelayedConnection {
  let state: RelayState = "open";
  const bytes = new ClientBytes();
  const hold = (): void => {
    if (state === "open") {
      state = "held";
      client.destroy();
    }
  };
  const end = (): void => {
    if (state !== "ended") {
      state = "ended";
      client.destroy();
      upstream.destroy();
    }
  };
  client.on("data", (chunk) => {
    if (state !== "open") {
      return;
    }
    for (const part of bytes.take(chunk)) {
      upstream.write(part);
    }
    if (bytes.closed) {
      hold();
    }
  });
  client.on("end", hold);
  client.on("close", hold);
  // An error is followed by `close`; the listener keeps it from being thrown.
  client.on("error", hold);
  upstream.on("data", (chunk) => {
    if (state === "open") {
      client.write(chunk);
    }
  });
  upstream.on("end", end);
  upstream.on("close", end);
  upstream.on("error", end);
  return { state: () => state, end };
}

/** A listening relay. */
export interface HoldingRelay {
  /** The port it listens on, on 127.0.0.1, for the client's `--port`. */
  readonly port: number;
  /** Its connections whose client has left, held open to the server. */
  readonly held: () => number;
  /** Stops listening and ends every connection, held or not. */
  readonly close: () => Promise<void>;
}

/** Starts a relay to the server at `port` on 127.0.0.1, listening on a port of its own. */
export async function startHoldingRelay(port: number): Promise<HoldingRelay> {
  const connections = new Set<RelayedConnection>();
  const server = createServer((client: Socket) => {
    connections.add(relayConnection(client, createConnection({ host: "127.0.0.1", port })));
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", reject);
      resolve();
    });
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("a relay listening on TCP has a port");
  }
  return {
    port: address.port,
    held: () => [...connections].filter((each) => each.state() === "held").length,
    close: async () => {
      const closed = new Promise<void>((resolve) => {
        server.close(() => {
          resolve();
        });
      });
      for (const each of connections) {
        each.end();
      }
      connections.clear();
      await closed;
    },
  };
}
