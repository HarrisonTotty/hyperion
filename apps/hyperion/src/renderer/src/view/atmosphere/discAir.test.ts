import { describe, expect, it } from "vitest";

import { add, dot, normalise, scale, vec3 } from "../../geometry/vec3";
import { wgslFunctions, wgslReached } from "../../test/atmosphereWgsl";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import {
  type DiscRecord,
  discCellJobs,
  packDiscRecords,
  shadeDiscPoint,
} from "../bodies/discShading";
import {
  BODY_DISC_AIR_MATERIALS,
  BODY_DISC_CELLS_AIR_KERNEL,
  BODY_DISC_CELLS_KERNEL,
  BODY_DISC_MATERIALS,
  type BodyFramePlan,
  type DiscCellFrame,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
  SMOOTH_MESH_AIR_MATERIAL,
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
import { AU_M } from "../scenes/kept";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import { type DiscAir, DISC_AIR_ROWS, packDiscAir, withDiscAir } from "./discReflectance";
import {
  DISC_AZIMUTHS,
  DISC_LAYER_VALUES,
  DISC_SUN_COSINES,
  DISC_VIEW_COSINES,
  discAzimuthOf,
  type DiscReflectanceTable,
  discTableAt,
  discTableValuesOf,
  discTexelIndex,
} from "./discTable";
import discAirWgsl from "./shaders/discAir.wgsl?raw";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, R05_EARTH_REFERENCE } from "./earth";
import { AtmosphereTables } from "./tables";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const RAD = Math.PI / 180;
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
const EXPOSURE = 1e-3;

/** A table of one band and slot whose texels follow a smooth pattern, as halves. */
function patternedTable(): DiscReflectanceTable {
  const values = new Float32Array(DISC_LAYER_VALUES);
  for (let s = 0; s < DISC_SUN_COSINES; s += 1) {
    for (let v = 0; v < DISC_VIEW_COSINES; v += 1) {
      for (let k = 0; k < DISC_AZIMUTHS; k += 1) {
        const at = discTexelIndex(0, s, v, k);
        for (let c = 0; c < 3; c += 1) {
          values[at + c] = 0.02 + 0.01 * (1 + Math.sin(0.3 * s + 0.7 * v + 0.5 * k + c));
        }
        values[at + 3] = 0.5 + (0.4 * v) / 15;
      }
    }
  }
  return { bands: 1, slots: 1, sunFloor: -0.18, values: discTableValuesOf(values) };
}

const AIR: DiscAir = {
  table: patternedTable(),
  texture: { kind: "texture", name: "disc table" },
  lnBands: [0],
  gravity: null,
};

/** A body with air `px` across beyond the boundary, lit by a Sun 1 au off at phase `phaseDeg`. */
function planOf(phaseDeg: number, px = 200, air: DiscAir | null = AIR): BodyFramePlan {
  const distanceM = 2e9;
  const radiusM = distanceM * Math.sin(px / 2 / PX_PER_RAD);
  const centreM = vec3(0, 0, -distanceM);
  const towardsStar = vec3(Math.sin(phaseDeg * RAD), 0, Math.cos(phaseDeg * RAD));
  const lit = aLitBody();
  const body: LitBodyInput = {
    id: lit.body,
    centreM,
    figure: { equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole: vec3(0, 1, 0) },
    photometry: lit.photometry,
    lighting: undefined,
    ...(air === null ? {} : { air }),
  };
  const hosts: PlacedLight[] = [
    { disc: aHostDisc(), centreM: add(centreM, scale(towardsStar, AU_M)) },
  ];
  return planLitBodies(
    [body],
    hosts,
    {
      camera: CAMERA,
      viewport: VIEWPORT,
      presentedPx: VIEWPORT,
      exposureScale: EXPOSURE,
      annuli: 4,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
      drawsAir: true,
    },
    new Map([[body.id, "disc"]]),
  );
}

