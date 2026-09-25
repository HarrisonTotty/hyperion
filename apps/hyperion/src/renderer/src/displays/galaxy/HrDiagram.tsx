import type { SystemIdHex } from "@hyperion/protocol";
import { type MouseEvent, useLayoutEffect, useMemo, useRef, useState } from "react";

import { SolarUnit } from "../../components/SolarUnit";
import { StaleMark } from "../../components/StaleMark";
import { formatNumber, formatSci } from "../../lib/format";
import {
  HR_LOG_L_TICKS,
  HR_TEFF_TICKS_K,
  hrDrawList,
  hrSpectralLetters,
  hrXPx,
  hrYPx,
  pickHr,
  type PlotAreaPx,
  projectHr,
} from "../../lib/galaxy/hrProjection";
import type { ChartSystem } from "../../lib/galaxy/model";
import { useElementSize } from "../../lib/useElementSize";
import { LegendReticle } from "../../spatial/LegendReticle";
import { LegendSymbol } from "../../spatial/LegendSymbol";
import { type ColourTokens, paint, readTokens, sameTokens, staleTokens } from "../../spatial/paint";
import { SIZE_CLASS_REM } from "../../spatial/symbols";
import { shownCountText, type StarFilter } from "./chartModel";
import { StarShapeLegend } from "./StarShapeLegend";

/**
 * Room kept round the plot inside the canvas, in `rem`: on the left for the luminosity values, on
 * the right for half the last temperature value, above for the class letters, below for the
 * temperature values.
 */
const MARGIN_LEFT_REM = 3.5;
const MARGIN_RIGHT_REM = 1.5;
const MARGIN_TOP_REM = 1.5;
const MARGIN_BOTTOM_REM = 1.5;

/**
 * A tick's value as its label writes it: an exact power of ten in E notation, as the guide writes
 * a legend tick (`1E-4`, `1E5`), and any other value as a plain number (`30,000`, `3000`), so that
 * both logarithmic axes read alike (the owner's draft, r9 D2.4).
 */
function tickText(value: number): string {
  const decade = Math.log10(value);
  return Number.isInteger(decade) ? formatSci(value, "tick") : formatNumber(value, 0);
}

/** The rectangle the axes enclose in a canvas of this size, or `null` when there is no room. */
function plotArea(widthPx: number, heightPx: number, remPx: number): PlotAreaPx | null {
  const area = {
    leftPx: MARGIN_LEFT_REM * remPx,
    topPx: MARGIN_TOP_REM * remPx,
    widthPx: widthPx - (MARGIN_LEFT_REM + MARGIN_RIGHT_REM) * remPx,
    heightPx: heightPx - (MARGIN_TOP_REM + MARGIN_BOTTOM_REM) * remPx,
  };
  return area.widthPx > 0 && area.heightPx > 0 ? area : null;
}

/** A `transform` that moves a label from the stage's top left to a point given in pixels. */
function at(xPx: number, yPx: number, remPx: number): string {
  return `translate(${xPx / remPx}rem, ${yPx / remPx}rem)`;
}

/** Props of {@link HrDiagram}. */
export interface HrDiagramProps {
  /** The chart's systems that pass its `STARS` filter, which the diagram plots. */
  readonly systems: ReadonlyArray<ChartSystem>;
  /** How many systems the answer on show holds before the filter. */
  readonly total: number;
  readonly starFilter: StarFilter;
  /** The chart's selection, which the diagram shares. */
  readonly selectedId: SystemIdHex | null;
  readonly onSelect: (id: SystemIdHex) => void;
  /** The range a system counts as available within, drawn in `--accent` (plan 05, design note D9). */
  readonly driveRangeLy: number;
  /** Whether the answer is a snapshot the link no longer backs: drawn muted, with a trailing `S`. */
  readonly stale: boolean;
}

