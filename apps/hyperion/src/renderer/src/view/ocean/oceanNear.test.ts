import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, vec3 } from "../../geometry/vec3";
import { oceanCellCentre, waterWorldAnswer, WATER_RECORD } from "../../test/oceanFixtures";
import { lookAlong } from "../camera/quaternion";
import { gerstnerDisplacement, gerstnerJacobian, resolvedSlopeVariance } from "./gerstner";
import { fresnelReflectance, oceanLiquidOf, refractedCosine, UPWELLING_Q_SR } from "./liquids";
import {
  type OceanFarOptics,
  oceanFarLight,
  oceanLiquidOptics,
  oceanSeaConstants,
  raySpheroidHit,
} from "./oceanFar";
import { type OceanTexel, packOceanTexels } from "./oceanMap";
import { oceanMaskOf } from "./oceanMask";
import {
  componentFade,
  drawsNearSea,
  facingNormal,
  nearSeaAnchor,
  OCEAN_DEPTH_THICKNESS_MAX_M,
  OCEAN_FADE_BAND,
  OCEAN_GRID_MARGIN,
  OCEAN_LAYER_CULL_SHARE,
  OCEAN_MAX_COMPONENTS,
  OCEAN_NEAR_BELOW_M,
  OCEAN_NEAR_LEAST_REFRACTED,
  OCEAN_NEAR_SHORTEST_OVER_PEAK,
  OCEAN_WAVE_ROWS,
  oceanGridMesh,
  oceanGridSize,
  oceanLayerCull,
  oceanNearFrame,
  oceanNearHit,
  oceanNearLight,
  oceanNearPatchRadiance,
  oceanNearSet,
  oceanNearSurface,
  oceanThickness,
  oceanWavesAt,
  packOceanWaves,
  spheroidNormalAt,
} from "./oceanNear";
import oceanNearWgsl from "./oceanNear.wgsl?raw";
import { EARTH_SEA } from "./seaState";

/** Earth's mean radius, m, a sphere for the tests. */
const R = 6_371_000;

/** WGS 84's semi-axes, m (NIMA TR8350.2, Table 3.1), an oblate sea for the hits. */
const WGS84_A = 6_378_137;
const WGS84_C = 6_356_752.314245;

const WATER = oceanLiquidOf(WATER_RECORD);

/** The far sea's optics over a sphere of radius `R`, the sun along `sun`, the cap far above. */
function opticsOf(sun: ReturnType<typeof vec3>, pixelAngleRad = 1e-3): OceanFarOptics {
  return {
    equatorialRadiusM: R,
    polarRadiusM: R,
    pixelAngleRad,
    whitecaps: false,
    sunDirection: normalise(sun),
    glintCapCdM2: Number.POSITIVE_INFINITY,
    sunLux: [1.1e5, 1.2e5, 1.25e5],
    skyLux: [1.5e4, 2e4, 3e4],
    sea: oceanSeaConstants(EARTH_SEA.gravityMps2, WATER, EARTH_SEA.air),
    ...oceanLiquidOptics(WATER),
    bedAlbedo: [0.2, 0.18, 0.15],
  };
}

/** No sky's light. */
const NO_SKY: [number, number, number] = [0, 0, 0];

/** The sea's set under a 12 m/s wind, 32 components. */
function windySet(windMps = 12): ReturnType<typeof oceanNearSet> {
  return oceanNearSet({ liquid: WATER, air: EARTH_SEA.air }, EARTH_SEA.gravityMps2, windMps, 32);
}

