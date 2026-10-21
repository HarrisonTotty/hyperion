import { beforeAll, describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3 } from "../../geometry/vec3";
import {
  BOTTOM_M,
  CONVERGED,
  describeFailures,
  errorOf,
  errors,
  evenAt,
  type Gate,
  type GateRay,
  GROUND_ALBEDO,
  HIGH,
  kernelOf,
  LOW,
  marchRays,
  overLowBounds,
  type Placement,
  type RayError,
  type RayResult,
  type Rgb3,
  runGate,
  type Scheme,
  skyRays,
  skySegmentOf,
  stepsFor,
} from "../../test/atmosphereGate";
import type { QualitySetting } from "../quality/qualitySetting";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
} from "./earth";
import { TABLE_SIZES } from "./hillaire";
import { grazingTwilight, MARCH_TOLERANCE, type MarchStep, TWILIGHT_TOLERANCE } from "./marchSteps";
import type { AtmosphereMedium } from "./medium";
import { MAX_SKY_SOURCES } from "./settings";
import {
  marchTwin,
  mediumMarchEdgesM,
  piecedSteps,
  planetTwin,
  tableSources,
  type TwinRay,
  type TwinRaySource,
  type TwinSources,
} from "./tablesCpu";

/**
 * R05.T12.e's quadrature gate for the march as R08.T8 widens it (plan R08, R08.T8: "R05.T12.e's
 * quadrature gate holds for the widened march, at one source and at `MAX_SKY_SOURCES`";
 * `decision-r08-t8-leans.md` §2, §4 and §5.5): R08.T6.a's twin on R05's Earth, the measured Earth
 * and the slab Earth, over the gate's march families, the night limb among them, the sky's last
 * light, and the gate's other sky families from cameras at or above each setting's march band's
 * lower edge, which the ray march draws there, marched as the kernel marches a sky pixel (nothing
 * drawn: its two-sided segments under the sun-aware warp, its one-sided ones about the lowest
 * point), at the setting's march count; the references R05.T12.e's placed steps. The floor is each
 * kernel's family group's brightest reference over the setting's rays.
 */

/** The gate's sphere, its rays' and the twin's marches'. */
const GATE_SPHERE = { kind: "sphere", bottomRadiusM: BOTTOM_M } as const;
const RAD = Math.PI / 180;
const TOP_M = R05_EARTH_REFERENCE.topHeightM;

/**
 * The five sources beside each ray's own sun, deterministic: zenith angle at the ray's sun point,
 * azimuth from its sun's, and weight in every channel, all light slot 0 (every source takes one
 * path and, on a medium with an absorber, its one light's curves).
 */
const FIVE_MORE = [
  [20, 60, 0.5],
  [50, 120, 0.3],
  [70, 180, 0.2],
  [85, 240, 0.1],
  [92, 300, 0.05],
] as const;

/** A direction at a zenith angle at `q` and an azimuth from the sun's own there (the ray's where none). */
function sourceAt(q: Vec3, sun: Vec3, d: Vec3, zenithDeg: number, azimuthDeg: number): Vec3 {
  const n = normalise(q);
  const horizontalOf = (v: Vec3): Vec3 => add(v, scale(n, -dot(v, n)));
  const towards = horizontalOf(sun);
  const f = normalise(dot(towards, towards) > 1e-12 ? towards : horizontalOf(d));
  const y = cross(n, f);
  const horizontal = add(
    scale(f, Math.cos(azimuthDeg * RAD)),
    scale(y, Math.sin(azimuthDeg * RAD)),
  );
  return normalise(
    add(scale(n, Math.cos(zenithDeg * RAD)), scale(horizontal, Math.sin(zenithDeg * RAD))),
  );
}

/** How many sources a ray's march sums: its sun alone, or six. */
type SourceCount = 1 | typeof MAX_SKY_SOURCES;
const COUNTS = [1, MAX_SKY_SOURCES] as const;

