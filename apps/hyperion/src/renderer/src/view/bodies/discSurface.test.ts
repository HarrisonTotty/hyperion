import { describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { PhotometricLaw } from "../appearance/law";
import { lawFor } from "../appearance/phase";
import { pixelSolidAngle, type ProjectionCamera, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import {
  type Rotation3,
  rotateToBody,
  rotateToBodyFixed,
  rotation3FromRows,
} from "../coords/rotation";
import type { TextureHandle } from "../engine/types";
import type { PlacedLight } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import { AU_M } from "../scenes/kept";
import { faceUvToXyz, stToUv, vertexDir } from "../terrain/cube";
import {
  MATURITY_NODES,
  maturityCodeRatio,
  maturityNodeAlbedo,
  maturitySegment,
} from "../terrain/maturityNodes";
import { EDGES, type Edge, edgeNeighbour, type Face } from "../terrain/patchKey";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import {
  type CompositePixel,
  compositeDiscPixels,
  DISC_ROWS,
  FIRST_NODE_ROW,
  packDiscRecords,
  rasteriseDisc,
  viewRay,
} from "./discShading";
import {
  CLASS_MAP_FORMAT,
  classMapAlbedoAt,
  type ClassMapMaturityInput,
  type ClassMapTexel,
  type ClassMapTexels,
  classMapLayers,
  classMapMaturityLayer,
  classMapSharesAt,
  classMapSide,
  classMapSurface,
  classMapTexelLayers,
  classMapTexelOf,
  classMapTextureSpec,
  classWeightsAt,
  discSurfaceLaws,
  MAX_DISC_CLASSES,
  packClassMap,
  surfaceShares,
} from "./discSurface";
import { LitBodyRenderer, type LitBodyInput, planLitBodies } from "./draw";
import { oblateAlbedoScale } from "./oblate";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const EXPOSURE = 1e-3;
const RAD = Math.PI / 180;
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  presentedPx: VIEWPORT,
  exposureScale: EXPOSURE,
  annuli: 4,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
};
const RADIUS_M = 6.371e6;

/** A dark lunar law, a bright terrestrial one and the provisional Lambert law, for unsurveyed ground. */
const DARK = lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon");
const BRIGHT = lawFor([0.5, 0.45, 0.4], [1.3, 1.3, 1.3], "earth");
const UNIFORM = PROVISIONAL_PHOTOMETRY.law;

/** R = Rz(γ) Rx(β): the pole tilted by β and turned by γ. */
function tilted(betaRad: number, gammaRad: number): Rotation3 {
  const [cb, sb, cg, sg] = [
    Math.cos(betaRad),
    Math.sin(betaRad),
    Math.cos(gammaRad),
    Math.sin(gammaRad),
  ];
  return rotation3FromRows([
    vec3(cg, -sg * cb, sg * sb),
    vec3(sg, cg * cb, -cg * sb),
    vec3(0, sb, cb),
  ]);
}

const ROTATION = tilted(55 * RAD, 30 * RAD);

/** A handle the twin never reads: the class map's texels go to `rasteriseDisc` directly. */
const WEIGHTS: TextureHandle = { kind: "texture", name: "test class map" };

/**
 * A surface's texels where no test reads them: the disc's twin takes its map as `rasteriseDisc`'s
 * argument, and no point is drawn.
 */
function unreadTexels(classes: number): ClassMapTexels {
  return { faceTexels: 1, classes, data: new Uint8Array(0) };
}

function classMap(
  laws: ReadonlyArray<PhotometricLaw>,
  elsewhere: PhotometricLaw = UNIFORM,
): ClassMapDiscSurface {
  return {
    kind: "class-map",
    weights: WEIGHTS,
    texels: unreadTexels(laws.length),
    laws,
    elsewhere,
    normals: null,
  };
}

/**
 * The synthetic two-class map: half the cells surveyed, those with i + j even; of them, class 0
 * where i and j are even, a mix of a quarter class 0 and three quarters class 1 where both are odd.
 */
function twoClassMap(faceTexels: number): ClassMapTexels {
  return packClassMap(faceTexels, 2, ({ i, j }) => {
    if ((i + j) % 2 !== 0) {
      return null;
    }
    return i % 2 === 0 ? [1, 0] : [0.25, 0.75];
  });
}

/** A body `diameterPx` across straight ahead, lit by a Sun 1 au away at `phaseDeg`. */
function bodyAt(
  diameterPx: number,
  phaseDeg: number,
  extra: Partial<LitBodyInput> = {},
  polarOverEquatorial = 1,
): { readonly body: LitBodyInput; readonly hosts: PlacedLight[] } {
  const distanceM = RADIUS_M / Math.sin(diameterPx / 2 / PX_PER_RAD);
  const centreM = vec3(0, 0, -distanceM);
  const towards = vec3(Math.sin(phaseDeg * RAD), 0, Math.cos(phaseDeg * RAD));
  return {
    body: {
      id: "0200080020000000.0300",
      centreM,
      figure: {
        equatorialRadiusM: RADIUS_M,
        polarRadiusM: RADIUS_M * polarOverEquatorial,
        pole: rotateToBody(ROTATION, vec3(0, 0, 1)),
      },
      photometry: PROVISIONAL_PHOTOMETRY,
      lighting: undefined,
      ...extra,
    },
    hosts: [{ disc: aHostDisc(), centreM: add(centreM, scale(towards, AU_M)) }],
  };
}

/** The body's disc as both draws leave it, keyed by pixel. */
function drawn(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  texels: ClassMapTexels | null = null,
): Map<string, CompositePixel> {
  const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
  const record = plan.discs[0];
  if (record === undefined) {
    throw new Error("the body is not drawn as a disc");
  }
  const pixels = compositeDiscPixels(rasteriseDisc(record, CAMERA, VIEWPORT, texels));
  return new Map(pixels.map((p) => [`${String(p.xPx)},${String(p.yPx)}`, p]));
}

/** The disc's flux at the camera per channel, lx: Σ L Ω over its pixels. */
function discFlux(pixels: ReadonlyMap<string, CompositePixel>): [number, number, number] {
  const flux: [number, number, number] = [0, 0, 0];
  for (const p of pixels.values()) {
    const omega = pixelSolidAngle(
      viewRay(p.xPx + 0.5, p.yPx + 0.5, CAMERA, VIEWPORT),
      CAMERA,
      VIEWPORT,
    );
    for (const c of [0, 1, 2] as const) {
      flux[c] += (p.rgb[c] * omega) / EXPOSURE;
    }
  }
  return flux;
}

/**
 * A sphere's flux 64 px across at zero phase, where F ÷ (E (a ÷ D)²) is its p, so that ratios of
 * p are ratios of flux.
 */
function fullFlux(
  extra: Partial<LitBodyInput>,
  texels: ClassMapTexels | null = null,
): [number, number, number] {
  const { body, hosts } = bodyAt(64, 0, { rotation: ROTATION, ...extra });
  return discFlux(drawn(body, hosts, texels));
}

/** Each channel's |measured ÷ expected − 1|, at most. */
function worstRatio(measured: ReadonlyArray<number>, expected: ReadonlyArray<number>): number {
  return Math.max(...measured.map((m, c) => Math.abs(m / (expected[c] ?? Number.NaN) - 1)));
}

/**
 * The cube-sphere direction under a pixel's centre, by an `f64` ray–spheroid hit of its own in
 * the body-fixed axes: the hit's body-fixed point over (a, a, c), which R05's `spheroidPoint` maps
 * back to it. In that space the spheroid is the unit sphere, and the hit is the ray's foot nearest
 * the centre, r̂ × (o × r̂), less √(1 − |foot|²) along r̂: cross products, so that no two large
 * numbers cancel, as the reconstruction's shares, which vary across a texel (R10.T10.d), need
 * the hit to some 10⁻¹⁵ where T8.b's nearest read did not.
 */
function cubeDirectionAt(
  body: LitBodyInput,
  xPx: number,
  yPx: number,
): readonly [number, number, number] | null {
  const { equatorialRadiusM: a, polarRadiusM: c } = body.figure;
  const toScaled = (v: Vec3): Vec3 => {
    const fixed = rotateToBodyFixed(ROTATION, v);
    return vec3(fixed.x / a, fixed.y / a, fixed.z / c);
  };
  const origin = toScaled(scale(body.centreM, -1));
  const ray = normalise(toScaled(viewRay(xPx + 0.5, yPx + 0.5, CAMERA, VIEWPORT)));
  if (dot(origin, ray) >= 0) {
    return null;
  }
  const foot = cross(ray, cross(origin, ray));
  const h2 = 1 - dot(foot, foot);
  if (h2 < 0) {
    return null;
  }
  const h = Math.sqrt(h2);
  return [foot.x - h * ray.x, foot.y - h * ray.y, foot.z - h * ray.z];
}

/** Values as the shader reads them, rounded to `f32`. */
function f32(values: ReadonlyArray<number>): number[] {
  return values.map((v) => Math.fround(v));
}

describe("the class map's layout", () => {
  it("puts each cell of R05's quadtree at its own texel: a level-2 patch's centre in (face, i, j) of a 4-texel map", () => {
    const misplaced: string[] = [];
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      for (let i = 0; i < 4; i += 1) {
        for (let j = 0; j < 4; j += 1) {
          const texel = classMapTexelOf(vertexDir({ face, level: 2, i, j }, 32, 32), 4);
          if (texel.face !== face || texel.i !== i || texel.j !== j) {
            misplaced.push(`(${face}, ${i}, ${j}) at (${texel.face}, ${texel.i}, ${texel.j})`);
          }
        }
      }
    }
    expect(misplaced).toEqual([]);
  });

  it("holds class k's weight in channel k mod 4 of layer face + 6 ⌊k ÷ 4⌋, six layers for every four classes", () => {
    const texel: ClassMapTexel = { face: 4, i: 2, j: 1 };
    const map = packClassMap(4, 6, (t) =>
      t.face === texel.face && t.i === texel.i && t.j === texel.j ? [0, 0, 0, 0, 0, 1] : null,
    );
    expect(classMapLayers(6)).toBe(12);
    // Six texels a side: the face's four and the gutter.
    expect(classMapSide(4)).toBe(6);
    expect(map.data).toHaveLength(12 * 6 * 6 * 4);
    // Class 5: channel 1 of layer 4 + 6, at (i + 1, j + 1).
    const at = (((4 + 6) * 6 + 2) * 6 + 3) * 4 + 1;
    expect(map.data[at]).toBe(255);
    expect(map.data.reduce((sum, b) => sum + b, 0)).toBe(255);
  });

  it("keeps a surveyed texel's weights summing to 255 when it rounds them to bytes", () => {
    const thirds = packClassMap(1, 3, () => [1 / 3, 1 / 3, 1 / 3]);
    const halves = packClassMap(1, 2, () => [0.5, 0.5]);
    const sums = [thirds, halves].map((map) => {
      const weights = classWeightsAt(map, [1, 0, 0]);
      return Math.round(255 * weights.reduce((sum, w) => sum + w, 0));
    });
    expect(sums).toEqual([255, 255]);
  });

  it("reads an unsurveyed texel as no weight, which leaves the whole share to the uniform law", () => {
    const map = twoClassMap(4);
    const unsurveyed = classMapTexelOf([1, 0.2, -0.3], 4);
    expect((unsurveyed.i + unsurveyed.j) % 2).toBe(1);
    expect(surfaceShares(classWeightsAt(map, [1, 0.2, -0.3]))).toEqual([1, 0, 0]);
  });

  it("gives the uniform law what the weights leave", () => {
    expect(surfaceShares([0.25, 0.75])).toEqual([0, 0.25, 0.75]);
    expect(surfaceShares([0.2, 0.3])).toEqual([0.5, 0.2, 0.3]);
  });

  it("scales weights summing past 1 down to 1, leaving the uniform law nothing", () => {
    expect(surfaceShares([0.75, 0.75])).toEqual([0, 0.5, 0.5]);
  });

  it("refuses a map of more than MAX_DISC_CLASSES classes", () => {
    expect(() => packClassMap(2, MAX_DISC_CLASSES + 1, () => null)).toThrow(/1 to 16 classes/);
  });

  it("refuses a map with no texels a side", () => {
    expect(() => packClassMap(0, 2, () => null)).toThrow(/positive whole number of texels/);
  });

  it("refuses a texel whose weights are not one per class", () => {
    expect(() => packClassMap(2, 2, () => [1])).toThrow(/has 1 weights for 2 classes/);
  });

  it("refuses a surface whose laws are not one per class", async () => {
    const engine = await countingRenderEngine();
    expect(() => classMapSurface(engine, "map", twoClassMap(2), [DARK], UNIFORM)).toThrow(
      /2 classes needs as many laws/,
    );
  });

  it("uploads a class map as a 2D array of rgba8unorm texels, gutter and every layer at once, under coarse-field-gpu", async () => {
    const engine = await countingRenderEngine();
    const map = twoClassMap(4);
    const surface = classMapSurface(engine, "moon class map", map, [DARK, BRIGHT], UNIFORM);
    expect(surface).toMatchObject({ kind: "class-map", laws: [DARK, BRIGHT], elsewhere: UNIFORM });
    // The texels travel with the texture, the CPU copy the point regime integrates (R10.T10.f).
    expect(surface.texels).toBe(map);
    expect(engine.textureSpecs.at(-1)).toMatchObject({
      name: "moon class map",
      size: { width: 6, height: 6, depthOrArrayLayers: 6 },
      dimension: "2d",
      format: CLASS_MAP_FORMAT,
      category: "coarse-field-gpu",
    });
    const write = engine.textureWritten.at(-1);
    expect(write?.size).toEqual({ width: 6, height: 6, depthOrArrayLayers: 6 });
    expect(write?.data).toEqual(map.data);
  });

  it("orders a surface's laws as the record does: the uniform law, or elsewhere then each class's", () => {
    expect(discSurfaceLaws({ kind: "uniform", law: DARK })).toEqual([DARK]);
    expect(discSurfaceLaws(classMap([DARK, BRIGHT]))).toEqual([UNIFORM, DARK, BRIGHT]);
  });
});

