import { formatSubstellarMass } from "../../lib/format";
import { SolarMassUnit } from "../../components/SolarMassUnit";
import { RINGED_CIRCLE_MIN_SIZE_CLASS } from "../../lib/galaxy/starSymbols";
import { LegendReticle } from "../../spatial/LegendReticle";
import { LegendSymbol } from "../../spatial/LegendSymbol";
import { SIZE_CLASS_REM } from "../../spatial/symbols";
import { formatBandMsun, type LayerBand, OBSERVER_LABEL } from "./chartModel";
import { StarShapeLegend } from "./StarShapeLegend";

/**
 * What the legend calls an apparent position's marks in words (plan 12, P12.T11): the tick, where
 * the light shows the system, and the dashed joiner to where it is now.
 */
export const APPARENT_LEGEND = "TICK APPARENT POSITION, DASHED TO PRESENT";

interface SymbolLegendProps {
  /** The mass bands of the last census; `null` before the first answer. */
  readonly bands: ReadonlyArray<LayerBand> | null;
  /**
   * The lower edge of the brown dwarfs' band in M☉ when the last query included them, whose
   * objects take the smallest size; `null` when it did not. None when absent.
   */
  readonly brownDwarfFloorMsun?: number | null;
  /**
   * Whether the chart on show was asked in observed mode, which draws the observer and the
   * apparent positions (plan 12, P12.T11); `false` when absent.
   */
  readonly observed?: boolean;
}

/** The observer's eye as the chart draws it, open, at its 1 rem width. */
function ObserverSample() {
  return (
    <svg
      className="symbol-legend__mark"
      style={{ width: "1rem", height: "1rem" }}
      viewBox="-5 -5 10 10"
      aria-hidden="true"
      focusable="false"
    >
      {/* Two quadratic arcs, their control points the half-width above and below: `eyePoints`. */}
      <path d="M -4.25 0 Q 0 -4.25 4.25 0 Q 0 4.25 -4.25 0 Z" />
    </svg>
  );
}

/**
 * An apparent position as the chart draws it: the dashed joiner, 1 px with a predicted path's dash,
 * and the tick across its end, at the line scale.
 */
function ApparentSample() {
  return (
    <svg
      className="symbol-legend__mark symbol-legend__apparent"
      style={{ width: "1.5rem", height: "0.5rem" }}
      viewBox="0 0 24 8"
      aria-hidden="true"
      focusable="false"
    >
      <path className="symbol-legend__joiner" d="M 0 4 H 22" />
      {/* 4 of the box's 16 units a rem: the chart's 0.25 rem. */}
      <path className="symbol-legend__tick" d="M 22 2 V 6" />
    </svg>
  );
}

/**
 * What a chart's marks mean: the kind of star each shape stands for, the mass band each size
 * stands for, and what fill and colour say.
 *
 * @remarks
 * The guide's 3D conventions require the legend: size encodes a class and never depth, which
 * `SYMBOLS NOT TO SCALE` states, fill says which side of the reference plane a system is on, and
 * `--accent` marks what is within the drive range, which is repeated here in words so that colour
 * is never the only signal; the setting's one name is `DRIVE RANGE` (the orchestrator's ruling 15).
 * Size is shown as the five marks between the lightest and heaviest initial mass the census names,
 * rather than one line per band, since the chart's page cannot spare five lines at 1280 px; each
 * band's edges are in the census table and each system's mass in the readout. The edges come from
 * the census the server sent, so the client holds no copy of the band table, and before the first
 * answer the legend shows only what shape, fill and colour mean. The five shapes are the star
 * symbol set (plan 06, design note 17), each named for assistive technology; a ringed circle is
 * never drawn smaller than the band of size class 2 (ruling 35.4), which the legend says, so that a
 * light giant's size is not read as its mass. The inverted triangle is a free-floating planet, and
 * the circle's kinds include the brown dwarf (plan 13, P13.T8.d); both are drawn at the lightest
 * band's size, since no size class below layer A is drawn (design note 15). So when the query
 * includes the brown dwarfs the size scale starts from their floor, `INIT MASS M☉ 0.012`, and the
 * smallest circle no longer claims 0.08 M☉ (the orchestrator's ruling 134.7); a planet's size is
 * the guide's to explain, and the size row, drawn in circles, claims nothing of triangles.
 *
 * A chart asked in observed mode also draws the observer's eye and the apparent positions' ticks
 * and dashed joiners, so the legend then names them in words, as it names every mark the chart can
 * draw (plan 12, P12.T11): `OBSERVER`, and `TICK APPARENT POSITION, DASHED TO PRESENT`.
 */
export function SymbolLegend({
  bands,
  brownDwarfFloorMsun = null,
  observed = false,
}: SymbolLegendProps) {
  const lightest = bands?.[0];
  const heaviest = bands?.at(-1);
  const floorBand = bands?.find((band) => band.index === RINGED_CIRCLE_MIN_SIZE_CLASS);
  return (
    // A group, so that what the marks mean is one block to read and to find, not loose text; no
    // HTML element names a legend of a picture (`fieldset` groups form controls).
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="symbol-legend" role="group" aria-label="Chart legend">
      <p className="symbol-legend__item">SYMBOLS NOT TO SCALE</p>
      <StarShapeLegend compactRemnants planets />
      {lightest === undefined || heaviest === undefined ? null : (
        <p className="symbol-legend__item">
          INIT MASS <SolarMassUnit />{" "}
          {brownDwarfFloorMsun === null
            ? formatBandMsun(lightest.minMsun)
            : formatSubstellarMass(brownDwarfFloorMsun, "brown_dwarf").value}
          {bands?.map((band) => (
            <LegendSymbol
              key={band.layer}
              shape="circle"
              diameterRem={SIZE_CLASS_REM[band.index]}
              filled
            />
          ))}
          {formatBandMsun(heaviest.maxMsun)}
        </p>
      )}
      {floorBand === undefined ? null : (
        // A giant of a light layer is drawn larger than its band, so that its ring and disc read
        // open or filled (the orchestrator's ruling 35.4); the size scale above would misread it.
        <p className="symbol-legend__item">
          <LegendSymbol
            shape="ringed-circle"
            diameterRem={SIZE_CLASS_REM[floorBand.index]}
            filled
          />
          RINGED CIRCLE SIZE AT LEAST {formatBandMsun(floorBand.minMsun)} <SolarMassUnit />
        </p>
      )}
      <p className="symbol-legend__item">
        <LegendSymbol shape="circle" diameterRem={SIZE_CLASS_REM[4]} filled />
        FILLED NORTH OF PLANE
      </p>
      <p className="symbol-legend__item">
        <LegendSymbol shape="circle" diameterRem={SIZE_CLASS_REM[4]} filled={false} />
        OPEN SOUTH OF PLANE
      </p>
      <p className="symbol-legend__item">
        <LegendSymbol shape="circle" diameterRem={SIZE_CLASS_REM[4]} filled available />
        IN DRIVE RANGE
      </p>
      <p className="symbol-legend__item">
        <LegendReticle />
        BRACKET SELECTED
      </p>
      {observed ? (
        <>
          <p className="symbol-legend__item">
            <ObserverSample />
            {OBSERVER_LABEL}
          </p>
          <p className="symbol-legend__item">
            <ApparentSample />
            {APPARENT_LEGEND}
          </p>
        </>
      ) : null}
    </div>
  );
}
