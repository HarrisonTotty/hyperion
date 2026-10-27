import { describe, expect, it } from "vitest";

import { scale, vec3 } from "../../geometry/vec3";
import {
  type CountingRenderEngine,
  countingRenderEngine,
  type RecordedDispatch,
  type RecordedWrite,
} from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { wgslFunctions } from "../../test/atmosphereWgsl";
import { datumNormalMap, unreadNormalMap } from "../../test/normalMapFixtures";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { SurfaceLightingTables } from "../atmosphere/surfaceLighting";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  type Viewport,
  viewRotation4,
} from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { packFrame } from "../engine/webgpu/uniforms";
import type { DrawItem, TextureHandle } from "../engine/types";
import type { PlacedLight } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import { AU_M } from "../scenes/kept";
import type { ScreenRect } from "../wireframe/submit";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import {
  type DiscCellSet,
  type DiscRecord,
  DISC_ROWS,
  discCellJobs,
  drawnThroughCells,
  MAX_CELL_JOBS,
  packDiscRecords,
} from "./discShading";
import { packClassMap } from "./discSurface";
import {
  BODY_DISC_CELLS_KERNEL,
  BODY_DISC_MATERIALS,
  type BodyFramePlan,
  cellWorkgroups,
  DISC_CELLS_PASS,
  type DiscCellFrame,
  discCellFrameBlock,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
  SMOOTH_MESH_MATERIAL,
} from "./draw";
import { sphereFootprint } from "./regime";

const VIEWPORT: Viewport = { widthPx: 320, heightPx: 180 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
const RADIUS_M = 6.371e6;

/** The `discs` pass's frame for {@link VIEWPORT}. */
const FRAME: DiscCellFrame = {
  viewRotation: viewRotation4(CAMERA.orientation),
  projection: perspectiveReversedInfinite(
    CAMERA.fovXRad,
    VIEWPORT.widthPx / VIEWPORT.heightPx,
    NEAR_PLANE_M,
  ),
  size: VIEWPORT,
};

/**
 * Earth-sized bodies `diameterPx` across, side by side across the view, lit at 60° by a Sun 1 au
 * away; and that Sun.
 */
function bodiesOf(diameters: ReadonlyArray<number>): {
  readonly bodies: LitBodyInput[];
  readonly hosts: PlacedLight[];
} {
  const lit = aLitBody();
  const bodies = diameters.map((diameterPx, i): LitBodyInput => {
    const distanceM = RADIUS_M / Math.sin(diameterPx / 2 / PX_PER_RAD);
    const xPx = (i - (diameters.length - 1) / 2) * 70;
    return {
      id: `0200080020000000.${(0x100 + i).toString(16).padStart(4, "0")}`,
      centreM: vec3((xPx * distanceM) / PX_PER_RAD, 0, -distanceM),
      figure: lit.figure,
      photometry: PROVISIONAL_PHOTOMETRY,
      lighting: undefined,
    };
  });
  const towards = vec3(Math.sin(Math.PI / 3), 0, Math.cos(Math.PI / 3));
  return { bodies, hosts: [{ disc: aHostDisc(), centreM: scale(towards, AU_M) }] };
}

const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  presentedPx: VIEWPORT,
  exposureScale: 1e-3,
  annuli: 4,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
};

/** A frame's plan of discs `diameterPx` across, each held to the disc regime. */
function planOf(diameters: ReadonlyArray<number>): BodyFramePlan {
  const { bodies, hosts } = bodiesOf(diameters);
  return planLitBodies(bodies, hosts, OPTIONS, new Map(bodies.map((b) => [b.id, "disc"])));
}

/** A plan's records, held to have as many as `count`. */
function recordsOf(plan: BodyFramePlan, count: number): ReadonlyArray<DiscRecord> {
  if (plan.discs.length !== count) {
    throw new Error(`the plan drew ${plan.discs.length} discs, not ${count}`);
  }
  return plan.discs;
}

