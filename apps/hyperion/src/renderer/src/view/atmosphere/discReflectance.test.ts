import { describe, expect, it, vi } from "vitest";

import { add, scale, vec3 } from "../../geometry/vec3";
import { wgslFunctions, wgslReached } from "../../test/atmosphereWgsl";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import {
  groundFieldOf,
  heldMagmaOceanSurface,
  lavaWorldSurface,
  magmaOceanSurface,
  warmAirlessSurface,
} from "../../test/thermalSurfaces";
import type { DiscRecord } from "../bodies/discShading";
import { discCellJobs, packDiscRecords, shadeDiscPoint } from "../bodies/discShading";
import {
  BODY_DISC_CELLS_EMISSIVE_KERNEL,
  BODY_DISC_CELLS_KERNEL,
  BODY_DISC_EMISSIVE_MATERIALS,
  BODY_DISC_MATERIALS,
  type BodyFramePlan,
  type DiscCellFrame,
  type LitBodyEngine,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
  SMOOTH_MESH_EMISSIVE_MATERIAL,
  SMOOTH_MESH_MATERIAL,
} from "../bodies/draw";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  type Viewport,
  viewRotation4,
} from "../camera/projection";
import { kernelBindings } from "../engine/webgpu/compute";
import type { PlacedLight } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import type { Rgb } from "../photometry/toneCurve";
import { AU_M } from "../scenes/kept";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import {
  TERRAIN_DEPTH_MATERIALS,
  TERRAIN_LIT_MATERIALS,
  TERRAIN_SHADOW_MATERIALS,
} from "../terrain/terrainMaterial";
import { stillMaterialSpec } from "../terrain/gpu/material";
import { fnv1a64 } from "../terrain/workers/f32Digest";
import {
  DISC_EMISSION_ROWS,
  discEmissionAt,
  packDiscEmission,
  withDiscEmission,
} from "./discReflectance";
import discEmissionWgsl from "./shaders/discEmission.wgsl?raw";
import {
  fieldTemperatureK,
  type SurfaceTemperatureField,
  surfaceTemperatureFieldOf,
  THERMAL_WGSL,
  thermalLuminance,
} from "./thermal";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const RAD = Math.PI / 180;
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
const EXPOSURE = 1e-3;

const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  presentedPx: VIEWPORT,
  exposureScale: EXPOSURE,
  annuli: 4,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
};

/** The lava world's radius, m (the protocol fixture's). */
const RADIUS_M = 8.99e5;

/**
 * The plan of a body `px` across, `offsetPx` right of the view's centre, lit by a Sun `distanceAu`
 * away at phase `phaseDeg`, with the ground temperature field `thermal` where given; drawn as a
 * mesh where `mesh`.
 */
function planOf(
  thermal: SurfaceTemperatureField | null,
  distanceAu: number,
  phaseDeg: number,
  px = 200,
  mesh = false,
): BodyFramePlan {
  const distanceM = RADIUS_M / Math.sin(px / 2 / PX_PER_RAD);
  const centreM = vec3(0, 0, -distanceM);
  const towardsStar = vec3(Math.sin(phaseDeg * RAD), 0, Math.cos(phaseDeg * RAD));
  const lit = aLitBody();
  const body: LitBodyInput = {
    id: lit.body,
    centreM,
    figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: vec3(0, 1, 0) },
    photometry: lit.photometry,
    lighting: undefined,
    ...(thermal === null ? {} : { thermal }),
  };
  const hosts: PlacedLight[] = [
    { disc: aHostDisc(), centreM: add(centreM, scale(towardsStar, distanceAu * AU_M)) },
  ];
  return planLitBodies(
    [body],
    hosts,
    { ...OPTIONS, ...(mesh ? { depthBodyShellRadiiM: new Map([[body.id, RADIUS_M]]) } : {}) },
    new Map([[body.id, "disc"]]),
  );
}

