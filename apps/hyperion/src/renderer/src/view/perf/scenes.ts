/**
 * The perf scenes (plan R12, R12.T5.a; Design notes 8, 9 and 13): what each of `@hyperion/perf`'s
 * `PERF_SCENE_IDS` draws, from where, for how long, and what it is judged on.
 *
 * @remarks
 * Each scene pins a universe's seed, a system and a body at the current generator version
 * (`@hyperion/perf`'s `PERF_PINS`, which the main process places the ship by), and
 * `crates/hyperion-server/tests/perf_scenes.rs` holds each to what its scene needs (R12.T5.b; a
 * Vitest holds the two files' pins equal). A generator bump that moves a pin re-pins it in the same commit
 * and bumps the scene's {@link PerfScene.version}, so that records from before and after never
 * compare. The server's placement and workers and the height workers are `@hyperion/perf`'s
 * `PERF_SCENE_HARNESS`, which the main process reads too.
 *
 * A repetition (Design note 13) is one pass over the scene's span from the same state: before each,
 * the client drops the patch cache and the height textures, cancels the height workers' queues and,
 * where the scene's arrival posts the field, posts it again; the main process restarts a `local` or
 * `co-resident` server with a fresh data directory, so that the arrival's work lands in every
 * repetition. The warm-up holds the script's first pose, the views open, while streaming settles
 * and the main process judges the machine steady (Design note 7).
 *
 * The views are R07's per-view policy (`budget/viewBudget.ts`): a photorealistic primary at its
 * setting's rate (60 Hz high, 30 Hz low) and the window's size, a wireframe primary or a station's
 * wireframe at 60 Hz, and each instrument at 30 Hz on R07's cockpit layout's 15 × 11.25 rem canvas,
 * 240 × 180 px at a device-pixel ratio of 1 (`styles.css`, `.view-slot__canvas`; R07's instrument
 * sizes, an open ask of R07, are taken from its code as built). A wireframe draws its strokes with
 * R02's analytic fringe (`wireframe/drawList.ts`), so its line antialiasing is `analytic`.
 */

import {
  PERF_PINS,
  PERF_SCENE_HARNESS,
  PERF_SURVEY,
  PERF_UNIVERSE_NAME,
  type PerfCriterion,
  type PerfSceneHarness,
  type PerfSceneId,
  type ViewRole,
} from "@hyperion/perf";

import { viewBudgets, type ViewSlot } from "../budget/viewBudget";
import { type RenderStyle, viewId } from "../camera/state";
import type { QualitySetting } from "../quality/qualitySetting";
import { DESCENT_SEGMENTS } from "../spike/descentProfile";
import { APPROACH_DURATION_S, ARRIVAL_HELD_S, SWEEP_DURATION_S } from "./scripts";

/** The universe a scene's server makes before each repetition: its name and its seed. */
export interface PerfUniverse {
  readonly name: string;
  /** 16 lowercase hexadecimal digits, as `create_universe` takes it. */
  readonly seed: string;
}

/** How a scene's frames are paced (Design note 9), the record's `harness.pacing`. */
export type PerfPacing = "every-vsync" | "every-second-vsync";

/** A view's size: the window's own at the setting, or a canvas of its own, device px. */
export type PerfViewSize =
  | { readonly kind: "window" }
  | { readonly kind: "canvas"; readonly widthPx: number; readonly heightPx: number };

/** One view of a scene, under R07's per-view policy. */
export interface PerfSceneView {
  readonly role: ViewRole;
  readonly style: RenderStyle;
  readonly size: PerfViewSize;
  readonly rateHz: 60 | 30;
  /** R02's analytic fringe on a wireframe, `none` on a photorealistic view. */
  readonly lineAntialiasing: "analytic" | "none";
}