describe("the near sea's grid (R11.T8.c)", () => {
  it("covers the view and its margin, a cell's two triangles counter-clockwise", () => {
    const size = oceanGridSize(1920, 1080, 4);
    expect(size).toEqual({
      columns: Math.ceil(((1 + OCEAN_GRID_MARGIN) * 1920) / 4),
      rows: Math.ceil(((1 + OCEAN_GRID_MARGIN) * 1080) / 4),
    });
    const mesh = oceanGridMesh(3, 2);
    expect(mesh.positions).toHaveLength(3 * 4 * 3);
    expect(mesh.indices).toHaveLength(6 * 3 * 2);
    const reach = 1 + OCEAN_GRID_MARGIN;
    expect([mesh.positions[0], mesh.positions[1]]).toEqual([-reach, -reach].map(Math.fround));
    expect([mesh.positions.at(-3), mesh.positions.at(-2)]).toEqual([reach, reach].map(Math.fround));
    // The first cell: (0, 1, 5) and (0, 5, 4), each turning counter-clockwise in NDC.
    expect(Array.from(mesh.indices.subarray(0, 6))).toEqual([0, 1, 5, 0, 5, 4]);
  });

  it("is cut at the setting's cell: the grid's cells are the cell's size in pixels", () => {
    const { columns } = oceanGridSize(1000, 500, 8);
    // Its mesh spans (1 + m) times the view's width in NDC's 2, so a cell is 8 px wide.
    expect(((1 + OCEAN_GRID_MARGIN) * 1000) / columns).toBeCloseTo(8, 1);
  });
});

describe("a component's fade by its footprint (R11.T8.c)", () => {
  it("is Bruneton's smoothstep from 1.0 to 2.5 footprints a wavelength", () => {
    expect(OCEAN_FADE_BAND).toEqual([1, 2.5]);
    expect(componentFade(25, 10)).toBe(1);
    expect(componentFade(40, 10)).toBe(1);
    expect(componentFade(17.5, 10)).toBeCloseTo(0.5, 12);
    expect(componentFade(13, 10)).toBeCloseTo(3 * 0.2 ** 2 - 2 * 0.2 ** 3, 12);
    expect(componentFade(10, 10)).toBe(0);
    expect(componentFade(5, 10)).toBe(0);
    expect(componentFade(1, 0)).toBe(1);
  });
});

describe("the lower-layer cull and the water's thickness (R11.T8.c)", () => {
  it("culls the bed within 10⁻⁶ of the camera's distance, and hides the sea behind a nearer bed", () => {
    const d = 5e4;
    expect(OCEAN_LAYER_CULL_SHARE).toBe(1e-6);
    expect(oceanLayerCull(d, d + 0.9e-6 * d)).toBe("culled");
    expect(oceanLayerCull(d, d - 0.9e-6 * d)).toBe("culled");
    expect(oceanLayerCull(d, d + 1.1e-6 * d)).toBe("sea");
    expect(oceanLayerCull(d, d - 1.1e-6 * d)).toBe("bed");
    // 1 m apart at 1,000 km is within the cull; at 100 km it is not.
    expect(oceanLayerCull(1e6, 1e6 + 1)).toBe("culled");
    expect(oceanLayerCull(1e5, 1e5 + 1)).toBe("sea");
  });

  it("takes the thickness from the depth within 10⁵ m, vertically, and from the bathymetry beyond or with no bed", () => {
    expect(OCEAN_DEPTH_THICKNESS_MAX_M).toBe(1e5);
    expect(oceanThickness(1000, 1010, 0.5, 4000)).toEqual({
      depthM: 5,
      from: "depth",
      layer: "sea",
    });
    expect(oceanThickness(1000, 1000 + 1e-4, 0.5, 4000)).toEqual({
      depthM: 0,
      from: "depth",
      layer: "culled",
    });
    expect(oceanThickness(1000, 990, 0.5, 4000).layer).toBe("bed");
    expect(oceanThickness(OCEAN_DEPTH_THICKNESS_MAX_M + 1, 2e5, 0.5, 4000)).toEqual({
      depthM: 4000,
      from: "bathymetry",
      layer: "sea",
    });
    expect(oceanThickness(1000, null, 0.5, 30).from).toBe("bathymetry");
  });
});

