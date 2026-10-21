/**
 * The descent spike's measurement over a generated world (plan R10, R10.T14.c;
 * decision-r10-t14c-leans §4): the landing site, its height and the stretches' floors, taken
 * through T16's terrain session (its store over the world's client, its pool and a lane of it)
 * once before the run, as `prepareDescent` takes the test planet's through the surface query.
 *
 * @remarks
 * - **The field.** {@link holdBodyField} notes a measuring view report, the camera in the body's
 *   frame, so that the session wants the body's field as the view's local body, and steps the
 *   session until it holds the field's coverage at the revision asked, its detail seed and a
 *   settled level table: every block posted reported by the workers (`TerrainSession.levels`).
 * - **The site** is drawn inside the survey from the seed ({@link bodySiteOf}): among the field's
 *   cells whose whole 3 × 3 block is surveyed, edge and corner neighbours both ({@link
 *   siteCandidates}), so that the track's last 18 km, the low pass and the slowdown, stay over
 *   surveyed ground (decision-r10-t14c-leans §4, "The site"); snapped to the centre vertex of the
 *   finest patch holding the drawn point.
 * - **The site's height** is that vertex's own-level height in the workers' bake of the patch:
 *   the drawn ground, the client's `ground_at` to `f32`, on whose triangles the vertex lies, as
 *   the test planet's was T4.c's collision interpolant (decision-r05-descent-clearance.md). Not
 *   `slantRange`'s: it marches the survey-level surface (Design note 7), off the drawn ground by the
 *   omitted relief where σ ≠ 0, which on the fixture's 20–53 m survey could put the hover's last
 *   metre inside the drawn ground.
 * - **The floors** are R05's rule (decision-r05-descent-clearance.md, rule 1; `measureTerrain`)
 *   over wholly surveyed ground: the highest baked height of the stretch's patches
 *   (`stretchKeys`) plus ε_n of their level from the workers' level table, a true upper bound on
 *   the finest mesh there, since every vertex of level n lies within ε_n of it (Design note 15) and
 *   the mesh interpolates its vertices. Wherever a stretch's patch has a vertex off the survey, the
 *   floor is instead the body-wide ceiling ({@link offSurveyCeilingM}): decision-r10-t16-contracts
 *   §2's "highest bound" for ground with no survey, as decision-r10-t14c-leans §4 rules it, since
 *   the workers' table bounds only the cells held, and the true ground beyond them, which collision
 *   reads, can stand kilometres higher. A stretch whose patches the coverage already shows off the
 *   survey takes it unbaked ({@link planFloors}).
 * - **A refusal.** Where the floors make the profile refuse (`DescentUnclearable`), the next
 *   candidate cell is drawn, in order, and the patches already baked are kept; the run is refused
 *   (`DescentRefused`) only when every candidate refuses.
 * - **The reading** (`SITE ELEVATION`): the workers' terrain readout down a vertical line of sight
 *   onto the site, its `reading`'s elevation and σ (`surveyed_reading`'s), the knowledge-gated
 *   figure of Design note 6, while the script flies the drawn ground.
 * - **σ_n** of the body is not in the workers' level table, so {@link BodyDescent}'s
 *   `omittedSigmaM` is +∞ at every level: min(ε_n, 4σ_n) is then ε_n (`calibratedLevelTable`), and
 *   T13.c's calibrated count the hard count. Nothing serialises it: the recorder keeps the
 *   calibrated table alone, which is the hard one.
 */

import type { BodyIdHex, DetailSeedHex, SurfaceHeaderDto } from "@hyperion/protocol";

import { scale, vec3 } from "../../geometry/vec3";
import type { ViewId } from "../camera/state";
import { keptTime } from "../scenes/kept";
import { type CoverageView, fieldCellAt, fieldCellCount } from "../terrain/coverage";
import { PATCH_QUADS, vertexDir, type Xyz } from "../terrain/cube";
import {
  cornerNeighbours,
  EDGES,
  edgeNeighbour,
  MAX_LEVEL,
  type PatchKey,
  patchKeyString,
} from "../terrain/patchKey";
import {
  levelBoundM,
  levelHeightRangeM,
  type PlanetGeometry,
  planetGeometry,
  spheroidNormal,
  surfacePoint,
} from "../terrain/planet";
import type { TerrainSession, TerrainViewReport } from "../terrain/session";
import type { BakedPatch, BakeSettings, TerrainLayers } from "../terrain/workers/messages";
import type { BakeFailure, HeightWorkerLane } from "../terrain/workers/pool";
import { type SegmentLift, segmentLifts, stretchKeys } from "./demandRecord";
import {
  DescentProfile,
  DescentUnclearable,
  type LandingSite,
  landingSiteOf,
  splitMix64,
  type TrackStretch,
  trackStretches,
  unitOf,
} from "./descentProfile";
import {
  DescentRefused,
  type PreparedDescent,
  type SiteReading,
  SPIKE_FOV_X_RAD,
  SPIKE_MAIN_VIEW_ID,
} from "./spikeRun";
import { contactRule, spikeCameraAt, spikeScene } from "./spikeScene";
import type { SpikeWorld } from "./spikeWorld";

