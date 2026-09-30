import { universeTimeFromYears } from "@hyperion/protocol";
import { useId, useState } from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import { useUniverse } from "../../lib/universe";
import { type SummaryTarget, useSystemSummary } from "../system/useSystemSummary";
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
 * link; with nothing selected it is held back and says why, and with a free-floating brown dwarf or
 * planet selected, whose bodies plan 14 has not yet modelled, it says `BODIES NOT YET MODELLED`
 * (plan 13, P13.T8.d; ruling 134.5).
 *
 * Selecting a system asks the server for its stars at the chart's time (`system_summary`, plan 06,
 * P06.T36; a brown dwarf is a system of one, and a free-floating planet, which has none, asks
 * nothing), through the `SYSTEM` display's own hook, whose channel lets the latest selection win and
 * drops an answer to a superseded one. Where that request stands is said between the readout and
 * `OPEN SYSTEM`, outside the live region, with `RETRY` after a failure; an answer the client cannot
 * read says `SYSTEM DATA INVALID` there. The answer on show while the chart's time has moved on, or
 * after the newer request failed, is read as stale. The selection's extinction from the chart's
 * centre (plan 07, P07.T11.c) is asked for by the chart ({@link LocalChartState}), and where that
 * request stands is said after the summary's, named `EXTINCTION`, so that two waits are told apart.
 */
export function SystemsPanel({ chart, onOpenSystem }: SystemsPanelProps) {
  const titleId = useId();
  const reasonId = useId();
  const { open } = useUniverse();
  const { result, fault, stale } = chart;
  const selected = chart.selected;
  const bands = chart.bands;
  // A free-floating object has no system to open until plan 14 gives it bodies (P14.T27).
  const freeFloating = selected !== null && selected.kind !== "stellar";
  const target =
    open === null || result === null || selected === null || bands === null || freeFloating
      ? null
      : systemTargetFor(open.id, selected, result.timeYr, bands);
  const [generation, setGeneration] = useState(0);
  // A free-floating planet has no stars to summarise (plan 13, P13.T5.d); a brown dwarf is one.
  const summaryTarget: SummaryTarget | null =
    open === null || result === null || selected === null || selected.kind === "rogue_planet"
      ? null
      : { universe: open.id, system: selected.id, designation: selected.designation };
  const chartTime = universeTimeFromYears(result?.timeYr ?? chart.timeYr);
  const summary = useSystemSummary(summaryTarget, chartTime, generation);
  const stars = summary.shown?.kind === "ok" ? summary.shown.model : null;
  // An answer for a time the chart has left, or kept through a newer request that failed, is a
  // snapshot the chart no longer stands on.
  const starsStale =
    stars !== null &&
    (stars.time.seconds !== chartTime.seconds ||
      stars.time.nanos !== chartTime.nanos ||
      summary.state.kind === "rejected" ||
      summary.state.kind === "timed_out");
  const retry = (): void => {
    setGeneration((count) => count + 1);
  };
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
          systems={chart.systems}
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
        stars={stars}
        starsStale={starsStale}
        extinction={chart.extinction.reading}
      />
      {summary.shown?.kind === "fault" ? (
        <StatusLine
          text={`SYSTEM DATA INVALID: ${summary.shown.fault}`}
          standing="fault"
          action={{ label: "RETRY", onAction: retry }}
        />
      ) : (
        <RequestStatus state={summary.state} onRetry={retry} />
      )}
      {chart.extinction.state.kind === "ok" && chart.extinction.reading === null ? (
        // Answered, but for an ID the server resolves to no system: its rows stay the em dash.
        <StatusLine text="EXTINCTION: NO SUCH SYSTEM" standing="refused" />
      ) : (
        <RequestStatus
          state={chart.extinction.state}
          subject="EXTINCTION"
          onRetry={chart.retryExtinction}
        />
      )}
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
            {/*
             * A subject and a state (ruling 113.5): at 1280 x 720 it wraps whole onto its own line
             * under the button, through the row's flex-wrap (the orchestrator's ruling 134.5).
             */}
            {freeFloating ? "BODIES NOT YET MODELLED" : "NO SYSTEM SELECTED"}
          </span>
        ) : null}
      </div>
    </section>
  );
}
