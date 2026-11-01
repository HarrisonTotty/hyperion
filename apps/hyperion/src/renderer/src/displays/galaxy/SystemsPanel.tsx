import {
  type ContactDto,
  type FeatureKindCensus,
  type RequestKind,
  type UniverseIdHex,
  universeTimeFromYears,
} from "@hyperion/protocol";
import { type ReactNode, useId, useState } from "react";

import { AlertList, type AlertSort } from "../../components/AlertList";
import { RequestStatus } from "../../components/RequestStatus";
import { StatusLine } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";
import { featureKindAbbreviation, notYetModelledWords } from "../../lib/galaxy/featureWords";
import { NOW_MODE, sameQueryMode } from "../../lib/galaxy/model";
import { linkDownReason } from "../../lib/serverLink";
import type { AlertsView } from "../../lib/useAlerts";
import { useUniverse } from "../../lib/universe";
import type { RequestState } from "../../lib/useServerRequest";
import { type SummaryTarget, useSystemSummary } from "../system/useSystemSummary";
import { useStreamReadout } from "./useStreamReadout";
import { type SystemTarget, systemTargetFor } from "../system/systemTarget";
import { selectionMarkId } from "./chartModel";
import { FeatureList } from "./FeatureList";
import { FeatureReadout } from "./FeatureReadout";
import { SystemList } from "./SystemList";
import { SystemReadout } from "./SystemReadout";
import { useBlackHoleState } from "./useBlackHoleState";
import { useFeatureDetail } from "./useFeatureDetail";
import type { LocalChartState } from "./useLocalChart";

/**
 * The extinction's status for a selection the server resolves to no system: the request's subject
 * and a refusal with its reason, as `RequestStatus` writes one (decision-gg-port-d0 §2.1).
 */
const NO_SUCH_SYSTEM = "EXTINCTION REJECTED: no such system";

/**
 * Which of the panel's lists it shows: the chart's two (plan 09, P09.T41.c), and the station's
 * alert list (plan 12, P12.T12).
 */
type ChartList = "systems" | "features" | "alerts";

/** Each list's choice, in the order offered. */
const LISTS: ReadonlyArray<{ readonly list: ChartList; readonly label: string }> = [
  { list: "systems", label: "SYSTEMS" },
  { list: "features", label: "FEATURES" },
  { list: "alerts", label: "ALERTS" },
];

/** Why the alert list is empty with no observer: the detections are seen from it (P12.T12). */
export const NO_OBSERVER = "NO OBSERVER: choose OBSERVED on the LOCAL CHART";

/**
 * What the features' census says beyond the list (plan 09, P09.T41.c), or `null` with nothing to
 * say: that none is in range, the kinds left out by the server's cell budget, as the systems'
 * census says of a layer, and those it does not generate, plan 10's streams and dwarf cores, in
 * the data state for what this generator version does not compute, composed as the guide composes
 * it, so that neither reads as none.
 */
export function featureCensusWords(
  census: ReadonlyArray<FeatureKindCensus>,
  returned: number,
): string | null {
  const of = (status: FeatureKindCensus["status"]): string[] =>
    census
      .filter((line) => line.status === status)
      .map((line) => featureKindAbbreviation(line.kind));
  const overBudget = of("over_cell_budget");
  const notGenerated = of("not_generated");
  const parts = [
    returned === 0 && overBudget.length === 0 ? "NO FEATURES IN RANGE" : null,
    overBudget.length === 0
      ? null
      : `LARGE VOLUME: ${overBudget.join(", ")} over the server cell budget; reduce radius`,
    notGenerated.length === 0 ? null : notYetModelledWords(notGenerated),
  ].filter((part) => part !== null);
  return parts.length === 0 ? null : parts.join(" · ");
}

/** Whether a request ended in a failure, which leaves an answer kept from before a snapshot. */
function failedState(state: RequestState<RequestKind>): boolean {
  return state.kind === "rejected" || state.kind === "timed_out";
}

/** The alert list's active row, and the universe whose contact it is. */
interface ActiveAlert {
  readonly universe: UniverseIdHex | null;
  readonly contact: number;
}

