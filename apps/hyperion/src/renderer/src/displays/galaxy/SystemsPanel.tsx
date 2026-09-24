import { useId } from "react";

import { StaleMark } from "../../components/StaleMark";
import { useUniverse } from "../../lib/universe";
import { type SystemTarget, systemTargetFor } from "../system/systemTarget";
import { SystemList } from "./SystemList";
import { SystemReadout } from "./SystemReadout";
import type { LocalChartState } from "./useLocalChart";

interface SystemsPanelProps {
  readonly chart: LocalChartState;
  /** Opens the `SYSTEM` display on the selected system, at the chart's time. */
  readonly onOpenSystem: (target: SystemTarget) => void;
}

/**
 * The `SYSTEMS` panel: the chart's systems by distance, and everything known about the one
 * selected.
 *
 * @remarks
 * The guide's 3D conventions pair a canvas with a DOM list of its marks, so this panel stands beside
 * the chart, in the column under `CURSOR`, and a selection made in either place is shown in both.
 * One panel holds both the list and the readout, since the column's height at 1280 px does not run
 * to two sets of panel chrome. Before the first answer it says so, and an answer the client cannot
 * chart says where the fault is read. An answer the link no longer backs is muted and named stale,
 * with the guide's trailing `S`, as the chart and a kept map picture are.
 *
 * `OPEN SYSTEM` follows the readout, outside its live region, since a control inside one would be
 * announced with every selection and a live region holds text only (the orchestrator's rulings 13
 * and 14). It opens the `SYSTEM` display on the selected system at the time of the chart on show
 * (plan 14, P14.T41.a). A display control, it only changes what the console shows, so it needs no
 * link; with nothing selected it is held back and says why.
 */
export function SystemsPanel({ chart, onOpenSystem }: SystemsPanelProps) {
  const titleId = useId();
  const reasonId = useId();
  const { open } = useUniverse();
  const { result, fault, stale } = chart;
  const selected = chart.selected;
  const bands = chart.bands;
  const target =
    open === null || result === null || selected === null || bands === null
      ? null
      : systemTargetFor(open.id, selected, result.timeYr, bands);
  return (
    <section
      className={
        stale
          ? "panel galaxy__systems systems-panel systems-panel--stale"
          : "panel galaxy__systems systems-panel"
      }
      aria-labelledby={titleId}
    >
      <h2 className="panel__title" id={titleId}>
        Systems{stale ? <StaleMark /> : null}
      </h2>
      {result === null ? (
        <p className="panel__empty">
          {fault === null
            ? "NO CHART: centre one to list its systems"
            : "CHART DATA INVALID: retry on the LOCAL CHART page"}
        </p>
      ) : (
        <SystemList
          systems={result.systems}
          selectedId={chart.selectedId}
          onSelect={chart.select}
          driveRangeLy={chart.driveRangeLy}
          distanceDecimals={chart.distanceDecimals}
        />
      )}
      <SystemReadout
        system={chart.selected}
        frame={chart.frame}
        driveRangeLy={chart.driveRangeLy}
        timeYr={result?.timeYr ?? chart.timeYr}
        distanceDecimals={chart.distanceDecimals}
      />
      <div className="systems-panel__actions">
        <button
          type="button"
          className="control"
          // Held back rather than disabled, so that it keeps its focus and can say why.
          aria-disabled={target === null ? "true" : undefined}
          aria-describedby={target === null ? reasonId : undefined}
          onClick={() => {
            if (target !== null) {
              onOpenSystem(target);
            }
          }}
        >
          OPEN SYSTEM
        </button>
        {target === null ? (
          <span className="systems-panel__reason" id={reasonId}>
            NO SYSTEM SELECTED
          </span>
        ) : null}
      </div>
    </section>
  );
}
