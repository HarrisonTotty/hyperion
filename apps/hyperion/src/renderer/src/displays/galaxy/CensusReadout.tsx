import type { LayerCensus, LayerStatus } from "@hyperion/protocol";
import { useId, useRef, useState } from "react";

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { annunciation } from "../../components/RequestStatus";
import { SolarMassUnit } from "../../components/SolarMassUnit";
import { StaleMark } from "../../components/StaleMark";
import { formatListPosition, formatMassMsun, formatNumber, formatSci } from "../../lib/format";
import type { ChartResult, ChartSystem } from "../../lib/galaxy/model";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import type { RequestState } from "../../lib/useServerRequest";
import { windowRange } from "../../lib/windowRange";
import {
  censusHint,
  censusLine,
  formatBandMsun,
  inRangeCount,
  shownCountText,
  type StarFilter,
} from "./chartModel";
import { usePageTabFocus } from "./pageTabFocus";

/** What a layer's line in the census table says about the layer, in words. */
function statusWords(status: LayerStatus): string {
  let words: string;
  switch (status) {
    case "included":
      words = "INCLUDED";
      break;
    case "over_limit":
      words = "OVER LIMIT";
      break;
    case "below_mass_floor":
      words = "BELOW MIN MASS";
      break;
    case "over_cell_budget":
      words = "OVER CELL BUDGET";
      break;
  }
  return words;
}

/** Above this an expected count is written in E notation, so that it cannot outgrow its field. */
const EXPECTED_E_NOTATION_ABOVE = 10_000;

/** Height of a census row, in `rem`: fixed, so that the table's position can be read from it. */
const ROW_REM = 1.25;

/**
 * An expected count, which the request's limit does not bound: to one decimal below 10,000 and in E
 * notation from there, the guide's form for a value outside its field's ladder (design note D7).
 */
function expectedCount(expected: number): string {
  if (Math.abs(expected) < EXPECTED_E_NOTATION_ABOVE) {
    return formatNumber(expected, 1);
  }
  return formatSci(expected);
}

interface CensusTableProps {
  readonly layers: ReadonlyArray<LayerCensus>;
}

