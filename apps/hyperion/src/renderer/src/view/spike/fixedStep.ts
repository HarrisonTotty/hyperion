/**
 * The descent's fixed-step run (plan R05, T13.a, Design note 19): the scripted path sampled at a
 * fixed rate through `selectPatches` and the terrain's own patch cache, CPU-only, so that the
 * selection sequence is identical bit for bit and its measured demand can be set beside the
 * prediction.
 *
 * @remarks
 * Each frame runs the terrain pass's order (T8, T11.c): select with the cache's baked ranges as
 * `heightRanges`, resolve the draw set, `retain` the selection and the draw set, then bake. The
 * pool is ideal, stated: every patch the selection requests is baked and inserted the same frame
 * (no bake latency, no worker limit), with the range `rangeOf` gives; the demand's breadth-first
 * rule still makes a deep patch wait a frame for each unbaked ancestor. So the measured demand is
 * the patches baked a second under an ideal pool, the plan's first-time-selected keys a second
 * with the cache's semantics. Each frame records the patches selected, the bakes, `limited`, the
 * draw set's stand-ins and missing patches, and how long `selectPatches` took; the bakes' own time
 * is kept out of the wall-time deadline, so that the cells' coverage compares.
 */

import { vec3 } from "../../geometry/vec3";
import type { QualitySetting } from "../quality/qualitySetting";
import { DrawSetResolver, PatchCache } from "../terrain/cache";
import {
  finestPatchSizeM,
  type GroundContact,
  heldRadiusM,
  isDescending,
} from "../terrain/grounded";
import { type PatchKey, patchKeyString } from "../terrain/patchKey";
import type { PlanetGeometry } from "../terrain/planet";
import { type SelectionInput, selectPatches } from "../terrain/select";
import type { SlotLayout } from "../terrain/slotLayout";
import { type DemandView, perLevelDemand } from "./demand";
import type { DescentPose, DescentProfile } from "./descentProfile";
import { SPIKE_CRAFT_RADIUS_M } from "./spikeScene";

/**
 * The scripted craft as the selection's contacts (Design note 9): one contact at the ground point
 * beneath it while it is descending (`isDescending`) or grounded, none otherwise.
 *
 * @remarks
 * Both read the height above the floor under the craft (`heightAboveFloorM`, the clearance ruling's
 * honest h, never above the true height). Grounded is that height within the held radius r_g, the craft's radius (the scene's hull, `SPIKE_CRAFT_RADIUS_M`) plus one
 * finest patch:
 * the held sphere about the craft then reaches the ground, as it does at an exact hover, where the
 * vertical speed is 0 and `isDescending` does not hold. The contact sits on the ground, not at the
 * camera, since the forced region's rule is 3-D (T11.c's note).
 *
 * @param patchSizeM - The finest patch's edge, metres (`finestPatchSizeM`).
 */
export function craftContacts(pose: DescentPose, patchSizeM: number): GroundContact[] {
  const contact: GroundContact = { positionM: pose.groundPointM, radiusM: SPIKE_CRAFT_RADIUS_M };
  const heightM = pose.heightAboveFloorM;
  const grounded = heightM <= heldRadiusM(contact, patchSizeM);
  return grounded || isDescending(heightM, pose.verticalSpeedMps) ? [contact] : [];
}

/** What a fixed-step run reads. */
export interface FixedStepOptions {
  readonly profile: DescentProfile;
  /** The planet under the bound rule the run records (`boundedPlanet`). */
  readonly planet: PlanetGeometry;
  readonly setting: QualitySetting;
  readonly view: DemandView & { readonly heightPx: number };
  /** Samples a second (64 in the plan). */
  readonly rateHz: number;
  /** The span sampled, script seconds. */
  readonly fromS: number;
  readonly toS: number;
  /**
   * Where measuring starts, script seconds: the frames before it fill the cache, whose first
   * bakes are the run's start rather than the motion, and are marked `warmup`.
   */
  readonly measureFromS: number;
  /** The patch budget, or `undefined` for none. */
  readonly maxPatches: number | undefined;
  /** The cache's slots, the setting's own. */
  readonly layout: SlotLayout;
  /** A baked patch's lowest and highest height, metres. */
  readonly rangeOf: (key: PatchKey) => readonly [number, number];
  /** The clock `selectPatches` and the bakes are timed by, ms. */
  readonly nowMs: () => number;
  /**
   * The wall time on `nowMs`'s clock after which the run stops, cut short, the bakes' time not
   * counted; none by default.
   */
  readonly deadlineMs?: number;
}

