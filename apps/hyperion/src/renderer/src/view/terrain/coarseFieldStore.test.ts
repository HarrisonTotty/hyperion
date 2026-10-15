import type {
  BulkManifestDto,
  CoverDto,
  LiquidDto,
  PaletteEntryDto,
  SurfaceHeaderDto,
} from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import protocolSurfaceSource from "../../../../../../../crates/hyperion-protocol/src/surface.rs?raw";
import wireSource from "../../../../../../../crates/hyperion-surface/src/wire.rs?raw";
import { RECONNECT_DELAY_MS } from "../../lib/connection";
import { type ServerLink, useServerLink } from "../../lib/serverLink";
import { binaryFrame } from "../../test/binaryFrames";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { keptBody } from "../scenes/kept";
import {
  BLOCK_FRAME_BYTES,
  BLOCK_HEADER_BYTES,
  BLOCK_MAGIC,
  BlockSplitter,
  CoarseFieldStore,
  type FieldCandidate,
  fieldDemand,
  FIELD_RETRY_DELAY_MS,
  FIELD_RETRY_MAX_DELAY_MS,
  headerFailure,
  LIQUID_BINS,
  MAX_BLOCK_BYTES,
} from "./coarseFieldStore";
import { type CoverageView, fieldCellAt, resolutionOfCodeM } from "./coverage";
import { HeightWorkerPool } from "./workers/pool";

const UNIVERSE = "000000000000002a";
const MOON = keptBody(1);
const PLANET = keptBody(2);

/**
 * A Moon-like body's header at level 5: its mean radius the Moon's 1,737.4 km (IAU WGCCRE 2015,
 * Archinal et al. 2018), its equatorial and polar radii NASA's Moon fact sheet's 1,738.1 and
 * 1,736.0 km, and its other figures plausible stand-ins, since no test here reads them.
 */
const HEADER: SurfaceHeaderDto = {
  body: MOON,
  level: 5,
  radius_m: 1_737_400,
  equatorial_radius_m: 1_738_100,
  polar_radius_m: 1_736_000,
  boundary_diameter_m: 120_000,
  sea_level_m: -2_000,
  lapse_rate_k_per_m: 0,
  realised_sigma_h_m: 1_900,
  realised_relief_m: 18_000,
  months: 1,
  season_eccentricity: 0,
  reference_temperature_k: 250,
  surface_age_gyr: 4.4,
  surface_pressure_pa: 0,
  albedo_scale: null,
  climate_model: "radiative_equilibrium",
  precipitation: "heuristic",
  palette: [],
  main_liquid: null,
};

/** `v` in each of a liquid record's bins. */
function bins(v: number): LiquidDto["refractive_index"] {
  return [v, v, v, v, v, v, v, v, v, v, v, v, v, v, v];
}

/**
 * Liquid water's record at 25 °C, every member present: n about 1.333, Pope and Fry's absorption
 * about 0.05 m⁻¹ in the green, IAPWS's density, surface tension and viscosity, and a stand-in
 * scattering of order Morel's, since no test here reads the values.
 */
const WATER: LiquidDto = {
  temperature_k: 298.15,
  density_kg_m3: 997.047,
  basis: "measured",
  refractive_index: bins(1.333),
  absorption_per_m: bins(0.05),
  scattering_per_m: bins(0.002),
  surface_tension_n_per_m: 0.071_97,
  viscosity_pa_s: 8.9e-4,
};

/** A basaltic melt's record, its optional members not modelled. */
const MELT: LiquidDto = {
  temperature_k: 1_473.15,
  density_kg_m3: 2_700,
  basis: "stand_in",
  refractive_index: bins(1.565),
};

/** A palette entry of `substance` as `role`, its record `liquid`. */
function paletteEntry(
  substance: string,
  role: PaletteEntryDto["role"],
  liquid: LiquidDto | null,
): PaletteEntryDto {
  return { substance, role, normal_albedo_bvr: [0.1, 0.12, 0.14], phase_row: 1, liquid };
}

/** {@link HEADER} with a lava-crust, an ice and a sea, the sea its main liquid. */
const SEA_HEADER: SurfaceHeaderDto = {
  ...HEADER,
  palette: [
    paletteEntry("basalt", "primary_crust", MELT),
    paletteEntry("H2O", "ice", null),
    { ...paletteEntry("H2O", "liquid", WATER), normal_albedo_bvr: [0.0208, 0.0204, 0.0202] },
  ],
  main_liquid: 2,
  maturity: 0.75,
  organic_cover: 0,
};

/** {@link SEA_HEADER} with its sea's record `liquid`. */
function seaWith(liquid: LiquidDto): SurfaceHeaderDto {
  const [crust, ice, sea] = SEA_HEADER.palette;
  if (crust === undefined || ice === undefined || sea === undefined) {
    throw new Error("the sea header holds three entries");
  }
  return { ...SEA_HEADER, palette: [crust, ice, { ...sea, liquid }] };
}

