import { PROTOCOL_VERSION } from "@hyperion/protocol";

import type { ConnectionState } from "../lib/connection";
import { formatNumber } from "../lib/format";

interface ConnectionPanelProps {
  readonly url: string;
  readonly clientVersion: string;
  readonly connection: ConnectionState;
}

function formatLatency(latencyMs: number): string {
  return latencyMs < 1 ? "<1 ms" : `${formatNumber(latencyMs, 0)} ms`;
}

interface ReadingProps {
  /** The formatted value, or `null` when the value is missing. */
  readonly value: string | null;
}

function Reading({ value }: ReadingProps) {
  return value === null ? <dd className="readout__missing">—</dd> : <dd>{value}</dd>;
}

interface ProtocolReadingProps {
  /** The server's protocol version, or `null` until it has sent one. */
  readonly server: number | null;
}

/**
 * The server's protocol version beside this client's, which must be equal for the link to work.
 *
 * @remarks
 * The two are named in full, as the rows above name them, since no abbreviation for either is on
 * the ship's nomenclature list.
 */
function ProtocolReading({ server }: ProtocolReadingProps) {
  return (
    <dd>
      SERVER {server === null ? <span className="readout__missing">—</span> : server} / CLIENT{" "}
      {PROTOCOL_VERSION}
    </dd>
  );
}

/** Details of the link to the game server: endpoint, versions and latency. */
export function ConnectionPanel({ url, clientVersion, connection }: ConnectionPanelProps) {
  const { serverVersion, serverProtocolVersion, latencyMs } = connection;
  return (
    <section className="panel" aria-labelledby="server-link-title">
      <h2 className="panel__title" id="server-link-title">
        Server Link
      </h2>
      <dl className="readout">
        <dt>Endpoint</dt>
        <Reading value={url} />
        <dt>Client Version</dt>
        <Reading value={clientVersion} />
        <dt>Server Version</dt>
        <Reading value={serverVersion} />
        <dt>Protocol</dt>
        <ProtocolReading server={serverProtocolVersion} />
        <dt>Latency</dt>
        <Reading value={latencyMs === null ? null : formatLatency(latencyMs)} />
      </dl>
    </section>
  );
}