/** A ray's sources: its sun at weight 1, and at six the five of {@link FIVE_MORE}. */
function lightsOf(ray: GateRay, count: SourceCount): TwinRaySource[] {
  const own: TwinRaySource = { direction: ray.s, weight: [1, 1, 1], slot: 0 };
  if (count === 1) {
    return [own];
  }
  return [
    own,
    ...FIVE_MORE.map(([zenithDeg, azimuthDeg, w]) => ({
      direction: sourceAt(ray.sunPoint, ray.s, ray.d, zenithDeg, azimuthDeg),
      weight: [w, w, w] as const,
      slot: 0,
    })),
  ];
}

/**
 * Whether the ray march draws a gate ray as its sky: the ray march families' with no ground (the
 * last light's among them), and here every other sky family's.
 */
const marchedAsSky = (ray: GateRay): boolean => kernelOf(ray) === "skyView" || skySegmentOf(ray);

/**
 * A ray's pixel through the twin of `medium` over its sources: the references' R05.T12.e placement,
 * the kernel's rule for the schemes (`rayMarch.wgsl`: a sky pixel's two-sided segment under the
 * sun-aware warp by its first source, the ray's sun, and a drawn surface's camera side toward the
 * camera), or even steps, each cut at the medium's layer edges as the kernels cut them; the gate's
 * lit ground through the ray's transmittance, each source's light on it.
 */
function pixelOf(medium: AtmosphereMedium, sources: TwinSources, count: SourceCount) {
  const edges = mediumMarchEdgesM(medium);
  return (ray: GateRay, placement: Placement, n: number): Rgb3 => {
    const lights = lightsOf(ray, count);
    const sky = marchedAsSky(ray);
    const twinRay: TwinRay = {
      originM: ray.o,
      direction: ray.d,
      sun: ray.s,
      sources: lights,
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
          { ...twinRay, fromCamera: twinRay.fromCamera && !sky, skySegment: sky },
          n,
          edges,
          GATE_SPHERE,
        );
        break;
      case "even":
        steps = stepsFor(placement, ray, n);
        break;
    }
    const { luminance: l, transmittance: t } = marchTwin(
      medium,
      sources,
      GATE_SPHERE,
      twinRay,
      steps,
    );
    const out: Rgb3 = [l[0], l[1], l[2]];
    if (!ray.ground) {
      return out;
    }
    const end = normalise(add(ray.o, scale(ray.d, ray.tEndM)));
    const toSun: Rgb3 = [0, 0, 0];
    for (const light of lights) {
      const mu = dot(end, light.direction);
      sources.sunTransmittance(0, mu, toSun, light.slot);
      for (const c of [0, 1, 2] as const) {
        out[c] += t[c] * light.weight[c] * (GROUND_ALBEDO / Math.PI) * Math.max(mu, 0) * toSun[c];
      }
    }
    return out;
  };
}

const cameraHeightM = (ray: GateRay): number => Math.hypot(ray.o.x, ray.o.y, ray.o.z) - BOTTOM_M;

/** A setting's rays: the march's, and the sky's from cameras at or above its band's lower edge. */
function raysOf(setting: QualitySetting): GateRay[] {
  const lowerM = TABLE_SIZES[setting].marchBand.lowerShareOfTop * TOP_M;
  return [...marchRays(), ...skyRays().filter((ray) => cameraHeightM(ray) >= lowerM)];
}

/** The setting's counts as the ray march takes them: its march count for every ray here. */
function marchScheme(setting: QualitySetting): Scheme {
  const scheme = setting === "high" ? HIGH : LOW;
  return { ...scheme, skySteps: scheme.marchSteps };
}

/** The setting's rays of a gate, each kernel group's floor its own brightest reference. */
function gateOf(gate: Gate, setting: QualitySetting): Gate {
  const names = new Set(raysOf(setting).map((ray) => ray.name));
  const results = gate.results.filter(({ ray }) => names.has(ray.name));
  const lMax = new Map<"rayMarch" | "skyView", Rgb3>();
  for (const kernel of ["rayMarch", "skyView"] as const) {
    const max: Rgb3 = [0, 0, 0];
    for (const { ray, reference } of results) {
      if (kernelOf(ray) === kernel) {
        for (const c of [0, 1, 2] as const) {
          max[c] = Math.max(max[c], reference[c]);
        }
      }
    }
    lMax.set(kernel, max);
  }
  return { results, lMax };
}

