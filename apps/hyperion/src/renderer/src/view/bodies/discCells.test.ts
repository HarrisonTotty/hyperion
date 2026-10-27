import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
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
import { type BodyFigure, PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { SurfaceLightingTables } from "../atmosphere/surfaceLighting";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  toViewAxes,
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
  type DiscCellJobs,
  type DiscCellSet,
  type DiscRecord,
  DISC_ROWS,
  discCellJobs,
  drawnThroughCells,
  faceBandSpans,
  LIMB_SPAN_ROWS,
  type LimbSpans,
  limbSpans,
  limbThroughCells,
  MAX_CELL_JOBS,
  packDiscRecords,
  pixelsPerGroup,
  viewRay,
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

  it("takes no job of a disc of 32 px or more without the pass's frame, whose row 51 says it sums its own cells", () => {
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

  it("dispatches once for a 3.5 px and a 13 px disc, a workgroup a run of their pixels, under its label", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const plan = planOf([3.5, 13]);
    const groups = discCellJobs(plan.discs).groups.length / 4;
    expect(renderer.dispatchCells(plan, kernel, FRAME)).toBe(true);
    expect(
      engine.dispatched.map((d) => [
        d.kernel,
        d.pass,
        d.workgroups,
        Array.from(d.bindings.uniforms["cell_pass"] ?? []),
      ]),
    ).toEqual([[BODY_DISC_CELLS_KERNEL.name, DISC_CELLS_PASS, [groups, 1, 1], [groups, 0, 0, 0]]]);
    renderer.dispose();
  });

  it("binds the records, its jobs, its workgroups' runs of them and room for every pixel's sums", async () => {
    const { engine, plan } = await dispatchedFor([3.5, 13]);
    const buffers = onlyDispatch(engine).bindings.buffers;
    expect([
      buffers["discs"]?.name,
      buffers["cell_jobs"]?.name,
      buffers["cell_groups"]?.name,
      buffers["cell_sums"]?.name,
      (buffers["cell_sums"]?.bytes ?? 0) >= 32 * discCellJobs(plan.discs).count,
    ]).toEqual([
      "bodies:discs",
      "bodies:disc cell jobs",
      "bodies:disc cell groups",
      "bodies:disc cell sums",
      true,
    ]);
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

  it("writes the jobs' workgroups into the buffer it binds (R07.T8.g)", async () => {
    const { engine, plan } = await dispatchedFor([3.5, 13]);
    const written = lastWrite(engine, "bodies:disc cell groups");
    expect(Array.from(new Uint32Array(written.data.buffer))).toEqual(
      Array.from(discCellJobs(plan.discs).groups),
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

  it("dispatches nothing where no disc has a pixel through it, and writes −1 in their row 51", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    // Since R07.T8.f a disc of 32 px or more takes its limb's pixels: these two take none, their
    // limbs' cells past 8 × 8.
    const plan = planOf([40, 64]);
    const discs: DiscRecord[] = [];
    for (const record of plan.discs) {
      discs.push({ ...record, limbSamples: 16 });
    }
    const past: BodyFramePlan = { ...plan, discs };
    expect(renderer.dispatchCells(past, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME)).toBe(
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
      expect.arrayContaining([
        "bodies:disc cell jobs",
        "bodies:disc cell groups",
        "bodies:disc cell sums",
      ]),
    );
    expect([
      before?.["cell_jobs"] === after?.["cell_jobs"],
      before?.["cell_groups"] === after?.["cell_groups"],
      before?.["cell_sums"] === after?.["cell_sums"],
    ]).toEqual([false, false, false]);
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
    // A 13 px disc's 4 × 4 cells take four pixels a workgroup (R07.T8.g).
    const groupsOf = (i: number): number => Math.ceil((pixels[i] ?? 0) / 4);
    let first = 0;
    let firstGroup = 0;
    const sets = expected.map((indices) => {
      const count = indices.reduce((sum, i) => sum + (pixels[i] ?? 0), 0);
      const groups = indices.reduce((sum, i) => sum + groupsOf(i), 0);
      const set = { first, count, record: indices[0], firstGroup, groups };
      first += count;
      firstGroup += groups;
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
    const cells = discCellJobs(plan.discs, MAX_CELL_JOBS, FRAME);
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
        [set.groups, set.firstGroup, 0, 0],
        cellWorkgroups(set.groups),
      ]),
    );
    const bodyOf = (set: DiscCellSet): string => plan.discs[set.record]?.body ?? "";
    expect(texturesBound(engine)).toEqual(
      cells.sets.map((set) => boundFor(kindOf.get(bodyOf(set)) ?? "plain", bodyOf(set))),
    );
    // Every dispatch binds the same jobs, workgroups and sums, which the draws read.
    expect(
      new Set(
        engine.dispatched.flatMap((d) => [
          d.bindings.buffers["cell_jobs"]?.name,
          d.bindings.buffers["cell_groups"]?.name,
          d.bindings.buffers["cell_sums"]?.name,
        ]),
      ),
    ).toEqual(
      new Set(["bodies:disc cell jobs", "bodies:disc cell groups", "bodies:disc cell sums"]),
    );
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

/** A view, its camera at the identity, and the `discs` pass's frame for it. */
function frameOf(viewport: Viewport): DiscCellFrame {
  return {
    viewRotation: viewRotation4(CAMERA.orientation),
    projection: perspectiveReversedInfinite(
      CAMERA.fovXRad,
      viewport.widthPx / viewport.heightPx,
      NEAR_PLANE_M,
    ),
    size: viewport,
  };
}

/** The record of one body drawn as a disc in `viewport`, lit as {@link bodiesOf}'s are. */
function recordOfBody(body: LitBodyInput, viewport: Viewport): DiscRecord {
  const { hosts } = bodiesOf([13]);
  const [record] = planLitBodies(
    [body],
    hosts,
    { ...OPTIONS, viewport, presentedPx: viewport },
    new Map([[body.id, "disc"]]),
  ).discs;
  if (record === undefined) {
    throw new Error(`body ${body.id} has no record`);
  }
  return record;
}

/** A body of `figure` whose centre lies `distanceM` away along `direction`. */
function bodyToward(figure: BodyFigure, direction: Vec3, distanceM: number): LitBodyInput {
  return {
    id: "0200080020000000.0400",
    centreM: scale(normalise(direction), distanceM),
    figure,
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
}

/**
 * The pixels the limb draw shades (`bodyDisc.wgsl`'s `disc_pixel` at `edge_pass` 1), in `f64`:
 * those of the rectangle whose corners' mean limb angle is at most 0.75 of their gradient, one of
 * whose corners lies outside the limb or within 10⁻³ of the gradient inside it.
 */
function limbDrawPixels(record: DiscRecord, viewport: Viewport): Array<readonly [number, number]> {
  const centre = toViewAxes(record.direction, CAMERA.orientation);
  const pole = normalise(toViewAxes(record.pole, CAMERA.orientation));
  const stretch = 1 / record.polarOverEquatorial;
  const stretched = (x: Vec3): Vec3 => add(x, scale(pole, (stretch - 1) * dot(x, pole)));
  const scaledCentre = stretched(centre);
  const limb = Math.asin(record.radiusOverDistance / norm(scaledCentre));
  const angle = (x: number, y: number): number => {
    const ray = stretched(viewRay(x, y, CAMERA, viewport));
    return Math.atan2(norm(cross(ray, scaledCentre)), dot(ray, scaledCentre)) - limb;
  };
  const pixels: Array<readonly [number, number]> = [];
  const { rect } = record;
  for (let y = Math.floor(rect.topPx); y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = Math.floor(rect.leftPx); x < Math.ceil(rect.rightPx); x += 1) {
      if (dot(centre, viewRay(x + 0.5, y + 0.5, CAMERA, viewport)) <= 0) {
        continue;
      }
      const [c00, c10, c01, c11] = [
        angle(x, y),
        angle(x + 1, y),
        angle(x, y + 1),
        angle(x + 1, y + 1),
      ];
      const gradient = Math.hypot((c10 + c11 - c00 - c01) / 2, (c01 + c11 - c00 - c10) / 2);
      const mean = (c00 + c10 + c01 + c11) / 4;
      if (!(gradient > 0) || mean / gradient > 0.75) {
        continue;
      }
      if ([c00, c10, c01, c11].every((c) => c / gradient < -1e-3)) {
        continue;
      }
      pixels.push([x, y]);
    }
  }
  return pixels;
}

/** Whether pixel (x, y) lies in one of its row's spans. */
function inSpans(spans: LimbSpans, x: number, y: number): boolean {
  const row = y - spans.topPx;
  if (row < 0 || row >= spans.rows) {
    return false;
  }
  const at = (k: number): number => spans.spans[row * 4 + k] ?? Number.NaN;
  return (x >= at(0) && x < at(1)) || (x >= at(2) && x < at(3));
}

const EARTH: BodyFigure = { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null };

/** A Saturn-like figure flattened to the record's cap, f = 0.2, its pole tilted towards the view. */
const FLATTENED: BodyFigure = {
  equatorialRadiusM: 6.0268e7,
  polarRadiusM: 6.0268e7 * 0.8,
  pole: normalise(vec3(0.3, 1, 0.4)),
};

/** A view large enough for a 300 px disc. */
const LARGE_VIEW: Viewport = { widthPx: 640, heightPx: 480 };

/** The cases of the spans' test: a figure, the centre's direction, its distance, and the view. */
const LIMB_CASES: ReadonlyArray<readonly [string, BodyFigure, Vec3, number, Viewport]> = [
  [
    "a 40 px sphere",
    EARTH,
    vec3(0.05, 0.02, -1),
    6.371e6 / Math.sin(20 / (640 / (2 * Math.tan(Math.PI / 6)))),
    LARGE_VIEW,
  ],
  [
    "a 300 px sphere across the view's edge",
    EARTH,
    vec3(0.5, -0.1, -1),
    6.371e6 / Math.sin(150 / (640 / (2 * Math.tan(Math.PI / 6)))),
    LARGE_VIEW,
  ],
  [
    "a 160 px figure of f = 0.2, its pole tilted",
    FLATTENED,
    vec3(-0.1, 0.05, -1),
    6.0268e7 / Math.sin(80 / (640 / (2 * Math.tan(Math.PI / 6)))),
    LARGE_VIEW,
  ],
  [
    "an Earth filling the view from 400 km",
    EARTH,
    vec3(0, -Math.sin((80 * Math.PI) / 180), -Math.cos((80 * Math.PI) / 180)),
    6.371e6 + 4e5,
    VIEWPORT,
  ],
];

/** A case of {@link LIMB_CASES}: the body's record, its limb's spans and the limb draw's pixels. */
function limbCase(
  figure: BodyFigure,
  direction: Vec3,
  distanceM: number,
  viewport: Viewport,
): {
  readonly record: DiscRecord;
  readonly spans: LimbSpans;
  readonly drawn: ReadonlyArray<readonly [number, number]>;
} {
  const record = recordOfBody(bodyToward(figure, direction, distanceM), viewport);
  const spans = limbSpans(record, frameOf(viewport));
  if (spans === null) {
    throw new Error("no spans");
  }
  return { record, spans, drawn: limbDrawPixels(record, viewport) };
}

describe("a large disc's limb through the cell pass (R07.T8.f)", () => {
  it("takes the limb of a disc whose interior takes other cells than its limb", () => {
    const [record] = recordsOf(planOf([40]), 1);
    if (record === undefined) {
      throw new Error("no disc");
    }
    const counts = (interior: number, limb: number): DiscRecord => ({
      ...record,
      interiorSamples: interior,
      limbSamples: limb,
    });
    expect(
      [counts(1, 4), counts(1, 8), counts(4, 4), counts(1, 16), counts(1, 2.5)].map(
        limbThroughCells,
      ),
    ).toEqual([true, true, false, false, false]);
  });

  it.each(LIMB_CASES)(
    "holds every pixel the limb draw shades in its spans: %s",
    (_, figure, direction, distanceM, viewport) => {
      const { spans, drawn } = limbCase(figure, direction, distanceM, viewport);
      expect([drawn.length > 0, drawn.filter(([x, y]) => !inSpans(spans, x, y))]).toEqual([
        true,
        [],
      ]);
    },
  );

  it.each(LIMB_CASES)(
    "lays each row's two spans in order within the rectangle: %s",
    (_, figure, direction, distanceM, viewport) => {
      const { record, spans } = limbCase(figure, direction, distanceM, viewport);
      const ordered = Array.from({ length: spans.rows }, (__, row) => {
        const [s0 = 0, e0 = 0, s1 = 0, e1 = 0] = spans.spans.subarray(row * 4, row * 4 + 4);
        return (
          s0 <= e0 &&
          e0 <= s1 &&
          s1 <= e1 &&
          s0 >= Math.floor(record.rect.leftPx) &&
          e1 <= Math.ceil(record.rect.rightPx)
        );
      });
      expect(ordered.every(Boolean)).toBe(true);
    },
  );

  // The spans' cost: 2.1 to 2.6 times the pixels drawn as built, most of the rest outside the limb,
  // whose cells shade nothing.
  it.each(LIMB_CASES)(
    "takes at most three times the limb draw's pixels: %s",
    (_, figure, direction, distanceM, viewport) => {
      const { spans, drawn } = limbCase(figure, direction, distanceM, viewport);
      expect(spans.pixels).toBeLessThanOrEqual(3 * drawn.length);
    },
  );

  it("finds a larger disc's limb in the pass's frame, and none without it", () => {
    const records = recordsOf(planOf([40, 13]), 2);
    const withFrame = discCellJobs(records, MAX_CELL_JOBS, FRAME);
    const without = discCellJobs(records);
    const kinds = (cells: DiscCellJobs): string[] =>
      records
        .map((record, i) => `${String(record.interiorSamples)}: ${cells.ranges[i]?.kind ?? "none"}`)
        .toSorted();
    expect([kinds(withFrame), kinds(without)]).toEqual([
      ["1: limb", "4: rectangle"],
      ["1: none", "4: rectangle"],
    ]);
  });

  it("lays one job a pixel of each limb span in row order, then one a pixel of its face band's (R07.T8.g)", () => {
    const [record] = recordsOf(planOf([40]), 1);
    if (record === undefined) {
      throw new Error("no disc");
    }
    const cells = discCellJobs([record], MAX_CELL_JOBS, FRAME);
    const spans = limbSpans(record, FRAME);
    const band = faceBandSpans(record, FRAME);
    if (spans === null || band === null) {
      throw new Error("no spans");
    }
    const expected: number[][] = [];
    for (const list of [spans, band]) {
      for (let row = 0; row < list.rows; row += 1) {
        const [s0 = 0, e0 = 0, s1 = 0, e1 = 0] = list.spans.subarray(row * 4, row * 4 + 4);
        for (let x = s0; x < e0; x += 1) {
          expected.push([0, x, list.topPx + row, 0]);
        }
        for (let x = s1; x < e1; x += 1) {
          expected.push([0, x, list.topPx + row, 0]);
        }
      }
    }
    const count = spans.pixels + band.pixels;
    // The limb's 4 × 4 cells four pixels a workgroup, the band's one cell 64.
    const groups = Math.ceil(spans.pixels / 4) + Math.ceil(band.pixels / 64);
    expect([band.pixels > 0, cells.count, rowsOf(cells.jobs), cells.sets, cells.spanRows]).toEqual([
      true,
      count,
      expected,
      [{ first: 0, count, record: 0, firstGroup: 0, groups }],
      LIMB_SPAN_ROWS * spans.rows,
    ]);
  });

  it("points row 51 at the limb's span rows past the records, each span its first sum, start and end", () => {
    const records = recordsOf(planOf([13, 40]), 2);
    const cells = discCellJobs(records, MAX_CELL_JOBS, FRAME);
    const index = cells.ranges.findIndex((range) => range?.kind === "limb");
    const range = cells.ranges[index];
    if (range?.kind !== "limb") {
      throw new Error("no limb");
    }
    const packed = packDiscRecords(records, cells);
    const { rows, topPx } = range.spans;
    const band = range.band;
    if (band === null) {
      throw new Error("no band");
    }
    expect(cellRows(packed, 2)[index]).toEqual([2 * DISC_ROWS + range.spanRow, rows, topPx, -1]);
    const tail = rowsOf(packed.subarray(2 * DISC_ROWS * 4));
    // Each pixel row's limb spans, then its face band's (R07.T8.g), the band's sums after the
    // limb's.
    let first = range.first;
    const expected: number[][] = Array.from({ length: LIMB_SPAN_ROWS * rows }, () => []);
    [range.spans.spans, band.spans].forEach((spans, list) => {
      for (let row = 0; row < rows; row += 1) {
        for (const span of [0, 1]) {
          const from = spans[row * 4 + 2 * span] ?? 0;
          const to = spans[row * 4 + 2 * span + 1] ?? 0;
          expected[LIMB_SPAN_ROWS * row + 2 * list + span] = [first, from, to, 0];
          first += to - from;
        }
      }
    });
    expect([tail, first]).toEqual([expected, range.first + range.spans.pixels + band.pixels]);
  });

  it("dispatches a larger disc's limb with the small discs of its set", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const plan = planOf([40, 13]);
    const cells = discCellJobs(plan.discs, MAX_CELL_JOBS, FRAME);
    renderer.dispatchCells(plan, engine.createCompute(BODY_DISC_CELLS_KERNEL), FRAME);
    expect(
      engine.dispatched.map((d) => Array.from(d.bindings.uniforms["cell_pass"] ?? [])),
    ).toEqual([[cells.groups.length / 4, 0, 0, 0]]);
    const packed = new Float32Array(lastWrite(engine, "bodies:discs").data.buffer);
    expect(Array.from(packed.subarray(0, (2 * DISC_ROWS + cells.spanRows) * 4))).toEqual(
      Array.from(packDiscRecords(plan.discs, cells)),
    );
    renderer.dispose();
  });

  it("leaves a limb too near a right angle from the camera to sum its own cells", () => {
    // From 10 m above an Earth's ground the limb's cone is within a pixel's turn of π ÷ 2.
    const record = recordOfBody(bodyToward(EARTH, vec3(0, -1, -0.2), 6.371e6 + 10), VIEWPORT);
    expect(limbSpans(record, frameOf(VIEWPORT))).toBeNull();
  });
});

/**
 * The face pixels whose one cell `cell_samples` shades at more than one point (`bodyDisc.wgsl`), in
 * `f64`: those of the rectangle whose four corners lie inside the limb, the interior draw's, whose
 * centre's depth δ = −angle ÷ slope, the slope by the angle's forward differences over 1/64 px, less
 * half the pixel's extent across the limb's normal, is at most two pixels.
 */
function bandPixels(record: DiscRecord, viewport: Viewport): Array<readonly [number, number]> {
  const centre = toViewAxes(record.direction, CAMERA.orientation);
  const pole = normalise(toViewAxes(record.pole, CAMERA.orientation));
  const stretch = 1 / record.polarOverEquatorial;
  const stretched = (x: Vec3): Vec3 => add(x, scale(pole, (stretch - 1) * dot(x, pole)));
  const scaledCentre = stretched(centre);
  const limb = Math.asin(record.radiusOverDistance / norm(scaledCentre));
  const angle = (x: number, y: number): number => {
    const ray = stretched(viewRay(x, y, CAMERA, viewport));
    return Math.atan2(norm(cross(ray, scaledCentre)), dot(ray, scaledCentre)) - limb;
  };
  const step = 1 / 64;
  const pixels: Array<readonly [number, number]> = [];
  const { rect } = record;
  for (let y = Math.floor(rect.topPx); y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = Math.floor(rect.leftPx); x < Math.ceil(rect.rightPx); x += 1) {
      const corners = [angle(x, y), angle(x + 1, y), angle(x, y + 1), angle(x + 1, y + 1)];
      if (!corners.every((c) => c < 0)) {
        continue;
      }
      const at = angle(x + 0.5, y + 0.5);
      const sx = (angle(x + 0.5 + step, y + 0.5) - at) / step;
      const sy = (angle(x + 0.5, y + 0.5 + step) - at) / step;
      const slope = Math.hypot(sx, sy);
      if (!(slope > 0)) {
        continue;
      }
      const halfExtent = (Math.abs(sx) + Math.abs(sy)) / slope / 2;
      if (-at / slope - halfExtent <= 2) {
        pixels.push([x, y]);
      }
    }
  }
  return pixels;
}

/** A case of {@link LIMB_CASES}: the body's record, its limb's and face band's spans, and the band. */
function bandCase(
  figure: BodyFigure,
  direction: Vec3,
  distanceM: number,
  viewport: Viewport,
): {
  readonly record: DiscRecord;
  readonly limb: LimbSpans;
  readonly band: LimbSpans;
  readonly shaded: ReadonlyArray<readonly [number, number]>;
} {
  const record = recordOfBody(bodyToward(figure, direction, distanceM), viewport);
  const limb = limbSpans(record, frameOf(viewport));
  const band = faceBandSpans(record, frameOf(viewport));
  if (limb === null || band === null) {
    throw new Error("no spans");
  }
  return { record, limb, band, shaded: bandPixels(record, viewport) };
}

describe("a large disc's face's near-limb band through the cell pass (R07.T8.g)", () => {
  it.each(LIMB_CASES)(
    "holds every face pixel shaded at more than one point in its spans: %s",
    (_, figure, direction, distanceM, viewport) => {
      const { band, shaded } = bandCase(figure, direction, distanceM, viewport);
      expect([shaded.length > 0, shaded.filter(([x, y]) => !inSpans(band, x, y))]).toEqual([
        true,
        [],
      ]);
    },
  );

  it.each(LIMB_CASES)(
    "lays each row's two spans in order within the rectangle, on the limb's rows: %s",
    (_, figure, direction, distanceM, viewport) => {
      const { record, limb, band } = bandCase(figure, direction, distanceM, viewport);
      const ordered = Array.from({ length: band.rows }, (__, row) => {
        const [s0 = 0, e0 = 0, s1 = 0, e1 = 0] = band.spans.subarray(row * 4, row * 4 + 4);
        return (
          s0 <= e0 &&
          e0 <= s1 &&
          s1 <= e1 &&
          s0 >= Math.floor(record.rect.leftPx) &&
          e1 <= Math.ceil(record.rect.rightPx)
        );
      });
      expect([ordered.every(Boolean), band.topPx, band.rows]).toEqual([
        true,
        limb.topPx,
        limb.rows,
      ]);
    },
  );

  // The spans' cost: 1.28 to 1.58 times the pixels shaded at nine points as built, the band's
  // margins and the gradient's bound over the slope where the view turns its rays less.
  it.each(LIMB_CASES)(
    "takes at most twice the pixels shaded at more than one point: %s",
    (_, figure, direction, distanceM, viewport) => {
      const { band, shaded } = bandCase(figure, direction, distanceM, viewport);
      expect(band.pixels).toBeLessThanOrEqual(2 * shaded.length);
    },
  );

  it("takes no band of a face of more than one cell, whose face sums its own", () => {
    const [record] = recordsOf(planOf([40]), 1);
    if (record === undefined) {
      throw new Error("no disc");
    }
    const twoCells = { ...record, interiorSamples: 2 };
    const cells = discCellJobs([twoCells], MAX_CELL_JOBS, FRAME);
    const range = cells.ranges[0];
    expect([
      faceBandSpans(twoCells, FRAME),
      range?.kind === "limb" ? range.band : "no limb",
      cells.count,
    ]).toEqual([null, null, limbSpans(twoCells, FRAME)?.pixels]);
  });
});

/**
 * The workgroups a frame's jobs should take: each range's runs of one cell count (a rectangle's,
 * or a limb's and then its face band's) in the jobs' order, each cut into workgroups of
 * {@link pixelsPerGroup} jobs, the last the rest, as (first job, jobs, n, 0).
 */
function expectedGroups(records: ReadonlyArray<DiscRecord>, cells: DiscCellJobs): number[][] {
  const runs: Array<readonly [number, number, number]> = [];
  records.forEach((record, index) => {
    const range = cells.ranges[index];
    if (range === null || range === undefined) {
      return;
    }
    if (range.kind === "rectangle") {
      runs.push([range.first, range.widthPx * range.heightPx, record.limbSamples]);
      return;
    }
    runs.push([range.first, range.spans.pixels, record.limbSamples]);
    if (range.band !== null && range.band.pixels > 0) {
      runs.push([range.first + range.spans.pixels, range.band.pixels, record.interiorSamples]);
    }
  });
  return runs
    .toSorted((a, b) => a[0] - b[0])
    .flatMap(([first, jobs, n]) => {
      const per = pixelsPerGroup(n);
      return Array.from({ length: Math.ceil(jobs / per) }, (_, g) => [
        first + g * per,
        Math.min(per, jobs - g * per),
        n,
        0,
      ]);
    });
}

describe("the cell pass's workgroups (R07.T8.g)", () => {
  it("takes ⌊64 ÷ n²⌋ pixels of n × n cells a workgroup", () => {
    expect([1, 2, 3, 4, 5, 6, 7, 8].map(pixelsPerGroup)).toEqual([64, 16, 7, 4, 2, 1, 1, 1]);
  });

  it("cuts each run of one cell count into full workgroups, the last the rest, in the jobs' order", () => {
    const records = recordsOf(planOf([3.5, 13, 40]), 3);
    const cells = discCellJobs(records, MAX_CELL_JOBS, FRAME);
    const groups = expectedGroups(records, cells);
    // Runs of 8 × 8, 4 × 4 and one cell, and every job in one workgroup.
    expect([
      new Set(groups.map((group) => group[2])),
      groups.reduce((sum, group) => sum + (group[1] ?? 0), 0),
    ]).toEqual([new Set([8, 4, 1]), cells.count]);
    expect(rowsOf(cells.groups)).toEqual(groups);
  });

  it("gives each set its run of the workgroups, over its own jobs", () => {
    const { plan } = planWith(["plain", "air", "map", "plain"]);
    const cells = discCellJobs(plan.discs, MAX_CELL_JOBS, FRAME);
    const groups = rowsOf(cells.groups);
    const spans = cells.sets.map((set) => {
      const own = groups.slice(set.firstGroup, set.firstGroup + set.groups);
      const last = own.at(-1);
      return [own[0]?.[0], (last?.[0] ?? 0) + (last?.[1] ?? 0)];
    });
    expect([spans, cells.sets.reduce((sum, set) => sum + set.groups, 0)]).toEqual([
      cells.sets.map((set) => [set.first, set.first + set.count]),
      groups.length,
    ]);
  });
});