/** What the measurement asks of the terrain session: a real session, or a test's stand-in. */
export type BodyDescentSession = Pick<
  TerrainSession,
  "noteView" | "step" | "survey" | "levels" | "lane" | "groundReadout" | "detailSeed"
>;

/** How the measurement waits on the session and its workers. */
export interface BodyMeasureClock {
  /** How long it waits in all before it gives up, ms of {@link BodyMeasureClock.nowMs}'s clock. */
  readonly timeoutMs: number;
  /** How long a wait between two looks is, ms; 10 by default. */
  readonly pollMs?: number;
  /** Waits `ms`; a timer's task by default, which ends early, rejecting, at an abort. */
  readonly wait?: (ms: number) => Promise<void>;
  /** The clock the timeout is reckoned on, ms; `performance.now` by default. */
  readonly nowMs?: () => number;
  /**
   * Stops the waiting (a closed or remounted spike): the wait rejects with the abort's reason before
   * its next look, the default wait at once, and nothing polls on.
   */
  readonly signal?: AbortSignal;
}

/** What {@link holdBodyField} needs: the view its report is noted for, and the revision to wait for. */
export interface HoldBodyFieldOptions extends BodyMeasureClock {
  /** The spike's main view (`SPIKE_VIEW_NAMES.main`), whose report wants the field. */
  readonly viewId: ViewId;
  /** The coverage revision to wait for, at least: 2 for `TERRAIN TEST`'s larger survey. */
  readonly revision: number;
}

/** What {@link measureBodyDescent} needs: the view whose report the readout is asked for. */
export interface MeasureBodyDescentOptions extends BodyMeasureClock {
  /** The view {@link holdBodyField} noted, for the site's terrain readout. */
  readonly viewId: ViewId;
}

/** A body's field as the session holds it, settled. */
export interface HeldBodyField {
  readonly coverage: CoverageView;
  readonly header: SurfaceHeaderDto;
  /** The workers' level table, 25 × 4 (`TerrainSession.levels`). */
  readonly levels: Float64Array;
  readonly detailSeed: DetailSeedHex;
}

/**
 * A generated world's landing site, drawn inside the survey ({@link bodySiteOf}): the centre vertex
 * of a finest patch.
 */
export interface BodySite {
  /** The site as the profile takes it: the vertex's parametric latitude, longitude and the azimuth. */
  readonly site: LandingSite;
  /** The field's cell it lies in. */
  readonly cell: PatchKey;
  /** Its cell's place among the candidates, and the attempt (0 first) that drew it. */
  readonly candidate: number;
  readonly attempt: number;
  /** The finest-level patch whose centre vertex, (32, 32), it is. */
  readonly key: PatchKey;
  /** That vertex's unit direction d, of the spheroid point M·d (the cube's `vertexDir`). */
  readonly dir: Xyz;
}

/** What one bake tells the floors. */
export interface KeyBake {
  /** Its highest baked height, metres above the datum (`BakedPatch.heightRangeM`'s). */
  readonly highestM: number;
  /** Whether every vertex of it is surveyed (its survey mask all {@link SURVEYED_VERTEX}). */
  readonly wholly: boolean;
}

/** The ground at the landing site, from the bake of its finest patch. */
export interface SiteGround {
  /** The centre vertex's own-level height, metres above the datum: the site's height. */
  readonly heightM: number;
  /** Whether the vertex is surveyed. */
  readonly surveyed: boolean;
  /** The bake's height range, metres above the datum. */
  readonly rangeM: readonly [number, number];
  /**
   * The own-level heights of the 3 × 3 vertices about the centre, metres, row y then column x from
   * (31, 31): the corners of every drawn triangle the site's vertex is on.
   */
  readonly neighbourhoodM: ReadonlyArray<number>;
}

/** A stretch's floor and where it came from. */
export interface StretchFloor {
  /** F_k, metres above the datum. */
  readonly floorM: number;
  /** `baked` over wholly surveyed ground (R05's rule), or the body-wide `ceiling`. */
  readonly source: "baked" | "ceiling";
}

/** Which patches the floors bake. */
export interface FloorPlan {
  /** Each stretch's patches to bake, or `null` where the coverage puts it off the survey. */
  readonly stretches: ReadonlyArray<ReadonlyArray<PatchKey> | null>;
  /** Every patch to bake, each once, in the order first named. */
  readonly keys: ReadonlyArray<PatchKey>;
}

/** A candidate the profile refused, before the one flown. */
export interface RefusedSite {
  readonly cell: PatchKey;
  readonly message: string;
}

/** The descent over a generated world, measured: `PreparedDescent` with what the record states. */
export interface BodyDescent extends PreparedDescent {
  readonly siteReading: SiteReading | null;
  /** The site flown, and the candidates refused before it. */
  readonly site: BodySite;
  readonly refused: ReadonlyArray<RefusedSite>;
  readonly siteGround: SiteGround;
  /** Each stretch's floor, in `stretches`' order, with its source. */
  readonly floors: ReadonlyArray<StretchFloor>;
  /** The body-wide ceiling, metres above the datum ({@link offSurveyCeilingM}): +∞ with no cell. */
  readonly ceilingM: number;
  /** Each segment's lift above Design note 19's table (`segmentLifts`). */
  readonly lifts: ReadonlyArray<SegmentLift>;
  /** What each patch baked told the floors, by `patchKeyString`: each once, over every attempt. */
  readonly bakes: ReadonlyMap<string, KeyBake>;
  /** The readout's slant range onto the site from 1 km above it, metres, or `null`. */
  readonly siteSlantRangeM: number | null;
}

