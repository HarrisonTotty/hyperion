import { useId } from "react";

import { StaleMark } from "../../components/StaleMark";
import { SystemList } from "./SystemList";
import { SystemReadout } from "./SystemReadout";
import type { LocalChartState } from "./useLocalChart";

interface SystemsPanelProps {
  readonly chart: LocalChartState;
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
 */
export function SystemsPanel({ chart }: SystemsPanelProps) {
  const titleId = useId();
  const { result, fault, stale } = chart;
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
    </section>
  );
}