describe("the near sea's frame and its rays (R11.T8.c)", () => {
  const oblate = { equatorialRadiusM: WGS84_A, polarRadiusM: WGS84_C };

  it("stands the camera over the sea-level spheroid, stretched, C₀ formed without cancellation", () => {
    const camera = vec3(0, 0, WGS84_C + 500);
    const frame = oceanNearFrame(oblate, camera);
    expect(frame.heightM).toBeCloseTo(500, 6);
    expect(frame.stretch).toBeCloseTo(WGS84_A / WGS84_C, 15);
    expect(frame.stretchedDistanceM).toBeCloseTo((WGS84_C + 500) * frame.stretch, 6);
    expect(frame.excessM2).toBeCloseTo(
      (frame.stretchedDistanceM - WGS84_A) * (frame.stretchedDistanceM + WGS84_A),
      0,
    );
  });

  it.each([
    ["straight down at the equator", vec3(WGS84_A + 500, 0, 0), vec3(-1, 0, 0)],
    ["slant at 45° latitude", vec3(4.52e6, 0, 4.49e6), normalise(vec3(-0.3, 0.7, -0.6))],
    ["near the horizon over the pole", vec3(0, 0, WGS84_C + 2_000), normalise(vec3(1, 0, -0.03))],
  ] as const)("meets the spheroid itself %s, as the exact hit does", (_, camera, direction) => {
    const frame = oceanNearFrame(oblate, camera);
    const hit = oceanNearHit(frame, direction);
    const exact = raySpheroidHit(camera, direction, WGS84_A, WGS84_C);
    expect(hit.clamped).toBe(false);
    expect(Math.abs(hit.tM - (exact?.tM ?? 0))).toBeLessThan(1e-9 * hit.tM + 1e-6);
    expect(norm(sub(add(camera, hit.pointM), exact?.point ?? vec3(0, 0, 0)))).toBeLessThan(1e-4);
    expect(
      dot(spheroidNormalAt(frame, add(camera, hit.pointM)), exact?.normal ?? vec3(0, 0, 0)),
    ).toBeCloseTo(1, 12);
  });

  it("takes a ray above the horizon down onto it: on the spheroid, tangent there, at its azimuth", () => {
    const camera = vec3(0, 0, WGS84_C + 500);
    const frame = oceanNearFrame(oblate, camera);
    const hit = oceanNearHit(frame, normalise(vec3(1, 0, 0.1)));
    expect(hit.clamped).toBe(true);
    const point = add(camera, hit.pointM);
    const level =
      (point.x * point.x + point.y * point.y) / (WGS84_A * WGS84_A) +
      (point.z * point.z) / (WGS84_C * WGS84_C);
    expect(level).toBeCloseTo(1, 12);
    expect(dot(spheroidNormalAt(frame, point), hit.direction)).toBeCloseTo(0, 9);
    expect(hit.pointM.y).toBeCloseTo(0, 6);
    expect(hit.pointM.x).toBeGreaterThan(0);
  });

  it("is drawn below 20 km over the sea, and not at or above it, nor below the sea", () => {
    const optics = opticsOf(vec3(1, 0, 1));
    expect(OCEAN_NEAR_BELOW_M).toBe(20_000);
    expect(drawsNearSea(optics, vec3(0, 0, R + 19_999))).toBe(true);
    expect(drawsNearSea(optics, vec3(0, 0, R + 20_001))).toBe(false);
    expect(drawsNearSea(optics, vec3(0, 0, R - 1))).toBe(false);
  });
});

/** A water world of one wind, but for a dry cell under the camera, at face 0's (16, 16). */
function worldWith(dryUnder: boolean): ReturnType<typeof packOceanTexels> {
  const cells = dryUnder ? [{ face: 0 as const, i: 16, j: 16, depthM: 0, levelM: 50, ice: 0 }] : [];
  const answer = waterWorldAnswer(4000, 64, 165, cells);
  return packOceanTexels(answer, oceanMaskOf(answer), { from: 0, to: 0, weight: 0 });
}

