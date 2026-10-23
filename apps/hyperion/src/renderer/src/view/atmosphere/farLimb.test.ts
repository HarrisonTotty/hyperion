import { beforeAll, describe, expect, it } from "vitest";

import { cross } from "../../geometry/vec3";
import { CONVERGED, REFERENCE_STEPS } from "../../test/atmosphereGate";
import {
  asRecorded,
  BODIES,
  BOUND,
  brightestOf,
  centreAt,
  CONVERGENCE_FLOOR,
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
  MAX_REFERENCE_ROWS,
  ON_THE_LIMB,
  OUTPUT,
  pickedTexel,
  pixelCuts,
  pixelTangentM,
  PLACES,
  recordedFigure,
  recordedFigures,
  rgbOf,
  SUNS,
  VARIANT_NAMES,
  VARIANTS,
} from "../../test/farLimb";
import type { Rgb } from "../photometry/toneCurve";
import { TABLE_SIZES } from "./hillaire";
import farLimbWgsl from "./shaders/farLimb.wgsl?raw";
import {
  compositeFootprintTransmittanceTwin,
  compositeTransmittanceTwin,
  FAR_LIMB_BLEND_FROM,
  FAR_LIMB_BLEND_WIDTH,
  FAR_LIMB_GAUSS_LEGENDRE,
  farLimbBlend,
  farLimbFootprintTwin,
  farLimbShellsOf,
  planetTwin,
  type PlanetTwin,
  type RayMarchRay,
  rayMarchRayTwin,
  rayMarchTargetTexelTwin,
  rayMarchTexelTwin,
  rayMarchTwin,
  tableSources,
  type TwinView,
  unitRayTwin,
} from "./tablesCpu";

/**
 * The far limb's fidelity on high under Design note 10's 5% (plan R08, R08.T8.b and R08.T8.c;
 * `decision-r08-t8-leans.md` §6.1): per distance, the march's limb pixels at the output resolution
 * against the footprint mean converged across the limb, on Design note 10's limb set (0.1, 0.3, 1, 3
 * and 10 scale heights), from 10⁷, 10⁸ and 10⁹ m from the body's centre and, on R05's Earth, at the
 * blend's upper edge (s = 1). The frames and the reference are `test/farLimb.ts`'s; low's figures
 * are `farLimbLow.test.ts`'s and the sweep across a texel `farLimbFlicker.test.ts`'s.
 *
 * @remarks
 * Each image is one distance, one sun and one setting: five frames from one camera position, each
 * pitched so that one pixel's centre lies on one ray of the limb set, at a 60° field across
 * (`DEFAULT_FOV_DEG`) on high's 1920 × 1080 (the 1080p target). Its sun is set at its zenith angle
 * and azimuth at the tangent point of the ray from the camera that grazes the ground, as the gate
 * sets a limb ray's: 30° at 0°, 90° and 180°, and 90° and 100° at 0° (beyond the limb) and 180°
 * (behind the camera). At 90°/0° the Sun's centre is on the limb, which Design note 10's 3° keeps out
 * of its set; it is recorded, not held. The metric is Design note 10's: e = max over channels
 * |L − M| ÷ max(M, 6 × 10⁻³ L_max), M the footprint's converged mean and L_max the brightest of the
 * image's five means.
 *
 * The drawn value is the kernel's since R08.T8.c (`tablesCpu.ts`'s `rayMarchTargetTexelTwin`):
 * from above the top a texel whose footprint meets the shell takes its mean over 2 + 8
 * Gauss–Legendre nodes across the limb, blended in by w_L = clamp((s − 0.6) ÷ 0.4, 0, 1), s the
 * texel's span in tangent height over the local scale height at its tangent point (the body's
 * table's, `localScaleHeightM`: on R05's Earth 3.20 km at 0.1 H, 14.87 km at 1 H, its ozone's
 * column above, and 8.43 km at 5 H; Saturn-class 49.7 km). The reference marches each sub-ray
 * with the same twin at the setting's count, so that e is what the drawn texel misses of the light
 * across the pixel, not the quadrature's error; that is e_q, recorded beside it per body against a
 * converged march.
 */

/** The most placed steps a reference takes, confirmed against twice as many (the gate's rule). */
const MAX_REFERENCE_STEPS = 8_192;

/**
 * The Saturn-class figure's 1 H pixel from 10⁸ m on high, the Sun 100°/0°: a named miss of
 * Design note 10's 5% (measured 2026-10-22: 5.016%, its pixel at s = 0.97, w_L 0.91, the centre
 * ray's share of its 58% error; reported to "main"), pinned to the 0.01 point (§4.3's rule).
 */
const SATURN_ONSET_MISS = 0.0502;

/** The named misses of the 5% on high, by place and pixel, and the bound each is held to. */
const NAMED_MISSES: Readonly<Record<string, number>> = {
  "Saturn-class, 10⁸ m, sun 100°/0°, 1 H": SATURN_ONSET_MISS,
};

const built = new Map<string, PlanetTwin>();

function planetOf(body: FarBody): PlanetTwin {
  const planet = built.get(body.name);
  if (planet === undefined) {
    throw new Error(`${body.name}'s twin is built before the tests read it`);
  }
  return planet;
}

// The three bodies' tables took 13 s at four workers on a loaded machine (2026-10-21): the limit is
// over ten times that, as the config's rule for CPU-bound work asks.
beforeAll(() => {
  for (const body of BODIES) {
    built.set(body.name, planetTwin(body.medium, body.radiusM, body.curves));
  }
}, 300_000);

const R05_EARTH = BODIES[0];
if (R05_EARTH === undefined) {
  throw new Error("BODIES opens with R05's Earth");
}

/** A limb-set pixel of an image: its name, its view and its pixel on high. */
interface ImagePixel {
  readonly name: string;
  readonly body: FarBody;
  readonly distanceM: number;
  readonly pitchRad: number;
  readonly view: TwinView;
}

