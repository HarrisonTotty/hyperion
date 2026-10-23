/**
 * The reference tracer's committed references as `gate.ts` reads them (plan R08, R08.T12.b.1):
 * each `<name>.reference.json`, `hyperion-fit atmosphere-reference`'s output
 * (`crates/hyperion-fit/src/atmosphere/reference.rs`, `REFERENCE_FORMAT` 1), joined with its case's
 * geometries for their images, and the client's own `f64` quadrature of the direct beam that each
 * reference's `sunOpticalDepth` is held to (Design note 10; `decision-r08-spheroid.md`).
 */

import type { Rgb } from "../../photometry/toneCurve";
import type { FluxReading, ReferenceReading } from "../gate";
import {
  absorberAmounts,
  absorberPressuresPa,
  curvesOpticalDepth,
  intersectsGround,
  opticalDepth,
  ORACLE_STEPS,
} from "../opticalDepth";
import { spheroidOpticalDepth } from "../tablesCpu";
import type { CaseFigure, ReferenceCase } from "./cases";
import { geodeticPoint, localDirectionVector, type ReferenceGeometry } from "./geometries";

/** One geometry of a reference file, the fields read here. */
export interface ReferenceGeometryFile {
  readonly name: string;
  readonly detectorHalfAngleDeg: number;
  readonly samples: number;
  readonly quarterSamples: number;
  readonly radiance: readonly number[];
  readonly standardError: readonly number[];
  readonly quarterStandardError: readonly number[];
  readonly stokes?: {
    readonly scalarRadiance: readonly number[];
    readonly scalarStandardError: readonly number[];
    readonly scalarQuarterStandardError: readonly number[];
  };
  /** Per sun, per wavelength; `null` where the ground hides the sun. */
  readonly sunOpticalDepth: readonly (readonly (number | null)[])[];
}

/** One aggregate of a reference file, the fields read here. */
export interface ReferenceAggregateFile {
  readonly name: string;
  readonly samples: number;
  readonly quarterSamples: number;
  readonly incidentTop: readonly number[];
  readonly directGround: readonly number[];
  readonly diffuseGround: readonly number[];
  readonly diffuseGroundStandardError: readonly number[];
  readonly diffuseGroundQuarterStandardError: readonly number[];
  readonly upwellingTop: readonly number[];
  readonly upwellingTopStandardError: readonly number[];
  readonly upwellingTopQuarterStandardError: readonly number[];
  /** The scalar fluxes of the same paths, in the Stokes mode. */
  readonly scalar?: {
    readonly diffuseGround: readonly number[];
    readonly diffuseGroundStandardError: readonly number[];
    readonly diffuseGroundQuarterStandardError: readonly number[];
    readonly upwellingTop: readonly number[];
    readonly upwellingTopStandardError: readonly number[];
    readonly upwellingTopQuarterStandardError: readonly number[];
  };
}

/** A reference file, the fields read here. */
export interface ReferenceFile {
  readonly format: number;
  readonly case: string;
  readonly polarisation: string;
  readonly samples: number;
  readonly seed: number;
  readonly wavelengthsNm: readonly number[];
  readonly geometries: readonly ReferenceGeometryFile[];
  readonly aggregates: readonly ReferenceAggregateFile[];
}

/** A per-wavelength array of the reference as a channel triple. */
function rgbOf(values: readonly number[], what: string): Rgb {
  const [r, g, b] = values;
  if (values.length !== 3 || r === undefined || g === undefined || b === undefined) {
    throw new Error(`${what} has ${values.length} wavelengths, not the three channels`);
  }
  return [r, g, b];
}

/** Which radiance of a Stokes reference a reading takes: the vector I, or the paths' scalar. */
export type RadianceKind = "vector" | "scalar";

/**
 * A reference's geometries as the gate's readings, each in its case's image.
 *
 * @throws Error for a geometry the case does not have, or a scalar reading of a reference traced
 *   without the Stokes mode.
 */