describe("the near sea's anchor (R11.T8.c)", () => {
  const optics = { equatorialRadiusM: R, polarRadiusM: R };
  const centre = oceanCellCentre(0, 16, 16);
  const under = normalise(vec3(centre[0], centre[1], centre[2]));
  const camera = scale(under, R + 500);

  it("lays the waves at the texel ahead, its centre on the spheroid and its wind's frame tangent there", () => {
    const texels = worldWith(false);
    const east = normalise(cross(vec3(0, 0, 1), under));
    const anchor = nearSeaAnchor(
      optics,
      texels,
      camera,
      lookAlong(add(scale(east, 1), scale(under, -0.05)), under),
    );
    expect(norm(anchor.pointM)).toBeCloseTo(R, 3);
    expect(Math.abs(dot(anchor.upwind, normalise(anchor.pointM)))).toBeLessThan(1e-12);
    expect(Math.hypot(...anchor.windEastNorthMps)).toBeGreaterThan(10);
  });

  it("takes the nadir's texel where the view looks above the horizon, and calm over no open water", () => {
    const texels = worldWith(false);
    const up = nearSeaAnchor(optics, texels, camera, lookAlong(under, vec3(0, 0, 1)));
    expect(norm(sub(normalise(up.pointM), under))).toBeLessThan(1e-12);
    const dry = worldWith(true);
    const calm = nearSeaAnchor(optics, dry, camera, lookAlong(under, vec3(0, 0, 1)));
    expect(calm.windEastNorthMps).toEqual([0, 0]);
  });
});