/**
 * The Hertzsprung-Russell diagram of the chart's systems: each primary's effective temperature
 * against its luminosity (plan 06, P06.T37 and design note 18).
 *
 * @remarks
 * A panel of the `GALAXY` display rather than a tool, plotting the chart's current answer and so
 * costing no request. A Canvas 2D scatter, painted by the spatial views' painter from
 * `hrDrawList`, with log T_eff on a reversed x axis from 200,000 to 1,000 K and log L ÷ L☉ on the y
 * axis from −6 to 6.5, both on log scales, which each axis's label says (`LOG SCALE`). As the guide requires of a
 * graph, the title stands above it, each axis has its label and unit, and the major ticks their
 * values; the spectral letters run along the top axis. All of that is DOM text, since the canvas
 * carries none (plan 05, D15). The grid is thin `--line`, the symbols the chart's, at the chart's
 * size classes (`SYMBOLS NOT TO SCALE`), in `--text`, and in `--accent` within the drive range, as
 * on the chart; the selection carries the chart's bracket reticle. A point beyond an axis is pegged
 * at its edge with the off-scale mark, and counted; neutron stars and black holes, which have no
 * photosphere to plot, are counted and not plotted, as are stars that left no remnant and systems
 * not yet formed. Under a `STARS` filter the caption says what the filter hides.
 *
 * A click selects the point within 1 rem, which selects the system everywhere. The canvas is an
 * image with an accessible name and is not focusable: keyboard access is through the shared list in
 * `SYSTEMS`, where every point is a row. It is painted when what it shows changes, never on a loop.
 */