/** A payload block of `length` bytes: R09's frame (HYSF, format 1, its length at offset 6), then `marker`. */
function block(length: number, marker: number): Uint8Array {
  const bytes = new Uint8Array(length).fill(marker);
  bytes.set(BLOCK_MAGIC, 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(4, 1, true);
  view.setUint32(6, length, true);
  return bytes;
}

/** `blocks` end to end, as a payload. */
function payloadOf(blocks: ReadonlyArray<Uint8Array>): Uint8Array {
  const payload = new Uint8Array(blocks.reduce((n, b) => n + b.length, 0));
  let at = 0;
  for (const b of blocks) {
    payload.set(b, at);
    at += b.length;
  }
  return payload;
}

/** Sends `payload` as request `id`'s binary frames, cut every `frameBytes`, and gives its manifest. */
function sendFrames(
  socket: FakeWebSocket,
  id: number,
  payload: Uint8Array,
  frameBytes: number,
): BulkManifestDto {
  const count = Math.ceil(payload.length / frameBytes);
  for (let k = 0; k < count; k += 1) {
    const part = payload.subarray(k * frameBytes, (k + 1) * frameBytes);
    socket.serverSendsBinary(binaryFrame(id, k, count, [...part]));
  }
  return { chunks: count, bytes: payload.length };
}

function cover(...ranges: ReadonlyArray<readonly [number, number, number]>): CoverDto {
  return { ranges: ranges.map(([start, end, resolution]) => ({ start, end, resolution })) };
}

function respondReady(
  socket: FakeWebSocket,
  id: number,
  revision: number,
  coverage: CoverDto,
  bulk: BulkManifestDto,
): void {
  socket.serverResponds(id, {
    kind: "surface_field",
    state: "ready",
    header: HEADER,
    revision,
    coverage,
    bulk,
  });
}

/** Resolves after `hops` turns of the microtask queue. */
function microtasks(hops: number): Promise<void> {
  return hops === 0 ? Promise.resolve() : Promise.resolve().then(() => microtasks(hops - 1));
}

/** Lets the outcomes' promise chains settle. */
async function settle(): Promise<void> {
  await act(async () => {
    await microtasks(8);
  });
}

/** Each field message a worker received: its kind, body, length and the marker after its frame. */
function fieldLog(w: ScriptedHeightWorker): string[] {
  return w.fieldMessages.map((m) =>
    m.kind === "field-release"
      ? `release ${m.body}`
      : `${m.kind === "field-init" ? "init" : "chunk"} ${m.body} ${m.block.byteLength} ` +
        `#${new Uint8Array(m.block)[BLOCK_FRAME_BYTES] ?? -1}`,
  );
}

interface Harness {
  readonly socket: () => FakeWebSocket;
  readonly link: () => ServerLink;
  readonly workers: ScriptedHeightWorker[];
  readonly pool: HeightWorkerPool;
  readonly store: CoarseFieldStore;
  readonly revisions: Array<{ readonly body: string; readonly coverage: CoverageView }>;
}

function harness(workerCount = 3): Harness {
  const hook = renderHook(() => useServerLink(), { wrapper: ServerLinkHarness });
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  const workers: ScriptedHeightWorker[] = [];
  const pool = new HeightWorkerPool({
    workers: workerCount,
    createWorker: () => {
      const worker = new ScriptedHeightWorker();
      workers.push(worker);
      return worker;
    },
    bake: {
      vertexPath: "face-differences",
      normals: "mesh",
      source: { kind: "test-planet", ridges: "off" },
    },
  });
  const store = new CoarseFieldStore({
    universe: UNIVERSE,
    requests: hook.result.current.requests,
    pool,
    linkUp: true,
  });
  const revisions: Array<{ readonly body: string; readonly coverage: CoverageView }> = [];
  store.onRevision((body, coverage) => revisions.push({ body, coverage }));
  return {
    socket: () => FakeWebSocket.latest(),
    link: () => hook.result.current,
    workers,
    pool,
    store,
    revisions,
  };
}

/** The moon's hand-over range in the tests' one style, m. */
const MOON_RANGE_M = 1e7;

/** The moon, seen by a view at `distanceM` from its centre, with a hand-over range of 10⁷ m. */
function moonAt(distanceM: number): FieldCandidate {
  return { body: MOON, local: false, rangeRatio: distanceM / MOON_RANGE_M };
}

/** The latest surface_field request's ID and `have_revision`. */
function latestAsk(socket: FakeWebSocket): { readonly id: number; readonly have: number | null } {
  const sent = socket.requestsOfKind("surface_field").at(-1);
  if (sent === undefined) {
    throw new Error("no surface_field request was sent");
  }
  return { id: sent.id, have: sent.body.have_revision };
}

/** Answers the latest request with `blocks` cut every `frameBytes`, at `revision` over `coverage`. */
async function answer(
  h: Harness,
  blocks: ReadonlyArray<Uint8Array>,
  revision: number,
  coverage: CoverDto,
  frameBytes = 64,
): Promise<void> {
  const socket = h.socket();
  const { id } = latestAsk(socket);
  respondReady(
    socket,
    id,
    revision,
    coverage,
    sendFrames(socket, id, payloadOf(blocks), frameBytes),
  );
  await settle();
}

/** Refuses the latest request as the server does when too busy, and lets the refusal settle. */
async function busyAnswer(h: Harness, code: "queue_full" | "too_many_requests"): Promise<void> {
  h.socket().serverRejects(latestAsk(h.socket()).id, { code, message: "busy", field: null });
  await settle();
}

/** Checks that the store asks again `delayMs` after a busy answer, and not a millisecond before. */
function expectAskAfter(h: Harness, delayMs: number, asks: number): void {
  act(() => {
    vi.advanceTimersByTime(delayMs - 1);
  });
  expect(h.socket().requestsOfKind("surface_field")).toHaveLength(asks - 1);
  act(() => {
    vi.advanceTimersByTime(1);
  });
  expect(h.socket().requestsOfKind("surface_field")).toHaveLength(asks);
}

/** Answers busy `times` times, waiting out the longest delay after each. */
async function busyThenWaitOut(h: Harness, times: number): Promise<void> {
  if (times === 0) {
    return;
  }
  await busyAnswer(h, "queue_full");
  act(() => {
    vi.advanceTimersByTime(FIELD_RETRY_MAX_DELAY_MS);
  });
  await busyThenWaitOut(h, times - 1);
}

describe("the block splitter", () => {
  it("holds R09's frame constants", () => {
    expect(wireSource).toContain("pub const MAX_BLOCK_BYTES: usize = 1 << 20;");
    expect(MAX_BLOCK_BYTES).toBe(1 << 20);
    expect(wireSource).toContain('pub const BLOCK_MAGIC: [u8; 4] = *b"HYSF";');
    expect(String.fromCharCode(...BLOCK_MAGIC)).toBe("HYSF");
    expect(wireSource).toContain("pub const BLOCK_HEADER_BYTES: usize = 33;");
    expect(BLOCK_HEADER_BYTES).toBe(33);
    expect(wireSource).toContain("const FRAME_BYTES: usize = 10;");
    expect(BLOCK_FRAME_BYTES).toBe(10);
    expect(wireSource).toContain(
      "//! | 6      | 4     | the block's length in bytes, this header included, `u32`           |",
    );
  });

  it("cuts blocks at their lengths across chunks of any size, one buffer a block", () => {
    const blocks = [block(40, 1), block(100, 2), block(33, 3)];
    const payload = payloadOf(blocks);
    for (const size of [1, 7, 10, 41, 173]) {
      const splitter = new BlockSplitter();
      const out: Uint8Array[] = [];
      for (let at = 0; at < payload.length; at += size) {
        const split = splitter.push(payload.subarray(at, at + size));
        if (!split.ok) {
          throw new Error(split.message);
        }
        out.push(...split.blocks.map((b) => new Uint8Array(b)));
      }
      expect(out, `chunks of ${size}`).toEqual(blocks);
      expect(splitter.atBoundary).toBe(true);
    }
  });

  it("is not at a boundary inside a block or its frame", () => {
    const splitter = new BlockSplitter();
    splitter.push(block(40, 1).subarray(0, 6));
    expect(splitter.atBoundary).toBe(false);
    splitter.push(block(40, 1).subarray(6, 20));
    expect(splitter.atBoundary).toBe(false);
  });

  it("refuses a frame without HYSF, or a length outside the header to 1 MiB, and all after", () => {
    const bad = block(40, 1);
    bad[0] = 0;
    const splitter = new BlockSplitter();
    expect(splitter.push(bad)).toMatchObject({
      ok: false,
      message: expect.stringMatching(/HYSF/u),
    });
    expect(splitter.push(block(40, 1))).toMatchObject({ ok: false });
    for (const length of [BLOCK_HEADER_BYTES - 1, MAX_BLOCK_BYTES + 1]) {
      const frame = block(BLOCK_FRAME_BYTES, 0);
      new DataView(frame.buffer).setUint32(6, length, true);
      expect(new BlockSplitter().push(frame)).toMatchObject({ ok: false });
    }
  });
});

describe("the field demand", () => {
  it("wants a view's local body at any distance, and one within its hand-over range", () => {
    const demand = fieldDemand(
      [{ body: PLANET, local: true, rangeRatio: 1e5 }, moonAt(9e6)],
      new Set(),
    );
    expect(demand.want).toEqual([
      { body: PLANET, reason: "local-body" },
      { body: MOON, reason: "in-range" },
    ]);
    expect(demand.release).toEqual([]);
  });

  it("keeps a wanted body inside twice its range, and releases it beyond, or once gone", () => {
    expect(fieldDemand([moonAt(1.9e7)], new Set([MOON]))).toEqual({ want: [], release: [] });
    expect(fieldDemand([moonAt(2.1e7)], new Set([MOON]))).toEqual({ want: [], release: [MOON] });
    expect(fieldDemand([moonAt(1.9e7)], new Set())).toEqual({ want: [], release: [] });
    expect(fieldDemand([], new Set([MOON]))).toEqual({ want: [], release: [MOON] });
  });

  it("judges each view by its own style's range, keeping a body while any view is in its band", () => {
    // A wireframe view near the moon but outside its 4 px range, and a high-setting view farther
    // but inside its 1 px range: the second wants it.
    const wireframe: FieldCandidate = { body: MOON, local: false, rangeRatio: 5e6 / 4.6e6 };
    const high: FieldCandidate = { body: MOON, local: false, rangeRatio: 1.5e7 / 1.8e7 };
    expect(fieldDemand([wireframe, high], new Set()).want).toEqual([
      { body: MOON, reason: "in-range" },
    ]);
    // Wanted, the wireframe view inside twice its range keeps it though the other has left.
    const highFar: FieldCandidate = { body: MOON, local: false, rangeRatio: 5e7 / 1.8e7 };
    expect(fieldDemand([wireframe, highFar], new Set([MOON]))).toEqual({ want: [], release: [] });
    expect(fieldDemand([highFar], new Set([MOON]))).toEqual({ want: [], release: [MOON] });
    // A body local to one view and out of every range is wanted once, as the local body.
    const localThere: FieldCandidate = { body: MOON, local: true, rangeRatio: 1e3 };
    expect(fieldDemand([highFar, localThere], new Set()).want).toEqual([
      { body: MOON, reason: "local-body" },
    ]);
  });
});

describe("the field header's checks", () => {
  it("holds the protocol's liquid bin count", () => {
    expect(protocolSurfaceSource).toContain("pub const SURFACE_LIQUID_BINS: usize = 15;");
    expect(LIQUID_BINS).toBe(15);
  });

  it("takes a sea's full record, a melt's with members not modelled, and the two fractions", () => {
    expect(headerFailure(HEADER)).toBeNull();
    expect(headerFailure(SEA_HEADER)).toBeNull();
    expect(headerFailure(seaWith(WATER))).toBeNull();
    expect(headerFailure({ ...SEA_HEADER, maturity: 1, organic_cover: null })).toBeNull();
    expect(
      headerFailure({ ...HEADER, palette: [paletteEntry("basalt", "bulk", null)] }),
    ).toBeNull();
  });

  it("refuses a main liquid that is not a liquid entry of the palette", () => {
    for (const main of [0, 1, 3, 1.5]) {
      expect(headerFailure({ ...SEA_HEADER, main_liquid: main })).toMatch(
        new RegExp(`main liquid, entry ${main}, is not a liquid`, "u"),
      );
    }
  });

  it("refuses a maturity or an organic cover outside 0 to 1", () => {
    expect(headerFailure({ ...SEA_HEADER, maturity: 1.5 })).toMatch(/maturity, 1.5, is not/u);
    expect(headerFailure({ ...SEA_HEADER, organic_cover: -0 })).toMatch(/organic cover, 0, is/u);
    expect(headerFailure({ ...SEA_HEADER, organic_cover: Number.NaN })).toMatch(/organic cover/u);
  });

  it("refuses a liquid entry without a liquid record", () => {
    const [crust, ice, sea] = SEA_HEADER.palette;
    if (crust === undefined || ice === undefined || sea === undefined) {
      throw new Error("the sea header holds three entries");
    }
    expect(headerFailure({ ...SEA_HEADER, palette: [crust, ice, { ...sea, liquid: null }] })).toBe(
      "the palette's entry 2, H2O as liquid, has no liquid record",
    );
  });

  it("refuses a liquid record on an ice", () => {
    const [crust, ice, sea] = SEA_HEADER.palette;
    if (crust === undefined || ice === undefined || sea === undefined) {
      throw new Error("the sea header holds three entries");
    }
    expect(
      headerFailure({ ...SEA_HEADER, palette: [crust, { ...ice, liquid: WATER }, sea] }),
    ).toMatch(/entry 1, H2O as ice, has a liquid record, which no ice/u);
  });

  it("refuses a record's member out of its range, or of another bin count", () => {
    const short = [...bins(1.333)].slice(1);
    const cases: ReadonlyArray<readonly [Partial<Record<keyof LiquidDto, unknown>>, RegExp]> = [
      [{ temperature_k: 0 }, /temperature, 0 K, is not positive/u],
      [{ density_kg_m3: Number.POSITIVE_INFINITY }, /density/u],
      [{ basis: "guessed" }, /basis, "guessed", is not one/u],
      [{ refractive_index: short }, /refractive index is not 15 finite values above 0/u],
      [{ refractive_index: bins(0) }, /refractive index/u],
      [{ absorption_per_m: bins(-0) }, /absorption is not 15/u],
      [{ scattering_per_m: [...bins(0.002), 0.002] }, /scattering is not 15/u],
      [{ surface_tension_n_per_m: -0.07 }, /surface tension, -0.07 N\/m/u],
      [{ viscosity_pa_s: 0 }, /viscosity, 0 Pa s/u],
    ];
    for (const [change, message] of cases) {
      // A member out of range is what a damaged answer holds, which the DTO's type cannot.
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      const record = { ...WATER, ...change } as LiquidDto;
      expect(headerFailure(seaWith(record))).toMatch(message);
    }
  });

  it("refuses an entry's normal albedo below 0", () => {
    const entry = paletteEntry("basalt", "primary_crust", null);
    const changed: PaletteEntryDto = { ...entry, normal_albedo_bvr: [0.1, -0.1, 0.1] };
    expect(headerFailure({ ...HEADER, palette: [changed] })).toMatch(/normal albedo/u);
  });

  it("refuses an entry's phase row that is not a byte", () => {
    const entry = paletteEntry("basalt", "primary_crust", null);
    expect(headerFailure({ ...HEADER, palette: [{ ...entry, phase_row: 256 }] })).toMatch(
      /phase row, 256, that is not a byte/u,
    );
  });
});

describe("the coarse-field store", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("asks once for a body within range and posts its blocks to every worker as they arrive", async () => {
    const h = harness();
    h.store.follow([moonAt(5e6)]);
    h.store.follow([moonAt(4e6)]);
    const asks = h.socket().requestsOfKind("surface_field");
    expect(asks).toHaveLength(1);
    expect(asks[0]?.body).toEqual({
      kind: "surface_field",
      universe: UNIVERSE,
      body: MOON,
      have_revision: null,
    });
    const { id } = latestAsk(h.socket());
    const payload = payloadOf([block(40, 1), block(100, 2), block(33, 3)]);
    // Frames of 25 bytes: the first block completes inside the second frame.
    const socket = h.socket();
    socket.serverSendsBinary(binaryFrame(id, 0, 7, [...payload.subarray(0, 25)]));
    for (const w of h.workers) {
      expect(fieldLog(w)).toEqual([]);
    }
    socket.serverSendsBinary(binaryFrame(id, 1, 7, [...payload.subarray(25, 50)]));
    for (const w of h.workers) {
      expect(fieldLog(w)).toEqual([`init ${MOON} 40 #1`]);
    }
    for (let k = 2; k < 7; k += 1) {
      socket.serverSendsBinary(binaryFrame(id, k, 7, [...payload.subarray(25 * k, 25 * k + 25)]));
    }
    // Every block is posted before the answer arrives.
    for (const w of h.workers) {
      expect(fieldLog(w)).toEqual([
        `init ${MOON} 40 #1`,
        `chunk ${MOON} 100 #2`,
        `chunk ${MOON} 33 #3`,
      ]);
    }
    expect(h.store.coverage(MOON)).toBeNull();
    expect(h.store.status(MOON)?.pending).toBe(true);
    respondReady(socket, id, 1, cover([0, 3, 17], [3, 5, 0]), { chunks: 7, bytes: payload.length });
    await settle();
    const coverage = h.store.coverage(MOON);
    expect(coverage?.revision).toBe(1);
    expect(coverage?.resolutionAtM(fieldCellAt(5, 2))).toBe(resolutionOfCodeM(17));
    expect(coverage?.resolutionAtM(fieldCellAt(5, 4))).toBeNull();
    expect(h.store.header(MOON)).toEqual(HEADER);
    expect(h.store.status(MOON)).toEqual({
      reason: "in-range",
      pending: false,
      notModelled: null,
      failure: null,
    });
    expect(h.revisions.map((r) => [r.body, r.coverage.revision])).toEqual([[MOON, 1]]);
  });

  it("asks a rise in revision for the new cells only, with the revision held", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1)], 1, cover([0, 2, 30]));
    h.store.noteSurfaceRevisions([
      { body: MOON, revision: 1 },
      { body: PLANET, revision: 4 },
    ]);
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(1);
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    expect(latestAsk(h.socket()).have).toBe(1);
    await answer(h, [block(40, 4), block(60, 5)], 2, cover([2, 3, 12]));
    const coverage = h.store.coverage(MOON);
    expect(coverage?.revision).toBe(2);
    expect(coverage?.resolutionAtM(fieldCellAt(5, 1))).toBe(resolutionOfCodeM(30));
    expect(coverage?.resolutionAtM(fieldCellAt(5, 2))).toBe(resolutionOfCodeM(12));
    // The delta's block 0 carries the whole header again, and is a chunk: the field is started.
    expect(fieldLog(h.workers[0] ?? new ScriptedHeightWorker())).toEqual([
      `init ${MOON} 50 #1`,
      `chunk ${MOON} 40 #4`,
      `chunk ${MOON} 60 #5`,
    ]);
  });

  it("asks again once an answer settles behind a revision announced while it was in flight", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 3 }]);
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(1);
    await answer(h, [block(50, 1)], 2, cover([0, 1, 30]));
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(2);
    expect(latestAsk(h.socket()).have).toBe(2);
  });

  it("asks nothing while a view moves to and fro inside the hysteresis band", async () => {
    const h = harness(1);
    h.store.follow([moonAt(5e6)]);
    await answer(h, [block(50, 1)], 1, cover([0, 1, 30]));
    h.store.follow([moonAt(1.5e7)]);
    h.store.follow([moonAt(1.9e7)]);
    h.store.follow([moonAt(8e6)]);
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(1);
    expect(fieldLog(h.workers[0] ?? new ScriptedHeightWorker())).toEqual([`init ${MOON} 50 #1`]);
    h.store.follow([moonAt(2.5e7)]);
    expect(h.store.wanted.has(MOON)).toBe(false);
    h.store.follow([moonAt(5e6)]);
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(2);
    expect(latestAsk(h.socket()).have).toBeNull();
  });

  it("starts each worker's field once and posts it every block, over whole and delta payloads", async () => {
    const h = harness(3);
    h.store.want(MOON, "local-body");
    await answer(h, [block(300, 1), block(200, 2)], 1, cover([0, 9, 20]), 256);
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    await answer(h, [block(120, 3), block(90, 4), block(70, 5)], 2, cover([9, 12, 20]), 100);
    for (const w of h.workers) {
      const kinds = w.fieldMessages.map((m) => m.kind);
      expect(kinds.filter((k) => k === "field-init")).toHaveLength(1);
      expect(kinds).toEqual(["field-init", ...Array<string>(4).fill("field-chunk")]);
    }
  });

  it("reports the workers' copies together, for T14", async () => {
    const h = harness(3);
    h.store.want(MOON, "local-body");
    await answer(h, [block(300, 1), block(200, 2)], 1, cover([0, 9, 20]));
    for (const w of h.workers) {
      w.answerFields();
    }
    expect(h.pool.fieldResidentBytes).toBe(3 * 500);
    h.store.release(MOON);
    for (const w of h.workers) {
      w.answerFields();
    }
    expect(h.pool.fieldResidentBytes).toBe(0);
  });

  it("frees a released body in every worker and cancels its request, ignoring what follows", async () => {
    const h = harness(2);
    h.store.want(MOON, "local-body");
    const { id } = latestAsk(h.socket());
    const payload = payloadOf([block(40, 1), block(40, 2)]);
    h.socket().serverSendsBinary(binaryFrame(id, 0, 2, [...payload.subarray(0, 50)]));
    h.store.release(MOON);
    expect(h.socket().cancelledIds()).toEqual([id]);
    h.socket().serverSendsBinary(binaryFrame(id, 1, 2, [...payload.subarray(50)]));
    await settle();
    for (const w of h.workers) {
      expect(fieldLog(w)).toEqual([`init ${MOON} 40 #1`, `release ${MOON}`]);
    }
    expect(h.pool.fieldBodies).toEqual([]);
    expect(h.store.status(MOON)).toBeNull();
    expect(h.store.coverage(MOON)).toBeNull();
  });

  it("keeps what was received when the link drops, marks it stale, and asks again on return", async () => {
    vi.useFakeTimers();
    const h = harness(1);
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1)], 1, cover([0, 2, 30]));
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    const { id } = latestAsk(h.socket());
    // Half the delta arrives: its first block is posted, then the link drops.
    const delta = payloadOf([block(40, 2), block(40, 3)]);
    h.socket().serverSendsBinary(binaryFrame(id, 0, 2, [...delta.subarray(0, 50)]));
    await act(async () => {
      h.socket().close();
      await Promise.resolve();
    });
    h.store.setLinkUp(h.link().status === "connected");
    await settle();
    expect(h.store.coverage(MOON)?.stale).toBe(true);
    expect(h.store.coverage(MOON)?.resolutionAtM(fieldCellAt(5, 1))).toBe(resolutionOfCodeM(30));
    expect(h.store.status(MOON)).toMatchObject({ pending: true, failure: null });
    const log = fieldLog(h.workers[0] ?? new ScriptedHeightWorker());
    expect(log).toEqual([`init ${MOON} 50 #1`, `chunk ${MOON} 40 #2`]);
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const next = FakeWebSocket.latest();
    act(() => {
      next.serverWelcomes();
    });
    h.store.setLinkUp(h.link().status === "connected");
    expect(next.requestsOfKind("surface_field")).toHaveLength(1);
    expect(latestAsk(next).have).toBe(1);
    await answer(h, [block(40, 2), block(40, 3)], 2, cover([2, 4, 30]));
    expect(h.store.coverage(MOON)?.stale).toBe(false);
    expect(h.store.coverage(MOON)?.revision).toBe(2);
  });

  it("asks for a body wanted while the link is down once it returns", () => {
    const h = harness(1);
    h.store.setLinkUp(false);
    h.store.want(MOON, "local-body");
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(0);
    expect(h.store.status(MOON)?.pending).toBe(true);
    h.store.setLinkUp(true);
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(1);
  });

  it("holds a body not modelled with the section it needs, and posts no field", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    h.socket().serverResponds(latestAsk(h.socket()).id, {
      kind: "surface_field",
      state: "not_modelled",
      section: "surface",
    });
    await settle();
    expect(h.store.status(MOON)).toMatchObject({ notModelled: "surface", failure: null });
    expect(h.store.coverage(MOON)).toBeNull();
    expect(h.workers[0]?.fieldMessages).toEqual([]);
  });

  it("holds a refusal and asks nothing more until the body is wanted afresh", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    h.socket().serverRejects(latestAsk(h.socket()).id, {
      code: "unknown_body",
      message: "no such body",
      field: "body",
    });
    await settle();
    expect(h.store.status(MOON)?.failure).toMatch(/unknown_body/u);
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 5 }]);
    h.store.setLinkUp(false);
    h.store.setLinkUp(true);
    h.store.want(MOON, "in-range");
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(1);
    h.store.release(MOON);
    h.store.want(MOON, "in-range");
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(2);
  });

  it("asks for the field whole, dropping what it held, when the server refuses the revision held", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1)], 2, cover([0, 2, 30]));
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 3 }]);
    expect(latestAsk(h.socket()).have).toBe(2);
    h.socket().serverRejects(latestAsk(h.socket()).id, {
      code: "bad_request",
      message: "have_revision 2 is above the body's revision 1",
      field: "have_revision",
    });
    await settle();
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(3);
    expect(latestAsk(h.socket()).have).toBeNull();
    expect(h.store.coverage(MOON)).toBeNull();
    await answer(h, [block(60, 7)], 1, cover([5, 6, 40]));
    expect(fieldLog(h.workers[0] ?? new ScriptedHeightWorker())).toEqual([
      `init ${MOON} 50 #1`,
      `release ${MOON}`,
      `init ${MOON} 60 #7`,
    ]);
    expect(h.store.coverage(MOON)?.revision).toBe(1);
    expect(h.store.coverage(MOON)?.resolutionAtM(fieldCellAt(5, 0))).toBeNull();
    // The revision announced from the other history is forgotten: nothing more is asked.
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(3);
    expect(h.store.status(MOON)).toMatchObject({ pending: false, failure: null });
  });

  it("asks again, waiting longer each time, while the server is too busy", async () => {
    vi.useFakeTimers();
    const h = harness(1);
    h.store.want(MOON, "local-body");
    await busyAnswer(h, "queue_full");
    expect(h.store.status(MOON)).toMatchObject({ pending: true, failure: null });
    expectAskAfter(h, FIELD_RETRY_DELAY_MS, 2);
    await busyAnswer(h, "too_many_requests");
    expectAskAfter(h, 2 * FIELD_RETRY_DELAY_MS, 3);
    await answer(h, [block(50, 1)], 1, cover([0, 1, 9]));
    expect(h.store.coverage(MOON)?.revision).toBe(1);
    expect(h.store.status(MOON)).toMatchObject({ pending: false, failure: null });
  });

  it("waits no longer than its longest delay, and stops waiting once released", async () => {
    vi.useFakeTimers();
    const h = harness(1);
    h.store.want(MOON, "local-body");
    await busyThenWaitOut(h, 8);
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(9);
    await busyAnswer(h, "queue_full");
    h.store.release(MOON);
    act(() => {
      vi.advanceTimersByTime(FIELD_RETRY_MAX_DELAY_MS);
    });
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(9);
  });

  it("lets a listener release the body, asking nothing more for it", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 3 }]);
    h.store.onRevision((body) => {
      h.store.release(body);
    });
    await answer(h, [block(50, 1)], 2, cover([0, 1, 9]));
    expect(h.socket().requestsOfKind("surface_field")).toHaveLength(1);
    expect(h.store.status(MOON)).toBeNull();
    expect(fieldLog(h.workers[0] ?? new ScriptedHeightWorker())).toEqual([
      `init ${MOON} 50 #1`,
      `release ${MOON}`,
    ]);
  });

  it("reports a listener that throws, and still tells the others and keeps the field", async () => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const h = harness(1);
    const told: number[] = [];
    h.store.onRevision(() => {
      throw new Error("a broken listener");
    });
    const off = h.store.onRevision((_, coverage) => told.push(coverage.revision ?? -1));
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1)], 1, cover([0, 1, 9]));
    expect(told).toEqual([1]);
    expect(errors).toHaveBeenCalledOnce();
    expect(h.store.status(MOON)?.failure).toBeNull();
    expect(h.store.coverage(MOON)?.revision).toBe(1);
    off();
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    await answer(h, [block(40, 2)], 2, cover([1, 2, 9]));
    expect(told).toEqual([1]);
  });

  it("refuses a field of a level R09 does not serve, or one whose level moved", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    const socket = h.socket();
    const first = latestAsk(socket).id;
    socket.serverResponds(first, {
      kind: "surface_field",
      state: "ready",
      header: { ...HEADER, level: 9 },
      revision: 1,
      coverage: cover(),
      bulk: sendFrames(socket, first, block(50, 1), 64),
    });
    await settle();
    expect(h.store.status(MOON)?.failure).toMatch(/level 9 is not 5 to 8/u);
    // Nothing was held before, so the workers drop the payload's blocks too.
    expect(fieldLog(h.workers[0] ?? new ScriptedHeightWorker())).toEqual([
      `init ${MOON} 50 #1`,
      `release ${MOON}`,
    ]);
    h.store.release(MOON);
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1)], 1, cover([0, 1, 9]));
    h.store.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    const second = latestAsk(socket).id;
    socket.serverResponds(second, {
      kind: "surface_field",
      state: "ready",
      header: { ...HEADER, level: 6 },
      revision: 2,
      coverage: cover(),
      bulk: sendFrames(socket, second, block(40, 2), 64),
    });
    await settle();
    expect(h.store.status(MOON)?.failure).toMatch(/moved from 5 to 6/u);
    // The field held stays, at its revision.
    expect(h.store.coverage(MOON)?.revision).toBe(1);
  });

  it("refuses an answer whose coverage is not a run of the field's cells", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1)], 1, cover([0, 7_000, 9]));
    expect(h.store.status(MOON)?.failure).toMatch(/beyond level 5/u);
    expect(h.store.coverage(MOON)).toBeNull();
  });

  it("gives a payload up whose frame is malformed, cancelling its request", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    const { id } = latestAsk(h.socket());
    const bad = block(40, 1);
    bad[1] = 0;
    h.socket().serverSendsBinary(binaryFrame(id, 0, 1, [...bad]));
    await settle();
    expect(h.socket().cancelledIds()).toEqual([id]);
    expect(h.store.status(MOON)).toMatchObject({
      pending: false,
      failure: expect.stringMatching(/HYSF/u),
    });
    expect(h.workers[0]?.fieldMessages).toEqual([]);
  });

  it("refuses an answer whose payload ends inside a block", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1), block(40, 2).subarray(0, 20)], 1, cover([0, 1, 9]));
    expect(h.store.status(MOON)?.failure).toMatch(/inside a block/u);
    expect(h.store.coverage(MOON)).toBeNull();
  });

  it("refuses an answer whose header is out of range, holding why", async () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    const socket = h.socket();
    const { id } = latestAsk(socket);
    socket.serverResponds(id, {
      kind: "surface_field",
      state: "ready",
      header: { ...SEA_HEADER, maturity: 2 },
      revision: 1,
      coverage: cover([0, 1, 9]),
      bulk: sendFrames(socket, id, block(50, 1), 64),
    });
    await settle();
    expect(h.store.status(MOON)?.failure).toMatch(/maturity, 2, is not from 0 to 1/u);
    expect(h.store.coverage(MOON)).toBeNull();
  });

  it("drops a body's field that a worker refuses, holding the refusal", async () => {
    const h = harness(2);
    h.store.want(MOON, "local-body");
    await answer(h, [block(50, 1)], 1, cover([0, 1, 9]));
    const worker = h.workers[1] ?? new ScriptedHeightWorker();
    const init = worker.fieldMessages[0];
    if (init === undefined) {
      throw new Error("no field-init was posted");
    }
    worker.reply({
      kind: "field-refused",
      id: init.id,
      body: MOON,
      message: "a block of another generator version",
      heldBytes: 0,
    });
    expect(h.store.status(MOON)?.failure).toMatch(/another generator version/u);
    expect(h.store.coverage(MOON)).toBeNull();
    expect(h.pool.fieldBodies).toEqual([]);
  });

  it("releases every body and its requests when disposed", () => {
    const h = harness(1);
    h.store.want(MOON, "local-body");
    h.store.dispose();
    expect(h.socket().cancelledIds()).toHaveLength(1);
    expect(() => {
      h.store.want(MOON, "local-body");
    }).toThrow(/disposed/u);
  });
});