describe("the near sea's waves (R11.T8.c)", () => {
  const set = windySet();
  const time = { seconds: 3_600, nanos: 250_000_000 };
  const anchor = {
    pointM: vec3(0, 0, R),
    upwind: vec3(1, 0, 0),
    across: vec3(0, 1, 0),
  };

  it("are a ladder the sea state fixes, from twice its peak's wavelength down to 1/64 of it", () => {
    expect(set.components).toHaveLength(32);
    const wavelengths = set.components.map((c) => (2 * Math.PI) / c.wavenumberRadPerM);
    const longest = Math.max(...wavelengths);
    expect(Math.min(...wavelengths)).toBeGreaterThan(
      (longest / 2) * OCEAN_NEAR_SHORTEST_OVER_PEAK * 0.99,
    );
    expect(
      oceanNearSet({ liquid: WATER, air: EARTH_SEA.air }, EARTH_SEA.gravityMps2, 0, 32).components,
    ).toEqual([]);
  });

  it("displace, normal and stretch the grid as gerstner.ts does, the camera at the anchor", () => {
    const waves = oceanWavesAt(set, time, anchor, anchor.pointM);
    const t = 3_600.25;
    for (const [x0, y0] of [
      [0, 0],
      [12.5, -40],
      [300, 77],
    ] as const) {
      const surface = oceanNearSurface(waves, x0, y0, 0);
      const [x, y, z] = gerstnerDisplacement(set, x0, y0, t);
      expect(surface.displacementM[0]).toBeCloseTo(x - x0, 6);
      expect(surface.displacementM[1]).toBeCloseTo(y - y0, 6);
      expect(surface.displacementM[2]).toBeCloseTo(z, 6);
      expect(surface.jacobian).toBeCloseTo(gerstnerJacobian(set, x0, y0, t), 6);
      const h = 1e-4;
      const at = (u: number, v: number): ReturnType<typeof vec3> => {
        const [px, py, pz] = gerstnerDisplacement(set, u, v, t);
        return vec3(px, py, pz);
      };
      const tx = scale(sub(at(x0 + h, y0), at(x0 - h, y0)), 1 / (2 * h));
      const ty = scale(sub(at(x0, y0 + h), at(x0, y0 - h)), 1 / (2 * h));
      expect(dot(normalise(cross(tx, ty)), vec3(...surface.normal))).toBeGreaterThan(1 - 1e-8);
    }
    const resolved = resolvedSlopeVariance(set);
    const drawn = oceanNearSurface(waves, 0, 0, 0).resolved;
    expect(drawn.upwind).toBeCloseTo(resolved.upwind, 12);
    expect(drawn.crosswind).toBeCloseTo(resolved.crosswind, 12);
  });

  it("stand still on the sea as the camera moves: a fixed point's displacement is the same from 1 km away", () => {
    const point = vec3(250, -130, R);
    const here = vec3(0, 0, R + 50);
    const there = add(here, vec3(1_000, 300, 120));
    const from = (camera: ReturnType<typeof vec3>): readonly [number, number, number] => {
      const waves = oceanWavesAt(set, time, anchor, camera);
      const p = sub(point, camera);
      return oceanNearSurface(waves, dot(p, anchor.upwind), dot(p, anchor.across), 0).displacementM;
    };
    const [a, b] = [from(here), from(there)];
    for (let k = 0; k < 3; k += 1) {
      expect(a[k]).toBeCloseTo(b[k] ?? Number.NaN, 6);
    }
  });

  it("form each phase at the scene's time exactly, held in [−π, π), however long the time", () => {
    for (const [seconds, nanos] of [
      [0, 0],
      [3_600, 250_000_000],
      [8_000_000_000_000, 999_999_999],
      [-12_345, 1],
    ] as const) {
      for (const w of oceanWavesAt(set, { seconds, nanos }, anchor, anchor.pointM)) {
        expect(w.phaseRad).toBeGreaterThanOrEqual(-Math.PI);
        expect(w.phaseRad).toBeLessThan(Math.PI);
      }
    }
    const waves = oceanWavesAt(set, time, anchor, anchor.pointM);
    const later = oceanWavesAt(set, { seconds: 3_601, nanos: 250_000_000 }, anchor, anchor.pointM);
    set.components.forEach((c, i) => {
      const step = (later[i]?.phaseRad ?? 0) - (waves[i]?.phaseRad ?? 0) + c.angularFrequencyRadS;
      const wrapped = step - 2 * Math.PI * Math.round(step / (2 * Math.PI));
      expect(Math.abs(wrapped)).toBeLessThan(1e-9);
    });
  });

  it("fade a component its footprint cannot resolve out of the geometry, and its slopes into the BRDF", () => {
    const waves = oceanWavesAt(set, time, anchor, anchor.pointM);
    const whole = oceanNearSurface(waves, 10, 20, 0).resolved;
    const coarse = oceanNearSurface(waves, 10, 20, 30).resolved;
    expect(coarse.upwind).toBeLessThan(whole.upwind);
    expect(oceanNearSurface(waves, 10, 20, 1e6).displacementM).toEqual([0, 0, 0]);
  });

  it("pack a header and two rows a component", () => {
    const waves = oceanWavesAt(set, time, anchor, anchor.pointM);
    const rows = packOceanWaves(waves, set);
    expect(rows).toHaveLength(4 * OCEAN_WAVE_ROWS);
    expect(rows[0]).toBe(32);
    const [first] = waves;
    expect(Array.from(rows.subarray(4, 12))).toEqual(
      Array.from(
        new Float32Array([
          first?.kxRadPerM ?? 0,
          first?.kyRadPerM ?? 0,
          first?.amplitudeM ?? 0,
          first?.phaseRad ?? 0,
          first?.steepness ?? 0,
          first?.wavelengthM ?? 0,
          first?.upwindShare ?? 0,
          0,
        ]),
      ),
    );
  });
});

