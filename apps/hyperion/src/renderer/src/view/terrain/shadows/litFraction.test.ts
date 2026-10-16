import { describe, expect, it } from "vitest";

import { sphereIrradianceBruteForce, UNIFORM_DISC } from "../../lighting/oracle";
import { sphereIrradianceFactor } from "../../lighting/sphereIrradiance";
import { decodeHorizonRad } from "./horizonLookup";
import {
  elementHorizonRad,
  litFraction,
  MAX_STAR_DISTANCE_RADII,
  terrainHorizonFactor,
} from "./litFraction";

const RAD = Math.PI / 180;

/** The Sun from 1 au: H = au ÷ R☉ (IAU 2012 B2's au, IAU 2015 B3's nominal radius). */
const H_SUN = 1.495_978_707e11 / 6.957e8;

/** The Sun's angular radius at 1 au, arcsin(1 ÷ H), 959.23″. */
const RHO_SUN = Math.asin(1 / H_SUN);

/** A body-fixed direction's components, the datum's normal along z. */
type Direction = readonly [number, number, number];

function dot3(a: Direction, b: Direction): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

/** The unit direction to a star at azimuth 0 and elevation `elevationRad` above the datum. */
function starDirection(elevationRad: number): Direction {
  return [Math.cos(elevationRad), 0, Math.sin(elevationRad)];
}

/** A slope's normal, tilted `slopeDeg` from the datum's towards the azimuth `bearingDeg`. */
function slopeNormal(slopeDeg: number, bearingDeg: number): Direction {
  const [tilt, bearing] = [slopeDeg * RAD, bearingDeg * RAD];
  return [Math.sin(tilt) * Math.cos(bearing), Math.sin(tilt) * Math.sin(bearing), Math.cos(tilt)];
}

/** The angle between unit `a` and unit `b`, by atan2 of their cross and dot products. */
function angleFrom(a: Direction, b: Direction): number {
  const c = Math.hypot(
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  );
  return Math.atan2(c, dot3(a, b));
}

/** A case of the hard-step test: its value, and the hard step's away from the edge, else `null`. */
interface StepCase {
  readonly value: number;
  readonly step: number | null;
}

/** The cases whose value is not finite and in [0, `top`]. */
function outside(cases: readonly StepCase[], top: number): StepCase[] {
  return cases.filter(({ value }) => !(Number.isFinite(value) && value >= 0 && value <= top));
}

/** The cases away from the edge whose value is not the hard step's. */
function missed(cases: readonly StepCase[]): StepCase[] {
  return cases.filter(({ value, step }) => step !== null && Math.abs(value - step) > 1e-12);
}

/** An arc of a ring's azimuths: its start and its length, radians; the whole ring at 2π. */
interface Arc {
  readonly start: number;
  readonly length: number;
}

/** The azimuths ψ where A + B cos ψ + C sin ψ exceeds `t`, as an arc, or `null` for none. */
function arcAbove(a: number, b: number, c: number, t: number): Arc | null {
  const r = Math.hypot(b, c);
  const cut = r === 0 ? (a > t ? -2 : 2) : (t - a) / r;
  if (cut <= -1) {
    return { start: 0, length: 2 * Math.PI };
  }
  if (cut >= 1) {
    return null;
  }
  const half = Math.acos(cut);
  // From [0, 2π), so that the copies of another arc a turn either side meet every overlap.
  const turn = 2 * Math.PI;
  return { start: (((Math.atan2(c, b) - half) % turn) + turn) % turn, length: 2 * half };
}

/**
 * The irradiance on a slope of unit normal `n` from a uniform disc of angular radius `rho` about
 * the star at elevation `starRad`, azimuth 0, over the disc's face-on irradiance, counting the
 * directions above the datum's horizon at `horizonRad` and above the slope's plane, each weighed
 * by n · ω: an oracle of the terrain's horizon term independent of R07's, by the midpoint rule
 * over rings about the disc's centre, each ring exact in azimuth (the two arcs intersected and
 * n · ω integrated over them in closed form).
 */
