import { describe, expect, it } from "vitest";

import { add, scale, vec3 } from "../../geometry/vec3";
import { wgslFunctions, wgslReached } from "../../test/atmosphereWgsl";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import {
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
  groundTemperatureFieldOf,
  type SurfaceTemperatureField,
  surfaceTemperatureFieldOf,
  THERMAL_WGSL,
  thermalRadiance,
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

/** The field of a surface whose ground this task draws. */
function groundOf(surface: ReturnType<typeof lavaWorldSurface>): SurfaceTemperatureField {
  const field = groundTemperatureFieldOf(surface);
  if (field === null) {
    throw new Error("the surface has a medium");
  }
  return field;
}

/** A kernel's source as one line-end form: a checkout's line ends do not move its digest. */
function digestOf(source: string): string {
  return fnv1a64(new TextEncoder().encode(source.replaceAll("\r\n", "\n"))).toString(16);
}

/**
 * The plain kernels' sources as they stood before R08.T18.a, by digest: the emissive forms are made
 * beside them and leave each to the byte (the brief's cut; R08.T11.f's cost of a branch not taken).
 */
const PLAIN_DIGESTS: ReadonlyArray<readonly [string, string, string]> = [
  ["the disc's interior", BODY_DISC_MATERIALS.interior.fragmentWgsl, "a5a28799a09284f8"],
  ["the disc's limb", BODY_DISC_MATERIALS.limb.fragmentWgsl, "a5a28799a09284f8"],
  ["the mesh regime's figure", SMOOTH_MESH_MATERIAL.fragmentWgsl, "123e93f990e998f9"],
  ["the disc cells", BODY_DISC_CELLS_KERNEL.reference, "58080e27ceb54f68"],
  ...[
    ["terrain-lit-baked-offsets", "ccf375ee96107a29"],
    ["terrain-lit-face-differences", "a95582e93db72e7c"],
    ["terrain-depth-baked-offsets", "7815d5cd63cc7d04"],
    ["terrain-depth-face-differences", "6aa9d333d78dc9ad"],
    ["terrain-shadow-baked-offsets", "bc5dc25267da523a"],
    ["terrain-shadow-face-differences", "687974744a8a3c4b"],
  ].map(([name = "", digest = ""]): readonly [string, string, string] => {
    const spec = [
      ...TERRAIN_LIT_MATERIALS,
      ...TERRAIN_DEPTH_MATERIALS,
      ...TERRAIN_SHADOW_MATERIALS,
    ].find((each) => each.name === name);
    return [name, spec?.fragmentWgsl ?? "", digest];
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
    expect(wgslReached(emissive, ["main"]).has("thermal_radiance")).toBe(true);
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
    const field = groundOf(warmAirlessSurface());
    const record = recordOf(field, 1, 60);
    expect(record.emission).toBeUndefined();
    expect(packDiscRecords([record])).toEqual(packDiscRecords([recordOf(null, 1, 60)]));
  });

  it("carries the locked lava world's emission 0.048 au from its star, its block past the records", () => {
    const record = recordOf(groundOf(lavaWorldSurface()), 0.048, 60);
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

  it("lights a lava world's night side by itself alone: its radiance there is the emission term, its reflected light 0", () => {
    const field = groundOf(magmaOceanSurface());
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

  it("adds the emission to the dayside's reflected light, which it leaves as it was", () => {
    const field = groundOf(lavaWorldSurface());
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
    const emitted = thermalRadiance(lawA, 1, t);
    for (const c of [0, 1, 2] as const) {
      const expected = reflected.radiance[c] + EXPOSURE * emitted[c];
      expect(Math.abs(drawn.radiance[c] - expected)).toBeLessThanOrEqual(1e-9 * expected);
    }
  });

  it("emits a Melt class at no less than its melt's temperature, the rest of the ground at the field's", () => {
    const field = groundOf(lavaWorldSurface());
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
    const expected = thermalRadiance([0.025, 0.025, 0.025], 0.5, meltK);
    for (const c of [0, 1, 2] as const) {
      expect(Math.abs(half[c] - expected[c])).toBeLessThanOrEqual(1e-12 * expected[0]);
    }
  });

  it("shades an emitting record's cells with the kernel's emissive form alone, and none without it", () => {
    // Two small discs of the same textures, which the pass shades whole: one emitting, one not.
    const plain = recordOf(null, 0.048, 60, 13);
    const emitting = recordOf(groundOf(lavaWorldSurface()), 0.048, 60, 13);
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
    const field = groundOf(lavaWorldSurface());
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
    const plan = planOf(groundOf(magmaOceanSurface()), 0.048, 90, 96, true);
    expect(plan.regimes.get(plan.discs[0]?.body ?? "0200080020000000.0300")).toBe("mesh");
    expect(renderer.meshDraws(plan).map((draw) => draw.material.name)).toEqual([
      "bodies:smoothMeshEmissive",
    ]);
    expect(renderer.draws(plan).map((draw) => draw.material.name)).toEqual([
      "bodies:discLimbEmissive",
    ]);
    renderer.dispose();
  });

  it("shades an emitting small disc's cells with the kernel's emissive form, and leaves them to its draws without it", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    await renderer.prepareEmissive();
    const plain = await engine.createComputeAsync(BODY_DISC_CELLS_KERNEL);
    const emissive = await engine.createComputeAsync(BODY_DISC_CELLS_EMISSIVE_KERNEL);
    const field = groundOf(lavaWorldSurface());
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
