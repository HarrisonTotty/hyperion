import type { LayerCensus, LayerStatus } from "@hyperion/protocol";
import { useId, useState } from "react";

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { RequestStatus } from "../../components/RequestStatus";
import { SolarMassUnit } from "../../components/SolarMassUnit";
import { StaleMark } from "../../components/StaleMark";
import { formatListPosition, formatMassMsun, formatNumber, formatSci } from "../../lib/format";
import type { ChartResult } from "../../lib/galaxy/model";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import type { RequestState } from "../../lib/useServerRequest";
import { windowRange } from "../../lib/windowRange";
import { censusHint, censusLine, formatBandMsun, inRangeCount } from "./chartModel";

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
  readonly state: RequestState<"systems_in_range">;
  /** The range the counts are taken against (plan 05, design note D9). */
  readonly driveRangeLy: number;
  /** Whether the answer is a snapshot the link no longer backs: then muted, with a trailing `S`. */
  readonly stale: boolean;
  /** Sends the query again after a failure. */
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
 * lies within the set range. The per-layer table, which is how placement is checked against the
 * fields by eye, folds: the chart's column cannot hold both it and the chart at 1280 px. While a
 * newer query is in flight its `PENDING` stands beside the census and the answer on show stays. An
 * answer the link no longer backs reads as stale: muted, with the guide's trailing `S`.
 *
 * The summary line is an `output`, read as a whole when it changes, so that a query given from the
 * keyboard announces that it started and then what came back; it is the one thing on the chart that
 * is announced, the camera's own readouts being deliberately silent.
 */
export function CensusReadout({ result, state, driveRangeLy, stale, onRetry }: CensusReadoutProps) {
  const tableId = useId();
  const [tableShown, setTableShown] = useState(false);
  const line = result === null ? null : censusLine(result.census);
  const hint = result === null ? null : censusHint(result.layers);

  return (
    <div className={stale ? "census-readout census-readout--stale" : "census-readout"}>
      <div className="census-readout__row">
        {/*
         * An `output`, so that a query's answer is announced once, as a whole: an operator who
         * presses `C` from the keyboard is told that the query started and then what came back,
         * which nothing else on the page says (the orchestrator's ruling 11). Atomic, so a changed
         * count is read with the line it belongs to. The per-layer table, the system list and the
         * camera's own readouts stay out of it: a spatial view announces nothing as it moves.
         */}
        <output className="census-readout__summary" aria-label="Census" aria-atomic="true">
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
          <RequestStatus state={state} onRetry={onRetry} />
          {result === null ? null : (
            <dl className="readout census-readout__counts">
              <dt>SYSTEMS</dt>
              <dd>{formatNumber(result.systems.length, 0)}</dd>
              <dt>IN RANGE</dt>
              <dd>
                {formatNumber(inRangeCount(result, driveRangeLy), 0)}
                {stale ? <StaleMark /> : null}
              </dd>
            </dl>
          )}
        </output>
        {result === null ? null : (
          // Outside the region: the table it shows is not part of what the answer says.
          <button
            type="button"
            className="control disclosure census-readout__toggle"
            aria-expanded={tableShown}
            aria-controls={tableId}
            onClick={() => {
              setTableShown((shown) => !shown);
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
