import { describe, expect, it } from "vitest";

import {
  earthBody,
  fourAbsorberMedium,
  gravityOfSpheroid,
  lineAbsorberMedium,
  saturnBody,
  saturnClassMedium,
  SATURN_SPHEROID,
  venusClassMedium,
} from "../../test/atmosphereFixtures";
import {
  arraysByValueFrom,
  forbiddenIn,
  wgslCode,
  wgslFunctions,
  wgslReached,
  wgslStructs,
} from "../../test/atmosphereWgsl";
import { boundNames, countingRenderEngine } from "../../test/countingRenderEngine";
import { lookAlong, quaternion } from "../camera/quaternion";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { TextureHandle } from "../engine/types";
import { kernelBindings } from "../engine/webgpu/compute";
import { SETTINGS } from "../quality/qualitySetting";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  HILLAIRE_REFERENCE,
  R05_EARTH_REFERENCE,
  referenceSunIlluminanceLx,
} from "./earth";
import type { AtmosphereMedium } from "./medium";
import { gravityRatio, principalRadiiM } from "./oblate";
import { MAX_SKY_LIGHT_SLOTS, MAX_SKY_SOURCES } from "./settings";
import {
  AERIAL_PERSPECTIVE_KERNEL,
  atmosphereInputs,
  type AtmosphereCamera,
  type AtmosphereScene,
  bodyOfFigure,
  FIT_SCALE_FLOOR_M,
  COMPOSITE_MATERIAL,
  createAtmosphereFrameKernels,
  frameSourcesOf,
  geodeticOf,
  HillaireAtmosphere,
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  RAY_MARCH_SLOTS_SPECIAL_KERNEL,
  RAY_MARCH_SPECIAL_KERNEL,
  SKY_VIEW_KERNEL,
  localScaleHeightM,
  marchSourcesOf,
  marchSpecialisedFor,
  marchWeightOf,
  skyAzimuthOrigin,
  skyViewColumnOf,
  skyViewHeight,
  type SpheroidFigure,
  skyAxisOf,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
  topShellOf,
  viewExactOf,
  withMarchSpecialised,
} from "./hillaire";
import { marchAreaOf, marchAreaTexels } from "./marchArea";
import { densityAt, extinction } from "./medium";
import { opticalDepth } from "./opticalDepth";
import aerialReadWgsl from "./shaders/aerialRead.wgsl?raw";
import compositeWgsl from "./shaders/composite.wgsl?raw";
import oblateWgsl from "./shaders/oblate.wgsl?raw";
import rayMarchCurvesWgsl from "./shaders/rayMarchCurves.wgsl?raw";
import rayMarchSlotsWgsl from "./shaders/rayMarchSlots.wgsl?raw";
import viewWgsl from "./shaders/view.wgsl?raw";
import {
  type AbsorberCurveSlots,
  type AtmosphereBody,
  AtmosphereTables,
  KERNEL_ARM_FORMAT,
  marchEdgeCountOf,
  MAX_TERMS,
  MULTI_SCATTERING_KERNEL,
  NO_ABSORBER_CURVES,
  TRANSMITTANCE_KERNEL,
} from "./tables";

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

/**
 * Saturn's 1-bar figure, as R08.T3.d's tests give it (Archinal et al. 2018): f = 0.098, past where
 * R05's fixed-point iteration converged.
 */
const SATURN: SpheroidFigure = { equatorialRadiusM: 60_268e3, polarRadiusM: 54_364e3 };

/**
 * Saturn's H₂–He scale height at 1 bar, m: k T ÷ (μ m_u g_ref) at the NSSDCA Saturn fact sheet's
 * 134 K and fractions (μ 2.1436 g mol⁻¹), g_ref 10.4525 m s⁻², as `tablesCpuOblate.test.ts`
 * builds it: 49.7 km.
 */
const SATURN_SCALE_HEIGHT_M = 49_700;

/** The body-fixed point at geodetic latitude φ, longitude λ and height h on a figure (eq. 4-14). */
function figurePoint(
  figure: SpheroidFigure,
  latitudeRad: number,
  longitudeRad: number,
  heightM: number,
): { x: number; y: number; z: number } {
  const a = figure.equatorialRadiusM;
  const e2 = 1 - (figure.polarRadiusM / a) ** 2;
  const sin = Math.sin(latitudeRad);
  const n = a / Math.sqrt(1 - e2 * sin * sin);
  const across = (n + heightM) * Math.cos(latitudeRad);
  return {
    x: across * Math.cos(longitudeRad),
    y: across * Math.sin(longitudeRad),
    z: (n * (1 - e2) + heightM) * sin,
  };
}