/** The pixels of [⌊left⌋, ⌈right⌉) × [⌊top⌋, ⌈bottom⌉) in row order. */
function pixelsOf(rect: ScreenRect): Array<readonly [number, number]> {
  const pixels: Array<readonly [number, number]> = [];
  for (let y = Math.floor(rect.topPx); y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = Math.floor(rect.leftPx); x < Math.ceil(rect.rightPx); x += 1) {
      pixels.push([x, y]);
    }
  }
  return pixels;
}

/** A packed buffer's rows of four. */
function rowsOf(values: ArrayLike<number>): number[][] {
  const rows: number[][] = [];
  for (let i = 0; i < values.length; i += 4) {
    rows.push(Array.from({ length: 4 }, (_, k) => values[i + k] ?? Number.NaN));
  }
  return rows;
}

/** A rectangle's width over its whole pixels, ⌈right⌉ − ⌊left⌋. */
function widthOf(rect: ScreenRect): number {
  return Math.ceil(rect.rightPx) - Math.floor(rect.leftPx);
}

/** The cell sums each draw binds, by name. */
function sumsBound(draws: ReadonlyArray<DrawItem>): Array<string | undefined> {
  return draws.map((draw) => draw.storageBuffers?.["cellSums"]?.name);
}

/** The engine's one dispatch. */
function onlyDispatch(engine: CountingRenderEngine): RecordedDispatch {
  const [dispatch, ...rest] = engine.dispatched;
  if (dispatch === undefined || rest.length > 0) {
    throw new Error(`expected one dispatch, got ${engine.dispatched.length}`);
  }
  return dispatch;
}

/** The last write to the buffer named `name`. */
function lastWrite(engine: CountingRenderEngine, name: string): RecordedWrite {
  const written = engine.writes.findLast((w) => w.buffer === name);
  if (written === undefined) {
    throw new Error(`nothing was written to ${name}`);
  }
  return written;
}

/** A fresh renderer's dispatch for a plan of discs `diameterPx` across, on a counting engine. */
async function dispatchedFor(diameters: ReadonlyArray<number>): Promise<{
  readonly engine: CountingRenderEngine;
  readonly plan: BodyFramePlan;
}> {
  const engine = await countingRenderEngine();
  const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
  const plan = planOf(diameters);
  renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
  renderer.dispose();
  return { engine, plan };
}

/** Row 51 of each record in a packed `discs` buffer. */
function cellRows(packed: Float32Array, count: number): number[][] {
  return Array.from({ length: count }, (_, index) =>
    Array.from(packed.subarray((index * DISC_ROWS + 51) * 4, (index * DISC_ROWS + 52) * 4)),
  );
}