/**
 * A scene's camera script (Design note 8): R05's descent (`descentProfile.ts`) over the pinned
 * world from the landing site its seed draws, with the arrival at script time 0; or one of
 * `scripts.ts`' pure scripts over the pinned giant's rings, the arrival's two poses, or the sky's
 * sweep.
 */
export type PerfSceneScript =
  | { readonly kind: "descent"; readonly landingSeed: string; readonly arrivalAtS: 0 }
  | { readonly kind: "approach" }
  | {
      readonly kind: "arrival";
      readonly landingSeed: string;
      /** R09's explicit survey request before the field is posted: its source and resolution, m. */
      readonly survey: { readonly source: "orbital"; readonly resolutionM: number };
    }
  | { readonly kind: "sweep" };

/**
 * A repetition's measured span (Design note 13): a script's length, or the arrival's, which lasts
 * until the field and its patches are resident and then holds the deepest pose `heldS`.
 */
export type PerfSpan =
  | { readonly kind: "script"; readonly seconds: number }
  | { readonly kind: "arrival"; readonly heldS: number };

/** What is reset before each repetition (Design note 13). */
export interface PerfResets {
  /** The patch cache and its height textures dropped, the height workers' queues cancelled. */
  readonly patchCache: true;
  /** Whether the arrival posts the field to the height workers again. */
  readonly field: boolean;
  /** Whether the main process restarts the server with a fresh data directory. */
  readonly server: boolean;
}

/** One scene of the catalogue. */
export interface PerfScene {
  readonly id: PerfSceneId;
  /** From 1; bumped with any change to its pins or script. */
  readonly version: number;
  readonly universe: PerfUniverse;
  /** The system the camera is in, by its ID. */
  readonly system: string;
  /** The body flown, by its ID, or `null` where the camera is beside the system's barycentre. */
  readonly body: string | null;
  /** The record's `scene.world`: the body's ID, or the system's. */
  readonly world: string;
  readonly script: PerfSceneScript;
  readonly span: PerfSpan;
  /** The warm-up's pose: the script's first, held still (Design note 13). */
  readonly warmUp: { readonly scriptTimeS: 0 };
  readonly resets: PerfResets;
  /** The views by setting, in R07's order: the primary or the station, then the instruments. */
  readonly views: Readonly<Record<QualitySetting, ReadonlyArray<PerfSceneView>>>;
  readonly pacing: Readonly<Record<QualitySetting, PerfPacing>>;
  readonly criterion: Readonly<Record<QualitySetting, PerfCriterion>>;
  readonly harness: PerfSceneHarness;
}

/** R07's cockpit layout's instrument canvas, device px at a ratio of 1: 15 × 11.25 rem at 16 px. */
export const INSTRUMENT_CANVAS_PX = { widthPx: 240, heightPx: 180 } as const;

/** A station's wireframe, device px: 1080p (Design note 8). */
export const STATION_CANVAS_PX = { widthPx: 1920, heightPx: 1080 } as const;

/** A view before its rate: its role, the style it asks for, and its size. */
interface ViewAsk {
  readonly role: ViewRole;
  readonly style: RenderStyle;
  readonly size: PerfViewSize;
}

/** R07's slot of each role: a station's wireframe is a station machine's primary (R12.T0's record). */
const SLOT_OF: Readonly<Record<ViewRole, ViewSlot>> = {
  primary: "primary",
  station: "primary",
  instrument: "instrument",
};

/**
 * The views of `asks` on `setting` under R07's policy (`viewBudgets`): each drawn in the style the
 * policy allows at the rate it gives.
 */
function policyViews(
  asks: ReadonlyArray<ViewAsk>,
  setting: QualitySetting,
): ReadonlyArray<PerfSceneView> {
  const specs = asks.map((ask, i) => ({
    id: viewId(`${ask.role}-${String(i)}`),
    slot: SLOT_OF[ask.role],
    style: ask.style,
  }));
  const budgets = viewBudgets(specs, setting);
  return asks.map((ask, i): PerfSceneView => {
    const spec = specs[i];
    const budget = spec === undefined ? undefined : budgets.get(spec.id);
    if (budget === undefined) {
      throw new Error(`R07's policy gave the ${ask.role} view no budget`);
    }
    return {
      role: ask.role,
      style: budget.style,
      size: ask.size,
      rateHz: budget.rateHz,
      lineAntialiasing: budget.style === "wireframe" ? "analytic" : "none",
    };
  });
}