describe("geodeticOf's closed form (R08.T6.e)", () => {
  const latitudesDeg = [0, 1e-7, 15, 30, 45, 60, 75, 89.999_99, 90];
  const heightsM = [
    0,
    1,
    1_000,
    ...[0.3, 1, 3, 10].map((scaleHeights) => scaleHeights * SATURN_SCALE_HEIGHT_M),
  ];

  it("round-trips a Saturn-class figure to 10⁻⁶ m from the datum to 10 scale heights", () => {
    let worstM = 0;
    for (const latitudeDeg of latitudesDeg) {
      for (const longitudeDeg of [0, 37, -120]) {
        for (const heightM of heightsM) {
          const phi = (latitudeDeg * Math.PI) / 180;
          const lambda = (longitudeDeg * Math.PI) / 180;
          const p = figurePoint(SATURN, phi, lambda, heightM);
          const g = geodeticOf(p, SATURN);
          const back = figurePoint(SATURN, g.latitudeRad, g.longitudeRad, g.heightM);
          worstM = Math.max(
            worstM,
            Math.abs(g.heightM - heightM),
            Math.hypot(back.x - p.x, back.y - p.y, back.z - p.z),
          );
        }
      }
    }
    expect(worstM).toBeLessThan(1e-6);
  });

  it("gives the datum's normal at the point's own latitude on a Saturn-class figure", () => {
    let worst = 0;
    for (const latitudeDeg of latitudesDeg) {
      for (const heightM of heightsM) {
        const phi = (latitudeDeg * Math.PI) / 180;
        const g = geodeticOf(figurePoint(SATURN, phi, 0.7, heightM), SATURN);
        worst = Math.max(
          worst,
          Math.abs(g.normal.x - Math.cos(phi) * Math.cos(0.7)),
          Math.abs(g.normal.y - Math.cos(phi) * Math.sin(0.7)),
          Math.abs(g.normal.z - Math.sin(phi)),
        );
      }
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("round-trips WGS 84 and a figure flattened to c ÷ a = 0.2 as closely", () => {
    for (const figure of [WGS84, { equatorialRadiusM: 7e7, polarRadiusM: 1.4e7 }]) {
      for (const latitudeDeg of latitudesDeg) {
        for (const heightM of [0, 2, 20_000, 400_000]) {
          const phi = (latitudeDeg * Math.PI) / 180;
          const p = figurePoint(figure, phi, 0.3, heightM);
          const g = geodeticOf(p, figure);
          const back = figurePoint(figure, g.latitudeRad, g.longitudeRad, g.heightM);
          expect(Math.abs(g.heightM - heightM)).toBeLessThan(1e-6);
          expect(Math.hypot(back.x - p.x, back.y - p.y, back.z - p.z)).toBeLessThan(1e-6);
        }
      }
    }
  });

  it("gives the Gaussian radius √(MN) at the point's latitude on Saturn", () => {
    for (const latitudeDeg of [0, 45, 90]) {
      const phi = (latitudeDeg * Math.PI) / 180;
      const a = SATURN.equatorialRadiusM;
      const e2 = 1 - (SATURN.polarRadiusM / a) ** 2;
      const w2 = 1 - e2 * Math.sin(phi) ** 2;
      const expected = Math.sqrt(((a * (1 - e2)) / (w2 * Math.sqrt(w2))) * (a / Math.sqrt(w2)));
      const g = geodeticOf(figurePoint(SATURN, phi, 0, 10_000), SATURN);
      expect(Math.abs(g.gaussianRadiusM / expected - 1)).toBeLessThan(1e-12);
    }
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

/** The body's table a new atmosphere on `figure` uploads, as floats (R08.T6.f). */
async function oblateBodyTexelsOf(figure: SpheroidFigure): Promise<number[]> {
  const engine = await countingRenderEngine();
  const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, figure);
  atmosphere.dispose();
  const written = engine.textureWritten.findLast((w) => w.texture === "atmosphere oblate body");
  return [...new Float32Array(written?.data.slice().buffer ?? new ArrayBuffer(0))];
}

describe("HillaireAtmosphere", () => {
  it("rebuilds the per-planet tables when the medium changes and not when the sun moves", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    atmosphere.drawFrame(CAMERA, [DUSK], SCENE);
    expect(atmosphere.tables.builds).toBe(1);
    atmosphere.setMedium(HILLAIRE_REFERENCE);
    atmosphere.drawFrame(CAMERA, [DUSK], SCENE);
    expect(atmosphere.tables.builds).toBe(2);
  });

  it("dispatches the sky view, the aerial perspective and the ray march each frame, and returns the composite", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    // An atmosphere's own kernels march the whole frame through the footprint's entry point
    // (R08.T11.d).
    expect(engine.dispatched.map((d) => d.kernel)).toEqual([
      "atmosphere sky view",
      "atmosphere aerial perspective",
      "atmosphere ray march",
    ]);
    expect(draw.material.name).toBe("atmosphere composite");
    expect(draw.textures["sceneDepth"]).toBe(SCENE.depth);
  });

  // From the march band's upper edge up the composite reads neither the sky view nor the aerial
  // perspective, so each waits for a reader (lever 2 of `decision-r08-t8-leans.md` §8, R08.T8.d).
  describe("from the march band's upper edge up (R08.T8.d)", () => {
    const orbit: AtmosphereCamera = { ...CAMERA, positionM: pointAt(0, 400_000) };
    const at = async (
      sizes = TABLE_SIZES.high,
    ): Promise<{
      engine: Awaited<ReturnType<typeof countingRenderEngine>>;
      atmosphere: HillaireAtmosphere;
    }> => {
      const engine = await countingRenderEngine();
      return {
        engine,
        atmosphere: new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, sizes, WGS84),
      };
    };

    it("dispatches the ray march alone", async () => {
      const { engine, atmosphere } = await at();
      engine.dispatched.length = 0;
      atmosphere.drawFrame(orbit, [NOON], SCENE);
      expect(engine.dispatched.map((d) => d.kernel)).toEqual(["atmosphere ray march"]);
    });

    it("makes each other table at its reader's first ask, once a frame", async () => {
      const { engine, atmosphere } = await at();
      atmosphere.drawFrame(orbit, [NOON], SCENE);
      engine.dispatched.length = 0;
      atmosphere.skyView(orbit);
      atmosphere.skyView(orbit);
      expect(atmosphere.aerialPerspective(orbit)).not.toBeNull();
      expect(atmosphere.frameTables.aerial.name).toBe("atmosphere aerial perspective");
      expect(engine.dispatched.map((d) => d.kernel)).toEqual([
        "atmosphere sky view",
        "atmosphere aerial perspective",
      ]);
    });

    it("makes a late table on its own frame's view, though the kernel arm drew another since", async () => {
      const { engine, atmosphere } = await at();
      atmosphere.drawFrame(orbit, [NOON], SCENE);
      const march = engine.dispatched.at(-1)?.bindings.uniforms["view"];
      atmosphere.drawKernelArm({ ...orbit, positionM: pointAt(0, 500_000) }, [DUSK], SCENE);
      const arm = engine.dispatched.at(-1)?.bindings.uniforms["view"];
      atmosphere.skyView(orbit);
      const sky = engine.dispatched.at(-1);
      expect(sky?.kernel).toBe("atmosphere sky view");
      expect(sky?.bindings.uniforms["view"]).toEqual(march);
      expect(sky?.bindings.uniforms["view"]).not.toEqual(arm);
    });

    it("still makes the kernel arm's three tables", async () => {
      const { engine, atmosphere } = await at();
      atmosphere.drawFrame(orbit, [NOON], SCENE);
      engine.dispatched.length = 0;
      atmosphere.drawKernelArm(orbit, [NOON], SCENE);
      expect(engine.dispatched).toHaveLength(3);
    });

    it.each([
      [
        "the next frame",
        (a: HillaireAtmosphere): void => {
          a.drawFrame(CAMERA, [DUSK], SCENE);
        },
      ],
      ["a new medium", (a: HillaireAtmosphere): void => a.setMedium(HILLAIRE_REFERENCE)],
      ["disposal", (a: HillaireAtmosphere): void => a.dispose()],
    ] as const)("drops what the frame left unread at %s", async (_name, after) => {
      const { engine, atmosphere } = await at();
      atmosphere.drawFrame(orbit, [NOON], SCENE);
      after(atmosphere);
      engine.dispatched.length = 0;
      expect(atmosphere.frameTables.skyView.name).toBe("atmosphere sky view");
      expect(engine.dispatched).toEqual([]);
    });

    it("drops what the frame left unread at a device restore", async () => {
      const { engine, atmosphere } = await at();
      atmosphere.drawFrame(orbit, [NOON], SCENE);
      engine.restore();
      engine.dispatched.length = 0;
      expect(atmosphere.frameTables.skyView.name).toBe("atmosphere sky view");
      expect(engine.dispatched).toEqual([]);
    });

    it("on low, where the composite alone reads the volume, publishes none and makes it only for the smoke's read", async () => {
      const { engine, atmosphere } = await at(TABLE_SIZES.low);
      atmosphere.drawFrame(orbit, [NOON], SCENE);
      engine.dispatched.length = 0;
      expect(atmosphere.aerialPerspective(orbit)).toBeNull();
      expect(engine.dispatched).toEqual([]);
      expect(atmosphere.frameTables.aerial.name).toBe("atmosphere aerial perspective");
      expect(engine.dispatched.map((d) => d.kernel)).toEqual([
        "atmosphere sky view",
        "atmosphere aerial perspective",
      ]);
    });
  });

  it("makes nothing new after the first frame, and grows the ray-march target only past its size", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const at = (widthPx: number, heightPx: number): AtmosphereCamera => ({
      ...CAMERA,
      viewport: { widthPx, heightPx },
    });
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const textures = engine.counts.textures;
    atmosphere.drawFrame(CAMERA, [DUSK], SCENE);
    atmosphere.drawFrame(at(32, 16), [DUSK], SCENE);
    expect(engine.counts.textures).toBe(textures);
    atmosphere.drawFrame(at(128, 64), [DUSK], SCENE);
    atmosphere.drawFrame(at(64, 32), [DUSK], SCENE);
    expect(engine.counts.textures).toBe(textures + 1);
    expect(engine.textureSpecs.at(-1)?.size).toEqual([128, 64]);
  });

  it("hands the smoke page the frame's sky-view table, aerial volume and ray-march target", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    expect(atmosphere.frameTables.rayMarch).toBeNull();
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const tables = atmosphere.tables;
    expect(engine.dispatched).toHaveLength(3);
    for (const { bindings } of engine.dispatched) {
      expect(bindings.buffers["terms"]).toBe(tables.terms);
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
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(engine.dispatched[0]?.bindings.uniforms["medium"]?.[2]).toBe(3);
  });

  it("allocates the low setting's sizes under atmosphere-view and keeps its aerial perspective to the terrain", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const views = engine.textureSpecs.filter((s) => s.category === "atmosphere-view");
    const sizeOf = (name: string): unknown => views.find((s) => s.name === name)?.size;
    expect(sizeOf("atmosphere sky view")).toEqual([128, 64]);
    expect(sizeOf("atmosphere aerial perspective")).toEqual([32, 32, 16]);
    expect(sizeOf("atmosphere ray march")).toEqual([32, 16]);
    expect(atmosphere.aerialPerspective(CAMERA)).toBeNull();
    const high = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    high.drawFrame(CAMERA, [NOON], SCENE);
    const volume: TextureHandle | undefined = high.aerialPerspective(CAMERA)?.volume;
    expect(volume?.name).toBe("atmosphere aerial perspective");
  });

  it("is the quality settings' atmosphere field", () => {
    expect(SETTINGS.high.atmosphere).toBe(TABLE_SIZES.high);
    expect(SETTINGS.low.atmosphere).toBe(TABLE_SIZES.low);
  });

  it("weights each source by its illuminance times the pre-exposure, its disc by it over its solid angle (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const [r, g, b] = NOON.illuminanceLx;
    const k = SCENE.exposureScale;
    // One source, no absorber: path 0; the composite's factor 1, the tables pre-exposed.
    expect([...(draw.uniforms["sky"] ?? [])]).toEqual([1, 1, 1, 1]);
    expect([...(draw.uniforms["sourceWeight0"] ?? [])]).toEqual(
      [r * k, g * k, b * k, 0].map(Math.fround),
    );
    const s = NOON.directionBodyFixed;
    expect([...(draw.uniforms["sourceDirection0"] ?? [])]).toEqual(
      [s.x, s.y, s.z, Math.sin(NOON.angularRadiusRad)].map(Math.fround),
    );
    const solidAngle = 2 * Math.PI * (1 - Math.cos(NOON.angularRadiusRad));
    const disc = [...(draw.uniforms["sourceDisc0"] ?? [])];
    for (const [c, e] of NOON.illuminanceLx.entries()) {
      expect((disc[c] ?? Number.NaN) / ((e * k) / solidAngle) - 1).toBeCloseTo(0, 6);
    }
    // The sources past the drawn ones weigh nothing.
    expect([...(draw.uniforms["sourceWeight1"] ?? [])]).toEqual([0, 0, 0, 0]);
    // Twice the illuminance, twice the weight.
    const brighter = atmosphere.drawFrame(
      CAMERA,
      [{ ...NOON, illuminanceLx: [2 * r, 2 * g, 2 * b] }],
      SCENE,
    );
    expect([...(brighter.uniforms["sourceWeight0"] ?? [])].slice(0, 3)).toEqual(
      [2 * r * k, 2 * g * k, 2 * b * k].map(Math.fround),
    );
  });

  it("draws no disc for a source that asks for none, keeping its disc's extent (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const draw = atmosphere.drawFrame(CAMERA, [{ ...NOON, drawsDisc: false }, DUSK], SCENE);
    expect([...(draw.uniforms["sourceDisc0"] ?? [])]).toEqual([0, 0, 0, 0]);
    // Its disc's extent stays, for the transmittance of what R06 draws there.
    expect(draw.uniforms["sourceDirection0"]?.[3]).toBe(
      Math.fround(Math.sin(NOON.angularRadiusRad)),
    );
    expect(draw.uniforms["sky"]?.[0]).toBe(2);
  });

  it("refuses more sources than the kernels hold (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const seven = Array.from({ length: MAX_SKY_SOURCES + 1 }, () => NOON);
    expect(() => atmosphere.drawFrame(CAMERA, seven, SCENE)).toThrow(/hold 6 sources/u);
  });

  it("refuses a frame scale that is not finite and above 0 (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    expect(() => atmosphere.drawFrame(CAMERA, [NOON], { ...SCENE, exposureScale: 0 })).toThrow(
      /finite and above 0/u,
    );
  });

  it.each([4, 1.5, -1])(
    "refuses a source on light slot %d, outside the kernels' four (R08.T7)",
    async (curveSlot) => {
      const engine = await countingRenderEngine();
      const atmosphere = new HillaireAtmosphere(
        engine,
        R05_EARTH_REFERENCE,
        TABLE_SIZES.high,
        WGS84,
      );
      expect(() => atmosphere.drawFrame(CAMERA, [{ ...NOON, curveSlot }], SCENE)).toThrow(
        /light slot must be an integer in \[0, 4\)/u,
      );
    },
  );

  it("refuses, under an absorber, a source on a light slot its curves do not hold (R08.T7)", () => {
    const { curves } = fourAbsorberMedium();
    expect(() => frameSourcesOf([{ ...NOON, curveSlot: 1 }], 1, true, curves.length)).toThrow(
      /curves for 1 lights/u,
    );
    expect(frameSourcesOf([{ ...NOON, curveSlot: 1 }], 1, false, 1)[0]?.weight[3]).toBe(0);
  });

  it("puts the sky view's axis on the first source's azimuth (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const tilted: SunState = {
      ...NOON,
      directionBodyFixed: { x: Math.SQRT1_2, y: 0, z: Math.SQRT1_2 },
    };
    const draw = atmosphere.drawFrame(CAMERA, [tilted, DUSK], SCENE);
    // The camera's normal is +x at the equator: the axis is the first source's horizontal, +z.
    expect([...(draw.uniforms["skyAxis"] ?? [])]).toEqual([0, 0, 1, 0]);
    expect(skyAxisOf({ x: 1, y: 0, z: 0 }, [])).toEqual({ x: 0, y: 1, z: 0 });
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
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(engine.dispatched).toHaveLength(3);
    // One absorber, the ozone, whose columns the composite's transmittance to space reads.
    expect([...(draw.uniforms["shell"] ?? [])]).toEqual([100_000, 1, 0, 0]);
  });
});

/** A ray-march kernel's source before its entry point's file (R08.T11.d), the march it shares. */
function beforeEntryPoint(source: string): string {
  return source.slice(0, source.lastIndexOf("// The ray march's entry point"));
}

/** A view block's member, its four floats, by its place in `hillaire.ts`'s `VIEW_MEMBERS`. */
function memberOf(block: unknown, index: number): number[] {
  if (!(block instanceof Float32Array)) {
    throw new Error("a dispatch's view is a block of floats");
  }
  return [...block.subarray(index * 4, index * 4 + 4)];
}

/** The view's members before its sources: camera, up, sky, skyAxis, …, marchGround, marchSky. */
const VIEW_HEAD_MEMBERS = 14;

/** The place of the view's `sky` member. */
const SKY_MEMBER = 2;

/** The place of source `i`'s weight among the view's members, after its direction. */
function weightMemberOf(i: number): number {
  return VIEW_HEAD_MEMBERS + 1 + 3 * i;
}

