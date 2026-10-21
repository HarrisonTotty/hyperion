import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { scale, type Vec3, vec3 } from "../../geometry/vec3";
import {
  bakePatch,
  initSync,
  SurfaceFields,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { InProcessHeightWorker } from "../../test/inProcessHeightWorker";
import { viewId } from "../camera/state";
import { keptTime } from "../scenes/kept";
import type { TerrainTestFixture } from "../scenes/terrainFixture";
import {
  TERRAIN_TEST_SCRIPT,
  TERRAIN_TEST_UNIVERSE,
  terrainSceneFixture,
  terrainTestGolden,
} from "../scenes/terrainScene";
import { CoverageView } from "../terrain/coverage";
import { PATCH_QUADS, uvToSt, type Xyz, xyzToFaceUv } from "../terrain/cube";
import {
  cornerNeighbours,
  EDGES,
  edgeNeighbour,
  type PatchKey,
  patchKeyString,
} from "../terrain/patchKey";
import {
  levelBoundM,
  type PlanetGeometry,
  planetGeometry,
  spheroidNormal,
  surfacePoint,
} from "../terrain/planet";
import { TerrainSession } from "../terrain/session";
import type { HeightModule } from "../terrain/workers/heightBake";
import {
  bakePatches,
  type BodyDescent,
  type BodySite,
  floorBakeSettings,
  floorsOf,
  type HeldBodyField,
  holdBodyField,
  type KeyBake,
  keyBakeOf,
  keyWhollyCovered,
  measureBodyDescent,
  offSurveyCeilingM,
  planFloors,
  type SiteGround,
  siteCandidates,
  siteGroundOf,
  bodySiteOf,
} from "./bodyDescent";
import { patchKeyAt, stretchKeys } from "./demandRecord";
import {
  DESCENT_SEGMENTS,
  DescentProfile,
  FLOOR_TOLERANCE_M,
  trackStretches,
} from "./descentProfile";
import { SPIKE_VIEW_NAMES } from "./spikeRun";
import { type SpikeWorld, terrainTestWorld } from "./spikeWorld";

/**
 * The descent's measurement over `TERRAIN TEST`'s moon (plan R10, R10.T14.c) on the real surface
 * module: the real terrain session over the fixture's client, its revision raised to 2 before the
 * first ask (so that the field comes whole, payload 3), and one in-process height worker.
 *
 * A bake costs 64 to 83 ms in-process (levels 12 to 17, 2026-10-20), and seed 7's whole measurement
 * bakes 1,584 patches, 110 s, so `just ci` runs its parts over the session: the field held, the
 * site and its track, the site's and the final approach's bakes, the reading and the ceiling; and
 * the whole measurement, the profile over its floors, runs only under `HYPERION_SLOW_TESTS=1`, as
 * `clearance.wasm.test.ts` gates its slower runs for `just test-slow-client`.
 */
const SLOW = import.meta.env["HYPERION_SLOW_TESTS"] === "1";

function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

const MODULE: HeightModule = {
  bakePatch,
  testPlanetVersion,
  createFields: () => new SurfaceFields(),
};

const VIEW = viewId(SPIKE_VIEW_NAMES.main);
const SEED = 7n;
const golden = terrainTestGolden();

/** The fixture's coverage at revision 2 as payload 3 carries it. */
function payloadCoverage(): CoverageView {
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

/** A session of its own over a fresh fixture, its link's surface events taken as `VIEW` takes them. */
interface Harness {
  readonly world: SpikeWorld;
  readonly fixture: TerrainTestFixture;
  readonly session: TerrainSession;
  readonly close: () => void;
}

async function openHarness(): Promise<Harness> {
  initSync({ module: wasmBytes() });
  const engine = await countingRenderEngine();
  const fixture = terrainSceneFixture();
  const link = fixture.open();
  // The revision rises before the session's first ask, so that it asks for the whole larger survey.
  fixture.notePlayed(TERRAIN_TEST_SCRIPT.deltaAtS);
  const session = new TerrainSession({
    engine,
    universe: TERRAIN_TEST_UNIVERSE,
    requests: link.requests,
    linkUp: true,
    createWorker: () => new InProcessHeightWorker(MODULE),
    workers: 1,
    setting: "high",
  });
  // As `useTerrainSession`'s `takeSurface`.
  const off = link.onSurface((event) => {
    switch (event.kind) {
      case "revisions":
        session.noteSurfaceRevisions(event.revisions);
        break;
      case "resync":
        session.resync();
        break;
    }
  });
  return {
    world: terrainTestWorld(),
    fixture,
    session,
    close: () => {
      off();
      session.dispose();
      link.close();
    },
  };
}

/**
 * The measurement's clock off the real one, as `bodyDescent.test.ts`' fake: each wait yields one
 * macrotask, which lets the fixture's and `InProcessHeightWorker`'s zero-delay replies through, and
 * moves the clock on by one poll, so that a timeout is a count of polls, not milliseconds.
 */
function pollClock(): {
  readonly pollMs: number;
  readonly nowMs: () => number;
  readonly wait: (ms: number) => Promise<void>;
} {
  let polls = 0;
  return {
    pollMs: 1,
    nowMs: () => polls,
    wait: (ms) => {
      polls += ms;
      return new Promise((resolve) => {
        setTimeout(resolve, 0);
      });
    },
  };
}

/** Whether `cell`'s 3 × 3 block is surveyed in `coverage`. */
function blockSurveyed(coverage: CoverageView, cell: PatchKey): boolean {
  return [
    cell,
    ...EDGES.map((edge) => edgeNeighbour(cell, edge)),
    ...cornerNeighbours(cell).filter((corner) => corner !== null),
  ].every((each) => coverage.resolutionAtM(each) !== null);
}

/** A triple's vector. */
function vecOf([x, y, z]: Xyz): Vec3 {
  return vec3(x, y, z);
}

/** The line of sight straight down onto the site from `aboveM` over its height. */
function downOnto(world: SpikeWorld, dir: Xyz, heightM: number, aboveM: number) {
  return {
    originM: vecOf(surfacePoint(world.figure, dir, heightM + aboveM)),
    direction: scale(vecOf(spheroidNormal(world.figure, dir)), -1),
    maxM: 2 * aboveM,
  };
}

/**
 * How far the drawn ground can stand above the site's vertex under the unit direction `g`, metres:
 * the point's offset from vertex (32, 32) of the site's patch in quads, |dx| + |dy|, times the
 * steepest rise to a neighbouring vertex. A point of a drawn triangle at that offset from one of its
 * corners is that corner's height plus at most the offsets times its edges' rises.
 */
function triangleRiseM(site: BodySite, ground: SiteGround, g: Vec3): number {
  const { face, u, v } = xyzToFaceUv([g.x, g.y, g.z]);
  expect(face).toBe(site.key.face);
  const quads = PATCH_QUADS * 2 ** site.key.level;
  const dx = uvToSt(u) * quads - (site.key.i * PATCH_QUADS + PATCH_QUADS / 2);
  const dy = uvToSt(v) * quads - (site.key.j * PATCH_QUADS + PATCH_QUADS / 2);
  expect(Math.abs(dx) + Math.abs(dy)).toBeLessThan(1e-3);
  const steepest = Math.max(...ground.neighbourhoodM.map((h) => Math.abs(h - ground.heightM)));
  return (Math.abs(dx) + Math.abs(dy)) * steepest;
}

describe("the descent's measurement over TERRAIN TEST's moon, through the terrain session", () => {
  let harness: Harness;
  let held: HeldBodyField;
  let planet: PlanetGeometry;
  let site: BodySite;
  let datum: DescentProfile;
  let ground: SiteGround;
  const bakes = new Map<string, KeyBake>();
  let finalKeys: ReadonlyArray<PatchKey>;

  beforeAll(async () => {
    harness = await openHarness();
    const { session, world } = harness;
    // Timeouts in polls of `pollClock`'s.
    held = await holdBodyField(session, world, {
      viewId: VIEW,
      revision: 2,
      timeoutMs: 3_000,
      ...pollClock(),
    });
    planet = planetGeometry(world.figure, held.levels);
    const drawn = bodySiteOf(siteCandidates(held.coverage), SEED, 0, planet.finestLevel);
    if (drawn === null) {
      throw new Error("the fixture's survey has candidates");
    }
    site = drawn;
    datum = new DescentProfile(world.figure, site.site);
    const final = trackStretches(datum).find(({ piece }) => piece === "final approach");
    if (final === undefined) {
      throw new Error("the track has a final approach");
    }
    finalKeys = stretchKeys(datum, final);
    const lane = session.lane(floorBakeSettings(world.body, held.detailSeed));
    if (lane === null) {
      throw new Error("the session is live");
    }
    const found: { ground: SiteGround | null } = { ground: null };
    try {
      // 46 bakes, 3.6 s here at 75 ms each.
      await bakePatches(
        lane,
        [site.key, ...finalKeys],
        (bake) => {
          bakes.set(patchKeyString(bake.key), keyBakeOf(bake));
          if (patchKeyString(bake.key) === patchKeyString(site.key)) {
            found.ground = siteGroundOf(bake);
          }
        },
        { timeoutMs: 6_000, ...pollClock() },
      );
    } finally {
      lane.terminate();
    }
    if (found.ground === null) {
      throw new Error("the site's patch was baked");
    }
    ground = found.ground;
  }, 120_000);

  afterAll(() => {
    harness.close();
  });

  it("holds the fixture's larger survey whole from one ask, with its seed and a settled table", () => {
    expect(held.coverage.revision).toBe(2);
    expect(held.coverage.maskTexels()).toEqual(payloadCoverage().maskTexels());
    expect(held.detailSeed).toBe(golden.seed);
    expect(held.header).toEqual(golden.header);
    expect(held.levels).toHaveLength(100);
    const traffic = harness.fixture.traffic();
    expect([traffic.fieldAsks, traffic.blocksSent]).toEqual([
      1,
      golden.payloads[2]?.blocksHex.length,
    ]);
  });

  it("draws seed 7's site in a cell whose 3 × 3 block is surveyed, its track surveyed to the end", () => {
    const { coverage } = held;
    expect(blockSurveyed(coverage, site.cell)).toBe(true);
    const off: string[] = [];
    const from = datum.segmentSpans().find(({ name }) => name === "low fast pass")?.startS ?? 0;
    for (let n = Math.round(from * 64); n <= Math.round(datum.durationS * 64); n += 1) {
      const g = datum.groundDirAt(n / 64);
      if (coverage.resolutionAtM(patchKeyAt([g.x, g.y, g.z], coverage.level)) === null) {
        off.push(`the pose at ${n / 64} s`);
      }
    }
    for (const stretch of trackStretches(datum)) {
      if (stretch.segment === "low fast pass" || stretch.segment === "slowdown") {
        for (const key of stretchKeys(datum, stretch)) {
          if (!keyWhollyCovered(coverage, key)) {
            off.push(`${stretch.piece}'s ${patchKeyString(key)}`);
          }
        }
      }
    }
    expect(off).toEqual([]);
  });

  it("stands the site on its finest bake's surveyed vertex, within the bake's range", () => {
    expect(ground.surveyed).toBe(true);
    expect(ground.heightM).toBeGreaterThanOrEqual(ground.rangeM[0]);
    expect(ground.heightM).toBeLessThanOrEqual(ground.rangeM[1]);
    expect(ground.neighbourhoodM[4]).toBe(ground.heightM);
  });

  it("bakes the final approach's patches wholly surveyed, so that its floor is baked", () => {
    expect(finalKeys.every((key) => bakes.get(patchKeyString(key))?.wholly === true)).toBe(true);
    const final = trackStretches(datum).filter(({ piece }) => piece === "final approach");
    const plan = planFloors([finalKeys], held.coverage);
    const [floor] = floorsOf(final, plan, bakes, planet, offSurveyCeilingM(planet, held.header));
    const highestM = Math.max(
      ...finalKeys.map((key) => bakes.get(patchKeyString(key))?.highestM ?? NaN),
    );
    expect(floor).toEqual({ floorM: highestM + levelBoundM(planet, 16), source: "baked" });
  });

  it("reads the survey at the site within 4σ and a metre of the drawn ground", async () => {
    const { session, world } = harness;
    const readout = await session.groundReadout(
      VIEW,
      world.body,
      downOnto(world, site.dir, ground.heightM, 1_000),
      keptTime(0),
      { slopeBaselineM: 2, padWidthM: 1 },
    );
    if (readout?.kind !== "ground") {
      throw new Error(`the readout found ${readout?.kind ?? "nothing"} at the site`);
    }
    // 2026-10-20: 1,900.194 ± 0.560 m over the drawn 1,900.124 m, unresolved.
    const { elevationM, elevationSigmaM } = readout.reading;
    expect(Math.abs(elevationM - ground.heightM)).toBeLessThanOrEqual(4 * elevationSigmaM + 1);
    // The slant range meets the same survey-level ground.
    expect(ground.heightM + 1_000 - readout.sight.rangeM).toBeCloseTo(elevationM, 2);
  });

  it("sets the off-survey ceiling at or above the module's own, where its march starts", async () => {
    const { session, world } = harness;
    const ceilingM = offSurveyCeilingM(planet, held.header);
    // Over ground the field does not hold (face 3's centre), a rising line of sight more than a
    // metre over the module's ceiling leaves at once, before it reads any ground (`reading.rs`'
    // `probe`, `FAR_MARGIN_M`), where one under it would find the ground not surveyed. Leaving from
    // 1.01 m over this ceiling puts `ceiling_m` under it plus 1 cm: the rule, whatever its value.
    // (2026-10-20: from 0.99 m over it the line finds the ground not surveyed, so the two agree
    // within 1 cm on the fixture; not held here, so that a tighter reach of the module's moves
    // nothing.)
    const dir: Xyz = [-1, 0, 0];
    const readout = await session.groundReadout(
      VIEW,
      world.body,
      {
        originM: vecOf(surfacePoint(world.figure, dir, ceilingM + 1.01)),
        direction: vecOf(spheroidNormal(world.figure, dir)),
        maxM: 1_000,
      },
      keptTime(0),
      { slopeBaselineM: 2, padWidthM: 1 },
    );
    expect(readout?.kind).toBe("no-ground");
  });
});

describe.runIf(SLOW)("the profile over TERRAIN TEST's moon (HYPERION_SLOW_TESTS=1)", () => {
  let harness: Harness;
  let measured: BodyDescent;

  beforeAll(async () => {
    harness = await openHarness();
    const { session, world } = harness;
    // Timeouts in polls of `pollClock`'s.
    await holdBodyField(session, world, {
      viewId: VIEW,
      revision: 2,
      timeoutMs: 3_000,
      ...pollClock(),
    });
    // 1,584 bakes, 110 s here at 64 to 83 ms each.
    measured = await measureBodyDescent(session, world, SEED, {
      viewId: VIEW,
      timeoutMs: 120_000,
      ...pollClock(),
    });
  }, 1_500_000);

  afterAll(() => {
    harness.close();
  });

  it("clears its floors, with every segment of the table", () => {
    const { profile } = measured;
    expect(profile.minFloorMarginM).toBeGreaterThanOrEqual(-FLOOR_TOLERANCE_M);
    expect(profile.segments.map(({ name }) => name)).toEqual(
      DESCENT_SEGMENTS.map(({ name }) => name),
    );
    expect(profile.segments.every(({ durationS }) => durationS > 0)).toBe(true);
  });

  it("floors each stretch over its baked patches plus ε_n, and at the ceiling off the survey", () => {
    const { planet, site, floors, bakes, ceilingM } = measured;
    const datum = new DescentProfile(measured.profile.figure, site.site);
    const survey = harness.session.survey(harness.world.body);
    const coverage = survey?.coverage ?? null;
    const header = survey?.header ?? null;
    if (coverage === null || header === null) {
      throw new Error("the field is held");
    }
    // The rule over the workers' table and the header, whatever its value.
    expect(ceilingM).toBe(offSurveyCeilingM(planet, header));
    const wrong: string[] = [];
    measured.stretches.forEach((stretch, k) => {
      const floor = floors[k];
      const keys = stretchKeys(datum, stretch);
      const known = keys.map((key) => bakes.get(patchKeyString(key)));
      const surveyed =
        keys.every((key) => keyWhollyCovered(coverage, key)) &&
        known.every((bake) => bake?.wholly === true);
      if (floor === undefined) {
        wrong.push(`${stretch.piece} has no floor`);
      } else if (surveyed) {
        const highestM = Math.max(...known.map((bake) => bake?.highestM ?? Infinity));
        if (
          floor.source !== "baked" ||
          floor.floorM < highestM + levelBoundM(planet, stretch.level)
        ) {
          wrong.push(`${stretch.piece}: ${floor.floorM} m (${floor.source}) under its bakes`);
        }
      } else if (floor.source !== "ceiling" || floor.floorM !== ceilingM) {
        wrong.push(`${stretch.piece}: ${floor.floorM} m (${floor.source}) off the survey`);
      }
      const late = stretch.segment === "low fast pass" || stretch.segment === "slowdown";
      if (late && floor?.source !== "baked") {
        wrong.push(`${stretch.piece} is not baked`);
      }
    });
    expect(wrong).toEqual([]);
  });

  it("stays above the site over it, and touches down a metre above, clear of the drawn ground", () => {
    const { profile, siteHeightM, site, siteGround } = measured;
    const spans = profile.segmentSpans();
    const from = spans.find(({ name }) => name === "vertical descent")?.startS ?? profile.durationS;
    let least = Infinity;
    for (let n = Math.ceil(from * 64); n <= Math.floor(profile.durationS * 64); n += 1) {
      least = Math.min(least, profile.poseAt(n / 64).altitudeM - siteHeightM);
    }
    expect(least).toBeGreaterThanOrEqual(1 - FLOOR_TOLERANCE_M);
    const end = profile.poseAt(profile.durationS);
    expect(end.clearanceM).toBeCloseTo(1, 9);
    const drawnAtMostM = siteHeightM + triangleRiseM(site, siteGround, end.groundDir);
    expect(end.altitudeM - drawnAtMostM).toBeGreaterThanOrEqual(1 - FLOOR_TOLERANCE_M);
  });

  it("reads the survey at the site within 4σ and a metre of its height", () => {
    const { siteReading, siteHeightM } = measured;
    if (siteReading?.kind !== "ground") {
      throw new Error(`the readout found the site's ground, not ${JSON.stringify(siteReading)}`);
    }
    expect(Math.abs(siteReading.elevationM - siteHeightM)).toBeLessThanOrEqual(
      4 * siteReading.sigmaM + 1,
    );
  });
});
