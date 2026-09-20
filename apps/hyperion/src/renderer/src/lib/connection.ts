import { type ClientMessage, decodeServerMessage, encodeClientMessage } from "@hyperion/protocol";
import { useEffect, useState } from "react";

/** Where a locally run hyperion-server listens; used unless `VITE_HYPERION_SERVER_URL` is set. */
export const DEFAULT_SERVER_URL = "ws://127.0.0.1:7878/ws";

const PING_INTERVAL_MS = 2_000;
const RECONNECT_DELAY_MS = 2_000;

/** Lifecycle of the server link. `connected` means the server has answered our hello. */
export type ConnectionStatus = "connecting" | "connected" | "disconnected";

/** Health of the server link, as reported by {@link useServerConnection}. */
export interface ConnectionState {
  readonly status: ConnectionStatus;
  /** Version reported in the server's welcome; `null` until connected. */
  readonly serverVersion: string | null;
  /** Round-trip time of the latest ping in milliseconds; `null` until the first pong. */
  readonly latencyMs: number | null;
}

const INITIAL_STATE: ConnectionState = {
  status: "connecting",
  serverVersion: null,
  latencyMs: null,
};

function send(socket: WebSocket, message: ClientMessage): void {
  socket.send(encodeClientMessage(message));
}

/**
 * Maintains a WebSocket connection to the hyperion-server at `url`,
 * reconnecting automatically, and reports its health.
 *
 * @param clientVersion - Sent to the server in the opening hello.
 */
export function useServerConnection(url: string, clientVersion: string): ConnectionState {
  const [state, setState] = useState<ConnectionState>(INITIAL_STATE);

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
        switch (message.type) {
          case "welcome":
            setState({
              status: "connected",
              serverVersion: message.server_version,
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
        }
      });

      current.addEventListener("close", () => {
        clearInterval(pingTimer);
        pingSentAt.clear();
        if (disposed) {
          return;
        }
        setState({ status: "disconnected", serverVersion: null, latencyMs: null });
        reconnectTimer = setTimeout(connect, RECONNECT_DELAY_MS);
      });
    };

    connect();

    return () => {
      disposed = true;
      clearInterval(pingTimer);
      clearTimeout(reconnectTimer);
      socket?.close();
    };
  }, [url, clientVersion]);

  return state;
}
