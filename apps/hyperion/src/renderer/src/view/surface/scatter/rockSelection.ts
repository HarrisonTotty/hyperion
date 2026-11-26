/**
 * Which authoritative rocks are drawn (plan R11, T4.b; Design note 7): the region a grounded or
 * descending body could touch, where every rock is drawn at its finest cut on every setting and in
 * both styles, and elsewhere the high setting's rocks out to where they subtend a pixel, the low
 * setting's none, with `TERRAIN: DETAIL LIMITED` wherever a finest patch in view holds rocks that
 * would.
 *
 * @remarks
 * - **Could touch** ({@link couldTouch}): a grounded body's contact footprint, its bounding sphere
 *   at the ground beneath it, widened by the largest rock octave's reach
 *   ({@link LARGEST_ROCK_REACH_M}), so that every rock that could reach under the body is in it.
 *   The region is a list of discs (`GroundContact`s), as R05's forced region takes them: a body's
 *   own, and for a descending body its swept path's (`couldTouchContacts`, R11.T4.d).
 * - **The rule** ({@link rockRule}): a finest patch's rocks are the region's where the patch's
 *   bounding sphere meets one of its discs, drawn at the finest cut whatever the setting (the
 *   cull's tolerance 0); else, on the high setting, its rocks are drawn where each subtends the
 *   camera pass's tolerance (each rock's parent error its largest diameter, `rockPages.ts`); on the
 *   low setting not at all. Both styles take the same rule (lean, 2026-11-18: the brainstorm binds
 *   the region in both and leaves the rest to the setting).
 * - **The annunciation** ({@link rocksLimitDetail}): on the low setting, exactly where a finest
 *   patch in view outside the region holds a rock that would subtend a pixel
 *   ({@link ROCK_DETAIL_PX}) at the patch's nearest point; and on either, where a region patch's
 *   mesh was refused by the rocks' pool (`RocksToDraw.regionRefused`), which holds the region's
 *   patches first (`rockMeshes.ts`), so only where the region alone passes it.
 * - **The swept region** ({@link couldTouchContacts}, R11.T4.d): a descending body's footprint
 *   swept along its path ahead over R05's `FORCED_REGION_RESIDENCY_S`, R03's `predictedPath`: the
 *   planned path where one is sent, else the straight line of its pose at its velocity, which is
 *   widened by the ground's share of {@link extrapolationMargin} ({@link sweptWideningM}). It is
 *   handed to selection as a chain of contacts on the ground beneath the path, so that every
 *   finest patch the body could touch over the horizon is forced, and so resident, before it
 *   arrives; the rocks there are drawn whole once its own contact reaches them.
 * - Every answer is a function of its arguments alone: the camera's pose and the view's field of
 *   view and viewport (through the distances and the pixels a radian), the setting and the bodies.
 */

import { add, type Vec3, norm, scale, sub } from "../../../geometry/vec3";
import type { QualitySetting } from "../../quality/qualitySetting";
import type { DrawSet } from "../../terrain/cache";
import type { GroundContact, GroundUnderFunction } from "../../terrain/grounded";
import type { BodyFigure } from "../../terrain/planet";

/**
 * The reach of the largest rock octave from a rock's axis, m: the surface crate's
 * `REACH_MARGIN × MAX_REACH_SHARE × 6.4 m` (`scatter::place`), 1.1 × 0.7695 × 6.4 = 5.4173 m, past
 * which no rock reaches a point (plan R11, Design note 5).
 */
export const LARGEST_ROCK_REACH_M = 1.1 * 0.7695 * 6.4;

/** A body that may touch the ground: where it would, and its bounding radius. */
export interface RockBody {
  /** The point of the ground beneath it, or where it touches down, body-fixed m. */
  readonly groundPointM: Vec3;
  /** Its bounding radius, m. */
  readonly radiusM: number;
}

/**
 * The region `body` could touch (see the module's remarks): its footprint at the ground beneath it
 * widened by {@link LARGEST_ROCK_REACH_M}, as one disc.
 */
export function couldTouch(body: RockBody): GroundContact {
  return { positionM: body.groundPointM, radiusM: body.radiusM + LARGEST_ROCK_REACH_M };
}