describe("a disc under a class map", () => {
  it("shades each point by its laws' reconstructed shares, the uniform law's alone in unsurveyed texels", () => {
    // 64 px across, f = 0.1, its pole tilted towards the camera; one sample a pixel inside.
    const map = twoClassMap(4);
    const { body, hosts } = bodyAt(
      64,
      40,
      { surface: classMap([DARK, BRIGHT]), rotation: ROTATION },
      0.9,
    );
    const mapped = drawn(body, hosts, map);
    const byLaw = [UNIFORM, DARK, BRIGHT].map((law) =>
      drawn({ ...body, surface: { kind: "uniform", law } }, hosts),
    );
    const cx = VIEWPORT.widthPx / 2;
    const cy = VIEWPORT.heightPx / 2;
    const seen = [0, 0, 0];
    let worst = 0;
    for (const [key, pixel] of mapped) {
      // Pixels well inside the limb, whose one sample is their centre's.
      if (Math.hypot(pixel.xPx + 0.5 - cx, pixel.yPx + 0.5 - cy) > 0.7 * 32 * 0.9) {
        continue;
      }
      const d = cubeDirectionAt(body, pixel.xPx, pixel.yPx);
      if (d === null) {
        throw new Error(`pixel ${key} misses the body`);
      }
      const shares = classMapSharesAt(map, d);
      shares.forEach((share, m) => {
        seen[m] = (seen[m] ?? 0) + (share > 0 ? 1 : 0);
      });
      for (const c of [0, 1, 2] as const) {
        const expected = shares.reduce(
          (sum, share, m) => sum + share * (byLaw[m]?.get(key)?.rgb[c] ?? Number.NaN),
          0,
        );
        worst = Math.max(worst, Math.abs(pixel.rgb[c] / expected - 1));
      }
    }
    expect(seen.every((count) => count > 100)).toBe(true);
    expect(worst).toBeLessThan(1e-12);
  });

  it("draws the uniform disc to 10⁻⁴ under a map of one class equal to the uniform law surveyed whole (R10.T10.d)", () => {
    const map = packClassMap(4, 1, () => [1]);
    const same = lawFor([0.2, 0.2, 0.2], [1.5, 1.5, 1.5], "lambert");
    const { body, hosts } = bodyAt(40, 60, {}, 0.9);
    const uniform = drawn(body, hosts);
    const mapped = drawn({ ...body, surface: classMap([same]), rotation: ROTATION }, hosts, map);
    expect(mapped.size).toBe(uniform.size);
    let worst = 0;
    for (const [key, pixel] of uniform) {
      for (const c of [0, 1, 2] as const) {
        const other = mapped.get(key)?.rgb[c] ?? Number.NaN;
        worst = Math.max(worst, Math.abs(other - pixel.rgb[c]) / Math.max(pixel.rgb[c], 1e-30));
      }
    }
    expect(worst).toBeLessThan(1e-4);
  });

  it("draws the uniform disc to 10⁻⁵ under a map of one class equal to the uniform law", () => {
    const map = packClassMap(4, 1, ({ i, j }) => ((i + j) % 2 === 0 ? [1] : null));
    // A law equal to the uniform one, not the same object.
    const same = lawFor([0.2, 0.2, 0.2], [1.5, 1.5, 1.5], "lambert");
    for (const diameterPx of [20, 64]) {
      const { body, hosts } = bodyAt(diameterPx, 70, {}, 0.9);
      const uniform = drawn(body, hosts);
      const mapped = drawn({ ...body, surface: classMap([same]), rotation: ROTATION }, hosts, map);
      expect(mapped.size).toBe(uniform.size);
      let worst = 0;
      for (const [key, pixel] of uniform) {
        for (const c of [0, 1, 2] as const) {
          const other = mapped.get(key)?.rgb[c] ?? Number.NaN;
          worst = Math.max(worst, Math.abs(other - pixel.rgb[c]) / Math.max(pixel.rgb[c], 1e-30));
        }
      }
      expect(worst).toBeLessThan(1e-5);
    }
  });

  it("integrates to the mix of its classes' p when every texel holds the same mix", () => {
    const mix = packClassMap(4, 2, () => [0.25, 0.75]);
    const [w0 = 0, w1 = 0] = classWeightsAt(mix, [1, 0, 0]);
    const dark = fullFlux({ surface: { kind: "uniform", law: DARK } });
    const bright = fullFlux({ surface: { kind: "uniform", law: BRIGHT } });
    const mixed = fullFlux({ surface: classMap([DARK, BRIGHT]) }, mix);
    expect(
      worstRatio(
        mixed,
        [0, 1, 2].map((c) => w0 * (dark[c] ?? 0) + w1 * (bright[c] ?? 0)),
      ),
    ).toBeLessThan(1e-9);
  });

  it("integrates to the area-weighted p of its classes and the uniform law when half surveyed", () => {
    // 16 texels a face, the pattern fine against the disc; each law's area over 2 × 10⁵ directions
    // spread evenly on the sphere, its shares as the reconstruction gives them (R10.T10.d's 1%;
    // T8.b's 0.3% holds).
    const map = twoClassMap(16);
    const points = 200_000;
    const area = [0, 0, 0];
    for (let n = 0; n < points; n += 1) {
      const z = 1 - (2 * (n + 0.5)) / points;
      const r = Math.sqrt(1 - z * z);
      const phi = n * Math.PI * (3 - Math.sqrt(5));
      classMapSharesAt(map, [r * Math.cos(phi), r * Math.sin(phi), z]).forEach((share, m) => {
        area[m] = (area[m] ?? 0) + share / points;
      });
    }
    const byLaw = [UNIFORM, DARK, BRIGHT].map((law) =>
      fullFlux({ surface: { kind: "uniform", law } }),
    );
    const weighted = [0, 1, 2].map((c) =>
      byLaw.reduce((sum, flux, m) => sum + (area[m] ?? 0) * (flux[c] ?? 0), 0),
    );
    const patterned = fullFlux({ surface: classMap([DARK, BRIGHT]) }, map);
    expect(worstRatio(patterned, weighted)).toBeLessThan(0.003);
  });

  it("shades with the uniform law where the body's rotation is not known", () => {
    const { body, hosts } = bodyAt(40, 30, { surface: classMap([DARK, BRIGHT]) });
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    expect(plan.discs[0]?.surface).toEqual({ kind: "uniform", law: UNIFORM });
    expect(plan.laws).toEqual([UNIFORM]);
  });

  it("refuses to draw its twin without the map's texels", () => {
    const { body, hosts } = bodyAt(40, 30, {
      surface: classMap([DARK, BRIGHT]),
      rotation: ROTATION,
    });
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs[0];
    if (record === undefined) {
      throw new Error("no disc");
    }
    expect(() => rasteriseDisc(record, CAMERA, VIEWPORT)).toThrow(/needs its map's texels/);
  });

  it("refuses to draw its twin with another map's texels", () => {
    const { body, hosts } = bodyAt(40, 30, {
      surface: classMap([DARK, BRIGHT]),
      rotation: ROTATION,
    });
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs[0];
    if (record === undefined) {
      throw new Error("no disc");
    }
    const oneClass = packClassMap(4, 1, () => [1]);
    expect(() => rasteriseDisc(record, CAMERA, VIEWPORT, oneClass)).toThrow(
      /needs its map's texels/,
    );
  });

  it("refuses to pack a surface of more than MAX_DISC_CLASSES classes", () => {
    const laws = Array.from({ length: MAX_DISC_CLASSES + 1 }, () => DARK);
    const { body, hosts } = bodyAt(40, 30, { surface: classMap(laws), rotation: ROTATION });
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    expect(() => packDiscRecords(plan.discs)).toThrow(/at most 16 classes, got 17/);
  });
});

describe("a class-map disc's record", () => {
  it("lists each law once in the phase table, the uniform law first", () => {
    const { body, hosts } = bodyAt(40, 30, {
      surface: classMap([DARK, UNIFORM]),
      rotation: ROTATION,
    });
    const other: LitBodyInput = {
      ...body,
      id: "0200080020000000.0301",
      centreM: vec3(1e8, 0, -1e9),
      surface: { kind: "uniform", law: BRIGHT },
    };
    const plan = planLitBodies(
      [body, other],
      hosts,
      OPTIONS,
      new Map([
        [body.id, "disc"],
        [other.id, "disc"],
      ]),
    );
    expect(plan.laws).toHaveLength(3);
    const record = plan.discs.find((r) => r.body === body.id);
    expect(record?.tableRows.map((row) => plan.laws[row])).toEqual([UNIFORM, DARK, UNIFORM]);
  });

  it("packs the body-fixed axes, each class's scaled A and L, and their table rows where the shader reads them", () => {
    const { body, hosts } = bodyAt(
      40,
      30,
      { surface: classMap([DARK, BRIGHT]), rotation: ROTATION },
      0.9,
    );
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs[0];
    if (record === undefined) {
      throw new Error("no disc");
    }
    const packed = packDiscRecords([record]);
    expect(packed).toHaveLength(DISC_ROWS * 4);
    const row = (i: number): number[] => Array.from(packed.subarray(4 * i, 4 * i + 4));
    const scaled = (law: PhotometricLaw): number[] => {
      const k = oblateAlbedoScale(law.lommelSeeligerShare, 0.9);
      return f32([law.a[0] * k, law.a[1] * k, law.a[2] * k, law.lommelSeeligerShare]);
    };
    expect(row(4)).toEqual(scaled(UNIFORM));
    expect(row(5)[3]).toBe(2);
    const x = rotateToBody(ROTATION, vec3(1, 0, 0));
    const y = rotateToBody(ROTATION, vec3(0, 1, 0));
    expect(row(24)).toEqual(f32([x.x, x.y, x.z, 0]));
    expect(row(25)).toEqual(f32([y.x, y.y, y.z, 0]));
    expect(row(26)).toEqual(scaled(DARK));
    expect(row(27)).toEqual(scaled(BRIGHT));
    expect(row(42)).toEqual([record.tableRows[1], record.tableRows[2], 0, 0]);
  });

  it("binds its map's texture to its draws, and a uniform disc a texel of no weight", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const { body, hosts } = bodyAt(40, 30, {
      surface: classMap([DARK, BRIGHT]),
      rotation: ROTATION,
    });
    const other: LitBodyInput = {
      ...bodyAt(40, 30).body,
      id: "0200080020000000.0301",
      centreM: vec3(1e8, 0, -1e9),
    };
    const plan = planLitBodies(
      [body, other],
      hosts,
      OPTIONS,
      new Map([
        [body.id, "disc"],
        [other.id, "disc"],
      ]),
    );
    const bound = renderer.draws(plan).map((draw) => draw.textures["classWeights"]?.name);
    expect(bound).toHaveLength(4);
    expect(bound.filter((name) => name === "test class map")).toHaveLength(2);
    expect(bound.filter((name) => name === "bodies:no class map")).toHaveLength(2);
    renderer.dispose();
  });
});

