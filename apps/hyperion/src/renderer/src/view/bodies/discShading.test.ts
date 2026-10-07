import { describe, expect, it } from "vitest";

import { add, scale, vec3 } from "../../geometry/vec3";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { pixelSolidAngle, type ProjectionCamera, type Viewport } from "../camera/projection";
import type { PlacedLight } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import type { Rgb } from "../photometry/toneCurve";
import { AU_M } from "../scenes/kept";
import type { BodyFigure } from "../terrain/planet";
import {
  compositeDiscPixels,
  type DiscRecord,
  type DiscSamples,
  discSamples,
  FINE_DISC_PX,
  pointSampleDisc,
  rasteriseDisc,
  viewRay,
} from "./discShading";
import { type LitBodyInput, planLitBodies } from "./draw";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const RAD = Math.PI / 180;

/** The centre pixel's scale, px per radian. */
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));

const RADIUS_M = 6.371e6;

const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  exposureScale: 1e-3,
  annuli: 4,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
};

/** The figures the ruling's gates take (decision-r07-small-disc-cost §2). */
const FIGURES: ReadonlyArray<readonly [string, BodyFigure]> = [
  ["a sphere", { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: vec3(0, 1, 0) }],
  [
    "an f = 0.098 spheroid equator-on",
    { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M * (1 - 0.098), pole: vec3(0, 1, 0) },
  ],
  [
    "an f = 0.098 spheroid seen from 45° latitude",
    {
      equatorialRadiusM: RADIUS_M,
      polarRadiusM: RADIUS_M * (1 - 0.098),
      pole: vec3(0, Math.SQRT1_2, Math.SQRT1_2),
    },
  ],
];

const SPHERE: BodyFigure = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: null };

/**
 * The disc record of a body `px` across at the centre pixel's scale, its centre moved `offsetPx`
 * from the view's centre, lit by a Sun 1 au away at phase `phaseDeg`, as `planLitBodies` plans it.
 */
function recordAt(
  px: number,
  phaseDeg: number,
  figure: BodyFigure,
  offsetPx: readonly [number, number] = [0, 0],
): DiscRecord {
  const distanceM = figure.equatorialRadiusM / Math.sin(px / 2 / PX_PER_RAD);
  const centreM = vec3(
    (offsetPx[0] * distanceM) / PX_PER_RAD,
    (offsetPx[1] * distanceM) / PX_PER_RAD,
    -distanceM,
  );
  const towardsStar = vec3(Math.sin(phaseDeg * RAD), 0, Math.cos(phaseDeg * RAD));
  const lit = aLitBody();
  const body: LitBodyInput = {
    id: lit.body,
    centreM,
    figure,
    photometry: lit.photometry,
    lighting: undefined,
  };
  const hosts: PlacedLight[] = [
    { disc: aHostDisc(), centreM: add(centreM, scale(towardsStar, AU_M)) },
  ];
  const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
  const record = plan.discs.find((each) => each.body === body.id);
  if (record === undefined) {
    throw new Error(`a body ${String(px)} px across was not drawn as a disc`);
  }
  return record;
}

/** The record with other cells per axis in its interior and limb pixels. */
function withSamples(record: DiscRecord, samples: DiscSamples): DiscRecord {
  return { ...record, interiorSamples: samples.interior, limbSamples: samples.limb };
}

/** The disc's flux at the camera per channel, in the twin's units (pre-exposed light × sr). */
function fluxOf(record: DiscRecord): Rgb {
  const flux: [number, number, number] = [0, 0, 0];
  for (const pixel of compositeDiscPixels(rasteriseDisc(record, CAMERA, VIEWPORT))) {
    const ray = viewRay(pixel.xPx + 0.5, pixel.yPx + 0.5, CAMERA, VIEWPORT);
    const omega = pixelSolidAngle(ray, CAMERA, VIEWPORT);
    for (const c of [0, 1, 2] as const) {
      flux[c] += pixel.rgb[c] * omega;
    }
  }
  return flux;
}

/** The 16 placements of the gates: centres on a 4 × 4 sub-pixel grid, px. */
const PLACEMENTS: ReadonlyArray<readonly [number, number]> = Array.from(
  { length: 16 },
  (_, k) => [Math.floor(k / 4) / 4, (k % 4) / 4] as const,
);

