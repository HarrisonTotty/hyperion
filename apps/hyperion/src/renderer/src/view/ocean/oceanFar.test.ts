import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, vec3 } from "../../geometry/vec3";
import { oceanCellCentre, waterWorldAnswer } from "../../test/oceanFixtures";
import type { OceanLiquid } from "./liquids";
import { fresnelReflectance, normalReflectance } from "./liquids";
import { monthBlend } from "./monthBlend";
import {
  diffuseFresnelReflectance,
  erfcApprox,
  type OceanFarOptics,
  oceanFarLight,
  oceanFarSample,
  oceanGlintFactor,
  oceanLiquidOptics,
  oceanSeaConstants,
  oceanSlopeVariance,
  oceanUpwellingOf,
  oceanWhitecapFraction,
  raySpheroidHit,
  smithLambda,
  WHITECAP_ALBEDO,
} from "./oceanFar";
import { packOceanTexels } from "./oceanMap";
import { oceanMaskOf } from "./oceanMask";
import { EARTH_SEA, seaState, SLOPE_VARIANCE_FLOOR } from "./seaState";

/** Earth's mean radius, m. */
const R = 6_371_000;

/** Pure water at 15 °C, red, green, blue (the shape of the palette's record at the channels). */
const WATER: OceanLiquid = {
  temperatureK: 288,
  densityKgM3: 999,
  refractiveIndex: [1.331, 1.334, 1.338],
  absorptionPerM: [0.35, 0.06, 0.0064],
  scatteringPerM: [0.0012, 0.0019, 0.0045],
  surfaceTensionNPerM: 0.0735,
  viscosityPaS: 1.14e-3,
  basis: "measured",
  missing: [],
  approximate: false,
};

const EARTH = oceanSeaConstants(EARTH_SEA.gravityMps2, EARTH_SEA.liquid, EARTH_SEA.air);

/** The far ocean's optics over a sphere of Earth's radius, the sun along `sun`. */
function opticsOf(sun = vec3(1, 0, 0), liquid: OceanLiquid = WATER): OceanFarOptics {
  return {
    equatorialRadiusM: R,
    polarRadiusM: R,
    pixelAngleRad: 1e-4,
    whitecaps: false,
    sunDirection: normalise(sun),
    glintCapCdM2: Number.POSITIVE_INFINITY,
    sunLux: [1.1e5, 1.2e5, 1.25e5],
    skyLux: [1.5e4, 2e4, 3e4],
    sea: EARTH,
    ...oceanLiquidOptics(liquid),
    bedAlbedo: [0.3, 0.3, 0.25],
  };
}

/** A water world with one closed basin at (face 0, 18, 16), `levelM` above the sea. */
function basin(levelM: number): ReturnType<typeof packOceanTexels> {
  const answer = waterWorldAnswer(4000, 64, 81, [
    { face: 0, i: 18, j: 16, depthM: 40, levelM, ice: 0 },
  ]);
  return packOceanTexels(answer, oceanMaskOf(answer), { from: 0, to: 0, weight: 0 });
}

describe("the far ocean's spheroid (R11.T8.b)", () => {
  it("meets a sphere and an oblate spheroid exactly, from outside and in front", () => {
    const down = raySpheroidHit(vec3(2 * R, 0, 0), vec3(-1, 0, 0), R, R);
    expect(down?.tM).toBeCloseTo(R, 6);
    expect(down?.normal).toEqual(vec3(1, 0, 0));
    const a = 6_378_137;
    const c = 6_356_752;
    const pole = raySpheroidHit(vec3(0, 0, 2 * c), vec3(0, 0, -1), a, c);
    expect(pole?.tM).toBeCloseTo(c, 6);
    const slant = raySpheroidHit(vec3(9e6, 2e6, 4e6), normalise(vec3(-1, -0.1, -0.5)), a, c);
    const p = slant?.point ?? vec3(0, 0, 0);
    expect((p.x * p.x + p.y * p.y) / (a * a) + (p.z * p.z) / (c * c)).toBeCloseTo(1, 12);
    // The normal is the gradient's: along (x / a², y / a², z / c²).
    const gradient = normalise(vec3(p.x / (a * a), p.y / (a * a), p.z / (c * c)));
    expect(dot(gradient, slant?.normal ?? vec3(0, 0, 0))).toBeCloseTo(1, 14);
    expect(raySpheroidHit(vec3(2 * R, 0, 0), vec3(1, 0, 0), R, R)).toBeNull();
    expect(raySpheroidHit(vec3(2 * R, 0, 0), vec3(0, 1, 0), R, R)).toBeNull();
    expect(raySpheroidHit(vec3(0.5 * R, 0, 0), vec3(-1, 0, 0), R, R)).toBeNull();
  });
});

