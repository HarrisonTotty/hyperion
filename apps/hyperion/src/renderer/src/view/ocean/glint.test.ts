import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { waterWorldAnswer } from "../../test/oceanFixtures";
import { glareSourceSolidAngleSr } from "../post/glare";
import { GLINT_GRID, oceanGlintGlare, specularPoint } from "./glint";
import { fresnelReflectance, type OceanLiquid } from "./liquids";
import {
  eastNorthOf,
  filteredSlopeVariance,
  type OceanFarOptics,
  oceanFarSample,
  oceanLiquidOptics,
  oceanSeaConstants,
  smithShadowing,
} from "./oceanFar";
import { oceanTexelAt, packOceanTexels } from "./oceanMap";
import { oceanMaskOf } from "./oceanMask";
import { EARTH_SEA } from "./seaState";

/** Earth's mean radius, m. */
const R = 6_371_000;

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

/** A water world, a 7 m/s wind towards the east every month. */
const ANSWER = waterWorldAnswer(4000, 64, 152);
const TEXELS = packOceanTexels(ANSWER, oceanMaskOf(ANSWER), { from: 0, to: 0, weight: 0 });

function opticsOf(sun: Vec3, capCdM2 = Number.POSITIVE_INFINITY): OceanFarOptics {
  return {
    equatorialRadiusM: R,
    polarRadiusM: R,
    pixelAngleRad: 2e-4,
    whitecaps: false,
    sunDirection: normalise(sun),
    glintCapCdM2: capCdM2,
    sunLux: [1.1e5, 1.2e5, 1.25e5],
    skyLux: [1.5e4, 2e4, 3e4],
    sea: oceanSeaConstants(EARTH_SEA.gravityMps2, EARTH_SEA.liquid, EARTH_SEA.air),
    ...oceanLiquidOptics(WATER),
    bedAlbedo: [0.3, 0.3, 0.25],
  };
}

/** The three geometries: overhead, oblique, and near the terminator. */
const GEOMETRIES: ReadonlyArray<{
  readonly name: string;
  readonly camera: Vec3;
  readonly sun: Vec3;
}> = [
  { name: "sun behind the camera", camera: vec3(3 * R, 0, 0), sun: vec3(1, 0, 0) },
  { name: "oblique", camera: vec3(2 * R, 0.5 * R, 0.3 * R), sun: normalise(vec3(1, -0.6, 0.2)) },
  {
    name: "near the terminator",
    camera: vec3(1.1 * R, 0.1 * R, 0),
    sun: normalise(vec3(Math.cos(1.35), Math.sin(1.35), 0.05)),
  },
];

/** The angle between unit vectors, rad. */
function angle(a: Vec3, b: Vec3): number {
  return Math.atan2(norm(cross(a, b)), dot(a, b));
}

