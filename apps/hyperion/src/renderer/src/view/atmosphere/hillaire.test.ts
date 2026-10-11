import { describe, expect, it } from "vitest";

import { forbiddenIn } from "../../test/atmosphereWgsl";
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
  skyViewHeight,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
  viewExactOf,
} from "./hillaire";
import { densityAt, extinction } from "./medium";
import { opticalDepth } from "./opticalDepth";
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

  it("hands the smoke page the frame's sky-view table, aerial volume and ray-march target", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
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
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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

  it("puts the per-frame medium's shell on the camera's own sphere", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const first = EARTH_REFERENCE.terms[0];
    if (first === undefined) {
      throw new Error("Earth's medium has no terms");
    }
    const tooMany = { ...EARTH_REFERENCE, terms: Array.from({ length: 9 }, () => first) };
    expect(() => atmosphere.setMedium(tooMany)).toThrow(/at most 8/);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, NOON, SCENE);
    expect(engine.dispatched[0]?.bindings.uniforms["medium"]?.[2]).toBe(3);
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
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    const textures = engine.counts.textures;
    const kernels = engine.counts.kernels;
    atmosphere.drawKernelArm(CAMERA, DUSK, SCENE);
    expect([engine.counts.textures, engine.counts.kernels]).toEqual([textures, kernels]);
  });

  it("remakes its ray-march target at a new size and releases the old one", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const before = atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    engine.restore();
    const after = atmosphere.drawKernelArm(CAMERA, NOON, SCENE);
    expect(after.skyView).not.toBe(before.skyView);
    expect(after.rayMarch).not.toBe(before.rayMarch);
  });

  it("releases its three targets when the atmosphere is disposed", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
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