/** The disc record of {@link planOf}'s body. */
function recordOf(
  thermal: SurfaceTemperatureField | null,
  distanceAu: number,
  phaseDeg: number,
  px = 200,
): DiscRecord {
  const record = planOf(thermal, distanceAu, phaseDeg, px).discs[0];
  if (record === undefined) {
    throw new Error("the body was not drawn as a disc");
  }
  return record;
}

/** The `discs` pass's frame for the view. */
const FRAME: DiscCellFrame = {
  viewRotation: viewRotation4(CAMERA.orientation),
  projection: perspectiveReversedInfinite(
    CAMERA.fovXRad,
    VIEWPORT.widthPx / VIEWPORT.heightPx,
    NEAR_PLANE_M,
  ),
  size: VIEWPORT,
};

/** A record as the plain kernels draw it, without its emission. */
function withoutEmission(record: DiscRecord): DiscRecord {
  const { emission: _emission, ...plain } = record;
  return plain;
}

/** A kernel's source as one line-end form: a checkout's line ends do not move its digest. */
function digestOf(source: string): string {
  return fnv1a64(new TextEncoder().encode(source.replaceAll("\r\n", "\n"))).toString(16);
}

/**
 * The plain kernels' sources as they stood before R08.T18.a, by digest: the emissive forms are made
 * beside them and leave each to the byte (the brief's cut; R08.T11.f's cost of a branch not taken).
 * Re-pinned by R08.T15.b.2, whose surface lighting reads the table below a cloud deck: with no deck
 * its eighth table is a one-texel stub, `belowDeckHeld` false, and every lookup's arithmetic is as
 * before, so that their pixels hold. The mesh regime's figure and the terrain's re-pinned by the
 * seamless lane's sub-patches (2026-11-01): the instance record's `sub`, which the figure's struct
 * steps over, and a sub-patch's vertex on its slot's surface, a whole patch's as before. The
 * figure's again by plan R14's `@invariant` clip position (R14.T4.a; `smoothMesh.wgsl`'s
 * `MeshVarying`), which holds its cluster path's vertices to this draw's bits; the arithmetic is as
 * before. The disc's three and the figure re-pinned by lane fix-coast-class-map's fine page
 * (2026-11-02): `surface_at` reads a refined texel's cells, and with no page past the map's own
 * layers every lookup and sum is as before, in the same order, a surveyed corner of no weight adding
 * 0 to the albedo where it was skipped. The figure and the terrain's re-pinned by plan R14's camera
 * strides (R14.T3.c): the instance record's `strides`, which the figure's struct steps over, and the
 * terrain's stride morph (`terrainStrides.wgsl`), which a material without it, or an instance with
 * no bands, skips, its place `lit_place`'s as before. Every one re-pinned by the fitted lookup's
 * least rise (`oblate.wgsl`'s `oblateLeastRiseOf`, 2026-11-03), which only a body of several
 * bands reaches: a body of one band returns before the fit, its every lookup as before. The
 * disc's three and the figure again by lane fix-coast-class-map's fine page at its cells' corners
 * (2026-11-03; the crate's `REFINED_CORNERS_FROM`): a tile whose index's a is 255 read at its
 * cells' corners, every page at the centres as before. The figure and the terrain's again by its
 * class sets (2026-11-03; `terrainLit.wgsl`'s `class_sets`): the instance record's `cells`, which
 * the figure's struct and the depth's and shadows' step over, and a lit texel shaded by its class
 * sets, one set of weight 1 where its instance takes none, as before. Every one re-pinned by
 * R08.T9.b's lit probe: `surfaceLighting.wgsl`'s `LIT_PROBE`, 0 but on a probed profile, whose
 * forms are constant branches that leave every lookup's arithmetic as before; and the disc's and
 * the terrain's by its exact cut, a star's direct term in a loop of one pass, skipped only where
 * its product is 0; and again by its lever, the sources' table, read only where one is bound, its
 * one-texel stub leaving every lookup's arithmetic as before; and by `SOURCE_TABLE_ONLY`, false
 * as written, whose table-only twins the lever's second step draws a body of one band by. The
 * figure and the terrain's again by the shared stride bands (fix-r14-t3c-shared-bands): the
 * record's `edgeBands` and the slot record's `blockErrors`, which the figure's structs step over,
 * and the stride morph's bands from them, which an instance with no scale skips.
 */