/** One pixel's light and covered share, whichever way it was drawn. */
interface PixelValue {
  readonly rgb: Rgb;
  readonly coverage: number;
}

/** How one drawing's pixels depart from another's (decision-r07-small-disc-cost, G5). */
interface PixelErrors {
  /** The worst |drawn − reference| over the pixels and channels, as a share of the channel's peak. */
  readonly worst: number;
  /** The RMS of that share over the pixels either drawing covers, the worst channel's. */
  readonly rms: number;
  /** The worst |drawn − reference| covered share over the pixels either covers only in part. */
  readonly coverage: number;
}

function pixelMap(
  pixels: ReadonlyArray<PixelValue & { readonly xPx: number; readonly yPx: number }>,
): Map<string, PixelValue> {
  return new Map(pixels.map((p) => [`${String(p.xPx)},${String(p.yPx)}`, p]));
}

const DARK: PixelValue = { rgb: [0, 0, 0], coverage: 0 };

/** The errors of `drawn` against `reference`, each channel's against the reference's peak in it. */
function pixelErrors(
  drawn: ReadonlyMap<string, PixelValue>,
  reference: ReadonlyMap<string, PixelValue>,
): PixelErrors {
  const keys = new Set([...reference.keys(), ...drawn.keys()]);
  let worst = 0;
  let rms = 0;
  let coverage = 0;
  for (const c of [0, 1, 2] as const) {
    const peak = Math.max(...[...reference.values()].map((p) => p.rgb[c]));
    let sumSq = 0;
    for (const key of keys) {
      const r = reference.get(key) ?? DARK;
      const g = drawn.get(key) ?? DARK;
      const share = Math.abs(g.rgb[c] - r.rgb[c]) / peak;
      worst = Math.max(worst, share);
      sumSq += share * share;
      if (r.coverage < 1 || g.coverage < 1) {
        coverage = Math.max(coverage, Math.abs(g.coverage - r.coverage));
      }
    }
    rms = Math.max(rms, Math.sqrt(sumSq / keys.size));
  }
  return { worst, rms, coverage };
}

/** The twin's composite of a record as a map from pixel to its value. */
function drawnPixels(record: DiscRecord): Map<string, PixelValue> {
  return pixelMap(compositeDiscPixels(rasteriseDisc(record, CAMERA, VIEWPORT)));
}

/**
 * The reference twin (G5, G5′): the same record at 32 × 32 cells in every pixel, and at 64 × 64
 * under {@link FINE_DISC_PX}. A 3 px crescent at 150° is 0.2 px deep, more than 32 × 32's
 * near-limb band (about 0.08 px) reaches: its terminator falls in centre-sampled cells, and the
 * 32 × 32 twin stands 0.32% of the peak from a converged brute force there, the 64 × 64 0.11%.
 */
function referenceSamples(px: number): DiscSamples {
  const cells = px < FINE_DISC_PX ? 64 : 32;
  return { interior: cells, limb: cells };
}

describe("the cells a disc takes by its size (T8.c)", () => {
  for (const [diameterPx, samples] of [
    [3.99, { interior: 8, limb: 8 }],
    [4, { interior: 4, limb: 4 }],
    [31.99, { interior: 4, limb: 4 }],
    [32, { interior: 1, limb: 4 }],
    [Number.POSITIVE_INFINITY, { interior: 1, limb: 4 }],
  ] as const) {
    it(`takes ${String(samples.interior)} inside and ${String(samples.limb)} on the limb per axis at ${String(diameterPx)} px`, () => {
      expect(discSamples(diameterPx)).toEqual(samples);
    });
  }
});

describe("the flux steps where the count changes (T8.c, G4)", () => {
  // At 4 px, 8 × 8 against 4 × 4; at 31.9 px, 4 × 4 against one inside and 4 × 4 on the limb.
  const steps = [
    [4, { interior: 8, limb: 8 }, { interior: 4, limb: 4 }],
    [31.9, { interior: 4, limb: 4 }, { interior: 1, limb: 4 }],
  ] as const;
  for (const [px, below, above] of steps) {
    for (const [name, figure] of FIGURES) {
      for (const phaseDeg of [0, 90, 120, 150]) {
        it(`changes the flux by at most 0.5% at ${String(px)} px for ${name} at ${String(phaseDeg)}°`, () => {
          let worst = 0;
          for (const offset of PLACEMENTS) {
            const record = recordAt(px, phaseDeg, figure, offset);
            const before = fluxOf(withSamples(record, below));
            const after = fluxOf(withSamples(record, above));
            for (const c of [0, 1, 2] as const) {
              worst = Math.max(worst, Math.abs(after[c] / before[c] - 1));
            }
          }
          expect(worst).toBeLessThan(0.005);
        });
      }
    }
  }
});

