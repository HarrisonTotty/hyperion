import { describe, expect, it } from "vitest";

import { fourAbsorberMedium, lineAbsorberMedium } from "../../test/atmosphereFixtures";
import { forbiddenIn, wgslCode } from "../../test/atmosphereWgsl";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { quaternion } from "../camera/quaternion";
import type { TextureHandle } from "../engine/types";
import { SETTINGS } from "../quality/qualitySetting";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  HILLAIRE_REFERENCE,
  R05_EARTH_REFERENCE,
  referenceSunIlluminanceLx,
} from "./earth";
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
  skyAzimuthOrigin,
  skyViewHeight,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
  viewExactOf,
} from "./hillaire";
import { densityAt, extinction } from "./medium";
import { opticalDepth } from "./opticalDepth";
import aerialReadWgsl from "./shaders/aerialRead.wgsl?raw";
import compositeWgsl from "./shaders/composite.wgsl?raw";
import viewWgsl from "./shaders/view.wgsl?raw";
import { KERNEL_ARM_FORMAT, MULTI_SCATTERING_KERNEL, TRANSMITTANCE_KERNEL } from "./tables";

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
  const top = R05_EARTH_REFERENCE.topHeightM;
  const terms = R05_EARTH_REFERENCE.terms.map((t) => ({
    density: t.density,
    sigma: extinction(t)[1],
  }));
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
          const [, table] = opticalDepth(R05_EARTH_REFERENCE, radius, radius, 0);
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
  illuminanceLx: referenceSunIlluminanceLx(),
  angularRadiusRad: 0.004_65,
};
const DUSK: SunState = { ...NOON, directionBodyFixed: { x: 0, y: 1, z: 0 } };