/**
 * The world's survey holds no ground to land on (R10.T14.c): no surveyed cell, or a site whose
 * vertex the bake finds off the survey; the spike then refuses to fly.
 */
export class NoLandingSite extends Error {
  constructor(body: BodyIdHex, why: string) {
    super(`no landing site on ${body}: ${why}`);
    this.name = "NoLandingSite";
  }
}

/** `TerrainLayers.surveyMask`'s byte for a surveyed vertex (Design note 12). */
export const SURVEYED_VERTEX = 255;

/**
 * What `reading.rs`' `ceiling_m` adds for the rounding of its terms, metres (`CEILING_SLACK_M`),
 * which {@link offSurveyCeilingM} adds likewise.
 */
export const CEILING_SLACK_M = 1;

/** How far above the site the reading's line of sight starts, metres, and how far it reaches. */
const READING_ABOVE_M = 1000;
const READING_REACH_M = 2000;

/** Vertices along a patch's side, and the centre vertex's index. */
const VERTICES = PATCH_QUADS + 1;
const CENTRE = PATCH_QUADS / 2;

/** The measuring view's presented size, device px: 1080p, as T13.b's high setting runs. */
const MEASURING_VIEW_PX = { widthPx: 1920, heightPx: 1080 } as const;

const DEFAULT_POLL_MS = 10;

/** A look that has its answer, or says what it still lacks. */
type Polled<T> =
  { readonly done: true; readonly value: T } | { readonly done: false; readonly missing: string };

/** {@link BodyMeasureClock} with its defaults. */
interface Clock {
  readonly timeoutMs: number;
  readonly pollMs: number;
  readonly wait: (ms: number) => Promise<void>;
  readonly nowMs: () => number;
  readonly signal: AbortSignal | null;
}

/** The abort's reason as an error to reject with. */
function abortError(signal: AbortSignal): Error {
  const reason: unknown = signal.reason;
  return reason instanceof Error
    ? reason
    : new Error("the measurement was aborted", { cause: reason });
}