export function HrDiagram({
  systems,
  total,
  starFilter,
  selectedId,
  onSelect,
  driveRangeLy,
  stale,
}: HrDiagramProps) {
  const { ref: stageRef, size } = useElementSize();
  const widthPx = size?.widthPx ?? 0;
  const heightPx = size?.heightPx ?? 0;
  const remPx = size?.remPx ?? 0;
  const pixelRatio = size?.devicePixelRatio ?? 1;
  const area = useMemo(() => plotArea(widthPx, heightPx, remPx), [widthPx, heightPx, remPx]);

  // Memoised for their identity, not their cost: an equal re-render keeps the draw list, and the
  // paint effect, which runs when it changes, is not run again (it redraws on demand only).
  const projection = useMemo(
    () =>
      projectHr(systems, area ?? { leftPx: 0, topPx: 0, widthPx: 0, heightPx: 0 }, driveRangeLy),
    [systems, area, driveRangeLy],
  );
  const drawList = useMemo(
    () => (area === null ? null : hrDrawList(projection, area, selectedId, remPx)),
    [projection, area, selectedId, remPx],
  );

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [tokens, setTokens] = useState<ColourTokens | null>(null);
  // Read when mounted and each time the page is shown again, as the spatial view reads them.
  useLayoutEffect(() => {
    if (canvasRef.current === null) {
      return;
    }
    const current = readTokens(canvasRef.current);
    setTokens((previous) =>
      previous !== null && sameTokens(previous, current) ? previous : current,
    );
  }, []);
  const paintTokens = useMemo(
    () => (tokens === null || !stale ? tokens : staleTokens(tokens)),
    [tokens, stale],
  );

  useLayoutEffect(() => {
    const context = canvasRef.current?.getContext("2d") ?? null;
    if (context === null || drawList === null || paintTokens === null) {
      return;
    }
    paint(context, drawList, paintTokens, pixelRatio);
  }, [drawList, paintTokens, pixelRatio]);

  const onClick = (event: MouseEvent<HTMLCanvasElement>): void => {
    if (drawList === null) {
      return;
    }
    const box = event.currentTarget.getBoundingClientRect();
    const picked = pickHr(
      drawList.anchors,
      { xPx: event.clientX - box.left, yPx: event.clientY - box.top },
      remPx,
    );
    if (picked !== null && picked !== selectedId) {
      onSelect(picked);
    }
  };

  const { counts } = projection;
  const accessibleName = "Hertzsprung-Russell diagram";
  // What the filter hides, then what is plotted and what cannot be; a count of values the server
  // should not send appears only when there are any.
  const countRows: ReadonlyArray<{ readonly label: string; readonly text: string }> = [
    ...(starFilter === "all"
      ? []
      : [{ label: "SYSTEMS", text: shownCountText(systems.length, total, starFilter) }]),
    { label: "PLOTTED", text: formatNumber(counts.plotted, 0) },
    { label: "OFF SCALE", text: formatNumber(counts.offScale, 0) },
    { label: "NO PHOTOSPHERE", text: formatNumber(counts.noPhotosphere, 0) },
    { label: "NO REMNANT", text: formatNumber(counts.noRemnant, 0) },
    { label: "NOT YET FORMED", text: formatNumber(counts.notYetFormed, 0) },
    ...(counts.noData === 0 ? [] : [{ label: "NO DATA", text: formatNumber(counts.noData, 0) }]),
  ];

  return (
    <figure className={stale ? "hr-diagram hr-diagram--stale" : "hr-diagram"}>
      <figcaption className="hr-diagram__title">HERTZSPRUNG-RUSSELL DIAGRAM</figcaption>
      <p className="hr-diagram__axis-label">
        LUMINOSITY <SolarUnit quantity="luminosity" />, LOG SCALE
      </p>
      <div className="hr-diagram__stage" ref={stageRef}>
        <canvas
          ref={canvasRef}
          className="hr-diagram__canvas"
          width={Math.round(widthPx * pixelRatio)}
          height={Math.round(heightPx * pixelRatio)}
          // A picture, not a control: it takes no focus, and its points are rows of the systems
          // list, where the keyboard selects them (plan 06, P06.T37).
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="img"
          aria-label={stale ? `${accessibleName}, stale` : accessibleName}
          // A click is a shortcut for the pointer; the list beside it is the keyboard's way to the
          // same selection, which a canvas that took focus would only duplicate.
          // oxlint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions
          onClick={onClick}
        />
        {area === null ? null : (
          <div className="hr-diagram__labels" aria-hidden="true">
            {hrSpectralLetters(area).map(({ letter, xPx }) => (
              <span
                key={letter}
                className="hr-diagram__label hr-diagram__label--class"
                style={{ transform: at(xPx, area.topPx, remPx) }}
              >
                {letter}
              </span>
            ))}
            {HR_TEFF_TICKS_K.map((teffK) => (
              <span
                key={teffK}
                className="hr-diagram__label hr-diagram__label--x"
                style={{ transform: at(hrXPx(teffK, area), area.topPx + area.heightPx, remPx) }}
              >
                {tickText(teffK)}
              </span>
            ))}
            {HR_LOG_L_TICKS.map((logL) => (
              <span
                key={logL}
                className="hr-diagram__label hr-diagram__label--y"
                style={{ transform: at(area.leftPx, hrYPx(logL, area), remPx) }}
              >
                {tickText(10 ** logL)}
              </span>
            ))}
          </div>
        )}
      </div>
      <p className="hr-diagram__axis-label hr-diagram__axis-label--x">
        EFFECTIVE TEMPERATURE K, LOG SCALE
      </p>
      <dl className="hr-diagram__counts">
        {countRows.map(({ label, text }) => (
          <div className="hr-diagram__count" key={label}>
            <dt>{label}</dt>
            <dd>
              {text}
              {/* Every count is read from the answer, so every one is stale with it. */}
              {stale ? <StaleMark /> : null}
            </dd>
          </div>
        ))}
      </dl>
      {/* A group, as the chart's legend is: no HTML element names a legend of a picture. */}
      {/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role */}
      <div className="symbol-legend" role="group" aria-label="Hertzsprung-Russell diagram legend">
        <p className="symbol-legend__item">SYMBOLS NOT TO SCALE</p>
        {/* Neutron stars and black holes are counted, not plotted, so their shapes are not here. */}
        <StarShapeLegend compactRemnants={false} />
        <p className="symbol-legend__item">
          <LegendSymbol shape="circle" diameterRem={SIZE_CLASS_REM[4]} filled available />
          IN DRIVE RANGE
        </p>
        <p className="symbol-legend__item">
          <LegendReticle />
          BRACKET SELECTED
        </p>
        <p className="symbol-legend__item">
          <svg
            className="symbol-legend__mark"
            style={{ width: "1rem", height: "1rem" }}
            viewBox="0 0 10 10"
            aria-hidden="true"
            focusable="false"
          >
            <path d="M3 2L7 5L3 8" />
          </svg>
          PEGGED OFF SCALE
        </p>
      </div>
    </figure>
  );
}