const PLAIN_DIGESTS: ReadonlyArray<readonly [string, string, string]> = [
  ["the disc's interior", BODY_DISC_MATERIALS.interior.fragmentWgsl, "e29de864d360e7a5"],
  ["the disc's limb", BODY_DISC_MATERIALS.limb.fragmentWgsl, "e29de864d360e7a5"],
  ["the mesh regime's figure", SMOOTH_MESH_MATERIAL.fragmentWgsl, "87aa4a373ea92e8f"],
  ["the disc cells", BODY_DISC_CELLS_KERNEL.reference, "68aabfba513c8d28"],
  // Re-pinned by plan R14's `@invariant` clip position (terrainLit.wgsl's `LitVertexOut`), which
  // holds every pipeline that places a patch's vertex to the same bits; the arithmetic is as before.
  // And by the seamless lane's sub-patches (R10.T16.h): the instance record's `sub` and a
  // sub-patch's vertex on its slot's surface, a whole patch's as before; and its fades' floor on k
  // and its rebakes', rises', refines' and coarsens' mix of heights, normals and maturity
  // (R10.T16.i), which with no fade are k and the newer bake, bit for bit. Re-pinned by the lane's
  // settled frame (R10.T16.i): the mix behind `TERRAIN_MIXES`, true, and each material's still form
  // beside it, false (`stillMaterialSpec`), the arithmetic as before. And by the hand-over's fade
  // of the terrain's own detail (R10.T15.b.2): the place a texel is lit at (`ShadePlace`), the
  // horizon map's share and the disc's normal map at 24, which at a share of 1, wherever no fade
  // stands, are the texel's own place, map and normal, bit for bit; the depth and the casters
  // compose the shared module alone.
  ...[
    ["terrain-lit-baked-offsets", "2f1d51e7dca5638c"],
    ["terrain-lit-face-differences", "f6fa79d7b7bedac5"],
    // Re-pinned by R10.T16.c's grazing fix, whose depth is pushed by its constant fraction alone.
    ["terrain-depth-baked-offsets", "625ac236b3f0f5"],
    ["terrain-depth-face-differences", "ecc920e6e2ccecd8"],
    ["terrain-shadow-baked-offsets", "164034387f222ea7"],
    ["terrain-shadow-face-differences", "1f9288dbaf5eee52"],
    ["terrain-lit-baked-offsets-still", "ac74e6144029f9ad"],
    ["terrain-lit-face-differences-still", "6cdc2c9431bb5940"],
    ["terrain-depth-baked-offsets-still", "68f5a1db2655cb1a"],
    ["terrain-depth-face-differences-still", "6d903b3d9a1978eb"],
    ["terrain-shadow-baked-offsets-still", "dbdf5be2e18c970a"],
    ["terrain-shadow-face-differences-still", "9b69cd7c5b56485b"],
  ].map(([name = "", digest = ""]): readonly [string, string, string] => {
    const still = name.endsWith("-still");
    const spec = [
      ...TERRAIN_LIT_MATERIALS,
      ...TERRAIN_DEPTH_MATERIALS,
      ...TERRAIN_SHADOW_MATERIALS,
    ].find((each) => each.name === name.replace(/-still$/u, ""));
    const source = spec === undefined ? undefined : still ? stillMaterialSpec(spec) : spec;
    return [name, source?.fragmentWgsl ?? "", digest];
  }),
];

/** The line `withDiscEmission` adds to `shade`. */
const EMISSION_LINE =
  "  out.radiance = out.radiance + disc_emission(body, normal, surface, laws);\n";

/** The emissive form undone: its added line and its appended modules taken out. */
function plainOf(emissive: string): string {
  const appended = `\n${THERMAL_WGSL}\n${discEmissionWgsl}`;
  expect(emissive.endsWith(appended)).toBe(true);
  const head = emissive.slice(0, -appended.length);
  expect(head.split(EMISSION_LINE)).toHaveLength(2);
  return head.replace(EMISSION_LINE, "");
}