function datumFrameIrradiance(
  n: Direction,
  starRad: number,
  horizonRad: number,
  rho: number,
): number {
  const s = starDirection(starRad);
  const e1: Direction = [-Math.sin(starRad), 0, Math.cos(starRad)];
  const e2: Direction = [0, 1, 0];
  const rings = 1500;
  let lit = 0;
  let faceOn = 0;
  for (let a = 0; a < rings; a += 1) {
    const theta = ((a + 0.5) / rings) * rho;
    const [cosT, sinT] = [Math.cos(theta), Math.sin(theta)];
    faceOn += sinT * cosT * 2 * Math.PI;
    // On the ring, v · ω = cos θ (v · s) + sin θ cos ψ (v · e₁) + sin θ sin ψ (v · e₂).
    const terms = (v: Direction): readonly [number, number, number] => [
      cosT * dot3(v, s),
      sinT * dot3(v, e1),
      sinT * dot3(v, e2),
    ];
    const [na, nb, nc] = terms(n);
    const [za, zb, zc] = terms([0, 0, 1]);
    const plane = arcAbove(na, nb, nc, 0);
    const sky = arcAbove(za, zb, zc, Math.sin(horizonRad));
    if (plane === null || sky === null) {
      continue;
    }
    for (const shift of [-2 * Math.PI, 0, 2 * Math.PI]) {
      const p = Math.max(plane.start, sky.start + shift);
      const q = Math.min(plane.start + plane.length, sky.start + shift + sky.length);
      if (q > p) {
        lit +=
          sinT *
          (na * (q - p) + nb * (Math.sin(q) - Math.sin(p)) - nc * (Math.cos(q) - Math.cos(p)));
      }
    }
  }
  return lit / faceOn;
}

describe("the horizon's lit fraction (Design note 10)", () => {
  it("is 0 and 1 beyond the disc's radius either side of the horizon, ½ on it", () => {
    for (const horizon of [-0.3, 0, 5 * RAD, 40 * RAD]) {
      expect(litFraction(horizon - RHO_SUN, horizon, RHO_SUN)).toBe(0);
      expect(litFraction(horizon - 3 * RHO_SUN, horizon, RHO_SUN)).toBe(0);
      expect(litFraction(horizon + RHO_SUN, horizon, RHO_SUN)).toBe(1);
      expect(litFraction(horizon + 3 * RHO_SUN, horizon, RHO_SUN)).toBe(1);
      expect(litFraction(horizon, horizon, RHO_SUN)).toBeCloseTo(0.5, 15);
    }
  });

  it("takes the share of a disc above a chord, by hand", () => {
    // x = ½: 1 − (π ÷ 3 − ½ √¾) ÷ π.
    expect(litFraction(0.5, 0, 1)).toBeCloseTo(
      1 - (Math.PI / 3 - 0.5 * Math.sqrt(0.75)) / Math.PI,
      15,
    );
    for (const x of [0.1, 0.37, 0.8]) {
      expect(litFraction(x, 0, 1) + litFraction(-x, 0, 1)).toBeCloseTo(1, 15);
    }
    let previous = 0;
    for (let k = -50; k <= 50; k += 1) {
      const f = litFraction((k / 50) * RHO_SUN, 0, RHO_SUN);
      expect(f).toBeGreaterThanOrEqual(previous);
      previous = f;
    }
  });
});

