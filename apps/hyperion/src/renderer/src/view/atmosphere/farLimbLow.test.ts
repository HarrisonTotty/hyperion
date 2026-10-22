import { beforeAll, describe, expect, it } from "vitest";

import {
  BODIES,
  brightestOf,
  centreAt,
  convergedFootprints,
  distanceName,
  drawnAt,
  errorOf,
  type FarBody,
  type Figures,
  figuresOfErrors,
  FLOOR,
  footprintMeanOf,
  imageSun,
  LIMB_SET,
  limbPitchRad,
  limbView,
  OUTPUT,
  pixelCuts,
  PLACES,
  recordedFigure,
  rgbOf,
  SUNS,
  type Variant,
  VARIANTS,
} from "../../test/farLimb";
import { planetTwin, type PlanetTwin } from "./tablesCpu";

/**
 * Low's far limb against the converged footprint of each output pixel (plan R08, R08.T8.c,
 * acceptance 5; `decision-r08-t8-leans.md` §6.1): `farLimb.test.ts`'s frames and reference at low's
 * 1280 × 720 and its 16 steps, the drawn value the half-resolution texel the composite picks for the
 * pixel (texel ⌊x ÷ 2⌋ in each axis, since R08.T8.b's tie-break), as the kernel draws it since
 * R08.T8.c: from above the top its footprint's mean, blended in by its span, the texel twice the
 * pixel's footprint. Both parities of pixel are taken (`low even` at (640, 360), `low odd` at
 * (641, 361)): the texel's centre 0.5 px below and right of an even pixel's centre and 0.5 px above
 * and left of an odd one's.
 *
 * @remarks
 * Low's figures are recorded, not held: its half-resolution trade (the texel's mean stands for two
 * output pixels in each axis) is part of each figure. A limb-set pixel drawn as 0 where its converged
 * footprint is above the floor would be a blocker for "main".
 */

const LOW_VARIANTS: readonly Variant[] = ["low even", "low odd"];

const built = new Map<string, PlanetTwin>();

function planetOf(body: FarBody): PlanetTwin {
  const planet = built.get(body.name);
  if (planet === undefined) {
    throw new Error(`${body.name}'s twin is built before the tests read it`);
  }
  return planet;
}

// The three bodies' tables took 13 s at four workers on a loaded machine (2026-10-21): the limit is
// over ten times that.
beforeAll(() => {
  for (const body of BODIES) {
    built.set(body.name, planetTwin(body.medium, body.radiusM, body.curves));
  }
}, 300_000);

/** Figures as recorded, with the pixel where the worst stands. */
const recorded = (figures: Readonly<Record<string, Figures & { at: string }>>): unknown =>
  Object.fromEntries(
    Object.entries(figures).map(([key, f]) => [key, { ...recordedFigure(f), at: f.at }]),
  );

/** A set of figures with the pixel where its worst stands. */
const at = (where: string, f: Figures): Figures & { at: string } => ({ ...f, at: where });