/** The high frame of `body` from `distanceM` whose middle pixel's centre lies `scaleHeights` up. */
function highPixel(
  body: FarBody,
  distanceM: number,
  scaleHeights: number,
  zenithDeg: number,
  azimuthDeg: number,
): ImagePixel {
  const { xPx, yPx } = VARIANTS.high;
  const viewport = OUTPUT.high;
  const pitchRad = limbPitchRad(
    distanceM,
    body.radiusM,
    scaleHeights * body.scaleHeightM,
    viewport,
    xPx,
    yPx,
  );
  return {
    name: `sun ${zenithDeg}°/${azimuthDeg}°, ${scaleHeights} H`,
    body,
    distanceM,
    pitchRad,
    view: limbView(
      body,
      distanceM,
      pitchRad,
      viewport,
      imageSun(distanceM, body.radiusM, zenithDeg, azimuthDeg),
    ),
  };
}

/** s of the high middle pixel's texel in `pixel`'s frame ({@link farLimbFootprintTwin}). */
function spanOf(planet: PlanetTwin, pixel: ImagePixel): number {
  const { xPx, yPx } = VARIANTS.high;
  const shells = farLimbShellsOf(planet, pixel.view);
  const footprint =
    shells === null
      ? null
      : farLimbFootprintTwin(shells, pixel.view, {
          x: xPx,
          y: yPx,
          widthTexels: OUTPUT.high.widthPx,
          heightTexels: OUTPUT.high.heightPx,
        });
  return footprint?.span ?? Number.NaN;
}

/**
 * The distance from `body`'s centre at which the high middle pixel on the ray `scaleHeights` up
 * stands at s = `span`: bisected in log distance, s rising with it as the texel's span does (the
 * pixel's tangent height and so its local scale height held).
 */