export function readingsOf(
  referenceCase: ReferenceCase,
  reference: ReferenceFile,
  kind: RadianceKind = "vector",
): ReferenceReading[] {
  const images = new Map(referenceCase.geometries.map((g) => [g.name, g.image] as const));
  return reference.geometries.map((g) => {
    const image = images.get(g.name);
    if (image === undefined) {
      throw new Error(`${referenceCase.name} has no geometry ${g.name}`);
    }
    const stokes = g.stokes;
    if (kind === "scalar" && stokes === undefined) {
      throw new Error(`${reference.case} was traced without --stokes`);
    }
    const scalar = kind === "scalar" && stokes !== undefined;
    return {
      name: g.name,
      image,
      radiance: rgbOf(scalar ? stokes.scalarRadiance : g.radiance, g.name),
      standardError: rgbOf(scalar ? stokes.scalarStandardError : g.standardError, g.name),
      samples: g.samples,
      quarterSamples: g.quarterSamples,
      quarterStandardError: rgbOf(
        scalar ? stokes.scalarQuarterStandardError : g.quarterStandardError,
        g.name,
      ),
    };
  });
}

/**
 * A reference's aggregates as the gate's flux readings: each downwelling flux at the ground, direct
 * and diffuse, and each plane albedo's upwelling flux at the top, of the vector or the scalar
 * radiance.
 *
 * @throws Error for a scalar reading of a reference traced without the Stokes mode.
 */
export function fluxReadingsOf(
  reference: ReferenceFile,
  kind: RadianceKind = "vector",
): FluxReading[] {
  return reference.aggregates.flatMap((a) => {
    let fluxes = {
      diffuseGround: a.diffuseGround,
      diffuseGroundStandardError: a.diffuseGroundStandardError,
      diffuseGroundQuarterStandardError: a.diffuseGroundQuarterStandardError,
      upwellingTop: a.upwellingTop,
      upwellingTopStandardError: a.upwellingTopStandardError,
      upwellingTopQuarterStandardError: a.upwellingTopQuarterStandardError,
    };
    if (kind === "scalar") {
      if (a.scalar === undefined) {
        throw new Error(`${reference.case} was traced without --stokes`);
      }
      fluxes = a.scalar;
    }
    const direct = rgbOf(a.directGround, a.name);
    const diffuse = rgbOf(fluxes.diffuseGround, a.name);
    return [
      {
        name: `${a.name}/downwelling`,
        flux: [direct[0] + diffuse[0], direct[1] + diffuse[1], direct[2] + diffuse[2]],
        standardError: rgbOf(fluxes.diffuseGroundStandardError, a.name),
        samples: a.samples,
        quarterSamples: a.quarterSamples,
        quarterStandardError: rgbOf(fluxes.diffuseGroundQuarterStandardError, a.name),
      },
      {
        name: `${a.name}/upwelling`,
        flux: rgbOf(fluxes.upwellingTop, a.name),
        standardError: rgbOf(fluxes.upwellingTopStandardError, a.name),
        samples: a.samples,
        quarterSamples: a.quarterSamples,
        quarterStandardError: rgbOf(fluxes.upwellingTopQuarterStandardError, a.name),
      },
    ];
  });
}

const RAD = Math.PI / 180;

/** The client's quadrature of a beam, converged: its optical depth per channel, or none. */
export interface ClientBeam {
  /** τ per channel, every term's and each absorber's −ln T of the beam's own column. */
  readonly opticalDepth: Rgb;
  /** The largest change on halving the steps, relative to max(1, τ). */
  readonly convergence: number;
}

/** How far the quadrature is refined: its steps double until they converge below 10⁻⁷. */
export const BEAM_CONVERGENCE = 1e-7;