describe("low's far limb against each output pixel's converged footprint (R08.T8.c)", () => {
  /** One pixel's figures: e of the drawn texel and of its centre ray alone, and whether it is 0. */
  interface PixelError {
    readonly name: string;
    readonly e: number;
    readonly eCentre: number;
    /** Whether the drawn texel is 0 in every channel where the footprint's light is over the floor. */
    readonly lost: boolean;
  }
  const results = new Map<string, PixelError[]>();

  // The limit is over ten times the run's time.
  beforeAll(() => {
    for (const { body, distanceM } of PLACES) {
      const planet = planetOf(body);
      for (const variant of LOW_VARIANTS) {
        const { xPx, yPx } = VARIANTS[variant];
        const viewport = OUTPUT.low;
        const errors = SUNS.flatMap(([zenithDeg, azimuthDeg]) => {
          const sun = imageSun(distanceM, body.radiusM, zenithDeg, azimuthDeg);
          const pixels = LIMB_SET.map((n) => {
            const pitch = limbPitchRad(
              distanceM,
              body.radiusM,
              n * body.scaleHeightM,
              viewport,
              xPx,
              yPx,
            );
            const view = limbView(body, distanceM, pitch, viewport, sun);
            const cuts = pixelCuts(body, distanceM, pitch, viewport, xPx, yPx);
            return { n, view, cuts };
          });
          const references = convergedFootprints(
            pixels.map(
              (p) => (rows: number) =>
                footprintMeanOf(planet, p.view, "low", xPx, yPx, p.cuts, rows),
            ),
          );
          const lMax = brightestOf(references.map((r) => rgbOf(r.mean)));
          return pixels.map((p, k) => {
            const reference = rgbOf(references[k]?.mean ?? [0, 0, 0, 0]);
            const drawn = rgbOf(drawnAt(planet, p.view, "low", xPx, yPx));
            return {
              name: `sun ${zenithDeg}°/${azimuthDeg}°, ${p.n} H`,
              e: errorOf(drawn, reference, lMax, FLOOR),
              eCentre: errorOf(
                rgbOf(centreAt(planet, p.view, "low", xPx, yPx)),
                reference,
                lMax,
                FLOOR,
              ),
              lost:
                drawn.every((c) => c === 0) &&
                reference.some((c, i) => c > FLOOR * (lMax[i] ?? Number.NaN)),
            };
          });
        });
        results.set(`${body.name}, ${distanceName(distanceM)}, ${variant}`, errors);
      }
    }
  }, 600_000);

  /** Each place's figures of e, with the pixel where the worst stands. */
  const figuresOf = (of: (p: PixelError) => number): Record<string, Figures & { at: string }> =>
    Object.fromEntries(
      [...results.entries()].map(([key, es]) => {
        const worst = es.reduce((w, p) => (of(p) > of(w) ? p : w));
        return [key, { ...figuresOfErrors(es.map(of)), at: worst.name }];
      }),
    );

  it("draws no limb-set pixel as 0 where its converged footprint is over the floor", () => {
    const lost = [...results.entries()].flatMap(([key, es]) =>
      es.filter((p) => p.lost).map((p) => `${key}, ${p.name}`),
    );
    expect(lost).toEqual([]);
  });

  // Measured 2026-10-22. The texel's footprint is twice the pixel's in each axis, so that a faint
  // pixel beside a bright one takes the bright one's light: from 10⁸ and 10⁹ m the worst pixels are
  // the 10 H rows of the even pixel, whose texel takes in the bright lower limb a pixel under them
  // (89–115 times their own light), and from 10⁷ m the odd pixel's 0.1 H row, its texel a pixel up
  // the limb (16–22 times). That is low's half-resolution trade, the ring drawn soft over its texels'
  // two rows. The medians move from the centre ray's 1 (nothing drawn) from 10⁸ and 10⁹ m to about
  // 0.5, and no pixel is lost.
  it("records the worst and median e and the count over 5% per body, distance and parity", () => {
    expect(figuresOf((p) => p.e)).toEqual(
      recorded({
        "R05's Earth, 10⁷ m, low even": at("sun 100°/0°, 10 H", {
          worst: 0.54667,
          median: 0.20111,
          over: 28,
        }),
        "R05's Earth, 10⁷ m, low odd": at("sun 100°/0°, 0.1 H", {
          worst: 16.25585,
          median: 0.19908,
          over: 30,
        }),
        "R05's Earth, 10⁸ m, low even": at("sun 30°/0°, 10 H", {
          worst: 89.40122,
          median: 0.49787,
          over: 35,
        }),
        "R05's Earth, 10⁸ m, low odd": at("sun 90°/0°, 3 H", {
          worst: 0.54542,
          median: 0.49364,
          over: 35,
        }),
        "R05's Earth, 10⁹ m, low even": at("sun 100°/180°, 10 H", {
          worst: 1.53761,
          median: 0.34277,
          over: 31,
        }),
        "R05's Earth, 10⁹ m, low odd": at("sun 90°/0°, 10 H", {
          worst: 0.56268,
          median: 0.43049,
          over: 35,
        }),
        "Saturn-class, 10⁸ m, low even": at("sun 90°/0°, 10 H", {
          worst: 1.57729,
          median: 0.46281,
          over: 23,
        }),
        "Saturn-class, 10⁸ m, low odd": at("sun 100°/0°, 0.3 H", {
          worst: 27.52541,
          median: 0.25372,
          over: 32,
        }),
        "Saturn-class, 10⁹ m, low even": at("sun 100°/180°, 10 H", {
          worst: 109.33231,
          median: 0.49795,
          over: 35,
        }),
        "Saturn-class, 10⁹ m, low odd": at("sun 90°/0°, 10 H", {
          worst: 0.52066,
          median: 0.49387,
          over: 35,
        }),
        "the measured Earth, 10⁷ m, low even": at("sun 100°/0°, 10 H", {
          worst: 0.71344,
          median: 0.18414,
          over: 22,
        }),
        "the measured Earth, 10⁷ m, low odd": at("sun 100°/0°, 0.1 H", {
          worst: 21.8283,
          median: 0.2177,
          over: 30,
        }),
        "the measured Earth, 10⁸ m, low even": at("sun 100°/180°, 10 H", {
          worst: 115.40271,
          median: 0.50342,
          over: 35,
        }),
        "the measured Earth, 10⁸ m, low odd": at("sun 90°/0°, 3 H", {
          worst: 0.53795,
          median: 0.49221,
          over: 35,
        }),
        "the measured Earth, 10⁹ m, low even": at("sun 100°/180°, 10 H", {
          worst: 1.794,
          median: 0.47379,
          over: 31,
        }),
        "the measured Earth, 10⁹ m, low odd": at("sun 100°/0°, 10 H", {
          worst: 0.57665,
          median: 0.4247,
          over: 35,
        }),
      }),
    );
  });

  // Measured 2026-10-22: T8.b's figures against the converged footprint (it recorded them against
  // its 8 × 8 mean).
  it("records the same of the picked texel's centre ray alone, the march as T8.b drew it", () => {
    expect(figuresOf((p) => p.eCentre)).toEqual(
      recorded({
        "R05's Earth, 10⁷ m, low even": at("sun 100°/0°, 0.3 H", {
          worst: 0.9966,
          median: 0.22267,
          over: 30,
        }),
        "R05's Earth, 10⁷ m, low odd": at("sun 100°/0°, 0.1 H", {
          worst: 6.57318,
          median: 0.21334,
          over: 30,
        }),
        "R05's Earth, 10⁸ m, low even": at("sun 30°/90°, 10 H", {
          worst: 9.20206,
          median: 1,
          over: 35,
        }),
        "R05's Earth, 10⁸ m, low odd": at("sun 100°/0°, 0.1 H", {
          worst: 1.72544,
          median: 0.97832,
          over: 35,
        }),
        "R05's Earth, 10⁹ m, low even": at("sun 100°/180°, 10 H", {
          worst: 2.58696,
          median: 0.96403,
          over: 35,
        }),
        "R05's Earth, 10⁹ m, low odd": at("sun 30°/0°, 0.1 H", { worst: 1, median: 1, over: 35 }),
        "Saturn-class, 10⁸ m, low even": at("sun 100°/0°, 1 H", {
          worst: 0.99195,
          median: 0.42477,
          over: 24,
        }),
        "Saturn-class, 10⁸ m, low odd": at("sun 100°/0°, 0.3 H", {
          worst: 5.9191,
          median: 0.32179,
          over: 26,
        }),
        "Saturn-class, 10⁹ m, low even": at("sun 100°/180°, 10 H", {
          worst: 7.59572,
          median: 0.99917,
          over: 35,
        }),
        "Saturn-class, 10⁹ m, low odd": at("sun 30°/0°, 10 H", {
          worst: 1,
          median: 0.99796,
          over: 35,
        }),
        "the measured Earth, 10⁷ m, low even": at("sun 100°/0°, 0.3 H", {
          worst: 0.99708,
          median: 0.2225,
          over: 23,
        }),
        "the measured Earth, 10⁷ m, low odd": at("sun 100°/0°, 0.1 H", {
          worst: 7.23475,
          median: 0.25406,
          over: 24,
        }),
        "the measured Earth, 10⁸ m, low even": at("sun 90°/180°, 10 H", {
          worst: 11.85408,
          median: 1,
          over: 35,
        }),
        "the measured Earth, 10⁸ m, low odd": at("sun 90°/0°, 10 H", {
          worst: 1,
          median: 0.98702,
          over: 35,
        }),
        "the measured Earth, 10⁹ m, low even": at("sun 100°/180°, 10 H", {
          worst: 2.69394,
          median: 1,
          over: 35,
        }),
        "the measured Earth, 10⁹ m, low odd": at("sun 30°/0°, 0.1 H", {
          worst: 1,
          median: 1,
          over: 35,
        }),
      }),
    );
  });
});
