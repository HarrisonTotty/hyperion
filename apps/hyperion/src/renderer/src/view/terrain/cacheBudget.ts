/**
 * The terrain cache's sizing arithmetic (plan R10, R10.T14.a, Design note 15): how many quadtree
 * patches a camera selects, the bytes one cache slot holds, the sizing rule over them, what the
 * caches allocate, and the most slots a storage binding holds.
 *
 * @remarks
 * Pure: no GPU, no module state. Nothing here feeds `SETTINGS`: the budgets in
 * `quality/qualitySetting.ts` stay provisional until R10.T14.b measures them on the desktop, and
 * this module's figures are recorded beside them (decision-r10-t14a, R2).
 *
 * The count model is built on selection's own metric (decision-r10-t14a, R7): R05's box distance
 * (`bounds.ts`' {@link patchBounds}, {@link relativeBounds} and {@link distanceToBoxM}) at Design
 * note 15's bound alone, R05's horizon and frustum predicates (`cull.ts`), over a premise planet.
 * It leaves out what `selectPatches` adds: the 2:1 balance, the chord sagitta, the forced region
 * under grounded bodies, the survey and the patch budget. Relief is 0 unless `reliefM` is given. It
 * also counts on the exact horizon, Design note 15's premise, against which the note's table is
 * held (decision-r10-t14a, the #56 follow-up).
 */

import { cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { IDENTITY_QUATERNION, lookAlong } from "../camera/quaternion";
import type { ViewSize } from "../engine/types";
import { type CameraRelativeBounds, distanceToBoxM, patchBounds, relativeBounds } from "./bounds";
import { faceUvToXyz, PATCH_QUADS, vertexDir, vertexSpacing } from "./cube";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import { childKeys, FACES, MAX_LEVEL, type PatchKey, rootKey } from "./patchKey";
import {
  type BodyFixedVec3,
  LEVEL_TABLE_STRIDE,
  levelBoundM,
  type PlanetGeometry,
  planetGeometry,
  subtreeHeightRangeM,
  surfacePoint,
} from "./planet";
import { screenSpaceErrorPx, type ViewSelectionInput } from "./select";
import {
  type SlotField,
  type SlotLayout,
  type TerrainShading,
  terrainSlotFields,
} from "./slotLayout";
import type { TerrainSettings } from "../quality/qualitySetting";

/** What {@link bytesPerPatch} sizes: a terrain setting (or the spike's variant) and its shading. */
export interface PatchBytesOptions {
  readonly terrain: TerrainSettings;
  readonly shading: TerrainShading;
}

/**
 * The bytes one cache slot holds for a setting and shading, as stored: the sum of
 * `terrainSlotFields(terrain, shading)`, the one list of fields (`slotLayout.ts`).
 *
 * @remarks
 * Unlike `terrainSlotLayout`, it needs no budget, so it never throws for one too small to hold six
 * slots.
 */
export function bytesPerPatch(options: PatchBytesOptions): number {
  let bytes = 0;
  for (const field of terrainSlotFields(options.terrain, options.shading)) {
    bytes += field.bytes;
  }
  return bytes;
}

/**
 * The equatorial radius of WGS 84, metres (NIMA TR8350.2, Table 3.1), the count model's default
 * radius: the test planet's `a` (`spike/testPlanetFigure.ts`' `TEST_PLANET_FIGURE`), taken here as
 * a sphere's.
 */
export const WGS84_EQUATORIAL_RADIUS_M = 6_378_137;

/**
 * k₀, the count model's default bound ratio: a patch is accepted at a distance of 5 times its
 * level's mean edge at 1 px in the reference view (Design note 15's premise, k = 5 at 1 px 1080p and
 * 60°).
 */
export const COUNT_MODEL_EDGES_AT_ONE_PX = 5;

/** The width of Design note 15's reference view, px: 1080p's 1,920 columns. */
export const REFERENCE_VIEW_WIDTH_PX = 1920;

/** The horizontal field of view of Design note 15's reference view, rad: 60°. */
export const REFERENCE_VIEW_FOV_X_RAD = Math.PI / 3;

/**
 * P₀, the reference view's pixels a radian at its centre, W ÷ (2 tan(fov ÷ 2)): 1,662.8 px rad⁻¹
 * for Design note 15's 1,920 px across 60°, what its k = 5 at 1 px is measured in.
 */
export const REFERENCE_PX_PER_RAD =
  REFERENCE_VIEW_WIDTH_PX / (2 * Math.tan(REFERENCE_VIEW_FOV_X_RAD / 2));

/**
 * The patch the count model's camera stands over, at its centre vertex: the one
 * `select.test.ts` and `terrainWireSelection.test.ts` stand over, on face 0 near its middle.
 */
export const COUNT_MODEL_SITE: PatchKey = { face: 0, level: 19, i: 300_001, j: 200_003 };

/**
 * The headings the frustum count looks along, rad from the site face's +u axis towards its +v
 * about the local up: along the grid, at 22.5° and across its diagonal, since the quadtree's count
 * in a frustum depends on the heading against the grid.
 */
export const COUNT_MODEL_HEADINGS_RAD: ReadonlyArray<number> = [0, Math.PI / 8, Math.PI / 4];

/** The pitches the frustum count steps through, from level to the nadir: 1° apart, 91 of them. */
const PITCH_STEPS = 90;

/** The count model's camera: its height over {@link COUNT_MODEL_SITE} and its view. */
export interface CountModelCamera {
  /** The height above the datum over the site, metres, above 0. */
  readonly heightM: number;
  /** The horizontal field of view, rad, in (0, π). */
  readonly fovXRad: number;
  /**
   * The angle each side of the frustum is opened outwards by, rad, at least 0, as `frustumOf`
   * opens it (the terrain pass's motion margin, at most `MAX_FRUSTUM_MARGIN_RAD`); absent, 0
   * (decision-r10-t14a, R3).
   */
  readonly frustumMarginRad?: number;
}

/** The count model's premise planet; each absent field takes Design note 15's value. */
export interface CountModelPremise {
  /** The sphere's radius, metres; {@link WGS84_EQUATORIAL_RADIUS_M} by default. */
  readonly radiusM?: number;
  /** k₀, the bound's ratio at 1 px in the reference view; {@link COUNT_MODEL_EDGES_AT_ONE_PX} by default. */
  readonly edgesAtOnePx?: number;
  /**
   * The relief's half-height, metres, at least 0: every patch's box spans ±`reliefM` about the
   * datum and the horizon's occluder sinks by it; 0 by default, Design note 15's smooth sphere
   * (decision-r10-t14a, R3).
   */
  readonly reliefM?: number;
}

/** One heading's frustum peak over the pitches. */
export interface CountModelPeak {
  /** The heading, rad (one of {@link COUNT_MODEL_HEADINGS_RAD}). */
  readonly headingRad: number;
  /** The pitch of the peak, rad below the horizontal: 0 level, π ÷ 2 the nadir; the first if tied. */
  readonly pitchRad: number;
  /** The leaves in the frustum at the peak, patches. */
  readonly count: number;
  /** The camera's orientation at the peak, body-fixed (looking down its −z, +y up). */
  readonly orientation: Quaternion;
}

/** Design note 15's two figures under one horizon test, and the poses of the frustum's peaks. */
export interface CountModelCounts {
  /** Every leaf above the horizon, patches: the union of the leaves over every view direction. */
  readonly allRound: number;
  /** The least and greatest of the headings' frustum peaks, patches: Design note 15's range form. */
  readonly frustum: readonly [least: number, greatest: number];
  /** Each heading's peak, in {@link COUNT_MODEL_HEADINGS_RAD}' order. */
  readonly peaks: ReadonlyArray<CountModelPeak>;
}

/**
 * What {@link patchCountModel} counts: on selection's own horizon test, and on the exact horizon
 * that Design note 15's premise names.
 *
 * @remarks
 * Its own fields are the counts on `cull.ts`' {@link aboveHorizon}, on each patch's box, which is
 * what `selectPatches` selects and the cache must hold; the sizing rule reads them. R05's bounds
 * added the largest chord between their ring samples, a sixteenth of the patch's edge, to every
 * half-extent, the normal's included, so a coarse patch wholly below the true horizon could have
 * its box's top above it: at 100 m over an Earth about a fifth of the high setting's all-round
 * leaves (654 of 3,014; decision-r10-t14a, the #56 follow-up). Since R10.T14.e the normal takes the
 * boundary's sagitta (`bounds.ts`' `normalMarginM`), millimetres, and the boxes' count there is
 * 2,397, 49 over the exact horizon's: a box's flat top still stands above its patch's curved edges,
 * and its tangents keep the chord. {@link PatchCountModel.exactHorizon} leaves them out.
 */
export interface PatchCountModel extends CountModelCounts {
  /** The camera, body-fixed metres from the body's centre. */
  readonly positionM: BodyFixedVec3;
  /**
   * The same leaves and poses, their refinement and frustum still on R05's boxes, with each leaf's
   * horizon tested on its own surface, exactly: visible where the camera's nadir lies in it or the
   * least angle from the nadir to its four great-circle edges is at most
   * acos(R ÷ (R + h)) + acos(R ÷ (R + H)), H the relief (Design note 15's "exact horizon culling";
   * decision-r10-t14a, the #56 follow-up, §2). With relief its occluder stays the sphere of R, where
   * the boxes' test sinks it to R − H (`horizonCone`).
   */
  readonly exactHorizon: CountModelCounts;
}

/** A finite number at least `least`, or `fallback` where absent. */
function checked(name: string, value: number | undefined, fallback: number, least: number): number {
  const x = value ?? fallback;
  if (!(Number.isFinite(x) && x >= least)) {
    throw new RangeError(
      `the count model's ${name} must be finite and at least ${least}, got ${x}`,
    );
  }
  return x;
}

/**
 * The premise planet: a sphere of `radiusM` whose level table holds ε_n = k₀ eₙ ÷ P₀ (Design note
 * 15's bound alone, eₙ = 64 × `vertexSpacing(radiusM, n).meanM`, the level's mean patch edge,
 * `cube.ts`), heights ±`reliefM` and the level's largest spacing, as
 * `terrainWireSelection.test.ts`' `countModelTable()` builds it.
 */
function premisePlanet(radiusM: number, edgesAtOnePx: number, reliefM: number): PlanetGeometry {
  const table = new Float64Array(LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1));
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const spacing = vertexSpacing(radiusM, level);
    const at = LEVEL_TABLE_STRIDE * level;
    table[at] = (edgesAtOnePx * PATCH_QUADS * spacing.meanM) / REFERENCE_PX_PER_RAD;
    table[at + 1] = -reliefM;
    table[at + 2] = reliefM;
    table[at + 3] = spacing.maxM;
  }
  return planetGeometry({ equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole: null }, table);
}

