import type { ObjectKindDto } from "@hyperion/protocol";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import { starSymbol } from "../../lib/galaxy/starSymbols";
import { objectKindLabel } from "../../lib/system/words";
import { LegendReticle } from "../../spatial/LegendReticle";
import { LegendSymbol } from "../../spatial/LegendSymbol";
import type { SizeClass, SymbolShape } from "../../spatial/marks";
import { SIZE_CLASS_REM } from "../../spatial/symbols";
import { formatBandMsun, type LayerBand } from "../galaxy/chartModel";
import type { OrbitPlane } from "./orbitMap";

interface PathSampleProps {
  readonly selected: boolean;
}

/** A sample of a path as the map draws it: 1 px for an orbit, 2 px for the selected one. */
function PathSample({ selected }: PathSampleProps) {
  return (
    <svg
      className={
        selected
          ? "symbol-legend__mark orbit-legend__path orbit-legend__path--selected"
          : "symbol-legend__mark orbit-legend__path orbit-legend__path--reference"
      }
      viewBox="0 0 16 4"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M 0 2 H 16" />
    </svg>
  );
}

/** A kind of body the map draws, with the symbol and the size it is drawn at. */
export interface BodyLegendEntry {
  /** The kind in words, which keys the entry: `GIANT PLANET`, `MOON`. */
  readonly label: string;
  readonly shape: SymbolShape;
  readonly sizeClass: SizeClass;
}

/** Props of {@link OrbitLegend}. */
export interface OrbitLegendProps {
  /** The kinds of the stars drawn, each named once with its symbol, in the order first drawn. */
  readonly kinds: ReadonlyArray<ObjectKindDto>;
  /**
   * The kinds of the other bodies drawn, each named once, at the size it is drawn at, since size
   * tells a giant planet from a smaller one.
   */
  readonly bodies: ReadonlyArray<BodyLegendEntry>;
  /** The chart census's mass bands, which each host's size stands for. */
  readonly bands: ReadonlyArray<LayerBand>;
  readonly plane: OrbitPlane;
}

/**
 * What the orbit map's marks and lines mean (plan 14, P14.T42.b; the owner's draft of the guide's
 * orbit-map conventions).
 *
 * @remarks
 * `BODIES NOT TO SCALE` first and alone, since every symbol here is a body and one name per thing
 * wants one label (the orchestrator's ruling 36). Then each kind drawn, with its symbol and its
 * name, so that shape is never the only signal; each other kind of body drawn, at the size it is
 * drawn at, so that a giant planet's larger mark is read as its class; what a star's size stands
 * for, the initial-mass bands of the chart's census, as on the chart (ruling 36); which side of
 * the reference plane fill marks, named for the plane the map is drawn on; the orbit's line and the
 * selected orbit's wider one, which a selected belt's, ring's or halo's edges share, since width,
 * not colour alone, carries the selection (ruling 44.2); and the reticle.
 */
export function OrbitLegend({ kinds, bodies, bands, plane }: OrbitLegendProps) {
  const lightest = bands[0];
  const heaviest = bands.at(-1);
  const [above, below] = plane.isSystemPlane
    ? [`FILLED ABOVE ${plane.name}`, `OPEN BELOW ${plane.name}`]
    : [`FILLED NORTH OF ${plane.name}`, `OPEN SOUTH OF ${plane.name}`];
  const named = [...new Set(kinds)].flatMap((kind) => {
    const shape = starSymbol(kind);
    return shape === null ? [] : [{ kind, shape }];
  });
  return (
    // A group, so that what the marks mean is one block to read and to find; no HTML element names
    // a legend of a picture (`fieldset` groups form controls).
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="symbol-legend orbit-legend" role="group" aria-label="Orbit map legend">
      <p className="symbol-legend__item">BODIES NOT TO SCALE</p>
      {named.map(({ kind, shape }) => (
        <p className="symbol-legend__item" key={kind}>
          <LegendSymbol shape={shape} diameterRem={SIZE_CLASS_REM[4]} filled />
          {objectKindLabel(kind)}
        </p>
      ))}
      {bodies.map(({ label, shape, sizeClass }) => (
        <p className="symbol-legend__item" key={label}>
          <LegendSymbol shape={shape} diameterRem={SIZE_CLASS_REM[sizeClass]} filled />
          {label}
        </p>
      ))}
      {lightest === undefined || heaviest === undefined ? null : (
        <p className="symbol-legend__item">
          INIT MASS <SolarMassUnit /> {formatBandMsun(lightest.minMsun)}
          {bands.map((band) => (
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
      <p className="symbol-legend__item">
        <LegendSymbol shape="circle" diameterRem={SIZE_CLASS_REM[4]} filled />
        {above}
      </p>
      <p className="symbol-legend__item">
        <LegendSymbol shape="circle" diameterRem={SIZE_CLASS_REM[4]} filled={false} />
        {below}
      </p>
      <p className="symbol-legend__item">
        <PathSample selected={false} />
        ORBIT
      </p>
      <p className="symbol-legend__item">
        <PathSample selected />
        SELECTED ORBIT OR EDGES
      </p>
      <p className="symbol-legend__item">
        <LegendReticle />
        BRACKET SELECTED
      </p>
    </div>
  );
}
