import type { ConnectionStatus } from "../lib/connection";

const STATUS_LABEL = {
  connecting: "ESTABLISHING LINK",
  connected: "LINK NOMINAL",
  disconnected: "NO CARRIER",
} as const satisfies Record<ConnectionStatus, string>;

interface LinkStatusProps {
  readonly status: ConnectionStatus;
}

/** Annunciator for the state of the server link, shown in the header strip of every console. */
export function LinkStatus({ status }: LinkStatusProps) {
  return (
    <output className={`annunciator annunciator--${status}`} aria-label="Server link">
      {STATUS_LABEL[status]}
    </output>
  );
}