/** The body-fixed direction of face coordinates (s, t) on `face`. */
function directionAt(face: Face, s: number, t: number): readonly [number, number, number] {
  return faceUvToXyz(face, stToUv(s), stToUv(t));
}

/** The bytes of class `k` at stored texel (x, y) of `face`, as the layout documents them. */
function storedByte(map: ClassMapTexels, face: Face, x: number, y: number, k: number): number {
  const side = classMapSide(map.faceTexels);
  const layer = face + 6 * Math.floor(k / 4);
  return map.data[((layer * side + y) * side + x) * 4 + (k % 4)] ?? Number.NaN;
}

describe("the survey-masked reconstruction (R10.T10.d)", () => {
  it("fills each face's gutter with the edge texels across its edges, its corners empty", () => {
    // Every texel its own weights: class 0 at (1 + its index) ÷ 255.
    const n = 4;
    const id = (face: number, i: number, j: number): number => (face * n + j) * n + i + 1;
    const map = packClassMap(n, 2, ({ face, i, j }) => [
      id(face, i, j) / 255,
      1 - id(face, i, j) / 255,
    ]);
    const wrong: string[] = [];
    const gutter = (edge: Edge, at: number): readonly [number, number, number, number] => {
      // The face texel beside the gutter texel, and the gutter texel's stored place.
      switch (edge) {
        case "UMin":
          return [0, at, 0, at + 1];
        case "UMax":
          return [n - 1, at, n + 1, at + 1];
        case "VMin":
          return [at, 0, at + 1, 0];
        case "VMax":
          return [at, n - 1, at + 1, n + 1];
      }
      throw new Error(`no edge ${String(edge)}`);
    };
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      for (const edge of EDGES) {
        for (let at = 0; at < n; at += 1) {
          const [i, j, x, y] = gutter(edge, at);
          // The quadtree's own neighbour across the edge (R05's `edgeNeighbour`), independently.
          const across = edgeNeighbour({ face, level: 2, i, j }, edge);
          const expected = id(across.face, across.i, across.j);
          const found = storedByte(map, face, x, y, 0);
          if (found !== expected) {
            wrong.push(`face ${face} ${edge} ${at}: ${found}, not ${expected}`);
          }
        }
      }
      for (const [x, y] of [
        [0, 0],
        [n + 1, 0],
        [0, n + 1],
        [n + 1, n + 1],
      ] as const) {
        if (storedByte(map, face, x, y, 0) !== 0 || storedByte(map, face, x, y, 1) !== 0) {
          wrong.push(`face ${face} corner (${x}, ${y}) holds weight`);
        }
      }
    }
    expect(wrong).toEqual([]);
  });

  it("gives a surveyed texel's own shares at its centre, a partly surveyed one's with elsewhere's rest", () => {
    // Half surveyed, and of the rest a quarter partly surveyed: half the texel, its weights halved.
    const n = 4;
    const map = packClassMap(n, 2, ({ i, j }) => {
      if ((i + j) % 2 === 0) {
        return i % 2 === 0 ? [1, 0] : [0.25, 0.75];
      }
      return i % 2 === 0 && j % 2 === 1 ? [0.25, 0.25] : null;
    });
    let worst = 0;
    let partly = 0;
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      for (let j = 0; j < n; j += 1) {
        for (let i = 0; i < n; i += 1) {
          const d = directionAt(face, (i + 0.5) / n, (j + 0.5) / n);
          const own = surfaceShares(classWeightsAt(map, d));
          if (own[0] === 1) {
            continue;
          }
          partly += own[0] === 0 ? 0 : 1;
          const shares = classMapSharesAt(map, d);
          worst = Math.max(worst, ...shares.map((share, m) => Math.abs(share - (own[m] ?? 0))));
        }
      }
    }
    expect(partly).toBe(6 * 4);
    expect(worst).toBeLessThan(1e-12);
    // The partly surveyed texel at (0, 1) of face 2: half `elsewhere`'s.
    // Each quarter is 63.75 bytes, kept as 64 by the largest remainder.
    expect(classMapSharesAt(map, directionAt(2, 0.5 / n, 1.5 / n))).toEqual([
      expect.closeTo(1 - 128 / 255, 12),
      expect.closeTo(64 / 255, 12),
      expect.closeTo(64 / 255, 12),
    ]);
  });

  it("gives a sample in an unsurveyed texel no surveyed weight, however near the survey", () => {
    const n = 4;
    const map = twoClassMap(n);
    let samples = 0;
    const leaked: string[] = [];
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      // Eight points across each texel, its edges' near sides among them.
      for (let a = 0; a < 8 * n; a += 1) {
        for (let b = 0; b < 8 * n; b += 1) {
          const s = (a + (a % 8 === 7 ? 0.999 : a % 8 === 0 ? 0.001 : 0.5)) / (8 * n);
          const t = (b + (b % 8 === 7 ? 0.999 : b % 8 === 0 ? 0.001 : 0.5)) / (8 * n);
          const d = directionAt(face, s, t);
          if (classWeightsAt(map, d).some((w) => w > 0)) {
            continue;
          }
          samples += 1;
          const shares = classMapSharesAt(map, d);
          if (shares[0] !== 1 || shares.slice(1).some((share) => share !== 0)) {
            leaked.push(`${face} (${s}, ${t}): ${shares.join(", ")}`);
          }
        }
      }
    }
    // Half of each face's texels, 64 points each.
    expect(samples).toBe((6 * n * n * 64) / 2);
    expect(leaked).toEqual([]);
  });

  it("keeps a linear ramp of weights linear across a face's edge to 10⁻⁶, through the gutter", () => {
    // Faces +x (0) and +y (1) meet where u = 1 on +x and u = −1 on +y, v along the edge on both.
    // With t the cell coordinate's distance from the edge, + on face 0 and − on face 1, class 0's
    // weight at the two texel columns either side is 0.5 + 0.2 N t at their centres: bytes 153 and
    // 204 on face 0, 102 and 51 on face 1; every other texel unsurveyed.
    const n = 8;
    const map = packClassMap(n, 2, ({ face, i }) => {
      let byte: number | null = null;
      if (face === 0 && i >= n - 2) {
        byte = 153 + 51 * (n - 1 - i);
      } else if (face === 1 && i <= 1) {
        byte = 102 - 51 * i;
      }
      return byte === null ? null : [byte / 255, 1 - byte / 255];
    });
    const along = (n / 2 + 0.5) / n;
    let worst = 0;
    let crossed = 0;
    for (let k = -29; k <= 29; k += 1) {
      const t = (k / 29) * (1.45 / n);
      const d = t > 0 ? directionAt(0, 1 - t, along) : directionAt(1, -t, along);
      crossed += t > 0 ? 1 : 0;
      const w = 0.5 + 0.2 * n * t;
      const [elsewhere = Number.NaN, first = Number.NaN, second = Number.NaN] = classMapSharesAt(
        map,
        d,
      );
      worst = Math.max(worst, Math.abs(elsewhere), Math.abs(first - w), Math.abs(second - (1 - w)));
    }
    expect(crossed).toBe(29);
    expect(worst).toBeLessThan(1e-6);
  });

  it("refuses no direction on a face's edge or at a cube's corner, its shares summing to 1", () => {
    const map = twoClassMap(4);
    const corner = 1 / Math.sqrt(3);
    for (const d of [
      [1, 1, 0.3],
      [1, -1, 0],
      [corner, corner, corner],
      [-corner, corner, -corner],
      [0, 0, -1],
    ] as const) {
      const shares = classMapSharesAt(map, d);
      expect(shares.reduce((sum, share) => sum + share, 0)).toBeCloseTo(1, 12);
      expect(shares.every((share) => share >= 0 && Number.isFinite(share))).toBe(true);
    }
  });
});