/** {@link planOf}'s record. */
function recordOf(phaseDeg: number, px = 200, air: DiscAir | null = AIR): DiscRecord {
  const record = planOf(phaseDeg, px, air).discs[0];
  if (record === undefined) {
    throw new Error("the body was not drawn as a disc");
  }
  return record;
}

/** The line `withDiscAir` puts at `shade`'s end. */
const AIR_LINE = "  out.radiance = disc_air(body, q, ray, out.radiance);\n";

/** The air form undone: its added line and its appended module taken out. */
function plainOf(aired: string): string {
  const appended = `\n${discAirWgsl}`;
  expect(aired.endsWith(appended)).toBe(true);
  const head = aired.slice(0, -appended.length);
  expect(head.split(AIR_LINE)).toHaveLength(2);
  return head.replace(AIR_LINE, "");
}

const FRAME: DiscCellFrame = {
  viewRotation: viewRotation4(CAMERA.orientation),
  projection: perspectiveReversedInfinite(
    CAMERA.fovXRad,
    VIEWPORT.widthPx / VIEWPORT.heightPx,
    NEAR_PLANE_M,
  ),
  size: VIEWPORT,
};

describe("the disc's kernels' air forms (R08.T16.b)", () => {
  it.each([
    ["the disc's interior", BODY_DISC_MATERIALS.interior, BODY_DISC_AIR_MATERIALS.interior],
    ["the disc's limb", BODY_DISC_MATERIALS.limb, BODY_DISC_AIR_MATERIALS.limb],
    ["the mesh regime's figure", SMOOTH_MESH_MATERIAL, SMOOTH_MESH_AIR_MATERIAL],
  ] as const)(
    "makes %s's air form from its own source, shade's light handed to disc_air and the read appended, the table at binding 15",
    (_name, plain, aired) => {
      expect(plainOf(aired.fragmentWgsl)).toBe(plain.fragmentWgsl);
      expect(aired.vertexWgsl).toBe(aired.fragmentWgsl);
      const table = (aired.textures ?? []).find((texture) => texture.binding === 15);
      expect(table).toEqual({
        name: "discReflectance",
        binding: 15,
        sampleType: "unfilterable-float",
        viewDimension: "2d-array",
      });
      expect({
        ...aired,
        name: plain.name,
        displayName: plain.displayName,
        vertexWgsl: plain.vertexWgsl,
        fragmentWgsl: plain.fragmentWgsl,
        textures: (aired.textures ?? []).filter((texture) => texture.binding !== 15),
      }).toEqual(plain);
      // The plain form's layout has no binding 15: its bind group is as it was.
      expect((plain.textures ?? []).some((texture) => texture.binding === 15)).toBe(false);
    },
  );

  it("makes the disc cells' air form so, its one binding more the table, reaching every one", () => {
    const aired = BODY_DISC_CELLS_AIR_KERNEL.reference;
    expect(plainOf(aired)).toBe(BODY_DISC_CELLS_KERNEL.reference);
    expect([...kernelBindings(aired).keys()]).toEqual([
      ...kernelBindings(BODY_DISC_CELLS_KERNEL.reference).keys(),
      "disc_reflectance",
    ]);
    const functions = wgslFunctions(aired);
    const reached = wgslReached(aired, ["main"]);
    const bodies = [...reached].map((name) => functions.get(name)?.body ?? "");
    expect(reached.has("disc_air")).toBe(true);
    expect(
      [...kernelBindings(aired).keys()].filter(
        (name) => !bodies.some((body) => new RegExp(`\\b${name}\\b`, "u").test(body)),
      ),
    ).toEqual([]);
  });

  it("refuses a source without shade's end, or with it twice", () => {
    expect(() => withDiscAir("fn main() {}")).toThrow(/once, found 0/u);
    const twice = BODY_DISC_CELLS_KERNEL.reference + BODY_DISC_CELLS_KERNEL.reference;
    expect(() => withDiscAir(twice)).toThrow(/once, found 2/u);
  });
});