describe("the cell pass's jobs (R07.T8.d)", () => {
  it("lays one job a pixel of each small disc's rectangle, the records in order, each in rows", () => {
    const records = recordsOf(planOf([3.5, 13]), 2);
    const cells = discCellJobs(records);
    const expected = records.flatMap((record, index) =>
      pixelsOf(record.rect).map(([x, y]) => [index, x, y, 0]),
    );
    expect([cells.count, rowsOf(cells.jobs)]).toEqual([expected.length, expected]);
  });

  it("carries each small disc's first sum, left, top and width in row 51", () => {
    const records = recordsOf(planOf([3.5, 13]), 2);
    const cells = discCellJobs(records);
    const [first, second] = records.map((record) => record.rect);
    if (first === undefined || second === undefined) {
      throw new Error("no rectangles");
    }
    expect(cellRows(packDiscRecords(records, cells), 2)).toEqual([
      [0, Math.floor(first.leftPx), Math.floor(first.topPx), widthOf(first)],
      [
        pixelsOf(first).length,
        Math.floor(second.leftPx),
        Math.floor(second.topPx),
        widthOf(second),
      ],
    ]);
  });

  it("takes no job of a disc of 32 px or more, whose row 51 says it sums its own cells", () => {
    const records = recordsOf(planOf([40, 13]), 2);
    const cells = discCellJobs(records);
    const rows = cellRows(packDiscRecords(records, cells), 2);
    // Each record's cells inside, whether it has a range, and its row 51's x.
    expect(
      records
        .map((record, i) => [record.interiorSamples, cells.ranges[i] === null, rows[i]?.[0]])
        .toSorted((a, b) => Number(a[0]) - Number(b[0])),
    ).toEqual([
      [1, true, -1],
      [4, false, 0],
    ]);
  });

  it("writes −1 in every row 51 where no pass runs", () => {
    const records = recordsOf(planOf([3.5, 13]), 2);
    expect(cellRows(packDiscRecords(records), 2)).toEqual([
      [-1, 0, 0, 0],
      [-1, 0, 0, 0],
    ]);
  });

  it("leaves a disc whose pixels would pass the jobs' limit to sum its own cells", () => {
    const records = recordsOf(planOf([3.5, 13]), 2);
    const [head] = records;
    if (head === undefined) {
      throw new Error("no disc");
    }
    const first = pixelsOf(head.rect);
    const cells = discCellJobs(records, first.length);
    expect([cells.count, cells.ranges.map((range) => range === null)]).toEqual([
      first.length,
      [false, true],
    ]);
  });

  it("takes no disc whose pixels pass 2^24, which row 51's f32 cannot index", () => {
    const [record] = recordsOf(planOf([3.5]), 1);
    if (record === undefined) {
      throw new Error("no disc");
    }
    const huge: DiscRecord = {
      ...record,
      rect: { leftPx: 0, topPx: 0, rightPx: 4097, bottomPx: 4096 },
    };
    const cells = discCellJobs([huge]);
    expect([MAX_CELL_JOBS, cells.count, cells.ranges]).toEqual([2 ** 24, 0, [null]]);
  });

  it("refuses jobs made for another number of records", () => {
    const records = recordsOf(planOf([3.5, 13]), 2);
    expect(() => packDiscRecords(records.slice(1), discCellJobs(records))).toThrow(
      /for 2 records, not 1/,
    );
  });

  it("takes every pixel of the 8 × 8 and 4 × 4 discs, a class map's among them since R07.T8.e, not the larger rule's", () => {
    const [record] = recordsOf(planOf([13]), 1);
    if (record === undefined) {
      throw new Error("no disc");
    }
    const law = PROVISIONAL_PHOTOMETRY.law;
    const mapped: DiscRecord = {
      ...record,
      surface: {
        kind: "class-map",
        weights: { kind: "texture", name: "test class map" },
        texels: packClassMap(1, 1, () => [1]),
        laws: [law],
        elsewhere: law,
        normals: null,
        rotation: IDENTITY_ROTATION,
      },
    };
    const counts = (interior: number, limb: number): DiscRecord => ({
      ...record,
      interiorSamples: interior,
      limbSamples: limb,
    });
    expect(
      [counts(8, 8), counts(4, 4), counts(1, 4), counts(16, 16), mapped].map(drawnThroughCells),
    ).toEqual([true, true, false, false, true]);
  });
});

