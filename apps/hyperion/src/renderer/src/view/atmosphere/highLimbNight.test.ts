import { beforeAll, describe, expect, it } from "vitest";

import { add, dot, scale } from "../../geometry/vec3";
import {
  BOTTOM_M,
  CONVERGED,
  describeFailures,
  errorOf,
  errors,
  failures,
  type Gate,
  type GateRay,
  HIGH,
  kernelOf,
  LOW,
  type Placement,
  REFERENCE_STEPS,
  type Rgb3,
  runGate,
  type Scheme,
  sphereHits,
  sunAt,
  twilight,
} from "../../test/atmosphereGate";
import { memoised } from "../../test/memo";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, R05_EARTH_REFERENCE } from "./earth";
import { LOW_TWIN_WORST, type MarchStep } from "./marchSteps";
import type { AtmosphereMedium } from "./medium";
import type { AbsorberCurveSlots } from "./tables";
import {
  marchTwin,
  mediumMarchEdgesM,
  piecedSteps,
  planetTwin,
  tableSources,
  type TwinRay,
  type TwinSources,
} from "./tablesCpu";

/**
 * The night side of the sky view's high limb (plan R08, R08.T8.b; `decision-r08-t8-leans.md`
 * §4.3, which left it open for the far limb's work): R05.T12.e's quadrature gate, the gate's
 * harness and metric (`test/atmosphereGate.ts`), on the gate's `high limb` rays from cameras at
 * 60 and 80 km and 1 m under the top, tangent at 0.84, 2.5, 8.4 and 25 km, with the Sun 90°, 95°,
 * 100° and 105° from the zenith at the tangent point and 0° (beyond the limb) or 180° (behind the
 * camera) in azimuth, and for the floor the gate's own day-side rays, the Sun 30° from the zenith
 * at 0°, 90° and 180°.
 *
 * @remarks
 * Each ray is marched twice, on R05's Earth and on the measured Earth (`earth.ts`'s
 * `EARTH_REFERENCE`). Once as the sky view's texel marches it from inside the air: V1, lane C's
 * placement that addendum A of `decision-r05-high-atmosphere.md` adopted, the camera side placed
 * toward the camera, at 75 steps on high and 16 on low. Once as the ray march draws it since
 * R08.T8.a, which takes the whole frame from these heights: a sky pixel's two-sided segment under
 * the sun-aware warp (§5.4), at 36 steps on high and 16 on low. The references are R05.T12.e's
 * placed steps, converged by the gate's rule, and the floor is 10⁻³ of the brightest reference
 * among these rays, the 30° rays included (the gate's metric).
 */

const TOP_M = BOTTOM_M + R05_EARTH_REFERENCE.topHeightM;
const GATE_SPHERE = { kind: "sphere", bottomRadiusM: BOTTOM_M } as const;

/** Addendum B's high cameras, m: 60 and 80 km, and 1 m under the top (the gate's). */
const CAMERAS_M = [60_000, 80_000, R05_EARTH_REFERENCE.topHeightM - 1] as const;

/** The high limb's tangent heights, m (the gate's `high limb`). */
const TANGENTS_M = [840, 2_500, 8_400, 25_000] as const;

/**
 * The suns, zenith angle and azimuth at the tangent point, degrees: the night side, then the
 * floor's.
 */
const SUNS = [
  ...[90, 95, 100, 105].flatMap((zenithDeg) => [0, 180].map((az) => [zenithDeg, az] as const)),
  ...[0, 90, 180].map((az) => [30, az] as const),
] as const;

/** The sun zeniths each worst is recorded at, degrees. */
const ZENITHS_DEG = [30, 90, 95, 100, 105] as const;

/**
 * The rays: the gate's `high limb` geometry under {@link SUNS}, named without the family, each sun
 * set at the tangent point as the gate sets it (`sunAt`, azimuth 0 ahead along the ray).
 */
function nightRays(): GateRay[] {
  return CAMERAS_M.flatMap((cameraM) => {
    const o = { x: 0, y: 0, z: BOTTOM_M + cameraM };
    return TANGENTS_M.flatMap((tangentM) => {
      const alpha = Math.asin((BOTTOM_M + tangentM) / o.z);
      const d = { x: Math.sin(alpha), y: 0, z: -Math.cos(alpha) };
      const shell = sphereHits(o, d, TOP_M);
      if (shell === null) {
        throw new Error("a high camera's limb ray misses the top");
      }
      const q = add(o, scale(d, -dot(o, d)));
      return SUNS.map(([zenithDeg, azimuthDeg]) => ({
        family: "high limb" as const,
        name: `${cameraM / 1_000} km, tangent ${tangentM / 1_000} km, sun ${zenithDeg}°/${azimuthDeg}°`,
        o,
        d,
        s: sunAt(q, d, zenithDeg, azimuthDeg),
        tStartM: 0,
        tEndM: shell[1],
        ground: false,
        sunPoint: q,
        sunZenithDeg: zenithDeg,
      }));
    });
  });
}

/** Which kernel's placement a ray's scheme pixel takes. */
type Kernel = "sky view" | "march";