describe("the march's sources (R08.T8.e, decision-r08-t8-leans.md §9)", () => {
  // Four sources left by the skips: the Sun, a companion on light slot 1, a full Moon and a bright
  // neighbour, the second brightest, after the companion in the list.
  const companion: SunState = {
    directionBodyFixed: { x: 0.6, y: 0.8, z: 0 },
    illuminanceLx: [900, 1_000, 1_100],
    angularRadiusRad: 0,
    curveSlot: 1,
  };
  const moon: SunState = {
    directionBodyFixed: { x: 0.6, y: 0, z: 0.8 },
    illuminanceLx: [0.3, 0.32, 0.33],
    angularRadiusRad: 0.004_5,
    drawsDisc: false,
  };
  const neighbour: SunState = {
    directionBodyFixed: { x: 0.8, y: -0.6, z: 0 },
    illuminanceLx: [2_000, 2_000, 2_000],
    angularRadiusRad: 0.01,
    drawsDisc: false,
  };
  const four: readonly SunState[] = Object.freeze([NOON, companion, moon, neighbour]);

  it("takes the two brightest at the camera, in their order, and leaves the list as given", () => {
    expect(marchSourcesOf(four, 2)).toEqual([NOON, neighbour]);
    expect(marchSourcesOf(four, 2)[1]).toBe(neighbour);
    expect(four).toEqual([NOON, companion, moon, neighbour]);
  });

  it("takes every source as given at or under the cap, and the earlier of two alike", () => {
    expect(marchSourcesOf([moon, NOON], 2)).toEqual([moon, NOON]);
    expect(marchSourcesOf([], 2)).toEqual([]);
    const twin: SunState = { ...companion, curveSlot: 0 };
    expect(marchSourcesOf([moon, companion, twin], 2)).toEqual([companion, twin]);
    expect(marchSourcesOf([companion, twin, NOON], 1)).toEqual([NOON]);
  });

  it("refuses a cap that is not a whole number of at least 1", () => {
    for (const cap of [0, 1.5, Number.NaN, -2]) {
      expect(() => marchSourcesOf(four, cap)).toThrow(RangeError);
    }
  });

  it("refuses such a cap where the sizes are given", async () => {
    const engine = await countingRenderEngine();
    const sizes = { ...TABLE_SIZES.high, marchSourceCap: 0 };
    expect(() => new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, sizes, WGS84)).toThrow(
      /source cap/u,
    );
  });

  it("marches the two brightest of four, where the sky view, the aerial perspective and the composite take all four", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(CAMERA, four, SCENE);
    const [sky, aerial, march] = engine.dispatched;
    expect([sky?.kernel, aerial?.kernel, march?.kernel]).toEqual([
      "atmosphere sky view",
      "atmosphere aerial perspective",
      "atmosphere ray march",
    ]);
    expect(memberOf(sky?.bindings.uniforms["view"], SKY_MEMBER)[0]).toBe(4);
    expect(memberOf(aerial?.bindings.uniforms["view"], SKY_MEMBER)[0]).toBe(4);
    expect(draw.uniforms["sky"]?.[0]).toBe(4);
    const view = march?.bindings.uniforms["view"];
    expect(memberOf(view, SKY_MEMBER).slice(0, 2)).toEqual([2, 1]);
    const marched = frameSourcesOf([NOON, neighbour], SCENE.exposureScale, false, 0);
    for (const [i, source] of marched.entries()) {
      expect(memberOf(view, weightMemberOf(i))).toEqual(source.weight.map((w) => Math.fround(w)));
    }
    for (let i = 2; i < MAX_SKY_SOURCES; i += 1) {
      expect(memberOf(view, weightMemberOf(i))).toEqual([0, 0, 0, 0]);
    }
    // Every other member is the frame's own.
    const own = sky?.bindings.uniforms["view"];
    const others = Array.from({ length: VIEW_HEAD_MEMBERS }, (_, m) => m).filter(
      (m) => m !== SKY_MEMBER,
    );
    for (const member of others) {
      expect(memberOf(view, member)).toEqual(memberOf(own, member));
    }
  });

  it("marches the frame's own view where it draws no more than the cap", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, [NOON, moon], SCENE);
    const [sky, , march] = engine.dispatched;
    expect(march?.bindings.uniforms["view"]).toBe(sky?.bindings.uniforms["view"]);
  });
});

describe("HillaireAtmosphere's aerial perspective and sky view, published (R08.T9.a)", () => {
  it("touches the terrain alone on the low setting: no volume is published, and only the composite over the given scene reads it", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    // The tables hold pre-exposed luminance since R08.T7: the factor back is 1 ÷ the pre-exposure.
    const unit = 1 / SCENE.exposureScale;
    expect(published?.scaleCdPerM2).toEqual([unit, unit, unit]);
    expect(published?.lineSlots).toBe(0);
  });

  it("publishes the line slots' pressure columns and bitmask the composite reads (R08.T4.d)", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = lineAbsorberMedium();
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
      atmosphere.drawFrame(CAMERA, [NOON], SCENE);
      const draw = atmosphere.drawFrame(CAMERA, [DUSK], SCENE);
      const sky = atmosphere.skyView(CAMERA);
      expect(sky.texture).toBe(draw.textures["skyView"]);
      expect(sky.texture).toBe(atmosphere.frameTables.skyView);
      expect(sky.size).toEqual(TABLE_SIZES[setting].skyView);
      // The frame's scale, 1 ÷ its pre-exposure (R08.T7), and azimuth 0 towards this frame's first
      // source (dusk, along +y).
      const unit = 1 / SCENE.exposureScale;
      expect(sky.scaleCdPerM2).toEqual([unit, unit, unit]);
      expect(sky.azimuthOrigin.y).toBeCloseTo(1, 12);
      const inputs = atmosphereInputs(CAMERA, WGS84);
      expect(sky.zenith).toEqual(inputs.normal);
      // R08.T6.f: each column on its own sphere R_α of the camera's M and N, the atmosphere the
      // medium's top over the camera's s (1 with no gravity), and no one sphere on WGS 84.
      const { meridionalM, primeVerticalM } = principalRadiiM(WGS84, inputs.latitudeRad);
      expect(sky.meridionalRadiusM).toBe(meridionalM);
      expect(sky.primeVerticalRadiusM).toBe(primeVerticalM);
      expect(sky.atmosphereM).toBe(R05_EARTH_REFERENCE.topHeightM);
      expect(sky.heightM).toBe(skyViewHeight(inputs.heightM, sky.atmosphereM));
      expect(sky.sphere).toBeNull();
      expect([...(draw.uniforms["curvature"] ?? [])]).toEqual(
        [meridionalM, primeVerticalM, 1, R05_EARTH_REFERENCE.topHeightM].map(Math.fround),
      );
      // Along the meridian the column's sphere is M, along the prime vertical N; β from it.
      const north = { x: 0, y: 0, z: 1 };
      const east = { x: 0, y: 1, z: 0 };
      const alongNorth = skyViewColumnOf(sky, north);
      expect(alongNorth.bottomRadiusM).toBeCloseTo(meridionalM, 3);
      expect(skyViewColumnOf(sky, east).bottomRadiusM).toBeCloseTo(primeVerticalM, 3);
      const rho = Math.sqrt(sky.heightM * (2 * meridionalM + sky.heightM));
      expect(alongNorth.horizonAngleRad).toBeCloseTo(Math.atan2(meridionalM, rho), 12);
    },
  );

  it("packs the figure itself into the body's table with no gravity given, and a sphere's R_ref (R08.T6.f)", async () => {
    // WGS 84 with no gravity: the datum the ray march's heights stand on is WGS 84's, s ≡ 1.
    const a = WGS84.equatorialRadiusM;
    const c = WGS84.polarRadiusM;
    const wgs84 = await oblateBodyTexelsOf(WGS84);
    expect(wgs84.slice(0, 4)).toEqual(
      [a, c, tableRadiusM(WGS84), ((a - c) * (a + c)) / (a * a)].map(Math.fround),
    );
    expect(wgs84.slice(4, 8)).toEqual([0, 0, 1, 1]);
    expect(wgs84.slice(8, 10)).toEqual([1, 1]);
    const sphere = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000 };
    expect((await oblateBodyTexelsOf(sphere)).slice(0, 4)).toEqual([
      6_371_000, 6_371_000, 6_371_000, 0,
    ]);
    expect(bodyOfFigure(sphere)).toBeNull();
    expect(bodyOfFigure(WGS84)?.gravity.spheroid).toBeNull();
  });

  it("gives a body's gravity to the frame: s at the camera, the air's height over it, the body's table (R08.T6.f)", async () => {
    const engine = await countingRenderEngine();
    const gravity = gravityOfSpheroid(SATURN_SPHEROID);
    const atmosphere = new HillaireAtmosphere(
      engine,
      R05_EARTH_REFERENCE,
      TABLE_SIZES.high,
      SATURN_SPHEROID,
      NO_ABSORBER_CURVES,
      gravity,
    );
    const phi = Math.PI / 4;
    const a = SATURN_SPHEROID.equatorialRadiusM;
    const e2 = 1 - (SATURN_SPHEROID.polarRadiusM / a) ** 2;
    const n = a / Math.sqrt(1 - e2 * Math.sin(phi) ** 2);
    const camera: AtmosphereCamera = {
      ...CAMERA,
      positionM: {
        x: (n + 2_000) * Math.cos(phi),
        y: 0,
        z: (n * (1 - e2) + 2_000) * Math.sin(phi),
      },
    };
    const draw = atmosphere.drawFrame(camera, [NOON], SCENE);
    const inputs = atmosphereInputs(camera, SATURN_SPHEROID, gravity);
    const s = gravityRatio(gravity, inputs.latitudeRad);
    expect(inputs.latitudeRad).toBeCloseTo(phi, 12);
    expect(inputs.gravityRatio).toBe(s);
    expect(Math.abs(s - 1)).toBeGreaterThan(1e-3);
    const { meridionalM, primeVerticalM } = principalRadiiM(SATURN_SPHEROID, inputs.latitudeRad);
    const top = R05_EARTH_REFERENCE.topHeightM;
    expect([...(draw.uniforms["curvature"] ?? [])]).toEqual(
      [meridionalM, primeVerticalM, s, top / s].map(Math.fround),
    );
    const sky = atmosphere.skyView(camera);
    expect([sky.atmosphereM, sky.sphere]).toEqual([top / s, null]);
    // The tables in the body's slices and bands, every reader given the body's table.
    expect(atmosphere.tables.plan.kappa).toEqual(Array.from(gravity.slicing.kappa));
    expect(atmosphere.tables.plan.bands).toHaveLength(gravity.slicing.bandGravityRatio.length);
    expect(draw.textures["oblateBody"]).toBe(atmosphere.tables.oblateBody);
    expect(atmosphere.aerialPerspective(camera)?.oblateBody).toBe(atmosphere.tables.oblateBody);
  });

  it("keeps a sphere's one sky-view sphere, its radius and β, as R08.T9.a published them (R08.T6.f)", async () => {
    const engine = await countingRenderEngine();
    const sphere = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000 };
    const atmosphere = new HillaireAtmosphere(
      engine,
      R05_EARTH_REFERENCE,
      TABLE_SIZES.high,
      sphere,
    );
    const camera: AtmosphereCamera = { ...CAMERA, positionM: { x: 6_371_002, y: 0, z: 0 } };
    atmosphere.drawFrame(camera, [NOON], SCENE);
    const sky = atmosphere.skyView(camera);
    const rho = Math.sqrt(sky.heightM * (2 * 6_371_000 + sky.heightM));
    expect(sky.sphere?.bottomRadiusM).toBe(6_371_000);
    expect(sky.sphere?.horizonAngleRad).toBeCloseTo(Math.acos(rho / (6_371_000 + sky.heightM)), 12);
    for (const d of [
      { x: 0, y: 0, z: 1 },
      { x: 0, y: Math.SQRT1_2, z: Math.SQRT1_2 },
      { x: 1, y: 0, z: 0 },
    ]) {
      expect(skyViewColumnOf(sky, d)).toEqual(sky.sphere);
    }
  });

  it("takes another object for the view drawFrame drew", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(() => atmosphere.skyView(other)).toThrow(/another view/);
    expect(() => atmosphere.aerialPerspective(other)).toThrow(/another view/);
  });

  it("refuses both after a device restore until the next frame", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    engine.restore();
    expect(() => atmosphere.skyView(CAMERA)).toThrow(/no atmosphere frame/);
    expect(() => atmosphere.aerialPerspective(CAMERA)).toThrow(/no atmosphere frame/);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(atmosphere.aerialPerspective(CAMERA)?.volume).toBe(atmosphere.frameTables.aerial);
  });

  it("refuses both after a new medium until the next frame, whose tables it may have remade", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const { medium, curves } = lineAbsorberMedium();
    atmosphere.setMedium(medium, curves);
    expect(() => atmosphere.skyView(CAMERA)).toThrow(/no atmosphere frame/);
    expect(() => atmosphere.aerialPerspective(CAMERA)).toThrow(/no atmosphere frame/);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    // Since R08.T7 the kernels take the axis formed here, `AtmosphereView.skyAxis`, at the first
    // source (`skyAxisOf`), in place of forming it in f32.
    expect(viewWgsl).toMatch(/let x = v\.skyAxis\.xyz;/u);
    expect(skyAxisOf(zenith, [])).toEqual(skyAzimuthOrigin(zenith, zenith));
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
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
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
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
  ])("%s reads the terms, their densities and their phases (R08.T6.b)", (_name, source) => {
    expect(source).toMatch(/var<storage, read> terms : array<Term, MAX_TERMS>;/);
    expect(source).toMatch(/var densityTables : texture_2d_array<f32>;/);
    expect(source).toMatch(/var phaseTables : texture_2d_array<f32>;/);
    // Each source's direction is read from the view at each step and its angle formed there
    // (R08.T8.d), as the kernels formed them once a pixel before.
    expect(source).toMatch(/let source = view\.sources\[k\];/);
    expect(source).toMatch(/let angle = scatteringAngle\(ray, dir\);/);
    expect(source).not.toMatch(/\bsourceAngles\b/);
  });

  it("keeps the composite's code clear of the medium, which it does not declare", () => {
    const code = COMPOSITE_MATERIAL.fragmentWgsl.replaceAll(/\/\/[^\n]*/gu, "");
    expect(code).not.toMatch(/\b(medium|terms|densityTables|phaseTables)\b/);
  });
});