// --- The class map's maturity (plan R10, R10.T1.c's client half) ---------------------------------

/**
 * Monotone nodes for `classes` classes, as `ClassMapMaturity.nodes` holds them: class c's node j
 * in channel ch its background albedo (0.1 + 0.05 c + 0.01 ch) times 1 + 1.5 (1 − j ÷ 15), fresh
 * ground 2.5 times the background, the background at node K − 1.
 */
function monotoneNodes(classes: number): Float32Array {
  const nodes = new Float32Array(3 * MATURITY_NODES * classes);
  for (let c = 0; c < classes; c += 1) {
    for (let j = 0; j < MATURITY_NODES; j += 1) {
      for (let ch = 0; ch < 3; ch += 1) {
        const background = 0.1 + 0.05 * c + 0.01 * ch;
        nodes[3 * (MATURITY_NODES * c + j) + ch] =
          background * (1 + 1.5 * (1 - j / (MATURITY_NODES - 1)));
      }
    }
  }
  return nodes;
}

/** A maturity of `classes` monotone classes, every one drawn unless `flags` says otherwise. */
function maturityOf(
  classes: number,
  codeOf: (texel: ClassMapTexel) => number,
  flags: ReadonlyArray<boolean> = Array.from({ length: classes }, () => true),
): ClassMapMaturityInput {
  return { codeOf, nodes: monotoneNodes(classes), drawn: flags };
}