/** A finest patch as the rule reads it: its bounding sphere, body-fixed. */
export interface RockPatchSphere {
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/**
 * How a finest patch's rocks are drawn: `region`, every one at its finest cut; `pixel`, each where
 * it subtends the camera pass's tolerance.
 */
export type RockDrawRule = "region" | "pixel";

/** Whether the patch's sphere meets one of `region`'s discs. */
export function inRegion(patch: RockPatchSphere, region: ReadonlyArray<GroundContact>): boolean {
  return region.some(
    (disc) => norm(sub(patch.centreM, disc.positionM)) <= disc.radiusM + patch.radiusM,
  );
}

/**
 * How the patch's rocks are drawn on `setting` (see the module's remarks), or `null` for not at
 * all.
 */
export function rockRule(
  patch: RockPatchSphere,
  region: ReadonlyArray<GroundContact>,
  setting: QualitySetting,
): RockDrawRule | null {
  if (inRegion(patch, region)) {
    return "region";
  }
  return setting === "high" ? "pixel" : null;
}

/** A finest patch in view as the annunciation reads it. */
export interface RockPatchInView extends RockPatchSphere {
  /** Its rocks' largest diameter, m, 0 for none. */
  readonly largestRockM: number;
}

/**
 * The pixels a rock of `diameterM` subtends at `distanceM`, the view taking `pxPerRad` pixels a
 * radian; a rock at or within its own size of the camera subtends the view.
 */
export function rockPx(diameterM: number, distanceM: number, pxPerRad: number): number {
  return distanceM <= 0 ? Infinity : (pxPerRad * diameterM) / distanceM;
}

/**
 * Whether the view annunciates `TERRAIN: DETAIL LIMITED` for its rocks (see the module's remarks):
 * on the low setting, where a finest patch in view outside the region holds a rock that would
 * subtend `tolerancePx` at the patch's nearest point to `cameraM`; never on the high setting.
 */
export function rocksLimitDetail(
  patches: ReadonlyArray<RockPatchInView>,
  region: ReadonlyArray<GroundContact>,
  setting: QualitySetting,
  cameraM: Vec3,
  pxPerRad: number,
  tolerancePx: number,
): boolean {
  if (setting === "high") {
    return false;
  }
  return patches.some((patch) => {
    if (patch.largestRockM <= 0 || inRegion(patch, region)) {
      return false;
    }
    const nearestM = Math.max(0, norm(sub(patch.centreM, cameraM)) - patch.radiusM);
    return rockPx(patch.largestRockM, nearestM, pxPerRad) >= tolerancePx;
  });
}

/** A finest patch whose rocks a view draws, by its rule (`rockDraw.ts`' `RockFramePatch`). */
export interface RockDrawnPatch {
  readonly keyString: string;
  /** Its terrain slot. */
  readonly terrainSlot: number;
  readonly rule: RockDrawRule;
  /**
   * A sphere that holds its rocks (plan R14, R14.T6.c), body-fixed: its bounding sphere widened by
   * its rocks' largest diameter, since no point of a rock standing on its ground lies farther from
   * that ground; the sphere its cascades are chosen by, as the terrain's patch's own is.
   */
  readonly casterSphere: RockPatchSphere;
}

/**
 * What a view's draw set gives its rocks: the patches drawn by rule, every finest one in view, and
 * whether a patch of the region the view sees has its mesh refused.
 */
export interface RocksToDraw {
  readonly patches: ReadonlyArray<RockDrawnPatch>;
  readonly inView: ReadonlyArray<RockPatchInView>;
  /** Whether a region patch in view has no mesh because the rocks' pool refused it. */
  readonly regionRefused: boolean;
}

/**
 * The size, px, at which a rock the low setting leaves out limits the view's detail: a pixel
 * (Design note 7's "would subtend a pixel").
 */
export const ROCK_DETAIL_PX = 1;

/**
 * The rocks a view of draw-set bit `mask` draws (see the module's remarks): of every patch of
 * `drawSet` the view sees drawn whole at `finestLevel`, its sphere about its origin and its rocks'
 * largest diameter where `meshes` holds its mesh, and, where its rule draws them and its mesh is
 * held, the patch and its rule; in the draw set's order; and whether a region patch's mesh was
 * refused.
 */
export function rocksToDraw(
  drawSet: Pick<DrawSet, "count" | "patches" | "views">,
  mask: number,
  finestLevel: number,
  meshes: {
    get(keyString: string): { readonly largestRockM: number } | undefined;
    isRefused(keyString: string): boolean;
  },
  region: ReadonlyArray<GroundContact>,
  setting: QualitySetting,
): RocksToDraw {
  const patches: RockDrawnPatch[] = [];
  const inView: RockPatchInView[] = [];
  let regionRefused = false;
  for (let n = 0; n < drawSet.count; n += 1) {
    const drawn = drawSet.patches[n];
    if (
      drawn === undefined ||
      ((drawSet.views[n] ?? 0) & mask) === 0 ||
      drawn.patch.key.level !== finestLevel ||
      drawn.within !== null
    ) {
      continue;
    }
    const held = meshes.get(drawn.keyString);
    const sphere = { centreM: drawn.patch.originM, radiusM: drawn.patch.boundingRadiusM };
    inView.push({ ...sphere, largestRockM: held?.largestRockM ?? 0 });
    const rule = rockRule(sphere, region, setting);
    if (rule !== null && held !== undefined) {
      patches.push({
        keyString: drawn.keyString,
        terrainSlot: drawn.patch.slot,
        rule,
        casterSphere: { centreM: sphere.centreM, radiusM: sphere.radiusM + held.largestRockM },
      });
    }
    regionRefused ||= rule === "region" && held === undefined && meshes.isRefused(drawn.keyString);
  }
  return { patches, inView, regionRefused };
}

/**
 * How far an unpowered arc departs from the straight line of its pose at its velocity within `tS`
 * seconds, metres: ½ g t² (Design note 7), `gravityMps2` the most the gravity along the arc reaches.
 */
export function extrapolationMargin(gravityMps2: number, tS: number): number {
  return 0.5 * gravityMps2 * tS * tS;
}

/**
 * How far the ground beneath an unpowered arc departs from the ground beneath its straight line
 * at `tS`, metres: the {@link extrapolationMargin} m times the sine of the most its direction can
 * turn from the datum's normal under the line, at most m, and, in a body's turning frame, the
 * drift of its Coriolis and centrifugal pulls.
 *
 * @remarks
 * Gravity pulls the arc towards the body's centre, so it falls from the line along the vertical
 * and the ground beneath the two is the same but for the turn between that pull and the normal
 * beneath the line: the radial directions along the arc turn from the line's by at most
 * (|p(t) − p(0)| + m) ÷ c, c the datum's polar radius, the least distance of the ground above
 * the datum from the centre; and the radial direction and gravity each stand off the datum's
 * normal by at most δ = atan((a² − c²) ÷ 2ac), a spheroid's greatest difference of geodetic and
 * geocentric latitude, 0 on a sphere. So the widening is m · min(1, (|p(t) − p(0)| + m) ÷ c + 2δ)
 * (lean, 2026-11-18): ½ g t² as a ground radius would force the finest level over 1.7 km² at 30 s
 * on a Moon and 61 km² on an Earth, some 3 × 10³ and 10⁵ finest patches. A line in the body's
 * turning frame at ω also drifts by its Coriolis pull, at most 2ω times the speed v₀ + g t, so by
 * ω t² (v₀ + g t ÷ 3), and by its centrifugal pull, at most ω² (a + |p(t) − p(0)|), so by
 * ½ ω² (a + |p(t) − p(0)|) t² (Goldstein, Poole and Safko 2002, _Classical Mechanics_, 3rd ed.,
 * §4.10), both taken whole.
 *
 * @param travelledM - |p(t) − p(0)|, the line's distance from the pose, m.
 * @param spinRadPerS - The body's rotation rate, rad/s, the frame's; 0 for a frame that does not
 *   turn.
 * @param speedMps - The line's speed v₀, m/s.
 */
export function sweptWideningM(
  figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">,
  gravityMps2: number,
  tS: number,
  travelledM: number,
  spinRadPerS = 0,
  speedMps = 0,
): number {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const m = extrapolationMargin(gravityMps2, tS);
  const offNormal = Math.atan(Math.abs(a * a - c * c) / (2 * a * c));
  const fall = m * Math.min(1, (travelledM + m) / Math.min(a, c) + 2 * offNormal);
  const w = spinRadPerS;
  const coriolis = w * tS * tS * (speedMps + (gravityMps2 * tS) / 3);
  const centrifugal = 0.5 * w * w * (Math.max(a, c) + travelledM) * tS * tS;
  return fall + coriolis + centrifugal;
}

/**
 * A descending body's path ahead, body-fixed m (R03's `predictedPath`): its pose's straight line at
 * its velocity, with the most gravity reaches along it (`null` where the body's gravity is not
 * known, when the line is not widened), or the planned path's positions in order.
 */
export type SweptPath =
  | {
      readonly kind: "extrapolated";
      readonly positionM: Vec3;
      /** Its velocity against the ground, body-fixed m/s. */
      readonly velocityMps: Vec3;
      readonly gravityMps2: number | null;
    }
  | { readonly kind: "planned"; readonly positionsM: ReadonlyArray<Vec3> };

/** A descending body as the sweep takes it. */
export interface SweptBody {
  /** Its bounding radius, m. */
  readonly radiusM: number;
  readonly path: SweptPath;
}

/** The body's ground the sweep is laid on. */
export interface SweptSurface {
  readonly figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">;
  /** The ground under a point: the workers' where it is held, the datum's elsewhere. */
  readonly ground: GroundUnderFunction;
  /** The finest patch's edge, m (`finestPatchSizeM`): R05's forced region reaches 3 past a contact. */
  readonly patchSizeM: number;
  /**
   * The body's rotation rate, rad/s, where the paths are in its turning frame, which widens an
   * extrapolation by its Coriolis and centrifugal drift ({@link sweptWideningM}); absent, 0.
   */
  readonly spinRadPerS?: number;
  /**
   * The most finest patches the chain may force, by its contacts' new ground, beyond which it is
   * cut, in time order; absent, no limit.
   */
  readonly maxForcedPatches?: number;
}

/** A sample of a path: its ground point, its footprint there, m, and whether it could touch. */
interface SweepSample {
  readonly groundM: Vec3;
  readonly footprintM: number;
  /**
   * Whether the body could be within its footprint and the reach of the ground there: the path no
   * higher above it than the footprint, the reach and the arc's fall, and not yet through it.
   */
  readonly near: boolean;
}

/**
 * The finest steps a path is sampled at: an eighth of the farthest a contact may stand from a
 * sample it covers.
 */
const SAMPLES_A_COVER = 8;

/** The samples of `path` over `sweepS` at most `stepM` apart along it, each footprint `radiusM` widened. */
function sweepSamples(
  path: SweptPath,
  radiusM: number,
  sweepS: number,
  stepM: number,
  surface: SweptSurface,
): SweepSample[] {
  const samples: SweepSample[] = [];
  const at = (positionM: Vec3, footprintM: number, fallM: number): void => {
    const under = surface.ground(positionM);
    const reachM = radiusM + LARGEST_ROCK_REACH_M;
    samples.push({
      groundM: under.groundPointM,
      footprintM,
      near: under.heightM - fallM <= reachM && under.heightM >= -reachM,
    });
  };
  switch (path.kind) {
    case "extrapolated": {
      const speed = norm(path.velocityMps);
      const steps = Math.max(1, Math.ceil((speed * sweepS) / stepM));
      for (let k = 0; k <= steps; k += 1) {
        const tS = (sweepS * k) / steps;
        const move = scale(path.velocityMps, tS);
        const g = path.gravityMps2;
        const widening =
          g === null
            ? 0
            : sweptWideningM(surface.figure, g, tS, norm(move), surface.spinRadPerS ?? 0, speed);
        at(
          add(path.positionM, move),
          radiusM + widening,
          g === null ? 0 : extrapolationMargin(g, tS),
        );
      }
      break;
    }
    case "planned": {
      const [first] = path.positionsM;
      if (first !== undefined) {
        at(first, radiusM, 0);
      }
      for (let n = 1; n < path.positionsM.length; n += 1) {
        const from = path.positionsM[n - 1];
        const to = path.positionsM[n];
        if (from === undefined || to === undefined) {
          continue;
        }
        const leg = sub(to, from);
        const steps = Math.max(1, Math.ceil(norm(leg) / stepM));
        for (let k = 1; k <= steps; k += 1) {
          at(add(from, scale(leg, k / steps)), radiusM, 0);
        }
      }
      break;
    }
  }
  return samples;
}

/** The ground point of sample `k` of `samples`, which must be there. */
function groundOf(samples: ReadonlyArray<SweepSample>, k: number): Vec3 {
  const sample = samples[k];
  if (sample === undefined) {
    throw new Error(`a sweep has no sample ${String(k)}`);
  }
  return sample.groundM;
}

/**
 * The chain of contacts that covers what `bodies` could touch over `sweepS` seconds ahead (see the
 * module's remarks), on the ground beneath their paths: what R05's selection takes beside the
 * bodies' own (`SelectionInput.grounded`).
 *
 * @remarks
 * - **Where**: only over the stretches of the path where the body could be within its footprint
 *   and the reach of the ground: no higher above it than those and, on an extrapolation with its
 *   gravity known, the {@link extrapolationMargin} it could have fallen, and not yet through it,
 *   past which a body on that line would have touched down already (lean, 2026-11-18).
 * - **The chain**: each contact stands on a sample of a stretch's ground track, its radius the
 *   largest footprint of the samples it covers and of the one after, and every sample lies within
 *   L = 3 P − reach − s of a contact that covers it, P the finest patch's edge, reach
 *   {@link LARGEST_ROCK_REACH_M} and s the sampling step: so R05's forced region about the chain,
 *   a contact's radius and 3 P, holds every point within the footprint and the reach of the track
 *   at every time of the stretch (lean, 2026-11-18: contacts up to 2 L apart rather than one
 *   radius, which at 300 m/s would be some 800 contacts, each selection's forced test looping over
 *   them all).
 * - **The cap**: where `surface.maxForcedPatches` is given, the chain stops at the first contact
 *   whose new ground, 2 (radius + 3 P) times its step from the one before, would take the patches
 *   it forces past it; the bodies in order, each path in time.
 * - A planned path is swept whole, its poses carrying no times, and not widened.
 *
 * @param sweepS - The horizon, s: R05's `FORCED_REGION_RESIDENCY_S`.
 * @throws RangeError where the patch is too small for the forced region to reach past the reach.
 */
export function couldTouchContacts(
  bodies: ReadonlyArray<SweptBody>,
  sweepS: number,
  surface: SweptSurface,
): GroundContact[] {
  const patchM = surface.patchSizeM;
  const coverM = 3 * patchM - LARGEST_ROCK_REACH_M;
  if (!(coverM > 0)) {
    throw new RangeError(`a finest patch of ${String(patchM)} m forces too little`);
  }
  const stepM = coverM / SAMPLES_A_COVER;
  const reachM = coverM - stepM;
  const contacts: GroundContact[] = [];
  let forcedPatches = 0;
  const budget = surface.maxForcedPatches ?? Infinity;
  for (const body of bodies) {
    const samples = sweepSamples(body.path, body.radiusM, sweepS, stepM, surface);
    let previous: GroundContact | null = null;
    let first = 0;
    while (first < samples.length) {
      if (samples[first]?.near !== true) {
        previous = null;
        first += 1;
        continue;
      }
      const start = groundOf(samples, first);
      // The farthest sample within reach of the first uncovered one stands for both sides.
      let anchor = first;
      while (
        samples[anchor + 1]?.near === true &&
        norm(sub(groundOf(samples, anchor + 1), start)) <= reachM
      ) {
        anchor += 1;
      }
      const centre = groundOf(samples, anchor);
      let last = anchor;
      while (
        samples[last + 1]?.near === true &&
        norm(sub(groundOf(samples, last + 1), centre)) <= reachM
      ) {
        last += 1;
      }
      let radiusM = 0;
      for (let k = first; k <= Math.min(last + 1, samples.length - 1); k += 1) {
        radiusM = Math.max(radiusM, samples[k]?.footprintM ?? 0);
      }
      const forcedM = radiusM + 3 * patchM;
      const newM2 =
        previous === null
          ? Math.PI * forcedM * forcedM
          : 2 * forcedM * norm(sub(centre, previous.positionM));
      forcedPatches += newM2 / (patchM * patchM);
      if (forcedPatches > budget) {
        return contacts;
      }
      previous = { positionM: centre, radiusM };
      contacts.push(previous);
      first = last + 1;
    }
  }
  return contacts;
}
