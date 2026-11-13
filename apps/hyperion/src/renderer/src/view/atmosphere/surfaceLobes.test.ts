import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { patternedThickBake } from "../../test/thickTables";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { Rgb } from "../photometry/toneCurve";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, referenceSunIlluminanceLx } from "./earth";
import { forwardLobeOfTable, lobeIsDrawn } from "./forwardLobe";
import {
  type AtmosphereCamera,
  RAY_MARCH_DECK_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_DECK_KERNEL,
  RAY_MARCH_THICK_KERNEL,
  type SpheroidFigure,
  type SunState,
  type SurfaceLobes,
  TABLE_SIZES,
} from "./hillaire";
import type { AtmosphereMedium, MediumTerm } from "./medium";
import { MARS_REFERENCE, MARS_REFERENCE_RADIUS_M } from "./reference/cases";
import type { ReferenceGeometry } from "./reference/geometries";
import { referenceTwin } from "./reference/twin";
import rayMarchWgsl from "./shaders/rayMarch.wgsl?raw";
import {
  type AbsorberCurveSlots,
  kernelMedium,
  NO_ABSORBER_CURVES,
  resampledPhase,
} from "./tables";
import {
  marchTwin,
  type PlanetTwin,
  placedSteps,
  rayMarchRayAlongTwin,
  rayMarchTexelsTwin,
  tableSources,
  transmittanceTablesTwin,
  type TwinMarch,
  type TwinMarchedTexel,
  type TwinSources,
  twinView,
} from "./tablesCpu";
import type { ThickTableKind } from "./thick/read";

/**
 * A drawn surface seen through the air keeps its light scattered within the medium's forward lobes
 * (plan R08, fix-r08-decked-air-opacity; `hillaire.ts`'s `SurfaceLobes`): the ray march's second
 * target with each drawn lobe's in-lobe scattering dropped from its extinction, R08.T7's δ-M
 * (Wiscombe 1977, J. Atmos. Sci. 34, 1408), held to a converged march of the δ-M medium itself, the
 * lobe terms' scattering and extinction less P σ_s, within 1% of each channel's optical depth
 * (`surfaceTransmittance.test.ts`'s measure). Mars's dust is the medium with a drawn lobe; the
 * measured Earth draws none.
 */

const CHANNELS = [0, 1, 2] as const;

/** The converged march's steps. */
const CONVERGED_STEPS = 4_096;

/** The most |ln T − ln T_ref| ÷ max(1, −ln T_ref) may reach in a channel. */
const TOLERANCE = 0.01;

/** The sun, high over the camera's meridian: a transmittance does not depend on it. */
const SUN: SunState = {
  directionBodyFixed: { x: Math.SQRT1_2, y: 0, z: Math.SQRT1_2 },
  illuminanceLx: referenceSunIlluminanceLx(),
  angularRadiusRad: 0.004_650_5,
};

/** A medium on its sphere and the tables the twin reads, its multiple scattering unread. */
interface Planet {
  readonly medium: AtmosphereMedium;
  readonly radiusM: number;
  readonly planet: PlanetTwin;
}

function planetOf(medium: AtmosphereMedium, radiusM: number, curves: AbsorberCurveSlots): Planet {
  return {
    medium,
    radiusM,
    planet: {
      medium,
      bottomRadiusM: radiusM,
      ...transmittanceTablesTwin(medium, radiusM, curves),
      multiScattering: { widthTexels: 2, heightTexels: 2, texels: new Float64Array(16) },
    },
  };
}

const MARS = planetOf(MARS_REFERENCE, MARS_REFERENCE_RADIUS_M, NO_ABSORBER_CURVES);

/** The measured Earth on a sphere of its mean radius, its ozone under the reference Sun's curve. */
const EARTH = planetOf(EARTH_REFERENCE, 6_371_008.8, EARTH_REFERENCE_CURVES);

/**
 * A term with its drawn lobe's in-lobe scattering taken out, as δ-M scales it: its scattering times
 * 1 − P per channel, its absorption kept; every other term as it is.
 */
function deltaScaled(term: MediumTerm): MediumTerm {
  if (term.phase.kind !== "tabulated") {
    return term;
  }
  const lobe = forwardLobeOfTable(resampledPhase(term.phase.table));
  if (!lobeIsDrawn(lobe)) {
    return term;
  }
  const kept = (c: 0 | 1 | 2): number => term.scattering[c] * (1 - lobe.share[c]);
  return { ...term, scattering: [kept(0), kept(1), kept(2)] };
}

/** A coefficient 400 times over. */
function times400(rgb: Rgb): Rgb {
  return [rgb[0] * 400, rgb[1] * 400, rgb[2] * 400];
}