/**
 * The exact horizon's reach from the nadir, rad: acos(R ÷ (R + h)) + acos(R ÷ (R + H)), the
 * camera's horizon over the sphere and a point H up beyond it (decision-r10-t14a, the #56
 * follow-up, §2).
 *
 * @param radiusM - The sphere's radius R, metres.
 * @param cameraHeightM - The camera's height h above it, metres.
 * @param topHeightM - The leaves' highest height H above it, metres: the relief.
 */
function horizonReachRad(radiusM: number, cameraHeightM: number, topHeightM: number): number {
  return (
    Math.acos(radiusM / (radiusM + cameraHeightM)) + Math.acos(radiusM / (radiusM + topHeightM))
  );
}

/** The angle between unit vectors `a` and `b`, rad: atan2(|a × b|, a · b), good at every angle. */
function angleBetweenRad(a: Vec3, b: Vec3): number {
  return Math.atan2(norm(cross(a, b)), dot(a, b));
}

/**
 * The least angle from the unit vector `p` to the great-circle arc from `a` to `b`, rad: to the
 * foot of the perpendicular on the arc's circle where it falls inside the arc, else to the nearer
 * end. The arc is shorter than π and runs anticlockwise about a × b.
 */
function angleToArcRad(p: Vec3, a: Vec3, b: Vec3): number {
  const normal = normalise(cross(a, b));
  const out = dot(p, normal);
  const foot = sub(p, scale(normal, out));
  if (dot(cross(a, foot), normal) >= 0 && dot(cross(foot, b), normal) >= 0) {
    return Math.atan2(Math.abs(out), norm(foot));
  }
  return Math.min(angleBetweenRad(p, a), angleBetweenRad(p, b));
}