describe("the disc's kernels (R08.T18.a)", () => {
  it.each(PLAIN_DIGESTS)("leaves %s as it was, to the byte", (_name, source, digest) => {
    expect(digestOf(source)).toBe(digest);
  });

  it.each([
    ["the disc's interior", BODY_DISC_MATERIALS.interior, BODY_DISC_EMISSIVE_MATERIALS.interior],
    ["the disc's limb", BODY_DISC_MATERIALS.limb, BODY_DISC_EMISSIVE_MATERIALS.limb],
    ["the mesh regime's figure", SMOOTH_MESH_MATERIAL, SMOOTH_MESH_EMISSIVE_MATERIAL],
  ] as const)(
    "makes %s's emissive form from its own source, one line added to shade and the emission appended, its states and bindings the same",
    (_name, plain, emissive) => {
      expect(plainOf(emissive.fragmentWgsl)).toBe(plain.fragmentWgsl);
      expect(emissive.vertexWgsl).toBe(emissive.fragmentWgsl);
      expect({
        ...emissive,
        name: plain.name,
        displayName: plain.displayName,
        vertexWgsl: plain.vertexWgsl,
        fragmentWgsl: plain.fragmentWgsl,
      }).toEqual(plain);
      expect(emissive.name).not.toBe(plain.name);
      expect(emissive.displayName).not.toBe(plain.displayName);
    },
  );

  it("makes the disc cells' emissive form so, declaring no binding of its own and reaching every one", () => {
    const emissive = BODY_DISC_CELLS_EMISSIVE_KERNEL.reference;
    expect(plainOf(emissive)).toBe(BODY_DISC_CELLS_KERNEL.reference);
    expect([...kernelBindings(emissive).keys()]).toEqual([
      ...kernelBindings(BODY_DISC_CELLS_KERNEL.reference).keys(),
    ]);
    // The engine's compute layouts are "auto": a binding the entry point never reaches drops out.
    const functions = wgslFunctions(emissive);
    const bodies = [...wgslReached(emissive, ["main"])].map(
      (name) => functions.get(name)?.body ?? "",
    );
    expect(wgslReached(emissive, ["main"]).has("disc_emission")).toBe(true);
    expect(wgslReached(emissive, ["main"]).has("thermal_luminance")).toBe(true);
    expect(
      [...kernelBindings(emissive).keys()].filter(
        (name) => !bodies.some((body) => new RegExp(`\\b${name}\\b`, "u").test(body)),
      ),
    ).toEqual([]);
  });

  it("refuses a source without shade's end, or with it twice", () => {
    expect(() => withDiscEmission("fn main() {}")).toThrow(/once, found 0/u);
    const twice = BODY_DISC_CELLS_KERNEL.reference + BODY_DISC_CELLS_KERNEL.reference;
    expect(() => withDiscEmission(twice)).toThrow(/once, found 2/u);
  });
});

