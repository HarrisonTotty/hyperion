import { SolarMassUnit } from "../../components/SolarMassUnit";
import { RINGED_CIRCLE_MIN_SIZE_CLASS } from "../../lib/galaxy/starSymbols";
import { LegendReticle } from "../../spatial/LegendReticle";
import { LegendSymbol } from "../../spatial/LegendSymbol";
import { SIZE_CLASS_REM } from "../../spatial/symbols";
import { formatBandMsun, type LayerBand } from "./chartModel";
import { StarShapeLegend } from "./StarShapeLegend";

interface SymbolLegendProps {
  /** The mass bands of the last census; `null` before the first answer. */
  readonly bands: ReadonlyArray<LayerBand> | null;
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
 * light giant's size is not read as its mass.
 */
export function SymbolLegend({ bands }: SymbolLegendProps) {
  const lightest = bands?.[0];
  const heaviest = bands?.at(-1);
  const floorBand = bands?.find((band) => band.index === RINGED_CIRCLE_MIN_SIZE_CLASS);
  return (
    // A group, so that what the marks mean is one block to read and to find, not loose text; no
    // HTML element names a legend of a picture (`fieldset` groups form controls).
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="symbol-legend" role="group" aria-label="Chart legend">
      <p className="symbol-legend__item">SYMBOLS NOT TO SCALE</p>
      <StarShapeLegend compactRemnants />
      {lightest === undefined || heaviest === undefined ? null : (
        <p className="symbol-legend__item">
          INIT MASS <SolarMassUnit /> {formatBandMsun(lightest.minMsun)}
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
    </div>
  );
}
