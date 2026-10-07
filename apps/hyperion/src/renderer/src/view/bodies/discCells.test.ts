import { describe, expect, it } from "vitest";

import { scale, vec3 } from "../../geometry/vec3";
import {
  type CountingRenderEngine,
  countingRenderEngine,
  type RecordedDispatch,
  type RecordedWrite,
} from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
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
import type { DrawItem } from "../engine/types";
import type { PlacedLight } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import { AU_M } from "../scenes/kept";
import type { ScreenRect } from "../wireframe/submit";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import {
  type DiscRecord,
  DISC_ROWS,
  discCellJobs,
  drawnThroughCells,
  MAX_CELL_JOBS,
  packDiscRecords,
} from "./discShading";
import {
  BODY_DISC_CELLS_KERNEL,
  type BodyFramePlan,
  cellWorkgroups,
  DISC_CELLS_PASS,
  type DiscCellFrame,
  discCellFrameBlock,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
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

  it("takes the 8 × 8 and 4 × 4 discs, not the larger rule's nor a class map's", () => {
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
        laws: [law],
        elsewhere: law,
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
    ).toEqual([true, true, false, false, false]);
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