/** One frame of a run. */
export interface FixedStepFrame {
  readonly tS: number;
  /** Whether the frame is before `measureFromS`, filling the cache. */
  readonly warmup: boolean;
  readonly segment: string;
  readonly patches: number;
  /** Patches requested and baked this frame. */
  readonly demanded: number;
  readonly limited: boolean;
  /** Selected patches drawn by a resident ancestor, and those with none. */
  readonly standingIn: number;
  readonly missing: number;
  readonly selectMs: number;
  /** The per-level prediction at this frame, patches a second, at the height above the floor. */
  readonly predictedPerS: number;
  /** The camera's height above the floor under it, metres (`DescentPose.heightAboveFloorM`). */
  readonly heightAboveFloorM: number;
}

/** A run's frames and its selection sequence's hash. */
export interface FixedStepRun {
  readonly frames: ReadonlyArray<FixedStepFrame>;
  /** FNV-1a (64-bit) over every frame's selected keys in their order, as 16 hex digits. */
  readonly hash: string;
  /** Whether `deadlineMs` cut the run before `toS`. */
  readonly truncated: boolean;
}

/** FNV-1a 64-bit, in two 32-bit halves so that each byte costs no `bigint` work. */
class Fnv64 {
  #hi = 0xcbf2_9ce4;
  #lo = 0x8422_2325;