function distanceAtSpan(
  planet: PlanetTwin,
  body: FarBody,
  scaleHeights: number,
  span: number,
): number {
  let lo = Math.log(body.radiusM + body.medium.topHeightM + 1_000);
  let hi = Math.log(1e10);
  for (let i = 0; i < 80; i += 1) {
    const mid = (lo + hi) / 2;
    if (spanOf(planet, highPixel(body, Math.exp(mid), scaleHeights, 30, 0)) < span) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  return Math.exp((lo + hi) / 2);
}

/** Output pixel `pixel`'s centre less its picked half-resolution texel's, output pixels, on 1,280. */
const pickOffsetPx = (pixel: number): number =>
  pixel + 0.5 - 2 * (pickedTexel(pixel, 1_280, 0.5) + 0.5);

/** A `vec2f` rule of `farLimb.wgsl`'s, by its name: its nodes and weights. */
const wgslRule = (name: string): number[][] => {
  const body = new RegExp(`const ${name} = array<vec2f, \\d+>\\(([^;]*)\\);`, "u").exec(
    farLimbWgsl,
  )?.[1];
  return [...(body ?? "").matchAll(/vec2f\(([-0-9.e]+), ([-0-9.e]+)\)/gu)].map((m) => [
    Number(m[1]),
    Number(m[2]),
  ]);
};

/** Each value as recorded to about 10⁻⁹ of itself. */
const toRounding = (values: readonly number[]): unknown =>
  values.map((x) => expect.closeTo(x, Math.max(0, 9 - Math.ceil(Math.log10(Math.abs(x) + 1e-30)))));

/** One pixel's figures: e of the drawn texel and of the centre ray alone, and the reference's. */
interface PixelError {
  readonly name: string;
  readonly e: number;
  readonly eCentre: number;
  /** |T − T_ref|, the drawn texel's mean transmittance against the reference's. */
  readonly alpha: number;
  readonly rows: number;
  readonly converged: number;
}

/** An image's pixels' figures, the image's pixels each in its own frame. */
function imageErrors(planet: PlanetTwin, pixels: readonly ImagePixel[]): PixelError[] {
  const { xPx, yPx } = VARIANTS.high;
  const viewport = OUTPUT.high;
  const references = convergedFootprints(
    pixels.map(
      (p) => (rows: number) =>
        footprintMeanOf(
          planet,
          p.view,
          "high",
          xPx,
          yPx,
          pixelCuts(p.body, p.distanceM, p.pitchRad, viewport, xPx, yPx),
          rows,
        ),
    ),
  );
  const lMax = brightestOf(references.map((r) => rgbOf(r.mean)));
  return pixels.map((p, k) => {
    const reference = references[k];
    if (reference === undefined) {
      throw new Error("every pixel has its reference");
    }
    const drawn = drawnAt(planet, p.view, "high", xPx, yPx);
    return {
      name: p.name,
      e: errorOf(rgbOf(drawn), rgbOf(reference.mean), lMax, FLOOR),
      eCentre: errorOf(
        rgbOf(centreAt(planet, p.view, "high", xPx, yPx)),
        rgbOf(reference.mean),
        lMax,
        FLOOR,
      ),
      alpha: Math.abs(drawn[3] - reference.mean[3]),
      rows: reference.rows,
      converged: reference.converged,
    };
  });
}

/**
 * The composite's transmittance to space over high's middle pixel's footprint, converged across
 * the limb: 1 above the top, 0 where the ground hides space, and in the shell midpoint rows at
 * two columns, doubled from 128 until within 10⁻⁵ of half as many in every channel.
 */
function convergedTransmittance(planet: PlanetTwin, pixel: ImagePixel): Rgb {
  const { xPx, yPx } = VARIANTS.high;
  const viewport = OUTPUT.high;
  const sources = tableSources(planet);
  const cuts = pixelCuts(pixel.body, pixel.distanceM, pixel.pitchRad, viewport, xPx, yPx);
  const meanAt = (rows: number): Rgb => {
    const mean: [number, number, number] = [cuts.topY, cuts.topY, cuts.topY];
    const share = (cuts.groundY - cuts.topY) / (rows * 2);
    for (let j = 0; j < rows; j += 1) {
      const v =
        (yPx + cuts.topY + ((j + 0.5) / rows) * (cuts.groundY - cuts.topY)) / viewport.heightPx;
      for (const column of [0.25, 0.75]) {
        const d = unitRayTwin(pixel.view, (xPx + column) / viewport.widthPx, v);
        const t = compositeTransmittanceTwin(sources, planet.medium.topHeightM, pixel.view, d);
        for (const c of [0, 1, 2] as const) {
          mean[c] += share * t[c];
        }
      }
    }
    return mean;
  };
  let rows = 128;
  let coarse = meanAt(rows);
  let fine = meanAt(2 * rows);
  while (
    !(Math.max(...coarse.map((t, c) => Math.abs(t - (fine[c] ?? Number.NaN)))) < 1e-5) &&
    rows < 4_096
  ) {
    rows *= 2;
    coarse = fine;
    fine = meanAt(2 * rows);
  }
  return fine;
}

/**
 * The converged march of a ray: R05.T12.e's placed steps, as the gate's references take them
 * (the camera side placed about the lowest point from outside the air, no warp).
 */
function placedMarch(
  planet: PlanetTwin,
  view: TwinView,
  marched: RayMarchRay,
  samples: number,
): Rgb {
  const ray = { ...marched.ray, fromCamera: marched.ray.tStartM <= 0, skySegment: false };
  const settings = { ...TABLE_SIZES.high, rayMarchSamples: samples, marchDepthSteps: null };
  return rgbOf(rayMarchTexelTwin(planet, view, settings, { ...marched, ray }));
}

/** A set of figures as recorded, with the pixel where its worst stands. */
const recordedAt = (where: string, f: Figures): unknown => ({ ...recordedFigure(f), at: where });

describe("the far limb's frames (R08.T8.b)", () => {
  it("aims each limb-set ray through its pixel's centre, to 1 mm of its tangent height", () => {
    const off: string[] = [];
    for (const { body, distanceM } of PLACES) {
      for (const variant of VARIANT_NAMES) {
        const { setting, xPx, yPx } = VARIANTS[variant];
        const viewport = OUTPUT[setting];
        for (const scaleHeights of LIMB_SET) {
          const tangentM = scaleHeights * body.scaleHeightM;
          const pitch = limbPitchRad(distanceM, body.radiusM, tangentM, viewport, xPx, yPx);
          const view = limbView(
            body,
            distanceM,
            pitch,
            viewport,
            imageSun(distanceM, body.radiusM, 30, 0),
          );
          const u = (xPx + 0.5) / viewport.widthPx;
          const v = (yPx + 0.5) / viewport.heightPx;
          const marched = rayMarchRayTwin(body.medium, view, u, v, 0, TABLE_SIZES[setting]);
          const c = marched === null ? null : cross(marched.ray.originM, marched.ray.direction);
          const twinTangentM = c === null ? Number.NaN : Math.hypot(c.x, c.y, c.z) - body.radiusM;
          if (!(Math.abs(twinTangentM - tangentM) < 1e-3)) {
            off.push(`${body.name}, ${distanceName(distanceM)}, ${variant}, ${scaleHeights} H`);
          }
        }
      }
    }
    expect(off).toEqual([]);
  });

  // The pixel's centre less the texel's, in output pixels: negative where the texel lies below and
  // right of it. The test's own emulation of the pick, which its figures read; the smoke's
  // `checkLowPick` holds the shader's.
  it("emulates low's pick: the texel 0.5 px from the pixel's centre, of either parity", () => {
    expect([640, 641, 360, 361].map(pickOffsetPx)).toEqual([-0.5, 0.5, -0.5, 0.5]);
    expect([0, 1, 2, 3, 1_279].map((x) => pickedTexel(x, 1_280, 0.5))).toEqual([0, 0, 1, 1, 639]);
    expect(pickedTexel(960, 1_920, 1)).toBe(960);
  });

  // A pixel's span in tangent height over Rayleigh's H (Earth) or the medium's own (Saturn-class):
  // each body's own, the two Earths' alike.
  it("records a pixel's span in tangent height at each distance", () => {
    const spans = Object.fromEntries(
      PLACES.filter(({ body }) => body.name !== "the measured Earth").flatMap(
        ({ body, distanceM }) =>
          (["high", "low"] as const).map((setting) => {
            const viewport = OUTPUT[setting];
            const x = viewport.widthPx / 2;
            const y = viewport.heightPx / 2;
            const pitch = limbPitchRad(distanceM, body.radiusM, body.scaleHeightM, viewport, x, y);
            const spanM =
              pixelTangentM(distanceM, body.radiusM, pitch, viewport, x + 0.5, y) -
              pixelTangentM(distanceM, body.radiusM, pitch, viewport, x + 0.5, y + 1);
            const key = `${body.name}, ${distanceName(distanceM)}, ${setting}`;
            return [key, spanM / body.scaleHeightM];
          }),
      ),
    );
    expect(spans).toEqual(
      asRecorded({
        "R05's Earth, 10⁷ m, high": 0.54867,
        "R05's Earth, 10⁷ m, low": 0.82301,
        "R05's Earth, 10⁸ m, high": 7.11568,
        "R05's Earth, 10⁸ m, low": 10.67337,
        "R05's Earth, 10⁹ m, high": 71.22245,
        "R05's Earth, 10⁹ m, low": 106.68413,
        "Saturn-class, 10⁸ m, high": 0.96472,
        "Saturn-class, 10⁸ m, low": 1.44708,
        "Saturn-class, 10⁹ m, high": 12.07303,
        "Saturn-class, 10⁹ m, low": 18.10927,
      }),
    );
  });
});

describe("the far limb's blend (R08.T8.c)", () => {
  it("is 0 to s = 0.6, rises linearly, and is 1 from s = 1", () => {
    expect([0, 0.3, 0.6, 0.7, 0.8, 0.9, 1, 1.5, 100].map(farLimbBlend)).toEqual([
      0,
      0,
      0,
      expect.closeTo(0.25, 12),
      expect.closeTo(0.5, 12),
      expect.closeTo(0.75, 12),
      1,
      1,
      1,
    ]);
    expect(FAR_LIMB_BLEND_FROM).toBe(0.6);
  });

  it("holds the same Gauss–Legendre rules and blend as farLimb.wgsl", () => {
    expect(wgslRule("FAR_LIMB_GAUSS_2")).toEqual(FAR_LIMB_GAUSS_LEGENDRE[2]);
    expect(wgslRule("FAR_LIMB_GAUSS_8")).toEqual(FAR_LIMB_GAUSS_LEGENDRE[8]);
    expect(farLimbWgsl).toContain(`const FAR_LIMB_BLEND_FROM : f32 = ${FAR_LIMB_BLEND_FROM};`);
    expect(farLimbWgsl).toContain(`const FAR_LIMB_BLEND_WIDTH : f32 = ${FAR_LIMB_BLEND_WIDTH};`);
  });

  it("takes Gauss–Legendre rules exact to degree 2n − 1", () => {
    for (const n of [2, 8] as const) {
      const moments = Array.from({ length: 2 * n }, (_, k) =>
        FAR_LIMB_GAUSS_LEGENDRE[n].reduce((sum, [x, w]) => sum + w * x ** k, 0),
      );
      expect(moments).toEqual(moments.map((_, k) => expect.closeTo(1 / (k + 1), 13)));
    }
  });

  // s of each limb-set ray's high pixel, by the local scale height at its tangent point.
  it("records s at each limb-set pixel per body and distance, on high and low", () => {
    const record = Object.fromEntries(
      PLACES.flatMap(({ body, distanceM }) =>
        (["high", "low"] as const).map((setting) => {
          const planet = planetOf(body);
          const viewport = OUTPUT[setting];
          const variant = setting === "high" ? VARIANTS.high : VARIANTS["low even"];
          const s = TABLE_SIZES[setting].rayMarchScale;
          const texel = {
            x: pickedTexel(variant.xPx, viewport.widthPx, s),
            y: pickedTexel(variant.yPx, viewport.heightPx, s),
            widthTexels: Math.ceil(viewport.widthPx * s),
            heightTexels: Math.ceil(viewport.heightPx * s),
          };
          const spans = LIMB_SET.map((scaleHeights) => {
            const pitch = limbPitchRad(
              distanceM,
              body.radiusM,
              scaleHeights * body.scaleHeightM,
              viewport,
              variant.xPx,
              variant.yPx,
            );
            const view = limbView(
              body,
              distanceM,
              pitch,
              viewport,
              imageSun(distanceM, body.radiusM, 30, 0),
            );
            const shells = farLimbShellsOf(planet, view);
            return shells === null
              ? Number.NaN
              : (farLimbFootprintTwin(shells, view, texel)?.span ?? Number.NaN);
          });
          return [
            `${body.name}, ${distanceName(distanceM)}, ${setting}`,
            spans.map((x) => Number(x.toFixed(3))),
          ];
        }),
      ),
    );
    expect(record).toEqual({
      "R05's Earth, 10⁷ m, high": [1.372, 0.758, 0.311, 0.603, 0.645],
      "R05's Earth, 10⁷ m, low": [5.861, 5.86, 1.288, 1.221, 1.824],
      "R05's Earth, 10⁸ m, high": [17.86, 9.864, 4.054, 7.872, 8.476],
      "R05's Earth, 10⁸ m, low": [76.917, 76.917, 76.916, 76.912, 28.89],
      "R05's Earth, 10⁹ m, high": [186.311, 102.904, 42.289, 82.109, 88.381],
      "R05's Earth, 10⁹ m, low": [864.472, 864.446, 864.355, 864.097, 863.204],
      "the measured Earth, 10⁷ m, high": [1.665, 1.042, 0.669, 0.729, 0.781],
      "the measured Earth, 10⁷ m, low": [6.62, 6.619, 2.121, 2.192, 2.256],
      "the measured Earth, 10⁸ m, high": [21.665, 13.558, 8.714, 9.513, 10.255],
      "the measured Earth, 10⁸ m, low": [86.868, 86.867, 86.866, 86.862, 28.674],
      "the measured Earth, 10⁹ m, high": [226.008, 141.437, 90.9, 99.227, 106.935],
      "the measured Earth, 10⁹ m, low": [976.306, 976.277, 976.175, 975.883, 974.874],
      "Saturn-class, 10⁸ m, high": [0.966, 0.965, 0.965, 0.964, 0.963],
      "Saturn-class, 10⁸ m, low": [2.9, 2.9, 2.899, 2.896, 2.89],
      "Saturn-class, 10⁹ m, high": [12.133, 12.133, 12.133, 12.133, 12.163],
      "Saturn-class, 10⁹ m, low": [36.762, 36.761, 36.761, 36.76, 36.756],
    });
  });

  // Acceptance 4: the output equals the centre ray at s = 0.6 and the footprint's mean at s = 1, to
  // rounding, on R05's Earth's 1 H pixel, the Sun 100°/0°.
  const edge = {
    sizes: TABLE_SIZES.high,
    ...VARIANTS.high,
    texel: {
      x: VARIANTS.high.xPx,
      y: VARIANTS.high.yPx,
      widthTexels: OUTPUT.high.widthPx,
      heightTexels: OUTPUT.high.heightPx,
    },
  };
  /** A twin texel of R05's Earth along the ray through (u, v), RGBA. */
  const texelAt = (view: TwinView, u: number, v: number): readonly number[] => {
    const planet = planetOf(R05_EARTH);
    return rayMarchTexelTwin(
      planet,
      view,
      edge.sizes,
      rayMarchRayTwin(planet.medium, view, u, v, 0, edge.sizes),
    );
  };

  it("draws the centre ray at s = 0.6", () => {
    const planet = planetOf(R05_EARTH);
    const near = highPixel(R05_EARTH, distanceAtSpan(planet, R05_EARTH, 1, 0.6), 1, 100, 0);
    const u = (edge.xPx + 0.5) / OUTPUT.high.widthPx;
    const v = (edge.yPx + 0.5) / OUTPUT.high.heightPx;
    expect(spanOf(planet, near)).toBeCloseTo(0.6, 9);
    expect(rayMarchTargetTexelTwin(planet, near.view, edge.sizes, edge.xPx, edge.yPx, 0)).toEqual(
      toRounding(texelAt(near.view, u, v)),
    );
  });

  it("draws the footprint's mean at s = 1", () => {
    const planet = planetOf(R05_EARTH);
    const far = highPixel(R05_EARTH, distanceAtSpan(planet, R05_EARTH, 1, 1), 1, 100, 0);
    const shells = farLimbShellsOf(planet, far.view);
    const footprint = shells === null ? null : farLimbFootprintTwin(shells, far.view, edge.texel);
    const mean = [0, 0, 0, footprint?.spaceShare ?? Number.NaN];
    // The 1 H texel at s = 1 spans 0.5–1.5 local scale heights there, all over the datum's edge:
    // its shell piece's 8 nodes alone.
    for (const node of footprint?.nodes ?? []) {
      const at = texelAt(far.view, node.u, node.v);
      for (let c = 0; c < 4; c += 1) {
        mean[c] = (mean[c] ?? Number.NaN) + node.weight * (at[c] ?? Number.NaN);
      }
    }
    expect(spanOf(planet, far)).toBeCloseTo(1, 9);
    expect(rayMarchTargetTexelTwin(planet, far.view, edge.sizes, edge.xPx, edge.yPx, 0)).toEqual(
      toRounding(mean),
    );
  });
});

describe("the far limb's fidelity on high against the converged footprint (R08.T8.c)", () => {
  const results = new Map<string, PixelError[]>();
  /** The s = 1 images' distances, by pixel. */
  const edgeDistances = new Map<string, number>();

  const EDGE = "R05's Earth, s = 1";

  // Measured 2026-10-22: see the records below. The limit is over ten times the run's time.
  beforeAll(() => {
    for (const { body, distanceM } of PLACES) {
      const planet = planetOf(body);
      results.set(
        `${body.name}, ${distanceName(distanceM)}`,
        SUNS.flatMap(([zenithDeg, azimuthDeg]) =>
          imageErrors(
            planet,
            LIMB_SET.map((n) => highPixel(body, distanceM, n, zenithDeg, azimuthDeg)),
          ),
        ),
      );
    }
    const planet = planetOf(R05_EARTH);
    for (const n of LIMB_SET) {
      edgeDistances.set(`${n} H`, distanceAtSpan(planet, R05_EARTH, n, 1));
    }
    results.set(
      EDGE,
      SUNS.flatMap(([zenithDeg, azimuthDeg]) =>
        imageErrors(
          planet,
          LIMB_SET.map((n) =>
            highPixel(
              R05_EARTH,
              edgeDistances.get(`${n} H`) ?? Number.NaN,
              n,
              zenithDeg,
              azimuthDeg,
            ),
          ),
        ),
      ),
    );
  }, 600_000);

  /** Each place's figures of e, of the drawn texel or of the centre ray alone. */
  const figuresOf = (of: (p: PixelError) => number): Record<string, Figures> =>
    Object.fromEntries(
      [...results.entries()].map(([key, es]) => [key, figuresOfErrors(es.map(of))]),
    );

  it("records the distances at which each limb-set pixel stands at s = 1 on R05's Earth", () => {
    expect(Object.fromEntries(edgeDistances)).toEqual(
      Object.fromEntries(
        Object.entries({
          "0.1 H": 8_496_578.47,
          "0.3 H": 11_996_915.14,
          "1 H": 25_515_920.18,
          "3 H": 14_248_739.51,
          "10 H": 13_472_581.74,
        }).map(([key, m]) => [key, expect.closeTo(m, 1)]),
      ),
    );
  });

  it("has a converged reference for every pixel but those recorded", () => {
    const unconverged = [...results.entries()].flatMap(([key, es]) =>
      es
        .filter((p) => !(p.converged < CONVERGED))
        .map((p) => [`${key}, ${p.name}`, Number(p.converged.toFixed(5))] as const),
    );
    // Measured 2026-10-22: from 10⁹ m with the Sun on the limb, where its light crosses the
    // shadow's edge within the footprint, 2,048 rows hold the reference to 0.09–0.31% of 4,096.
    expect(Object.fromEntries(unconverged)).toEqual({
      "R05's Earth, 10⁹ m, sun 90°/0°, 0.1 H": 0.00311,
      "R05's Earth, 10⁹ m, sun 90°/0°, 0.3 H": 0.00286,
      "R05's Earth, 10⁹ m, sun 90°/0°, 1 H": 0.00191,
      "R05's Earth, 10⁹ m, sun 90°/0°, 3 H": 0.00088,
      "the measured Earth, 10⁹ m, sun 90°/0°, 0.1 H": 0.00218,
      "the measured Earth, 10⁹ m, sun 90°/0°, 0.3 H": 0.00204,
      "the measured Earth, 10⁹ m, sun 90°/0°, 1 H": 0.00153,
    });
    expect(MAX_REFERENCE_ROWS).toBe(2_048);
  });

  // Acceptance 2: at 10⁸ and 10⁹ m on every body, and at s = 1 on R05's Earth, every limb-set pixel
  // but the Sun on the limb (90°/0°) is within 5% of its converged footprint, or of its named miss.
  it("holds every limb-set pixel within 5% at 10⁸ and 10⁹ m and at s = 1, but 90°/0°", () => {
    const over = [...results.entries()]
      .filter(([key]) => !key.endsWith("10⁷ m"))
      .flatMap(([key, es]) =>
        es
          .filter((p) => !p.name.startsWith(ON_THE_LIMB))
          .filter((p) => !(p.e <= (NAMED_MISSES[`${key}, ${p.name}`] ?? BOUND)))
          .map((p) => `${key}, ${p.name}: ${(100 * p.e).toFixed(2)}%`),
      );
    expect(over).toEqual([]);
  });

  // Measured 2026-10-22. From 10⁸ m every pixel holds on both Earths (worst 3.22% and 2.85%) and
  // from 10⁹ m every pixel but the Sun on the limb's; at s = 1 the worst is 0.15%. The Saturn-class
  // limb from 10⁸ m stands at s 0.96–0.97, w_L 0.91, its one pixel over the centre ray's share
  // (`SATURN_ONSET_MISS`). From 10⁷ m (s 0.31–1.67 on high, by the local scale height at each
  // tangent point) the pixels over are the backlit 0.3 and 1 H rows that T8.b recorded as the edge's
  // aliasing, the 0.1 H row's s over 1 now taking its footprint's mean. Since R08.T8.e the bare
  // ground's count follows its optical depth on high: the medians on R05's Earth from 10⁸ m and the
  // measured Earth from 10⁹ m moved (0.35% → 0.38%, 0.71% → 0.70%; 2026-10-22), no worst or count;
  // its thin sky at 28 steps those on R05's Earth and the Saturn-class figure from 10⁹ m (1.47% →
  // 1.48%, 0.30% → 0.34%; 2026-10-23).
  it("records the worst and median e and the count over 5% per body and distance", () => {
    expect(figuresOf((p) => p.e)).toEqual(
      recordedFigures({
        "R05's Earth, 10⁷ m": { worst: 0.4221, median: 0.00594, over: 4 },
        "R05's Earth, 10⁸ m": { worst: 0.03225, median: 0.00382, over: 0 },
        "R05's Earth, 10⁹ m": { worst: 0.1086, median: 0.01479, over: 5 },
        "the measured Earth, 10⁷ m": { worst: 0.26588, median: 0.00095, over: 2 },
        "the measured Earth, 10⁸ m": { worst: 0.02854, median: 0.00201, over: 0 },
        "the measured Earth, 10⁹ m": { worst: 0.09207, median: 0.00697, over: 5 },
        "Saturn-class, 10⁸ m": { worst: 0.05016, median: 0.00096, over: 1 },
        "Saturn-class, 10⁹ m": { worst: 0.02953, median: 0.0034, over: 0 },
        "R05's Earth, s = 1": { worst: 0.00153, median: 0.00012, over: 0 },
      }),
    );
  });

  // Measured 2026-10-22: the centre ray alone, against the converged footprint (T8.b recorded it
  // against its 8 × 8 mean). From 10⁹ m the worst is 27 and 30 times the footprint's light. Moved in
  // the fourth or fifth figure by R08.T8.e's count on the bare ground (2026-10-22).
  it("records the same of the centre ray alone, the march as T8.b drew it", () => {
    expect(figuresOf((p) => p.eCentre)).toEqual(
      recordedFigures({
        "R05's Earth, 10⁷ m": { worst: 0.9489, median: 0.00789, over: 6 },
        "R05's Earth, 10⁸ m": { worst: 1.92698, median: 0.55373, over: 35 },
        "R05's Earth, 10⁹ m": { worst: 27.1252, median: 0.99945, over: 35 },
        "the measured Earth, 10⁷ m": { worst: 0.95701, median: 0.00757, over: 6 },
        "the measured Earth, 10⁸ m": { worst: 2.19016, median: 0.46668, over: 35 },
        "the measured Earth, 10⁹ m": { worst: 29.72396, median: 0.99994, over: 35 },
        "Saturn-class, 10⁸ m": { worst: 0.58009, median: 0.01077, over: 7 },
        "Saturn-class, 10⁹ m": { worst: 3.23772, median: 0.52268, over: 35 },
        "R05's Earth, s = 1": { worst: 0.8886, median: 0.02282, over: 10 },
      }),
    );
  });

  it("records the drawn texel's mean transmittance against the reference's, the worst", () => {
    const worst = Object.fromEntries(
      [...results.entries()].map(([key, es]) => [key, Math.max(...es.map((p) => p.alpha))]),
    );
    // Measured 2026-10-22: what lies behind the limb takes the same mean, the worst |T − T_ref| at
    // most 0.0038 (from 10⁷ m, where the centre ray stands for most pixels).
    expect(worst).toEqual(
      asRecorded({
        "R05's Earth, 10⁷ m": 0.00381,
        "R05's Earth, 10⁸ m": 0.0011,
        "R05's Earth, 10⁹ m": 0.00279,
        "the measured Earth, 10⁷ m": 0.0004,
        "the measured Earth, 10⁸ m": 0.00119,
        "the measured Earth, 10⁹ m": 0.00297,
        "Saturn-class, 10⁸ m": 0.00028,
        "Saturn-class, 10⁹ m": 0.00109,
        "R05's Earth, s = 1": 0.00008,
      }),
    );
  });

  it("names the pixels over 5% on high", () => {
    const named = Object.fromEntries(
      [...results.entries()].flatMap(([key, es]) =>
        es.filter((p) => !(p.e <= BOUND)).map((p) => [`${key}, ${p.name}`, p.e]),
      ),
    );
    // Measured 2026-10-22: from 10⁷ m the backlit and terminator rows, from 10⁹ m the Sun on the
    // limb (its centre within Design note 10's 3°: recorded, not held), and the named miss.
    expect(named).toEqual(
      asRecorded({
        "R05's Earth, 10⁷ m, sun 90°/0°, 0.3 H": 0.20264,
        "R05's Earth, 10⁷ m, sun 90°/0°, 1 H": 0.10072,
        "R05's Earth, 10⁷ m, sun 100°/0°, 0.3 H": 0.4221,
        "R05's Earth, 10⁷ m, sun 100°/0°, 1 H": 0.23144,
        "R05's Earth, 10⁹ m, sun 90°/0°, 0.1 H": 0.1086,
        "R05's Earth, 10⁹ m, sun 90°/0°, 0.3 H": 0.10859,
        "R05's Earth, 10⁹ m, sun 90°/0°, 1 H": 0.10849,
        "R05's Earth, 10⁹ m, sun 90°/0°, 3 H": 0.10839,
        "R05's Earth, 10⁹ m, sun 90°/0°, 10 H": 0.10799,
        "the measured Earth, 10⁷ m, sun 90°/0°, 1 H": 0.11467,
        "the measured Earth, 10⁷ m, sun 100°/0°, 1 H": 0.26588,
        "the measured Earth, 10⁹ m, sun 90°/0°, 0.1 H": 0.09207,
        "the measured Earth, 10⁹ m, sun 90°/0°, 0.3 H": 0.09207,
        "the measured Earth, 10⁹ m, sun 90°/0°, 1 H": 0.09201,
        "the measured Earth, 10⁹ m, sun 90°/0°, 3 H": 0.09195,
        "the measured Earth, 10⁹ m, sun 90°/0°, 10 H": 0.09177,
        "Saturn-class, 10⁸ m, sun 100°/0°, 1 H": 0.05016,
      }),
    );
  });
});

describe("the footprint's nodes over a drawn surface (R08.T8.c)", () => {
  // On low a texel is 2 × 2 output pixels, the kernel's centre ray at the depth under the lower
  // right one and each node under its own. With a surface drawn on every odd row, 2 × 10⁸ m away
  // (beyond the body), a texel's upper nodes stand on the sky and its lower ones on the surface.
  it("marches each node at the scene's depth under its own point", () => {
    const planet = planetOf(R05_EARTH);
    const sizes = TABLE_SIZES.low;
    const viewport = { widthPx: 64, heightPx: 36 };
    const distanceM = 1e8;
    const pitch = limbPitchRad(distanceM, R05_EARTH.radiusM, 8_000, viewport, 32, 18);
    const view = limbView(
      R05_EARTH,
      distanceM,
      pitch,
      viewport,
      imageSun(distanceM, R05_EARTH.radiusM, 30, 0),
    );
    const surface = 0.1 / 2e8;
    const depthAt = (_x: number, y: number): number => (y % 2 === 1 ? surface : 0);
    const target = rayMarchTwin(planet, view, sizes, depthAt);
    const shells = farLimbShellsOf(planet, view);
    const at = (u: number, v: number): readonly number[] => {
      const depth = depthAt(0, Math.floor(v * viewport.heightPx));
      return rayMarchTexelTwin(
        planet,
        view,
        sizes,
        rayMarchRayTwin(planet.medium, view, u, v, depth, sizes),
      );
    };
    const off: string[] = [];
    let mixed = 0;
    for (let y = 0; y < target.heightTexels; y += 1) {
      for (let x = 0; x < target.widthTexels; x += 1) {
        const texel = { x, y, widthTexels: target.widthTexels, heightTexels: target.heightTexels };
        const footprint = shells === null ? null : farLimbFootprintTwin(shells, view, texel);
        if (footprint === null || !(footprint.blend > 0)) {
          continue;
        }
        const rows = new Set(footprint.nodes.map((n) => Math.floor(n.v * viewport.heightPx) % 2));
        mixed += rows.size > 1 ? 1 : 0;
        const centre = at((x + 0.5) / target.widthTexels, (y + 0.5) / target.heightTexels);
        const mean = [0, 0, 0, footprint.spaceShare];
        for (const node of footprint.nodes) {
          const value = at(node.u, node.v);
          for (let c = 0; c < 4; c += 1) {
            mean[c] = (mean[c] ?? Number.NaN) + node.weight * (value[c] ?? Number.NaN);
          }
        }
        for (let c = 0; c < 4; c += 1) {
          const expected =
            (1 - footprint.blend) * (centre[c] ?? Number.NaN) +
            footprint.blend * (mean[c] ?? Number.NaN);
          const drawn = target.texels[(y * target.widthTexels + x) * 4 + c] ?? Number.NaN;
          if (!(Math.abs(drawn - expected) <= 1e-12 * Math.max(Math.abs(expected), 1e-30))) {
            off.push(`texel (${x}, ${y}) channel ${c}: ${drawn} against ${expected}`);
          }
        }
      }
    }
    expect(mixed).toBeGreaterThan(0);
    expect(off).toEqual([]);
  });
});

describe("what lies behind the far limb (R08.T8.c)", () => {
  // Measured 2026-10-22: a star, or the Sun's disc in an eclipse seen from afar, behind the limb
  // takes the footprint's mean transmittance per channel, where the centre ray's cut it out row by
  // row (§6.1). The worst |T − T_ref| over the channels and the limb set, on R05's Earth: 0.0013 and
  // 0.0029 across the footprint, 0.21 and 0.47 by the centre ray alone.
  it("takes the composite's transmittance across the pixel's footprint", () => {
    const planet = planetOf(R05_EARTH);
    const sources = tableSources(planet);
    const { xPx, yPx } = VARIANTS.high;
    const viewport = OUTPUT.high;
    const record = Object.fromEntries(
      [1e8, 1e9].flatMap((distanceM) => {
        let drawn = 0;
        let centre = 0;
        for (const n of LIMB_SET) {
          const pixel = highPixel(R05_EARTH, distanceM, n, 30, 0);
          const reference = convergedTransmittance(planet, pixel);
          const across = compositeFootprintTransmittanceTwin(planet, pixel.view, xPx, yPx);
          const d = unitRayTwin(
            pixel.view,
            (xPx + 0.5) / viewport.widthPx,
            (yPx + 0.5) / viewport.heightPx,
          );
          const alone = compositeTransmittanceTwin(
            sources,
            planet.medium.topHeightM,
            pixel.view,
            d,
          );
          for (const c of [0, 1, 2] as const) {
            drawn = Math.max(drawn, Math.abs(across[c] - reference[c]));
            centre = Math.max(centre, Math.abs(alone[c] - reference[c]));
          }
        }
        return [
          [`${distanceName(distanceM)}, across the footprint`, drawn],
          [`${distanceName(distanceM)}, the centre ray`, centre],
        ];
      }),
    );
    expect(record).toEqual(
      asRecorded({
        "10⁸ m, across the footprint": 0.00128,
        "10⁸ m, the centre ray": 0.20747,
        "10⁹ m, across the footprint": 0.0029,
        "10⁹ m, the centre ray": 0.47029,
      }),
    );
  });
});

describe("the kernel's own quadrature on the limb set, e_q (R08.T8.b)", () => {
  /** One limb-set ray's e_q, and its reference's steps and convergence. */
  interface QuadratureError {
    readonly name: string;
    readonly e: number;
    readonly steps: number;
    readonly converged: number;
  }
  const results = new Map<string, QuadratureError[]>();

  // A limb chord seen from outside the air is the same at every distance, so e_q is measured once
  // a body, from its nearest distance, where its pixels come nearest the 5%. The image's sun is set
  // at the grazing ray's tangent point, up to 0.7° from where it stands at a ray's own (10 H from
  // 10⁷ m). 3.0 s at four workers on a loaded machine (2026-10-21); the limit is over ten times
  // that.
  beforeAll(() => {
    for (const body of BODIES) {
      const planet = planetOf(body);
      const distanceM = body.distancesM[0] ?? Number.NaN;
      for (const setting of ["high", "low"] as const) {
        const sizes = TABLE_SIZES[setting];
        const viewport = OUTPUT[setting];
        const x = viewport.widthPx / 2;
        const y = viewport.heightPx / 2;
        const items = SUNS.flatMap(([zenithDeg, azimuthDeg]) => {
          const sun = imageSun(distanceM, body.radiusM, zenithDeg, azimuthDeg);
          const rays = LIMB_SET.map((scaleHeights) => {
            const pitch = limbPitchRad(
              distanceM,
              body.radiusM,
              scaleHeights * body.scaleHeightM,
              viewport,
              x,
              y,
            );
            const view = limbView(body, distanceM, pitch, viewport, sun);
            const u = (x + 0.5) / viewport.widthPx;
            const v = (y + 0.5) / viewport.heightPx;
            const marched = rayMarchRayTwin(planet.medium, view, u, v, 0, sizes);
            if (marched === null) {
              throw new Error(`the ${scaleHeights} H ray from ${distanceM} m misses the air`);
            }
            return {
              scaleHeights,
              view,
              marched,
              kernel: rgbOf(rayMarchTexelTwin(planet, view, sizes, marched)),
              reference: placedMarch(planet, view, marched, REFERENCE_STEPS),
              confirm: placedMarch(planet, view, marched, 2 * REFERENCE_STEPS),
            };
          });
          const lMax = brightestOf(rays.map((r) => r.reference));
          return rays.map((r) => {
            let steps = REFERENCE_STEPS;
            let { reference, confirm } = r;
            while (
              steps < MAX_REFERENCE_STEPS &&
              !(errorOf(reference, confirm, lMax, CONVERGENCE_FLOOR) < CONVERGED)
            ) {
              steps *= 2;
              reference = confirm;
              confirm = placedMarch(planet, r.view, r.marched, 2 * steps);
            }
            return {
              name: `sun ${zenithDeg}°/${azimuthDeg}°, ${r.scaleHeights} H`,
              e: errorOf(r.kernel, reference, lMax, FLOOR),
              steps,
              converged: errorOf(reference, confirm, lMax, CONVERGENCE_FLOOR),
            };
          });
        });
        results.set(`${body.name}, ${setting}`, items);
      }
    }
  }, 60_000);

  it("has a reference of 2,048 placed steps within 0.05% of 4,096 on every limb-set ray", () => {
    const unconverged = [...results.entries()].flatMap(([key, es]) =>
      es
        .filter((p) => !(p.steps === REFERENCE_STEPS && p.converged < CONVERGED))
        .map((p) => `${key}, ${p.name}`),
    );
    expect(unconverged).toEqual([]);
  });

  // Measured 2026-10-21. High's 36 steps hold every limb-set ray within 5% on all three bodies,
  // the worst at 0.1 H with the Sun 90–100° behind the camera (the gate's limb from 400 km stands
  // at 4.35% and its night limb at 4.69% on R05's Earth). Low's 16 steps put nine rays a body
  // over, the Saturn-class limb's worst at 1 H, sun 100°/0° (31.86%), over the 24.0% that
  // `LOW_TWIN_WORST.rayMarch` measured on Earth's media. R08.T8.e's thin sky at 28 steps moves only
  // high's medians (2026-10-23); at 16 and 20 steps the Saturn-class limb at 10 H, sun 100°/0°, rose
  // to 4.48% and 2.32%, and at 12 to 14.6%.
  it("records e_q per body and setting: the worst, where it is, the median and the count over 5%", () => {
    const record = Object.fromEntries(
      [...results.entries()].map(([key, es]) => {
        const worst = es.reduce((w, p) => (p.e > w.e ? p : w));
        return [key, { ...figuresOfErrors(es.map((p) => p.e)), at: worst.name }];
      }),
    );
    const at = recordedAt;
    expect(record).toEqual({
      "R05's Earth, high": at("sun 100°/180°, 0.1 H", { worst: 0.04645, median: 0.00337, over: 0 }),
      "R05's Earth, low": at("sun 90°/0°, 0.1 H", { worst: 0.24039, median: 0.0107, over: 9 }),
      "the measured Earth, high": at("sun 100°/180°, 0.1 H", {
        worst: 0.04418,
        median: 0.00109,
        over: 0,
      }),
      "the measured Earth, low": at("sun 100°/180°, 0.1 H", {
        worst: 0.22169,
        median: 0.01938,
        over: 9,
      }),
      "Saturn-class, high": at("sun 90°/180°, 0.1 H", { worst: 0.01303, median: 0.00186, over: 0 }),
      "Saturn-class, low": at("sun 100°/0°, 1 H", { worst: 0.31862, median: 0.01014, over: 9 }),
    });
  });
});