/** Waits `ms` on a timer, or rejects at once with the abort's reason when `signal` aborts. */
function timerWait(ms: number, signal: AbortSignal | null): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    if (signal === null) {
      setTimeout(resolve, ms);
      return;
    }
    if (signal.aborted) {
      reject(abortError(signal));
      return;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const onAbort = (): void => {
      clearTimeout(timer);
      reject(abortError(signal));
    };
    timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

function clockOf(options: BodyMeasureClock): Clock {
  const signal = options.signal ?? null;
  return {
    timeoutMs: options.timeoutMs,
    pollMs: options.pollMs ?? DEFAULT_POLL_MS,
    wait: options.wait ?? ((ms) => timerWait(ms, signal)),
    nowMs: options.nowMs ?? (() => performance.now()),
    signal,
  };
}

/**
 * Looks with `look` every `clock.pollMs` until it answers, and rejects once `clock.timeoutMs` has
 * passed since the first look, naming what it lacked; a look that throws rejects at once, and an
 * abort of `clock.signal` before any look rejects with its reason.
 */
async function pollUntil<T>(look: () => Polled<T>, clock: Clock, what: string): Promise<T> {
  const startMs = clock.nowMs();
  const poll = async (): Promise<T> => {
    if (clock.signal?.aborted === true) {
      throw abortError(clock.signal);
    }
    const looked = look();
    if (looked.done) {
      return looked.value;
    }
    if (clock.nowMs() - startMs >= clock.timeoutMs) {
      throw new Error(`${what} within ${String(clock.timeoutMs)} ms: ${looked.missing}`);
    }
    await clock.wait(clock.pollMs);
    return poll();
  };
  return poll();
}

/**
 * The measuring view's report: the spike's scene of the world at a provisional site, its camera
 * the craft's at the orbit's start, 400 km up in the body's frame, so that the session's
 * `sceneFrameFor` makes the body the view's local body, which wants its field; photorealistic,
 * handing nothing over (no regimes), so that no pass is made.
 */
function measuringReport(world: SpikeWorld): TerrainViewReport {
  const provisional = new DescentProfile(world.figure, landingSiteOf(0n));
  return {
    scene: spikeScene(provisional, world).sceneAt(0),
    pose: spikeCameraAt(provisional, 0, world),
    fovXRad: SPIKE_FOV_X_RAD,
    presentedPx: MEASURING_VIEW_PX,
    style: "photorealistic",
    drawnStyle: "photorealistic",
    setting: "high",
    streamPriority: "primary",
    regimes: new Map(),
    readoutBody: world.body,
  };
}

/** The body's field as the session holds it, or what it lacks for `revision`. */
function heldFieldOf(
  session: BodyDescentSession,
  body: BodyIdHex,
  revision: number,
): Polled<HeldBodyField> {
  const survey = session.survey(body);
  const failure = survey?.status.failure ?? null;
  if (failure !== null) {
    throw new Error(
      `the field of ${body} failed: ${failure.kind === "rejected" ? failure.message : failure.check}`,
    );
  }
  const coverage = survey?.coverage ?? null;
  const header = survey?.header ?? null;
  const levels = session.levels(body);
  const detailSeed = session.detailSeed(body);
  if (
    coverage !== null &&
    (coverage.revision ?? 0) >= revision &&
    header !== null &&
    levels !== null &&
    detailSeed !== null
  ) {
    return { done: true, value: { coverage, header, levels, detailSeed } };
  }
  const missing = [
    ...(coverage === null || (coverage.revision ?? 0) < revision
      ? [
          `its coverage at revision ${String(revision)} (${String(coverage?.revision ?? "none")} held)`,
        ]
      : []),
    ...(header === null ? ["its field header"] : []),
    ...(levels === null ? ["a settled level table"] : []),
    ...(detailSeed === null ? ["its detail seed"] : []),
  ];
  return { done: false, missing: missing.join(", ") };
}

/**
 * Waits until `session` holds `world`'s body's field at `options.revision` or later, its detail
 * seed and a settled level table, noting the measuring view's report for `options.viewId` first
 * and stepping the session between looks.
 *
 * @throws Error (a rejection) where the field fails, or is not held within `options.timeoutMs`,
 *   naming what it lacked, or with the abort's reason once `options.signal` aborts.
 */
export async function holdBodyField(
  session: BodyDescentSession,
  world: SpikeWorld,
  options: HoldBodyFieldOptions,
): Promise<HeldBodyField> {
  session.noteView(options.viewId, measuringReport(world));
  return pollUntil(
    () => {
      session.step();
      return heldFieldOf(session, world.body, options.revision);
    },
    clockOf(options),
    `the field of ${world.body} was not held`,
  );
}

/**
 * The field's cells a site may be drawn in: those whose whole 3 × 3 block is surveyed, the four
 * edge neighbours and the corner neighbours (three at a cube corner, `cornerNeighbours`), in rising
 * cell index; empty where no block is whole, when the spike refuses to fly ({@link NoLandingSite}):
 * a site elsewhere could fly its low pass over unsurveyed ground at the ceiling, which would make
 * the run's low-pass demand meaningless (decision-r10-t14c-leans §4).
 *
 * @remarks
 * A 3 × 3 block keeps the track's last 18 km, the low pass and the slowdown, over surveyed ground
 * from any point of its middle cell, whose neighbours are each at least 25.5 km across on the
 * fixture: (2√2 ÷ 3) ÷ 64 × 1,736.01 km = 25.57 km, the cube's least arc rate, 2√2 ÷ 3 a unit of s
 * (`cube.ts`' `vertexSpacing`), over a 64th of a face on its polar radius. Edge neighbours alone
 * leave a site within 18 km of a corner crossing an unsurveyed diagonal one
 * (decision-r10-t14c-leans §4).
 */
export function siteCandidates(
  coverage: Pick<CoverageView, "level" | "resolutionAtM">,
): ReadonlyArray<PatchKey> {
  const { level } = coverage;
  const surveyed = (cell: PatchKey): boolean => coverage.resolutionAtM(cell) !== null;
  const interior: PatchKey[] = [];
  for (let index = 0; index < fieldCellCount(level); index += 1) {
    const cell = fieldCellAt(level, index);
    if (!surveyed(cell)) {
      continue;
    }
    const edges = EDGES.map((edge) => edgeNeighbour(cell, edge));
    const corners = cornerNeighbours(cell).filter((corner) => corner !== null);
    if ([...edges, ...corners].every(surveyed)) {
      interior.push(cell);
    }
  }
  return interior;
}

/**
 * The landing site of `seed` among `candidates` ({@link siteCandidates}) at attempt `attempt`, or
 * `null` with no candidate.
 *
 * @remarks
 * Drawn by SplitMix64 from the seed alone, as `landingSiteOf` draws the test planet's: u₀ the
 * cell, the ⌊u₀ n⌋-th of the n candidates, then the next in order at each later attempt (wrapping);
 * u₁ and u₂ a point uniform in the cell's face square, (i + u₁, j + u₂) ÷ 2ᴸ in s and t; u₃ the
 * approach's azimuth, 2π u₃. The site is the centre vertex (32, 32) of the finest patch holding
 * that point, which lies in the same cell (the quadtree nests), at its parametric latitude
 * β = asin(d_z) and longitude λ = atan2(d_y, d_x) of the vertex's direction d, so that the
 * profile's `siteDir`, (cos β cos λ, cos β sin λ, sin β), is d to rounding.
 *
 * @param finestLevel - The body's finest level (`PlanetGeometry.finestLevel`).
 * @throws RangeError if `seed` is not in [0, 2^64), or `attempt` is not a whole number.
 */
export function bodySiteOf(
  candidates: ReadonlyArray<PatchKey>,
  seed: bigint,
  attempt: number,
  finestLevel: number,
): BodySite | null {
  if (seed < 0n || seed >= 1n << 64n) {
    throw new RangeError(`a seed is a u64, got ${seed}`);
  }
  if (!Number.isInteger(attempt) || attempt < 0) {
    throw new RangeError(`an attempt is a whole number, got ${String(attempt)}`);
  }
  const n = candidates.length;
  if (n === 0) {
    return null;
  }
  const [s1, a] = splitMix64(seed);
  const [s2, b] = splitMix64(s1);
  const [s3, c] = splitMix64(s2);
  const [, e] = splitMix64(s3);
  const candidate = (Math.floor(unitOf(a) * n) + attempt) % n;
  const cell = candidates[candidate];
  if (cell === undefined) {
    throw new Error(`candidate ${String(candidate)} of ${String(n)} is missing`);
  }
  // The finest patch holding (s, t) = ((i + u₁), (j + u₂)) ÷ 2ᴸ: its index floor(s 2ᶠ), kept within
  // the cell where u₁ or u₂ rounds the sum up to the next cell's edge.
  const shift = 2 ** (finestLevel - cell.level);
  const index = (base: number, u: number): number =>
    Math.min((base + 1) * shift - 1, Math.max(base * shift, Math.floor((base + u) * shift)));
  const key: PatchKey = {
    face: cell.face,
    level: finestLevel,
    i: index(cell.i, unitOf(b)),
    j: index(cell.j, unitOf(c)),
  };
  const dir = vertexDir(key, CENTRE, CENTRE);
  return {
    site: {
      latitudeRad: Math.asin(dir[2]),
      longitudeRad: Math.atan2(dir[1], dir[0]),
      azimuthRad: 2 * Math.PI * unitOf(e),
    },
    cell,
    candidate,
    attempt,
    key,
    dir,
  };
}

/**
 * Whether the coverage shows every coarse cell under `key` surveyed: the cell holding it for a
 * patch at or below the field's level, every cell of the field's level under it for a coarser one.
 */
export function keyWhollyCovered(
  coverage: Pick<CoverageView, "level" | "resolutionAtM">,
  key: PatchKey,
): boolean {
  if (key.level >= coverage.level) {
    return coverage.resolutionAtM(key) !== null;
  }
  const n = 2 ** (coverage.level - key.level);
  for (let dj = 0; dj < n; dj += 1) {
    for (let di = 0; di < n; di += 1) {
      const cell = { face: key.face, level: coverage.level, i: key.i * n + di, j: key.j * n + dj };
      if (coverage.resolutionAtM(cell) === null) {
        return false;
      }
    }
  }
  return true;
}

/**
 * Which patches the floors bake: a stretch's patches where the coverage shows every cell under
 * every one of them surveyed ({@link keyWhollyCovered}), each patch once over the stretches; none
 * of a stretch with a patch on any unsurveyed cell, which holds a vertex off the survey (every
 * coarse cell of a patch at most six levels finer holds one of its vertices) and so takes the
 * ceiling whatever its bakes.
 */
export function planFloors(
  keySets: ReadonlyArray<ReadonlyArray<PatchKey>>,
  coverage: Pick<CoverageView, "level" | "resolutionAtM">,
): FloorPlan {
  const keys = new Map<string, PatchKey>();
  const stretches = keySets.map((set) => {
    if (!set.every((key) => keyWhollyCovered(coverage, key))) {
      return null;
    }
    for (const key of set) {
      keys.set(patchKeyString(key), key);
    }
    return set;
  });
  return { stretches, keys: [...keys.values()] };
}

/**
 * The floor off the survey, metres above the datum: the body-wide ceiling, a height no ground of
 * the body reaches at any level, held or not, once the field holds a cell (decision-r10-t14c-leans
 * §4; `reading.rs`' `ceiling_m`, the slant range march's start). The one place the off-survey bound
 * is taken, so that a tighter one can replace it.
 *
 * @remarks
 * max over the 25 rows n of the workers' level table's highest (index 4n + 2), plus the header's
 * `realised_relief_m`, plus {@link CEILING_SLACK_M}: each row's highest is at least the highest
 * cell held plus the level's contributions (`worker.rs`' `level_table`), and `ceiling_m`'s argument
 * holds: the base elevation is a convex blend of the cells read, and no cell of the body lies
 * above its lowest by more than the relief, nor its lowest above any cell held. The table's own
 * highest bounds only the ground held.
 *
 * +∞ where no cell is held, as `ceiling_m` is where its view holds none: every row's lowest and
 * highest are then exactly 0 (`level_table`'s rows before any cell, or no table at all), which a
 * table of any cell held never gives (its highest is above 0 or its lowest below), and a ceiling
 * over the relief alone would bound nothing.
 *
 * The contributions' bound dominates on `TERRAIN TEST`'s fixture as R09 sets it (2026-10-20): the
 * rows' highest is 160.5 km from level 11 on against a realised relief of 15.8 km, so the ceiling is
 * 176.3 km, the module's own march's start to 1 cm, and the off-survey pieces fly above it. A
 * tighter reach of R09's lowers it with the table; the rule stands (decision-r10-t14c-leans §4).
 *
 * @throws Error where a cell is held but the header's relief is not finite, so that neither is the
 *   ceiling.
 */
export function offSurveyCeilingM(
  planet: PlanetGeometry,
  header: Pick<SurfaceHeaderDto, "realised_relief_m">,
): number {
  let highestM = -Infinity;
  let anyHeld = false;
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const [lowM, highM] = levelHeightRangeM(planet, level);
    anyHeld ||= lowM !== 0 || highM !== 0;
    highestM = Math.max(highestM, highM);
  }
  if (!anyHeld) {
    return Infinity;
  }
  const ceilingM = highestM + header.realised_relief_m + CEILING_SLACK_M;
  if (!Number.isFinite(ceilingM)) {
    throw new Error(`a ceiling from a highest of ${String(highestM)} m is not finite`);
  }
  return ceilingM;
}