describe("the cell pass's dispatch (R07.T8.d)", () => {
  it("lays its workgroups along x to 65,535, then in rows", () => {
    expect([1, 65_535, 65_536, 200_000].map(cellWorkgroups)).toEqual([
      [1, 1, 1],
      [65_535, 1, 1],
      [65_535, 2, 1],
      [65_535, 4, 1],
    ]);
  });

  it("packs the discs pass's frame as the engine packs a submission's", () => {
    const submission = {
      label: "discs",
      viewRotation: FRAME.viewRotation,
      projection: FRAME.projection,
      draws: [],
      postProcesses: [],
    };
    expect(Array.from(discCellFrameBlock(FRAME))).toEqual(
      Array.from(packFrame(submission, VIEWPORT)),
    );
  });

  it("dispatches once for a 3.5 px and a 13 px disc, one job a pixel, under its label", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const plan = planOf([3.5, 13]);
    const jobs = discCellJobs(plan.discs);
    expect(renderer.dispatchCells(plan, kernel, FRAME)).toBe(true);
    expect(
      engine.dispatched.map((d) => [
        d.kernel,
        d.pass,
        d.workgroups,
        Array.from(d.bindings.uniforms["cell_pass"] ?? []),
      ]),
    ).toEqual([
      [BODY_DISC_CELLS_KERNEL.name, DISC_CELLS_PASS, [jobs.count, 1, 1], [jobs.count, 0, 0, 0]],
    ]);
    renderer.dispose();
  });

  it("binds the records, its jobs and room for every pixel's sums", async () => {
    const { engine, plan } = await dispatchedFor([3.5, 13]);
    const buffers = onlyDispatch(engine).bindings.buffers;
    expect([
      buffers["discs"]?.name,
      buffers["cell_jobs"]?.name,
      buffers["cell_sums"]?.name,
      (buffers["cell_sums"]?.bytes ?? 0) >= 32 * discCellJobs(plan.discs).count,
    ]).toEqual(["bodies:discs", "bodies:disc cell jobs", "bodies:disc cell sums", true]);
  });

  it("binds the texel of no weight as the class map and normal map its kernel reaches for uniform discs, which none of them reads (R10.T10.g)", async () => {
    const { engine } = await dispatchedFor([3.5, 13]);
    const { sampled } = onlyDispatch(engine).bindings;
    expect([sampled["class_weights"]?.name, sampled["surface_normals"]?.name]).toEqual([
      "bodies:no class map",
      "bodies:no class map",
    ]);
  });

  it("writes the plan's jobs into the buffer it binds", async () => {
    const { engine, plan } = await dispatchedFor([3.5, 13]);
    const written = lastWrite(engine, "bodies:disc cell jobs");
    expect(Array.from(new Int32Array(written.data.buffer))).toEqual(
      Array.from(discCellJobs(plan.discs).jobs),
    );
  });

  it("shades in the discs pass's own frame", async () => {
    const { engine } = await dispatchedFor([3.5, 13]);
    expect(Array.from(onlyDispatch(engine).bindings.uniforms["frame"] ?? [])).toEqual(
      Array.from(discCellFrameBlock(FRAME)),
    );
  });

  it("writes the records with each small disc's sums in row 51", async () => {
    const { engine, plan } = await dispatchedFor([3.5, 13]);
    const packed = new Float32Array(lastWrite(engine, "bodies:discs").data.buffer);
    expect(cellRows(packed, 2)).toEqual(
      cellRows(packDiscRecords(plan.discs, discCellJobs(plan.discs)), 2),
    );
  });

  it("dispatches nothing where every disc is 32 px or more, and writes −1 in their row 51", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const plan = planOf([40, 64]);
    expect(renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME)).toBe(
      false,
    );
    const packed = new Float32Array(lastWrite(engine, "bodies:discs").data.buffer);
    expect([engine.dispatched.length, cellRows(packed, 2)]).toEqual([
      0,
      [
        [-1, 0, 0, 0],
        [-1, 0, 0, 0],
      ],
    ]);
    renderer.dispose();
  });

  it("leaves a plan whose draws were made first to sum its own cells", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const plan = planOf([3.5, 13]);
    renderer.draws(plan);
    expect([
      renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME),
      engine.dispatched.length,
    ]).toEqual([false, 0]);
    renderer.dispose();
  });

  it("grows the sums by doubling", async () => {
    const engine = await countingRenderEngine();
    const created: Array<readonly [string, number]> = [];
    engine.onAllocation((event) => {
      if (event.kind === "created" && event.name === "bodies:disc cell sums") {
        created.push([event.kind, event.bytes]);
      }
    });
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    // A 24 px disc's rectangle holds about 900 pixels, 32 bytes each: past the first 4,096 bytes.
    const plan = planOf([24]);
    renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
    const bytes = 32 * discCellJobs(plan.discs).count;
    const grown = 4_096 * 2 ** Math.ceil(Math.log2(bytes / 4_096));
    expect(created).toEqual([
      ["created", 4_096],
      ["created", grown],
    ]);
    renderer.dispose();
  });

  it("binds the sums to every disc draw and mesh figure", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const { bodies, hosts } = bodiesOf([13, 20]);
    const [promoted] = bodies;
    if (promoted === undefined) {
      throw new Error("no body");
    }
    const writer = sphereFootprint(
      promoted.centreM,
      promoted.figure.equatorialRadiusM,
      CAMERA,
      VIEWPORT,
    );
    const plan = planLitBodies(
      bodies,
      hosts,
      { ...OPTIONS, depthWriters: writer === null ? [] : [writer] },
      new Map(bodies.map((b) => [b.id, "disc"])),
    );
    renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
    const meshes = renderer.meshDraws(plan);
    expect([meshes.length, plan.regimes.get(promoted.id)]).toEqual([1, "mesh"]);
    expect(new Set([...sumsBound(meshes), ...sumsBound(renderer.draws(plan))])).toEqual(
      new Set(["bodies:disc cell sums"]),
    );
    renderer.dispose();
  });

  it("makes its buffers again after a device loss, and binds the new ones", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    renderer.dispatchCells(planOf([13]), kernel, FRAME);
    const created: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "created") {
        created.push(event.name);
      }
    });
    engine.restore();
    renderer.dispatchCells(planOf([13]), engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
    const [before, after] = engine.dispatched.map((d) => d.bindings.buffers);
    expect(created).toEqual(
      expect.arrayContaining(["bodies:disc cell jobs", "bodies:disc cell sums"]),
    );
    expect([
      before?.["cell_jobs"] === after?.["cell_jobs"],
      before?.["cell_sums"] === after?.["cell_sums"],
    ]).toEqual([false, false]);
    renderer.dispose();
  });

  it("releases its buffers with the renderer", async () => {
    const engine = await countingRenderEngine();
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    renderer.dispatchCells(planOf([24]), engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
    const grown = released.length;
    renderer.dispose();
    expect(released.slice(grown)).toEqual(
      expect.arrayContaining(["bodies:disc cell jobs", "bodies:disc cell sums"]),
    );
  });
});

