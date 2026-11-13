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