/**
 * Each stretch's floor: over a planned stretch whose bakes are all wholly surveyed, the highest
 * baked height plus ε_n of its level (R05's rule); elsewhere `ceilingM`.
 *
 * @throws Error where a planned patch has no bake in `bakes`.
 */
export function floorsOf(
  stretches: ReadonlyArray<Pick<TrackStretch, "level">>,
  plan: FloorPlan,
  bakes: ReadonlyMap<string, KeyBake>,
  planet: PlanetGeometry,
  ceilingM: number,
): StretchFloor[] {
  return stretches.map((stretch, k): StretchFloor => {
    const keys = plan.stretches[k] ?? null;
    if (keys === null) {
      return { floorM: ceilingM, source: "ceiling" };
    }
    let highestM = -Infinity;
    for (const key of keys) {
      const bake = bakes.get(patchKeyString(key));
      if (bake === undefined) {
        throw new Error(`patch ${patchKeyString(key)} of a planned stretch was not baked`);
      }
      if (!bake.wholly) {
        return { floorM: ceilingM, source: "ceiling" };
      }
      highestM = Math.max(highestM, bake.highestM);
    }
    return { floorM: highestM + levelBoundM(planet, stretch.level), source: "baked" };
  });
}

/** The layers of a body's bake, or an error naming the patch: the test planet's have none. */
function layersOf(bake: BakedPatch): TerrainLayers {
  if (bake.layers === null) {
    throw new Error(`patch ${patchKeyString(bake.key)} was baked without a body's layers`);
  }
  return bake.layers;
}

