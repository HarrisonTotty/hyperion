import {
  type ClientMessage,
  decodeServerMessage,
  encodeClientMessage,
  PROTOCOL_VERSION,
  RequestClient,
} from "@hyperion/protocol";
import { useEffect, useState } from "react";

/** Where a locally run hyperion-server listens; used unless `VITE_HYPERION_SERVER_URL` is set. */
export const DEFAULT_SERVER_URL = "ws://127.0.0.1:7878/ws";

const PING_INTERVAL_MS = 2_000;
const RECONNECT_DELAY_MS = 2_000;

/**
 * Lifecycle of the server link.
 *
 * @remarks
 * `connected` means the server has answered our hello with our protocol version. `incompatible`
 * means it answered with another version, which no retry can fix.
 */
export type ConnectionStatus = "connecting" | "connected" | "disconnected" | "incompatible";

/** Health of the server link, as reported by {@link useServerConnection}. */
export interface ConnectionState {
  readonly status: ConnectionStatus;
  /** Version reported in the server's welcome; `null` until welcomed. */
  readonly serverVersion: string | null;
  /** Protocol version reported in the server's welcome; `null` until welcomed. */
  readonly serverProtocolVersion: number | null;
  /**
   * Generator version reported in the server's welcome; `null` until welcomed, and while the link
   * is incompatible, since another protocol version's welcome need not carry it.
   */
  readonly serverGeneratorVersion: number | null;
  /** Round-trip time of the latest ping in milliseconds; `null` until the first pong. */
  readonly latencyMs: number | null;
}

/** The server link: its health, and the client that makes requests over it. */
export interface ServerConnection extends ConnectionState {
  /**
   * Makes requests over the link. It lives as long as the hook, across reconnects; a request made
   * while the link is not `connected` settles as `link_lost`.
   */
  readonly requests: RequestClient;
}

const INITIAL_STATE: ConnectionState = {
  status: "connecting",
  serverVersion: null,
  serverProtocolVersion: null,
  serverGeneratorVersion: null,
  latencyMs: null,
};

const DISCONNECTED_STATE: ConnectionState = { ...INITIAL_STATE, status: "disconnected" };

function send(socket: WebSocket, message: ClientMessage): void {
  socket.send(encodeClientMessage(message));
}

/** The request client and the socket it may write to, kept for the life of the hook. */
class RequestLink {
  #socket: WebSocket | null = null;

  readonly requests = new RequestClient((message) => {
    if (this.#socket === null) {
      return false;
    }
    send(this.#socket, message);
    return true;
  });

  /** Lets requests use `socket`, once it has been welcomed with our protocol version. */
  attach(socket: WebSocket): void {
    this.#socket = socket;
  }

  /**
   * Stops requests using `socket` and settles every request in flight as `link_lost`, if `socket`
   * is the one they use.
   *
   * @remarks
   * Every request in flight was sent on the attached socket, so a socket that was never attached,
   * or one already replaced whose close event arrives late, has none to lose.
   */
  detach(socket: WebSocket | null): void {
    if (socket === null || this.#socket !== socket) {
      return;
    }
    this.#socket = null;
    this.requests.linkLost();
  }
}

/**
 * Maintains a WebSocket connection to the hyperion-server at `url`,
 * reconnecting automatically, and reports its health.
 *
 * @remarks
 * One effect owns the socket, the ping and the reconnection. Every server message passes through
 * the request client before the link's own handling, and the client learns of every drop.
 *
 * @param clientVersion - Sent to the server in the opening hello.
 */
export function useServerConnection(url: string, clientVersion: string): ServerConnection {
  const [state, setState] = useState<ConnectionState>(INITIAL_STATE);
  const [link] = useState(() => new RequestLink());

  useEffect(() => {
    let socket: WebSocket | null = null;
    let pingTimer: ReturnType<typeof setInterval> | undefined;
    let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
    let disposed = false;
    let nextNonce = 0;
    const pingSentAt = new Map<number, number>();

    const connect = (): void => {
      setState((previous) => ({ ...previous, status: "connecting" }));
      const current = new WebSocket(url);
      socket = current;

      current.addEventListener("open", () => {
        send(current, { type: "hello", client_version: clientVersion });
      });

      current.addEventListener("message", (event: MessageEvent<unknown>) => {
        if (typeof event.data !== "string") {
          return;
        }
        const message = decodeServerMessage(event.data);
        if (link.requests.handleServerMessage(message)) {
          return;
        }
        switch (message.type) {
          case "welcome":
            if (message.protocol_version !== PROTOCOL_VERSION) {
              // A retry reaches the same server, so the socket stays open and idle and the
              // operator sees why nothing works.
              setState({
                status: "incompatible",
                serverVersion: message.server_version,
                serverProtocolVersion: message.protocol_version,
                serverGeneratorVersion: null,
                latencyMs: null,
              });
              break;
            }
            link.attach(current);
            setState({
              status: "connected",
              serverVersion: message.server_version,
              serverProtocolVersion: message.protocol_version,
              serverGeneratorVersion: message.generator_version,
              latencyMs: null,
            });
            pingTimer = setInterval(() => {
              const nonce = nextNonce++;
              pingSentAt.set(nonce, performance.now());
              send(current, { type: "ping", nonce });
            }, PING_INTERVAL_MS);
            break;
          case "pong": {
            const sentAt = pingSentAt.get(message.nonce);
            pingSentAt.delete(message.nonce);
            if (sentAt !== undefined) {
              const latencyMs = performance.now() - sentAt;
              setState((previous) => ({ ...previous, latencyMs }));
            }
            break;
          }
          case "error":
            console.error("server rejected message:", message.message);
            break;
          case "response":
          case "request_error":
            // The request client consumes every one of these above.
            break;
        }
      });

      current.addEventListener("close", () => {
        clearInterval(pingTimer);
        pingSentAt.clear();
        link.detach(current);
        if (disposed) {
          return;
        }
        setState(DISCONNECTED_STATE);
        reconnectTimer = setTimeout(connect, RECONNECT_DELAY_MS);
      });
    };

    connect();

    return () => {
      disposed = true;
      clearInterval(pingTimer);
      clearTimeout(reconnectTimer);
      link.detach(socket);
      socket?.close();
    };
  }, [url, clientVersion, link]);

  return { ...state, requests: link.requests };
}