describe("HillaireAtmosphere", () => {
  it("rebuilds the per-planet tables when the medium changes and not when the sun moves", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    atmosphere.drawFrame(CAMERA, DUSK, SCENE);
    expect(atmosphere.tables.builds).toBe(1);
    atmosphere.setMedium(HILLAIRE_REFERENCE);
    atmosphere.drawFrame(CAMERA, DUSK, SCENE);
    expect(atmosphere.tables.builds).toBe(2);
  });

  it("dispatches the sky view, the aerial perspective and the ray march each frame, and returns the composite", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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

  it("hands the smoke page the frame's sky-view table, aerial volume and ray-march target", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    expect(atmosphere.frameTables.rayMarch).toBeNull();
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const { skyView, aerial, rayMarch } = atmosphere.frameTables;
    expect(skyView.name).toBe("atmosphere sky view");
    // On low too, where no other pass samples the volume.
    expect(aerial.name).toBe("atmosphere aerial perspective");
    expect(rayMarch?.name).toBe("atmosphere ray march");
  });

  it("binds the medium's terms and tables to every per-frame kernel (R08.T6.b)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const tables = atmosphere.tables;
    expect(engine.dispatched).toHaveLength(3);
    for (const { bindings } of engine.dispatched) {
      expect(bindings.buffers).toEqual({ terms: tables.terms });
      expect(bindings.sampled).toMatchObject({
        densityTables: tables.densityTables,
        phaseTables: tables.phaseTables,
        transmittance: tables.transmittance,
        multiScattering: tables.multiScattering,
      });
    }
  });

  it("binds the medium's layer edges to the sky view and the ray march, which cut their steps at them (R08.T6.i)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const edges = (kernel: string): unknown =>
      engine.dispatched.find((d) => d.kernel === kernel)?.bindings.uniforms["layerEdges"];
    expect(edges("atmosphere sky view")).toBe(atmosphere.tables.layerEdges);
    expect(edges("atmosphere ray march")).toBe(atmosphere.tables.layerEdges);
    // The aerial perspective keeps its even steps, and declares no edges.
    expect(edges("atmosphere aerial perspective")).toBeUndefined();
    expect(AERIAL_PERSPECTIVE_KERNEL.reference).not.toMatch(/var<uniform> layerEdges/);
  });

  it("puts the per-frame medium's shell on the camera's own sphere", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    // √(MN) at the equator, 100 km of air, three terms.
    for (const { bindings } of engine.dispatched) {
      expect([...(bindings.uniforms["medium"] ?? [])].slice(0, 3)).toEqual(
        [GAUSSIAN_RADIUS_M[0], 100_000, 3].map(Math.fround),
      );
    }
  });

  it("draws on with the medium its tables keep when they refuse a new one", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const first = R05_EARTH_REFERENCE.terms[0];
    if (first === undefined) {
      throw new Error("Earth's medium has no terms");
    }
    const tooMany = { ...R05_EARTH_REFERENCE, terms: Array.from({ length: 9 }, () => first) };
    expect(() => atmosphere.setMedium(tooMany)).toThrow(/at most 8/);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(engine.dispatched[0]?.bindings.uniforms["medium"]?.[2]).toBe(3);
  });

  it("allocates the low setting's sizes under atmosphere-view and keeps its aerial perspective to the terrain", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const views = engine.textureSpecs.filter((s) => s.category === "atmosphere-view");
    const sizeOf = (name: string): unknown => views.find((s) => s.name === name)?.size;
    expect(sizeOf("atmosphere sky view")).toEqual([128, 64]);
    expect(sizeOf("atmosphere aerial perspective")).toEqual([32, 32, 16]);
    expect(sizeOf("atmosphere ray march")).toEqual([32, 16]);
    expect(atmosphere.aerialPerspective(CAMERA)).toBeNull();
    const high = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    high.drawFrame(CAMERA, NOON, SCENE);
    const volume: TextureHandle | undefined = high.aerialPerspective(CAMERA)?.volume;
    expect(volume?.name).toBe("atmosphere aerial perspective");
  });

  it("is the quality settings' atmosphere field", () => {
    expect(SETTINGS.high.atmosphere).toBe(TABLE_SIZES.high);
    expect(SETTINGS.low.atmosphere).toBe(TABLE_SIZES.low);
  });

  it("scales the sky by the sun's own illuminance in each channel, and its disc by it over its solid angle (R08.T6.d)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const [r, g, b] = NOON.illuminanceLx;
    expect([...(draw.uniforms["skyScale"] ?? [])]).toEqual(
      [r, g, b, SCENE.exposureScale].map(Math.fround),
    );
    const solidAngle = 2 * Math.PI * (1 - Math.cos(NOON.angularRadiusRad));
    const disc = [...(draw.uniforms["sunDisc"] ?? [])];
    for (const [c, e] of NOON.illuminanceLx.entries()) {
      expect((disc[c] ?? Number.NaN) / (e / solidAngle) - 1).toBeCloseTo(0, 6);
    }
    // Twice the illuminance, twice the sky: the tables hold light per unit of it.
    const brighter = atmosphere.drawFrame(
      CAMERA,
      { ...NOON, illuminanceLx: [2 * r, 2 * g, 2 * b] },
      SCENE,
    );
    expect([...(brighter.uniforms["skyScale"] ?? [])].slice(0, 3)).toEqual(
      [2 * r, 2 * g, 2 * b].map(Math.fround),
    );
  });

  it("draws the rebuilt Earth with its ozone's curves, and refuses it without them", async () => {
    const engine = await countingRenderEngine();
    expect(() => new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84)).toThrow(
      /no light's curves/u,
    );
    const atmosphere = new HillaireAtmosphere(
      engine,
      EARTH_REFERENCE,
      TABLE_SIZES.high,
      WGS84,
      EARTH_REFERENCE_CURVES,
    );
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(engine.dispatched).toHaveLength(3);
    // One absorber, the ozone, whose columns the composite's transmittance to space reads.
    expect([...(draw.uniforms["shell"] ?? [])]).toEqual([100_000, 1, 0, 0]);
  });
});