/** Whether any of a ray's sources makes it a twilight ray. */
const twilightOf =
  (count: SourceCount) =>
  (ray: GateRay): boolean =>
    lightsOf(ray, count).some((light) =>
      grazingTwilight(ray.o, ray.d, light.direction, ray.tStartM, ray.tEndM),
    );

/** The group a ray's worst is recorded in: the march's families, the last light, or the sky's others. */
function groupOf(ray: GateRay): "march" | "last light" | "sky" {
  if (ray.family === "last light") {
    return "last light";
  }
  return kernelOf(ray) === "rayMarch" ? "march" : "sky";
}

/** The worst e of each group ({@link groupOf}) and class. */
function worstByGroup(
  es: readonly RayError[],
  twilight: (ray: GateRay) => boolean,
): Record<string, number> {
  const worst: Record<string, number> = {};
  for (const { ray, e } of es) {
    const key = `${groupOf(ray)} ${twilight(ray) ? "twilight" : "ordinary"}`;
    worst[key] = Math.max(worst[key] ?? 0, e);
  }
  return worst;
}

/**
 * A setting's worst figures as recorded, each within 5 × 10⁻⁵ (0.005 percentage points): the
 * march's families by class, the other sky families' by class, and the last light's (every one a
 * twilight ray).
 */
interface GroupFigures {
  readonly march: number;
  readonly marchTwilight: number;
  readonly sky: number;
  readonly skyTwilight: number;
  readonly lastLight: number;
}

const recorded = (f: GroupFigures): unknown => ({
  "march ordinary": expect.closeTo(f.march, 4),
  "march twilight": expect.closeTo(f.marchTwilight, 4),
  "sky ordinary": expect.closeTo(f.sky, 4),
  "sky twilight": expect.closeTo(f.skyTwilight, 4),
  "last light twilight": expect.closeTo(f.lastLight, 4),
});

/** A medium's figures at one count: under the kernel's rule and with even steps throughout. */
interface CountFigures {
  readonly kernel: GroupFigures;
  readonly even: GroupFigures;
}

/** What the gate records on a medium, measured 2026-10-21 (plan R08's Risks, "Deviations in T8.a, as built"). */
interface MediumRecord {
  /** The rays high misses, at one source and at six: named, to the 0.01 point both ways. */
  readonly highMisses: readonly [readonly string[], readonly string[]];
  readonly high: readonly [CountFigures, CountFigures];
  readonly low: readonly [CountFigures, CountFigures];
  /** The rays over `LOW_TWIN_WORST` marched at low's 16 steps, at one source and at six. */
  readonly overLow: readonly [readonly string[], readonly string[]];
}