describe("the near sea's light (R11.T8.c)", () => {
  const optics = opticsOf(vec3(0.3, 0, 1));
  const texel: OceanTexel = { depthM: 6, levelM: 0, windEastMps: 0, windNorthMps: 7 };
  const up = vec3(0, 0, 1);
  const view = normalise(vec3(-0.4, 0.1, 1));

  it("reduces to the deep sea's under refraction where the bed reflects as the deep column does", () => {
    const thickness = { depthM: 3, from: "depth" as const, layer: "sea" as const };
    const point = {
      view,
      up,
      normal: up,
      texel,
      resolved: { upwind: 0, crosswind: 0 },
      footprintM: 1,
      thickness,
    };
    // A bed of albedo R∞, lit in air by the sun alone: its radiance R∞ E μ_s ÷ π.
    const muS = dot(up, optics.sunDirection);
    const bedOf = (k: 0 | 1 | 2): number =>
      (optics.deepReflectance[k] * optics.sunLux[k] * muS) / Math.PI;
    const bed: [number, number, number] = [bedOf(0), bedOf(1), bedOf(2)];
    const sunOnly = { sunLux: optics.sunLux, skyLux: NO_SKY };
    const shallow = oceanNearLight(optics, sunOnly, { ...point, bedRadiance: bed });
    const deep = oceanNearLight(optics, sunOnly, {
      ...point,
      thickness: { ...thickness, depthM: 1e6 },
      bedRadiance: bed,
    });
    for (let k = 0; k < 3; k += 1) {
      expect(shallow.radiance[k]).toBeCloseTo(deep.radiance[k] ?? Number.NaN, 9);
    }
  });

  it("takes the refracted bed's light through the column as its hand-made sum does", () => {
    const sunOnly = { sunLux: optics.sunLux, skyLux: NO_SKY };
    const bed: [number, number, number] = [0.5, 0.6, 0.7];
    const depth = 2;
    const lit = oceanNearLight(optics, sunOnly, {
      view,
      up,
      normal: up,
      texel,
      resolved: { upwind: 0, crosswind: 0 },
      footprintM: 1,
      thickness: { depthM: depth, from: "depth", layer: "sea" },
      bedRadiance: bed,
    });
    const k = 1;
    const n = optics.refractiveIndex[k];
    const muS = dot(up, optics.sunDirection);
    const muV = dot(up, view);
    const fs = fresnelReflectance(n, muS);
    const fv = fresnelReflectance(n, muV);
    const kd = optics.attenuationPerM?.[k] ?? 0;
    const through = Math.exp(
      -kd *
        depth *
        (1 / refractedCosine(n, muS) +
          1 / Math.max(refractedCosine(n, muV), OCEAN_NEAR_LEAST_REFRACTED)),
    );
    const entering = optics.sunLux[k] * muS * (1 - fs);
    const water =
      ((1 - fv) * optics.deepReflectance[k] * (1 - through) * entering) / (n * n * UPWELLING_Q_SR) +
      ((bed[k] * (1 - fs) * (1 - fv)) / (n * n)) * through;
    // Less the glint, which the same factors make on both sides.
    const glintless = oceanNearLight({ ...optics, glintCapCdM2: 0 }, sunOnly, {
      view,
      up,
      normal: up,
      texel,
      resolved: { upwind: 0, crosswind: 0 },
      footprintM: 1,
      thickness: { depthM: depth, from: "depth", layer: "sea" },
      bedRadiance: bed,
    });
    expect(glintless.radiance[k]).toBeCloseTo(water, 9);
    expect(lit.radiance[k]).toBeGreaterThan(water);
  });

  it("reflects a wave's normal facing away from the camera towards it", () => {
    const away = normalise(vec3(1, 0, 0.1));
    const v = vec3(-1, 0, 0);
    const facing = facingNormal(away, v);
    expect(dot(facing, v)).toBeGreaterThan(0);
    expect(norm(facing)).toBeCloseTo(1, 12);
    expect(facingNormal(up, view)).toBe(up);
  });
});