/** On a sphere: the beam's depth from (r, μ), entering at the top where it starts above it. */
function sphereBeam(
  referenceCase: ReferenceCase,
  radiusM: number,
  geometry: ReferenceGeometry,
  steps: number,
): Rgb | null {
  const { medium } = referenceCase;
  const top = radiusM + medium.topHeightM;
  let r = radiusM + geometry.observer.heightM;
  let mu = Math.cos(geometry.sun.zenithDeg * RAD);
  if (r > top) {
    const discriminant = r * r * (mu * mu - 1) + top * top;
    if (mu >= 0 || discriminant <= 0) {
      return [0, 0, 0];
    }
    const t = -r * mu - Math.sqrt(discriminant);
    mu = (r * mu + t) / top;
    r = top;
  }
  if (intersectsGround({ bottomRadiusM: radiusM, topRadiusM: top }, r, mu)) {
    return null;
  }
  const terms = opticalDepth(medium, radiusM, r, mu, steps);
  if (referenceCase.curves.length === 0) {
    return terms;
  }
  const amounts = absorberAmounts(medium, radiusM, r, mu, steps);
  const pressures = absorberPressuresPa(medium, radiusM, r, mu, false, steps);
  const absorbed = curvesOpticalDepth(referenceCase.curves, amounts, pressures);
  return [terms[0] + absorbed[0], terms[1] + absorbed[1], terms[2] + absorbed[2]];
}

/** On the spheroid: R08.T6.e's brute-force march, density at h\* (`spheroidOpticalDepth`). */
function spheroidBeam(
  referenceCase: ReferenceCase,
  gravity: Extract<CaseFigure, { kind: "spheroid" }>["gravity"],
  geometry: ReferenceGeometry,
  pieceM: number,
): Rgb | null {
  const spheroid = gravity.spheroid;
  const position = geodeticPoint(
    spheroid,
    geometry.observer.latitudeDeg,
    geometry.observer.heightM,
  );
  const direction = localDirectionVector(geometry.observer.latitudeDeg, geometry.sun);
  const beam = spheroidOpticalDepth(
    referenceCase.medium,
    { figure: spheroid, gravity },
    position,
    direction,
    pieceM,
  );
  return beam.hitsGround ? null : beam.opticalDepth;
}

/**
 * The client's own `f64` quadrature of a geometry's beam towards its sun, converged below
 * {@link BEAM_CONVERGENCE} (Design note 10's beam check): on a sphere `opticalDepth.ts`'s
 * `opticalDepth` for the terms with `absorberAmounts`, `absorberPressuresPa` and
 * `curvesOpticalDepth` for the absorbers, from the top where the observer is above it; on the
 * spheroid `tablesCpu.ts`'s `spheroidOpticalDepth`, its pieces halved from 5 km. `null` where the
 * ground hides the sun.
 *
 * @throws Error where the quadrature does not converge within twelve halvings.
 */
export function clientBeam(
  referenceCase: ReferenceCase,
  geometry: ReferenceGeometry,
): ClientBeam | null {
  const { figure } = referenceCase;
  const at = (refinement: number): Rgb | null => {
    let depth: Rgb | null;
    switch (figure.kind) {
      case "sphere":
        depth = sphereBeam(referenceCase, figure.radiusM, geometry, ORACLE_STEPS * 2 ** refinement);
        break;
      case "spheroid":
        depth = spheroidBeam(referenceCase, figure.gravity, geometry, 5_000 / 2 ** refinement);
        break;
    }
    return depth;
  };
  let previous = at(0);
  for (let refinement = 1; refinement <= 12; refinement += 1) {
    const next = at(refinement);
    if (previous === null || next === null) {
      if (previous !== next) {
        throw new Error(`${geometry.name}'s beam is hidden at one refinement and not another`);
      }
      return null;
    }
    const before = previous;
    const change = Math.max(
      ...next.map((tau, c) => Math.abs(tau - (before[c] ?? Number.NaN)) / Math.max(1, tau)),
    );
    if (change < BEAM_CONVERGENCE) {
      return { opticalDepth: next, convergence: change };
    }
    previous = next;
  }
  throw new Error(`${geometry.name}'s beam quadrature did not converge`);
}