/**
 * A ray's pixel through the twin of `medium`, as `tablesCpu.test.ts`'s `twinPixel` and
 * `marchGate.test.ts`'s `pixelOf` take it: the references' R05.T12.e placement, and the kernel's
 * for the schemes, the sky view's V1 (the camera side toward the camera) or the ray march's sky
 * (the sun-aware warp), each cut at the medium's layer edges.
 *
 * @throws Error for even steps, which no scheme here takes.
 */
function pixelOf(medium: AtmosphereMedium, sources: TwinSources, kernel: Kernel) {
  const edges = mediumMarchEdgesM(medium);
  return (ray: GateRay, placement: Placement, n: number): Rgb3 => {
    const twinRay: TwinRay = {
      originM: ray.o,
      direction: ray.d,
      sun: ray.s,
      tStartM: ray.tStartM,
      tEndM: ray.tEndM,
      fromCamera: ray.tStartM <= 0,
    };
    let steps: MarchStep[];
    switch (placement) {
      case "placed":
        steps = piecedSteps(twinRay, n, edges, GATE_SPHERE);
        break;
      case "kernel":
        steps = piecedSteps(
          kernel === "sky view"
            ? { ...twinRay, skySegment: false }
            : { ...twinRay, fromCamera: false, skySegment: true },
          n,
          edges,
          GATE_SPHERE,
        );
        break;
      case "even":
        throw new Error("no scheme here takes even steps");
    }
    const { luminance: l } = marchTwin(medium, sources, GATE_SPHERE, twinRay, steps);
    return [l[0], l[1], l[2]];
  };
}

/** A setting's counts as the ray march takes them: its march count for every ray here. */
const marchScheme = (scheme: Scheme): Scheme => ({ ...scheme, skySteps: scheme.marchSteps });

/**
 * Each kernel's schemes at high's and low's counts, and the bound of `LOW_TWIN_WORST` its low
 * pixels are held to: the sky view's from 60–100 km, and the ray march's.
 */
const SCHEMES = {
  "sky view": { high: HIGH, low: LOW, lowBound: LOW_TWIN_WORST.skyViewHigh },
  march: { high: marchScheme(HIGH), low: marchScheme(LOW), lowBound: LOW_TWIN_WORST.rayMarch },
} as const;

/**
 * A kernel's figures at one setting: its worst e at each of {@link ZENITHS_DEG}, and the ray of
 * its worst.
 */
interface WorstRecord {
  readonly byZenith: readonly [number, number, number, number, number];
  readonly at: string;
}

/** What a medium records, measured 2026-10-21. */
interface NightRecord {
  readonly worst: Readonly<Record<`${Kernel}, ${"high" | "low"}`, WorstRecord>>;
  /** The rays over low's bound, the sky view's against `skyViewHigh` and the march's `rayMarch`. */
  readonly overLow: readonly string[];
}

const MEDIA: ReadonlyArray<{
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots | undefined;
  readonly record: NightRecord;
}> = [
  {
    name: "R05's Earth",
    medium: R05_EARTH_REFERENCE,
    curves: undefined,
    // The sky view's night side under V1 holds at 75 steps, its worst 1.55% at 100°; at 16 it
    // reaches 39.25%, inside `skyViewHigh`'s 49.5%. The march's worst at 36 is 4.69% (99.999 km,
    // tangent 0.84 km, sun 100°/180°). At 30° and 90° both kernels' figures are the gate's own
    // (`tablesCpu.test.ts`'s `high limb` at 0.31% and 1.28% on high; `marchGate.test.ts`'s sky at
    // 0.59% and 4.32% on high, 24.00% on low).
    record: {
      worst: {
        "sky view, high": {
          byZenith: [0.00309, 0.01278, 0.00907, 0.01548, 0.00597],
          at: "80 km, tangent 8.4 km, sun 100°/0°",
        },
        "sky view, low": {
          byZenith: [0.07853, 0.18621, 0.27301, 0.39249, 0.04389],
          at: "80 km, tangent 8.4 km, sun 100°/0°",
        },
        "march, high": {
          byZenith: [0.00593, 0.04317, 0.01779, 0.04688, 0.00648],
          at: "99.999 km, tangent 0.84 km, sun 100°/180°",
        },
        "march, low": {
          byZenith: [0.03147, 0.24005, 0.12102, 0.22313, 0.03125],
          at: "80 km, tangent 0.84 km, sun 90°/0°",
        },
      },
      // One of the gate's own high-limb rays (its suns are at 30° and 90°), which
      // `marchGate.test.ts` holds to the sky view's bound by its family: `LOW_TWIN_WORST.rayMarch`
      // was measured over the gate's ray-march families alone, and the ray stands 0.005 points
      // over its 24.0%.
      overLow: ["march: 80 km, tangent 0.84 km, sun 90°/0°: 24.00%"],
    },
  },
  {
    name: "the measured Earth",
    medium: EARTH_REFERENCE,
    curves: EARTH_REFERENCE_CURVES,
    // The sky view's night side under V1 holds at 75 steps, its worst 1.85% at 100°; at 16 it
    // reaches 44.89%, inside `skyViewHigh`'s 49.5%. The march's worst at 36 is 4.47%.
    record: {
      worst: {
        "sky view, high": {
          byZenith: [0.00209, 0.01301, 0.00689, 0.01853, 0.00057],
          at: "99.999 km, tangent 0.84 km, sun 100°/180°",
        },
        "sky view, low": {
          byZenith: [0.07932, 0.23748, 0.33323, 0.44889, 0.01222],
          at: "99.999 km, tangent 8.4 km, sun 100°/180°",
        },
        "march, high": {
          byZenith: [0.00529, 0.03295, 0.01612, 0.04469, 0.00062],
          at: "99.999 km, tangent 0.84 km, sun 100°/180°",
        },
        "march, low": {
          byZenith: [0.02914, 0.18242, 0.13008, 0.22363, 0.0042],
          at: "99.999 km, tangent 0.84 km, sun 100°/180°",
        },
      },
      overLow: [],
    },
  },
];