/** The unit direction of `key`'s vertex (`x`, `y`) as a vector. */
function vertexVec(key: PatchKey, x: number, y: number): Vec3 {
  const [dx, dy, dz] = vertexDir(key, x, y);
  return vec3(dx, dy, dz);
}

/**
 * Whether any of `key`'s own surface on the premise's sphere clears the exact horizon: the nadir
 * `nadir` lies in it, or the least angle from the nadir to its four edges is at most `reachRad`
 * ({@link horizonReachRad}). On a sphere every patch edge is a great-circle arc, since a line of
 * constant u or v on the cube lies in a plane through the centre and the warp only reparametrises
 * it (decision-r10-t14a, the #56 follow-up, §2).
 */
function patchAboveHorizon(key: PatchKey, nadir: Vec3, reachRad: number): boolean {
  // Anticlockwise seen from outside: each face's (u, v) is right-handed about its outward normal.
  const corners = [
    vertexVec(key, 0, 0),
    vertexVec(key, PATCH_QUADS, 0),
    vertexVec(key, PATCH_QUADS, PATCH_QUADS),
    vertexVec(key, 0, PATCH_QUADS),
  ];
  let inside = true;
  let leastRad = Number.POSITIVE_INFINITY;
  for (let n = 0; n < corners.length; n += 1) {
    const a = corners[n];
    const b = corners[(n + 1) % corners.length];
    if (a === undefined || b === undefined) {
      throw new Error("a patch has four corners");
    }
    inside &&= dot(nadir, cross(a, b)) >= 0;
    leastRad = Math.min(leastRad, angleToArcRad(nadir, a, b));
  }
  return inside || leastRad <= reachRad;
}

/** A leaf of the model's refinement: its box from the camera, and whether its own surface is seen. */
interface ModelLeaf {
  readonly bounds: CameraRelativeBounds;
  readonly exactlyAbove: boolean;
}

/** The peaks over the pitches at one heading, on the box's horizon and on the exact one. */
function headingPeaks(
  leaves: ReadonlyArray<ModelLeaf>,
  frame: SiteFrame,
  headingRad: number,
  camera: CountModelCamera,
  viewport: ViewSize,
  marginRad: number,
): readonly [box: CountModelPeak, exact: CountModelPeak] {
  let box: CountModelPeak | null = null;
  let exact: CountModelPeak | null = null;
  for (let step = 0; step <= PITCH_STEPS; step += 1) {
    const pitchRad = (step * (Math.PI / 2)) / PITCH_STEPS;
    const orientation = poseOrientation(frame, headingRad, pitchRad);
    const frustum = frustumOf({ orientation, fovXRad: camera.fovXRad, viewport }, marginRad);
    let count = 0;
    let exactCount = 0;
    for (const leaf of leaves) {
      if (inFrustum(leaf.bounds, frustum)) {
        count += 1;
        exactCount += leaf.exactlyAbove ? 1 : 0;
      }
    }
    if (box === null || count > box.count) {
      box = { headingRad, pitchRad, count, orientation };
    }
    if (exact === null || exactCount > exact.count) {
      exact = { headingRad, pitchRad, count: exactCount, orientation };
    }
  }
  if (box === null || exact === null) {
    throw new Error("the pitch search visits at least one pitch");
  }
  return [box, exact];
}