interface SystemsPanelProps {
  readonly chart: LocalChartState;
  /** The sensor detections seen from the chart's observer, the alert list's (plan 12, P12.T12). */
  readonly alerts: AlertsView;
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
 * chart says where the fault is read. An answer the link no longer backs is muted, and its values
 * carry the guide's trailing `S` themselves, so the panel's title takes none, as `LOCAL CHART`'s
 * and `HR DIAGRAM`'s take none: each row of the list trails one `S` at its end and is named stale,
 * and the readout marks each value it reads from the answer, the summary's with them (plan 05,
 * P05.T14).
 *
 * `OPEN SYSTEM` follows the readout, outside its live region, since a control inside one would be
 * announced with every selection and a live region holds text only (the orchestrator's rulings 13
 * and 14). It opens the `SYSTEM` display on the selected system at the time of the chart on show
 * (plan 14, P14.T41.a). A display control, it only changes what the console shows, so it needs no
 * link; with nothing selected it is held back and says why (ruling 134.5). It opens a free-floating
 * brown dwarf or planet too, whose bodies plan 14 models from generator version 26 (P14.T27).
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
 *
 * The chart's features are its second list (plan 09, P09.T41.c), shown in the systems' place, one
 * list at a time, so that the column keeps its rows: `SYSTEMS` and `FEATURES` beside the panel's
 * title choose between them, one stop in the tab order, the arrows moving from one to the other,
 * and a selection made on the chart shows the list it is in. The features' list
 * ({@link FeatureList}) has the systems' keyboard selection, and under it what the features'
 * census leaves out; until an answer for the chart on show arrives it says `FEATURES PENDING`. The
 * selection is a system or a feature, and a feature's readout ({@link FeatureReadout}) stands in
 * the system's: its structure is asked for at the chart's time (`feature_detail`), and for the
 * galactic centre its black hole (`black_hole_state`), each request's state said after it, its
 * subject `STRUCTURE` or `BLACK HOLE` before it. A feature is no system to open, so `OPEN SYSTEM`
 * says `NO SYSTEM SELECTED` meanwhile.
 *
 * The third list is the station's alert list (plan 12, P12.T12; {@link AlertList}): the sensor
 * detections seen from the chart's observer, in the column beside the chart that marks them, as
 * the guide's "every station has an alert list" allows, with the census of each class under it.
 * Where the detections stand is said above it, `NO OBSERVER` until the chart has one. Making a
 * resolved alert's row active selects its host, which the readout then reads and the chart
 * brackets, and the list stays on show; an unresolved one, whose host is not known, selects
 * nothing. A selection made on the chart, as elsewhere, shows the list it is in.
 */