const KERNELS: readonly Kernel[] = ["sky view", "march"];
const SETTING_NAMES = ["high", "low"] as const;

for (const { name, medium, curves, record } of MEDIA) {
  describe(`the sky view's high limb on the night side, on ${name} (R08.T8.b)`, () => {
    const gates = new Map<Kernel, Gate>();
    const gateOf = (kernel: Kernel): Gate => {
      const gate = gates.get(kernel);
      if (gate === undefined) {
        throw new Error(`the ${kernel}'s gate runs before the tests read it`);
      }
      return gate;
    };

    // The tables and both gates took 6 s on R05's Earth and 12 s on the measured Earth at four
    // workers on a loaded machine (2026-10-21); the limit is ten times the longer.
    beforeAll(() => {
      const sources = tableSources(planetTwin(medium, BOTTOM_M, curves));
      // Both kernels' references are the same placed march, taken once.
      const placed = memoised(
        ({ ray, n }: { readonly ray: GateRay; readonly n: number }) =>
          pixelOf(medium, sources, "sky view")(ray, "placed", n),
        ({ ray, n }) => `${ray.name} ${n}`,
      );
      const rays = nightRays();
      for (const kernel of KERNELS) {
        const own = pixelOf(medium, sources, kernel);
        const { high, low } = SCHEMES[kernel];
        const pixel = (ray: GateRay, placement: Placement, n: number): Rgb3 =>
          placement === "placed" ? placed({ ray, n }) : own(ray, placement, n);
        gates.set(kernel, runGate(rays, pixel, [high, low]));
      }
    }, 120_000);

    it("has a reference of 2,048 placed steps within 0.05% of 4,096 on every ray", () => {
      const gate = gateOf("sky view");
      const unconverged = gate.results
        .map(({ ray, reference, confirm, referenceSteps }) => ({
          ray,
          steps: referenceSteps,
          e: errorOf(reference, confirm, gate.lMax.get(kernelOf(ray)) ?? [0, 0, 0]),
        }))
        .filter(({ steps, e }) => !(steps === REFERENCE_STEPS && e < CONVERGED));
      expect(describeFailures(unconverged)).toEqual([]);
    });

    it("records each kernel's worst by sun zenith at high's and low's counts, and its ray", () => {
      const measured = Object.fromEntries(
        KERNELS.flatMap((kernel) =>
          SETTING_NAMES.map((setting) => {
            const es = errors(gateOf(kernel), SCHEMES[kernel][setting]);
            const worst = es.reduce((w, p) => (p.e > w.e ? p : w));
            const byZenith = ZENITHS_DEG.map((zenithDeg) =>
              Math.max(...es.filter(({ ray }) => ray.sunZenithDeg === zenithDeg).map(({ e }) => e)),
            );
            return [`${kernel}, ${setting}`, { byZenith, at: worst.ray.name }];
          }),
        ),
      );
      expect(measured).toEqual(
        Object.fromEntries(
          Object.entries(record.worst).map(([key, { byZenith, at }]) => [
            key,
            { byZenith: byZenith.map((e) => expect.closeTo(e, 4)), at },
          ]),
        ),
      );
    });

    it("holds every ray within the gate's 2%, and 5% at twilight, under both kernels on high", () => {
      const missed = KERNELS.flatMap((kernel) =>
        describeFailures(failures(gateOf(kernel), SCHEMES[kernel].high)).map(
          (miss) => `${kernel}: ${miss}`,
        ),
      );
      expect(missed).toEqual([]);
    });

    it("names the rays over low's bounds, the sky view's `skyViewHigh` and the march's `rayMarch`", () => {
      const over = KERNELS.flatMap((kernel) => {
        const bound = SCHEMES[kernel].lowBound;
        return describeFailures(
          errors(gateOf(kernel), SCHEMES[kernel].low).filter(
            ({ ray, e }) => !(e <= (twilight(ray) ? bound.twilight : bound.ordinary)),
          ),
        ).map((miss) => `${kernel}: ${miss}`);
      });
      expect(over).toEqual(record.overLow);
    });
  });
}