describe("the disc's air in its twin (R08.T16.b)", () => {
  it("dims a sample's ground by the march's mean transmittance and adds its star's air", () => {
    const aired = recordOf(60);
    const { air: _air, ...plain } = aired;
    const q = normalise(vec3(0.3, 0.2, 1));
    const ray = normalise(vec3(-0.01, -0.005, -1));
    const ground = shadeDiscPoint(plain, CAMERA, q, ray).radiance;
    const got = shadeDiscPoint(aired, CAMERA, q, ray).radiance;
    const light = aired.lights[0];
    if (light === undefined) {
      throw new Error("the record has its star");
    }
    const towards = normalise(
      vec3(
        light.direction.x * light.distance - q.x,
        light.direction.y * light.distance - q.y,
        light.direction.z * light.distance - q.z,
      ),
    );
    const toCamera = scale(ray, -1);
    const muView = dot(q, toCamera);
    const through = discTableAt(AIR.table, [0], 1, 0, muView, 1, 0)[3];
    const lit = discTableAt(
      AIR.table,
      [0],
      1,
      0,
      muView,
      dot(q, towards),
      discAzimuthOf(q, toCamera, towards),
    );
    for (const c of [0, 1, 2] as const) {
      const want = ground[c] * through + light.illuminance[c] * aired.exposureOverPi * lit[c];
      expect(got[c] / want - 1).toBeCloseTo(0, 12);
    }
  });

  it("lights no air on the night side past the table's floor, and dims its ground alone", () => {
    const aired = recordOf(180);
    const { air: _air, ...plain } = aired;
    // The disc's centre faces away from the star, μ₀ = −1 there.
    const q = vec3(0, 0, 1);
    const ray = vec3(0, 0, -1);
    const ground = shadeDiscPoint(plain, CAMERA, q, ray).radiance;
    const got = shadeDiscPoint(aired, CAMERA, q, ray).radiance;
    const through = discTableAt(AIR.table, [0], 1, 0, 1, 1, 0)[3];
    for (const c of [0, 1, 2] as const) {
      expect(got[c]).toBeCloseTo(ground[c] * through, 15);
    }
  });

  it("packs a record's air block past the records, row 46's z its first, and none for one without", () => {
    const aired = recordOf(30);
    const { air: _air, ...plain } = aired;
    const packed = packDiscRecords([plain, aired]);
    const rows = packed.length / 4;
    const discRows = 2 * 308;
    expect(rows).toBe(discRows + DISC_AIR_ROWS);
    expect(packed[46 * 4 + 2]).toBe(0);
    expect(packed[(308 + 46) * 4 + 2]).toBe(discRows);
    expect(Array.from(packed.subarray(discRows * 4))).toEqual(
      Array.from(packDiscAir(AIR)).map((v) => Math.fround(v)),
    );
    // A plan without air packs as it did: no block, row 46's z 0.
    const bare = packDiscRecords([plain]);
    expect([bare.length / 4, bare[46 * 4 + 2]]).toEqual([308, 0]);
  });

  it("shades a record with air's cells with the kernel's air form alone, and none without it", () => {
    const record = recordOf(30, 13);
    expect(discCellJobs([record], 2 ** 24, FRAME, false, true).sets.map((set) => set.air)).toEqual([
      true,
    ]);
    expect(discCellJobs([record], 2 ** 24, FRAME, false, false).count).toBe(0);
  });
});