export function SystemsPanel({ chart, alerts, onOpenSystem }: SystemsPanelProps) {
  const titleId = useId();
  const reasonId = useId();
  const { open } = useUniverse();
  const { result, fault, stale } = chart;
  const selected = chart.selected;
  const selectedFeature = chart.selectedFeature;
  const [listChoice, setListChoice] = useState<ChartList>("systems");
  const [alertSort, setAlertSort] = useState<AlertSort>("priority");
  // A contact's number is its universe's alone, so the active alert lasts until another opens.
  const [activeChoice, setActiveChoice] = useState<ActiveAlert | null>(null);
  const activeContact =
    activeChoice !== null && activeChoice.universe === (open?.id ?? null)
      ? activeChoice.contact
      : null;
  const activeAlert = alerts.contacts.find((contact) => contact.contact === activeContact) ?? null;
  const activeHost = activeAlert?.level === "resolved" ? activeAlert.host : null;
  // A selection made on the chart shows the list it is in, so that it is shown in both places: the
  // operator's selection, which stays while the chart's two answers arrive apart.
  const selectionKey = chart.selection === null ? null : selectionMarkId(chart.selection);
  const [seenSelection, setSeenSelection] = useState<string | null>(null);
  let list = listChoice;
  if (selectionKey !== seenSelection) {
    setSeenSelection(selectionKey);
    const wanted: ChartList | null =
      chart.selection === null ? null : chart.selection.kind === "feature" ? "features" : "systems";
    // The host an active alert selected is shown where it was chosen, in the alert list.
    const fromAlerts =
      listChoice === "alerts" &&
      chart.selection?.kind === "system" &&
      chart.selection.id === activeHost;
    if (wanted !== null && wanted !== listChoice && !fromAlerts) {
      list = wanted;
      setListChoice(wanted);
    }
  }
  const chartTimeYr = result?.timeYr ?? chart.timeYr;
  const [featureGeneration, setFeatureGeneration] = useState(0);
  const detail = useFeatureDetail({
    universe: open?.id ?? null,
    feature: chart.selection?.kind === "feature" ? chart.selection.id : null,
    timeYr: chartTimeYr,
    generation: featureGeneration,
  });
  const centreSelected = selectedFeature?.kind === "galactic_centre";
  const blackHole = useBlackHoleState(
    centreSelected ? (open?.id ?? null) : null,
    chartTimeYr,
    featureGeneration,
  );
  const retryFeature = (): void => {
    setFeatureGeneration((count) => count + 1);
  };
  // An answer for a time the chart has left, or kept through a newer request that failed.
  const detailStale =
    detail.shown !== null && (detail.shown.timeYr !== chartTimeYr || failedState(detail.state));
  const blackHoleStale =
    blackHole.shown !== null &&
    (blackHole.shown.timeYr !== chartTimeYr || failedState(blackHole.state));
  const bands = chart.bands;
  // An object of a layer this build does not know is offered, and the server answers for what it
  // has.
  const target =
    open === null || result === null || selected === null || bands === null
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
  // A stream member's stream, for its rows under the member's ORIGIN (plan 10, P10.T11).
  const memberOf = chart.selected?.origin.kind === "stream" ? chart.selected.origin.stream : null;
  const stream = useStreamReadout(open?.id ?? null, memberOf, generation);
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
      <div className="systems-panel__head">
        <h2 className="panel__title" id={titleId}>
          Systems
        </h2>
        {/*
         * The chart's two lists, one shown at a time, so that the column at 1280 x 720 keeps the
         * list's four rows and the readout (plan 09, P09.T41.c). A display control, never held
         * back, its choice's button pressed and ruled under, as the mode's is.
         */}
        <fieldset className="form-choice systems-panel__lists">
          <legend className="visually-hidden">LIST</legend>
          {LISTS.map(({ list: each, label }) => (
            <button
              key={each}
              type="button"
              className="control mode-control__button"
              aria-pressed={each === list}
              // One stop in the tab order, the list shown's, the arrows moving between the two, so
              // that the list is still one `Tab` from the `CURSOR` fields beyond it.
              tabIndex={each === list ? 0 : -1}
              onClick={() => {
                setListChoice(each);
              }}
              onKeyDown={(event) => {
                if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
                  return;
                }
                event.preventDefault();
                const index = LISTS.findIndex((option) => option.list === each);
                const step = event.key === "ArrowRight" ? 1 : LISTS.length - 1;
                const other = LISTS[(index + step) % LISTS.length]?.list ?? each;
                setListChoice(other);
                const group = event.currentTarget.parentElement;
                group?.querySelector<HTMLButtonElement>(`[data-list="${other}"]`)?.focus();
              }}
              data-list={each}
            >
              {label}
            </button>
          ))}
        </fieldset>
      </div>
      {list === "alerts" ? (
        <AlertListPart
          alerts={alerts}
          heldBack={chart.heldBack}
          sort={alertSort}
          onSort={setAlertSort}
          activeContact={activeContact}
          onActivate={(contact) => {
            setActiveChoice({ universe: open?.id ?? null, contact: contact.contact });
            if (contact.level === "resolved") {
              chart.select(contact.host);
            }
          }}
        />
      ) : result === null ? (
        <p className="panel__empty">
          <StatusText
            text={
              fault === null
                ? `NO CHART: centre one to list its ${list}`
                : "CHART DATA INVALID: retry on the LOCAL CHART page"
            }
          />
        </p>
      ) : list === "systems" ? (
        <SystemList
          systems={chart.systems}
          selectedId={chart.selectedId}
          onSelect={chart.select}
          driveRangeLy={chart.driveRangeLy}
          distanceUnit={chart.distanceUnit}
          stale={stale}
        />
      ) : (
        <FeatureListPart chart={chart} />
      )}
      {selectedFeature === null ? (
        <SystemReadout
          system={chart.selected}
          frame={chart.frame}
          driveRangeLy={chart.driveRangeLy}
          timeYr={chartTimeYr}
          distanceUnit={chart.distanceUnit}
          stars={stars}
          stale={stale}
          starsStale={starsStale}
          starsAt={summary.shownEmitted}
          extinction={answer.kind === "line" ? answer.line : null}
          features={chart.features?.features}
          stream={memberOf === null ? null : stream}
        />
      ) : (
        <FeatureReadout
          feature={selectedFeature}
          timeYr={chartTimeYr}
          distanceUnit={chart.distanceUnit}
          stale={stale || chart.selectedFeatureStale}
          detail={detail.shown}
          detailStale={detailStale}
          blackHole={centreSelected ? blackHole.shown : null}
          blackHoleStale={blackHoleStale}
        />
      )}
      {selectedFeature === null ? (
        <>
          {summary.shown?.kind === "fault" ? (
            <StatusLine
              text={`SYSTEM DATA INVALID: ${summary.shown.fault}`}
              standing="fault"
              action={{ label: "RETRY", onAction: retry }}
            />
          ) : (
            <RequestStatus state={summary.state} onRetry={retry} />
          )}
          {/* The link's state is said once, by the summary's status. */}
          {stream.state === null || stream.state.kind === "link_down" ? null : (
            <RequestStatus state={stream.state} subject="STREAM" onRetry={retry} />
          )}
          {extinctionStatus}
        </>
      ) : (
        <>
          <RequestStatus state={detail.state} subject="STRUCTURE" onRetry={retryFeature} />
          {/* The link's state is said once, by the structure's status. */}
          {centreSelected && blackHole.state.kind !== "link_down" ? (
            <RequestStatus state={blackHole.state} subject="BLACK HOLE" onRetry={retryFeature} />
          ) : null}
        </>
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
            NO SYSTEM SELECTED
          </span>
        ) : null}
      </div>
    </section>
  );
}