/** The counts of one horizon test from its leaves' number and its headings' peaks. */
function countsOf(allRound: number, peaks: ReadonlyArray<CountModelPeak>): CountModelCounts {
  const counts = peaks.map((p) => p.count);
  return { allRound, frustum: [Math.min(...counts), Math.max(...counts)], peaks };
}

/** The site's frame: the local up, and the site face's +u and +v axes in the tangent plane. */
interface SiteFrame {
  readonly up: Vec3;
  readonly alongU: Vec3;
  readonly alongV: Vec3;
}

/** The camera looking along `headingRad` and `pitchRad` below the horizontal, at the site's frame. */
function poseOrientation(frame: SiteFrame, headingRad: number, pitchRad: number): Quaternion {
  const { up, alongU, alongV } = frame;
  const c = Math.cos(headingRad);
  const s = Math.sin(headingRad);
  const h = vec3(
    c * alongU.x + s * alongV.x,
    c * alongU.y + s * alongV.y,
    c * alongU.z + s * alongV.z,
  );
  const cp = Math.cos(pitchRad);
  const sp = Math.sin(pitchRad);
  const forward = vec3(cp * h.x - sp * up.x, cp * h.y - sp * up.y, cp * h.z - sp * up.z);
  // The camera's up, square to the forward in the plane of the heading and the local up, so that
  // the nadir has an up as well.
  const cameraUp = vec3(sp * h.x + cp * up.x, sp * h.y + cp * up.y, sp * h.z + cp * up.z);
  return lookAlong(forward, cameraUp);
}

/**
 * Design note 15's count model: how many patches a camera at `camera.heightM` over
 * {@link COUNT_MODEL_SITE} selects at tolerance `tauPx` in a view of `viewport`, all round and at
 * its frustum's peak.
 *
 * @remarks
 * The rule, on the premise planet: a patch above the horizon (`horizonCone`, `aboveHorizon`) and
 * coarser than the finest level is split while `screenSpaceErrorPx(ε_n, d, view) > tauPx`, d its
 * box's distance (`patchBounds`, `relativeBounds`, `distanceToBoxM`); a patch below the horizon is
 * dropped. That is what `selectPatches` counts without the 2:1 balance, the sagitta, the forced
 * region, the survey and the budget. Refinement depends on distance alone, so it runs once, its
 * leaves the all-round count; each pose's frustum (`frustumOf(camera, frustumMarginRad)`,
 * `inFrustum`) is then counted over those leaves, at each of {@link COUNT_MODEL_HEADINGS_RAD} and
 * every pitch from level to the nadir, 1° apart. The same leaves and poses are counted again with
 * each leaf's horizon tested exactly on its own surface ({@link PatchCountModel.exactHorizon}).
 *
 * Premise deviations from Design note 15: the vertical field of view follows from the viewport's
 * aspect as the engine's projection takes it, 2 atan(tan(fovX ÷ 2) × H ÷ W), 36.0° at 16:9 against
 * the note's 34° (which scales the angle, 60° × 9 ÷ 16, not its tangent); the distance is to the
 * patch's box, not its centre; and the pose is searched over three headings and 91 pitches.
 *
 * @param tauPx - The tolerance, px, above 0: a setting's τ, or τ_sel = τ ÷ 1.1 as the terrain pass
 *   selects (`selectionTolerancePx`).
 * @param viewport - The presented size in device pixels.
 * @throws RangeError for a height, field of view, margin, tolerance, viewport or premise out of
 *   range.
 */
