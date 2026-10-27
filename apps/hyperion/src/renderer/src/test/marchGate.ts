/**
 * R05.T12.e's quadrature gate for the march as R08.T8 widens it (plan R08, R08.T8: "R05.T12.e's
 * quadrature gate holds for the widened march, at one source and at `MAX_SKY_SOURCES`";
 * `decision-r08-t8-leans.md` §2, §4 and §5.5): R08.T6.a's twin on R05's Earth, the measured Earth
 * and the slab Earth, over the gate's march families, the night limb among them, the sky's last
 * light, and the gate's other sky families from cameras at or above each setting's march band's
 * lower edge, which the ray march draws there, marched as the kernel marches a sky pixel (nothing
 * drawn: its two-sided segments under the sun-aware warp, its one-sided ones about the lowest
 * point), at the setting's march count, the bare ground's and the thin sky's scaled since R08.T8.e by
 * their optical depth as the kernel scales it (`TableSizes`' `marchDepthSteps`, high's; `atmosphereGate.ts`'s
 * `gateMarchSamples`); the references R05.T12.e's placed steps. The floor is each kernel's family
 * group's brightest reference over the setting's rays.
 *
 * Since R08.T11.b (`decision-r08-t11-frame.md` §2, lever A) the march's families gain its drawn
 * surfaces (`atmosphereGate.ts`'s `drawnSurfaceRays`), on the datum and at the band's edge, which
 * high's march counts as the bare ground, as it does the inside and near-level families' terrain:
 * each scored on its in-scatter and its transmittance together over its surface at the gate's
 * albedo, lit at its own height.
 *
 * @remarks
 * Moved here from `view/atmosphere/marchGate.test.ts` (2026-10-22, main's infra ruling) and split
 * by medium, so that Vitest runs the three gates side by side, on three workers:
 * `marchGate.test.ts` holds R05's Earth and the rays' count, `marchGateMeasured.test.ts` the
 * measured Earth and `marchGateSlab.test.ts` the slab Earth, each its medium's record and its
 * {@link describeMarchGate}.
 */

import { beforeAll, describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3 } from "../geometry/vec3";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import { TABLE_SIZES } from "../view/atmosphere/hillaire";
import {
  grazingTwilight,
  MARCH_TOLERANCE,
  type MarchDepthSteps,
  type MarchStep,
  TWILIGHT_TOLERANCE,
} from "../view/atmosphere/marchSteps";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import { MAX_SKY_SOURCES } from "../view/atmosphere/settings";
import {
  marchTwin,
  mediumMarchEdgesM,
  piecedSteps,
  type PlanetTwin,
  rayMarchSunReadOf,
  tableSources,
  type TwinRay,
  type TwinRaySource,
  type TwinSources,
} from "../view/atmosphere/tablesCpu";
import type { QualitySetting } from "../view/quality/qualitySetting";
import {
  BOTTOM_M,
  CONVERGED,
  describeFailures,
  drawnSurfaceRays,
  errorOf,
  errors,
  evenAt,
  type Gate,
  type GateRay,
  gateMarchSamples,
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
} from "./atmosphereGate";

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
 * lit ground through the ray's transmittance, each source's light on it. Under the kernel's rule the
 * bare ground's and the thin sky's count is scaled by its optical depth where the scheme gives a rule (R08.T8.e;
 * `gateMarchSamples`), read through the twin's own transmittance.
 *
 * Since R08.T8.e (lever 5, `decision-r08-t8-leans.md` §10 and §13, point 7) both schemes, `kernel`
 * and `even`, take the ray march's read of each source's light and the lit ground's, `slotSources`
 * (by slot where the kernel's tables hold the slot layers), while the `placed` references keep the
 * curves' read, `sources` (§4.2's rule), and the count keeps it in both.
 */
