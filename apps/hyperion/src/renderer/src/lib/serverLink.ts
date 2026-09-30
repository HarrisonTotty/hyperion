import type { RequestClient } from "@hyperion/protocol";
import { createContext, useContext, useMemo } from "react";

import type { ConnectionStatus, ServerConnection } from "./connection";

/**
 * The words for each state of the server link, shared by the header's annunciator (`LinkStatus`)
 * and every control that the link's state inhibits.
 */
export const LINK_STATUS_LABEL = {
  connecting: "ESTABLISHING LINK",
  connected: "LINK NOMINAL",
  disconnected: "NO CARRIER",
  incompatible: "LINK INCOMPATIBLE",
} as const satisfies Record<ConnectionStatus, string>;

/** What a display needs of the server link to make requests: its state and its request client. */
export interface ServerLink {
  readonly status: ConnectionStatus;
  /** The one request client of the link, which lives as long as the connection. */
  readonly requests: RequestClient;
}

/**
 * Hands the server link to every display, so that any of them can make requests over the client's
 * one socket.
 *
 * @remarks
 * `App` provides it. `null` outside a provider, which {@link useServerLink} reports.
 */
export const ServerLinkContext = createContext<ServerLink | null>(null);

/**
 * The server link from the nearest {@link ServerLinkContext}.
 *
 * @throws Error when no provider is above the caller, which is a wiring bug.
 */
export function useServerLink(): ServerLink {
  const link = useContext(ServerLinkContext);
  if (link === null) {
    throw new Error("useServerLink needs a ServerLinkContext provider above it, as App supplies");
  }
  return link;
}

/**
 * The value to provide as {@link ServerLinkContext} for a connection.
 *
 * @remarks
 * Memoised on the link's status and request client alone, so that a latency update, which
 * changes the connection every ping, re-renders no consumer.
 */
export function useServerLinkValue(connection: ServerConnection): ServerLink {
  const { status, requests } = connection;
  return useMemo(() => ({ status, requests }), [status, requests]);
}

/**
 * Why requests cannot be made in this state of the link, in the header annunciator's words, or
 * `null` when the link is up.
 */
export function linkDownReason(status: ConnectionStatus): string | null {
  let reason: string | null;
  switch (status) {
    case "connected":
      reason = null;
      break;
    case "connecting":
    case "disconnected":
    case "incompatible":
      reason = LINK_STATUS_LABEL[status];
      break;
  }
  return reason;
}