export function patchCountModel(
  camera: CountModelCamera,
  tauPx: number,
  viewport: ViewSize,
  premise: CountModelPremise = {},
): PatchCountModel {
  const radiusM = checked("radius", premise.radiusM, WGS84_EQUATORIAL_RADIUS_M, Number.MIN_VALUE);
  const edgesAtOnePx = checked("bound ratio", premise.edgesAtOnePx, COUNT_MODEL_EDGES_AT_ONE_PX, 0);
  const reliefM = checked("relief", premise.reliefM, 0, 0);
  const heightM = checked("height", camera.heightM, NaN, Number.MIN_VALUE);
  const marginRad = checked("frustum margin", camera.frustumMarginRad, 0, 0);
  if (!(camera.fovXRad > 0 && camera.fovXRad < Math.PI)) {
    throw new RangeError(
      `the count model's field of view must be in (0, π), got ${camera.fovXRad}`,
    );
  }
  if (!(tauPx > 0 && viewport.widthPx > 0 && viewport.heightPx > 0)) {
    throw new RangeError("the count model's tolerance and viewport must be positive");
  }
  const planet = premisePlanet(radiusM, edgesAtOnePx, reliefM);
  const centre = PATCH_QUADS / 2;
  const [x, y, z] = surfacePoint(
    planet.figure,
    vertexDir(COUNT_MODEL_SITE, centre, centre),
    heightM,
  );
  const positionM = vec3(x, y, z);
  // The direction of the camera's nadir from the body's centre, which is also its local up.
  const nadir = normalise(positionM);
  const reachRad = horizonReachRad(radiusM, heightM, reliefM);
  // Only the viewport's width and the field of view reach the error; the orientation is the pose's.
  const view: ViewSelectionInput = {
    camera: { positionM, orientation: IDENTITY_QUATERNION },
    fovXRad: camera.fovXRad,
    viewport,
    weight: 1,
    tauPx,
  };
  const horizon = horizonCone(planet, positionM);
  const leaves: ModelLeaf[] = [];
  let exactLeaves = 0;
  const stack: PatchKey[] = FACES.map(rootKey);
  for (let key = stack.pop(); key !== undefined; key = stack.pop()) {
    // Pruned with its subtree where it is below the horizon, so boxed over the subtree's range,
    // as selection boxes it (R09.T8.d).
    const range = subtreeHeightRangeM(planet, key.level);
    const bounds = relativeBounds(patchBounds(planet, key, range), positionM);
    if (!aboveHorizon(bounds, horizon)) {
      continue;
    }
    const split =
      key.level < planet.finestLevel &&
      screenSpaceErrorPx(levelBoundM(planet, key.level), distanceToBoxM(bounds), view) > tauPx;
    if (split) {
      stack.push(...childKeys(key));
    } else {
      const exactlyAbove = patchAboveHorizon(key, nadir, reachRad);
      leaves.push({ bounds, exactlyAbove });
      exactLeaves += exactlyAbove ? 1 : 0;
    }
  }
  const up = nadir;
  const [ux, uy, uz] = faceUvToXyz(COUNT_MODEL_SITE.face, 1, 0);
  const [ox, oy, oz] = faceUvToXyz(COUNT_MODEL_SITE.face, 0, 0);
  const alongV = normalise(cross(up, sub(vec3(ux, uy, uz), vec3(ox, oy, oz))));
  const frame: SiteFrame = { up, alongU: cross(alongV, up), alongV };
  const peaks = COUNT_MODEL_HEADINGS_RAD.map((headingRad) =>
    headingPeaks(leaves, frame, headingRad, camera, viewport, marginRad),
  );
  return {
    positionM,
    ...countsOf(
      leaves.length,
      peaks.map(([box]) => box),
    ),
    exactHorizon: countsOf(
      exactLeaves,
      peaks.map(([, exact]) => exact),
    ),
  };
}

/** The sizing rule's factor on the all-round peak: 1.3 (Design note 15's sizing rule). */
export const ALL_ROUND_FACTOR = 1.3;

/** The sizing rule's factor on the frustum peak: 3 (Design note 15's sizing rule). */
export const FRUSTUM_FACTOR = 3;