/** The media the widened march is held on, their twins' sources, and what each records. */
const MEDIA: ReadonlyArray<{
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly sources: () => TwinSources;
  readonly record: MediumRecord;
}> = [
  {
    name: "R05's Earth",
    medium: R05_EARTH_REFERENCE,
    sources: () => tableSources(planetTwin(R05_EARTH_REFERENCE, BOTTOM_M)),
    // At one source on high the night limb sets the march's twilight worst (4.69%, from 400 km at
    // 0.1 H, sun 100°/180°; 4.79% in even steps alone) and the high limb from 99.999 km the sky's
    // (4.32%); the last light's is 0.79%, where even steps put it at 12.47%. On low the high limb
    // from 80 km at tangent 0.84 km, sun 90°/0°, sets the sky's (24.00%, against even steps' 46.37%
    // on the band from 50 km), and no ray stands over `LOW_TWIN_WORST`: the two band rays from 50 km
    // that stood over the sky view's bound in even steps (19.44% and 46.37%) fall under it.
    record: {
      highMisses: [[], []],
      high: [
        {
          kernel: {
            march: 0.00447,
            marchTwilight: 0.04687,
            sky: 0.00593,
            skyTwilight: 0.04317,
            lastLight: 0.00788,
          },
          even: {
            march: 0.05739,
            marchTwilight: 0.04786,
            sky: 0.01028,
            skyTwilight: 0.04317,
            lastLight: 0.12466,
          },
        },
        {
          kernel: {
            march: 0.00332,
            marchTwilight: 0.00597,
            sky: 0.00074,
            skyTwilight: 0.00634,
            lastLight: 0.00457,
          },
          even: {
            march: 0.03459,
            marchTwilight: 0.00766,
            sky: 0.00112,
            skyTwilight: 0.00977,
            lastLight: 0.00509,
          },
        },
      ],
      low: [
        {
          kernel: {
            march: 0.02589,
            marchTwilight: 0.23945,
            sky: 0.0331,
            skyTwilight: 0.24005,
            lastLight: 0.04502,
          },
          even: {
            march: 0.27464,
            marchTwilight: 0.23945,
            sky: 0.05183,
            skyTwilight: 0.46367,
            lastLight: 0.35943,
          },
        },
        {
          kernel: {
            march: 0.01689,
            marchTwilight: 0.02736,
            sky: 0.00374,
            skyTwilight: 0.03315,
            lastLight: 0.02344,
          },
          even: {
            march: 0.19377,
            marchTwilight: 0.03137,
            sky: 0.00565,
            skyTwilight: 0.04859,
            lastLight: 0.02716,
          },
        },
      ],
      overLow: [[], []],
    },
  },
  {
    name: "the measured Earth",
    medium: EARTH_REFERENCE,
    sources: () => tableSources(planetTwin(EARTH_REFERENCE, BOTTOM_M, EARTH_REFERENCE_CURVES)),
    record: {
      highMisses: [[], []],
      high: [
        {
          kernel: {
            march: 0.00528,
            marchTwilight: 0.03239,
            sky: 0.00529,
            skyTwilight: 0.03295,
            lastLight: 0.00554,
          },
          even: {
            march: 0.05093,
            marchTwilight: 0.03239,
            sky: 0.00883,
            skyTwilight: 0.03295,
            lastLight: 0.16369,
          },
        },
        {
          kernel: {
            march: 0.00293,
            marchTwilight: 0.00396,
            sky: 0.00073,
            skyTwilight: 0.00434,
            lastLight: 0.0022,
          },
          even: {
            march: 0.0282,
            marchTwilight: 0.00634,
            sky: 0.00128,
            skyTwilight: 0.00828,
            lastLight: 0.0034,
          },
        },
      ],
      low: [
        {
          kernel: {
            march: 0.0359,
            marchTwilight: 0.22354,
            sky: 0.02914,
            skyTwilight: 0.18242,
            lastLight: 0.03705,
          },
          even: {
            march: 0.25225,
            marchTwilight: 0.24471,
            sky: 0.04348,
            skyTwilight: 0.28025,
            lastLight: 0.35647,
          },
        },
        {
          kernel: {
            march: 0.01461,
            marchTwilight: 0.02526,
            sky: 0.00368,
            skyTwilight: 0.03032,
            lastLight: 0.01852,
          },
          even: {
            march: 0.16568,
            marchTwilight: 0.0248,
            sky: 0.00648,
            skyTwilight: 0.04029,
            lastLight: 0.01955,
          },
        },
      ],
      overLow: [[], []],
    },
  },
  {
    name: "the slab Earth",
    medium: EARTH_SLAB_REFERENCE,
    sources: () =>
      tableSources(planetTwin(EARTH_SLAB_REFERENCE, BOTTOM_M, EARTH_SLAB_REFERENCE_CURVES)),
    // At one source high misses the limb's and night limb's rays `tablesCpu.test.ts`'s
    // `SLAB_HIGH_MISS` names, and two of the sky's from 99.999 km that the ray march draws there:
    // the high band at view zenith 99.879°, sun 90°/180° (6.44% under the warp, a = 1; 6.74% in
    // §4's even steps), and the high limb at tangent 0.84 km, sun 90°/0° (5.25%, a = 0, as in §4).
    // Neither is the warp's: both stood over 5% under §4's even steps, which this gate did not run
    // on the slab Earth before (measured 2026-10-21; `decision-r08-t8-leans.md` §4.3, named misses
    // reported to main). No tolerance or count moves. `LOW_TWIN_WORST` is not re-pinned on a test
    // medium (`decision-r08-t6d-findings.md` §10.4.3): the rays over it are recorded.
    record: {
      highMisses: [
        [
          "limb, 0.1 H, sun 90°/0°: 5.15%",
          "night limb, 400 km, 0.3 H, sun 100°/180°: 6.27%",
          "night limb, 101 km, 0.1 H, sun 90°/0°: 5.15%",
          "night limb, 101 km, 0.3 H, sun 100°/180°: 6.27%",
          "high band, 99999 m, view zenith 99.879°, sun 90°/180°: 6.44%",
          "high limb, 99.999 km, tangent 0.84 km, sun 90°/0°: 5.25%",
        ],
        [],
      ],
      high: [
        {
          kernel: {
            march: 0.01121,
            marchTwilight: 0.06275,
            sky: 0.01188,
            skyTwilight: 0.06439,
            lastLight: 0.00553,
          },
          even: {
            march: 0.16263,
            marchTwilight: 0.20829,
            sky: 0.16369,
            skyTwilight: 0.20916,
            lastLight: 0.16461,
          },
        },
        {
          kernel: {
            march: 0.00728,
            marchTwilight: 0.00963,
            sky: 0.00073,
            skyTwilight: 0.01124,
            lastLight: 0.00913,
          },
          even: {
            march: 0.02186,
            marchTwilight: 0.16481,
            sky: 0.00128,
            skyTwilight: 0.16305,
            lastLight: 0.17249,
          },
        },
      ],
      low: [
        {
          kernel: {
            march: 0.0597,
            marchTwilight: 0.28556,
            sky: 0.05435,
            skyTwilight: 0.28562,
            lastLight: 0.07273,
          },
          even: {
            march: 0.30338,
            marchTwilight: 0.44193,
            sky: 0.30292,
            skyTwilight: 0.49632,
            lastLight: 0.35863,
          },
        },
        {
          kernel: {
            march: 0.02603,
            marchTwilight: 0.06909,
            sky: 0.00368,
            skyTwilight: 0.04765,
            lastLight: 0.03925,
          },
          even: {
            march: 0.23908,
            marchTwilight: 0.30509,
            sky: 0.00648,
            skyTwilight: 0.30465,
            lastLight: 0.37626,
          },
        },
      ],
      overLow: [
        [
          "disc, ground zenith 60°, sun 80°/0°: 5.97%",
          "disc, ground zenith 80°, sun 80°/0°: 3.78%",
          "limb, 0.1 H, sun 90°/0°: 28.56%",
          "limb, 0.3 H, sun 30°/180°: 4.11%",
          "limb, 1 H, sun 30°/180°: 4.10%",
          "night limb, 400 km, 0.1 H, sun 95°/180°: 27.51%",
          "night limb, 400 km, 0.3 H, sun 95°/180°: 24.12%",
          "night limb, 101 km, 0.1 H, sun 90°/0°: 28.56%",
          "night limb, 101 km, 0.1 H, sun 95°/180°: 27.51%",
          "night limb, 101 km, 0.3 H, sun 95°/180°: 24.12%",
          "band, 50000 m, view zenith 97.008°, sun 90°/180°: 18.23%",
        ],
        ["limb, 0.3 H, sun 30°/180°: 3.95%", "limb, 1 H, sun 30°/180°: 4.12%"],
      ],
    },
  },
];

