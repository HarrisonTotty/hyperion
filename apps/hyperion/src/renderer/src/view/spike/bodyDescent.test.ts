import type { BodyIdHex } from "@hyperion/protocol";
import { afterEach, describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { viewId } from "../camera/state";
import { terrainTestGolden } from "../scenes/terrainScene";
import type { FieldFailure } from "../terrain/coarseFieldStore";
import { CoverageView, fieldCellIndex } from "../terrain/coverage";
import { finestLevel, vertexDir } from "../terrain/cube";
import {
  cornerNeighbours,
  EDGES,
  edgeNeighbour,
  MAX_LEVEL,
  type PatchKey,
  patchKeyString,
} from "../terrain/patchKey";
import { LEVEL_TABLE_STRIDE, levelBoundM, planetGeometry } from "../terrain/planet";
import type { TerrainSurvey, TerrainViewReport } from "../terrain/session";
import type { BakedPatch, BakeSettings, SurfaceGroundReadout } from "../terrain/workers/messages";
import type { BakeFailure, HeightWorkerLane } from "../terrain/workers/pool";
import {
  type BodyDescentSession,
  type BodySite,
  offSurveyCeilingM,
  bodySiteOf,
  CEILING_SLACK_M,
  floorsOf,
  holdBodyField,
  keyBakeOf,
  keyWhollyCovered,
  measureBodyDescent,
  NoLandingSite,
  planFloors,
  siteCandidates,
  siteGroundOf,
  SURVEYED_VERTEX,
} from "./bodyDescent";
import { patchKeyAt, stretchKeys } from "./demandRecord";
import { DescentProfile, DescentUnclearable, trackStretches } from "./descentProfile";
import { DescentRefused, SPIKE_VIEW_NAMES } from "./spikeRun";
import { terrainTestWorld } from "./spikeWorld";

const golden = terrainTestGolden();
const world = terrainTestWorld();
const FINEST = finestLevel(world.figure.equatorialRadiusM);
const VIEW = viewId(SPIKE_VIEW_NAMES.main);

/** The fixture's coverage at revision 2, the whole field's answer (payload 3). */
function fixtureCoverage(): CoverageView {
  const payload = golden.payloads[2];
  if (payload === undefined) {
    throw new Error("the terrain golden has no third payload");
  }
  const merged = CoverageView.empty(golden.header.level).merged(payload.coverage, payload.revision);
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

const COVERAGE = fixtureCoverage();
const CANDIDATES = siteCandidates(COVERAGE);

/** A coverage of the field's level surveying `cells` alone. */
function coverageOf(cells: ReadonlyArray<number>): CoverageView {
  const merged = CoverageView.empty(golden.header.level).merged(
    { ranges: cells.map((start) => ({ start, end: start + 1, resolution: 100 })) },
    1,
  );
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** Whether `cell`'s 3 × 3 block is surveyed in the fixture's coverage. */
function blockSurveyed(cell: PatchKey): boolean {
  const block = [
    cell,
    ...EDGES.map((edge) => edgeNeighbour(cell, edge)),
    ...cornerNeighbours(cell).filter((corner) => corner !== null),
  ];
  return block.every((each) => COVERAGE.resolutionAtM(each) !== null);
}

/** A key of `level` under the fixture's cell `index`, offset `(di, dj)` from its corner. */
function keyIn(index: number, level: number, di = 0, dj = 0): PatchKey {
  const cell = CANDIDATES.find((each) => fieldCellIndex(each) === index) ?? {
    face: 0,
    level: golden.header.level,
    i: 0,
    j: 0,
  };
  const shift = 2 ** (level - cell.level);
  return { face: cell.face, level, i: cell.i * shift + di, j: cell.j * shift + dj };
}

/** A level table of 25 rows: ε_n = 1,000 ÷ 2ⁿ m, heights −2 km to `highM(n)`, spacing 1 m. */
function tableOf(highM: (level: number) => number): Float64Array {
  const table = new Float64Array(LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1));
  for (let n = 0; n <= MAX_LEVEL; n += 1) {
    table.set([1000 / 2 ** n, -2000, highM(n), 1], LEVEL_TABLE_STRIDE * n);
  }
  return table;
}

const TABLE = tableOf((n) => 3000 - n);

/** A bake of `key`, flat at `heightM` with a ±10 m range, every vertex surveyed unless `masked`. */
function bakeOf(key: PatchKey, heightM: number, masked = false): BakedPatch {
  const surveyMask = new Uint8Array(65 * 65).fill(SURVEYED_VERTEX);
  if (masked) {
    surveyMask[64] = 0;
  }
  return {
    key,
    generation: 0,
    originM: vec3(0, 0, 0),
    heights: new Float32Array(2 * 65 * 65).fill(heightM),
    offsets: null,
    normals: new Float16Array(2),
    heightRangeM: [heightM - 10, heightM + 10],
    boundingRadiusM: 1,
    originHeightM: heightM,
    skirtDepthM: 1,
    layers: {
      palette: new Uint8Array(16),
      weights: new Uint8Array(0),
      surveyMask,
      fieldLevel: golden.header.level,
      boundM: 0,
      sigmaM: 0,
      complete: !masked,
      unsurveyedCells: new Uint32Array(0),
      horizonNear: new Uint32Array(0),
      horizonFar: new Uint32Array(0),
      horizonComplete: true,
      horizonRefusedCells: new Uint32Array(0),
      photometry: new Float32Array(40),
    },
  };
}

describe("the landing site's candidates", () => {
  it("are the fixture's cells whose whole 3 × 3 block is surveyed, in rising index", () => {
    const indices = CANDIDATES.map(fieldCellIndex);
    expect(indices).toEqual(indices.toSorted((a, b) => a - b));
    expect(indices.length).toBeGreaterThan(0);
    expect(CANDIDATES.every(blockSurveyed)).toBe(true);
  });

  it("are none where no block is whole, and none with no survey", () => {
    expect(siteCandidates(coverageOf([4061, 4063]))).toEqual([]);
    expect(siteCandidates(CoverageView.empty(golden.header.level))).toEqual([]);
  });
});

describe("the landing site", () => {
  const site = bodySiteOf(CANDIDATES, 7n, 0, FINEST);
  if (site === null) {
    throw new Error("the fixture has candidates");
  }

  it("is drawn from the seed alone", () => {
    expect(bodySiteOf(CANDIDATES, 7n, 0, FINEST)).toEqual(site);
    expect(bodySiteOf(CANDIDATES, 1n, 0, FINEST)?.cell).not.toEqual(site.cell);
  });

  it("lies in a candidate cell whose 3 × 3 block is surveyed, seed 7's among them", () => {
    expect(CANDIDATES).toContainEqual(site.cell);
    expect(blockSurveyed(site.cell)).toBe(true);
    const shift = 2 ** (FINEST - site.cell.level);
    expect([Math.floor(site.key.i / shift), Math.floor(site.key.j / shift)]).toEqual([
      site.cell.i,
      site.cell.j,
    ]);
    expect(site.key.face).toBe(site.cell.face);
  });

  it("is the centre vertex of a finest patch, which the profile's site direction reproduces", () => {
    expect(site.key.level).toBe(FINEST);
    expect(site.dir).toEqual(vertexDir(site.key, 32, 32));
    const profile = new DescentProfile(world.figure, site.site);
    const d = profile.siteDir;
    expect(Math.abs(d.x - site.dir[0])).toBeLessThan(1e-15);
    expect(Math.abs(d.y - site.dir[1])).toBeLessThan(1e-15);
    expect(Math.abs(d.z - site.dir[2])).toBeLessThan(1e-15);
    expect(patchKeyAt([d.x, d.y, d.z], FINEST)).toEqual(site.key);
  });

  it("moves to the next candidate in order at the next attempt, wrapping", () => {
    const n = CANDIDATES.length;
    const next = bodySiteOf(CANDIDATES, 7n, 1, FINEST);
    expect(next?.candidate).toBe((site.candidate + 1) % n);
    expect(bodySiteOf(CANDIDATES, 7n, n, FINEST)?.cell).toEqual(site.cell);
  });

  it("is none without a candidate", () => {
    expect(bodySiteOf([], 7n, 0, FINEST)).toBeNull();
  });

  it("keeps the track over surveyed ground from the low fast pass to touchdown", () => {
    // decision-r10-t14c-leans §4: every 64 Hz pose over a surveyed cell, and every key of the low
    // pass, the slowdown's pieces and the final approach wholly covered, for seed 7 and seven more.
    const off: string[] = [];
    for (const seed of [7n, 0n, 1n, 2n, 3n, 4n, 5n, 6n]) {
      const drawn = bodySiteOf(CANDIDATES, seed, 0, FINEST);
      if (drawn === null) {
        throw new Error("the fixture has candidates");
      }
      const datum = new DescentProfile(world.figure, drawn.site);
      const from = datum.segmentSpans().find(({ name }) => name === "low fast pass")?.startS ?? 0;
      for (let n = Math.round(from * 64); n <= Math.round(datum.durationS * 64); n += 1) {
        const g = datum.groundDirAt(n / 64);
        if (COVERAGE.resolutionAtM(patchKeyAt([g.x, g.y, g.z], COVERAGE.level)) === null) {
          off.push(`seed ${seed} at ${n / 64} s`);
        }
      }
      for (const stretch of trackStretches(datum)) {
        if (stretch.segment !== "low fast pass" && stretch.segment !== "slowdown") {
          continue;
        }
        for (const key of stretchKeys(datum, stretch)) {
          if (!keyWhollyCovered(COVERAGE, key)) {
            off.push(`seed ${seed}, ${stretch.piece}: ${patchKeyString(key)}`);
          }
        }
      }
    }
    expect(off).toEqual([]);
  });
});

describe("the floors", () => {
  const inside = keyIn(4061, 14, 3, 5);
  const beside = keyIn(4061, 14, 4, 5);
  const offSurvey: PatchKey = { face: 3, level: 14, i: 100, j: 100 };

  it("bake no patch that has an unsurveyed cell under it, and no stretch with one", () => {
    // A level-4 patch spans 16 of the field's cells, some unsurveyed.
    const coarse: PatchKey = { face: 0, level: 4, i: 15, j: 14 };
    expect(keyWhollyCovered(COVERAGE, offSurvey)).toBe(false);
    expect(keyWhollyCovered(COVERAGE, coarse)).toBe(false);
    expect(keyWhollyCovered(COVERAGE, inside)).toBe(true);
    const plan = planFloors([[inside, beside], [inside, offSurvey], [coarse]], COVERAGE);
    expect(plan.stretches).toEqual([[inside, beside], null, null]);
    expect(plan.keys).toEqual([inside, beside]);
  });

  it("take the highest bake plus ε_n over wholly surveyed ground, else the ceiling", () => {
    const planet = planetGeometry(world.figure, TABLE);
    const masked = keyIn(4061, 14, 5, 5);
    const plan = {
      stretches: [[inside, beside], null, [inside, masked]],
      keys: [inside, beside, masked],
    };
    const bakes = new Map([
      [patchKeyString(inside), keyBakeOf(bakeOf(inside, 1_900))],
      [patchKeyString(beside), keyBakeOf(bakeOf(beside, 1_950))],
      [patchKeyString(masked), keyBakeOf(bakeOf(masked, 1_800, true))],
    ]);
    const levels = [{ level: 14 }, { level: 14 }, { level: 16 }];
    expect(floorsOf(levels, plan, bakes, planet, 20_000)).toEqual([
      { floorM: 1_960 + levelBoundM(planet, 14), source: "baked" },
      { floorM: 20_000, source: "ceiling" },
      { floorM: 20_000, source: "ceiling" },
    ]);
    expect(() => floorsOf(levels, plan, new Map(), planet, 20_000)).toThrow(/was not baked/);
  });

  it("set the ceiling at the table's highest row, plus the relief and a metre", () => {
    const planet = planetGeometry(
      world.figure,
      tableOf((n) => (n === 11 ? 12_345.5 : 1_000)),
    );
    expect(offSurveyCeilingM(planet, { realised_relief_m: 15_812.216 })).toBe(
      12_345.5 + 15_812.216 + CEILING_SLACK_M,
    );
  });

  it("set the ceiling at +∞ where no cell is held, every row's lowest and highest exactly 0", () => {
    // `level_table`'s rows before any cell: their bounds and spacings, the heights 0 and 0.
    const noCell = new Float64Array(LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1));
    for (let n = 0; n <= MAX_LEVEL; n += 1) {
      noCell.set([1000 / 2 ** n, 0, 0, 1], LEVEL_TABLE_STRIDE * n);
    }
    // One row's lowest below the datum is a cell held, though every highest is 0.
    const oneCell = Float64Array.from(noCell);
    oneCell[LEVEL_TABLE_STRIDE * 7 + 1] = -0.5;
    const relief = { realised_relief_m: 15_812.216 };
    expect([
      offSurveyCeilingM(planetGeometry(world.figure, noCell), relief),
      offSurveyCeilingM(planetGeometry(world.figure, null), relief),
      offSurveyCeilingM(planetGeometry(world.figure, oneCell), relief),
    ]).toEqual([Infinity, Infinity, 15_812.216 + CEILING_SLACK_M]);
  });

  it("read a bake's highest height and survey, and the site's vertex and its neighbours", () => {
    const bake = bakeOf(inside, 1_900);
    expect(keyBakeOf(bake)).toEqual({ highestM: 1_910, wholly: true });
    expect(keyBakeOf(bakeOf(inside, 1_900, true)).wholly).toBe(false);
    bake.heights[2 * (65 * 32 + 32)] = 1_901.5;
    bake.heights[2 * (65 * 33 + 31)] = 1_899;
    const ground = siteGroundOf(bake);
    expect(ground.heightM).toBe(1_901.5);
    expect(ground.surveyed).toBe(true);
    expect(ground.rangeM).toEqual([1_890, 1_910]);
    expect(ground.neighbourhoodM).toEqual([1900, 1900, 1900, 1900, 1901.5, 1900, 1899, 1900, 1900]);
  });
});

/** How the stand-in session's lane answers its demand, set by a test before it measures. */
interface LaneScript {
  /** The bake it lands for `key`: flat ground at 1,900 m by default. */
  bake: (key: PatchKey) => BakedPatch;
  /** The keys, by `patchKeyString`, whose bakes it fails ({@link LANE_FAILURE}) instead. */
  failing: ReadonlySet<string>;
  /** Whether every height worker is lost, so that it bakes nothing. */
  unavailable: boolean;
  /** How many patches wait for a block of the field, so that it bakes nothing. */
  waitingCount: number;
}

/** What the stand-in's lane says of a bake it fails. */
const LANE_FAILURE = "the module trapped";

/**
 * A session over the fixture's coverage whose lane bakes flat ground at 1,900 m at once, or as its
 * {@link LaneScript} says.
 */
class StubSession implements BodyDescentSession {
  readonly reports: Array<readonly [string, TerrainViewReport]> = [];
  readonly requested: PatchKey[] = [];
  readonly lanes: Array<{ readonly settings: BakeSettings; terminated: boolean }> = [];
  steps = 0;
  /** Steps after which the field is held. */
  heldAfter = 0;
  coverage: CoverageView = COVERAGE;
  /** The workers' level table once the field is held. */
  table: Float64Array = TABLE;
  /** The store's failure of the field, if it failed. */
  failure: FieldFailure | null = null;
  readonly script: LaneScript = {
    bake: (key) => bakeOf(key, 1_900),
    failing: new Set(),
    unavailable: false,
    waitingCount: 0,
  };
  readonly readouts: Array<Parameters<BodyDescentSession["groundReadout"]>> = [];

  noteView(id: string, report: TerrainViewReport): void {
    this.reports.push([id, report]);
  }

  step(): void {
    this.steps += 1;
  }

  #held(): boolean {
    return this.steps >= this.heldAfter;
  }

  survey(body: BodyIdHex): TerrainSurvey | null {
    if (body !== golden.body) {
      return null;
    }
    const held = this.#held();
    return {
      status: { reason: "local-body", pending: !held, notModelled: null, failure: this.failure },
      coverage: held ? this.coverage : null,
      header: held ? golden.header : null,
      stale: false,
    };
  }

  levels(body: BodyIdHex): Float64Array | null {
    return body === golden.body && this.#held() ? this.table : null;
  }

  detailSeed(body: BodyIdHex): typeof golden.seed | null {
    return body === golden.body && this.#held() ? golden.seed : null;
  }

  lane(settings: BakeSettings): HeightWorkerLane {
    const record = { settings, terminated: false };
    this.lanes.push(record);
    const baked = new Set<(bake: BakedPatch) => void>();
    const failed = new Set<(failure: BakeFailure) => void>();
    const { requested, script } = this;
    return {
      settings,
      currentGeneration: 0,
      queuedCount: 0,
      get waitingCount(): number {
        return script.waitingCount;
      },
      get unavailable(): boolean {
        return script.unavailable;
      },
      get terminated(): boolean {
        return record.terminated;
      },
      request: () => {
        throw new Error("the measurement asks by reprioritise");
      },
      reprioritise: (demand) => {
        for (const { key } of demand) {
          requested.push(key);
          if (script.unavailable || script.waitingCount > 0) {
            continue;
          }
          const keyString = patchKeyString(key);
          if (script.failing.has(keyString)) {
            for (const listener of failed) {
              listener({ keyString, message: LANE_FAILURE });
            }
            continue;
          }
          for (const listener of baked) {
            listener(script.bake(key));
          }
        }
      },
      cancelStale: () => undefined,
      onBaked: (cb) => {
        baked.add(cb);
        return () => baked.delete(cb);
      },
      onFailed: (cb) => {
        failed.add(cb);
        return () => failed.delete(cb);
      },
      terminate: () => {
        record.terminated = true;
      },
    };
  }

  /** The readout's answer at the site: `ground`, the reading below, or one the test gives. */
  readoutAnswer: SurfaceGroundReadout | null | "ground" = "ground";

  groundReadout(
    ...args: Parameters<BodyDescentSession["groundReadout"]>
  ): Promise<SurfaceGroundReadout | null> {
    this.readouts.push(args);
    if (this.readoutAnswer !== "ground") {
      return Promise.resolve(this.readoutAnswer);
    }
    return Promise.resolve({
      kind: "ground",
      sight: {
        rangeM: 999.9,
        sigmaM: 0.5,
        resolved: false,
        hitM: [0, 0, 0],
        at: { face: 0, u: 0, v: 0 },
      },
      reading: {
        elevationM: 1_900.1,
        elevationSigmaM: 0.56,
        slopeRmsRad: 0.1,
        slopeSigmaRad: 0.01,
        slopeBaselineM: 2,
        resolutionM: 20,
        resolved: false,
      },
      material: { classCode: 0, entry: 0 },
      properties: {
        bearingCapacityPa: null,
        capacitySlopeRad: 0.1,
        padFriction: null,
        frictionOnTemperature: false,
        temperatureK: null,
      },
    });
  }
}