/** One streaming view's peaks, patches, from the count model or a measurement. */
export interface StreamingViewPeaks {
  /** The all-round peak, patches. */
  readonly allRound: number;
  /** The frustum peak, patches: the greatest of a range. */
  readonly frustum: number;
}

/**
 * Design note 15's sizing rule, slots: max(1.3 × the all-round peak, 3 × the frustum peak), summed
 * over the views that stream, plus the forced region under grounded bodies, unrounded.
 *
 * @param forcedPatches - The forced region's patches, which the budget never refuses: a measured or
 *   replayed peak, 0 or more.
 */
export function sizingRuleSlots(
  views: ReadonlyArray<StreamingViewPeaks>,
  forcedPatches: number,
): number {
  let slots = forcedPatches;
  for (const v of views) {
    slots += Math.max(ALL_ROUND_FACTOR * v.allRound, FRUSTUM_FACTOR * v.frustum);
  }
  return slots;
}

/**
 * Design note 15's frustum rule, slots: 3 × the frustum peak, summed over the views that stream,
 * plus the forced region, unrounded: the high setting's alternative, if T14.b finds the all-round
 * peak never resident.
 */
export function frustumRuleSlots(
  views: ReadonlyArray<StreamingViewPeaks>,
  forcedPatches: number,
): number {
  let slots = forcedPatches;
  for (const v of views) {
    slots += FRUSTUM_FACTOR * v.frustum;
  }
  return slots;
}

/** The bytes of a budget of `slots` slots (rounded up to whole slots) of `bytesPerSlot` each. */
export function slotBudgetBytes(slots: number, bytesPerSlot: number): number {
  return Math.ceil(slots) * bytesPerSlot;
}

/**
 * The bytes a cache of `layout` allocates on the GPU: its slots whole, `slotCount × bytesPerSlot`,
 * what the terrain pass makes (`PatchCache.heldBytes` counts only the slots in use).
 */
export function cacheAllocationBytes(
  layout: Pick<SlotLayout, "slotCount" | "bytesPerSlot">,
): number {
  return layout.slotCount * layout.bytesPerSlot;
}

/**
 * The bytes several caches allocate together: two bodies handed over, or one body drawn in both
 * styles (decision-r10-t14a, R5).
 */
export function allocationSumBytes(
  layouts: ReadonlyArray<Pick<SlotLayout, "slotCount" | "bytesPerSlot">>,
): number {
  let bytes = 0;
  for (const layout of layouts) {
    bytes += cacheAllocationBytes(layout);
  }
  return bytes;
}

/**
 * WebGPU's default `maxStorageBufferBindingSize`, bytes: 134,217,728, 128 MiB (W3C WebGPU §3.6.2,
 * the limits table), what a device has unless it asks for more; the engine asks the adapter for its
 * own up to 1 GiB (`requiredLimits`), so this binds on adapters at the defaults and in the harness's
 * default-limits runs.
 */
export const WEBGPU_DEFAULT_STORAGE_BINDING_BYTES = 134_217_728;

/**
 * The most slots for which every storage-buffer field of `fields` fits one binding of
 * `bindingBytes`, the storage-binding ceiling: ⌊bindingBytes ÷ the largest storage field⌋;
 * `+∞` where no storage field has bytes (decision-r10-t14a, R6).
 *
 * @param bindingBytes - The binding's size, bytes; WebGPU's default,
 *   {@link WEBGPU_DEFAULT_STORAGE_BINDING_BYTES}, if absent.
 */
export function bindingCeilingSlots(
  fields: ReadonlyArray<SlotField>,
  bindingBytes: number = WEBGPU_DEFAULT_STORAGE_BINDING_BYTES,
): number {
  let slots = Number.POSITIVE_INFINITY;
  for (const field of fields) {
    if (field.storage === "storage-buffer" && field.bytes > 0) {
      slots = Math.min(slots, Math.floor(bindingBytes / field.bytes));
    }
  }
  return slots;
}