describe("the disc's emission (R08.T18.a)", () => {
  it("computes nothing for a 300 K world: its record carries no emission and packs as before, byte for byte", () => {
    const field = groundFieldOf(warmAirlessSurface());
    const record = recordOf(field, 1, 60);
    expect(record.emission).toBeUndefined();
    expect(packDiscRecords([record])).toEqual(packDiscRecords([recordOf(null, 1, 60)]));
  });

  it("carries the locked lava world's emission 0.048 au from its star, its block past the records", () => {
    const record = recordOf(groundFieldOf(lavaWorldSurface()), 0.048, 60);
    const emission = record.emission;
    if (emission === undefined) {
      throw new Error("the lava world is above the cut");
    }
    const packed = packDiscRecords([record]);
    // Row 46's y: the block's first row, the record's 308 rows being the buffer's first.
    expect(packed[46 * 4 + 1]).toBe(308);
    expect(packed.length).toBe((308 + DISC_EMISSION_ROWS) * 4);
    expect([...packed.subarray(308 * 4)]).toEqual([...packDiscEmission(emission, EXPOSURE)]);
  });

  it("lights a young magma ocean's night side by itself alone: its radiance there is the emission term, its reflected light 0", () => {
    const field = groundFieldOf(magmaOceanSurface());
    // The star behind the body: the camera sees the night side face-on at the disc's centre.
    const record = recordOf(field, 0.048, 180);
    const emission = record.emission;
    if (emission === undefined) {
      throw new Error("the magma ocean is above the cut");
    }
    const q = vec3(0, 0, 1);
    const ray = vec3(0, 0, -1);
    const reflected = shadeDiscPoint(withoutEmission(record), CAMERA, q, ray);
    const drawn = shadeDiscPoint(record, CAMERA, q, ray);
    expect(reflected.radiance).toEqual([0, 0, 0]);
    expect(drawn.lit).toBe(false);
    // The term at the point: the night's temperature, the solidus, and the law's A.
    const lawA: Rgb = record.surface.kind === "uniform" ? record.surface.law.a : [0, 0, 0];
    const term = discEmissionAt(emission, EXPOSURE, q, vec3(0, 0, -1), vec3(0, 1, 0), [1], [lawA]);
    expect(fieldTemperatureK(field, -1, 0)).toBe(1_394);
    for (const c of [0, 1, 2] as const) {
      expect(Math.abs(drawn.radiance[c] - term[c])).toBeLessThanOrEqual(1e-12 * term[0]);
    }
    expect(term[0]).toBeGreaterThan(0);
    // Its luminance at the solidus, emissivity 1 − A: thousands of cd m⁻² before the exposure.
    expect(term[0] / EXPOSURE).toBeGreaterThan(1e3);
  });

  it("draws a locked magma ocean its star alone holds molten black on its night side, its solid ground some 26 K", () => {
    const field = groundFieldOf(heldMagmaOceanSurface());
    // The star behind the body, as above: its dayside's glow keeps the record above the cut.
    const record = recordOf(field, 0.048, 180);
    if (record.emission === undefined) {
      throw new Error("the held magma ocean is above the cut");
    }
    const drawn = shadeDiscPoint(record, CAMERA, vec3(0, 0, 1), vec3(0, 0, -1));
    expect(drawn.lit).toBe(false);
    expect(fieldTemperatureK(field, -1, 0)).toBeLessThan(30);
    expect(drawn.radiance).toEqual([0, 0, 0]);
    // Nor does a Melt class light it: the melt's least temperature is the night's.
    const melt = discEmissionAt(
      { field, star: vec3(0, 0, 1), meltClasses: [true] },
      1,
      vec3(0, 0, -1),
      vec3(0, 0, 1),
      vec3(0, 1, 0),
      [0, 1],
      [
        [0.1, 0.1, 0.1],
        [0.05, 0.05, 0.05],
      ],
    );
    expect(melt).toEqual([0, 0, 0]);
  });

  it("adds the emission to the dayside's reflected light, which it leaves as it was", () => {
    const field = groundFieldOf(lavaWorldSurface());
    const record = recordOf(field, 0.048, 0);
    const emission = record.emission;
    if (emission === undefined) {
      throw new Error("the lava world is above the cut");
    }
    const q = vec3(0, 0, 1);
    const ray = vec3(0, 0, -1);
    const reflected = shadeDiscPoint(withoutEmission(record), CAMERA, q, ray);
    const drawn = shadeDiscPoint(record, CAMERA, q, ray);
    expect(drawn.lit).toBe(reflected.lit);
    const lawA: Rgb = record.surface.kind === "uniform" ? record.surface.law.a : [0, 0, 0];
    // The substellar point faces the camera at zero phase: the eyeball's T_ss.
    const substellar = surfaceTemperatureFieldOf(lavaWorldSurface());
    const t = fieldTemperatureK(substellar, 1, 0);
    expect(t).toBeCloseTo(1_751.8, 0);
    const emitted = thermalLuminance(lawA, 1, t);
    for (const c of [0, 1, 2] as const) {
      const expected = reflected.radiance[c] + EXPOSURE * emitted[c];
      expect(Math.abs(drawn.radiance[c] - expected)).toBeLessThanOrEqual(1e-9 * expected);
    }
  });

  it("emits a Melt class at no less than its melt's temperature, the rest of the ground at the field's", () => {
    const field = groundFieldOf(lavaWorldSurface());
    const emission = { field, star: vec3(0, 0, 1), meltClasses: [true] };
    // On the night side the field is its 21 K floor; the melt class holds its transition.
    const at = (shares: ReadonlyArray<number>) =>
      discEmissionAt(emission, 1, vec3(0, 0, -1), vec3(0, 0, 1), vec3(0, 1, 0), shares, [
        [0.1, 0.1, 0.1],
        [0.05, 0.05, 0.05],
      ]);
    const meltK = field.meltK ?? 0;
    expect(at([1, 0])[0]).toBeLessThan(1e-20);
    const half = at([0.5, 0.5]);
    const expected = thermalLuminance([0.025, 0.025, 0.025], 0.5, meltK);
    for (const c of [0, 1, 2] as const) {
      expect(Math.abs(half[c] - expected[c])).toBeLessThanOrEqual(1e-12 * expected[0]);
    }
  });

  it("shades an emitting record's cells with the kernel's emissive form alone, and none without it", () => {
    // Two small discs of the same textures, which the pass shades whole: one emitting, one not.
    const plain = recordOf(null, 0.048, 60, 13);
    const emitting = recordOf(groundFieldOf(lavaWorldSurface()), 0.048, 60, 13);
    const without = discCellJobs([plain, emitting], undefined, null, false);
    expect(without.ranges[0]).not.toBeNull();
    expect(without.ranges[1]).toBeNull();
    expect(without.sets.map((set) => set.emissive)).toEqual([false]);
    const withForm = discCellJobs([plain, emitting], undefined, null, true);
    expect(withForm.sets.map((set) => [set.record, set.emissive])).toEqual([
      [0, false],
      [1, true],
    ]);
  });
});