/** Both settings' views of `asks`. */
function bySetting(
  asks: ReadonlyArray<ViewAsk>,
): Readonly<Record<QualitySetting, ReadonlyArray<PerfSceneView>>> {
  return { high: policyViews(asks, "high"), low: policyViews(asks, "low") };
}

const WINDOW: PerfViewSize = { kind: "window" };
const INSTRUMENT: PerfViewSize = { kind: "canvas", ...INSTRUMENT_CANVAS_PX };

/**
 * The single-player cockpit's views (Design note 8): one photorealistic view at the window's size
 * and two wireframe instruments on R07's layout.
 */
const COCKPIT_VIEWS = bySetting([
  { role: "primary", style: "photorealistic", size: WINDOW },
  { role: "instrument", style: "wireframe", size: INSTRUMENT },
  { role: "instrument", style: "wireframe", size: INSTRUMENT },
]);

/** A photorealistic primary alone at the window's size. */
const PRIMARY_ALONE = bySetting([{ role: "primary", style: "photorealistic", size: WINDOW }]);

/**
 * The pacing of a scene judged on R05's columns (Design notes 7 and 9): the UHD 620's 720p30 low
 * setting at every second vsync of its 60 Hz display, the discrete target's high at every vsync.
 */
const BY_SETTING_PACING: Readonly<Record<QualitySetting, PerfPacing>> = {
  high: "every-vsync",
  low: "every-second-vsync",
};

/** R05's frame columns by setting (Design note 9). */
const FRAME_COLUMNS: Readonly<Record<QualitySetting, PerfCriterion>> = {
  high: { kind: "frame", column: "1080p60" },
  low: { kind: "frame", column: "720p30" },
};

/** One gigabyte, B: R05's memory rows state their ceilings in GB. */
const GB = 1e9;

/** The descents' span, s: R05's whole script, 1,230 s as built (R12.T0's record). */
export const DESCENT_SPAN_S = DESCENT_SEGMENTS.reduce((sum, { durationS }) => sum + durationS, 0);

/** R05's spike seed, which draws the descents' landing site and approach (`landingSiteOf`). */
export const PERF_LANDING_SEED = "7";

/** R09's survey before `arrival-memory`'s field is posted: orbital, at 50 m, as place-ship's. */
export const ARRIVAL_SURVEY = PERF_SURVEY;

const UNIVERSE: PerfUniverse = { name: PERF_UNIVERSE_NAME, seed: PERF_PINS.seed };

/**
 * Why `nuclear-disc-sky` is judged on nothing (Design note 8): the budget's star-field row is the
 * solar neighbourhood's, which `cockpit-descent` measures; this run records what the other end
 * costs.
 */
export const NUCLEAR_DISC_REASON =
  "the budget's star-field row is the solar neighbourhood's, which cockpit-descent measures; this run records what the nuclear disc's sky costs";