/** What a bake tells the floors: its highest height, and whether it is wholly surveyed. */
export function keyBakeOf(bake: BakedPatch): KeyBake {
  return {
    highestM: bake.heightRangeM[1],
    wholly: layersOf(bake).surveyMask.every((m) => m === SURVEYED_VERTEX),
  };
}

/**
 * The ground at the site from the bake of its finest patch: the centre vertex's own-level height
 * (`BakedPatch.heights`, own-level height then morph target a vertex, vertex (x, y) at
 * 2 (65 y + x)), its mask, the bake's range and the 3 × 3 heights about it.
 *
 * @throws Error for a bake whose arrays are not a patch's 65 × 65.
 */
export function siteGroundOf(bake: BakedPatch): SiteGround {
  const mask = layersOf(bake).surveyMask;
  if (bake.heights.length !== 2 * VERTICES * VERTICES || mask.length !== VERTICES * VERTICES) {
    throw new Error(`patch ${patchKeyString(bake.key)}'s bake is not 65 × 65 vertices`);
  }
  const heightAt = (x: number, y: number): number => {
    const h = bake.heights[2 * (VERTICES * y + x)];
    if (h === undefined) {
      throw new Error(`vertex (${String(x)}, ${String(y)}) is outside the bake`);
    }
    return h;
  };
  const neighbourhoodM: number[] = [];
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      neighbourhoodM.push(heightAt(CENTRE + dx, CENTRE + dy));
    }
  }
  return {
    heightM: heightAt(CENTRE, CENTRE),
    surveyed: mask[VERTICES * CENTRE + CENTRE] === SURVEYED_VERTEX,
    rangeM: bake.heightRangeM,
    neighbourhoodM,
  };
}

/**
 * The floors' bakes of `body`: its heights alone, the weights and the horizon map skipped, since
 * the floors and the site read only the heights and the survey mask (decision-r10-t14c-leans §1).
 */
export function floorBakeSettings(body: BodyIdHex, detailSeed: DetailSeedHex): BakeSettings {
  return {
    vertexPath: "face-differences",
    normals: "mesh",
    source: { kind: "body", body, detailSeed, weights: "skipped" },
  };
}

/**
 * Bakes `keys` through `lane`, each once and in their order (the first sooner), handing each bake
 * to `take` as it lands.
 *
 * @remarks
 * The lane's demand is replaced by `keys` (`HeightWorkerLane.reprioritise`); its caller terminates
 * it. A throw of `take`, which the pool would report and pass over, rejects the bakes instead.
 *
 * @throws Error (a rejection) on a failed bake, a patch the field cannot make (one waiting for a
 *   block of a field that is held whole), the loss of every worker, `options.timeoutMs`, or an
 *   abort of `options.signal`, its listeners released either way.
 */