/** A clock that moves on by each wait, so that a timeout needs no real time. */
function fakeClock(): {
  readonly nowMs: () => number;
  readonly wait: (ms: number) => Promise<void>;
} {
  let now = 0;
  return {
    nowMs: () => now,
    wait: (ms) => {
      now += ms;
      return Promise.resolve();
    },
  };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("holding the field", () => {
  it("notes a view in the moon's frame, and steps the session until it holds the field", async () => {
    const session = new StubSession();
    session.heldAfter = 3;
    const held = await holdBodyField(session, world, {
      viewId: VIEW,
      revision: 2,
      timeoutMs: 1_000,
      ...fakeClock(),
    });
    expect(held).toEqual({
      coverage: COVERAGE,
      header: golden.header,
      levels: TABLE,
      detailSeed: golden.seed,
    });
    expect(session.steps).toBe(3);
    const [id, report] = session.reports[0] ?? [];
    expect(id).toBe(VIEW);
    expect(report?.pose.frame).toEqual({ kind: "body", body: golden.body });
    expect(report?.readoutBody).toBe(golden.body);
    expect(report?.regimes.size).toBe(0);
  });

  it("gives up after its timeout, naming what the session lacks", async () => {
    const session = new StubSession();
    session.heldAfter = Infinity;
    await expect(
      holdBodyField(session, world, { viewId: VIEW, revision: 2, timeoutMs: 100, ...fakeClock() }),
    ).rejects.toThrow(/not held within 100 ms: its coverage at revision 2.*a settled level table/);
  });

  it("waits for the revision asked", async () => {
    const session = new StubSession();
    session.coverage = coverageOf([4061]);
    await expect(
      holdBodyField(session, world, { viewId: VIEW, revision: 2, timeoutMs: 50, ...fakeClock() }),
    ).rejects.toThrow(/revision 2 \(1 held\)/);
  });

  it("stops at once when aborted, with the abort's reason, its timer cleared", async () => {
    vi.useFakeTimers();
    const session = new StubSession();
    session.heldAfter = Infinity;
    const controller = new AbortController();
    const closed = new Error("the spike was closed");
    const held = holdBodyField(session, world, {
      viewId: VIEW,
      revision: 2,
      timeoutMs: 60_000,
      pollMs: 1_000,
      nowMs: () => 0,
      signal: controller.signal,
    });
    // The first look, then its wait's timer.
    expect(vi.getTimerCount()).toBe(1);
    controller.abort(closed);
    await expect(held).rejects.toBe(closed);
    expect(vi.getTimerCount()).toBe(0);
    expect(session.steps).toBe(1);
  });
});

/** A stand-in session whose lane's first demand aborts `controller` with `reason`. */
class AbortingSession extends StubSession {
  constructor(
    readonly controller: AbortController,
    readonly reason: Error,
  ) {
    super();
  }

  override lane(settings: BakeSettings): HeightWorkerLane {
    const made = super.lane(settings);
    return {
      ...made,
      reprioritise: (demand) => {
        this.controller.abort(this.reason);
        made.reprioritise(demand);
      },
    };
  }
}

describe("the measurement over a stand-in session", () => {
  it("bakes the site and the surveyed stretches alone, and flies the profile over their floors", async () => {
    const session = new StubSession();
    const measured = await measureBodyDescent(session, world, 7n, {
      viewId: VIEW,
      timeoutMs: 1_000,
      ...fakeClock(),
    });
    const [lane] = session.lanes;
    expect(lane?.settings).toEqual({
      vertexPath: "face-differences",
      normals: "mesh",
      source: { kind: "body", body: golden.body, detailSeed: golden.seed, weights: "skipped" },
    });
    expect(lane?.terminated).toBe(true);
    expect(session.requested[0]).toEqual(measured.site.key);
    expect(session.requested.every((key) => keyWhollyCovered(COVERAGE, key))).toBe(true);
    expect([...measured.bakes.keys()].toSorted()).toEqual(
      [...new Set(session.requested.map(patchKeyString))].toSorted(),
    );
    expect(measured.siteHeightM).toBe(1_900);
    const planet = planetGeometry(world.figure, TABLE);
    const ceilingM = offSurveyCeilingM(planet, golden.header);
    expect(measured.ceilingM).toBe(ceilingM);
    // Baked where the coverage shows every patch of the stretch on surveyed cells: the stand-in's
    // flat 1,910 m highest plus ε_n; the ceiling elsewhere, every level-4 stretch among them.
    const datum = new DescentProfile(world.figure, measured.site.site);
    expect(measured.floors).toEqual(
      measured.stretches.map((stretch) =>
        stretchKeys(datum, stretch).every((key) => keyWhollyCovered(COVERAGE, key))
          ? { floorM: 1_910 + levelBoundM(planet, stretch.level), source: "baked" }
          : { floorM: ceilingM, source: "ceiling" },
      ),
    );
    const coarse = measured.floors.filter((_, k) => measured.stretches[k]?.level === 4);
    expect(coarse.length).toBeGreaterThan(0);
    expect(coarse.every(({ source }) => source === "ceiling")).toBe(true);
    expect(measured.stretchMaxHeightsM).toEqual(measured.floors.map(({ floorM }) => floorM));
    expect(measured.profile.minFloorMarginM).toBeGreaterThanOrEqual(-1e-6);
    expect(measured.siteReading).toEqual({
      kind: "ground",
      elevationM: 1_900.1,
      sigmaM: 0.56,
      resolved: false,
    });
    expect(measured.siteSlantRangeM).toBe(999.9);
    expect([...measured.omittedSigmaM]).toEqual(Array.from({ length: 25 }, () => Infinity));
    expect(measured.refused).toEqual([]);
    // The readout looks straight down from 1 km above the site.
    const [, body, sight] = session.readouts[0] ?? [];
    expect(body).toBe(golden.body);
    expect(sight?.maxM).toBe(2_000);
  });

  it("terminates its lane when aborted, rejecting with the abort's reason", async () => {
    const controller = new AbortController();
    const closed = new Error("the spike was closed");
    const session = new AbortingSession(controller, closed);
    await expect(
      measureBodyDescent(session, world, 7n, {
        viewId: VIEW,
        timeoutMs: 1_000,
        ...fakeClock(),
        signal: controller.signal,
      }),
    ).rejects.toBe(closed);
    expect(session.lanes.map(({ terminated }) => terminated)).toEqual([true]);
    expect(session.readouts).toEqual([]);
  });

  it.each([
    [{ kind: "not-surveyed" }, { kind: "not-surveyed" }],
    [{ kind: "no-ground" }, { kind: "no-ground" }],
    [null, null],
  ] as const)(
    "reads the readout's own kind at the site, %j, with no slant range",
    async (answer, reading) => {
      const session = new StubSession();
      session.readoutAnswer = answer;
      const measured = await measureBodyDescent(session, world, 7n, {
        viewId: VIEW,
        timeoutMs: 1_000,
        ...fakeClock(),
      });
      expect([measured.siteReading, measured.siteSlantRangeM]).toEqual([reading, null]);
    },
  );

  it("refuses to draw a site where nothing is surveyed, baking nothing", async () => {
    const session = new StubSession();
    session.coverage = CoverageView.empty(golden.header.level);
    await expect(
      measureBodyDescent(session, world, 7n, { viewId: VIEW, timeoutMs: 100, ...fakeClock() }),
    ).rejects.toThrow(NoLandingSite);
    expect(session.lanes).toEqual([]);
  });
});

/** What `promise` rejects with, which must be an `Error`; a fulfilment fails the test. */
async function rejectionOf(promise: Promise<unknown>): Promise<Error> {
  try {
    await promise;
  } catch (error: unknown) {
    if (error instanceof Error) {
      return error;
    }
    throw new Error("the promise rejected with a value that is not an Error", { cause: error });
  }
  throw new Error("the promise fulfilled where it should have rejected");
}

/** Seed 7's site at `attempt` among the fixture's candidates. */
function siteAt(attempt: number): BodySite {
  const site = bodySiteOf(CANDIDATES, 7n, attempt, FINEST);
  if (site === null) {
    throw new Error("the fixture has candidates");
  }
  return site;
}

/** Whether `a` and `b` are the same patch. */
function sameKey(a: PatchKey, b: PatchKey): boolean {
  return patchKeyString(a) === patchKeyString(b);
}

/** `bake` with its centre vertex, the site's, off the survey. */
function unsurveyedAtCentre(bake: BakedPatch): BakedPatch {
  if (bake.layers === null) {
    throw new Error("a body's bake has layers");
  }
  const surveyMask = Uint8Array.from(bake.layers.surveyMask);
  surveyMask[65 * 32 + 32] = 0;
  return { ...bake, layers: { ...bake.layers, surveyMask, complete: false } };
}

/** Measures seed 7's descent over `session` on the fake clock. */
function measure(session: StubSession): Promise<unknown> {
  return measureBodyDescent(session, world, 7n, { viewId: VIEW, timeoutMs: 1_000, ...fakeClock() });
}

/** The lanes' standing after a measurement: one, terminated. */
function lanesTerminated(session: StubSession): boolean[] {
  return session.lanes.map(({ terminated }) => terminated);
}

describe("the measurement's failures over a stand-in session", () => {
  it("rejects while holding the field once the store's ask fails, naming the server's message", async () => {
    const session = new StubSession();
    session.heldAfter = Infinity;
    session.failure = {
      kind: "rejected",
      code: "unknown_body",
      message: "no such body",
      refusal: true,
    };
    const error = await rejectionOf(
      holdBodyField(session, world, { viewId: VIEW, revision: 2, timeoutMs: 100, ...fakeClock() }),
    );
    expect([error.name, error.message, session.steps]).toEqual([
      "Error",
      `the field of ${golden.body} failed: no such body`,
      1,
    ]);
  });

  it("rejects the measurement of a field whose answer failed a check, making no lane", async () => {
    const session = new StubSession();
    session.failure = { kind: "invalid", check: "header unusable" };
    const error = await rejectionOf(measure(session));
    expect([error.name, error.message, session.lanes]).toEqual([
      "Error",
      `the field of ${golden.body} failed: header unusable`,
      [],
    ]);
  });

  it("rejects naming a bake its lane failed, and terminates the lane", async () => {
    const session = new StubSession();
    const siteKey = patchKeyString(siteAt(0).key);
    session.script.failing = new Set([siteKey]);
    const error = await rejectionOf(measure(session));
    expect([error.name, error.message, lanesTerminated(session)]).toEqual([
      "Error",
      `patch ${siteKey} could not be baked: ${LANE_FAILURE}`,
      [true],
    ]);
  });

  it("rejects where taking a bake throws, which the pool alone would pass over, and terminates the lane", async () => {
    // A bake without a body's layers, which the floors cannot read.
    const session = new StubSession();
    const site = siteAt(0);
    session.script.bake = (key) =>
      sameKey(key, site.key) ? { ...bakeOf(key, 1_900), layers: null } : bakeOf(key, 1_900);
    const error = await rejectionOf(measure(session));
    expect([error.name, error.message, lanesTerminated(session)]).toEqual([
      "Error",
      `patch ${patchKeyString(site.key)} was baked without a body's layers`,
      [true],
    ]);
  });

  it("rejects once every height worker is lost, and terminates the lane", async () => {
    const session = new StubSession();
    session.script.unavailable = true;
    const error = await rejectionOf(measure(session));
    expect([error.name, error.message, lanesTerminated(session)]).toEqual([
      "Error",
      "every height worker was lost",
      [true],
    ]);
  });

  it("rejects where patches wait for a block of a field held whole, and terminates the lane", async () => {
    const session = new StubSession();
    session.script.waitingCount = 3;
    const error = await rejectionOf(measure(session));
    expect([error.name, error.message, lanesTerminated(session)]).toEqual([
      "Error",
      "3 patches wait for a block of a field that is held whole",
      [true],
    ]);
  });

  it("refuses a site whose vertex its bake finds off the survey, trying no other, and terminates the lane", async () => {
    const session = new StubSession();
    const site = siteAt(0);
    session.script.bake = (key) =>
      sameKey(key, site.key) ? unsurveyedAtCentre(bakeOf(key, 1_900)) : bakeOf(key, 1_900);
    const error = await rejectionOf(measure(session));
    expect([
      error instanceof NoLandingSite,
      error.message,
      session.requested.some((key) => sameKey(key, siteAt(1).key)),
      lanesTerminated(session),
    ]).toEqual([
      true,
      `no landing site on ${golden.body}: the site's vertex in ${patchKeyString(site.key)} is not surveyed`,
      false,
      [true],
    ]);
  });
});

/**
 * A floor the profile cannot clear, m: 10²⁰, where a double's step is 16 km, so that the lifts'
 * tenth of a metre and the clearances above it are lost to rounding (decision-r05-descent-clearance
 * rule 4's four lifts all fall short).
 */
const UNCLEARABLE_M = 1e20;

describe("the measurement's refusals over a stand-in session", () => {
  it("moves on to the next candidate where the floors refuse one, keeping its bakes", async () => {
    // The first site's low fast pass on ground it cannot clear; the next site's track is elsewhere.
    const first = siteAt(0);
    const datum = new DescentProfile(world.figure, first.site);
    const stretches = trackStretches(datum);
    const lowPass = stretches.find(({ segment }) => segment === "low fast pass");
    if (lowPass === undefined) {
      throw new Error("the track has a low fast pass");
    }
    const unclearable = new Set(stretchKeys(datum, lowPass).map(patchKeyString));
    const firstKeys = [
      first.key,
      ...planFloors(
        stretches.map((stretch) => stretchKeys(datum, stretch)),
        COVERAGE,
      ).keys,
    ];
    const session = new StubSession();
    session.script.bake = (key) =>
      bakeOf(key, unclearable.has(patchKeyString(key)) ? UNCLEARABLE_M : 1_900);
    const measured = await measureBodyDescent(session, world, 7n, {
      viewId: VIEW,
      timeoutMs: 1_000,
      ...fakeClock(),
    });
    const asked = session.requested.map(patchKeyString);
    expect([
      measured.refused.map(({ cell }) => cell),
      measured.refused.every(({ message }) => message.startsWith("the descent cannot clear")),
      [measured.site.attempt, measured.site.cell],
      new Set(asked).size === asked.length,
      firstKeys.every((key) => measured.bakes.has(patchKeyString(key))),
      lanesTerminated(session),
    ]).toEqual([[first.cell], true, [1, siteAt(1).cell], true, true, [true]]);
  });

  it("refuses the descent once every candidate's floors refuse it, and terminates the lane", async () => {
    // Every candidate of the fixture's survey, under floors of 10²⁰ m.
    const session = new StubSession();
    session.table = tableOf(() => UNCLEARABLE_M);
    const candidates = siteCandidates(session.coverage);
    const error = await rejectionOf(measure(session));
    const sites = [0, 1].map((attempt) => bodySiteOf(candidates, 7n, attempt, FINEST)?.key);
    expect([
      error instanceof DescentRefused,
      error.message,
      error.cause instanceof DescentUnclearable,
      sites.map((key) =>
        session.requested.some((asked) => key !== undefined && sameKey(asked, key)),
      ),
      lanesTerminated(session),
    ]).toEqual([
      true,
      "the descent of seed 7 cannot clear its terrain",
      true,
      [true, true],
      [true],
    ]);
  });

  it("rethrows a profile's error other than DescentUnclearable, trying no other candidate", async () => {
    // No cell held: the ceiling is +∞, a floor the profile takes as a bug, not a refusal.
    const session = new StubSession();
    session.table = new Float64Array(LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1));
    const count = trackStretches(new DescentProfile(world.figure, siteAt(0).site)).length;
    const error = await rejectionOf(measure(session));
    expect([
      error instanceof RangeError,
      error instanceof DescentUnclearable,
      error.message,
      session.requested.some((key) => sameKey(key, siteAt(1).key)),
      lanesTerminated(session),
    ]).toEqual([
      true,
      false,
      `a descent needs ${String(count)} finite stretch floors, got ${String(count)}`,
      false,
      [true],
    ]);
  });
});