interface FeatureListPartProps {
  readonly chart: LocalChartState;
}

/**
 * The chart's features, as the panel lists them (plan 09, P09.T41.c): the list and what the census
 * says beyond it, or where the features' request stands, `FEATURES PENDING` until an answer for the
 * chart on show arrives, so that unfetched features never look absent, a failure with `RETRY`, and
 * an answer the client cannot use as `FEATURE DATA INVALID` with `RETRY`.
 */
function FeatureListPart({ chart }: FeatureListPartProps) {
  const { features, featureFault, featureState, stale } = chart;
  if (featureFault !== null) {
    return (
      <StatusLine
        text={`FEATURE DATA INVALID: ${featureFault}`}
        standing="fault"
        action={{ label: "RETRY", onAction: chart.retryFeatures }}
      />
    );
  }
  if (features === null) {
    // An answer for another chart than the one on show is one still to come.
    const state = featureState.kind === "ok" ? { kind: "pending" as const } : featureState;
    return <RequestStatus state={state} subject="FEATURES" onRetry={chart.retryFeatures} />;
  }
  const censusWords = featureCensusWords(features.census, features.features.length);
  return (
    <>
      <FeatureList
        features={features.features}
        selectedId={chart.selectedFeature?.id ?? null}
        onSelect={chart.selectFeature}
        distanceUnit={chart.distanceUnit}
        stale={stale}
      />
      {censusWords === null ? null : (
        <p className="feature-list__census">
          <StatusText text={censusWords} />
        </p>
      )}
    </>
  );
}

interface AlertListPartProps {
  readonly alerts: AlertsView;
  /** Why the commands that ask the server are held back, such as `NO CARRIER`; `null` when they act. */
  readonly heldBack: string | null;
  readonly sort: AlertSort;
  readonly onSort: (sort: AlertSort) => void;
  readonly activeContact: number | null;
  readonly onActivate: (contact: ContactDto) => void;
}

/**
 * The station's alert list, as the panel shows it (plan 12, P12.T12): where its detections stand,
 * `NO OBSERVER` until the chart has one, `ALERTS PENDING` while the subscription opens, a refusal or
 * a timeout with `RETRY`, a move of the observer the server did not accept, and the list itself,
 * stale with the link, as soon as it has opened.
 */
function AlertListPart({
  alerts,
  heldBack,
  sort,
  onSort,
  activeContact,
  onActivate,
}: AlertListPartProps) {
  const { status } = alerts;
  let state: RequestState<RequestKind> | null;
  switch (status.kind) {
    case "idle":
    case "live":
      state = null;
      break;
    case "no_observer":
      return (
        <p className="panel__empty">
          <StatusText text={NO_OBSERVER} />
        </p>
      );
    case "pending":
      state = { kind: "pending" };
      break;
    case "stale":
      state = status.reason === "resubscribing" ? { kind: "pending" } : null;
      break;
    case "link_down":
      state = { kind: "link_down", reason: heldBack ?? linkDownReason("disconnected") ?? "" };
      break;
    case "rejected":
      state = { kind: "rejected", code: status.code, reason: status.reason };
      break;
    case "timed_out":
      state = { kind: "timed_out" };
      break;
  }
  const opened = status.kind === "live" || status.kind === "stale";
  const { observerFault } = alerts;
  return (
    <>
      {state === null ? null : (
        <RequestStatus state={state} subject="ALERTS" onRetry={alerts.retry} />
      )}
      {observerFault === null ? null : (
        <StatusLine
          text={
            observerFault.kind === "refused"
              ? `OBSERVER REJECTED: ${observerFault.reason}`
              : "OBSERVER TIMED OUT"
          }
          standing={observerFault.kind === "refused" ? "refused" : "fault"}
        />
      )}
      {opened ? (
        <AlertList
          contacts={alerts.contacts}
          census={alerts.census}
          acknowledgements={alerts.acknowledgements}
          sort={sort}
          onSort={onSort}
          activeContact={activeContact}
          onActivate={onActivate}
          onAcknowledge={alerts.acknowledge}
          heldBack={heldBack}
          stale={status.kind === "stale"}
        />
      ) : null}
    </>
  );
}