export async function bakePatches(
  lane: HeightWorkerLane,
  keys: ReadonlyArray<PatchKey>,
  take: (bake: BakedPatch) => void,
  options: BodyMeasureClock,
): Promise<void> {
  const clock = clockOf(options);
  const left = new Set(keys.map(patchKeyString));
  if (left.size === 0) {
    return;
  }
  let failed: Error | null = null;
  const offBaked = lane.onBaked((bake) => {
    if (!left.delete(patchKeyString(bake.key))) {
      return;
    }
    try {
      take(bake);
    } catch (error: unknown) {
      // The pool reports a listener's throw and goes on; the measurement must not.
      failed ??= error instanceof Error ? error : new Error(String(error), { cause: error });
    }
  });
  const offFailed = lane.onFailed((failure: BakeFailure) => {
    failed ??= new Error(`patch ${failure.keyString} could not be baked: ${failure.message}`);
  });
  try {
    lane.reprioritise(keys.map((key, n) => ({ key, priority: keys.length - n, forced: false })));
    await pollUntil(
      (): Polled<undefined> => {
        if (failed !== null) {
          throw failed;
        }
        if (lane.unavailable) {
          throw new Error("every height worker was lost");
        }
        if (lane.waitingCount > 0) {
          throw new Error(
            `${String(lane.waitingCount)} patches wait for a block of a field that is held whole`,
          );
        }
        return left.size === 0
          ? { done: true, value: undefined }
          : { done: false, missing: `${String(left.size)} of ${String(keys.length)} bakes` };
      },
      clock,
      "the floors' patches were not baked",
    );
  } finally {
    offBaked();
    offFailed();
  }
}

/**
 * The workers' reading of the ground at the site: the terrain readout down the spheroid's normal
 * from {@link READING_ABOVE_M} above the site's height, by its kind: over ground its `reading`
 * (`surveyed_reading`'s, not the slant range's σ, which is a range's along a line of sight;
 * decision-r10-t14c-leans §4) with the slant range itself, else `not-surveyed` or `no-ground` with
 * no range; `null` for each where the readout gives no answer (no detail seed or field held, or no
 * worker left).
 */
async function siteReadingOf(
  session: BodyDescentSession,
  viewId: ViewId,
  world: SpikeWorld,
  profile: DescentProfile,
  siteHeightM: number,
): Promise<{ readonly reading: SiteReading | null; readonly rangeM: number | null }> {
  const d = profile.siteDir;
  const dir: Xyz = [d.x, d.y, d.z];
  const [ox, oy, oz] = surfacePoint(world.figure, dir, siteHeightM + READING_ABOVE_M);
  const [nx, ny, nz] = spheroidNormal(world.figure, dir);
  const readout = await session.groundReadout(
    viewId,
    world.body,
    { originM: vec3(ox, oy, oz), direction: scale(vec3(nx, ny, nz), -1), maxM: READING_REACH_M },
    keptTime(0),
    { slopeBaselineM: 2, padWidthM: 1 },
  );
  if (readout === null) {
    return { reading: null, rangeM: null };
  }
  let site: { readonly reading: SiteReading; readonly rangeM: number | null };
  switch (readout.kind) {
    case "ground":
      site = {
        reading: {
          kind: "ground",
          elevationM: readout.reading.elevationM,
          sigmaM: readout.reading.elevationSigmaM,
          resolved: readout.reading.resolved,
        },
        rangeM: readout.sight.rangeM,
      };
      break;
    case "not-surveyed":
    case "no-ground":
      site = { reading: { kind: readout.kind }, rangeM: null };
      break;
  }
  return site;
}

/**
 * Measures the descent of `seed` over `world`'s body through `session`, which must hold its field
 * ({@link holdBodyField}): the site inside the survey, its height, each stretch's floor, the
 * profile flown over them with its contact rule, and the site's reading.
 *
 * @throws {@link NoLandingSite} where the survey holds no cell whose whole 3 × 3 block is surveyed (or the site's vertex is
 *   off it), {@link DescentRefused} where every candidate site's floors refuse the profile, and
 *   Error where the field is not held, the session is disposed or a bake fails or times out, or
 *   with the abort's reason once `options.signal` aborts; its lane is terminated whatever the end.
 */
