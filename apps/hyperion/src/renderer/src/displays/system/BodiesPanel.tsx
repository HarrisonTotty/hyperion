import { useId } from "react";

import { annunciation, RequestStatus } from "../../components/RequestStatus";
import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import { formatAge, formatSigned } from "../../lib/format";
import { architectureLabel, detailLevelLabel } from "../../lib/system/bodyWords";
import { formatHex64 } from "../../lib/seed";
import { BodyList } from "./BodyList";
import { BodyReadout } from "./BodyReadout";
import type { SystemViewState } from "./useSystemView";

/** Props of {@link BodiesPanel}. */
export interface BodiesPanelProps {
  readonly view: SystemViewState;
}

/**
 * The `BODIES` panel: the system named, its age and metallicity, the detail level its bodies were
 * granted and the architecture class of the primary's planets, its bodies as a tree, and everything
 * known about the one selected (plan 14, P14.T41.b and T43.a–b).
 *
 * @remarks
 * The list and the readout share one panel, as the chart's do, beside the orbit map they are paired
 * with. The system's age is its age at the time of the answer on show, which the display asks
 * again once its time has moved a year, and its metallicity is fixed at its birth. Until there is
 * an answer, or while the system is not yet formed, the panel says so in the orbit map's words
 * without being a live region of its own, since the map's status already announces them. An answer
 * that is a stale snapshot, its link down or its newer request failed, is muted and named stale,
 * with the guide's trailing `S`.
 */
export function BodiesPanel({ view }: BodiesPanelProps) {
  const titleId = useId();
  const { target, model, fault, stale, status, selected, bodies, primaryZone } = view;
  const formed = model?.formed === true ? model : null;
  const age = model === null ? null : formatAge(model.ageMyr);

  let empty: string | null = null;
  if (fault !== null) {
    empty = "SYSTEM DATA INVALID";
  } else if (model === null) {
    empty = annunciation(status)?.text ?? null;
  } else if (formed === null) {
    empty = "NOT YET FORMED";
  }

  // The record's own request stands under the readout, outside its live region: `PENDING` until
  // the whole record comes, then nothing, or why it did not.
  let detailStatus = null;
  if (selected?.kind === "body" && !selected.whole) {
    detailStatus =
      view.detailFault === null ? (
        <RequestStatus state={view.detailState} onRetry={view.retry} />
      ) : (
        <StatusLine
          text={`BODY DATA INVALID: ${view.detailFault}`}
          standing="fault"
          action={{ label: "RETRY", onAction: view.retry }}
        />
      );
  }

  return (
    <section
      className={
        stale
          ? "panel system__bodies bodies-panel bodies-panel--stale"
          : "panel system__bodies bodies-panel"
      }
      aria-labelledby={titleId}
    >
      <h2 className="panel__title" id={titleId}>
        Bodies{stale ? <StaleMark /> : null}
      </h2>
      <dl className="readout bodies-panel__system">
        <dt>SYSTEM</dt>
        <dd className="bodies-panel__wide">{target.designation}</dd>
        <dt>ID</dt>
        <dd className="bodies-panel__wide">{formatHex64(target.system)}</dd>
        <dt>AGE</dt>
        <dd>
          {age === null ? (
            <span className="readout__missing">—</span>
          ) : (
            <>
              {age.value} <span className="bodies-panel__unit">{age.unit}</span>
            </>
          )}
        </dd>
        <dt className="bodies-panel__symbol">[Fe/H]</dt>
        <dd>
          {model === null ? (
            <span className="readout__missing">—</span>
          ) : (
            <>
              {formatSigned(model.feHDex, 2)} <span className="bodies-panel__unit">dex</span>
            </>
          )}
        </dd>
        <dt>DETAIL</dt>
        <dd>
          {bodies === null ? (
            <span className="readout__missing">—</span>
          ) : (
            detailLevelLabel(bodies.granted)
          )}
        </dd>
        <dt>ARCH</dt>
        <dd>
          {primaryZone === null ? (
            <span className="readout__missing">—</span>
          ) : (
            architectureLabel(primaryZone.architecture)
          )}
        </dd>
      </dl>
      {empty === null ? (
        <>
          <BodyList
            rows={view.rows}
            selectedId={
              selected === null
                ? null
                : selected.kind === "host"
                  ? selected.host.id
                  : selected.body.id
            }
            onSelect={view.select}
          />
          <BodyReadout selected={selected} />
          {detailStatus}
        </>
      ) : (
        <p className="panel__empty">{empty}</p>
      )}
    </section>
  );
}