describe("the glint across the 20 km switch (R11.T8.c)", () => {
  it.each([
    [7, 2e-4],
    [12, 2e-4],
    [18, 2e-4],
    // A pixel of 2 mrad: 40 m at 20 km, the shorter components within the fade's band.
    [12, 2e-3],
  ])(
    "is continuous to 2% over a patch of sea under a %d m/s wind at %d rad a pixel, glint and whole",
    (windMps, pixelAngleRad) => {
      // The sun 30° from the zenith, the patch about its specular point under a camera at 20 km.
      const sun = normalise(vec3(Math.sin(Math.PI / 6), 0, Math.cos(Math.PI / 6)));
      const optics = opticsOf(sun, pixelAngleRad);
      const up = vec3(0, 0, 1);
      const height = OCEAN_NEAR_BELOW_M;
      const offset = height * Math.tan(Math.PI / 12);
      const cameraM = vec3(-offset, 0, height);
      const texel: OceanTexel = { depthM: 4000, levelM: 0, windEastMps: windMps, windNorthMps: 0 };
      const set = windySet(windMps);
      // The wind towards local east, +y at the pole this patch stands on (`eastNorthOf`).
      const frame = { upwind: vec3(0, 1, 0), across: vec3(-1, 0, 0) };
      const waves = oceanWavesAt(
        set,
        { seconds: 1000, nanos: 0 },
        { pointM: vec3(0, 0, 0), ...frame },
        vec3(0, 0, 0),
      );
      const light = { sunLux: optics.sunLux, skyLux: optics.skyLux };
      const halfWidthM = 2_000;
      const near = oceanNearPatchRadiance(
        { optics, light, texel, cameraM, up, halfWidthM, waves, frame },
        200,
      );
      let sum = 0;
      let glint = 0;
      let weight = 0;
      const samples = 200;
      for (let i = 0; i < samples; i += 1) {
        for (let j = 0; j < samples; j += 1) {
          const p = vec3(
            halfWidthM * ((2 * (i + 0.5)) / samples - 1),
            halfWidthM * ((2 * (j + 0.5)) / samples - 1),
            0,
          );
          const toCamera = sub(cameraM, p);
          const distance = norm(toCamera);
          const view = scale(toCamera, 1 / distance);
          const far = oceanFarLight(optics, light, view, up, texel, distance);
          const w = dot(view, up);
          sum += w * far.radiance[1];
          glint += w * far.glint[1];
          weight += w;
        }
      }
      expect(Math.abs(near.glint[1] - glint / weight) / (glint / weight)).toBeLessThan(0.02);
      expect(Math.abs(near.radiance[1] - sum / weight) / (sum / weight)).toBeLessThan(0.02);
    },
  );
});

describe("the near sea's shader, against its twin's figures (R11.T8.c)", () => {
  it("holds the twin's depth range, cull, fade band, least cosine and components", () => {
    expect(oceanNearWgsl).toContain(
      `const OCEAN_NEAR_DEPTH_MAX : f32 = ${OCEAN_DEPTH_THICKNESS_MAX_M.toExponential(0).replace("+", "")};`,
    );
    expect(oceanNearWgsl).toContain(
      `const OCEAN_NEAR_CULL : f32 = ${OCEAN_LAYER_CULL_SHARE.toExponential(0)};`,
    );
    expect(oceanNearWgsl).toContain(
      `const OCEAN_NEAR_LEAST_REFRACTED : f32 = ${OCEAN_NEAR_LEAST_REFRACTED.toExponential(0)};`,
    );
    const [low, high] = OCEAN_FADE_BAND;
    expect(oceanNearWgsl).toContain(
      `(wavelength / footprint - ${low.toFixed(1)}) / ${(high - low).toFixed(1)}`,
    );
    expect(oceanNearWgsl).toContain(`${String(OCEAN_MAX_COMPONENTS)}u`);
  });
});