/** Every texel class 0's alone. */
function firstClassOnly(): ReadonlyArray<number> {
  return [1, 0];
}

/** The code at stored texel (x, y) of `face`, as the layout documents it: r high, g low. */
function storedCodeOf(map: ClassMapTexels, face: Face, x: number, y: number): number {
  const side = classMapSide(map.faceTexels);
  const layer = 6 * Math.ceil(map.classes / 4) + face;
  const at = ((layer * side + y) * side + x) * 4;
  return (map.data[at] ?? Number.NaN) * 256 + (map.data[at + 1] ?? Number.NaN);
}

/** Class `c`'s albedo by its nodes at `code`, as the twin's formula gives it. */
function albedoAtCode(nodes: Float32Array, c: number, code: number): readonly number[] {
  return maturityNodeAlbedo(
    nodes,
    3 * MATURITY_NODES * c,
    maturitySegment(maturityCodeRatio(code)),
  );
}

/** The largest absolute difference between two albedos, channel by channel. */
function albedoMiss(
  found: ReadonlyArray<number> | null | undefined,
  expected: ReadonlyArray<number>,
): number {
  if (found === null || found === undefined) {
    return Number.POSITIVE_INFINITY;
  }
  return Math.max(...expected.map((value, c) => Math.abs((found[c] ?? Number.NaN) - value)));
}

