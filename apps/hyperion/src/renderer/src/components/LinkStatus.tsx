import type { ConnectionStatus } from "../lib/connection";
import { LINK_STATUS_LABEL } from "../lib/serverLink";

interface LinkStatusProps {
  readonly status: ConnectionStatus;
}

/** Annunciator for the state of the server link, shown in the header strip of every console. */
export function LinkStatus({ status }: LinkStatusProps) {
  return (
    <output className={`annunciator annunciator--${status}`} aria-label="Server link">
      {LINK_STATUS_LABEL[status]}
    </output>
  );
}