/** Every scene of the catalogue, keyed by its ID. */
export const PERF_SCENES: Readonly<Record<PerfSceneId, PerfScene>> = {
  "cockpit-descent": {
    id: "cockpit-descent",
    version: 1,
    universe: UNIVERSE,
    system: PERF_PINS.system,
    body: PERF_PINS.world,
    world: PERF_PINS.world,
    script: { kind: "descent", landingSeed: PERF_LANDING_SEED, arrivalAtS: 0 },
    span: { kind: "script", seconds: DESCENT_SPAN_S },
    warmUp: { scriptTimeS: 0 },
    resets: { patchCache: true, field: true, server: true },
    views: COCKPIT_VIEWS,
    pacing: BY_SETTING_PACING,
    criterion: FRAME_COLUMNS,
    harness: PERF_SCENE_HARNESS["cockpit-descent"],
  },
  "station-wireframe-descent": {
    id: "station-wireframe-descent",
    version: 1,
    universe: UNIVERSE,
    system: PERF_PINS.system,
    body: PERF_PINS.world,
    world: PERF_PINS.world,
    script: { kind: "descent", landingSeed: PERF_LANDING_SEED, arrivalAtS: 0 },
    span: { kind: "script", seconds: DESCENT_SPAN_S },
    warmUp: { scriptTimeS: 0 },
    resets: { patchCache: true, field: true, server: true },
    views: bySetting([
      { role: "station", style: "wireframe", size: { kind: "canvas", ...STATION_CANVAS_PX } },
    ]),
    // A station's wireframe at every vsync on both settings (Design note 9).
    pacing: { high: "every-vsync", low: "every-vsync" },
    criterion: {
      high: { kind: "frame", column: "station-1080p60" },
      low: { kind: "frame", column: "station-1080p60" },
    },
    harness: PERF_SCENE_HARNESS["station-wireframe-descent"],
  },
  "ringed-giant-approach": {
    id: "ringed-giant-approach",
    version: 1,
    universe: UNIVERSE,
    system: PERF_PINS.system,
    body: PERF_PINS.giant,
    world: PERF_PINS.giant,
    script: { kind: "approach" },
    span: { kind: "script", seconds: APPROACH_DURATION_S },
    warmUp: { scriptTimeS: 0 },
    // A giant has no surface: no field is posted, and the cache holds none of its patches.
    resets: { patchCache: true, field: false, server: true },
    views: PRIMARY_ALONE,
    pacing: BY_SETTING_PACING,
    criterion: FRAME_COLUMNS,
    harness: PERF_SCENE_HARNESS["ringed-giant-approach"],
  },
  "arrival-memory": {
    id: "arrival-memory",
    version: 1,
    universe: UNIVERSE,
    system: PERF_PINS.system,
    body: PERF_PINS.world,
    world: PERF_PINS.world,
    script: { kind: "arrival", landingSeed: PERF_LANDING_SEED, survey: ARRIVAL_SURVEY },
    span: { kind: "arrival", heldS: ARRIVAL_HELD_S },
    warmUp: { scriptTimeS: 0 },
    resets: { patchCache: true, field: true, server: true },
    views: COCKPIT_VIEWS,
    pacing: BY_SETTING_PACING,
    // R05's memory rows (Design note 9): on the discrete target 3 GB fails and 2 GB is a finding;
    // on the low setting 1 GB, both.
    criterion: {
      high: { kind: "memory", ceilingBytes: 3 * GB, findingBytes: 2 * GB },
      low: { kind: "memory", ceilingBytes: 1 * GB, findingBytes: 1 * GB },
    },
    harness: PERF_SCENE_HARNESS["arrival-memory"],
  },
  "nuclear-disc-sky": {
    id: "nuclear-disc-sky",
    version: 1,
    universe: UNIVERSE,
    system: PERF_PINS.nuclearSystem,
    body: null,
    world: PERF_PINS.nuclearSystem,
    script: { kind: "sweep" },
    span: { kind: "script", seconds: SWEEP_DURATION_S },
    warmUp: { scriptTimeS: 0 },
    resets: { patchCache: true, field: false, server: true },
    views: PRIMARY_ALONE,
    pacing: BY_SETTING_PACING,
    criterion: {
      high: { kind: "informative", reason: NUCLEAR_DISC_REASON },
      low: { kind: "informative", reason: NUCLEAR_DISC_REASON },
    },
    harness: PERF_SCENE_HARNESS["nuclear-disc-sky"],
  },
};