export async function measureBodyDescent(
  session: BodyDescentSession,
  world: SpikeWorld,
  seed: bigint,
  options: MeasureBodyDescentOptions,
): Promise<BodyDescent> {
  const body = world.body;
  const held = heldFieldOf(session, body, 0);
  if (!held.done) {
    throw new Error(`measure ${body}'s descent once its field is held: it lacks ${held.missing}`);
  }
  const { coverage, header, levels, detailSeed } = held.value;
  const planet = planetGeometry(world.figure, levels);
  const ceilingM = offSurveyCeilingM(planet, header);
  const candidates = siteCandidates(coverage);
  if (candidates.length === 0) {
    throw new NoLandingSite(body, "its survey holds no cell whose whole 3 × 3 block is surveyed");
  }
  const lane = session.lane(floorBakeSettings(body, detailSeed));
  if (lane === null) {
    throw new Error("the terrain session was disposed before the descent was measured");
  }
  const bakes = new Map<string, KeyBake>();
  const grounds = new Map<string, SiteGround>();
  const refused: RefusedSite[] = [];

  /** Flies attempt `attempt`'s site, or the next candidate's where its floors refuse it. */
  const fly = async (attempt: number): Promise<BodyDescent> => {
    const site = bodySiteOf(candidates, seed, attempt, planet.finestLevel);
    if (site === null) {
      throw new NoLandingSite(body, "its survey holds no cell whose whole 3 × 3 block is surveyed");
    }
    const siteKey = patchKeyString(site.key);
    // The ground track and the stretches do not depend on the terrain: the bare datum's profile
    // gives them.
    const datum = new DescentProfile(world.figure, site.site);
    const stretches = trackStretches(datum);
    const plan = planFloors(
      stretches.map((stretch) => stretchKeys(datum, stretch)),
      coverage,
    );
    const wanted = [site.key, ...plan.keys].filter((key) => {
      const keyString = patchKeyString(key);
      return !bakes.has(keyString) || (keyString === siteKey && !grounds.has(keyString));
    });
    await bakePatches(
      lane,
      wanted,
      (bake) => {
        const keyString = patchKeyString(bake.key);
        bakes.set(keyString, keyBakeOf(bake));
        if (keyString === siteKey) {
          grounds.set(keyString, siteGroundOf(bake));
        }
      },
      options,
    );
    const ground = grounds.get(siteKey);
    if (ground === undefined) {
      throw new Error(`the site's patch ${siteKey} was not baked`);
    }
    if (!ground.surveyed) {
      throw new NoLandingSite(body, `the site's vertex in ${siteKey} is not surveyed`);
    }
    const floors = floorsOf(stretches, plan, bakes, planet, ceilingM);
    const stretchMaxHeightsM = floors.map((floor) => floor.floorM);
    let profile: DescentProfile;
    try {
      profile = new DescentProfile(world.figure, site.site, {
        siteHeightM: ground.heightM,
        stretchMaxHeightsM,
      });
    } catch (error: unknown) {
      if (!(error instanceof DescentUnclearable)) {
        throw error;
      }
      refused.push({ cell: site.cell, message: error.message });
      if (attempt + 1 < candidates.length) {
        return fly(attempt + 1);
      }
      throw new DescentRefused(seed, error);
    }
    const { reading, rangeM } = await siteReadingOf(
      session,
      options.viewId,
      world,
      profile,
      ground.heightM,
    );
    const lowPass = stretches.findIndex((stretch) => stretch.segment === "low fast pass");
    return {
      planet,
      profile,
      contact: contactRule(profile),
      siteHeightM: ground.heightM,
      stretches,
      stretchMaxHeightsM,
      trackMaxHeightM: stretchMaxHeightsM[lowPass] ?? ground.heightM,
      omittedSigmaM: new Float64Array(MAX_LEVEL + 1).fill(Infinity),
      siteReading: reading,
      site,
      refused,
      siteGround: ground,
      floors,
      ceilingM,
      lifts: segmentLifts(profile),
      bakes,
      siteSlantRangeM: rangeM,
    };
  };

  try {
    return await fly(0);
  } finally {
    lane.terminate();
  }
}

/**
 * The coverage revision the spike waits for over `TERRAIN TEST`: 2, the larger survey, which the
 * fixture's first answer gives once its revision is raised before the session's first step
 * (payload 3; R10.T16.d).
 */
export const TERRAIN_TEST_REVISION = 2;

/**
 * How long the spike waits for the world's field, ms: two minutes, against a few seconds for the
 * fixture's one answer of about 2 MB and the workers' level table.
 */
export const FIELD_HOLD_TIMEOUT_MS = 120_000;

/**
 * How long each attempt of the descent's measurement waits for its bakes, ms: ten minutes. The
 * measurement over `TERRAIN TEST` bakes about 1,600 patches, some 40 s on three workers (110 s on
 * one in-process worker in `bodyDescent.wasm.test.ts`); the room is for one worker (`--workers 1`)
 * on a loaded machine.
 */
export const MEASURE_TIMEOUT_MS = 600_000;

/** How the spike measures its descent over a generated world: {@link measureWorldDescent}, or a test's. */
export type MeasureDescent = (
  session: BodyDescentSession,
  world: SpikeWorld,
  seed: bigint,
  signal: AbortSignal,
) => Promise<PreparedDescent>;

/**
 * The spike's measurement of `seed`'s descent over `world` through `session` (R10.T14.c): its field
 * held at {@link TERRAIN_TEST_REVISION} ({@link holdBodyField}) for the spike's main view, then the
 * descent measured over it ({@link measureBodyDescent}), each within its timeout and stopped by
 * `signal`.
 *
 * @throws As {@link holdBodyField} and {@link measureBodyDescent} do.
 */
export async function measureWorldDescent(
  session: BodyDescentSession,
  world: SpikeWorld,
  seed: bigint,
  signal: AbortSignal,
): Promise<BodyDescent> {
  await holdBodyField(session, world, {
    viewId: SPIKE_MAIN_VIEW_ID,
    revision: TERRAIN_TEST_REVISION,
    timeoutMs: FIELD_HOLD_TIMEOUT_MS,
    signal,
  });
  return measureBodyDescent(session, world, seed, {
    viewId: SPIKE_MAIN_VIEW_ID,
    timeoutMs: MEASURE_TIMEOUT_MS,
    signal,
  });
}