describe("the lit bodies' renderer's emissive forms (R08.T18.a)", () => {
  it("makes none for a frame with no emitting record", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const materials = engine.counts.materials;
    renderer.draws(planOf(null, 1, 60));
    expect(engine.counts.materials).toBe(materials);
    renderer.dispose();
  });

  it("draws an emitting record as the plain forms do until its forms are prepared, then by them", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const field = groundFieldOf(lavaWorldSurface());
    const first = renderer.draws(planOf(field, 0.048, 60));
    expect(first.map((draw) => draw.material.name)).toEqual(["bodies:disc", "bodies:discLimb"]);
    // Its emission is dropped from the records written while the forms are not prepared.
    const written = engine.writes.findLast((write) => write.buffer === "bodies:discs");
    expect(new Float32Array(written?.data.buffer ?? new ArrayBuffer(0))[46 * 4 + 1]).toBe(0);
    await renderer.prepareEmissive();
    expect(engine.prepares.map((each) => each.name)).toEqual(
      expect.arrayContaining([
        "bodies:discEmissive",
        "bodies:discLimbEmissive",
        "bodies:smoothMeshEmissive",
      ]),
    );
    const next = renderer.draws(planOf(field, 0.048, 60));
    expect(next.map((draw) => draw.material.name)).toEqual([
      "bodies:discEmissive",
      "bodies:discLimbEmissive",
    ]);
    const rewritten = engine.writes.findLast((write) => write.buffer === "bodies:discs");
    expect(new Float32Array(rewritten?.data.buffer ?? new ArrayBuffer(0))[46 * 4 + 1]).toBe(308);
    renderer.dispose();
  });

  it("draws an emitting mesh body's figure and limb by the emissive forms", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    await renderer.prepareEmissive();
    const plan = planOf(groundFieldOf(magmaOceanSurface()), 0.048, 90, 96, true);
    expect(plan.regimes.get(plan.discs[0]?.body ?? "0200080020000000.0300")).toBe("mesh");
    // On the cluster path (plan R14, R14.T4.a): its stride-1 blocks' and strips' buckets.
    expect(renderer.meshDraws(plan).map((draw) => draw.material.name)).toEqual([
      "bodies:smoothMeshEmissiveCluster",
      "bodies:smoothMeshEmissiveCluster",
    ]);
    expect(renderer.draws(plan).map((draw) => draw.material.name)).toEqual([
      "bodies:discLimbEmissive",
    ]);
    renderer.dispose();
  });

  it("makes and prepares its emissive forms again after a device loss, at the next emitting record", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const field = groundFieldOf(lavaWorldSurface());
    await renderer.prepareEmissive();
    const prepared = (): number =>
      engine.prepares.filter((each) => each.name === "bodies:discEmissive").length;
    expect(prepared()).toBe(1);
    engine.loseDevice();
    engine.restore();
    // The restore's renderer holds none until an emitting record asks for them.
    renderer.draws(planOf(null, 0.048, 60));
    expect(prepared()).toBe(1);
    renderer.draws(planOf(field, 0.048, 60));
    await renderer.prepareEmissive();
    expect(prepared()).toBe(2);
    expect(renderer.draws(planOf(field, 0.048, 60)).map((draw) => draw.material.name)).toEqual([
      "bodies:discEmissive",
      "bodies:discLimbEmissive",
    ]);
    renderer.dispose();
  });

  it("reports emissive forms the device refuses to prepare, and draws on by the plain forms", async () => {
    const engine = await countingRenderEngine();
    const refusing: LitBodyEngine = {
      createMaterial: (...args) => engine.createMaterial(...args),
      createMesh: (...args) => engine.createMesh(...args),
      createBuffer: (...args) => engine.createBuffer(...args),
      createTexture: (...args) => engine.createTexture(...args),
      writeBuffer: (...args) => {
        engine.writeBuffer(...args);
      },
      writeTexture: (...args) => {
        engine.writeTexture(...args);
      },
      releaseBuffer: (...args) => {
        engine.releaseBuffer(...args);
      },
      releaseTexture: (...args) => {
        engine.releaseTexture(...args);
      },
      dispatch: (...args) => {
        engine.dispatch(...args);
      },
      createComputeAsync: (...args) => engine.createComputeAsync(...args),
      releaseCompute: (...args) => {
        engine.releaseCompute(...args);
      },
      onRestored: (...args) => engine.onRestored(...args),
      get capabilities() {
        return engine.capabilities;
      },
      prepareMaterial: (material, targets, meshes) =>
        material.name.endsWith("Emissive")
          ? Promise.reject(new Error(`${material.name} is refused`))
          : engine.prepareMaterial(material, targets, meshes),
    };
    const reported = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const renderer = new LitBodyRenderer(refusing, WIREFRAME_MATERIALS.starSprite);
    await renderer.prepareEmissive();
    expect(reported).toHaveBeenCalledWith(
      "the emissive forms of the lit bodies could not be prepared:",
      expect.any(Error),
    );
    const draws = renderer.draws(planOf(groundFieldOf(lavaWorldSurface()), 0.048, 60));
    expect(draws.map((draw) => draw.material.name)).toEqual(["bodies:disc", "bodies:discLimb"]);
    renderer.dispose();
  });

  it("shades an emitting small disc's cells with the kernel's emissive form, and leaves them to its draws without it", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    await renderer.prepareEmissive();
    const plain = await engine.createComputeAsync(BODY_DISC_CELLS_KERNEL);
    const emissive = await engine.createComputeAsync(BODY_DISC_CELLS_EMISSIVE_KERNEL);
    const field = groundFieldOf(lavaWorldSurface());
    expect(renderer.dispatchCells(planOf(field, 0.048, 60, 13), plain, FRAME, emissive)).toBe(true);
    expect(engine.dispatched.map((each) => each.kernel)).toEqual([
      BODY_DISC_CELLS_EMISSIVE_KERNEL.name,
    ]);
    const before = engine.dispatched.length;
    expect(renderer.dispatchCells(planOf(field, 0.048, 60, 13), plain, FRAME, null)).toBe(false);
    expect(engine.dispatched).toHaveLength(before);
    renderer.dispose();
  });
});
