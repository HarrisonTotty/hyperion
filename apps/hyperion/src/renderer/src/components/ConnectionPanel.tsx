import type { ConnectionState } from "../lib/connection";

interface ConnectionPanelProps {
  readonly url: string;
  readonly clientVersion: string;
  readonly connection: ConnectionState;
}

// Groups digits only from five digits up, as the UX guidelines require.
const WHOLE_NUMBER = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 0,
  useGrouping: "min2",
});

function formatLatency(latencyMs: number): string {
  return latencyMs < 1 ? "<1 ms" : `${WHOLE_NUMBER.format(latencyMs)} ms`;
}

interface ReadingProps {
  /** The formatted value, or `null` when the value is missing. */
  readonly value: string | null;
}

function Reading({ value }: ReadingProps) {
  return value === null ? <dd className="readout__missing">—</dd> : <dd>{value}</dd>;
}

/** Details of the link to the game server: endpoint, versions and latency. */
export function ConnectionPanel({ url, clientVersion, connection }: ConnectionPanelProps) {
  const { serverVersion, latencyMs } = connection;
  return (
    <section className="panel" aria-labelledby="server-link-title">
      <h2 className="panel__title" id="server-link-title">
        Server Link
      </h2>
      <dl className="readout">
        <dt>Endpoint</dt>
        <Reading value={url} />
        <dt>Client Ver</dt>
        <Reading value={clientVersion} />
        <dt>Server Ver</dt>
        <Reading value={serverVersion} />
        <dt>Latency</dt>
        <Reading value={latencyMs === null ? null : formatLatency(latencyMs)} />
      </dl>
    </section>
  );
}