/** A stand-in texture handle, named. */
function texture(name: string): TextureHandle {
  return { kind: "texture", name };
}

/** A body's atmosphere tables as stand-ins, each named after the body's `name` (R08.T9.b). */
function airTables(name: string): SurfaceLightingTables {
  return {
    body: texture(`${name} air body`),
    transmittance: texture(`${name} air transmittance`),
    absorberColumns: texture(`${name} air absorber columns`),
    absorberPressureColumns: texture(`${name} air absorber pressure columns`),
    lastAbsorberPressureColumn: texture(`${name} air last absorber pressure column`),
    absorberCurves: texture(`${name} air absorber curves`),
    irradiance: texture(`${name} air irradiance`),
  };
}

/** A one-class map surveyed whole, its weights a stand-in texture named `name` (R10.T10.d). */
function classMapOf(name: string): ClassMapDiscSurface {
  const law = PROVISIONAL_PHOTOMETRY.law;
  return {
    kind: "class-map",
    weights: texture(name),
    texels: packClassMap(1, 1, () => [1]),
    laws: [law],
    elsewhere: law,
    normals: null,
  };
}

/** How a body of {@link planWith} is drawn: plain, with air, or with a class map. */
type BodyKind = "plain" | "air" | "map";

/** A frame's plan of 13 px discs, one a kind, each held to the disc regime; and their kinds by id. */
function planWith(kinds: ReadonlyArray<BodyKind>): {
  readonly plan: BodyFramePlan;
  readonly kindOf: ReadonlyMap<string, BodyKind>;
} {
  const { bodies, hosts } = bodiesOf(kinds.map(() => 13));
  const drawn: LitBodyInput[] = [];
  bodies.forEach((body, i) => {
    drawn.push(drawnAs(body, kinds[i] ?? "plain"));
  });
  const plan = planLitBodies(drawn, hosts, OPTIONS, new Map(drawn.map((b) => [b.id, "disc"])));
  return { plan, kindOf: new Map(drawn.map((b, i) => [b.id, kinds[i] ?? "plain"])) };
}

/** A body as {@link planWith} draws one of `kind`. */
function drawnAs(body: LitBodyInput, kind: BodyKind): LitBodyInput {
  let drawn: LitBodyInput;
  switch (kind) {
    case "air":
      drawn = { ...body, atmosphere: airTables(body.id) };
      break;
    case "map":
      drawn = { ...body, surface: classMapOf(`${body.id} class map`), rotation: IDENTITY_ROTATION };
      break;
    case "plain":
      drawn = body;
      break;
  }
  return drawn;
}