describe("the terrain's horizon term (R10.T8.b)", () => {
  it("is R07's smooth-figure value bit for bit with a horizon of 0 where the element is the datum", () => {
    for (const elevation of [60, 10, 0.5, 0.2, 0, -0.1, -0.26, -2].map((d) => d * RAD)) {
      const phi = Math.PI / 2 - elevation;
      const star = Math.PI / 2 - phi;
      expect(elementHorizonRad(0, star, phi)).toBe(0);
      expect(
        Object.is(terrainHorizonFactor(H_SUN, phi, star, 0), sphereIrradianceFactor(H_SUN, phi)),
      ).toBe(true);
      // The smooth ground's own horizon is below its plane, which R07 takes as the plane.
      const curvature = decodeHorizonRad(-1);
      expect(terrainHorizonFactor(H_SUN, phi, star, curvature)).toBe(
        sphereIrradianceFactor(H_SUN, phi),
      );
    }
  });

  it("is R07's oracle for the map's horizon in the element's frame", () => {
    // A slope tilted 20° towards the Sun under a horizon of 5° against the datum.
    const tilt = 20 * RAD;
    const horizon = decodeHorizonRad(1820);
    for (const elevationDeg of [8, 5.5, 5.27, 5.2, 5.1, 5, 4.9, 4.8, 4.73, 4.5, 2]) {
      const star = elevationDeg * RAD;
      const phi = Math.PI / 2 - star - tilt;
      const local = elementHorizonRad(horizon, star, phi);
      const factor = terrainHorizonFactor(H_SUN, phi, star, horizon);
      expect(factor).toBe(sphereIrradianceFactor(H_SUN, phi, local));
      const oracle = sphereIrradianceBruteForce(H_SUN, phi, local, UNIFORM_DISC, 100, 20_000);
      const cut = Math.abs(star - horizon) < RHO_SUN;
      // Wholly up or wholly down, to 10⁻⁶; across the cut, to R07.T6.a's own tolerance there.
      expect(Math.abs(factor - oracle)).toBeLessThan(cut ? 2e-3 * Math.cos(phi) + 1e-6 : 1e-6);
    }
  });

  it("cuts the disc where its centre stands at the horizon above the datum, whatever the slope", () => {
    const horizon = 5 * RAD;
    // Slopes towards the Sun, and one away from it by less than the horizon, which still cuts.
    for (const tilt of [-3, 0, 10, 25].map((d) => d * RAD)) {
      const phiAt = (star: number): number => Math.PI / 2 - star - tilt;
      const at = (star: number): number =>
        terrainHorizonFactor(H_SUN, phiAt(star), star, horizon) / Math.cos(phiAt(star));
      // Below the horizon by the disc's radius, dark; above, the whole face-on cosine.
      expect(at(horizon - 1.01 * RHO_SUN)).toBe(0);
      expect(at(horizon + 1.01 * RHO_SUN)).toBeCloseTo(1, 9);
      // On it, half the disc, to Howell's term of the half above, 2 ÷ (3π H) cot η of it.
      expect(at(horizon)).toBeCloseTo(0.5, 1);
      // Rising through the cut as the Sun rises.
      let previous = 0;
      for (let k = -40; k <= 40; k += 1) {
        const star = horizon + (k / 40) * RHO_SUN;
        const f = terrainHorizonFactor(H_SUN, phiAt(star), star, horizon);
        expect(f).toBeGreaterThanOrEqual(previous);
        previous = f;
      }
    }
    // A slope facing the Sun 1° below the horizon is dark, though the Sun stands 24° above the
    // slope's own plane: R07's term at the map's raw horizon would light it, at 0.41 of face-on.
    const star = 4 * RAD;
    const phi = Math.PI / 2 - star - 20 * RAD;
    expect(terrainHorizonFactor(H_SUN, phi, star, horizon)).toBe(0);
    expect(sphereIrradianceFactor(H_SUN, phi, horizon)).toBeGreaterThan(0.3);
  });

  it("is the disc's light above the datum's horizon and the slope's plane on a slope across the Sun's bearing", () => {
    // The datum's normal z, the Sun at azimuth 0, a slope of 20° whose fall line is β from it.
    const horizon = 5 * RAD;
    for (const bearing of [45, 90]) {
      const n = slopeNormal(20, bearing);
      for (const k of [-0.5, 0, 0.5]) {
        const star = horizon + k * RHO_SUN;
        const phi = angleFrom(n, starDirection(star));
        expect(
          Math.abs(
            terrainHorizonFactor(H_SUN, phi, star, horizon) -
              datumFrameIrradiance(n, star, horizon, RHO_SUN),
          ),
        ).toBeLessThan(1e-3 * Math.cos(phi) + 1e-6);
      }
    }
  });

  it("is the disc's light above the datum's horizon and the slope's plane within 1.5e-3 of face-on, over slopes, bearings and horizons", () => {
    // decision-r10-t8b §5 L1: horizons to 60°, slopes to 45°, fall lines towards, across and away
    // from the Sun, the Sun at nine elevations within ±1.5ρ of the horizon.
    let cases = 0;
    let worst = 0;
    for (const horizonDeg of [0.5, 5, 30, 60]) {
      const horizon = horizonDeg * RAD;
      for (const slopeDeg of [0, 20, 45]) {
        for (const bearing of [0, 90, 180]) {
          const n = slopeNormal(slopeDeg, bearing);
          for (let k = -4; k <= 4; k += 1) {
            const star = horizon + ((1.5 * k) / 4) * RHO_SUN;
            const phi = angleFrom(n, starDirection(star));
            // The slope's own plane hides the whole disc: nothing to compare.
            if (Math.PI / 2 - phi <= -RHO_SUN) {
              continue;
            }
            const error = Math.abs(
              terrainHorizonFactor(H_SUN, phi, star, horizon) -
                datumFrameIrradiance(n, star, horizon, RHO_SUN),
            );
            worst = Math.max(worst, error);
            cases += 1;
          }
        }
      }
    }
    expect(cases).toBeGreaterThan(250);
    expect(worst).toBeLessThan(1.5e-3);
  });

  it("is the hard step for a star of 0.1″ and a point star, finite and in [0, 1]", () => {
    // Main's condition on Design note 10's hard step under 20″ (R10's Risks): R07's soft edge
    // converges to it.
    const horizon = 5 * RAD;
    const arcsecond = RAD / 3600;
    // Each case's lit fraction and term over cos φ, and the hard step's value where it is away.
    const fractions: StepCase[] = [];
    const terms: StepCase[] = [];
    for (const rho of [0.1 * arcsecond, 0]) {
      const h = rho > 0 ? 1 / Math.sin(rho) : Number.POSITIVE_INFINITY;
      for (const offset of [-1e-3, -2 * rho, 0, 2 * rho, 1e-3]) {
        const star = horizon + offset;
        const step = Math.abs(offset) > rho ? (offset > 0 ? 1 : 0) : null;
        fractions.push({ value: litFraction(star, horizon, rho), step });
        for (const tilt of [0, 20 * RAD]) {
          const phi = Math.PI / 2 - star - tilt;
          terms.push({ value: terrainHorizonFactor(h, phi, star, horizon) / Math.cos(phi), step });
        }
      }
    }
    expect(outside(fractions, 1)).toEqual([]);
    expect(outside(terms, 1 + 1e-12)).toEqual([]);
    expect(missed(fractions)).toEqual([]);
    expect(missed(terms)).toEqual([]);
    expect(fractions.filter(({ step }) => step !== null).length).toBeGreaterThan(5);
    expect(litFraction(horizon, horizon, 0)).toBe(0.5);
  });

  it("takes a point star's distance at most 10¹⁸ radii, where R07's term alone is NaN", () => {
    const phi = 30 * RAD;
    expect(sphereIrradianceFactor(Number.POSITIVE_INFINITY, phi)).toBeNaN();
    expect(MAX_STAR_DISTANCE_RADII ** 2).toBeLessThan(3.4e38);
    expect(terrainHorizonFactor(Number.POSITIVE_INFINITY, phi, Math.PI / 2 - phi, 0)).toBeCloseTo(
      Math.cos(phi),
      12,
    );
  });

  it("takes the slope's own plane where it hides more than the horizon", () => {
    // A slope tilted 10° away from a Sun 3° up over a horizon of 0: the plane hides it.
    const star = 3 * RAD;
    const phi = Math.PI / 2 - star + 10 * RAD;
    expect(elementHorizonRad(0, star, phi)).toBeLessThan(0);
    expect(terrainHorizonFactor(H_SUN, phi, star, 0)).toBe(sphereIrradianceFactor(H_SUN, phi));
  });
});
