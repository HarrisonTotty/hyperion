import { universeTimeFromYears } from "@hyperion/protocol";
import { type ReactNode, useId, useState } from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";
import { NOW_MODE, sameQueryMode } from "../../lib/galaxy/model";
import { useUniverse } from "../../lib/universe";
import { type SummaryTarget, useSystemSummary } from "../system/useSystemSummary";
import { type SystemTarget, systemTargetFor } from "../system/systemTarget";
import { SystemList } from "./SystemList";
import { SystemReadout } from "./SystemReadout";
import type { LocalChartState } from "./useLocalChart";

/**
 * The extinction's status for a selection the server resolves to no system: the request's subject
 * and a refusal with its reason, as `RequestStatus` writes one (decision-gg-port-d0 §2.1).
 */
const NO_SUCH_SYSTEM = "EXTINCTION REJECTED: no such system";

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
 * after the newer request failed, is read as stale. The summary is asked in the mode of the chart
 * on show (plan 12, P12.T11), so that a system read `AS OBSERVED` shows its stars as observed, as
 * they come; one answered in another mode or from another observer than the selection is read in
 * is stale too, among them one answered without its observation, by a server that ignores the
 * mode, for a selection whose row carries one. The selection's extinction from the chart's
 * centre (plan 07, P07.T11.c) is asked for by the chart ({@link LocalChartState}), and where that
 * request stands is said after the summary's, its subject `EXTINCTION` before its state
 * (`EXTINCTION PENDING`), so that two waits are told apart; a selection the server resolves to no
 * system reads `EXTINCTION REJECTED: no such system`, a refusal with no `RETRY`, and an answer the
 * client cannot read `EXTINCTION DATA INVALID` with `RETRY`. The link's state is shown once, by the
 * summary's status where that shows it (decision-gg-port-d0 §2.1).
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
  // Asked in the mode of the chart on show, so that its stars are as the chart's observer sees them
  // (plan 12, P12.T11).
  const chartMode = result?.mode ?? NOW_MODE;
  const summary = useSystemSummary(summaryTarget, chartTime, generation, chartMode);
  const stars = summary.shown?.kind === "ok" ? summary.shown.model : null;
  // The selection is read as observed only where its row carries an observation, so its stars
  // must be answered in that mode, and in `now` for a row a server answered so.
  const readoutMode = selected?.observed === null ? NOW_MODE : chartMode;
  // An answer for a time or a mode the chart has left, or kept through a newer request that failed,
  // is a snapshot the chart no longer stands on.
  const starsStale =
    stars !== null &&
    (stars.time.seconds !== chartTime.seconds ||
      stars.time.nanos !== chartTime.nanos ||
      (summary.shownMode !== null && !sameQueryMode(summary.shownMode, readoutMode)) ||
      summary.state.kind === "rejected" ||
      summary.state.kind === "timed_out");
  const retry = (): void => {
    setGeneration((count) => count + 1);
  };
  const { state: extinctionState, answer } = chart.extinction;
  let extinctionStatus: ReactNode;
  switch (answer.kind) {
    case "no_such_system":
      // Answered, but for an ID the server resolves to no system: a refusal, which asking again
      // would only repeat, so it offers no RETRY; its rows stay the em dash.
      extinctionStatus = <StatusLine text={NO_SUCH_SYSTEM} standing="refused" />;
      break;
    case "invalid":
      // The server's fault, reported as every answer that fails a display's checks is.
      extinctionStatus = (
        <StatusLine
          text={`EXTINCTION DATA INVALID: ${answer.cause}`}
          standing="fault"
          action={{ label: "RETRY", onAction: chart.retryExtinction }}
        />
      );
      break;
    case "none":
    case "line":
      // The link's state is the link's, and the summary's status says it where it shows it.
      extinctionStatus =
        extinctionState.kind === "link_down" && summary.state.kind === "link_down" ? null : (
          <RequestStatus
            state={extinctionState}
            subject="EXTINCTION"
            onRetry={chart.retryExtinction}
          />
        );
      break;
  }
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
          <StatusText
            text={
              fault === null
                ? "NO CHART: centre one to list its systems"
                : "CHART DATA INVALID: retry on the LOCAL CHART page"
            }
          />
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
        starsAt={summary.shownEmitted}
        extinction={answer.kind === "line" ? answer.line : null}
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
      {extinctionStatus}
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
