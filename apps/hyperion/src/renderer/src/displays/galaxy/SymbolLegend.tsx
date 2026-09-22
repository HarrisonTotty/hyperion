import { SolarMassUnit } from "../../components/SolarMassUnit";
import { SIZE_CLASS_REM } from "../../spatial/symbols";
import { formatBandMsun, type LayerBand } from "./chartModel";

/** The unit box every legend mark is drawn in, so that one circle serves at every size. */
const MARK_BOX = "0 0 10 10";

/** Radius of a legend circle in its box, leaving room for the outline drawn inside the diameter. */
const MARK_RADIUS = 4.25;

interface MarkProps {
  /** Diameter in `rem`, from the size class it stands for. */
  readonly diameterRem: number;
  readonly filled: boolean;
  /** Whether the mark stands for what is within the set range, drawn in `--accent`. */
  readonly available?: boolean;
}

/** One circle of the legend, at the size and fill it stands for. */
function Mark({ diameterRem, filled, available = false }: MarkProps) {
  return (
    <svg
      className={
        available ? "symbol-legend__mark symbol-legend__mark--available" : "symbol-legend__mark"
      }
      style={{ width: `${diameterRem}rem`, height: `${diameterRem}rem` }}
      viewBox={MARK_BOX}
      aria-hidden="true"
      focusable="false"
    >
      <circle
        cx="5"
        cy="5"
        r={MARK_RADIUS}
        className={filled ? "symbol-legend__filled" : undefined}
      />
    </svg>
  );
}

/** The bracket reticle that marks the selection, at the size of the largest mark. */
function Reticle() {
  return (
    <svg
      className="symbol-legend__mark"
      style={{ width: "1.5rem", height: "1.5rem" }}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M4 9V4H9M15 4H20V9M20 15V20H15M9 20H4V15" />
    </svg>
  );
}

interface SymbolLegendProps {
  /** The mass bands of the last census; `null` before the first answer. */
  readonly bands: ReadonlyArray<LayerBand> | null;
}

/**
 * What a chart's marks mean: the mass band each size stands for, and what fill and colour say.
 *
 * @remarks
 * The guide's 3D conventions require the legend: size encodes a class and never depth, which
 * `SYMBOLS NOT TO SCALE` states, fill says which side of the reference plane a system is on, and
 * `--accent` marks what is within the set range, which is repeated here in words so that colour is
 * never the only signal. Size is shown as the five marks between the lightest and heaviest initial
 * mass the census names, rather than one line per band, since the chart's page cannot spare five
 * lines at 1280 px; each band's edges are in the census table and each system's mass in the readout.
 * The edges come from the census the server sent, so the client holds no copy of the band table, and
 * before the first answer the legend shows only what fill and colour mean.
 */
export function SymbolLegend({ bands }: SymbolLegendProps) {
  const lightest = bands?.[0];
  const heaviest = bands?.at(-1);
  return (
    // A group, so that what the marks mean is one block to read and to find, not loose text; no
    // HTML element names a legend of a picture (`fieldset` groups form controls).
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="symbol-legend" role="group" aria-label="Chart legend">
      <p className="symbol-legend__item">SYMBOLS NOT TO SCALE</p>
      {lightest === undefined || heaviest === undefined ? null : (
        <p className="symbol-legend__item">
          INIT MASS <SolarMassUnit /> {formatBandMsun(lightest.minMsun)}
          {bands?.map((band) => (
            <Mark key={band.layer} diameterRem={SIZE_CLASS_REM[band.index]} filled />
          ))}
          {formatBandMsun(heaviest.maxMsun)}
        </p>
      )}
      <p className="symbol-legend__item">
        <Mark diameterRem={SIZE_CLASS_REM[4]} filled />
        FILLED NORTH OF PLANE
      </p>
      <p className="symbol-legend__item">
        <Mark diameterRem={SIZE_CLASS_REM[4]} filled={false} />
        OPEN SOUTH OF PLANE
      </p>
      <p className="symbol-legend__item">
        <Mark diameterRem={SIZE_CLASS_REM[4]} filled available />
        ACCENT WITHIN SET RANGE
      </p>
      <p className="symbol-legend__item">
        <Reticle />
        BRACKET SELECTED
      </p>
    </div>
  );
}
