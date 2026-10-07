import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { quaternion } from "../camera/quaternion";
import type { TextureHandle } from "../engine/types";
import { SETTINGS } from "../quality/qualitySetting";
import { EARTH_REFERENCE, HILLAIRE_REFERENCE } from "./earth";
import {
  AERIAL_PERSPECTIVE_KERNEL,
  atmosphereInputs,
  type AtmosphereCamera,
  type AtmosphereScene,
  COMPOSITE_MATERIAL,
  geodeticOf,
  HillaireAtmosphere,
  RAY_MARCH_KERNEL,
  SKY_VIEW_KERNEL,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import { densityAt, extinction } from "./medium";
import { opticalDepth } from "./opticalDepth";
import { MULTI_SCATTERING_KERNEL, TRANSMITTANCE_KERNEL } from "./tables";

/** WGS 84 (NIMA TR8350.2): a = 6,378,137 m, f = 1 ÷ 298.257223563. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};
const E2 = 1 - (WGS84.polarRadiusM / WGS84.equatorialRadiusM) ** 2;

/**
 * √(MN) at 0°, 45° and 90°, hand-computed from a and f: M = a(1 − e²) ÷ (1 − e² sin²φ)^(3/2),
 * N = a ÷ √(1 − e² sin²φ).
 */
const GAUSSIAN_RADIUS_M = {
  0: 6_356_752.314_245_179,
  45: 6_378_101.030_201_018,
  90: 6_399_593.625_758_493,
};

/** The body-fixed point at geodetic latitude φ, longitude 0 and height h. */
function pointAt(latitudeDeg: number, heightM: number): { x: number; y: number; z: number } {
  const phi = (latitudeDeg * Math.PI) / 180;
  const n = WGS84.equatorialRadiusM / Math.sqrt(1 - E2 * Math.sin(phi) ** 2);
  return {
    x: (n + heightM) * Math.cos(phi),
    y: 0,
    z: (n * (1 - E2) + heightM) * Math.sin(phi),
  };
}

describe("atmosphereInputs", () => {
  for (const latitude of [0, 45, 90] as const) {
    it(`gives r = √(MN) + h and the spheroid's normal at ${latitude}°`, () => {
      const inputs = atmosphereInputs({ positionM: pointAt(latitude, 1_500) }, WGS84);
      expect(inputs.heightM).toBeCloseTo(1_500, 6);
      expect(inputs.radiusM).toBeCloseTo(GAUSSIAN_RADIUS_M[latitude] + 1_500, 5);
      const phi = (latitude * Math.PI) / 180;
      expect(inputs.normal.x).toBeCloseTo(Math.cos(phi), 12);
      expect(inputs.normal.y).toBeCloseTo(0, 12);
      expect(inputs.normal.z).toBeCloseTo(Math.sin(phi), 12);
    });
  }

  it("equals the plain sphere's when a = c", () => {
    const sphere = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000 };
    const p = { x: 3_000_000, y: -4_000_000, z: 3_500_000 };
    const r = Math.hypot(p.x, p.y, p.z);
    const inputs = atmosphereInputs({ positionM: p }, sphere);
    expect(inputs.radiusM).toBeCloseTo(r, 6);
    expect(inputs.heightM).toBeCloseTo(r - 6_371_000, 6);
    expect(inputs.normal.x).toBeCloseTo(p.x / r, 12);
    expect(inputs.normal.z).toBeCloseTo(p.z / r, 12);
  });
});

/** The unit tangent at a ground point of latitude φ, northwards or eastwards. */
function tangent(
  latitudeDeg: number,
  towards: "north" | "east",
): { x: number; y: number; z: number } {
  const phi = (latitudeDeg * Math.PI) / 180;
  return towards === "east" ? { x: 0, y: 1, z: 0 } : { x: -Math.sin(phi), y: 0, z: Math.cos(phi) };
}

/**
 * The optical depth at 550 nm along a ray from `start` along `dir` to the top of the atmosphere,
 * the density taken at each point's geodetic height above WGS 84, in f64 (Simpson's rule).
 */