function pixelOf(
  medium: AtmosphereMedium,
  sources: TwinSources,
  slotSources: TwinSources,
  count: SourceCount,
) {
  const edges = mediumMarchEdgesM(medium);
  return (ray: GateRay, placement: Placement, n: number, rule: MarchDepthSteps | null): Rgb3 => {
    const lights = lightsOf(ray, count);
    const read = placement === "placed" ? sources : slotSources;
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
          gateMarchSamples(ray, n, rule, edges.length, sources.sunTransmittance),
          edges,
          GATE_SPHERE,
        );
        break;
      case "even":
        steps = stepsFor(placement, ray, n);
        break;
    }
    const { luminance: l, transmittance: t } = marchTwin(medium, read, GATE_SPHERE, twinRay, steps);
    const out: Rgb3 = [l[0], l[1], l[2]];
    if (!ray.ground) {
      return out;
    }
    const end = normalise(add(ray.o, scale(ray.d, ray.tEndM)));
    const toSun: Rgb3 = [0, 0, 0];
    for (const light of lights) {
      // A surface drawn in the air is lit at its own height (R08.T11.b).
      const mu = dot(end, light.direction);
      read.sunTransmittance(ray.drawnHeightM ?? 0, mu, toSun, light.slot);
      for (const c of [0, 1, 2] as const) {
        out[c] += t[c] * light.weight[c] * (GROUND_ALBEDO / Math.PI) * Math.max(mu, 0) * toSun[c];
      }
    }
    return out;
  };
}

const cameraHeightM = (ray: GateRay): number => Math.hypot(ray.o.x, ray.o.y, ray.o.z) - BOTTOM_M;

/**
 * A setting's rays: the march's, its drawn surfaces since R08.T11.b, and the sky's from cameras at
 * or above its band's lower edge.
 */
export function raysOf(setting: QualitySetting): GateRay[] {
  const lowerM = TABLE_SIZES[setting].marchBand.lowerShareOfTop * TOP_M;
  return [
    ...marchRays(),
    ...drawnSurfaceRays(),
    ...skyRays().filter((ray) => cameraHeightM(ray) >= lowerM),
  ];
}

/**
 * The setting's counts as the ray march takes them: its march count for every ray here, the bare
 * ground's and the thin sky's scaled by their optical depth under the setting's rule (R08.T8.e).
 */
function marchScheme(setting: QualitySetting): Scheme {
  const scheme = setting === "high" ? HIGH : LOW;
  return { ...scheme, skySteps: scheme.marchSteps, skyDepthSteps: scheme.marchDepthSteps };
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
export interface GroupFigures {
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
export interface CountFigures {
  readonly kernel: GroupFigures;
  readonly even: GroupFigures;
}

/** What the gate records on a medium, measured 2026-10-21 (plan R08's Risks, "Deviations in T8.a, as built"). */
export interface MediumRecord {
  /** The rays high misses, at one source and at six: named, to the 0.01 point both ways. */
  readonly highMisses: readonly [readonly string[], readonly string[]];
  readonly high: readonly [CountFigures, CountFigures];
  readonly low: readonly [CountFigures, CountFigures];
  /** The rays over `LOW_TWIN_WORST` marched at low's 16 steps, at one source and at six. */
  readonly overLow: readonly [readonly string[], readonly string[]];
}

/**
 * A medium the widened march is held on, its twin's per-planet tables, whose sources the gate reads
 * under the curves and the ray march's read (R08.T8.e), and what it records.
 */
export interface MarchGateMedium {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly planet: () => PlanetTwin;
  readonly record: MediumRecord;
}

/**
 * Registers R05.T12.e's quadrature gate for the widened march on one medium: a block whose
 * `beforeAll` runs the gate at one source and at {@link MAX_SKY_SOURCES}, and whose seven tests
 * compare the figures with the medium's record.
 *
 * @remarks
 * Each test file calls it once, at its top level, for its own medium, so that Vitest runs each
 * medium's gate on a worker of its own.
 */
export function describeMarchGate({ name, medium, planet, record }: MarchGateMedium): void {
  describe(`R05.T12.e's quadrature gate for the widened march on ${name} (R08.T8)`, () => {
    const gates = new Map<SourceCount, Gate>();
    const gateAt = (count: SourceCount, setting: QualitySetting): Gate => {
      const gate = gates.get(count);
      if (gate === undefined) {
        throw new Error(`no gate at ${count} sources`);
      }
      return gateOf(gate, setting);
    };

    beforeAll(() => {
      const twin = planet();
      const twinSources = tableSources(twin);
      const slotSources = tableSources(twin, rayMarchSunReadOf(twin));
      const schemes = [
        marchScheme("high"),
        evenAt(marchScheme("high")),
        marchScheme("low"),
        evenAt(marchScheme("low")),
      ];
      for (const count of COUNTS) {
        gates.set(
          count,
          runGate(raysOf("low"), pixelOf(medium, twinSources, slotSources, count), schemes),
        );
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
  });
}