  /** Folds one byte. */
  byte(b: number): void {
    // h ^= b; h *= 0x100000001b3 = 2^40 + 0x1b3, as (hi, lo) halves mod 2^64.
    const lo = (this.#lo ^ b) >>> 0;
    const hi = this.#hi;
    const loTimes = lo * 0x1b3;
    const newLo = loTimes >>> 0;
    const carry = Math.floor(loTimes / 0x1_0000_0000);
    this.#hi = (Math.imul(hi, 0x1b3) + carry + ((lo << 8) >>> 0)) >>> 0;
    this.#lo = newLo;
  }

  /** Folds a string's UTF-16 code units, each as one byte (keys are ASCII), then a separator. */
  text(s: string): void {
    for (let i = 0; i < s.length; i += 1) {
      this.byte(s.charCodeAt(i) & 0xff);
    }
    this.byte(0x0a);
  }

  hex(): string {
    return this.#hi.toString(16).padStart(8, "0") + this.#lo.toString(16).padStart(8, "0");
  }
}

const ORIGIN = vec3(0, 0, 0);

/** Runs the descent at a fixed step from `fromS` to `toS`. */
export function runFixedStep(options: FixedStepOptions): FixedStepRun {
  const { profile, planet, view } = options;
  const cache = new PatchCache(options.layout);
  const resolver = new DrawSetResolver(cache);
  const frames: FixedStepFrame[] = [];
  const hash = new Fnv64();
  const steps = Math.round((options.toS - options.fromS) * options.rateHz);
  const deadline = options.deadlineMs ?? Infinity;
  const patchSizeM = finestPatchSizeM(planet);
  let bakeMs = 0;
  let truncated = false;
  for (let step = 0; step <= steps; step += 1) {
    if (options.nowMs() - bakeMs > deadline) {
      truncated = true;
      break;
    }
    const tS = options.fromS + step / options.rateHz;
    const pose = profile.poseAt(tS);
    const grounded = craftContacts(pose, patchSizeM);
    const input: SelectionInput = {
      planet,
      views: [
        {
          camera: { positionM: pose.positionM, orientation: pose.orientation },
          fovXRad: view.fovXRad,
          viewport: { widthPx: view.widthPx, heightPx: view.heightPx },
          weight: 1,
          tauPx: view.tauPx,
        },
      ],
      setting: options.setting,
      grounded,
      heightRanges: cache,
      ...(options.maxPatches === undefined ? {} : { maxPatches: options.maxPatches }),
    };
    const start = options.nowMs();
    const selection = selectPatches(input);
    const selectMs = options.nowMs() - start;
    for (const keyString of selection.patches.keys()) {
      hash.text(keyString);
    }
    hash.text("|");
    const draw = resolver.resolve(selection);
    cache.retain(selection, draw);
    let demanded = 0;
    for (const request of selection.demand) {
      if (cache.has(patchKeyString(request.key))) {
        continue;
      }
      const bakeStart = options.nowMs();
      const heightRangeM = options.rangeOf(request.key);
      bakeMs += options.nowMs() - bakeStart;
      cache.insert({
        key: request.key,
        generation: 0,
        originM: ORIGIN,
        heightRangeM,
        boundingRadiusM: 0,
      });
      demanded += 1;
    }
    frames.push({
      tS,
      warmup: tS < options.measureFromS,
      segment: pose.segment,
      patches: selection.patches.size,
      demanded,
      limited: selection.limited,
      standingIn: draw.standingIn,
      missing: draw.missing,
      selectMs,
      predictedPerS: perLevelDemand(planet, { ...pose, altitudeM: pose.heightAboveFloorM }, view)
        .perS,
      heightAboveFloorM: pose.heightAboveFloorM,
    });
  }
  return { frames, hash: hash.hex(), truncated };
}

/** A segment's figures over a run. */
export interface SegmentFigures {
  readonly segment: string;
  readonly frames: number;
  readonly meanPatches: number;
  readonly maxPatches: number;
  /** Measured demand: patches baked over the segment's measured span, a second. */
  readonly demandPerS: number;
  /** The mean per-level prediction over the segment's frames. */
  readonly predictedPerS: number;
  readonly limitedFraction: number;
  /** The share of frames drawing some selected patch by a stand-in. */
  readonly standingInFraction: number;
  /** The least and greatest height above the floor over the frames, metres. */
  readonly minHeightAboveFloorM: number;
  readonly maxHeightAboveFloorM: number;
  readonly selectMsP50: number;
  readonly selectMsP95: number;
  readonly selectMsMax: number;
}

function nearestRank(sorted: ReadonlyArray<number>, p: number): number {
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))] ?? 0;
}

/** A run's figures by segment, over its measured frames (the warm-up left out). */
export function segmentFigures(run: FixedStepRun, rateHz: number): SegmentFigures[] {
  const bySegment = new Map<string, FixedStepFrame[]>();
  for (const frame of run.frames.filter(({ warmup }) => !warmup)) {
    const list = bySegment.get(frame.segment) ?? [];
    list.push(frame);
    bySegment.set(frame.segment, list);
  }
  return [...bySegment].map(([segment, frames]) => {
    const spanS = frames.length / rateHz;
    const times = frames.map(({ selectMs }) => selectMs).toSorted((a, b) => a - b);
    const sum = (pick: (f: FixedStepFrame) => number): number =>
      frames.reduce((total, f) => total + pick(f), 0);
    return {
      segment,
      frames: frames.length,
      meanPatches: sum((f) => f.patches) / frames.length,
      maxPatches: frames.reduce((most, f) => Math.max(most, f.patches), 0),
      demandPerS: spanS > 0 ? sum((f) => f.demanded) / spanS : 0,
      predictedPerS: sum((f) => f.predictedPerS) / frames.length,
      limitedFraction: frames.filter(({ limited }) => limited).length / frames.length,
      standingInFraction: frames.filter(({ standingIn }) => standingIn > 0).length / frames.length,
      minHeightAboveFloorM: Math.min(...frames.map((f) => f.heightAboveFloorM)),
      maxHeightAboveFloorM: Math.max(...frames.map((f) => f.heightAboveFloorM)),
      selectMsP50: nearestRank(times, 0.5),
      selectMsP95: nearestRank(times, 0.95),
      selectMsMax: times.at(-1) ?? 0,
    };
  });
}