/** The names a set's dispatch binds of a body of `kind` (`texturesBound`'s), its body's id `body`. */
function boundFor(kind: BodyKind, body: string): string[] {
  let names: string[];
  switch (kind) {
    case "air":
      names = ["bodies:no class map", "bodies:no class map", `${body} air body`];
      break;
    case "map":
      names = [`${body} class map`, "bodies:no class map", "bodies:no atmosphere"];
      break;
    case "plain":
      names = ["bodies:no class map", "bodies:no class map", "bodies:no atmosphere"];
      break;
  }
  return names;
}

/** Each dispatch's class map, normal map and air body table, by name. */
function texturesBound(engine: CountingRenderEngine): Array<Array<string | undefined>> {
  return engine.dispatched.map(({ bindings: { sampled } }) => [
    sampled["class_weights"]?.name,
    sampled["surface_normals"]?.name,
    sampled["atmosphere_body"]?.name,
  ]);
}

/**
 * How many copies of `target` an inliner makes in `entry` of `source`: its call paths from there,
 * each function's calls of every other function the source declares counted in its body.
 */
function inlinedCopies(source: string, entry: string, target: string): number {
  const functions = wgslFunctions(source);
  const names = [...functions.keys()];
  const known = new Map<string, number>();
  const copiesIn = (name: string): number => {
    if (name === target) {
      return 1;
    }
    const seen = known.get(name);
    if (seen !== undefined) {
      return seen;
    }
    const body = functions.get(name)?.body ?? "";
    let copies = 0;
    for (const callee of names) {
      const calls = callee === name ? 0 : body.split(new RegExp(`\\b${callee}\\(`, "u")).length - 1;
      copies += calls > 0 ? calls * copiesIn(callee) : 0;
    }
    known.set(name, copies);
    return copies;
  };
  return copiesIn(entry);
}