/** One row per mass layer: its band, what was expected in the sphere, and what came back. */
function CensusTable({ layers }: CensusTableProps) {
  return (
    <table className="census-table">
      <thead>
        <tr>
          <th scope="col">LAYER</th>
          <th scope="col">
            BAND <SolarMassUnit />
          </th>
          <th scope="col">EXP</th>
          <th scope="col">RET</th>
          <th scope="col">STATUS</th>
        </tr>
      </thead>
      <tbody>
        {layers.map((layer) => (
          <tr key={layer.layer}>
            <th scope="row">{layer.layer.toUpperCase()}</th>
            <td className="census-table__band">
              {formatBandMsun(layer.mass_min_msun)}-{formatBandMsun(layer.mass_max_msun)}
            </td>
            <td className="census-table__count">{expectedCount(layer.expected)}</td>
            <td className="census-table__count">{formatNumber(layer.returned, 0)}</td>
            <td className="census-table__status">{statusWords(layer.status)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

interface CensusByLayerProps {
  readonly layers: ReadonlyArray<LayerCensus>;
  readonly id: string;
  readonly hidden: boolean;
}

/**
 * The census layer by layer: the table in a box that scrolls where the page cannot hold it, with
 * the rows in view and their total read beneath it.
 *
 * @remarks
 * The guide allows a scrolling list only where it shows its position and total, and the chart's page
 * cannot hold the whole table at 1280 px. The column headers stay in view while it scrolls, so a row
 * is never read without them.
 */
function CensusByLayer({ layers, id, hidden }: CensusByLayerProps) {
  const { ref, metrics } = useScrollMetrics();
  const rowPx = ROW_REM * metrics.remPx;
  // The head row is sticky, so it stands in the scrolling box and takes a row of its height.
  const range = windowRange(
    metrics.scrollTopPx,
    rowPx,
    metrics.viewportPx - rowPx,
    layers.length,
    0,
  );

  return (
    <div className="census-readout__by-layer" id={id} hidden={hidden}>
      <div className="census-readout__table" ref={ref}>
        <CensusTable layers={layers} />
      </div>
      {range === null ? null : (
        <p className="list-position">
          {formatListPosition(range.firstVisible, range.lastVisible, layers.length)}
        </p>
      )}
    </div>
  );
}

interface CensusReadoutProps {
  /** The answer the chart draws, or `null` before the first one arrives. */
  readonly result: ChartResult | null;
  /** The systems of the answer that pass the `STARS` filter, which the counts are of. */
  readonly systems: ReadonlyArray<ChartSystem>;
  readonly starFilter: StarFilter;
  readonly state: RequestState<"systems_in_range">;
  /** The range the counts are taken against (plan 05, design note D9). */
  readonly driveRangeLy: number;
  /** Whether the answer is a snapshot the link no longer backs: then muted, with a trailing `S`. */
  readonly stale: boolean;
  /** Sends the query again after a failure; the readout moves the focus off `RETRY` first. */
  readonly onRetry: () => void;
}

/**
 * What a chart's range query returned: what it is complete above, how many systems it holds, and
 * the census layer by layer.
 *
 * @remarks
 * The census line is the brainstorm's rule that a result is a complete census or nothing, per
 * layer, so it stands beside every chart. Nothing fitting the census limit is an answer, not a
 * failure, and reads in `--status-caution` as a limit reached, with what the operator can do. A
 * layer the server left out adds a hint under it. The counts are of what was returned and of what
 * lies within the drive range; under a `STARS` filter other than `ALL` the first says what the
 * filter hides (`412 OF 1630 SHOWN: LIVING`), and both count the systems shown. The per-layer table, which is how placement is checked against the
 * fields by eye, folds: the chart's column cannot hold both it and the chart at 1280 px. While a
 * newer query is in flight its `PENDING` stands beside the census and the answer on show stays. An
 * answer the link no longer backs reads as stale: muted, with the guide's trailing `S`.
 *
 * The summary is a live region holding text only — the census line, the request's state in words
 * and the counts — read as a whole when it changes, so that a query given from the keyboard
 * announces that it started and then what came back. `RETRY` and `CENSUS BY LAYER` stand beside it,
 * outside it. The camera's readouts are deliberately silent; `CHART DATA INVALID`, the
 * selected-system readout and the cursor line announce from their own places, each being a region
 * in its own right. `RETRY` goes as it is pressed, since the query is then pending, so it hands the
 * focus to `CENSUS BY LAYER` beside it, or, before the first answer, when there is no toggle, to
 * the page's tab: never to the document's body (the orchestrator's ruling 18).
 */
export function CensusReadout({
  result,
  systems,
  starFilter,
  state,
  driveRangeLy,
  stale,
  onRetry,
}: CensusReadoutProps) {
  const tableId = useId();
  const stateId = useId();
  const [tableShown, setTableShown] = useState(false);
  const toggleRef = useRef<HTMLButtonElement>(null);
  const focusPageTab = usePageTabFocus();
  const line = result === null ? null : censusLine(result.census);
  const hint = result === null ? null : censusHint(result.layers);
  const shown = annunciation(state);

  return (
    <div className={stale ? "census-readout census-readout--stale" : "census-readout"}>
      <div className="census-readout__row">
        {/*
         * The announced region, so that an operator who presses `C` from the keyboard is told that
         * the query started and then what came back, which nothing else on the page says (the
         * orchestrator's ruling 11). Atomic, so a changed count is read with the line it belongs
         * to.
         *
         * Text only, and `div role="status"` rather than `output`, on two counts (rulings 13 and
         * 14). An `output`'s content model is phrasing content, and this region holds a `p` and a
         * `dl`, so an `output` here would be invalid HTML; the rules' "semantic element before
         * ARIA" asks for the native element where one fits, and an element that cannot legally
         * hold a `dl` does not fit. And the request's state is rendered from `annunciation` rather
         * than by nesting `RequestStatus`, whose `StatusLine` is a live region itself: a region
         * inside a region is read differently by every screen reader, and silencing the inner one
         * leaves a change confined to its text unspecified. `RETRY` and `CENSUS BY LAYER` are
         * siblings of the region, since a control appearing inside one is announced with it. The
         * per-layer table, the system list and the camera's own readouts stay out of it as well: a
         * spatial view announces nothing as it moves.
         */}
        <div
          className="census-readout__summary"
          // The `output` the rule asks for cannot hold this region's `dl`, as above.
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="status"
          aria-label="Census"
          aria-atomic="true"
        >
          {line === null ? null : (
            <p
              className={
                line.kind === "nothing_fits"
                  ? "census-readout__line census-readout__line--caution"
                  : "census-readout__line"
              }
            >
              {line.kind === "complete" ? (
                <>
                  {line.text} {formatMassMsun(line.aboveMsun)} <SolarMassUnit />
                </>
              ) : (
                line.text
              )}
            </p>
          )}
          {shown === null ? null : (
            <p
              id={stateId}
              className={
                shown.standing === "fault"
                  ? "census-readout__state census-readout__state--fault"
                  : "census-readout__state"
              }
            >
              {shown.text}
            </p>
          )}
          {result === null ? null : (
            <dl className="readout census-readout__counts">
              <dt>SYSTEMS</dt>
              <dd>{shownCountText(systems.length, result.systems.length, starFilter)}</dd>
              <dt>IN RANGE</dt>
              <dd>
                {formatNumber(inRangeCount(systems, driveRangeLy), 0)}
                {stale ? <StaleMark /> : null}
              </dd>
            </dl>
          )}
        </div>
        {shown === null || shown.standing === "waiting" ? null : (
          // Beside the region, not in it: what the operator can do about a failure is a control,
          // and a control that appears inside a region is announced with it. Described by the
          // words, so that it says which failure it retries wherever it is reached from.
          <button
            type="button"
            className="control"
            aria-describedby={stateId}
            onClick={() => {
              // The query goes pending and RETRY with it, so the focus goes to a control that
              // stays (ruling 18). The ruling's first choice, the map picture `C` was pressed on,
              // is never on show here: `C` shows this page, which hides the map's.
              if (toggleRef.current === null) {
                focusPageTab?.();
              } else {
                toggleRef.current.focus();
              }
              onRetry();
            }}
          >
            RETRY
          </button>
        )}
        {result === null ? null : (
          // Outside the region: the table it shows is not part of what the answer says.
          <button
            ref={toggleRef}
            type="button"
            className="control disclosure census-readout__toggle"
            aria-expanded={tableShown}
            aria-controls={tableId}
            onClick={() => {
              setTableShown((wasShown) => !wasShown);
            }}
          >
            <DisclosureGlyph expanded={tableShown} />
            CENSUS BY LAYER
          </button>
        )}
      </div>
      {hint === null ? null : <p className="census-readout__hint">{hint}</p>}
      {result === null ? null : (
        <CensusByLayer layers={result.layers} id={tableId} hidden={!tableShown} />
      )}
    </div>
  );
}