function spheroidOpticalDepth(
  start: { x: number; y: number; z: number },
  dir: { x: number; y: number; z: number },
): number {
  const top = EARTH_REFERENCE.topHeightM;
  const terms = EARTH_REFERENCE.terms.map((t) => ({ density: t.density, sigma: extinction(t)[1] }));
  const heightAt = (t: number): number =>
    geodeticOf({ x: start.x + t * dir.x, y: start.y + t * dir.y, z: start.z + t * dir.z }, WGS84)
      .heightM;
  // The exit: the first t past which the height exceeds the top, by bisection on a coarse scan.
  let tMax = 0;
  while (heightAt(tMax + 10_000) < top) {
    tMax += 10_000;
  }
  let lo = tMax;
  let hi = tMax + 10_000;
  for (let i = 0; i < 60; i += 1) {
    const mid = (lo + hi) / 2;
    if (heightAt(mid) < top) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  tMax = lo;
  const n = 40_000;
  const dt = tMax / n;
  let sum = 0;
  for (let i = 0; i <= n; i += 1) {
    const w = i === 0 || i === n ? 1 : i % 2 === 1 ? 4 : 2;
    const h = Math.max(heightAt(i * dt), 0);
    for (const term of terms) {
      sum += w * term.sigma * densityAt(term.density, h);
    }
  }
  return (sum * dt) / 3;
}

describe("the spherical tables on the spheroid", () => {
  for (const latitude of [0, 45] as const) {
    for (const towards of ["north", "east"] as const) {
      it(`hold the grazing optical depth from the ground within 0.5% at ${latitude}°, looking ${towards}`, () => {
        const ground = pointAt(latitude, 0);
        const spheroid = spheroidOpticalDepth(ground, tangent(latitude, towards));
        // On the camera's own sphere, as the sky view and the aerial perspective are built, and on
        // R₁, as the per-planet tables are and are read at the point's height.
        for (const radius of [GAUSSIAN_RADIUS_M[latitude], tableRadiusM(WGS84)]) {
          const [, table] = opticalDepth(EARTH_REFERENCE, radius, radius, 0);
          expect(Math.abs(table / spheroid - 1)).toBeLessThan(0.005);
        }
      });
    }
  }
});

const SCENE: AtmosphereScene = {
  colour: { kind: "texture", name: "terrain colour" },
  depth: { kind: "texture", name: "terrain depth" },
  nearM: 0.1,
  exposureScale: 1e-4,
};

/** A camera 2 m above the equator, looking north along the horizon. */
const CAMERA: AtmosphereCamera = {
  positionM: pointAt(0, 2),
  // Camera −z (forward) to body +z (north), camera +y (up) to body +x (the normal): a half turn
  // about (1, 1, 0) ÷ √2.
  orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
  fovXRad: Math.PI / 2,
  viewport: { widthPx: 64, heightPx: 32 },
};

const NOON: SunState = {
  directionBodyFixed: { x: 1, y: 0, z: 0 },
  distanceAu: 1,
  angularRadiusRad: 0.004_65,
};
const DUSK: SunState = { ...NOON, directionBodyFixed: { x: 0, y: 1, z: 0 } };

describe("HillaireAtmosphere", () => {
  it("rebuilds the per-planet tables when the medium changes and not when the sun moves", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    atmosphere.drawFrame(CAMERA, DUSK, SCENE);
    expect(atmosphere.tables.builds).toBe(1);
    atmosphere.setMedium(HILLAIRE_REFERENCE);
    atmosphere.drawFrame(CAMERA, DUSK, SCENE);
    expect(atmosphere.tables.builds).toBe(2);
  });

  it("dispatches the sky view, the aerial perspective and the ray march each frame, and returns the composite", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(engine.dispatched.map((d) => d.kernel)).toEqual([
      "atmosphere sky view",
      "atmosphere aerial perspective",
      "atmosphere ray march",
    ]);
    expect(draw.material.name).toBe("atmosphere composite");
    expect(draw.textures["sceneDepth"]).toBe(SCENE.depth);
  });

  it("makes nothing new after the first frame, and grows the ray-march target only past its size", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const at = (widthPx: number, heightPx: number): AtmosphereCamera => ({
      ...CAMERA,
      viewport: { widthPx, heightPx },
    });
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const textures = engine.counts.textures;
    atmosphere.drawFrame(CAMERA, DUSK, SCENE);
    atmosphere.drawFrame(at(32, 16), DUSK, SCENE);
    expect(engine.counts.textures).toBe(textures);
    atmosphere.drawFrame(at(128, 64), DUSK, SCENE);
    atmosphere.drawFrame(at(64, 32), DUSK, SCENE);
    expect(engine.counts.textures).toBe(textures + 1);
    expect(engine.textureSpecs.at(-1)?.size).toEqual([128, 64]);
  });

  it("hands the smoke page the frame's sky-view table and ray-march target", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    expect(atmosphere.frameTables.rayMarch).toBeNull();
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const { skyView, rayMarch } = atmosphere.frameTables;
    expect(skyView.name).toBe("atmosphere sky view");
    expect(rayMarch?.name).toBe("atmosphere ray march");
  });

  it("allocates the low setting's sizes under atmosphere-view and keeps its aerial perspective to the terrain", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const views = engine.textureSpecs.filter((s) => s.category === "atmosphere-view");
    const sizeOf = (name: string): unknown => views.find((s) => s.name === name)?.size;
    expect(sizeOf("atmosphere sky view")).toEqual([128, 64]);
    expect(sizeOf("atmosphere aerial perspective")).toEqual([32, 32, 16]);
    expect(sizeOf("atmosphere ray march")).toEqual([32, 16]);
    expect(atmosphere.aerialPerspectiveVolume()).toBeNull();
    const high = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const volume: TextureHandle | null = high.aerialPerspectiveVolume();
    expect(volume?.name).toBe("atmosphere aerial perspective");
  });

  it("is the quality settings' atmosphere field", () => {
    expect(SETTINGS.high.atmosphere).toBe(TABLE_SIZES.high);
    expect(SETTINGS.low.atmosphere).toBe(TABLE_SIZES.low);
  });
});

describe("the atmosphere's WGSL", () => {
  it.each([
    ["the transmittance kernel", TRANSMITTANCE_KERNEL.reference],
    ["the multiple-scattering kernel", MULTI_SCATTERING_KERNEL.reference],
    ["the sky-view kernel", SKY_VIEW_KERNEL.reference],
    ["the aerial-perspective kernel", AERIAL_PERSPECTIVE_KERNEL.reference],
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the composite", COMPOSITE_MATERIAL.fragmentWgsl],
  ])("%s holds no Medium by value", (_name, source) => {
    // A Medium held by value and indexed by a loop variable is copied whole into each invocation,
    // and the ray march did so three times a sample (plan R05's Risks, "R05.T14, the high run's
    // pass timer and atmosphere, diagnosed"): no helper takes one, and no local copies the uniform.
    expect(source).not.toMatch(/\bfn\s+\w+\s*\([^)]*:\s*Medium\b/);
    expect(source).not.toMatch(/=\s*medium(\.terms)?\s*;/);
  });
});