/** A tabulated term 400 times as dense: Mars's dust deep enough that no throughput holds it. */
function deepened(term: MediumTerm): MediumTerm {
  if (term.phase.kind !== "tabulated") {
    return term;
  }
  return { ...term, scattering: times400(term.scattering), absorption: times400(term.absorption) };
}

/** The camera `heightM` over the equator, looking along `direction` (body-fixed). */
function cameraAt(radiusM: number, heightM: number): AtmosphereCamera {
  return {
    positionM: { x: radiusM + heightM, y: 0, z: 0 },
    orientation: IDENTITY_QUATERNION,
    fovXRad: Math.PI / 2,
    viewport: { widthPx: 64, heightPx: 32 },
  };
}

/** The march's twin texel along `direction` to a surface `surfaceM` on, under `lobes`. */
function texelOf(
  { medium, radiusM, planet }: Planet,
  heightM: number,
  direction: Vec3,
  surfaceM: number,
  lobes: SurfaceLobes,
): TwinMarchedTexel {
  const figure: SpheroidFigure = { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
  const view = twinView(cameraAt(radiusM, heightM), SUN, figure, 0.1);
  const marched = rayMarchRayAlongTwin(medium, view, view.cameraM, direction, surfaceM);
  return rayMarchTexelsTwin(planet, view, { ...TABLE_SIZES.high, surfaceLobes: lobes }, marched);
}

/** The converged march's transmittance of the δ-M medium along the same ray. */
function convergedDeltaM(
  { medium, radiusM, planet }: Planet,
  heightM: number,
  direction: Vec3,
  surfaceM: number,
): Rgb {
  const scaled: AtmosphereMedium = { ...medium, terms: medium.terms.map(deltaScaled) };
  const figure: SpheroidFigure = { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
  const view = twinView(cameraAt(radiusM, heightM), SUN, figure, 0.1);
  const clipped = rayMarchRayAlongTwin(scaled, view, view.cameraM, direction, surfaceM);
  if (clipped === null) {
    throw new Error("the ray meets the air");
  }
  return marchTwin(
    scaled,
    tableSources(planet),
    { kind: "spheroid", figure },
    clipped.ray,
    placedSteps(clipped.ray, CONVERGED_STEPS),
  ).transmittance;
}

/** The worst |ln T − ln T_ref| ÷ max(1, −ln T_ref) over the channels. */
function worstDepthError(t: ArrayLike<number>, converged: Rgb): number {
  let worst = 0;
  for (const c of CHANNELS) {
    const depth = -Math.log(converged[c]);
    worst = Math.max(worst, Math.abs(Math.log(t[c] ?? Number.NaN) + depth) / Math.max(1, depth));
  }
  return worst;
}

/** Nadir from 1,000 km, and through the limb from 400 km to the ground short of the tangent point. */
const NADIR: Vec3 = { x: -1, y: 0, z: 0 };

function limbOf(radiusM: number): { readonly direction: Vec3; readonly surfaceM: number } {
  const distanceM = radiusM + 400_000;
  const sin = (radiusM - 2_000) / distanceM;
  const cos = Math.sqrt(1 - sin * sin);
  // The near root of the ground's sphere along the ray.
  const surfaceM = distanceM * cos - Math.sqrt(radiusM * radiusM - (distanceM * sin) ** 2);
  return { direction: { x: -cos, y: sin, z: 0 }, surfaceM };
}

describe("a drawn surface keeping its forward lobes' light (fix-r08-decked-air-opacity)", () => {
  const nadir = {
    transmitted: texelOf(MARS, 1_000_000, NADIR, 1_000_000, "transmitted"),
    extinguished: texelOf(MARS, 1_000_000, NADIR, 1_000_000, "extinguished"),
    converged: convergedDeltaM(MARS, 1_000_000, NADIR, 1_000_000),
  };

  it("holds Mars's nadir from 1,000 km within 1% of a 4,096-step march of the δ-M medium", () => {
    expect(worstDepthError(nadir.transmitted.transmittance, nadir.converged)).toBeLessThan(
      TOLERANCE,
    );
  });

  it("keeps a fifth or more of Mars's surface light that the dust's extinction took", () => {
    for (const c of CHANNELS) {
      const raised = nadir.transmitted.transmittance[c] / nadir.extinguished.transmittance[c];
      expect(raised).toBeGreaterThan(1.2);
    }
  });

  it("leaves the first target and the second's mean as they were, to the bit", () => {
    expect(nadir.transmitted.light).toEqual(nadir.extinguished.light);
    expect(nadir.transmitted.transmittance[3]).toBe(nadir.extinguished.light[3]);
  });

  it("dims the surface by the throughput itself on the way back", () => {
    const t = nadir.extinguished.transmittance;
    expect(t[3]).toBe((t[0] + t[1] + t[2]) / 3);
  });

  it("holds Mars's limb from 400 km within 1% of the δ-M medium's march", () => {
    const { direction, surfaceM } = limbOf(MARS.radiusM);
    const texel = texelOf(MARS, 400_000, direction, surfaceM, "transmitted");
    const converged = convergedDeltaM(MARS, 400_000, direction, surfaceM);
    expect(worstDepthError(texel.transmittance, converged)).toBeLessThan(TOLERANCE);
  });

  it("keeps a deep path's own transmittance where its throughput is past f32 and f64 alike", () => {
    const thick: AtmosphereMedium = {
      ...MARS_REFERENCE,
      terms: MARS_REFERENCE.terms.map(deepened),
    };
    const deep = planetOf(thick, MARS_REFERENCE_RADIUS_M, NO_ABSORBER_CURVES);
    const { direction, surfaceM } = limbOf(deep.radiusM);
    const texel = texelOf(deep, 400_000, direction, surfaceM, "transmitted");
    const through = texelOf(deep, 400_000, direction, surfaceM, "extinguished").transmittance;
    for (const c of CHANNELS) {
      const t = texel.transmittance[c];
      expect(Number.isFinite(t) && t >= through[c] && t <= 1).toBe(true);
    }
  });

  it("draws a medium with no drawn lobe as before, both targets to the bit", () => {
    const { direction, surfaceM } = limbOf(EARTH.radiusM);
    for (const [heightM, d, s] of [
      [1_000_000, NADIR, 1_000_000],
      [400_000, direction, surfaceM],
    ] as const) {
      expect(texelOf(EARTH, heightM, d, s, "transmitted")).toEqual(
        texelOf(EARTH, heightM, d, s, "extinguished"),
      );
    }
  });
});

describe("a drawn surface's lobes on a bake's table (fix-r08-decked-air-opacity)", () => {
  // Mars from 1,000 km at nadir, its sources' multiple scattering read from a stand-in table of
  // `kind`, or Ψ_ms alone.
  const surfaceUnder = (kind: ThickTableKind | null): TwinMarch => {
    const figure: SpheroidFigure = {
      equatorialRadiusM: MARS.radiusM,
      polarRadiusM: MARS.radiusM,
    };
    const view = twinView(cameraAt(MARS.radiusM, 1_000_000), SUN, figure, 0.1);
    const clipped = rayMarchRayAlongTwin(MARS.medium, view, view.cameraM, NADIR, 1_000_000);
    if (clipped === null) {
      throw new Error("the ray meets the air");
    }
    const own = tableSources(MARS.planet);
    const sources: TwinSources =
      kind === null
        ? own
        : {
            ...own,
            thick: {
              kind,
              read: (_h, _mu, _travel, _horizontal, out) => {
                out.fill(0.01);
              },
            },
          };
    return marchTwin(
      MARS.medium,
      sources,
      { kind: "spheroid", figure },
      clipped.ray,
      placedSteps(clipped.ray, 64),
    );
  };

  it("keeps none where the march reads a bake's J, which holds that light", () => {
    const baked = surfaceUnder("thick");
    expect(baked.surface).toEqual(baked.transmittance);
  });

  it("keeps it beside the correction alone and over Ψ_ms alone", () => {
    for (const kind of ["correction", null] as const) {
      const march = surfaceUnder(kind);
      for (const c of CHANNELS) {
        expect(march.surface[c]).toBeGreaterThan(1.2 * march.transmittance[c]);
      }
    }
  });

  it("keeps none in a thick ray march's kernel unless its table is the correction alone or it departs from a decked air's bake (fix-r08-decked-air-route)", () => {
    expect(RAY_MARCH_THICK_KERNEL.reference).toContain(
      "let surfaceLobes = lobes && view.forward.w > 0.5 && depth > 0.0 && (thickCorrectionOnly() || view.forward.w > 1.5);",
    );
    expect(RAY_MARCH_KERNEL.reference).toContain(
      "let surfaceLobes = lobes && view.forward.w > 0.5 && depth > 0.0;",
    );
  });
});

describe("the ray march's second target with the lobes' light kept (fix-r08-decked-air-opacity)", () => {
  const code = rayMarchWgsl.replaceAll(/\/\/[^\n]*/gu, "");

  it("keeps it only on a ray that ends on a drawn surface, where the medium has a lobe and the view's forward.w asks (fix-r08-decked-air-route)", () => {
    expect(code).toMatch(/let surfaceLobes = lobes && view\.forward\.w > 0\.5 && depth > 0\.0;/u);
    expect(code).toMatch(
      /var transmittanceRgb = march\.throughput0;\s*if \(surfaceLobes\) \{\s*transmittanceRgb = exp\(-surfaceDepth\);\s*\}/u,
    );
  });

  it("sums slot 0's step depth less the step's in-lobe scattering from the first step, in a loop of at most one pass, with one exp a ray (fix-r08-decked-air-route)", () => {
    expect(code).toMatch(
      /let absorbedBefore = \(\*march\)\.path0\.depth;\s*viewMarchStep\(march, local, shares, stepAt\.dtM\);\s*rayMarchSurfaceStep\(surfaceDepth, march, local, absorbedBefore, stepAt\.dtM, surfaceLobes\);/u,
    );
    expect(code).toMatch(
      /for \(var lobed = surfaceLobes; lobed; lobed = false\) \{\s*let stepDepth = local\.extinction \* dtM \+ \(\(\*march\)\.path0\.depth - absorbedBefore\);\s*let kept = rayMarchLobeScatteringOf\(local\) \* dtM;\s*\*surfaceDepth \+= max\(stepDepth - kept, vec3f\(0\.0\)\);\s*\}/u,
    );
    expect(code).toMatch(/var surfaceDepth = vec3f\(0\.0\);/u);
    // The second target's, and the departure's T_lobe (fix-r08-decked-air-route).
    expect(code.split("exp(-surfaceDepth)").length - 1).toBe(2);
    expect(code).not.toMatch(/if \(surfaceLobes\)\s*\{\s*let stepDepth/u);
  });

  it("compiles it out of a deck's march, whose bake's J holds that light (fix-r08-decked-air-route)", () => {
    for (const pair of [RAY_MARCH_DECK_KERNEL, RAY_MARCH_SLOTS_DECK_KERNEL]) {
      const deck = pair.reference.replaceAll(/\/\/[^\n]*/gu, "");
      expect(deck).not.toMatch(/rayMarchSurfaceStep\(surfaceDepth/u);
      expect(deck).toMatch(/let surfaceLobes = false;/u);
    }
  });
});

describe("a decked air's departure in the twin (fix-r08-decked-air-route)", () => {
  const FIGURE = { kind: "sphere", radiusM: MARS_REFERENCE_RADIUS_M } as const;
  const ONE_BAND = [
    {
      gravityRatio: 1,
      latitudeRad: 0,
      bottomRadiusM: MARS_REFERENCE_RADIUS_M,
      coefficientFactor: 1,
    },
  ];
  /** A patterned bake over Mars's dust, the medium with a drawn lobe, and a disc from orbit. */
  const bake = patternedThickBake(ONE_BAND, kernelMedium(MARS_REFERENCE), 1);
  const DISC: ReferenceGeometry = {
    name: "disc",
    image: "disc-p30",
    observer: { heightM: 1_000_000, latitudeDeg: 0 },
    view: { zenithDeg: 180, azimuthDeg: 0 },
    sun: { zenithDeg: 30, azimuthDeg: 0 },
  };

  /** Each twin over the bake, built once. */
  const twins = new Map<string, ReturnType<typeof referenceTwin>>();
  const twinOf = (
    deckedAirSurface: "departure" | "uniform" | null,
    surfaceTransmittance: "channels" | "grey" = "channels",
  ): ReturnType<typeof referenceTwin> => {
    const key = `${String(deckedAirSurface)} ${surfaceTransmittance}`;
    let twin = twins.get(key);
    if (twin === undefined) {
      twin = referenceTwin(MARS_REFERENCE, FIGURE, {
        thick: bake,
        deckedAirSurface,
        surfaceTransmittance,
      });
      twins.set(key, twin);
    }
    return twin;
  };

  it("equals the route over a ground of the bake's albedo", { timeout: 120_000 }, () => {
    const routed = twinOf("uniform").radiance(DISC);
    const total = twinOf("departure").radiance(DISC);
    // S̄ is lit as the drawn ground is, so the two differ by the lookups' rounding alone: the disc
    // measured 3 × 10⁻⁸ apart (2026-11-06).
    expect(CHANNELS.map((c) => Math.abs(total[c] / routed[c] - 1) < 1e-6)).toEqual([
      true,
      true,
      true,
    ]);
  });

  it("draws its surface's part through T_lobe", { timeout: 120_000 }, () => {
    const routed = twinOf("uniform").surfaceRadiance(DISC);
    const departed = twinOf("departure").surfaceRadiance(DISC);
    expect(CHANNELS.map((c) => departed[c] > 1.2 * routed[c])).toEqual([true, true, true]);
  });

  it("is the route for a bake that is no decked air's, the default", { timeout: 120_000 }, () => {
    expect(twinOf(null).radiance(DISC)).toEqual(twinOf("uniform").radiance(DISC));
  });

  it(
    "is the route with the surfaces dimmed by the mean, as the frame takes it",
    { timeout: 120_000 },
    () => {
      expect(twinOf("departure", "grey").radiance(DISC)).toEqual(
        twinOf("uniform", "grey").radiance(DISC),
      );
    },
  );
});