describe("the glint's specular point (R11.T8.b)", () => {
  it.each(GEOMETRIES)("obeys the reflection law: $name", ({ camera, sun }) => {
    const point = specularPoint(camera, sun, R);
    if (point === null) {
      throw new Error("no specular point");
    }
    expect(norm(point.pointM)).toBeCloseTo(R, 6);
    const view = normalise(sub(camera, point.pointM));
    // The normal bisects the view and the sun: equal angles, in one plane.
    expect(Math.abs(angle(point.normal, view) - angle(point.normal, sun))).toBeLessThan(1e-12);
    // Out of the plane of the view and the sun by nothing, where they span one.
    const plane = cross(view, sun);
    const outOfPlane = norm(plane) > 1e-9 ? Math.abs(dot(point.normal, normalise(plane))) : 0;
    expect(outOfPlane).toBeLessThan(1e-12);
    expect(dot(normalise(add(view, sun)), point.normal)).toBeCloseTo(1, 14);
    expect(point.cosine).toBeCloseTo(dot(point.normal, view), 12);
  });

  it("lies under the camera with the sun behind it, and is none with the sun opposite", () => {
    const point = specularPoint(vec3(3 * R, 0, 0), vec3(1, 0, 0), R);
    expect(point?.pointM).toEqual(vec3(R, 0, 0));
    expect(specularPoint(vec3(3 * R, 0, 0), vec3(-1, 0, 0), R)).toBeNull();
    expect(specularPoint(vec3(0.5 * R, 0, 0), vec3(1, 0, 0), R)).toBeNull();
  });

  it.each(GEOMETRIES)(
    "is where the far ocean's facets face the bisector, its glint unfiltered there: $name",
    ({ camera, sun }) => {
      const point = specularPoint(camera, sun, R);
      if (point === null) {
        throw new Error("no specular point");
      }
      const optics = opticsOf(sun);
      const ray = normalise(sub(point.pointM, camera));
      const sample = oceanFarSample(optics, TEXELS, camera, ray);
      if (sample === null) {
        throw new Error("no sea at the specular point");
      }
      expect(norm(sub(sample.hit.point, point.pointM))).toBeLessThan(1e-3);
      // There the half vector is the normal: β = 0, the slopes 0, the glint F(μ) p(0) ÷ 4μ exactly.
      const mu = point.cosine;
      const variance = filteredSlopeVariance(
        Math.hypot(sample.texel.windEastMps, sample.texel.windNorthMps),
        optics.sea,
        (sample.hit.tM * optics.pixelAngleRad) / Math.sqrt(mu),
        R,
      );
      const density = 1 / (2 * Math.PI * Math.sqrt(variance.upwind * variance.crosswind));
      const { east, north } = eastNorthOf(sample.hit.normal);
      const wind = add(
        scale(east, sample.texel.windEastMps),
        scale(north, sample.texel.windNorthMps),
      );
      const shadowing = smithShadowing(
        normalise(sub(camera, sample.hit.point)),
        sun,
        sample.hit.normal,
        normalise(wind),
        variance,
      );
      const expected =
        (optics.sunLux[1] *
          fresnelReflectance(WATER.refractiveIndex[1], mu) *
          density *
          shadowing) /
        (4 * mu);
      expect(sample.light.glint[1] / expected).toBeCloseTo(1, 8);
    },
  );

  it.each(GEOMETRIES)(
    "is where a glassy sea's glint peaks, to its spot's own size: $name",
    ({ camera, sun }) => {
      const point = specularPoint(camera, sun, R);
      if (point === null) {
        throw new Error("no specular point");
      }
      // A breeze below the generation threshold: σ² at the floor, 10⁻⁴, a spot of 0.01 rad.
      const glassy = waterWorldAnswer(4000, 64, 1);
      const texels = packOceanTexels(glassy, oceanMaskOf(glassy), { from: 0, to: 0, weight: 0 });
      const optics = opticsOf(sun);
      const centre = normalise(sub(point.pointM, camera));
      const e1 = normalise(cross(centre, vec3(0, 0, 1)));
      const e2 = cross(centre, e1);
      const distance = norm(sub(point.pointM, camera));
      // Steps of 200 m at the point, over ±8 km.
      const step = 200 / distance;
      let best = { glint: -1, at: vec3(0, 0, 0) };
      for (let i = -40; i <= 40; i += 1) {
        for (let j = -40; j <= 40; j += 1) {
          const ray = normalise(add(centre, add(scale(e1, i * step), scale(e2, j * step))));
          const sample = oceanFarSample(optics, texels, camera, ray);
          const glint = sample?.light.glint[1] ?? -1;
          if (glint > best.glint) {
            best = { glint, at: sample?.hit.point ?? vec3(0, 0, 0) };
          }
        }
      }
      // The peak moves off the bisector's point only by the Fresnel and 1 ÷ μ_v factors' slope over
      // the spot: about σ² R tan θ, under 2 km here, and the grid's step, foreshortened.
      const foreshortened = 200 / Math.max(point.cosine, 0.1);
      expect(norm(sub(best.at, point.pointM))).toBeLessThan(2000 + 2 * foreshortened);
    },
  );
});