describe("the per-frame marches' steps (R08.T8.d)", () => {
  // An array held by value and indexed by a loop variable is copied whole into each invocation at
  // every call, and a function-scope array indexed by a variable lives in local memory: the
  // sources' directions and angles passed to each step so, with the slots' and terms' arrays,
  // made the ray march's step about 7 times R05's on the RTX 3080 (`decision-r08-t8-leans.md` §8),
  // R05.T14's `Medium` by value over again. No helper a step reaches takes or returns one, nor a
  // struct that holds one; pointers are allowed.
  it.each([
    [
      "the ray-march kernel",
      RAY_MARCH_KERNEL.reference,
      ["rayMarchStep", "rayMarchStepOf", "rayMarchWalkStepOf", "rayMarchGroundLightAt"],
    ],
    // The ray march by slot (R08.T8.e, lever 5): its step's sources and its bare ground's light, each
    // read from its slot's layer, return a matrix, no struct.
    [
      "the ray march by slot",
      RAY_MARCH_SLOTS_KERNEL.reference,
      [
        "rayMarchStep",
        "rayMarchStepOf",
        "rayMarchWalkStepOf",
        "rayMarchSourcesAt",
        "slicedSlotDepthsOf",
        "rayMarchGroundLightAt",
      ],
    ],
    [
      "the sky-view kernel",
      SKY_VIEW_KERNEL.reference,
      ["skyViewStep", "marchStep", "marchWalkStep"],
    ],
    [
      "the aerial-perspective kernel",
      AERIAL_PERSPECTIVE_KERNEL.reference,
      [
        "shellHeightAlong",
        "quadricAlong",
        "oblateNeedsLatitude",
        "datumPointOf",
        "gravityRatioAt",
        "sampleMediumAt",
        "sampleStartOf",
        "viewSourcesAt",
        "viewMarchStep",
      ],
    ],
    // The far limb's footprint (R08.T8.c), whose nodes are formed as each is marched or read, not
    // held (R08.T8.d).
    [
      "the ray-march kernel's footprint",
      RAY_MARCH_KERNEL.reference,
      ["farLimbFootprintOf", "farLimbNodeOf"],
    ],
    [
      "the composite's footprint",
      COMPOSITE_MATERIAL.fragmentWgsl,
      ["farLimbFootprintOf", "farLimbNodeOf"],
    ],
  ] as const)(
    "%s's per-step helpers take and return no array, nor a struct that holds one, by value",
    (_name, source, roots) => {
      // The roots are the step's own: each is declared.
      const declared = wgslFunctions(source);
      expect(roots.filter((root) => !declared.has(root))).toEqual([]);
      expect(arraysByValueFrom(source, roots)).toEqual([]);
    },
  );

  it("gives each light slot's share and each term's density a member of its own, and the march's slot 0", () => {
    const structs = wgslStructs(RAY_MARCH_KERNEL.reference);
    expect(structs.get("ViewShares")).toHaveLength(MAX_SKY_LIGHT_SLOTS);
    expect(structs.get("TermDensities")).toHaveLength(MAX_TERMS);
    // The luminance and slot 0's throughput and path, then the other slots' throughputs and path
    // depths, held by pointer; their columns are slot 0's (R08.T8.e, E3).
    expect(structs.get("ViewMarch")).toEqual([
      "vec3f",
      "vec3f",
      "AbsorberPath",
      "array<vec3f, MAX_SKY_LIGHT_SLOTS - 1u>",
      "array<vec3f, MAX_SKY_LIGHT_SLOTS - 1u>",
    ]);
  });

  // SwiftShader inlines every call, and with the march's steps emitted twice in one entry point its
  // GPU process segfaults (R08.T8.d): the centre ray and the footprint's nodes share one call.
  // Since R08.T11.d's tile cull the march has two entry points, each its own kernel with one call
  // (`decision-r08-t8-leans.md` §11, Ruling 4).
  it.each([
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
    ["the ray march at the centre ray", RAY_MARCH_CENTRE_KERNEL.reference],
    ["the ray march by slot at the centre ray", RAY_MARCH_SLOTS_CENTRE_KERNEL.reference],
    ["the specialised ray march", RAY_MARCH_SPECIAL_KERNEL.reference],
    ["the specialised ray march by slot", RAY_MARCH_SLOTS_SPECIAL_KERNEL.reference],
  ])("calls the march from one place in %s's entry point", (_name, source) => {
    const main = wgslFunctions(source).get("main");
    expect(main?.body.match(/\bmarchRay\(/gu)).toHaveLength(1);
  });

  // The centre ray's entry point holds none of the footprint across its march (R08.T11.d), and the
  // fill marches nothing.
  it.each([
    ["the ray march at the centre ray", RAY_MARCH_CENTRE_KERNEL.reference],
    ["the ray march by slot at the centre ray", RAY_MARCH_SLOTS_CENTRE_KERNEL.reference],
  ])("forms no footprint in %s's entry point", (_name, source) => {
    const reached = [...wgslReached(source, ["main"])];
    expect(
      reached.filter((name) => name.startsWith("farLimbFootprint") || name === "farLimbNodeOf"),
    ).toEqual([]);
  });

  it("makes the two entry points of each march from one source, the entry its own", () => {
    for (const [footprint, centre] of [
      [RAY_MARCH_KERNEL, RAY_MARCH_CENTRE_KERNEL],
      [RAY_MARCH_SLOTS_KERNEL, RAY_MARCH_SLOTS_CENTRE_KERNEL],
    ] as const) {
      expect(beforeEntryPoint(centre.reference)).toBe(beforeEntryPoint(footprint.reference));
    }
    expect(RAY_MARCH_FILL_KERNEL.reference).not.toMatch(/\bmarchRay\(/u);
  });

  // The footprint's state held across the march cost its shape 0.56–0.86 ms from orbit on the RTX
  // 3080 (R08.T8.e, QW2): it is formed again for each ray inside the loop, and gives that ray, so
  // that none of it is live across a march (E1′).
  it("forms the far limb's footprint once, inside the ray-march kernel's loop", () => {
    const body = wgslFunctions(RAY_MARCH_KERNEL.reference).get("main")?.body ?? "";
    const loop = body.search(/\bfor \(/u);
    const formed = [...body.matchAll(/\bfarLimbFootprintOf\(/gu)].map((call) => call.index);
    expect({ loop: loop >= 0, formed: formed.length }).toEqual({ loop: true, formed: 1 });
    expect(formed[0]).toBeGreaterThan(loop);
  });

  // Where w_L is 1 the mean reads none of the centre ray, so the loop starts at the first node
  // (R08.T8.e, E4).
  it("starts the ray-march kernel's loop at the first node where the footprint's blend is 1", () => {
    const body = wgslFunctions(RAY_MARCH_KERNEL.reference).get("main")?.body ?? "";
    expect(body).toMatch(/if \(blend >= 1\.0\) \{\s*j = 1u;\s*\}/u);
  });
});

describe("the transmittance read as optical depth (R08.T6.c)", () => {
  it("the multiple-scattering kernel reads its band's table only through transmittanceThroughLayer, exponentiated in f32", () => {
    const code = MULTI_SCATTERING_KERNEL.reference.replaceAll(/\/\/[^\n]*/gu, "");
    // The one bilinear read of the stored optical depth is transmittanceThroughLayer's own, which
    // adds the curves and exponentiates, at the build's layer (R08.T6.f); the forward lobes alone
    // read the stored depth itself, as their x (R08.T7), in this form and no other.
    const lobeReads = code.replaceAll(
      "exp(min(tableAtUnitLayer(transmittance, layer, unit).rgb * share,",
      "",
    );
    expect([
      /tableAtUnit(Layer)?\(transmittance,/u.test(lobeReads),
      code.match(/tableAtUnitLayer\(depthTable, layer, unit\)/gu)?.length,
      /return exp\(-\(stored\.rgb \+ absorberDepth\(/u.test(code),
      /transmittanceThroughLayer\(\s*transmittance,\s*absorberColumns,\s*absorberPressureColumns,\s*lastAbsorberPressureColumn,\s*absorberCurves,\s*layer,/u.test(
        code,
      ),
    ]).toEqual([false, 1, true, true]);
  });

  it.each([
    ["the sky-view kernel", SKY_VIEW_KERNEL.reference],
    ["the aerial-perspective kernel", AERIAL_PERSPECTIVE_KERNEL.reference],
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
    ["the composite", COMPOSITE_MATERIAL.fragmentWgsl],
  ])(
    "%s reads the slices only through the sliced lookup, exponentiated in f32 (R08.T6.f)",
    (_n, source) => {
      const code = source.replaceAll(/\/\/[^\n]*/gu, "");
      // The one bilinear read of a slice's stored optical depth is sliceStoredAt's own, called by
      // slicedStoredOf with the module's four tables; its sum, over s, takes the curves and is
      // exponentiated; the local scale height alone reads slice 0's vertical depth itself. The
      // other read of the form is `common.wgsl`'s transmittanceThroughLayer, which none calls.
      expect([
        /\btableAtUnit\(\s*(transmittance|depthTable|absorberColumns|columnTable)\b/u.test(code),
        code.match(/tableAtUnitLayer\(depthTable, layer, unit\)/gu)?.length,
        code.match(/\btransmittanceThroughLayer\(/gu)?.length,
        code.match(/tableAtUnitLayer\(depthTable, 0u, unit\)\.g/gu)?.length,
        /slicedStoredOf\(\s*ray, body, oblateBody, transmittance, absorberColumns, absorberPressureColumns,\s*lastAbsorberPressureColumn, count\s*\)/u.test(
          code,
        ),
        /exp\(-\(stored\.depth \+ absorbed\)\)/u.test(code),
      ]).toEqual([false, 2, 1, 1, true, true]);
    },
  );

  it("writes the absorbers' pressure-weighted columns beside their columns (R08.T4.d)", () => {
    const code = TRANSMITTANCE_KERNEL.reference.replaceAll(/\/\/[^\n]*/gu, "");
    expect([
      /pressureColumns = addColumns\(pressureColumns, local\.pressureColumns, dt\);/u.test(code),
      /textureStore\(absorberPressureColumnsOut, id\.xy, layer, min\(firstFour, vec4f\(HALF_MAX\)\)\);/u.test(
        code,
      ),
      /textureStore\(\s*lastAbsorberPressureColumnOut, id\.xy, layer,/u.test(code),
    ]).toEqual([true, true, true]);
  });

  it("writes the optical depth and the absorber columns, never e^(−τ), in the transmittance kernel", () => {
    const code = TRANSMITTANCE_KERNEL.reference.replaceAll(/\/\/[^\n]*/gu, "");
    expect(code).toMatch(
      /let stored = min\(vec4f\(ray\.depth, ray\.columns\.first\), vec4f\(HALF_MAX\)\);\s*textureStore\(transmittanceOut, id\.xy, layer, stored\);/u,
    );
    expect(code).toMatch(
      /textureStore\(absorberColumnsOut, id\.xy, layer, min\(ray\.columns\.more, vec4f\(HALF_MAX\)\)\);/u,
    );
    expect(code).toMatch(/const HALF_MAX : f32 = 65504\.0;/u);
    expect(code).not.toMatch(/exp\(-opticalDepth/u);
  });

  it.each([
    ["the multiple-scattering kernel", MULTI_SCATTERING_KERNEL.reference],
    ["the sky-view kernel", SKY_VIEW_KERNEL.reference],
    ["the aerial-perspective kernel", AERIAL_PERSPECTIVE_KERNEL.reference],
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
    ["the composite", COMPOSITE_MATERIAL.fragmentWgsl],
  ])(
    "%s declares the absorber-column table, its pressure tables and the lights' curves",
    (_n, source) => {
      // The transmittance tables are 2D arrays of one layer a κ slice, or a band's (R08.T6.f).
      expect([
        /var absorberColumns : texture_2d_array<f32>;/u.test(source),
        /var absorberCurves : texture_2d<f32>;/u.test(source),
        /var absorberPressureColumns : texture_2d_array<f32>;/u.test(source),
        /var lastAbsorberPressureColumn : texture_2d_array<f32>;/u.test(source),
      ]).toEqual([true, true, true, true]);
    },
  );

  const absorbing = fourAbsorberMedium();

  it("binds the absorber columns and the lights' curves to every per-frame kernel", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(
      engine.dispatched.map(({ bindings }) =>
        [...(bindings.uniforms["medium"] ?? [])].slice(8, 12),
      ),
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
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const { absorberColumns, absorberCurves } = atmosphere.tables;
    expect(draw.textures).toMatchObject({ absorberColumns, absorberCurves });
    expect([...(draw.uniforms["shell"] ?? [])]).toEqual([100_000, 4, 0, 0]);
  });

  it("binds the pressure-column tables to the composite (R08.T4.d)", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = lineAbsorberMedium();
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const { absorberPressureColumns, lastAbsorberPressureColumn } = atmosphere.tables;
    expect(draw.textures).toMatchObject({ absorberPressureColumns, lastAbsorberPressureColumn });
  });

  it("gives the composite the medium's line slots, slot 1's bit, in shell.z (R08.T4.d)", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = lineAbsorberMedium();
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
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
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
    ["the composite", COMPOSITE_MATERIAL.fragmentWgsl],
  ])("%s calls no builtin trigonometry, R05 radius form or sebh remap", (_name, source) => {
    expect(forbiddenIn(source)).toEqual([]);
  });

  it("gives the view's members in one order to the kernels and the composite, the sources last (R08.T7)", () => {
    const view = vec4Members(viewWgsl, "AtmosphereView");
    expect(view.slice(-4)).toEqual(["exact", "curvature", "marchGround", "marchSky"]);
    expect(vec4Members(viewWgsl, "SkySource")).toEqual(["direction", "weight", "disc"]);
    expect(viewWgsl).toMatch(/sources : array<SkySource, MAX_SKY_SOURCES>,\s*\}/u);
    const sources = Array.from({ length: MAX_SKY_SOURCES }, (_, i) =>
      ["sourceDirection", "sourceWeight", "sourceDisc"].map((name) => name + String(i)),
    ).flat();
    expect(COMPOSITE_MATERIAL.uniforms.map((u) => u.name)).toEqual([
      ...view,
      ...sources,
      "shell",
      "marchArea",
    ]);
    // The composite's Draw: the offset (a vec3f), then the view's members in order, then shell and
    // the march's area (R08.T11.d).
    expect(vec4Members(compositeWgsl, "Draw")).toEqual([...view, ...sources, "shell", "marchArea"]);
    expect(viewWgsl).toContain(`const MAX_SKY_SOURCES : u32 = ${String(MAX_SKY_SOURCES)}u;`);
    expect(viewWgsl).toContain(
      `const MAX_SKY_LIGHT_SLOTS : u32 = ${String(MAX_SKY_LIGHT_SLOTS)}u;`,
    );
  });

  it("forms the ray march's quadric constants in f64 from the camera's position", () => {
    const sphere = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000 };
    const position = { x: 0, y: 0, z: 6_371_002 };
    const inputs = atmosphereInputs({ positionM: position }, sphere);
    const exact = viewExactOf(position, sphere, R05_EARTH_REFERENCE, inputs);
    // q_g = h (2a + h) and q_t = (h − H)(2a + h + H) on a sphere, 2 m up.
    expect(exact.groundQuadricM2 / (2 * (2 * 6_371_000 + 2)) - 1).toBeCloseTo(0, 9);
    expect(exact.topQuadricM2 / ((2 - 100_000) * (2 * 6_371_000 + 100_002)) - 1).toBeCloseTo(0, 12);
    // The camera's local scale height (R08.T6.f): the column above it over its extinction.
    expect(exact.localScaleHeightM).toBe(localScaleHeightM(R05_EARTH_REFERENCE, inputs.heightM));
  });

  it("takes the local scale height as one scale height of an exponential, held to the room left", () => {
    const medium: AtmosphereMedium = {
      name: "one exponential",
      topHeightM: 100_000,
      groundAlbedo: [0, 0, 0],
      terms: [
        {
          name: "gas",
          density: { kind: "exponential", scaleHeightM: 8_000 },
          scattering: [1e-5, 2e-5, 3e-5],
          absorption: [0, 0, 0],
          phase: { kind: "none" },
        },
      ],
    };
    // H (1 − e^(−(top − h) ÷ H)) in green: one H to 10⁻⁵ from the ground.
    expect(localScaleHeightM(medium, 0) / 8_000 - 1).toBeCloseTo(-Math.exp(-12.5), 9);
    // At the top, the fit's floor (R08.T6.f), oblate.wgsl's too.
    expect(localScaleHeightM(medium, 99_999.5)).toBe(FIT_SCALE_FLOOR_M);
    expect(oblateWgsl).toContain(
      `const OBLATE_SCALE_FLOOR_M : f32 = ${FIT_SCALE_FLOOR_M.toFixed(1)};`,
    );
    expect(localScaleHeightM({ ...medium, terms: [] }, 50_000)).toBe(50_000);
  });

  it("puts the ray march's top through h* = H at the equator and the poles (R08.T6.f)", () => {
    const gravity = gravityOfSpheroid(SATURN_SPHEROID);
    const at = { x: SATURN_SPHEROID.equatorialRadiusM + 2_000, y: 0, z: 0 };
    const inputs = atmosphereInputs({ positionM: at }, SATURN_SPHEROID, gravity);
    const medium = { ...R05_EARTH_REFERENCE, topHeightM: 800_000 };
    const exact = viewExactOf(at, SATURN_SPHEROID, medium, inputs, gravity);
    const a = SATURN_SPHEROID.equatorialRadiusM + 800_000 / gravityRatio(gravity, 0);
    const c = SATURN_SPHEROID.polarRadiusM + 800_000 / gravityRatio(gravity, Math.PI / 2);
    expect(exact.topQuadricM2).toBe(at.x ** 2 - a * a);
    // s rises from about 0.87 at the equator to 1.15 at the pole: the top stands higher over the
    // equator than H and lower over the pole.
    expect(a).toBeGreaterThan(SATURN_SPHEROID.equatorialRadiusM + 800_000);
    expect(c).toBeLessThan(SATURN_SPHEROID.polarRadiusM + 800_000);
    expect(inputs.gravityRatio).toBe(gravityRatio(gravity, 0));
  });

  it("forms the spheroid's constants with its own z scale, the top shell's with (a + H) ÷ (c + H)", () => {
    const at = pointAt(45, 3);
    const exact = viewExactOf(
      at,
      WGS84,
      R05_EARTH_REFERENCE,
      atmosphereInputs({ positionM: at }, WGS84),
    );
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
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const inputs = atmosphereInputs(CAMERA, WGS84);
    const exact = viewExactOf(CAMERA.positionM, WGS84, R05_EARTH_REFERENCE, inputs);
    const expected = [exact.groundQuadricM2, exact.topQuadricM2, exact.localScaleHeightM, 0].map(
      Math.fround,
    );
    expect([...(draw.uniforms["exact"] ?? [])]).toEqual(expected);
    expect([...(draw.uniforms["shell"] ?? [])]).toEqual([100_000, 0, 0, 0]);
    // The kernels' view block: the members in order, `exact` the eleventh, `curvature` the twelfth
    // (R08.T6.f), `marchGround` and `marchSky` the thirteenth and fourteenth (R08.T8.e), the drawn
    // surface's band in `marchSky.z` (R08.T11.b), the sources after them.
    const block = engine.dispatched.find((d) => d.kernel === "atmosphere sky view")?.bindings
      .uniforms["view"];
    expect(Array.from(block ?? []).slice(40, 44)).toEqual(expected);
    expect(Array.from(block ?? []).slice(44, 48)).toEqual([...(draw.uniforms["curvature"] ?? [])]);
    expect(Array.from(block ?? []).slice(48, 52)).toEqual([20, 4, 2, 8]);
    expect(Array.from(block ?? []).slice(52, 56)).toEqual([28, Math.fround(0.05), 25_000, 0]);
    expect(block).toHaveLength((14 + 3 * 6) * 4);
  });

  it("gives the march no ground or sky rule on low, so every ray takes its 16 steps (R08.T8.e)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.low, WGS84);
    const draw = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect([...(draw.uniforms["marchGround"] ?? [])]).toEqual([0, 0, 0, 0]);
    expect([...(draw.uniforms["marchSky"] ?? [])]).toEqual([0, 0, 0, 0]);
  });

  it.each(["high", "low"] as const)(
    "puts the march's share of the frame at the camera's h* in the view's exact.w on %s (R08.T8)",
    async (setting) => {
      const engine = await countingRenderEngine();
      const sizes = TABLE_SIZES[setting];
      const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, sizes, WGS84);
      const top = R05_EARTH_REFERENCE.topHeightM;
      const { lowerShareOfTop: lower, upperShareOfTop: upper } = sizes.marchBand;
      // Below the band, at its middle, above it, and from orbit; WGS 84 with no gravity, s ≡ 1.
      const shares = [
        2,
        lower * top - 1_000,
        ((lower + upper) / 2) * top,
        upper * top + 1_000,
        400_000,
      ].map((heightM) => {
        const camera: AtmosphereCamera = { ...CAMERA, positionM: pointAt(0, heightM) };
        engine.dispatched.length = 0;
        const draw = atmosphere.drawFrame(camera, [NOON], SCENE);
        const inputs = atmosphereInputs(camera, WGS84);
        const expected = Math.fround(marchWeightOf(inputs.heightM, top, sizes.marchBand));
        // The composite's and the ray march's view block alike, `exact` its eleventh member.
        const block = engine.dispatched.find((d) => d.kernel === "atmosphere ray march")?.bindings
          .uniforms["view"];
        expect([draw.uniforms["exact"]?.[3], block?.[43]]).toEqual([expected, expected]);
        return expected;
      });
      expect(shares.map((w) => Math.round(w * 1e6) / 1e6)).toEqual([0, 0, 0.5, 1, 1]);
    },
  );

  it("takes the march's share at h* = s h on a body with gravity (R08.T8)", async () => {
    const engine = await countingRenderEngine();
    const gravity = gravityOfSpheroid(SATURN_SPHEROID);
    const atmosphere = new HillaireAtmosphere(
      engine,
      R05_EARTH_REFERENCE,
      TABLE_SIZES.high,
      SATURN_SPHEROID,
      NO_ABSORBER_CURVES,
      gravity,
    );
    const s = gravityRatio(gravity, 0);
    const top = R05_EARTH_REFERENCE.topHeightM;
    const band = TABLE_SIZES.high.marchBand;
    // At the equator, h* = 40 km, the middle of high's band, stands at h = 40 km ÷ s, about 46 km.
    const camera: AtmosphereCamera = {
      ...CAMERA,
      positionM: { x: SATURN_SPHEROID.equatorialRadiusM + 40_000 / s, y: 0, z: 0 },
    };
    const draw = atmosphere.drawFrame(camera, [NOON], SCENE);
    const inputs = atmosphereInputs(camera, SATURN_SPHEROID, gravity);
    expect(inputs.gravityRatio).toBe(s);
    expect(Math.abs(s - 1)).toBeGreaterThan(0.05);
    expect(draw.uniforms["exact"]?.[3]).toBe(
      Math.fround(marchWeightOf(inputs.heightM * s, top, band)),
    );
    expect(draw.uniforms["exact"]?.[3]).toBeCloseTo(0.5, 6);
    expect(marchWeightOf(inputs.heightM, top, band)).toBeGreaterThan(0.6);
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
      RAY_MARCH_SLOTS_KERNEL.reference,
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
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const shipped = engine.dispatched.slice(-3);
    engine.dispatched.length = 0;
    const arm = atmosphere.drawKernelArm(CAMERA, [NOON], SCENE);
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
    atmosphere.drawKernelArm(CAMERA, [NOON], SCENE);
    const textures = engine.counts.textures;
    const kernels = engine.counts.kernels;
    atmosphere.drawKernelArm(CAMERA, [DUSK], SCENE);
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
    const first = atmosphere.drawKernelArm(CAMERA, [NOON], SCENE).rayMarch;
    const wider = { ...CAMERA, viewport: { widthPx: 96, heightPx: 32 } };
    const second = atmosphere.drawKernelArm(wider, [NOON], SCENE).rayMarch;
    expect(second).not.toBe(first);
    // And the tile cull's kinds, grown with the target (R08.T11.d).
    expect(destroyed).toEqual([
      "atmosphere ray march (kernel arm)",
      "atmosphere ray march's tiles",
    ]);
    expect(engine.textureSpecs.at(-1)?.size).toEqual([96, 32]);
  });

  it("makes its kernels and targets anew after a device restore", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const before = atmosphere.drawKernelArm(CAMERA, [NOON], SCENE);
    engine.restore();
    const after = atmosphere.drawKernelArm(CAMERA, [NOON], SCENE);
    expect(after.skyView).not.toBe(before.skyView);
    expect(after.rayMarch).not.toBe(before.rayMarch);
  });

  it("releases its three targets, with its frame's and its own tables, when the atmosphere is disposed (R08.T10.b)", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.drawKernelArm(CAMERA, [NOON], SCENE);
    const destroyed: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    atmosphere.dispose();
    expect(destroyed).toEqual(
      expect.arrayContaining([
        "atmosphere aerial perspective (kernel arm)",
        "atmosphere ray march (kernel arm)",
        "atmosphere sky view (kernel arm)",
        "atmosphere sky view",
        "atmosphere aerial perspective",
        "atmosphere transmittance",
        "atmosphere terms",
        // The tile cull's kinds, which the kernel arm's frame made (R08.T11.d).
        "atmosphere ray march's tiles",
      ]),
    );
  });
});

describe("HillaireAtmosphere over held tables (R08.T10.b)", () => {
  it("draws with the tables it is given, on their figure, and never disposes them", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const atmosphere = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, null);
    const composite = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(composite.textures["transmittance"]).toBe(tables.transmittance);
    expect(atmosphere.tables).toBe(tables);
    const destroyed: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    atmosphere.dispose();
    expect(destroyed).toContain("atmosphere sky view");
    expect(destroyed).not.toContain("atmosphere transmittance");
    tables.dispose();
    expect(destroyed).toContain("atmosphere transmittance");
  });

  it("follows another set of held tables, and refuses a medium of its own", async () => {
    const engine = await countingRenderEngine();
    const radiusM = tableRadiusM(WGS84);
    const first = new AtmosphereTables(engine, R05_EARTH_REFERENCE, radiusM);
    const second = new AtmosphereTables(engine, R05_EARTH_REFERENCE, radiusM);
    const atmosphere = HillaireAtmosphere.over(engine, first, TABLE_SIZES.high, null);
    atmosphere.setTables(second);
    const composite = atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(composite.textures["transmittance"]).toBe(second.transmittance);
    expect(() => {
      atmosphere.setMedium(R05_EARTH_REFERENCE);
    }).toThrow(/held tables/);
    atmosphere.dispose();
  });

  it("refuses a frame after a restore of the device its shared kernels were made on", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const kernels = await createAtmosphereFrameKernels(engine);
    const atmosphere = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    engine.restore();
    expect(() => atmosphere.drawFrame(CAMERA, [NOON], SCENE)).toThrow(/made again by its owner/);
    atmosphere.dispose();
  });
});

describe("AtmosphereTables' release (R08.T10.b)", () => {
  it("releases its buffer and textures when disposed, once, and refuses a medium after", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const destroyed: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    tables.dispose();
    tables.dispose();
    expect(destroyed.filter((name) => name === "atmosphere transmittance")).toHaveLength(1);
    expect(destroyed).toContain("atmosphere terms");
    expect(() => tables.setMedium(R05_EARTH_REFERENCE, tableRadiusM(WGS84))).toThrow(/disposed/);
  });
});

/** The bindings `wgsl` declares that neither its entry point nor any function it reaches names. */
function unreachedBindings(wgsl: string): string[] {
  const functions = wgslFunctions(wgsl);
  const bodies = [...wgslReached(wgsl, ["main"])].map((name) => functions.get(name)?.body ?? "");
  return [...kernelBindings(wgsl).keys()].filter(
    (name) => !bodies.some((body) => new RegExp(`\\b${name}\\b`, "u").test(body)),
  );
}

describe("the ray march by slot (R08.T8.e, lever 5; decision-r08-t8-leans.md §10 and §13)", () => {
  const slots = RAY_MARCH_SLOTS_KERNEL.reference;
  const functions = wgslFunctions(slots);

  it("is the ray-march kernel's own march, the hook appended after it apart", () => {
    expect(slots.replace(rayMarchSlotsWgsl, rayMarchCurvesWgsl)).toBe(RAY_MARCH_KERNEL.reference);
  });

  // SwiftShader inlines every call, and its GPU process crashed on more copies of a read's code
  // (R08.T8.d, R08.T8.e): one sliced ray and one slot read in the step's source loop.
  it("reads each source's light at a step from one sliced ray and one slot read, in its source loop", () => {
    const body = functions.get("rayMarchSourcesAt")?.body ?? "";
    const loop = body.search(/\bfor \(/u);
    const sliced = [...body.matchAll(/\bslicedRayOf\(/gu)].map((call) => call.index);
    const read = [...body.matchAll(/\bslicedSlotDepthsOf\(/gu)].map((call) => call.index);
    expect({
      sliced: sliced.length,
      read: read.length,
      inLoop: loop >= 0 && [...sliced, ...read].every((at) => at > loop),
      curves: /\bsourceLightAt\(/u.test(body),
    }).toEqual({ sliced: 1, read: 1, inLoop: true, curves: false });
  });

  it("returns the slot read as a matrix of the slot's depth and the extinction, no struct", () => {
    expect(functions.get("slicedSlotDepthsOf")?.returns).toBe("mat2x3f");
  });

  it("keeps the forward lobes on the shared table's stored depth, the read's second column", () => {
    const body = functions.get("rayMarchSourcesAt")?.body ?? "";
    expect([
      /slicedSlotDepthsOf\(sliced, body, slot, lobes\)/u.test(body),
      /SourceLight\(select\(1\.0, 0\.0, sliced\.shadowed\), exp\(-read\[0\]\), read\[1\]\)/u.test(
        body,
      ),
      /lobeOrdersOf\(local, angle, light\.depth\)/u.test(body),
    ]).toEqual([true, true, true]);
  });

  it("lights the bare ground by the slot read too (§13, point 4)", () => {
    const body = functions.get("rayMarchGroundLightAt")?.body ?? "";
    expect([/\bslicedSlotDepthsOf\(/u.test(body), /\bsourceLightAt\(/u.test(body)]).toEqual([
      true,
      false,
    ]);
  });

  it.each([
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
  ])("keeps the curves' read for the ray's count in %s (§13, point 4)", (_name, source) => {
    expect(wgslFunctions(source).get("rayMarchDepthAlong")?.body).toMatch(/\bsourceLightAt\(/u);
  });

  it("declares the slot layers in the ray march by slot alone", () => {
    expect(
      [
        SKY_VIEW_KERNEL.reference,
        AERIAL_PERSPECTIVE_KERNEL.reference,
        RAY_MARCH_KERNEL.reference,
        RAY_MARCH_SLOTS_KERNEL.reference,
        COMPOSITE_MATERIAL.fragmentWgsl,
      ].map((source) => /var slotDepths : texture_2d_array<f32>;/u.test(source)),
    ).toEqual([false, false, false, true, false]);
  });

  // The engine's compute layouts are "auto": a declared binding the entry point never reaches drops
  // out of the layout, and the bind group, which takes every declared binding, fails.
  it.each([
    ["the ray-march kernel", RAY_MARCH_KERNEL.reference],
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL.reference],
    ["the specialised ray march", RAY_MARCH_SPECIAL_KERNEL.reference],
    ["the specialised ray march by slot", RAY_MARCH_SLOTS_SPECIAL_KERNEL.reference],
  ])("reaches every binding %s declares from its entry point", (_name, source) => {
    expect(unreachedBindings(source)).toEqual([]);
  });

  it("is catalogued beside the ray-march kernel, with the slot-depth kernel", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(
      expect.arrayContaining([
        "atmosphere ray march",
        "atmosphere ray march by slot",
        RAY_MARCH_SPECIAL_KERNEL.name,
        RAY_MARCH_SLOTS_SPECIAL_KERNEL.name,
        "atmosphere slot depths",
      ]),
    );
  });
});

/** `atmosphere` with the tile cull asked for (R08.T11.d's `tiles` cover, off for a view). */
function culled(atmosphere: HillaireAtmosphere): HillaireAtmosphere {
  atmosphere.setMarchCover("tiles");
  return atmosphere;
}

/** The ray march's dispatches among `engine`'s (R08.T11.d): its entry points' and its fill's. */
function marchesOf(
  engine: Awaited<ReturnType<typeof countingRenderEngine>>,
): ReadonlyArray<(typeof engine.dispatched)[number]> {
  return engine.dispatched.filter((d) => d.kernel.startsWith("atmosphere ray march"));
}

describe("HillaireAtmosphere's march over the air's area, and the tile cull (R08.T11.d)", () => {
  /** From 10⁹ m over the equator, looking at the centre: the Earth's air a few texels across. */
  const far: AtmosphereCamera = {
    positionM: { x: 1e9, y: 0, z: 0 },
    orientation: lookAlong({ x: -1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 320, heightPx: 180 },
  };
  /** R05's Earth's top shell on WGS 84, s ≡ 1. */
  const shell = {
    equatorialM: WGS84.equatorialRadiusM + 100_000,
    polarM: WGS84.polarRadiusM + 100_000,
  };
  const atmosphereOf = async (): Promise<{
    engine: Awaited<ReturnType<typeof countingRenderEngine>>;
    atmosphere: HillaireAtmosphere;
  }> => {
    const engine = await countingRenderEngine();
    return {
      engine,
      atmosphere: culled(
        new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84),
      ),
    };
  };
  it("dispatches both entry points over the area's tiles alone, the centre ray's first, and gives the composite its texels", async () => {
    const { engine, atmosphere } = await atmosphereOf();
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(far, [NOON], SCENE);
    const area = marchAreaOf(far, shell, 1);
    const texels = marchAreaTexels(area);
    expect(area.whole).toBe(false);
    const marches = marchesOf(engine);
    const tiles = [area.tilesAcross, area.tilesDown, 1];
    expect(marches.map((d) => [d.kernel, d.workgroups])).toEqual([
      ["atmosphere ray march at the centre ray", tiles],
      ["atmosphere ray march", tiles],
    ]);
    // Its first tile, its tiles, the cull on, the widening, and the target's size.
    expect([...(marches[0]?.bindings.uniforms["marchArea"] ?? [])]).toEqual([
      area.firstTileX,
      area.firstTileY,
      area.tilesAcross,
      area.tilesDown,
      1,
      Math.fround(area.widenM),
      320,
      180,
    ]);
    // One buffer of the tiles' kinds, which the centre ray's entry point writes and the
    // footprint's reads.
    const kinds = marches[0]?.bindings.buffers["marchTiles"];
    expect(kinds).toBeDefined();
    expect(marches[1]?.bindings.buffers["marchTiles"]).toBe(kinds);
    expect([...(draw.uniforms["marchArea"] ?? [])]).toEqual([
      texels.x0,
      texels.y0,
      texels.x1,
      texels.y1,
    ]);
  });

  it("marches every tile through the footprint's entry point alone on the reference path", async () => {
    const { engine, atmosphere } = await atmosphereOf();
    atmosphere.setMarchCover("whole frame");
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(far, [NOON], SCENE);
    const marches = marchesOf(engine);
    expect(marches.map((d) => [d.kernel, d.workgroups])).toEqual([
      ["atmosphere ray march", [40, 23, 1]],
    ]);
    // The cull off: the entry point reads no tile's kind.
    expect(marches[0]?.bindings.uniforms["marchArea"]?.[4]).toBe(0);
    expect([...(draw.uniforms["marchArea"] ?? [])]).toEqual([0, 0, 320, 180]);
    // The whole target is the march's: a reader's ask fills nothing.
    engine.dispatched.length = 0;
    expect(atmosphere.frameTables.rayMarch?.name).toBe("atmosphere ray march");
    expect(marchesOf(engine)).toEqual([]);
  });

  it("fills the rest of the target for a reader of the whole, once a frame, and never for the frame", async () => {
    const { engine, atmosphere } = await atmosphereOf();
    atmosphere.drawFrame(far, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).not.toContain("atmosphere ray march's fill");
    engine.dispatched.length = 0;
    const { rayMarch } = atmosphere.frameTables;
    expect(atmosphere.frameTables.rayMarch).toBe(rayMarch);
    const fills = marchesOf(engine);
    expect(fills.map((d) => [d.kernel, d.workgroups])).toEqual([
      ["atmosphere ray march's fill", [40, 23, 1]],
    ]);
    expect(fills[0]?.bindings.storage["rayMarchOut"]?.texture).toBe(rayMarch);
  });

  it("bounds the area by the top shell through h* = H at the equator and the poles on a body whose gravity varies with latitude", async () => {
    const engine = await countingRenderEngine();
    const gravity = gravityOfSpheroid(SATURN_SPHEROID);
    const atmosphere = culled(
      new HillaireAtmosphere(
        engine,
        R05_EARTH_REFERENCE,
        TABLE_SIZES.high,
        SATURN_SPHEROID,
        NO_ABSORBER_CURVES,
        gravity,
      ),
    );
    // From 4 × 10⁸ m over the equator, looking at the centre: the giant's air a few tiles across.
    const giant: AtmosphereCamera = { ...far, positionM: { x: 4e8, y: 0, z: 0 } };
    engine.dispatched.length = 0;
    atmosphere.drawFrame(giant, [NOON], SCENE);
    const top = topShellOf(SATURN_SPHEROID, R05_EARTH_REFERENCE.topHeightM, gravity);
    expect([top.equatorialM, top.polarM]).toEqual([
      SATURN_SPHEROID.equatorialRadiusM + 100_000 / gravityRatio(gravity, 0),
      SATURN_SPHEROID.polarRadiusM + 100_000 / gravityRatio(gravity, Math.PI / 2),
    ]);
    const area = marchAreaOf(giant, top, 1);
    expect(area.whole).toBe(false);
    const footprint = marchesOf(engine).find((d) => d.kernel === "atmosphere ray march");
    expect([...(footprint?.bindings.uniforms["marchArea"] ?? [])].slice(0, 4)).toEqual([
      area.firstTileX,
      area.firstTileY,
      area.tilesAcross,
      area.tilesDown,
    ]);
    // The march's own top: the quadric viewExactOf forms is the same shell's.
    const exact = viewExactOf(
      giant.positionM,
      SATURN_SPHEROID,
      R05_EARTH_REFERENCE,
      atmosphereInputs(giant, SATURN_SPHEROID, gravity),
      gravity,
    );
    const z = (giant.positionM.z * top.equatorialM) / top.polarM;
    expect(exact.topQuadricM2).toBe(
      giant.positionM.x ** 2 + giant.positionM.y ** 2 + z * z - top.equatorialM ** 2,
    );
  });

  it("makes no shared-read march for a body whose tables hold the slot layers", async () => {
    const engine = await countingRenderEngine();
    const made: string[] = [];
    const create = engine.createCompute.bind(engine);
    engine.createCompute = (pair) => {
      made.push(pair.name);
      return create(pair);
    };
    const { medium, curves } = fourAbsorberMedium();
    const atmosphere = culled(
      new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves),
    );
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    atmosphere.drawFrame(CAMERA, [DUSK], SCENE);
    expect(made).toContain("atmosphere ray march by slot");
    expect(made).toContain("atmosphere ray march by slot at the centre ray");
    expect(made).not.toContain(RAY_MARCH_KERNEL.name);
    expect(made).not.toContain(RAY_MARCH_CENTRE_KERNEL.name);
  });

  it("marches the area over the device's shared kernels from its making, and over its own the whole frame until asked", async () => {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    engine.dispatched.length = 0;
    const drawn = view.drawFrame(far, [NOON], SCENE);
    const area = marchAreaOf(far, shell, 1);
    const marches = marchesOf(engine);
    // One kernel over the area's tiles, the tile cull off: no tile's kind is read. R05's Earth on
    // WGS 84, of one slice and one band and no layer edge, takes the specialised march.
    expect(marches.map((d) => [d.kernel, d.workgroups])).toEqual([
      [RAY_MARCH_SPECIAL_KERNEL.name, [area.tilesAcross, area.tilesDown, 1]],
    ]);
    expect(marches[0]?.bindings.uniforms["marchArea"]?.[4]).toBe(0);
    const texels = marchAreaTexels(area);
    expect([...(drawn.uniforms["marchArea"] ?? [])]).toEqual([
      texels.x0,
      texels.y0,
      texels.x1,
      texels.y1,
    ]);
    const own = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    const before = engine.counts.kernels;
    engine.dispatched.length = 0;
    own.drawFrame(far, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => [d.kernel, d.workgroups])).toEqual([
      ["atmosphere ray march", [40, 23, 1]],
    ]);
    // Its specialised march is made at its first frame over the area, its centre ray's kernel at
    // its first tile-culled frame, neither before.
    expect(engine.counts.kernels - before).toBe(1);
    own.setMarchCover("area");
    engine.dispatched.length = 0;
    own.drawFrame(far, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual([RAY_MARCH_SPECIAL_KERNEL.name]);
    expect(engine.counts.kernels - before).toBe(2);
    own.setMarchCover("tiles");
    engine.dispatched.length = 0;
    own.drawFrame(far, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual([
      "atmosphere ray march at the centre ray",
      "atmosphere ray march",
    ]);
    expect(engine.counts.kernels - before).toBe(3);
  });

  it("makes up front only the reads asked of the device's shared kernels, the other at its first frame", async () => {
    const engine = await countingRenderEngine();
    const made: string[] = [];
    const create = engine.createComputeAsync.bind(engine);
    engine.createComputeAsync = (pair) => {
      made.push(pair.name);
      return create(pair);
    };
    const kernels = await createAtmosphereFrameKernels(engine, { reads: "by slot" });
    // The footprint's general and specialised marches by slot, and the tile cull's centre ray's
    // kernels only where asked.
    expect(made).toEqual(
      expect.arrayContaining([RAY_MARCH_SLOTS_KERNEL.name, RAY_MARCH_SLOTS_SPECIAL_KERNEL.name]),
    );
    for (const name of [
      RAY_MARCH_SLOTS_CENTRE_KERNEL.name,
      RAY_MARCH_CENTRE_KERNEL.name,
      RAY_MARCH_KERNEL.name,
      RAY_MARCH_SPECIAL_KERNEL.name,
    ]) {
      expect(made).not.toContain(name);
    }
    const before = engine.counts.kernels;
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    engine.dispatched.length = 0;
    view.drawFrame(far, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual([RAY_MARCH_SPECIAL_KERNEL.name]);
    // The tables' own build kernels beside the shared read's two made at the frame.
    expect(engine.counts.kernels - before).toBeGreaterThanOrEqual(2);
    made.length = 0;
    await createAtmosphereFrameKernels(engine, { reads: "by slot", tiles: true });
    expect(made).toContain("atmosphere ray march by slot at the centre ray");
  });

  it("marches nothing where the air lies behind the camera, and the composite reads none of it", async () => {
    const { engine, atmosphere } = await atmosphereOf();
    const away = { ...far, orientation: lookAlong({ x: 1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }) };
    engine.dispatched.length = 0;
    const draw = atmosphere.drawFrame(away, [NOON], SCENE);
    expect(marchesOf(engine)).toEqual([]);
    expect([...(draw.uniforms["marchArea"] ?? [])]).toEqual([0, 0, 0, 0]);
  });
});

/** The lines of `special` that differ from `general`'s, each as [general's, special's]. */
function linesDiffering(
  general: string,
  special: string,
): ReadonlyArray<readonly [string, string]> {
  const ours = special.split("\n");
  return general.split("\n").flatMap((line, i) => {
    const theirs = ours[i] ?? "";
    return line === theirs ? [] : [[line.trim(), theirs.trim()] as const];
  });
}

describe("the ray march specialised for one band, edge-free (R08.T11.d)", () => {
  /** From 10⁹ m over the equator, looking at the centre: the Earth's air a few texels across. */
  const distant: AtmosphereCamera = {
    positionM: { x: 1e9, y: 0, z: 0 },
    orientation: lookAlong({ x: -1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 320, heightPx: 180 },
  };

  it.each([
    ["the ray march", RAY_MARCH_KERNEL, RAY_MARCH_SPECIAL_KERNEL],
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL, RAY_MARCH_SLOTS_SPECIAL_KERNEL],
  ] as const)(
    "holds the slices' and bands' counts at 1 and the edges' at 0 in %s, and nothing else",
    (_name, general, special) => {
      expect(special.name).toBe(`${general.name} for one band, edge-free`);
      expect(special.reference.split("\n")).toHaveLength(general.reference.split("\n").length);
      expect(linesDiffering(general.reference, special.reference)).toEqual([
        ["return clamp(u32(body.counts.x), 1u, MAX_SLICES);", "return 1u;"],
        ["return clamp(u32(body.counts.y), 1u, MAX_BANDS);", "return 1u;"],
        [
          "let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);",
          "let edgeCount = min(u32(layerEdges.count.x), 0u);",
        ],
      ]);
      expect([...kernelBindings(special.reference).keys()]).toEqual([
        ...kernelBindings(general.reference).keys(),
      ]);
    },
  );

  it("refuses sources that hold a place to rewrite nowhere, or twice", () => {
    const empty = { ...RAY_MARCH_KERNEL, name: "no march", reference: "fn main() {}" };
    expect(() => withMarchSpecialised(empty)).toThrow(/^kernel no march holds no single place/u);
    const twice = {
      ...RAY_MARCH_KERNEL,
      name: "two marches",
      reference: `${RAY_MARCH_KERNEL.reference}\n${RAY_MARCH_KERNEL.reference}`,
    };
    expect(() => withMarchSpecialised(twice)).toThrow(/^kernel two marches holds no single place/u);
  });

  // From the tables alone (R08.T11.d's brief item 5, ruled through main): one slice, one band and
  // no layer edge, never the body's class. Each row names what its tables hold: several slices,
  // several bands, layer edges.
  const venus = venusClassMedium();
  it.each<
    readonly [
      string,
      AtmosphereMedium,
      AbsorberCurveSlots,
      AtmosphereBody | null,
      number,
      readonly [boolean, boolean, boolean],
      boolean,
    ]
  >([
    [
      "R05's Earth on WGS 84",
      R05_EARTH_REFERENCE,
      NO_ABSORBER_CURVES,
      earthBody(),
      tableRadiusM(WGS84),
      [false, false, false],
      true,
    ],
    [
      "the measured Earth on WGS 84",
      EARTH_REFERENCE,
      EARTH_REFERENCE_CURVES,
      earthBody(),
      tableRadiusM(WGS84),
      [false, false, false],
      true,
    ],
    [
      "the slab Earth",
      EARTH_SLAB_REFERENCE,
      EARTH_SLAB_REFERENCE_CURVES,
      earthBody(),
      tableRadiusM(WGS84),
      [false, false, true],
      false,
    ],
    [
      "a Saturn-class figure",
      saturnClassMedium(),
      NO_ABSORBER_CURVES,
      saturnBody(),
      tableRadiusM(SATURN_SPHEROID),
      [true, true, false],
      false,
    ],
    [
      "a medium with a cloud deck",
      venus.medium,
      NO_ABSORBER_CURVES,
      null,
      venus.radiusM,
      [false, false, true],
      false,
    ],
  ])(
    "chooses the march for %s from its tables alone",
    async (_name, medium, curves, body, radiusM, holds, specialised) => {
      const engine = await countingRenderEngine();
      const tables = new AtmosphereTables(engine, medium, radiusM, curves, body);
      expect([
        tables.plan.kappa.length > 1,
        tables.plan.bands.length > 1,
        marchEdgeCountOf(tables.layerEdges) > 0,
      ]).toEqual(holds);
      expect(marchSpecialisedFor(tables)).toBe(specialised);
      tables.dispose();
    },
  );

  it("marches such a body's area by the specialised march, and the reference path and the tile cull by the general", async () => {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    const covers = (["area", "whole frame", "tiles"] as const).map((cover) => {
      view.setMarchCover(cover);
      engine.dispatched.length = 0;
      view.drawFrame(distant, [NOON], SCENE);
      return [cover, marchesOf(engine).map((d) => d.kernel)];
    });
    expect(covers).toEqual([
      ["area", [RAY_MARCH_SPECIAL_KERNEL.name]],
      ["whole frame", [RAY_MARCH_KERNEL.name]],
      ["tiles", [RAY_MARCH_CENTRE_KERNEL.name, RAY_MARCH_KERNEL.name]],
    ]);
  });

  it("marches the area of a body of layer edges by the general march", async () => {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables = new AtmosphereTables(
      engine,
      EARTH_SLAB_REFERENCE,
      tableRadiusM(WGS84),
      EARTH_SLAB_REFERENCE_CURVES,
    );
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    engine.dispatched.length = 0;
    view.drawFrame(distant, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual([
      tables.slotDepths === null ? RAY_MARCH_KERNEL.name : RAY_MARCH_SLOTS_KERNEL.name,
    ]);
  });

  it("gives the specialised march every binding it declares, and no other", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    atmosphere.setMarchCover("area");
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const march = engine.dispatched.find((d) => d.kernel === RAY_MARCH_SPECIAL_KERNEL.name);
    expect(march === undefined ? [] : boundNames(march.bindings)).toEqual(
      [...kernelBindings(RAY_MARCH_SPECIAL_KERNEL.reference).keys()].toSorted(),
    );
  });
});

describe("HillaireAtmosphere's march by slot (R08.T8.e, lever 5)", () => {
  const absorbing = fourAbsorberMedium();

  it("marches by slot a body whose tables hold the slot layers, binding them", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const march = engine.dispatched.at(-1);
    expect([march?.kernel, march?.bindings.sampled["slotDepths"]]).toEqual([
      "atmosphere ray march by slot",
      atmosphere.tables.slotDepths,
    ]);
  });

  it("marches with the shared read a body that keeps it, and binds it no layers", async () => {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, TABLE_SIZES.high, WGS84);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    const march = engine.dispatched.at(-1);
    expect([march?.kernel, Object.hasOwn(march?.bindings.sampled ?? {}, "slotDepths")]).toEqual([
      "atmosphere ray march",
      false,
    ]);
  });

  it.each([
    ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL, absorbing.medium, absorbing.curves],
    ["the ray-march kernel", RAY_MARCH_KERNEL, R05_EARTH_REFERENCE, NO_ABSORBER_CURVES],
  ] as const)(
    "gives %s every binding it declares, and no other",
    async (_name, kernel, medium, curves) => {
      const engine = await countingRenderEngine();
      const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
      engine.dispatched.length = 0;
      atmosphere.drawFrame(CAMERA, [NOON], SCENE);
      const march = engine.dispatched.find((d) => d.kernel === kernel.name);
      expect(march === undefined ? [] : boundNames(march.bindings)).toEqual(
        [...kernelBindings(kernel.reference).keys()].toSorted(),
      );
    },
  );

  it("makes the march by slot at the first frame that takes it, and once", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    const before = engine.counts.kernels;
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    atmosphere.drawFrame(CAMERA, [DUSK], SCENE);
    expect(engine.counts.kernels - before).toBe(1);
  });

  it("returns to the shared march for a new medium that keeps the shared read", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    atmosphere.setMedium(R05_EARTH_REFERENCE);
    engine.dispatched.length = 0;
    atmosphere.drawFrame(CAMERA, [NOON], SCENE);
    expect(engine.dispatched.at(-1)?.kernel).toBe("atmosphere ray march");
  });

  it("marches by slot in the kernel arm too, in rgba32float", async () => {
    const engine = await countingRenderEngine();
    const { medium, curves } = absorbing;
    const atmosphere = new HillaireAtmosphere(engine, medium, TABLE_SIZES.high, WGS84, curves);
    engine.dispatched.length = 0;
    atmosphere.drawKernelArm(CAMERA, [NOON], SCENE);
    expect(engine.dispatched.at(-1)?.kernel).toBe(
      `atmosphere ray march by slot (${KERNEL_ARM_FORMAT})`,
    );
  });
});
