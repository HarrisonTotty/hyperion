import { beforeAll, describe, expect, it } from "vitest";

import {
  asRecorded,
  BODIES,
  BOUND,
  centreAt,
  CHANNELS,
  distanceName,
  drawnAt,
  type FarBody,
  imageSun,
  limbPitchRad,
  limbView,
  ON_THE_LIMB,
  OUTPUT,
  pixelTangentM,
  SUNS,
  VARIANTS,
} from "../../test/farLimb";
import type { QualitySetting } from "../quality/qualitySetting";
import { TABLE_SIZES } from "./hillaire";
import { planetTwin, type PlanetTwin } from "./tablesCpu";

/**
 * No flicker at the far limb (plan R08, R08.T8.c, acceptance 3; `decision-r08-t8-leans.md` §6.1):
 * the limb swept across one texel in eight steps of pitch from 10⁸ and 10⁹ m, on high and low, each
 * column's summed radiance over the limb against its mean over the eight steps. With one ray a
 * texel the limb broke into specks and gaps that flickered as the body moved by a fraction of a
 * pixel; with the footprint's mean blended in by the texel's span the light across a column is
 * conserved. On high each column's sum stays within 5% of its mean for every sun but 90°/0°; on low
 * it never falls below half its mean, so that the limb is always drawn; low's figures are recorded.
 *
 * @remarks
 * `farLimb.test.ts`'s frames: the camera on +x looking past the limb, the limb level across the
 * frame at its middle column, the frame's middle pixel's centre on the 1 H ray at the sweep's first
 * step, then pitched by an eighth of a texel's angular height at a step. Three columns: the middle,
 * and a third and two thirds of the body's apparent radius to its right, where the limb is lower in
 * the frame and tilted across the columns (to about 48° from 10⁹ m), so that the footprint's
 * trapezoid is taken. A column's window runs from two texels above the top's row down to a floor
 * fixed on the body, the tangent depth three texels into the face at the first step, each row taken
 * by its share above the floor; a floor at a fixed row cuts the face's haze at a depth that moves
 * with the limb, which moved the sums from 10⁸ m by 6% (the converged footprints' sums alike). The
 * sum is of the air's light alone (the body's ground black): the face's lit ground would otherwise
 * outweigh the limb many times.
 */

const STEPS = 8;
const DISTANCES_M = [1e8, 1e9] as const;
const SETTINGS: readonly QualitySetting[] = ["high", "low"];

/** Low's least sum over the sweep, as a share of its mean (§6.1: the limb always drawn). */
const LOW_FLOOR = 0.5;

/** The air twins: each body's twin with its ground black. */
const air = new Map<string, PlanetTwin>();

function airOf(body: FarBody): PlanetTwin {
  const planet = air.get(body.name);
  if (planet === undefined) {
    throw new Error(`${body.name}'s twin is built before the tests read it`);
  }
  return planet;
}

// The three bodies' tables took 13 s at four workers on a loaded machine (2026-10-21): the limit is
// over ten times that.
beforeAll(() => {
  for (const body of BODIES) {
    const planet = planetTwin(body.medium, body.radiusM, body.curves);
    air.set(body.name, { ...planet, medium: { ...planet.medium, groundAlbedo: [0, 0, 0] } });
  }
}, 300_000);

/** A column's sums over the sweep, drawn and by the centre ray alone, per channel. */
interface ColumnSweep {
  readonly name: string;
  readonly drawn: number[][];
  readonly centre: number[][];
}

/** The largest |S_k ÷ S̄ − 1| over the steps and channels, and the least S_k ÷ S̄. */
function spreadOf(sums: readonly number[][]): { readonly worst: number; readonly least: number } {
  let worst = 0;
  let least = Number.POSITIVE_INFINITY;
  for (const c of CHANNELS) {
    const values = sums.map((s) => s[c] ?? Number.NaN);
    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    for (const v of values) {
      worst = Math.max(worst, Math.abs(v / mean - 1));
      least = Math.min(least, v / mean);
    }
  }
  return { worst, least };
}