describe("the glint's glare source (R11.T8.b)", () => {
  const camera = vec3(4 * R, 0.3 * R, 0.2 * R);
  const sun = normalise(vec3(1, -0.2, 0.1));

  it("is none where nothing passes the cap, or no specular point is seen", () => {
    expect(oceanGlintGlare(opticsOf(sun), TEXELS, camera, (d) => d)).toBeNull();
    expect(
      oceanGlintGlare(opticsOf(scale(normalise(camera), -1), 1), TEXELS, camera, (d) => d),
    ).toBeNull();
  });

  it("carries the energy the cap takes from the texels, and points at the specular point", () => {
    const uncapped = opticsOf(sun);
    const point = specularPoint(camera, sun, R);
    if (point === null) {
      throw new Error("no specular point");
    }
    const view = normalise(sub(camera, point.pointM));
    const texel = oceanTexelAt(TEXELS, [point.normal.x, point.normal.y, point.normal.z]);
    expect(texel).not.toBeNull();
    const peak = oceanFarSample(uncapped, TEXELS, camera, scale(view, -1))?.light.glint[1] ?? 0;
    const cap = peak / 8;
    const capped = opticsOf(sun, cap);
    const source = oceanGlintGlare(capped, TEXELS, camera, (d) => d);
    if (source === null) {
      throw new Error("no glare source");
    }
    expect(dot(source.direction, scale(view, -1))).toBeCloseTo(1, 12);
    // An independent integral over the camera's view of the spot, finer than the source's grid:
    // the glint's light above the cap, ∫ (L − cap)⁺ dΩ, is the source's excess times its solid angle.
    const e1 = normalise(cross(view, vec3(0, 0, 1)));
    const e2 = cross(view, e1);
    const distance = norm(sub(camera, point.pointM));
    const halfWidth = (0.35 * R) / distance;
    const n = 2 * GLINT_GRID + 1;
    const step = (2 * halfWidth) / n;
    let above = 0;
    for (let i = 0; i < n; i += 1) {
      for (let j = 0; j < n; j += 1) {
        const x = -halfWidth + (i + 0.5) * step;
        const y = -halfWidth + (j + 0.5) * step;
        const ray = normalise(add(scale(view, -1), add(scale(e1, x), scale(e2, y))));
        const glint = oceanFarSample(uncapped, TEXELS, camera, ray)?.light.glint[1] ?? 0;
        // dΩ of a gnomonic step, (1 + x² + y²)^(−3/2) dx dy.
        above += (Math.max(glint - cap, 0) * step * step) / (1 + x * x + y * y) ** 1.5;
      }
    }
    const carried = source.excessLuminance[1] * glareSourceSolidAngleSr(source);
    expect(carried / above).toBeGreaterThan(0.97);
    expect(carried / above).toBeLessThan(1.03);
    expect(source.luminance[1]).toBeGreaterThan(cap);
    expect(source.excessLuminance[1]).toBeLessThan(source.luminance[1]);
  });

  it("turns its direction into the scene's frame", () => {
    const point = specularPoint(camera, sun, R);
    const peak =
      oceanFarSample(
        opticsOf(sun),
        TEXELS,
        camera,
        normalise(sub(point?.pointM ?? vec3(0, 0, 0), camera)),
      )?.light.glint[1] ?? 0;
    const turned = oceanGlintGlare(opticsOf(sun, peak / 4), TEXELS, camera, (d) =>
      vec3(-d.y, d.x, d.z),
    );
    const plain = oceanGlintGlare(opticsOf(sun, peak / 4), TEXELS, camera, (d) => d);
    expect(turned?.direction.x).toBeCloseTo(-(plain?.direction.y ?? 0), 14);
    expect(turned?.direction.y).toBeCloseTo(plain?.direction.x ?? 0, 14);
  });
});