it("covers the march's 268 rays, the night limb's among them, the last light's 525, and the sky's others 278 from high's band's cameras and 358 from low's", () => {
  const counts = (["high", "low"] as const).map((setting) => {
    const rays = raysOf(setting);
    return [
      rays.filter((ray) => kernelOf(ray) === "rayMarch" && ray.family !== "last light").length,
      rays.filter((ray) => ray.family === "last light").length,
      rays.length,
    ];
  });
  expect(counts).toEqual([
    [268, 525, 1_071],
    [268, 525, 1_151],
  ]);
});

describe.each(MEDIA)(
  "R05.T12.e's quadrature gate for the widened march on $name (R08.T8)",
  ({ medium, sources, record }) => {
    const gates = new Map<SourceCount, Gate>();
    const gateAt = (count: SourceCount, setting: QualitySetting): Gate => {
      const gate = gates.get(count);
      if (gate === undefined) {
        throw new Error(`no gate at ${count} sources`);
      }
      return gateOf(gate, setting);
    };

    beforeAll(() => {
      const twinSources = sources();
      const schemes = [
        marchScheme("high"),
        evenAt(marchScheme("high")),
        marchScheme("low"),
        evenAt(marchScheme("low")),
      ];
      for (const count of COUNTS) {
        gates.set(count, runGate(raysOf("low"), pixelOf(medium, twinSources, count), schemes));
      }
    }, 600_000);

    it("has every reference of 2,048–8,192 placed steps within 0.05% of twice them, at one source and at six", () => {
      const unconverged = COUNTS.flatMap((count) => {
        const gate = gateAt(count, "low");
        return gate.results
          .filter(
            ({ ray, reference, confirm }: RayResult) =>
              !(errorOf(reference, confirm, gate.lMax.get(kernelOf(ray)) ?? [0, 0, 0]) < CONVERGED),
          )
          .map(({ ray }) => `${count}: ${ray.name}`);
      });
      expect(unconverged).toEqual([]);
    });

    it("holds every ray within 2%, and twilight rays within 5%, at high's counts, at one source and at six, but its named misses", () => {
      const failed = COUNTS.map((count) => {
        const twilight = twilightOf(count);
        return describeFailures(
          errors(gateAt(count, "high"), marchScheme("high")).filter(
            ({ ray, e }) => !(e <= (twilight(ray) ? TWILIGHT_TOLERANCE : MARCH_TOLERANCE)),
          ),
        );
      });
      expect(failed).toEqual(record.highMisses.map((misses) => [...misses]));
    });

    it("records high's worst by group and class beside even steps', at one source and at six", () => {
      const figures = COUNTS.map((count) => {
        const gate = gateAt(count, "high");
        const twilight = twilightOf(count);
        return {
          kernel: worstByGroup(errors(gate, marchScheme("high")), twilight),
          even: worstByGroup(errors(gate, evenAt(marchScheme("high"))), twilight),
        };
      });
      expect(figures).toEqual(
        record.high.map(({ kernel, even }) => ({ kernel: recorded(kernel), even: recorded(even) })),
      );
    });

    // R05.T12.e's low gate (addendum A, 4(a)).
    it.each(COUNTS)(
      "leaves low's worst ray over the set no worse than even steps' at low's counts with %i source(s)",
      (count) => {
        const gate = gateAt(count, "low");
        const worst = (scheme: Scheme): number =>
          Math.max(...errors(gate, scheme).map(({ e }) => e));
        expect(worst(marchScheme("low"))).toBeLessThanOrEqual(worst(evenAt(marchScheme("low"))));
      },
    );

    it("records low's worst by group and class beside even steps', at one source and at six", () => {
      const figures = COUNTS.map((count) => {
        const gate = gateAt(count, "low");
        const twilight = twilightOf(count);
        return {
          kernel: worstByGroup(errors(gate, marchScheme("low")), twilight),
          even: worstByGroup(errors(gate, evenAt(marchScheme("low"))), twilight),
        };
      });
      expect(figures).toEqual(
        record.low.map(({ kernel, even }) => ({ kernel: recorded(kernel), even: recorded(even) })),
      );
    });

    // `LOW_TWIN_WORST` holds the march's families, the last light's among them, to the ray march's
    // bounds, and the other sky families to the sky view's.
    it("records the rays over the low bounds, marched at low's 16 steps", () => {
      const over = COUNTS.map((count) =>
        describeFailures(overLowBounds(gateAt(count, "low"), marchScheme("low"))),
      );
      expect(over).toEqual(record.overLow.map((rays) => [...rays]));
    });
  },
);