describe("HillaireAtmosphere's aerial perspective and sky view, published (R08.T9.a)", () => {
  it("touches the terrain alone on the low setting: no volume is published, and only the composite over the given scene reads it", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const { aerial } = atmosphere.frameTables;
    expect(atmosphere.aerialPerspective(CAMERA)).toBeNull();
    // The frame's kernels write the volume and read none of it; the composite reads it over the
    // scene it was given, the terrain's colour and depth, and nothing else.
    for (const { bindings } of engine.dispatched) {
      expect(Object.values(bindings.sampled)).not.toContain(aerial);
    }
    expect(draw.textures["aerial"]).toBe(aerial);
    expect(draw.textures["sceneColour"]).toBe(SCENE.colour);
    expect(draw.textures["sceneDepth"]).toBe(SCENE.depth);
  });

  it("publishes on the high setting the volume and the transmittance tables the frame's composite reads", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(
      engine,
      EARTH_REFERENCE,
      TABLE_SIZES.high,
      WGS84,
      EARTH_REFERENCE_CURVES,
    );
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const published = atmosphere.aerialPerspective(CAMERA);
    expect(published?.volume).toBe(draw.textures["aerial"]);
    expect(published?.transmittance).toBe(draw.textures["transmittance"]);
    expect(published?.absorberColumns).toBe(draw.textures["absorberColumns"]);
    expect(published?.absorberCurves).toBe(draw.textures["absorberCurves"]);
    expect(published?.size).toEqual({ widthTexels: 32, heightTexels: 32, slices: 32 });
    expect(published?.reachM).toBe(32_000);
    // The rebuilt Earth's ozone is its one absorber; the composite's own count agrees.
    expect(published?.absorberCount).toBe(1);
    expect(draw.uniforms["shell"]?.[1]).toBe(1);
    expect(published?.bottomRadiusM).toBe(tableRadiusM(WGS84));
    expect(published?.topHeightM).toBe(EARTH_REFERENCE.topHeightM);
    expect(published?.scaleLx).toEqual(NOON.illuminanceLx);
    expect(published?.lineSlots).toBe(0);
  });

  it("publishes the line slots' pressure columns and bitmask the composite reads (R08.T4.d)", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = lineAbsorberMedium();
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const published = atmosphere.aerialPerspective(CAMERA);
    expect(published?.absorberPressureColumns).toBe(draw.textures["absorberPressureColumns"]);
    expect(published?.lastAbsorberPressureColumn).toBe(draw.textures["lastAbsorberPressureColumn"]);
    expect(published?.lineSlots).toBe(draw.uniforms["shell"]?.[2]);
    expect(published?.lineSlots).toBe(2);
  });

  it.each(["high", "low"] as const)(
    "gives skyView(view) the table drawFrame built this frame, with its read, on %s",
    async (setting) => {
      const engine = await countingRenderEngine();
      const atmosphere = new HillaireAtmosphere(
        engine,
        R05_EARTH_REFERENCE,
        TABLE_SIZES[setting],
        WGS84,
      );
      atmosphere.drawFrame(CAMERA, NOON, SCENE);
      const draw = atmosphere.drawFrame(CAMERA, DUSK, SCENE);
      const sky = atmosphere.skyView(CAMERA);
      expect(sky.texture).toBe(draw.textures["skyView"]);
      expect(sky.texture).toBe(atmosphere.frameTables.skyView);
      expect(sky.size).toEqual(TABLE_SIZES[setting].skyView);
      // This frame's sun: its illuminance, and azimuth 0 towards it (dusk, along +y).
      expect(sky.scaleLx).toEqual(DUSK.illuminanceLx);
      expect(sky.azimuthOrigin.y).toBeCloseTo(1, 12);
      const inputs = atmosphereInputs(CAMERA, WGS84);
      expect(sky.zenith).toEqual(inputs.normal);
      expect(sky.bottomRadiusM).toBeCloseTo(GAUSSIAN_RADIUS_M[0], 3);
      expect(sky.heightM).toBe(skyViewHeight(inputs.heightM, R05_EARTH_REFERENCE.topHeightM));
      const exact = viewExactOf(
        CAMERA.positionM,
        WGS84,
        R05_EARTH_REFERENCE.topHeightM,
        inputs,
        DUSK.angularRadiusRad,
      );
      expect(sky.horizonAngleRad).toBe(exact.horizonAngleRad);
      expect(draw.uniforms["exact"]?.[2]).toBe(Math.fround(sky.horizonAngleRad));
    },
  );

  it("takes another object for the view drawFrame drew", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const copy: AtmosphereCamera = {
      ...CAMERA,
      positionM: { ...CAMERA.positionM },
      orientation: { ...CAMERA.orientation },
      viewport: { ...CAMERA.viewport },
    };
    expect(atmosphere.skyView(copy).texture).toBe(atmosphere.frameTables.skyView);
    expect(atmosphere.aerialPerspective(copy)?.volume).toBe(atmosphere.frameTables.aerial);
  });

  it("refuses both before the first frame", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    expect(() => atmosphere.skyView(CAMERA)).toThrow(/no atmosphere frame/);
    expect(() => atmosphere.aerialPerspective(CAMERA)).toThrow(/no atmosphere frame/);
  });

  it.each([
    ["moved", { ...CAMERA, positionM: pointAt(0, 3) }],
    ["turned", { ...CAMERA, orientation: quaternion(0, 0, 1, 0) }],
    ["zoomed", { ...CAMERA, fovXRad: Math.PI / 3 }],
    ["resized", { ...CAMERA, viewport: { widthPx: 64, heightPx: 33 } }],
  ] as const)("refuses both for a view drawFrame did not draw: %s", async (_name, other) => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(() => atmosphere.skyView(other)).toThrow(/another view/);
    expect(() => atmosphere.aerialPerspective(other)).toThrow(/another view/);
  });

  it("refuses both after a device restore until the next frame", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    engine.restore();
    expect(() => atmosphere.skyView(CAMERA)).toThrow(/no atmosphere frame/);
    expect(() => atmosphere.aerialPerspective(CAMERA)).toThrow(/no atmosphere frame/);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(atmosphere.aerialPerspective(CAMERA)?.volume).toBe(atmosphere.frameTables.aerial);
  });

  it("refuses both after a new medium until the next frame, whose tables it may have remade", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const { medium, curves } = lineAbsorberMedium();
    atmosphere.setMedium(medium, curves);
    expect(() => atmosphere.skyView(CAMERA)).toThrow(/no atmosphere frame/);
    expect(() => atmosphere.aerialPerspective(CAMERA)).toThrow(/no atmosphere frame/);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(atmosphere.aerialPerspective(CAMERA)?.lineSlots).toBe(2);
  });

  it("puts the sky view's azimuth 0 along the sun's horizontal direction, as view.wgsl's skyFrame", () => {
    const zenith = { x: 1, y: 0, z: 0 };
    const along = skyAzimuthOrigin(zenith, { x: 0.6, y: 0, z: 0.8 });
    expect(along.x).toBeCloseTo(0, 15);
    expect(along.z).toBeCloseTo(1, 15);
    // The sun at the zenith: skyFrame's fallback axis, y where the zenith lies near x.
    expect(skyAzimuthOrigin(zenith, zenith)).toEqual({ x: 0, y: 1, z: 0 });
    expect(skyAzimuthOrigin({ x: 0, y: 0, z: 1 }, { x: 0, y: 0, z: -1 })).toEqual({
      x: 1,
      y: 0,
      z: 0,
    });
    expect(viewWgsl).toMatch(
      /select\(vec3f\(1\.0, 0\.0, 0\.0\), vec3f\(0\.0, 1\.0, 0\.0\), abs\(z\.x\) > 0\.9\)/u,
    );
  });

  it("reads the volume in the composite through the one read other passes take", () => {
    const code = wgslCode(COMPOSITE_MATERIAL.fragmentWgsl);
    expect(code).toContain(wgslCode(aerialReadWgsl));
    expect(wgslCode(compositeWgsl)).toMatch(
      /aerialPerspectiveAt\(aerial, linearClamp, uv, distance, reach\)/u,
    );
    // The composite no longer forms its own slice coordinate: the read alone maps the volume.
    expect(wgslCode(compositeWgsl)).not.toMatch(/0\.5 \/ slices/u);
    expect(wgslCode(aerialReadWgsl)).toMatch(/distanceM \/ reachM - 0\.5 \/ slices/u);
    expect(wgslCode(aerialReadWgsl)).toMatch(/texelUvFromUnit\(uv, vec2f\(size\.xy\)\)/u);
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
  ])("%s holds no Medium or terms by value", (_name, source) => {
    // A Medium held by value and indexed by a loop variable is copied whole into each invocation,
    // and the ray march did so three times a sample (plan R05's Risks, "R05.T14, the high run's
    // pass timer and atmosphere, diagnosed"): no helper takes one or the terms (R08.T6.b's storage
    // buffer), and no local copies the uniform or the buffer.
    expect(source).not.toMatch(/\bfn\s+\w+\s*\([^)]*:\s*Medium\b/);
    expect(source).not.toMatch(/\bfn\s+\w+\s*\([^)]*:\s*array<Term\b/);
    expect(source).not.toMatch(/=\s*(medium|terms)\s*;/);
  });

  it.each([
    ["the sky-view kernel", SKY_VIEW_KERNEL.reference],
    ["the aerial-perspective kernel", AERIAL_PERSPECTIVE_KERNEL.reference],
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
  ])("%s reads the terms, their densities and their phases (R08.T6.b)", (_name, source) => {
    expect(source).toMatch(/var<storage, read> terms : array<Term, MAX_TERMS>;/);
    expect(source).toMatch(/var densityTables : texture_2d_array<f32>;/);
    expect(source).toMatch(/var phaseTables : texture_2d_array<f32>;/);
    expect(source).toMatch(/let angle = scatteringAngle\(dir, /);
  });

  it("keeps the composite's code clear of the medium, which it does not declare", () => {
    const code = COMPOSITE_MATERIAL.fragmentWgsl.replaceAll(/\/\/[^\n]*/gu, "");
    expect(code).not.toMatch(/\b(medium|terms|densityTables|phaseTables)\b/);
  });
});

