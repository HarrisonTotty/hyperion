import type { ConnectionState } from "../lib/connection";

const STATUS_LABEL = {
  connecting: "ESTABLISHING LINK",
  connected: "LINK NOMINAL",
  disconnected: "NO CARRIER",
} as const satisfies Record<ConnectionState["status"], string>;

interface ConnectionPanelProps {
  readonly url: string;
  readonly connection: ConnectionState;
}

/** Readout of the link to the game server: status, endpoint, server version and latency. */
export function ConnectionPanel({ url, connection }: ConnectionPanelProps) {
  const { status, serverVersion, latencyMs } = connection;
  return (
    <section className="panel" aria-label="Server link">
      <h2 className="panel__title">Server Link</h2>
      <output className={`status status--${status}`}>{STATUS_LABEL[status]}</output>
      <dl className="readout">
        <dt>Endpoint</dt>
        <dd>{url}</dd>
        <dt>Server</dt>
        <dd>{serverVersion ?? "—"}</dd>
        <dt>Latency</dt>
        <dd>{latencyMs === null ? "—" : `${latencyMs.toFixed(1)} ms`}</dd>
      </dl>
    </section>
  );
}