describe("each pixel of a small disc against the reference twin (T8.c, G5)", () => {
  // One placement off the sub-pixel grid's centre; the disc drawn with the counts it is planned
  // with. 4.01 px stands for the ruling's 4 px, which a placement off the centre, a little farther
  // off, takes just under 4 px and back to 8 × 8.
  const offset = [0.25, 0.75] as const;
  for (const px of [3, 4.01, 6, 12, 24]) {
    for (const phaseDeg of [0, 90, 150]) {
      it(`keeps every pixel within 1.5% of the peak, the RMS within 0.3% and the coverage within 0.0025 at ${String(px)} px and ${String(phaseDeg)}°`, () => {
        const record = recordAt(px, phaseDeg, SPHERE, offset);
        const errors = pixelErrors(
          drawnPixels(record),
          drawnPixels(withSamples(record, referenceSamples(px))),
        );
        expect(errors.worst, "the worst pixel").toBeLessThan(0.015);
        expect(errors.rms, "the RMS").toBeLessThan(0.003);
        expect(errors.coverage, "the coverage").toBeLessThan(0.0025);
      });
    }
  }

  it("draws each size at the counts the ruling sets", () => {
    const counts = [3, 4.01, 6, 12, 24].map((px) => {
      const record = recordAt(px, 0, SPHERE, offset);
      return [record.interiorSamples, record.limbSamples];
    });
    expect(counts).toEqual([
      [8, 8],
      [4, 4],
      [4, 4],
      [4, 4],
      [4, 4],
    ]);
  });
});

describe("the reference twin against brute force (T8.c, G5′)", () => {
  it("draws a 4 px crescent at 150° within 0.3% of its peak of 256 × 256 point samples in each pixel", () => {
    const record = withSamples(recordAt(4, 150, SPHERE, [0.25, 0.75]), referenceSamples(4));
    const brute = pixelMap(pointSampleDisc(record, CAMERA, VIEWPORT, 256));
    expect(pixelErrors(drawnPixels(record), brute).worst).toBeLessThan(0.003);
  });

  // At 3 px the brute force takes 512 × 512 samples a pixel: 256 × 256 is itself 0.20% of the
  // peak from 1,024 × 1,024 on the 0.2 px crescent, 512 × 512 0.017%. It takes several seconds.
  it("draws a 3 px crescent at 150° within 0.3% of its peak of 512 × 512 point samples in each pixel", () => {
    const record = withSamples(recordAt(3, 150, SPHERE, [0.25, 0.75]), referenceSamples(3));
    const brute = pixelMap(pointSampleDisc(record, CAMERA, VIEWPORT, 512));
    expect(pixelErrors(drawnPixels(record), brute).worst).toBeLessThan(0.003);
  }, 60_000);
});

describe("the brute force", () => {
  it("covers a pixel at the disc's centre wholly", () => {
    const pixels = pixelMap(pointSampleDisc(recordAt(20, 0, SPHERE), CAMERA, VIEWPORT, 4));
    const centre = `${String(VIEWPORT.widthPx / 2)},${String(VIEWPORT.heightPx / 2)}`;
    expect(pixels.get(centre)?.coverage).toBe(1);
  });

  it("leaves out a corner of the disc's rectangle, which no sample meets", () => {
    const record = recordAt(20, 0, SPHERE);
    const pixels = pixelMap(pointSampleDisc(record, CAMERA, VIEWPORT, 4));
    const corner = `${String(Math.floor(record.rect.leftPx))},${String(Math.floor(record.rect.topPx))}`;
    expect(pixels.has(corner)).toBe(false);
  });

  it("refuses a sample count that is not a positive whole number", () => {
    const record = recordAt(4, 0, SPHERE);
    expect(() => pointSampleDisc(record, CAMERA, VIEWPORT, 0)).toThrow(/positive whole number/u);
  });
});