describe("the transmittance read as optical depth (R08.T6.c)", () => {
  it.each([
    ["the multiple-scattering kernel", MULTI_SCATTERING_KERNEL.reference],
    ["the sky-view kernel", SKY_VIEW_KERNEL.reference],
    ["the aerial-perspective kernel", AERIAL_PERSPECTIVE_KERNEL.reference],
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the composite", COMPOSITE_MATERIAL.fragmentWgsl],
  ])(
    "%s reads the table only through transmittanceThroughTables, exponentiated in f32",
    (_n, source) => {
      const code = source.replaceAll(/\/\/[^\n]*/gu, "");
      // The one bilinear read of the stored optical depth is transmittanceThroughTables's own, which
      // adds the curves and exponentiates; every read site calls it with the module's two tables.
      expect([
        /tableAtUnit\(transmittance,/u.test(code),
        code.match(/tableAtUnit\(depthTable, unit\)/gu)?.length,
        /return exp\(-\(stored\.rgb \+ absorberDepth\(/u.test(code),
        /transmittanceThroughTables\(\s*\w+,\s*absorberColumns,\s*absorberPressureColumns,\s*lastAbsorberPressureColumn,\s*absorberCurves,/u.test(
          code,
        ),
      ]).toEqual([false, 1, true, true]);
    },
  );

  it("writes the absorbers' pressure-weighted columns beside their columns (R08.T4.d)", () => {
    const code = TRANSMITTANCE_KERNEL.reference.replaceAll(/\/\/[^\n]*/gu, "");
    expect([
      /pressureColumns = addColumns\(pressureColumns, local\.pressureColumns, dt\);/u.test(code),
      /textureStore\(absorberPressureColumnsOut, id\.xy, min\(firstFour, vec4f\(HALF_MAX\)\)\);/u.test(
        code,
      ),
      /textureStore\(\s*lastAbsorberPressureColumnOut, id\.xy,/u.test(code),
    ]).toEqual([true, true, true]);
  });

  it("writes the optical depth and the absorber columns, never e^(−τ), in the transmittance kernel", () => {
    const code = TRANSMITTANCE_KERNEL.reference.replaceAll(/\/\/[^\n]*/gu, "");
    expect(code).toMatch(
      /let stored = min\(vec4f\(ray\.depth, ray\.columns\.first\), vec4f\(HALF_MAX\)\);\s*textureStore\(transmittanceOut, id\.xy, stored\);/u,
    );
    expect(code).toMatch(
      /textureStore\(absorberColumnsOut, id\.xy, min\(ray\.columns\.more, vec4f\(HALF_MAX\)\)\);/u,
    );
    expect(code).toMatch(/const HALF_MAX : f32 = 65504\.0;/u);
    expect(code).not.toMatch(/exp\(-opticalDepth/u);
  });

  it.each([
    ["the multiple-scattering kernel", MULTI_SCATTERING_KERNEL.reference],
    ["the sky-view kernel", SKY_VIEW_KERNEL.reference],
    ["the aerial-perspective kernel", AERIAL_PERSPECTIVE_KERNEL.reference],
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the composite", COMPOSITE_MATERIAL.fragmentWgsl],
  ])(
    "%s declares the absorber-column table, its pressure tables and the lights' curves",
    (_n, source) => {
      expect([
        /var absorberColumns : texture_2d<f32>;/u.test(source),
        /var absorberCurves : texture_2d<f32>;/u.test(source),
        /var absorberPressureColumns : texture_2d<f32>;/u.test(source),
        /var lastAbsorberPressureColumn : texture_2d<f32>;/u.test(source),
      ]).toEqual([true, true, true, true]);
    },
  );

  const absorbing = fourAbsorberMedium();

  it("binds the absorber columns and the lights' curves to every per-frame kernel", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const { absorberColumns, absorberCurves, absorberPressureColumns, lastAbsorberPressureColumn } =
      atmosphere.tables;
    expect(engine.dispatched.map(({ bindings }) => bindings.sampled)).toEqual(
      Array.from({ length: 3 }, () =>
        expect.objectContaining({
          absorberColumns,
          absorberCurves,
          absorberPressureColumns,
          lastAbsorberPressureColumn,
        }),
      ),
    );
  });

  it("gives every per-frame kernel the medium's four absorber slots and light slot 0", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(
      engine.dispatched.map(({ bindings }) => [...(bindings.uniforms["medium"] ?? [])].slice(8)),
    ).toEqual([
      [4, 0, 0, 0],
      [4, 0, 0, 0],
      [4, 0, 0, 0],
    ]);
  });

  it("binds both tables to the composite and gives it the absorbers' count", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const { absorberColumns, absorberCurves } = atmosphere.tables;
    expect(draw.textures).toMatchObject({ absorberColumns, absorberCurves });
    expect([...(draw.uniforms["shell"] ?? [])]).toEqual([100_000, 4, 0, 0]);
  });

  it("binds the pressure-column tables to the composite (R08.T4.d)", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = lineAbsorberMedium();
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const { absorberPressureColumns, lastAbsorberPressureColumn } = atmosphere.tables;
    expect(draw.textures).toMatchObject({ absorberPressureColumns, lastAbsorberPressureColumn });
  });

  it("gives the composite the medium's line slots, slot 1's bit, in shell.z (R08.T4.d)", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = lineAbsorberMedium();
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect([...(draw.uniforms["shell"] ?? [])]).toEqual([100_000, 2, 2, 0]);
  });

  it("declares the curves to the composite as unfilterable rgba32float", () => {
    expect(COMPOSITE_MATERIAL.textures).toContainEqual({
      name: "absorberCurves",
      binding: 8,
      sampleType: "unfilterable-float",
    });
  });

  it("takes new curves for its medium as a new build", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const own = curves[0] ?? [];
    atmosphere.setMedium(medium, [own, own]);
    expect(atmosphere.tables.builds).toBe(2);
  });

  it("refuses a medium with absorbers given no curves", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    expect(() => {
      atmosphere.setMedium(medium);
    }).toThrow(/no light's curves/u);
  });
});

/** The `vec4f` members of a WGSL struct, in order. */
function vec4Members(wgsl: string, struct: string): string[] {
  const body = new RegExp(`struct ${struct} \\{([^}]*)\\}`, "u").exec(wgsl)?.[1] ?? "";
  return [...body.replaceAll(/\/\/[^\n]*/gu, "").matchAll(/(\w+) : vec4f/gu)].map(
    (m) => m[1] ?? "",
  );
}

describe("the atmosphere's arithmetic (R08.T6.g)", () => {
  it.each([
    ["the transmittance kernel", TRANSMITTANCE_KERNEL.reference],
    ["the multiple-scattering kernel", MULTI_SCATTERING_KERNEL.reference],
    ["the sky-view kernel", SKY_VIEW_KERNEL.reference],
    ["the aerial-perspective kernel", AERIAL_PERSPECTIVE_KERNEL.reference],
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the composite", COMPOSITE_MATERIAL.fragmentWgsl],
  ])("%s calls no builtin trigonometry, R05 radius form or sebh remap", (_name, source) => {
    expect(forbiddenIn(source)).toEqual([]);
  });

  it("gives the view's members in one order to the kernels and the composite, `exact` last", () => {
    const view = vec4Members(viewWgsl, "AtmosphereView");
    expect(view.at(-1)).toBe("exact");
    expect(COMPOSITE_MATERIAL.uniforms.map((u) => u.name)).toEqual([...view, "shell"]);
    // The composite's Draw: the offset (a vec3f), then the view's members in order, then shell.
    expect(vec4Members(compositeWgsl, "Draw")).toEqual([...view, "shell"]);
  });

  it("forms the ray march's quadric constants in f64 from the camera's position", () => {
    const sphere = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000 };
    const position = { x: 0, y: 0, z: 6_371_002 };
    const inputs = atmosphereInputs({ positionM: position }, sphere);
    const exact = viewExactOf(position, sphere, 100_000, inputs, 0.004_65);
    // q_g = h (2a + h) and q_t = (h − H)(2a + h + H) on a sphere, 2 m up.
    expect(exact.groundQuadricM2 / (2 * (2 * 6_371_000 + 2)) - 1).toBeCloseTo(0, 9);
    expect(exact.topQuadricM2 / ((2 - 100_000) * (2 * 6_371_000 + 100_002)) - 1).toBeCloseTo(0, 12);
    expect(exact.sinSunRadius).toBe(Math.sin(0.004_65));
    // β = acos(√(h(2R + h)) ÷ (R + h)), the angle from the nadir to the horizon, from 2 m.
    const rho = Math.sqrt(2 * (2 * 6_371_000 + 2));
    expect(exact.horizonAngleRad).toBeCloseTo(Math.acos(rho / 6_371_002), 12);
  });

  it("forms the spheroid's constants with its own z scale, the top shell's with (a + H) ÷ (c + H)", () => {
    const at = pointAt(45, 3);
    const exact = viewExactOf(at, WGS84, 100_000, atmosphereInputs({ positionM: at }, WGS84), 0);
    const a = WGS84.equatorialRadiusM;
    const c = WGS84.polarRadiusM;
    const ground = at.x ** 2 + at.y ** 2 + ((at.z * a) / c) ** 2 - a * a;
    const top =
      at.x ** 2 + at.y ** 2 + ((at.z * (a + 100_000)) / (c + 100_000)) ** 2 - (a + 100_000) ** 2;
    expect(exact.groundQuadricM2).toBe(ground);
    expect(exact.topQuadricM2).toBe(top);
    // 3 m above the datum: q_g ÷ (2a) is about the height along the radius.
    expect(exact.groundQuadricM2 / (2 * a)).toBeGreaterThan(2.9);
    expect(exact.groundQuadricM2 / (2 * a)).toBeLessThan(3.1);
  });

  it("fills the view's exact member and the composite's atmosphere height each frame", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const draw = atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const inputs = atmosphereInputs(CAMERA, WGS84);
    const exact = viewExactOf(CAMERA.positionM, WGS84, 100_000, inputs, NOON.angularRadiusRad);
    const expected = [
      exact.groundQuadricM2,
      exact.topQuadricM2,
      exact.horizonAngleRad,
      exact.sinSunRadius,
    ].map(Math.fround);
    expect([...(draw.uniforms["exact"] ?? [])]).toEqual(expected);
    expect([...(draw.uniforms["shell"] ?? [])]).toEqual([100_000, 0, 0, 0]);
    const block = engine.dispatched.find((d) => d.kernel === "atmosphere sky view")?.bindings
      .uniforms["view"];
    expect(Array.from(block ?? []).slice(-4)).toEqual(expected);
  });

  it("keeps skyViewHeight to view.wgsl's clamp", () => {
    expect(viewWgsl).toContain("return clamp(heightM, 1.0, atmosphereM - 1.0);");
    expect(skyViewHeight(0.25, 100_000)).toBe(1);
    expect(skyViewHeight(500, 100_000)).toBe(500);
    expect(skyViewHeight(400_000, 100_000)).toBe(99_999);
  });

  it("takes view/shaders/preciseTrig.wgsl first in every per-frame module", () => {
    for (const source of [
      SKY_VIEW_KERNEL.reference,
      AERIAL_PERSPECTIVE_KERNEL.reference,
      RAY_MARCH_KERNEL.reference,
      COMPOSITE_MATERIAL.fragmentWgsl,
    ]) {
      expect(source.indexOf("fn preciseSinCos")).toBeLessThan(source.indexOf("struct Term"));
    }
  });
});