describe("the far ocean's sea state (R11.T8.b)", () => {
  it("takes seaState's slope variances and whitecaps, linear in the 10 m wind", () => {
    for (const wind of [0.5, 2, 5, 12, 30]) {
      const state = seaState({
        windMps: wind,
        windHeightM: 10,
        gravityMps2: EARTH_SEA.gravityMps2,
        liquid: EARTH_SEA.liquid,
        air: EARTH_SEA.air,
        fetchM: null,
      });
      const variance = oceanSlopeVariance(wind, EARTH);
      expect(variance.upwind).toBeCloseTo(state.slopeVariance.upwind, 12);
      expect(variance.crosswind).toBeCloseTo(state.slopeVariance.crosswind, 12);
      expect(oceanWhitecapFraction(wind, EARTH)).toBeCloseTo(state.whitecapFraction, 12);
    }
  });
});

describe("the far ocean's light (R11.T8.b)", () => {
  const texels = packOceanTexels(
    waterWorldAnswer(4000, 64, 81),
    oceanMaskOf(waterWorldAnswer(4000, 64, 81)),
    monthBlend(12, 0.0167, 0) ?? { from: 0, to: 0, weight: 0 },
  );

  it("takes Fresnel at the palette liquid's index, channel by channel", () => {
    const n = vec3(0, 0, 1);
    const variance = { upwind: SLOPE_VARIANCE_FLOOR, crosswind: SLOPE_VARIANCE_FLOOR };
    const factor = oceanGlintFactor(n, n, n, vec3(1, 0, 0), variance, WATER.refractiveIndex);
    const density = 1 / (2 * Math.PI * SLOPE_VARIANCE_FLOOR);
    for (const k of [0, 1, 2] as const) {
      expect(factor[k]).toBeCloseTo((normalReflectance(WATER.refractiveIndex[k]) * density) / 4, 9);
    }
    expect(factor[2]).toBeGreaterThan(factor[0]);
  });

  it("draws the water's own light from its record: deep and bed, Beer–Lambert alone, or none", () => {
    expect(oceanUpwellingOf(WATER)).toBe("deep-and-bed");
    const clear: OceanLiquid = { ...WATER, scatteringPerM: null, missing: ["scattering"] };
    expect(oceanUpwellingOf(clear)).toBe("beer-lambert");
    expect(oceanLiquidOptics(clear).deepReflectance).toEqual([0, 0, 0]);
    const opaque: OceanLiquid = { ...WATER, absorptionPerM: null, missing: ["absorption"] };
    expect(oceanUpwellingOf(opaque)).toBe("opaque");
    // Off the glint, looking straight down with the sun overhead: the surface's sky reflection and
    // the water's own light. Deep water shows the deep term; an opaque column none of it.
    const overhead = opticsOf(vec3(0, 0, 1));
    const view = vec3(0, 0, 1);
    const normal = vec3(0, 0, 1);
    const shallow = { depthM: 1, levelM: 0, windEastMps: 0, windNorthMps: 0 };
    const deep = { ...shallow, depthM: 4000 };
    const glintless: OceanFarOptics = { ...overhead, sunLux: [0, 0, 0] };
    const sky = (k: 0 | 1 | 2): number =>
      (fresnelReflectance(WATER.refractiveIndex[k], 1) * glintless.skyLux[k]) / Math.PI;
    const deepLight = oceanFarLight(glintless, glintless, view, normal, deep, 1e6).radiance;
    const shallowLight = oceanFarLight(glintless, glintless, view, normal, shallow, 1e6).radiance;
    const opaqueLight = oceanFarLight(
      { ...glintless, ...oceanLiquidOptics(opaque) },
      glintless,
      view,
      normal,
      shallow,
      1e6,
    ).radiance;
    expect(opaqueLight[1]).toBeCloseTo(sky(1), 9);
    expect(deepLight[2]).toBeGreaterThan(sky(2));
    // The bed shows through a metre of water and not through four kilometres.
    expect(shallowLight[0]).toBeGreaterThan(deepLight[0] * 2);
    // Beer–Lambert alone: the deep sea is dark but for the sky's reflection.
    const clearDeep = oceanFarLight(
      { ...glintless, ...oceanLiquidOptics(clear) },
      glintless,
      view,
      normal,
      deep,
      1e6,
    ).radiance;
    expect(clearDeep[0]).toBeCloseTo(sky(0), 6);
  });

  it("covers the high setting's sea with whitecaps by Monahan and O'Muircheartaigh, and the low's with none", () => {
    const optics = opticsOf(vec3(0, 0, 1));
    const view = normalise(vec3(0.5, 0, 1));
    const normal = vec3(0, 0, 1);
    const windy = { depthM: 4000, levelM: 0, windEastMps: 0, windNorthMps: 15 };
    const low = oceanFarLight(optics, optics, view, normal, windy, 1e6);
    const high = oceanFarLight({ ...optics, whitecaps: true }, optics, view, normal, windy, 1e6);
    expect(low.whitecapFraction).toBe(0);
    expect(high.whitecapFraction).toBeCloseTo(oceanWhitecapFraction(15, EARTH), 12);
    const foam = (WHITECAP_ALBEDO * (optics.sunLux[1] + optics.skyLux[1])) / Math.PI;
    const w = high.whitecapFraction;
    expect(high.radiance[1]).toBeCloseTo((1 - w) * low.radiance[1] + w * foam, 6);
  });

  it("filters the glint by the pixel's footprint, lower and wider for a coarser pixel", () => {
    const optics = opticsOf(vec3(0, 0, 1));
    const normal = vec3(0, 0, 1);
    const calm = { depthM: 4000, levelM: 0, windEastMps: 0, windNorthMps: 0.5 };
    const fine = oceanFarLight(optics, optics, normal, normal, calm, 1e6).glint[1];
    // From the Moon's distance with a 1 mrad pixel, a 400 km footprint: its normals spread by
    // 0.063 rad, a variance of 3.3 × 10⁻⁴ against the glassy sea's floor of 10⁻⁴.
    const coarse = oceanFarLight(
      { ...optics, pixelAngleRad: 1e-3 },
      optics,
      normal,
      normal,
      calm,
      4e8,
    ).glint[1];
    const spread = (4e8 * 1e-3) / R;
    const pixel = (spread * spread) / 12;
    expect(coarse / fine).toBeCloseTo(SLOPE_VARIANCE_FLOOR / (SLOPE_VARIANCE_FLOOR + pixel), 3);
  });

  it("meets open water and nothing over dry ground, sea ice or unsurveyed cells", () => {
    const answer = waterWorldAnswer(4000, 64, 81, [
      { face: 0, i: 16, j: 16, depthM: 0, levelM: 300, ice: 0 },
      { face: 0, i: 17, j: 16, depthM: 30, levelM: 0, ice: 255 },
      { face: 0, i: 18, j: 16, depthM: 40, levelM: 250, ice: 0 },
    ]);
    const map = packOceanTexels(answer, oceanMaskOf(answer), { from: 0, to: 0, weight: 0 });
    const optics = opticsOf();
    const at = (i: number, j: number): ReturnType<typeof oceanFarSample> => {
      const d = oceanCellCentre(0, i, j);
      const target = scale(normalise(vec3(d[0], d[1], d[2])), R);
      const origin = scale(normalise(target), 3 * R);
      return oceanFarSample(optics, map, origin, normalise(sub(target, origin)));
    };
    expect(at(15, 16)).not.toBeNull();
    expect(at(16, 16)).toBeNull();
    expect(at(17, 16)).toBeNull();
    // A closed basin at its own level: 250 m above the sea, met 250 m sooner looking straight down.
    const lake = at(18, 16);
    const sea = at(19, 16);
    expect(lake).not.toBeNull();
    expect(norm(lake?.hit.point ?? vec3(0, 0, 0))).toBeCloseTo(R + 250, 3);
    expect((sea?.hit.tM ?? 0) - (lake?.hit.tM ?? 0)).toBeCloseTo(250, 3);
    // An unsurveyed world: no sea at all.
    const none = waterWorldAnswer(4000, 64, 81);
    const unsurveyed = packOceanTexels(
      {
        ...none,
        cells: new Uint32Array(),
        depthM: new Float32Array(),
        levelM: new Float32Array(),
        ice: new Uint8Array(),
        winds: new Uint8Array(),
      },
      oceanMaskOf({
        ...none,
        cells: new Uint32Array(),
        depthM: new Float32Array(),
        ice: new Uint8Array(),
      }),
      { from: 0, to: 0, weight: 0 },
    );
    expect(oceanFarSample(optics, unsurveyed, vec3(3 * R, 0, 0), vec3(-1, 0, 0))).toBeNull();
    expect(oceanFarSample(optics, texels, vec3(3 * R, 0, 0), vec3(-1, 0, 0))).not.toBeNull();
  });

  it("misses a closed basin whose raised surface the ray meets in another cell", () => {
    // Grazing at the basin's cell, 80° from its vertical: a surface 60 km up is met some 340 km
    // short of it, over another cell; at the sea's own level the same ray meets it.
    const d = oceanCellCentre(0, 18, 16);
    const up = normalise(vec3(d[0], d[1], d[2]));
    const target = scale(up, R);
    const along = normalise(cross(up, vec3(0, 0, 1)));
    const slant = (80 * Math.PI) / 180;
    const origin = add(
      target,
      scale(add(scale(up, Math.cos(slant)), scale(along, Math.sin(slant))), 2 * R),
    );
    const ray = normalise(sub(target, origin));
    expect(oceanFarSample(opticsOf(), basin(0), origin, ray)).not.toBeNull();
    expect(oceanFarSample(opticsOf(), basin(60_000), origin, ray)).toBeNull();
  });
});