describe("the cell pass for every record (R07.T8.e)", () => {
  it("takes a disc with air and a class map's disc as it takes a plain one", () => {
    const { plan } = planWith(["plain", "air", "map"]);
    expect(plan.discs.map(drawnThroughCells)).toEqual([true, true, true]);
  });

  it("lays its jobs by the textures their records bind, a set each, in the order of its first record", () => {
    const { plan, kindOf } = planWith(["plain", "air", "map", "plain"]);
    const records = recordsOf(plan, 4);
    const cells = discCellJobs(records);
    const pixels = records.map((record) => pixelsOf(record.rect).length);
    // The records of each set, in record order, the sets in the order of their first records.
    const expected: number[][] = [];
    const setKinds: Array<BodyKind | undefined> = [];
    records.forEach((record, i) => {
      const kind = kindOf.get(record.body);
      const at = setKinds.indexOf(kind);
      if (at < 0) {
        setKinds.push(kind);
        expected.push([i]);
      } else {
        expected[at]?.push(i);
      }
    });
    let first = 0;
    const sets = expected.map((indices) => {
      const count = indices.reduce((sum, i) => sum + (pixels[i] ?? 0), 0);
      const set = { first, count, record: indices[0] };
      first += count;
      return set;
    });
    expect([cells.sets, cells.count]).toEqual([sets, first]);
    // Each record's first job, and its jobs its own pixels in row order.
    const order = expected.flat();
    let job = 0;
    for (const index of order) {
      const range = cells.ranges[index];
      const record = records[index];
      if (range?.kind !== "rectangle" || record === undefined) {
        throw new Error(`record ${index} has no rectangle of sums`);
      }
      expect(range.first).toBe(job);
      const laid = rowsOf(cells.jobs.subarray(job * 4, (job + pixelsOf(record.rect).length) * 4));
      expect(laid).toEqual(pixelsOf(record.rect).map(([x, y]) => [index, x, y, 0]));
      job += pixelsOf(record.rect).length;
    }
  });

  it("dispatches once a set, each over its own run of jobs, binding its body's air or its class map", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const { plan, kindOf } = planWith(["plain", "air", "map", "plain"]);
    const cells = discCellJobs(plan.discs);
    expect(renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME)).toBe(
      true,
    );
    expect(
      engine.dispatched.map((d) => [
        d.pass,
        Array.from(d.bindings.uniforms["cell_pass"] ?? []),
        d.workgroups,
      ]),
    ).toEqual(
      cells.sets.map((set) => [
        DISC_CELLS_PASS,
        [set.count, set.first, 0, 0],
        cellWorkgroups(set.count),
      ]),
    );
    const bodyOf = (set: DiscCellSet): string => plan.discs[set.record]?.body ?? "";
    expect(texturesBound(engine)).toEqual(
      cells.sets.map((set) => boundFor(kindOf.get(bodyOf(set)) ?? "plain", bodyOf(set))),
    );
    // Every dispatch binds the same jobs and sums, which the draws read.
    expect(
      new Set(
        engine.dispatched.flatMap((d) => [
          d.bindings.buffers["cell_jobs"]?.name,
          d.bindings.buffers["cell_sums"]?.name,
        ]),
      ),
    ).toEqual(new Set(["bodies:disc cell jobs", "bodies:disc cell sums"]));
    renderer.dispose();
  });

  it("binds a class map's normal map where it carries one (R10.T10.g)", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const { plan } = planWith(["map"]);
    const [record] = recordsOf(plan, 1);
    if (record?.surface.kind !== "class-map") {
      throw new Error("no class map");
    }
    const normals = { ...record.surface, normals: unreadNormalMap(datumNormalMap(5), "normals") };
    const mapped: BodyFramePlan = { ...plan, discs: [{ ...record, surface: normals }] };
    renderer.dispatchCells(mapped, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
    expect(texturesBound(engine)).toEqual([
      [record.surface.weights.name, "normals", "bodies:no atmosphere"],
    ]);
    renderer.dispose();
  });

  it("gives two bodies' airs and two class maps a dispatch each, each binding its own", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const { plan, kindOf } = planWith(["air", "air", "map", "map"]);
    renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
    const expected = plan.discs.map((record) =>
      boundFor(kindOf.get(record.body) ?? "plain", record.body),
    );
    expect([engine.dispatched.length, new Set(texturesBound(engine).map(String))]).toEqual([
      4,
      new Set(expected.map(String)),
    ]);
    renderer.dispose();
  });

  // SwiftShader inlines every call: R08.T9.b's lookups made `shade` too large to take more than one
  // copy in an entry point (plan R08, "R07's shading called from one place").
  it.each([
    ["the disc's interior", BODY_DISC_MATERIALS.interior.fragmentWgsl, "fragmentMain"],
    ["the disc's limb", BODY_DISC_MATERIALS.limb.fragmentWgsl, "fragmentMain"],
    ["the smooth mesh", SMOOTH_MESH_MATERIAL.fragmentWgsl, "fragmentMain"],
    ["the disc cells kernel", BODY_DISC_CELLS_KERNEL.reference, "main"],
  ])("reaches `shade` from one call site in %s's entry point", (_name, source, entry) => {
    expect(inlinedCopies(source, entry, "shade")).toBe(1);
  });

  it("makes its dispatches again after a device loss, a set each, on the new buffers", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    renderer.dispatchCells(
      planWith(["plain", "air", "map"]).plan,
      engine.createCompute(BODY_DISC_CELLS_KERNEL),
      FRAME,
    );
    const before = engine.dispatched.map((d) => d.bindings.buffers["cell_sums"]);
    engine.restore();
    renderer.dispatchCells(
      planWith(["plain", "air", "map"]).plan,
      engine.createCompute(BODY_DISC_CELLS_KERNEL),
      FRAME,
    );
    const after = engine.dispatched
      .slice(before.length)
      .map((d) => d.bindings.buffers["cell_sums"]);
    expect([before.length, after.length, after.every((sums) => !before.includes(sums))]).toEqual([
      3,
      3,
      true,
    ]);
    renderer.dispose();
  });
});