describe("HillaireAtmosphere's kernel arm (R08.T6.g)", () => {
  it("dispatches the three per-frame kernels again into rgba32float, over the same tables", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    const shipped = engine.dispatched.slice(-3);
    engine.dispatched.length = 0;
    const arm = atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    expect(engine.dispatched.map((d) => d.kernel)).toEqual([
      `atmosphere sky view (${KERNEL_ARM_FORMAT})`,
      `atmosphere aerial perspective (${KERNEL_ARM_FORMAT})`,
      `atmosphere ray march (${KERNEL_ARM_FORMAT})`,
    ]);
    for (const [i, dispatch] of engine.dispatched.entries()) {
      expect(dispatch.bindings.sampled).toEqual(shipped[i]?.bindings.sampled);
      expect(dispatch.bindings.uniforms).toEqual(shipped[i]?.bindings.uniforms);
      expect(dispatch.workgroups).toEqual(shipped[i]?.workgroups);
    }
    expect(engine.dispatched[0]?.bindings.storage["skyViewOut"]?.texture).toBe(arm.skyView);
    expect(engine.dispatched[1]?.bindings.storage["aerialOut"]?.texture).toBe(arm.aerial);
    expect(engine.dispatched[2]?.bindings.storage["rayMarchOut"]?.texture).toBe(arm.rayMarch);
    const armSpecs = engine.textureSpecs.filter((spec) => spec.name.endsWith("(kernel arm)"));
    expect(armSpecs.map((spec) => spec.format)).toEqual([
      KERNEL_ARM_FORMAT,
      KERNEL_ARM_FORMAT,
      KERNEL_ARM_FORMAT,
    ]);
  });

  it("makes nothing new for a second frame of the same size", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    const textures = engine.counts.textures;
    const kernels = engine.counts.kernels;
    atmosphere.drawKernelArm(CAMERA, DUSK, SCENE);
    expect([engine.counts.textures, engine.counts.kernels]).toEqual([textures, kernels]);
  });

  it("remakes its ray-march target at a new size and releases the old one", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const destroyed: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    const first = atmosphere.drawKernelArm(CAMERA, NOON, SCENE).rayMarch;
    const wider = { ...CAMERA, viewport: { widthPx: 96, heightPx: 32 } };
    const second = atmosphere.drawKernelArm(wider, NOON, SCENE).rayMarch;
    expect(second).not.toBe(first);
    expect(destroyed).toEqual(["atmosphere ray march (kernel arm)"]);
    expect(engine.textureSpecs.at(-1)?.size).toEqual([96, 32]);
  });

  it("makes its kernels and targets anew after a device restore", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const before = atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    engine.restore();
    const after = atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    expect(after.skyView).not.toBe(before.skyView);
    expect(after.rayMarch).not.toBe(before.rayMarch);
  });

  it("releases its three targets when the atmosphere is disposed", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    const destroyed: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    atmosphere.dispose();
    expect(destroyed.toSorted()).toEqual([
      "atmosphere aerial perspective (kernel arm)",
      "atmosphere ray march (kernel arm)",
      "atmosphere sky view (kernel arm)",
    ]);
  });
});