describe("the class map's maturity layers (R10.T1.c)", () => {
  it("holds a surveyed texel's code in layer face + 6 ⌈c ÷ 4⌉ past the weights', its high byte in r and its low in g", () => {
    const texel: ClassMapTexel = { face: 4, i: 2, j: 1 };
    const map = packClassMap(
      4,
      6,
      (t) =>
        t.face === texel.face && t.i === texel.i && t.j === texel.j ? [0, 0, 0, 0, 0, 1] : null,
      maturityOf(6, () => 0xabcd),
    );
    expect([classMapTexelLayers(map), classMapMaturityLayer(4, 6)]).toEqual([18, 16]);
    expect(map.data).toHaveLength(18 * 6 * 6 * 4);
    // Layer 16's texel (i + 1, j + 1), channels r to a.
    const at = ((16 * 6 + 2) * 6 + 3) * 4;
    expect([...map.data.subarray(at, at + 4)]).toEqual([0xab, 0xcd, 0, 0]);
    const maturityBytes = map.data.subarray(12 * 6 * 6 * 4);
    expect(maturityBytes.reduce((sum, b) => sum + b, 0)).toBe(0xab + 0xcd);
  });

  it("takes six more layers for its maturity: 30 at 16 classes, 523 KB at 64 texels a face", () => {
    const maturity = { nodes: monotoneNodes(16), drawn: Array.from({ length: 16 }, () => true) };
    expect(classMapTexelLayers({ classes: 16 })).toBe(24);
    expect(classMapTexelLayers({ classes: 16, maturity })).toBe(30);
    const spec = classMapTextureSpec("moon", { faceTexels: 64, classes: 16, maturity });
    expect(spec.size).toEqual({ width: 66, height: 66, depthOrArrayLayers: 30 });
    expect(66 * 66 * 30 * 4).toBe(522_720);
  });

  it("fills the maturity layers' gutter with the codes across each edge, as the weights', its corners empty", () => {
    const n = 4;
    const code = (face: number, i: number, j: number): number => 500 * ((face * n + j) * n + i + 1);
    const map = packClassMap(
      n,
      2,
      () => [0.5, 0.5],
      maturityOf(2, ({ face, i, j }) => code(face, i, j)),
    );
    const wrong: string[] = [];
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      for (const edge of EDGES) {
        for (let at = 0; at < n; at += 1) {
          const [i, j, x, y] = ((): readonly [number, number, number, number] => {
            switch (edge) {
              case "UMin":
                return [0, at, 0, at + 1];
              case "UMax":
                return [n - 1, at, n + 1, at + 1];
              case "VMin":
                return [at, 0, at + 1, 0];
              case "VMax":
                return [at, n - 1, at + 1, n + 1];
            }
            throw new Error(`no edge ${String(edge)}`);
          })();
          const across = edgeNeighbour({ face, level: 2, i, j }, edge);
          const expected = code(across.face, across.i, across.j);
          const found = storedCodeOf(map, face, x, y);
          if (found !== expected) {
            wrong.push(`face ${face} ${edge} ${at}: ${found}, not ${expected}`);
          }
        }
      }
      for (const [x, y] of [
        [0, 0],
        [n + 1, 0],
        [0, n + 1],
        [n + 1, n + 1],
      ] as const) {
        if (storedCodeOf(map, face, x, y) !== 0) {
          wrong.push(`face ${face} corner (${x}, ${y}) holds a code`);
        }
      }
    }
    expect(wrong).toEqual([]);
  });

  it("packs the weights' bytes as without a maturity, and leaves an unsurveyed texel's code 0", () => {
    const without = twoClassMap(4);
    const map = packClassMap(
      4,
      2,
      ({ i, j }) => {
        if ((i + j) % 2 !== 0) {
          return null;
        }
        return i % 2 === 0 ? [1, 0] : [0.25, 0.75];
      },
      maturityOf(2, () => 65_535),
    );
    const weights = classMapLayers(2) * 6 * 6 * 4;
    expect(map.data.subarray(0, weights)).toEqual(without.data);
    const unsurveyed = classMapTexelOf([1, 0.2, -0.3], 4);
    expect(storedCodeOf(map, unsurveyed.face, unsurveyed.i + 1, unsurveyed.j + 1)).toBe(0);
    expect(storedCodeOf(map, 0, 1, 1)).toBe(65_535);
  });

  it("refuses nodes that are not K × 3 a class, flags not one a class, and a code that is not a whole unorm16", () => {
    expect(() =>
      packClassMap(2, 2, firstClassOnly, {
        ...maturityOf(2, () => 0),
        nodes: new Float32Array(95),
      }),
    ).toThrow(/96 node values and as many flags as classes, not 95 and 2/u);
    expect(() =>
      packClassMap(2, 2, firstClassOnly, { ...maturityOf(2, () => 0), drawn: [true] }),
    ).toThrow(/not 96 and 1/u);
    expect(() =>
      packClassMap(
        2,
        2,
        firstClassOnly,
        maturityOf(2, () => 65_536),
      ),
    ).toThrow(/maturity code 65536, not an integer 0 to 65535/u);
    expect(() =>
      packClassMap(
        2,
        2,
        firstClassOnly,
        maturityOf(2, () => 1.5),
      ),
    ).toThrow(/maturity code 1.5/u);
  });

  it("carries the nodes and the flags with its texels, and no maturity without one", () => {
    const maturity = maturityOf(2, () => 7);
    const map = packClassMap(2, 2, () => [1, 0], maturity);
    expect(map.maturity).toEqual({ nodes: maturity.nodes, drawn: maturity.drawn });
    expect(
      Object.hasOwn(
        packClassMap(2, 2, () => [1, 0]),
        "maturity",
      ),
    ).toBe(false);
  });

  it("uploads every layer, its maturity's included", async () => {
    const engine = await countingRenderEngine();
    const map = packClassMap(
      4,
      2,
      () => [1, 0],
      maturityOf(2, () => 1_000),
    );
    classMapSurface(engine, "matured", map, [DARK, BRIGHT], UNIFORM);
    expect(engine.textureSpecs.at(-1)?.size).toEqual({
      width: 6,
      height: 6,
      depthOrArrayLayers: 12,
    });
    expect(engine.textureWritten.at(-1)?.size).toEqual({
      width: 6,
      height: 6,
      depthOrArrayLayers: 12,
    });
  });
});

