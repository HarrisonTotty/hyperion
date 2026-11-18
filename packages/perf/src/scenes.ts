/**
 * The performance scenes' IDs and the criteria they are judged on (plan R12, Design notes 8 and 9).
 *
 * @remarks
 * The scenes themselves (seeds, pins, camera scripts, views, pacing) are the renderer's
 * `view/perf/scenes.ts` (R12.T5.a), keyed by {@link PerfSceneId}; this module holds only what the
 * record, the table and the comparison need to name a scene.
 */

/**
 * Every scene of the catalogue, in Design note 8's order: the order the tables list scenes in.
 *
 * @remarks
 * `cockpit-descent` is the single-player cockpit with its two wireframe instruments over R05's
 * descent; `station-wireframe-descent` a station's wireframe alone over the same descent;
 * `ringed-giant-approach` a ringed gas giant from 10¹⁰ m to inside its rings' annulus;
 * `arrival-memory` the resident memory's peak after a full survey's field reaches three workers;
 * `nuclear-disc-sky` an informative sky sweep in the nuclear disc.
 */
export const PERF_SCENE_IDS = [
  "cockpit-descent",
  "station-wireframe-descent",
  "ringed-giant-approach",
  "arrival-memory",
  "nuclear-disc-sky",
] as const;

/** A scene of the catalogue. */
export type PerfSceneId = (typeof PERF_SCENE_IDS)[number];

/** Whether a string names a scene of the catalogue. */
export function isPerfSceneId(value: string): value is PerfSceneId {
  return PERF_SCENE_IDS.some((id) => id === value);
}

/**
 * What a scene is judged on (Design note 9): R05's frame criterion at one of its columns, the
 * memory ceiling, or nothing, with the reason.
 *
 * @remarks
 * `frame`'s columns are R05 Design note 21's: `1080p60` on the discrete target, `720p30` on the
 * UHD 620's low setting, and `station-1080p60`, a station's wireframe at every vsync on the UHD 620.
 * `memory`'s `ceilingBytes` fails a resident peak above it and `findingBytes` makes one above it a
 * finding (on the discrete target 3 GB and 2 GB; on the low setting both 1 GB).
 */
export type PerfCriterion =
  | { readonly kind: "frame"; readonly column: "1080p60" | "720p30" | "station-1080p60" }
  | { readonly kind: "memory"; readonly ceilingBytes: number; readonly findingBytes: number }
  | { readonly kind: "informative"; readonly reason: string };

/**
 * Where a scene's server runs, and its CPU pool's `--num-workers` (Design notes 8 and 13), for the
 * main process, which launches and restarts it.
 *
 * @remarks
 * `local`: on the machine, as single-player has it. `co-resident`: a station's stand-in, run on the
 * station's machine pinned to one core, which the table shows but never counts as the station's
 * figure; a station's own server is on a second machine (`remote`), restarted by its operator's
 * script, which a launch names with `--address`.
 */
export interface PerfSceneServer {
  readonly placement: "local" | "co-resident";
  readonly numWorkers: number;
}

/**
 * The pins (R12.T5.b): the universe's seed, the system and the bodies the scenes fly, at generator
 * version 31, in the wire's text forms. `crates/hyperion-server/tests/perf_scenes.rs` holds them
 * to what each scene needs, and a Vitest (`scenes.pins.test.ts`) holds its constants and these
 * equal; a generator bump that moves one re-pins it and bumps the scenes' versions.
 */
export const PERF_PINS = {
  /** The universe's seed, 16 lowercase hexadecimal digits. */
  seed: "00000000000004d2",
  /**
   * A system near the Sun holding the Earth-sized world and the ringed giant: one F5.5 V star at a
   * galactocentric radius of 26,128 ly.
   */
  system: "42106cb600400013",
  /**
   * An Earth-sized world with an atmosphere, an ocean above 0.3 of its surface, clouds and ice:
   * 1.10 R⊕, 124 kPa, ocean 0.61, ice 0.14, cloud 0.67 at version 31.
   */
  world: "42106cb600400013.0200",
  /** A gas giant with rings, in the same system: 2.2 Jupiter masses, its massive ring 1.10–1.75 R_p. */
  giant: "42106cb600400013.0400",
  /** A system of the nuclear disc, beside whose barycentre the sky is swept: 117 pc from the centre. */
  nuclearSystem: "4200202e000030eb",
} as const;

/** The name a scene's universe is made with on its server's fresh data directory. */
export const PERF_UNIVERSE_NAME = "HYPERION PERF";

/** R09's survey of a world before its descent: orbital at 50 m, place-ship's (`--survey`). */
export const PERF_SURVEY = { source: "orbital", resolutionM: 50 } as const;

/**
 * Where a scene's arrival places the ship (Design notes 8 and 13; `just place-ship`): beside a
 * pinned body, surveyed first where the scene flies its terrain, or beside a system's barycentre.
 */
export type PerfPlacement =
  | {
      readonly kind: "body";
      readonly body: string;
      readonly survey: typeof PERF_SURVEY | null;
    }
  | { readonly kind: "barycentre"; readonly system: string };

/**
 * What the main process needs of each scene (Design notes 8 and 13); the rest of a scene, its
 * script and views, is the renderer's `view/perf/scenes.ts`.
 *
 * @remarks
 * `heightWorkers` is the scene's height-worker count, or `null` for the machine's default, a
 * player's (`defaultHeightWorkers`); the station and the arrival take three (Design note 8).
 */
export interface PerfSceneHarness {
  readonly server: PerfSceneServer;
  readonly heightWorkers: number | null;
  readonly placement: PerfPlacement;
}

/** The descents' and the arrival's placement: beside the world, its field surveyed first. */
const AT_THE_WORLD: PerfPlacement = { kind: "body", body: PERF_PINS.world, survey: PERF_SURVEY };

/** Each scene's server, height workers and arrival. */
export const PERF_SCENE_HARNESS: Readonly<Record<PerfSceneId, PerfSceneHarness>> = {
  "cockpit-descent": {
    server: { placement: "local", numWorkers: 2 },
    heightWorkers: null,
    placement: AT_THE_WORLD,
  },
  "station-wireframe-descent": {
    server: { placement: "co-resident", numWorkers: 1 },
    heightWorkers: 3,
    placement: AT_THE_WORLD,
  },
  "ringed-giant-approach": {
    server: { placement: "local", numWorkers: 2 },
    heightWorkers: null,
    placement: { kind: "body", body: PERF_PINS.giant, survey: null },
  },
  "arrival-memory": {
    server: { placement: "local", numWorkers: 2 },
    heightWorkers: 3,
    placement: AT_THE_WORLD,
  },
  "nuclear-disc-sky": {
    server: { placement: "local", numWorkers: 2 },
    heightWorkers: null,
    placement: { kind: "barycentre", system: PERF_PINS.nuclearSystem },
  },
};
