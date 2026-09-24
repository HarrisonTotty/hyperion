import { useId } from "react";

import { annunciation } from "../../components/RequestStatus";
import { StaleMark } from "../../components/StaleMark";
import { formatAge, formatSigned } from "../../lib/format";
import { formatHex64 } from "../../lib/seed";
import { BodyList } from "./BodyList";
import { BodyReadout } from "./BodyReadout";
import { hostRows } from "./bodyRows";
import type { SystemViewState } from "./useSystemView";

/** Props of {@link BodiesPanel}. */
export interface BodiesPanelProps {
  readonly view: SystemViewState;
}

/**
 * The `BODIES` panel: the system named, its age and metallicity, its bodies as a tree, and
 * everything known about the one selected (plan 14, P14.T43.a–b).
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
  const { target, model, fault, stale, layout, summary, selected } = view;
  const formed = model?.formed === true ? model : null;
  const rows = formed === null || layout === null ? [] : hostRows(formed.hosts, layout);
  const age = model === null ? null : formatAge(model.ageMyr);

  let empty: string | null = null;
  if (fault !== null) {
    empty = "SYSTEM DATA INVALID";
  } else if (model === null) {
    empty = annunciation(summary)?.text ?? null;
  } else if (formed === null) {
    empty = "NOT YET FORMED";
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
      </dl>
      {empty === null ? (
        <>
          <BodyList rows={rows} selectedId={selected?.id ?? null} onSelect={view.select} />
          <BodyReadout host={selected} />
        </>
      ) : (
        <p className="panel__empty">{empty}</p>
      )}
    </section>
  );
}