describe("each class's A from its nodes (R10.T1.c, classMapAlbedoAt)", () => {
  /** Every texel surveyed, half each class, its code from its place: 600 (24 f + 4 j + i). */
  const n = 4;
  const codeAt = (face: number, i: number, j: number): number => 600 * (face * n * n + j * n + i);
  const everywhere = packClassMap(
    n,
    2,
    () => [0.5, 0.5],
    maturityOf(2, ({ face, i, j }) => codeAt(face, i, j)),
  );

  it("gives none for a map without a maturity", () => {
    const d = directionAt(1, 0.3, 0.6);
    expect(classMapAlbedoAt(twoClassMap(4), d, classMapSharesAt(twoClassMap(4), d))).toBeNull();
  });

  it("gives a texel's own albedo at its centre", () => {
    let worst = 0;
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      for (let j = 0; j < n; j += 1) {
        for (let i = 0; i < n; i += 1) {
          const d = directionAt(face, (i + 0.5) / n, (j + 0.5) / n);
          const albedo = classMapAlbedoAt(everywhere, d, classMapSharesAt(everywhere, d));
          for (const c of [0, 1]) {
            const own = albedoAtCode(
              everywhere.maturity?.nodes ?? new Float32Array(),
              c,
              codeAt(face, i, j),
            );
            worst = Math.max(worst, albedoMiss(albedo?.[c], own));
          }
        }
      }
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("gives the bilinear mean of the four nearest texels' albedos between their centres", () => {
    const nodes = everywhere.maturity?.nodes ?? new Float32Array();
    let worst = 0;
    // Points of face 3 inside the square of texels (1, 1) to (2, 2)'s centres.
    for (const [a, b] of [
      [0.5, 0.5],
      [0.1, 0.8],
      [0.73, 0.21],
      [0.95, 0.05],
    ] as const) {
      const d = directionAt(3, (1.5 + a) / n, (1.5 + b) / n);
      const albedo = classMapAlbedoAt(everywhere, d, classMapSharesAt(everywhere, d));
      for (const c of [0, 1]) {
        const corner = (di: number, dj: number): readonly number[] =>
          albedoAtCode(nodes, c, codeAt(3, 1 + di, 1 + dj));
        const expected = [0, 1, 2].map(
          (ch) =>
            (1 - a) * (1 - b) * (corner(0, 0)[ch] ?? 0) +
            a * (1 - b) * (corner(1, 0)[ch] ?? 0) +
            (1 - a) * b * (corner(0, 1)[ch] ?? 0) +
            a * b * (corner(1, 1)[ch] ?? 0),
        );
        worst = Math.max(worst, albedoMiss(albedo?.[c], expected));
      }
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("leaves an unsurveyed corner out, renormalising over the rest", () => {
    // Texel (1, 1) of face 2 unsurveyed: at the corner it shares with (2, 2), the others' mean.
    const map = packClassMap(
      n,
      2,
      ({ face, i, j }) => (face === 2 && i === 1 && j === 1 ? null : [0.5, 0.5]),
      maturityOf(2, ({ face, i, j }) => codeAt(face, i, j)),
    );
    const nodes = map.maturity?.nodes ?? new Float32Array();
    const d = directionAt(2, 2 / n, 2 / n);
    const albedo = classMapAlbedoAt(map, d, classMapSharesAt(map, d));
    const others = [
      [2, 1],
      [1, 2],
      [2, 2],
    ].map(([i = 0, j = 0]) => albedoAtCode(nodes, 0, codeAt(2, i, j)));
    const expected = [0, 1, 2].map(
      (ch) => others.reduce((sum, each) => sum + (each[ch] ?? 0), 0) / 3,
    );
    expect(albedoMiss(albedo?.[0], expected)).toBeLessThan(1e-12);
  });

  it("gives none in an unsurveyed texel, where elsewhere takes it all", () => {
    const map = packClassMap(
      n,
      2,
      ({ i, j }) => ((i + j) % 2 === 0 ? [0.5, 0.5] : null),
      maturityOf(2, () => 30_000),
    );
    const d = directionAt(5, 1.5 / n, 0.5 / n);
    expect(classMapSharesAt(map, d)).toEqual([1, 0, 0]);
    expect(classMapAlbedoAt(map, d, classMapSharesAt(map, d))).toBeNull();
  });

  it("gives none for a class not drawn from its nodes or of no share at the point", () => {
    // Eight texels a side, so that each point's four nearest texels are of its own kind.
    const map = packClassMap(
      8,
      3,
      ({ i }) => (i < 4 ? [1, 0, 0] : [0, 0.5, 0.5]),
      maturityOf(3, () => 40_000, [true, true, false]),
    );
    const left = directionAt(0, 1.5 / 8, 3.5 / 8);
    const right = directionAt(0, 6.5 / 8, 3.5 / 8);
    const atLeft = classMapAlbedoAt(map, left, classMapSharesAt(map, left));
    const atRight = classMapAlbedoAt(map, right, classMapSharesAt(map, right));
    expect([atLeft?.[0] === null, atLeft?.[1], atLeft?.[2]]).toEqual([false, null, null]);
    expect([atRight?.[0], atRight?.[1] === null, atRight?.[2]]).toEqual([null, false, null]);
  });

  it("keeps an albedo linear in the nodes' coordinate linear across a face's edge to 10⁻⁴, through the gutter", () => {
    // Nodes linear in u = ∛(1 − r): T_j = 0.1 + 0.2 j ÷ 15, so that T = 0.1 + 0.2 u. Faces +x (0)
    // and +y (1) meet where u = 1 on +x and u = −1 on +y; u at the two texel columns either side
    // is 0.5 + 0.2 N t at their centres, t the distance from the edge, + on face 0; each column's
    // code that u's, rounded; every other texel unsurveyed.
    const edgeN = 8;
    const linear = new Float32Array(3 * MATURITY_NODES * 2);
    for (let k = 0; k < 2 * MATURITY_NODES; k += 1) {
      linear.fill(0.1 + (0.2 * (k % MATURITY_NODES)) / (MATURITY_NODES - 1), 3 * k, 3 * k + 3);
    }
    const uAt = (face: number, i: number): number | null => {
      if (face === 0 && i >= edgeN - 2) {
        return 0.5 + 0.2 * (edgeN - i - 0.5);
      }
      if (face === 1 && i <= 1) {
        return 0.5 - 0.2 * (i + 0.5);
      }
      return null;
    };
    const map = packClassMap(
      edgeN,
      2,
      ({ face, i }) => (uAt(face, i) === null ? null : [0.5, 0.5]),
      {
        codeOf: ({ face, i }) => Math.round(65_535 * (1 - (uAt(face, i) ?? 0) ** 3)),
        nodes: linear,
        drawn: [true, true],
      },
    );
    const along = (edgeN / 2 + 0.5) / edgeN;
    let worst = 0;
    let crossed = 0;
    for (let k = -29; k <= 29; k += 1) {
      const t = (k / 29) * (1.45 / edgeN);
      const d = t > 0 ? directionAt(0, 1 - t, along) : directionAt(1, -t, along);
      crossed += t > 0 ? 1 : 0;
      const expected = 0.1 + 0.2 * (0.5 + 0.2 * edgeN * t);
      const albedo = classMapAlbedoAt(map, d, classMapSharesAt(map, d));
      worst = Math.max(worst, albedoMiss(albedo?.[0], [expected, expected, expected]));
    }
    expect(crossed).toBe(29);
    expect(worst).toBeLessThan(1e-4);
  });

  it("leaves the shares as they are without a maturity, bit for bit", () => {
    const without = packClassMap(n, 2, ({ i, j }) => ((i + j) % 2 === 0 ? [0.3, 0.6] : null));
    const withMaturity = packClassMap(
      n,
      2,
      ({ i, j }) => ((i + j) % 2 === 0 ? [0.3, 0.6] : null),
      maturityOf(2, ({ face, i, j }) => codeAt(face, i, j)),
    );
    const differ: string[] = [];
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      for (let a = 0; a < 40; a += 1) {
        for (let b = 0; b < 40; b += 1) {
          const d = directionAt(face, (a + 0.37) / 40, (b + 0.61) / 40);
          const one = classMapSharesAt(without, d);
          const other = classMapSharesAt(withMaturity, d);
          if (one.some((share, m) => !Object.is(share, other[m]))) {
            differ.push(`${face} ${a} ${b}`);
          }
        }
      }
    }
    expect(differ).toEqual([]);
  });
});

describe("a class-map disc's record under its map's maturity (R10.T1.c)", () => {
  /** The disc's record over a two-class map of `texels`, its figure's c ÷ a 0.9. */
  function recordOver(texels: ClassMapTexels): Float32Array {
    const { body, hosts } = bodyAt(
      40,
      30,
      { surface: { ...classMap([DARK, BRIGHT]), texels }, rotation: ROTATION },
      0.9,
    );
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs[0];
    if (record === undefined) {
      throw new Error("no disc");
    }
    return packDiscRecords([record]);
  }

  it("says so in row 25's w and packs each drawn class's nodes, scaled as its A, node 0's w its flag", () => {
    const maturity = maturityOf(2, () => 0, [true, false]);
    const packed = recordOver(packClassMap(4, 2, () => [0.5, 0.5], maturity));
    expect(packed).toHaveLength(DISC_ROWS * 4);
    const row = (i: number): number[] => Array.from(packed.subarray(4 * i, 4 * i + 4));
    expect(row(25)[3]).toBe(1);
    const k = oblateAlbedoScale(DARK.lommelSeeligerShare, 0.9);
    const wrong: string[] = [];
    for (let j = 0; j < MATURITY_NODES; j += 1) {
      const at = 3 * j;
      const expected = f32([
        (maturity.nodes[at] ?? 0) * k,
        (maturity.nodes[at + 1] ?? 0) * k,
        (maturity.nodes[at + 2] ?? 0) * k,
        j === 0 ? 1 : 0,
      ]);
      if (row(FIRST_NODE_ROW + j).some((value, c) => value !== expected[c])) {
        wrong.push(`class 0 node ${j}: ${row(FIRST_NODE_ROW + j).join(", ")}`);
      }
      // The undrawn class's rows are 0.
      if (row(FIRST_NODE_ROW + MATURITY_NODES + j).some((value) => value !== 0)) {
        wrong.push(`class 1 node ${j} written`);
      }
    }
    expect(wrong).toEqual([]);
    expect(packed.subarray(4 * (FIRST_NODE_ROW + 2 * MATURITY_NODES)).every((v) => v === 0)).toBe(
      true,
    );
  });

  it("writes no row past 51 and row 25's w 0 without a maturity, its rows before as they were", () => {
    const without = recordOver(twoClassMap(4));
    const matured = recordOver(
      packClassMap(
        4,
        2,
        () => [0.5, 0.5],
        maturityOf(2, () => 0),
      ),
    );
    expect(without[4 * 25 + 3]).toBe(0);
    expect(without.subarray(4 * FIRST_NODE_ROW).every((v) => v === 0)).toBe(true);
    // The rows before differ in row 25's w alone.
    const before = Array.from(without.subarray(0, 4 * FIRST_NODE_ROW));
    const after = Array.from(matured.subarray(0, 4 * FIRST_NODE_ROW));
    expect(after.flatMap((value, at) => (value === before[at] ? [] : [at]))).toEqual([4 * 25 + 3]);
  });
});
