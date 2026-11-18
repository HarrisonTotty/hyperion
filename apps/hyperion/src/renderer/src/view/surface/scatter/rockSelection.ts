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
 *   patch in view outside the region holds a rock that would subtend the tolerance at the patch's
 *   nearest point.
 * - Every answer is a function of its arguments alone: the camera's pose and the view's field of
 *   view and viewport (through the distances and the pixels a radian), the setting and the bodies.
 */

import { type Vec3, norm, sub } from "../../../geometry/vec3";
import type { QualitySetting } from "../../quality/qualitySetting";
import type { DrawSet } from "../../terrain/cache";
import type { GroundContact } from "../../terrain/grounded";

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
}

/** What a view's draw set gives its rocks: the patches drawn by rule, and every finest one in view. */
export interface RocksToDraw {
  readonly patches: ReadonlyArray<RockDrawnPatch>;
  readonly inView: ReadonlyArray<RockPatchInView>;
}

/**
 * The rocks a view of draw-set bit `mask` draws (see the module's remarks): of every patch of
 * `drawSet` the view sees drawn whole at `finestLevel`, its sphere about its origin and its rocks'
 * largest diameter where `meshes` holds its mesh, and, where its rule draws them and its mesh is
 * held, the patch and its rule; in the draw set's order.
 */
export function rocksToDraw(
  drawSet: Pick<DrawSet, "count" | "patches" | "views">,
  mask: number,
  finestLevel: number,
  meshes: { get(keyString: string): { readonly largestRockM: number } | undefined },
  region: ReadonlyArray<GroundContact>,
  setting: QualitySetting,
): RocksToDraw {
  const patches: RockDrawnPatch[] = [];
  const inView: RockPatchInView[] = [];
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
      patches.push({ keyString: drawn.keyString, terrainSlot: drawn.patch.slot, rule });
    }
  }
  return { patches, inView };
}