describe("the lit bodies' renderer's air forms (R08.T16.b)", () => {
  it("draws a record with air by the plain forms until its forms are prepared, then by them with its table", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    expect(renderer.airReady()).toBe(false);
    const early = renderer.draws(planOf(30));
    expect(early.map((draw) => draw.material.name)).toEqual(["bodies:disc", "bodies:discLimb"]);
    await renderer.prepareAir();
    expect(renderer.airReady()).toBe(true);
    const draws = renderer.draws(planOf(30));
    expect(draws.map((draw) => draw.material.name)).toEqual([
      "bodies:discAir",
      "bodies:discLimbAir",
    ]);
    expect(draws[0]?.textures["discReflectance"]).toBe(AIR.texture);
    renderer.dispose();
  });

  it("shades a small disc's cells with the kernel's air form, binding its table, and leaves them to its draws without it", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    await renderer.prepareAir();
    const plain = await engine.createComputeAsync(BODY_DISC_CELLS_KERNEL);
    const aired = await engine.createComputeAsync(BODY_DISC_CELLS_AIR_KERNEL);
    expect(renderer.dispatchCells(planOf(30, 13), plain, FRAME, null, aired)).toBe(true);
    expect(engine.dispatched.map((each) => each.kernel)).toEqual([BODY_DISC_CELLS_AIR_KERNEL.name]);
    expect(engine.dispatched[0]?.bindings.sampled["disc_reflectance"]).toBe(AIR.texture);
    const before = engine.dispatched.length;
    expect(renderer.dispatchCells(planOf(30, 13), plain, FRAME, null, null)).toBe(false);
    expect(engine.dispatched).toHaveLength(before);
    // A record with no air binds no table in a plain set.
    expect(renderer.dispatchCells(planOf(30, 13, null), plain, FRAME, null, aired)).toBe(true);
    expect(engine.dispatched.at(-1)?.bindings.sampled["disc_reflectance"]).toBeUndefined();
    renderer.dispose();
  });
});

/** One-band tables over a counting engine, with no disc table yet. */
async function earthTables(): Promise<{
  readonly engine: CountingRenderEngine;
  readonly tables: AtmosphereTables;
}> {
  const engine = await countingRenderEngine();
  return { engine, tables: new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6.371e6) };
}

describe("the atmosphere's tables' disc table (R08.T16.b)", () => {
  it("uploads it as halves of its layers and counts its bytes in the row's", async () => {
    const { engine, tables } = await earthTables();
    const base = tables.rowBytes;
    expect([tables.discReflectance, tables.discBytes]).toEqual([null, 0]);
    tables.setDiscReflectance(AIR.table);
    const held = tables.discReflectance;
    const write = engine.textureWritten.at(-1);
    expect([
      held?.table,
      tables.discBytes,
      tables.rowBytes - base,
      write?.texture,
      write?.size,
      engine.textureSpecs.at(-1)?.format,
    ]).toEqual([AIR.table, 53_248, 53_248, held?.texture.name, [208, 32, 1], "rgba16float"]);
    tables.dispose();
  });

  it.each([
    [
      "of other bands",
      { ...AIR.table, bands: 2, values: new Float32Array(2 * DISC_LAYER_VALUES) },
      /2 bands is not the tables' 1/u,
    ],
    [
      "whose values are not its layers'",
      { ...AIR.table, slots: 2 },
      /26624 values is not 2 layers of 26624/u,
    ],
  ] as const)("refuses a table %s", async (_name, table, message) => {
    const { tables } = await earthTables();
    expect(() => tables.setDiscReflectance(table)).toThrow(message);
    expect(tables.discReflectance).toBeNull();
    tables.dispose();
  });

  it("makes it again after a loss", async () => {
    const { engine, tables } = await earthTables();
    tables.setDiscReflectance(AIR.table);
    const before = tables.discReflectance?.texture;
    engine.restore();
    expect([tables.discReflectance?.table, tables.discReflectance?.texture === before]).toEqual([
      AIR.table,
      false,
    ]);
    tables.dispose();
  });

  it("takes it away with the medium", async () => {
    const { tables } = await earthTables();
    tables.setDiscReflectance(AIR.table);
    tables.setMedium(EARTH_REFERENCE, 6.371e6, EARTH_REFERENCE_CURVES, null);
    expect(tables.discReflectance).toBeNull();
    tables.dispose();
  });
});