describe("the far limb swept across a texel (R08.T8.c)", () => {
  const sweeps = new Map<string, ColumnSweep[]>();

  // The limit is over ten times the run's time.
  beforeAll(() => {
    for (const body of BODIES) {
      const planet = airOf(body);
      for (const distanceM of DISTANCES_M.filter((d) => body.distancesM.includes(d))) {
        for (const setting of SETTINGS) {
          const variant = setting === "high" ? VARIANTS.high : VARIANTS["low even"];
          const viewport = OUTPUT[setting];
          const texelPx = 1 / TABLE_SIZES[setting].rayMarchScale;
          const base = limbPitchRad(
            distanceM,
            body.radiusM,
            body.scaleHeightM,
            viewport,
            variant.xPx,
            variant.yPx,
          );
          // A texel's angular height at the frame's middle: the pitch that moves the 1 H ray down a
          // texel.
          const texelRad =
            limbPitchRad(
              distanceM,
              body.radiusM,
              body.scaleHeightM,
              viewport,
              variant.xPx,
              variant.yPx - texelPx,
            ) - base;
          const pixelRad = texelRad / texelPx;
          const radiusPx = Math.asin(body.radiusM / distanceM) / Math.abs(pixelRad);
          const columns = [0, 1, 2].map((j) => variant.xPx + Math.round((j * radiusPx) / 3));
          const result = SUNS.flatMap(([zenithDeg, azimuthDeg]) => {
            const sun = imageSun(distanceM, body.radiusM, zenithDeg, azimuthDeg);
            return columns.map((x) => {
              // The window at the first step: the rows whose centres' rays meet the top and the
              // datum's edge, two texels over and three under.
              const tangentAt = (y: number): number =>
                pixelTangentM(distanceM, body.radiusM, base, viewport, x + 0.5, y + 0.5);
              let topRow = 0;
              while (topRow < viewport.heightPx - 1 && tangentAt(topRow) > body.medium.topHeightM) {
                topRow += 1;
              }
              let edgeRow = topRow;
              while (edgeRow < viewport.heightPx - 1 && tangentAt(edgeRow) > 0) {
                edgeRow += 1;
              }
              const first = Math.max(topRow - 2 * texelPx, 0);
              const last = Math.min(edgeRow + 4 * texelPx, viewport.heightPx - 1);
              // The window's floor, fixed on the body: the tangent depth into the face of the bottom
              // edge of the row three texels under the datum's edge at the first step. Each row is
              // taken by its share above the floor, so that the window moves with the body and its
              // floor cuts the face's haze at the same depth at every step.
              const floorM = pixelTangentM(
                distanceM,
                body.radiusM,
                base,
                viewport,
                x + 0.5,
                last + 1 - texelPx,
              );
              const drawn: number[][] = [];
              const centre: number[][] = [];
              for (let k = 0; k < STEPS; k += 1) {
                const pitch = base + (k / STEPS) * texelRad;
                const view = limbView(body, distanceM, pitch, viewport, sun);
                const sum = [0, 0, 0];
                const sumCentre = [0, 0, 0];
                for (let y = first; y <= last; y += 1) {
                  const above = pixelTangentM(distanceM, body.radiusM, pitch, viewport, x + 0.5, y);
                  const below = pixelTangentM(
                    distanceM,
                    body.radiusM,
                    pitch,
                    viewport,
                    x + 0.5,
                    y + 1,
                  );
                  const share = Math.min(Math.max((above - floorM) / (above - below), 0), 1);
                  if (!(share > 0)) {
                    continue;
                  }
                  const texel = drawnAt(planet, view, setting, x, y);
                  const ray = centreAt(planet, view, setting, x, y);
                  for (const c of CHANNELS) {
                    sum[c] = (sum[c] ?? Number.NaN) + share * texel[c];
                    sumCentre[c] = (sumCentre[c] ?? Number.NaN) + share * ray[c];
                  }
                }
                drawn.push(sum);
                centre.push(sumCentre);
              }
              return {
                name: `sun ${zenithDeg}°/${azimuthDeg}°, column +${x - variant.xPx}`,
                drawn,
                centre,
              };
            });
          });
          sweeps.set(`${body.name}, ${distanceName(distanceM)}, ${setting}`, result);
        }
      }
    }
  }, 600_000);

  it("holds each column's summed light on high within 5% of its mean, but with the Sun on the limb", () => {
    const over = [...sweeps.entries()]
      .filter(([key]) => key.endsWith("high"))
      .flatMap(([key, columns]) =>
        columns
          .filter((column) => !column.name.startsWith(ON_THE_LIMB))
          .map((column) => {
            const { worst, least } = spreadOf(column.drawn);
            return { name: `${key}, ${column.name}`, worst, least };
          })
          .filter((spread) => !(spread.worst <= BOUND))
          .map((spread) => `${spread.name}: ${(100 * spread.worst).toFixed(2)}%`),
      );
    expect(over).toEqual([]);
  });

  it("never draws a column's light on low under half its mean", () => {
    const under = [...sweeps.entries()]
      .filter(([key]) => key.endsWith("low"))
      .flatMap(([key, columns]) =>
        columns
          .map((column) => {
            const { worst, least } = spreadOf(column.drawn);
            return { name: `${key}, ${column.name}`, worst, least };
          })
          .filter((spread) => !(spread.least >= LOW_FLOOR))
          .map((spread) => `${spread.name}: ${(100 * spread.least).toFixed(2)}%`),
      );
    expect(under).toEqual([]);
  });

  // The worst spread over each place's columns and suns, drawn and of the centre ray alone, the
  // Sun on the limb apart. Measured 2026-10-22: on high the drawn sums hold within 0.75–3.5% of
  // their means (5.8% at worst with the Sun on the limb), where the centre ray alone swings by up to
  // 7 times (all of a column's light on one step of eight, none on another) and falls to 0; on low
  // the least share drawn is 0.66 (the measured Earth from 10⁹ m, a half-resolution texel 107 H
  // tall), where the centre ray's falls to 0. The Saturn-class figure from 10⁸ m (s 0.96, w_L 0.91)
  // holds within 0.8% with the centre ray alone too; since R08.T8.e's count on the bare ground (its
  // face in the footprint, high) 0.76% drawn and 0.81% centre, from 0.75% and 0.79% (2026-10-22).
  // Its thin sky at 28 steps (2026-10-23) moves the drawn worst on high from 10⁸ m to 1.35% on
  // R05's Earth and 2.39% on the measured one (1.12% and 1.99%), and from 10⁹ m on the
  // Saturn-class figure to 2.31% (2.48%).
  it("records each place's worst spread and least share, drawn and of the centre ray alone", () => {
    const record = Object.fromEntries(
      [...sweeps.entries()].flatMap(([key, columns]) => {
        const held = columns.filter((column) => !column.name.startsWith(ON_THE_LIMB));
        const onLimb = columns.filter((column) => column.name.startsWith(ON_THE_LIMB));
        const worst = (of: (c: ColumnSweep) => number[][], set: readonly ColumnSweep[]): number =>
          Math.max(...set.map((c) => spreadOf(of(c)).worst));
        const least = (of: (c: ColumnSweep) => number[][], set: readonly ColumnSweep[]): number =>
          Math.min(...set.map((c) => spreadOf(of(c)).least));
        return [
          [`${key}, drawn, worst`, worst((c) => c.drawn, held)],
          [`${key}, drawn, least`, least((c) => c.drawn, held)],
          [`${key}, drawn, worst on the limb`, worst((c) => c.drawn, onLimb)],
          [`${key}, centre, worst`, worst((c) => c.centre, held)],
          [`${key}, centre, least`, least((c) => c.centre, held)],
        ];
      }),
    );
    expect(record).toEqual(
      asRecorded({
        "R05's Earth, 10⁸ m, high, centre, least": 0.19678,
        "R05's Earth, 10⁸ m, high, centre, worst": 0.85497,
        "R05's Earth, 10⁸ m, high, drawn, least": 0.98851,
        "R05's Earth, 10⁸ m, high, drawn, worst": 0.01349,
        "R05's Earth, 10⁸ m, high, drawn, worst on the limb": 0.0546,
        "R05's Earth, 10⁸ m, low, centre, least": 0,
        "R05's Earth, 10⁸ m, low, centre, worst": 4.60316,
        "R05's Earth, 10⁸ m, low, drawn, least": 0.97763,
        "R05's Earth, 10⁸ m, low, drawn, worst": 0.03101,
        "R05's Earth, 10⁸ m, low, drawn, worst on the limb": 0.07824,
        "R05's Earth, 10⁹ m, high, centre, least": 0,
        "R05's Earth, 10⁹ m, high, centre, worst": 6.99621,
        "R05's Earth, 10⁹ m, high, drawn, least": 0.96705,
        "R05's Earth, 10⁹ m, high, drawn, worst": 0.03295,
        "R05's Earth, 10⁹ m, high, drawn, worst on the limb": 0.05782,
        "R05's Earth, 10⁹ m, low, centre, least": 0,
        "R05's Earth, 10⁹ m, low, centre, worst": 7,
        "R05's Earth, 10⁹ m, low, drawn, least": 0.79238,
        "R05's Earth, 10⁹ m, low, drawn, worst": 0.33403,
        "R05's Earth, 10⁹ m, low, drawn, worst on the limb": 0.11907,
        "Saturn-class, 10⁸ m, high, centre, least": 0.99195,
        "Saturn-class, 10⁸ m, high, centre, worst": 0.00805,
        "Saturn-class, 10⁸ m, high, drawn, least": 0.99239,
        "Saturn-class, 10⁸ m, high, drawn, worst": 0.00761,
        "Saturn-class, 10⁸ m, high, drawn, worst on the limb": 0.00034,
        "Saturn-class, 10⁸ m, low, centre, least": 0.80898,
        "Saturn-class, 10⁸ m, low, centre, worst": 0.19592,
        "Saturn-class, 10⁸ m, low, drawn, least": 0.97444,
        "Saturn-class, 10⁸ m, low, drawn, worst": 0.02556,
        "Saturn-class, 10⁸ m, low, drawn, worst on the limb": 0.00505,
        "Saturn-class, 10⁹ m, high, centre, least": 0.02369,
        "Saturn-class, 10⁹ m, high, centre, worst": 2.0617,
        "Saturn-class, 10⁹ m, high, drawn, least": 0.97685,
        "Saturn-class, 10⁹ m, high, drawn, worst": 0.02315,
        "Saturn-class, 10⁹ m, high, drawn, worst on the limb": 0.09196,
        "Saturn-class, 10⁹ m, low, centre, least": 0,
        "Saturn-class, 10⁹ m, low, centre, worst": 6.31948,
        "Saturn-class, 10⁹ m, low, drawn, least": 0.97631,
        "Saturn-class, 10⁹ m, low, drawn, worst": 0.02731,
        "Saturn-class, 10⁹ m, low, drawn, worst on the limb": 0.09713,
        "the measured Earth, 10⁸ m, high, centre, least": 0.10438,
        "the measured Earth, 10⁸ m, high, centre, worst": 1.34023,
        "the measured Earth, 10⁸ m, high, drawn, least": 0.97606,
        "the measured Earth, 10⁸ m, high, drawn, worst": 0.02394,
        "the measured Earth, 10⁸ m, high, drawn, worst on the limb": 0.03496,
        "the measured Earth, 10⁸ m, low, centre, least": 0,
        "the measured Earth, 10⁸ m, low, centre, worst": 5.78242,
        "the measured Earth, 10⁸ m, low, drawn, least": 0.97263,
        "the measured Earth, 10⁸ m, low, drawn, worst": 0.03249,
        "the measured Earth, 10⁸ m, low, drawn, worst on the limb": 0.05533,
        "the measured Earth, 10⁹ m, high, centre, least": 0,
        "the measured Earth, 10⁹ m, high, centre, worst": 6.99743,
        "the measured Earth, 10⁹ m, high, drawn, least": 0.96523,
        "the measured Earth, 10⁹ m, high, drawn, worst": 0.03477,
        "the measured Earth, 10⁹ m, high, drawn, worst on the limb": 0.05301,
        "the measured Earth, 10⁹ m, low, centre, least": 0,
        "the measured Earth, 10⁹ m, low, centre, worst": 7,
        "the measured Earth, 10⁹ m, low, drawn, least": 0.66305,
        "the measured Earth, 10⁹ m, low, drawn, worst": 0.75685,
        "the measured Earth, 10⁹ m, low, drawn, worst on the limb": 0.13972,
      }),
    );
  });
});