describe("the far ocean's shadowing and the sky's way in (R11.T8.b)", () => {
  it("is Smith's Λ: none along the normal, about a third at 10° of elevation and σ² = 0.1", () => {
    expect(smithLambda(1, 0.1)).toBe(0);
    const tenDegrees = Math.sin((10 * Math.PI) / 180);
    expect(smithLambda(tenDegrees, 0.1)).toBeCloseTo(0.324, 2);
    expect(smithLambda(0.9, 0.01)).toBeLessThan(1e-6);
    expect(erfcApprox(0)).toBe(1);
    expect(erfcApprox(1)).toBeCloseTo(0.1573, 2);
  });

  it("never reflects more light than a grazing sun brings", () => {
    const n = vec3(0, 0, 1);
    const sun = normalise(vec3(Math.sqrt(1 - 0.05 * 0.05), 0, 0.05));
    const variance = { upwind: 0.1, crosswind: 0.1 };
    // ∫ f μ_v dΩ over the view's hemisphere, f = the glint factor ÷ μ_s: at most 1.
    const steps = 400;
    let albedo = 0;
    for (let i = 0; i < steps; i += 1) {
      const mu = (i + 0.5) / steps;
      const sin = Math.sqrt(1 - mu * mu);
      for (let j = 0; j < steps; j += 1) {
        const phi = ((j + 0.5) / steps) * 2 * Math.PI;
        const view = vec3(sin * Math.cos(phi), sin * Math.sin(phi), mu);
        const factor = oceanGlintFactor(view, sun, n, vec3(1, 0, 0), variance, [1.34, 1.34, 1.34]);
        albedo += (factor[1] / 0.05) * mu * (1 / steps) * ((2 * Math.PI) / steps);
      }
    }
    expect(albedo).toBeLessThan(1);
    expect(albedo).toBeGreaterThan(0.1);
  });

  it("lets a uniform sky into the water through a flat surface's R̄, about 0.066 for water", () => {
    expect(diffuseFresnelReflectance(1.34)).toBeCloseTo(0.0675, 3);
    expect(diffuseFresnelReflectance(1.333)).toBeGreaterThan(0.064);
    expect(diffuseFresnelReflectance(1.333)).toBeLessThan(0.068);
  });
});
