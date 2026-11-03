/**
 * The disc's hand-over to terrain (plan R10, R10.T10.e; Design note 13): the range inside which a
 * body's own relief subtends τ pixels of a view, the style's τ, and the decision with R07's
 * hysteresis, by which R07's regime plan hands a body's geometry to R05's quadtree.
 *
 * @remarks
 * - **The range** is the relief over τ pixels' angle, `handoverRangeM` = relief ÷ (τ · rad a
 *   pixel): about 1.8 × 10⁷ m for 10 km of relief at 1 px of 5.454 × 10⁻⁴ rad (Design note 13's
 *   1080p across 60° by width ÷ field), and 4.6 × 10⁶ m at the wireframe's 4 px. A view measures
 *   its pixel at the centre pixel, as R07's 3 px switch does ({@link pixelAngleRad}, the inverse
 *   of `angularDiameterPx`'s scale): 6.01 × 10⁻⁴ rad at 1080p across 60°, where 10 km hands over
 *   at 1.66 × 10⁷ m. It is the centre pixel of the size the view is presented at, its canvas's,
 *   as the field's want measures it (R10.T10.g; decision-r10-t16-leans §13), not of the frame's
 *   own viewport, so that the resolution controller's internal scale (0.5 to 1, and the low
 *   setting's 720 rows) never moves the range. The two switches so count different pixels: R07's
 *   3 px switch keeps the frame's own, in which the disc's summed flux meets the point's.
 * - **The relief** is R09's realised relief, the field header's `realised_relief_m` (the highest
 *   elevation less the lowest), known once the body's field is held; a body without a header, or
 *   whose survey covers no ground, never hands over ({@link handoverReliefM}).
 * - **τ** is the style's: `TerrainSettings.tauPx` in the photorealistic style (1 px high, 2 px
 *   low) and `WIREFRAME_TERRAIN_TAU_PX` (4 px) in the wireframe ({@link handoverTauPx}).
 * - **The decision** ({@link handsOver}) takes R07's `REGIME_HYSTERESIS` on the relief's subtense,
 *   as R07's 3 px switch takes it on the diameter: a body hands over once its relief subtends
 *   τ (1 + h) px and hands back once under τ (1 − h), so that one at the range does not flip.
 * - **The distance** is the camera's to the body's limb, √(D² − a²) for a camera D from the centre
 *   of a body of equatorial radius a ({@link limbDistanceM}), where the relief stands against the
 *   sky across the line of sight: the centre's distance far off (7% less at an Earth's
 *   1.66 × 10⁷ m), and 0 at the ground, so that a body whose relief is small beside its radius
 *   still hands over as the camera nears it, which the centre's distance would not let a Venus of
 *   13 km do in the wireframe (a · 4 px is 14.6 km at 1080p).
 * - **At the field's cells** ({@link HANDOVER_AT_CELLS}; lane fix-coast-class-map): in the
 *   photorealistic style a body in range hands over only once its terrain draws the field's own
 *   cells, its pass warmed while the disc is drawn, so that the terrain's first frame draws them
 *   as its settled one does ({@link drawsCells}). Its selection refines seen surveyed ground to the
 *   level whose quads are the cells ({@link cellPatchLevel}). Without it the pass was made at the first
 *   hand-over and refined from the roots, a level a bake's round trip, so that on the served wet
 *   world the terrain's first frame differed from its settled one by over a third of a stop in 431
 *   of 750 coast cells, and the disc → terrain switch failed there whatever the disc drew.
 */
import type { BodyIdHex, SurfaceHeaderDto } from "@hyperion/protocol";

import { norm } from "../../geometry/vec3";
import { type LitRegime, type LitSphere, REGIME_HYSTERESIS } from "../bodies/regime";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import type { RenderStyle } from "../camera/state";
import { type QualitySetting, SETTINGS, WIREFRAME_TERRAIN_TAU_PX } from "../quality/qualitySetting";
import type { DrawSet } from "./cache";
import type { CoverageView } from "./coverage";

/**
 * The angle of a view's centre pixel, rad: 2 tan(fov ÷ 2) ÷ width, R02's centre-pixel scale, by
 * which R07's regimes measure a body's diameter (`angularDiameterPx`).
 *
 * @remarks
 * The hand-over takes it of the selection's size ({@link selectionViewport}, the presented
 * viewport's CSS px; R10.T10.g: `BodyFrameOptions.selectionPx`), R07's 3 px switch of the frame's
 * own.
 */
export function pixelAngleRad(camera: ProjectionCamera, viewport: Viewport): number {
  return (2 * Math.tan(camera.fovXRad / 2)) / viewport.widthPx;
}

/**
 * The size a view's terrain selects, hands over and is judged `DETAIL LIMITED` at (R05 Design
 * notes 7 and 23; the owner's Retina report, lean 2026-11-01): the presented size, device px,
 * ÷ max(1, `devicePixelRatio`), its CSS px, so that τ is a CSS pixel's visual angle whatever the
 * display's density.
 *
 * @remarks
 * CSS's reference pixel is a visual angle of about 0.0213°, 1.28′ (CSS Values and Units Level 4,
 * §6.2, "reference pixel"), near the eye's 1′ limit; a Retina display's device pixel, half that,
 * lies below it, so selecting at device px drew about 4× the patches at a ratio of 2 for an error
 * no one sees. The canvas, the render and every pass stay at device px. At a ratio of 1 or under,
 * or one not finite, it is `presentedPx` itself, so that nothing moves on such a display.
 */
export function selectionViewport(presentedPx: Viewport, devicePixelRatio: number): Viewport {
  if (!(Number.isFinite(devicePixelRatio) && devicePixelRatio > 1)) {
    return presentedPx;
  }
  return {
    widthPx: presentedPx.widthPx / devicePixelRatio,
    heightPx: presentedPx.heightPx / devicePixelRatio,
  };
}

/**
 * The distance inside which a body's relief subtends τ pixels, m: relief ÷ (τ · rad a pixel),
 * Design note 13's hand-over range, against the camera's distance to the body's limb.
 *
 * @param reliefM - The body's realised relief, m, finite and non-negative; 0 gives a range of 0.
 * @param tauPx - The style's τ, px, positive.
 * @param radPerPx - A pixel's angle, rad, positive ({@link pixelAngleRad} for a view).
 * @throws RangeError for a relief that is negative or not finite, or a τ or pixel angle that is
 *   not finite and positive.
 */
export function handoverRangeM(reliefM: number, tauPx: number, radPerPx: number): number {
  if (!(Number.isFinite(reliefM) && reliefM >= 0)) {
    throw new RangeError(`a relief of ${String(reliefM)} m has no hand-over range`);
  }
  if (!(Number.isFinite(tauPx) && tauPx > 0) || !(Number.isFinite(radPerPx) && radPerPx > 0)) {
    throw new RangeError(
      `τ of ${String(tauPx)} px over ${String(radPerPx)} rad a pixel has no hand-over range`,
    );
  }
  return reliefM / (tauPx * radPerPx);
}

/**
 * A body's relief as its hand-over takes it, m: its field header's `realised_relief_m`, or `null`
 * where it never hands over, with no header held, no coverage, or no surveyed ground (Design
 * note 13: "a body with no survey never hands over").
 */
export function handoverReliefM(
  header: Pick<SurfaceHeaderDto, "realised_relief_m"> | null,
  coverage: CoverageView | null,
): number | null {
  if (header === null || coverage === null || !(coverage.surveyedFraction() > 0)) {
    return null;
  }
  const reliefM = header.realised_relief_m;
  return Number.isFinite(reliefM) && reliefM > 0 ? reliefM : null;
}

/**
 * The camera's distance to a body's limb, m: √(D² − a²), the tangent's length from the camera to
 * the body's equatorial bounding sphere; 0 from inside it.
 *
 * @param distanceM - The camera's distance from the body's centre, D.
 * @param radiusM - The body's equatorial radius, a.
 */
export function limbDistanceM(distanceM: number, radiusM: number): number {
  return distanceM > radiusM ? Math.sqrt((distanceM - radiusM) * (distanceM + radiusM)) : 0;
}

/**
 * Whether a body hands over disc → terrain in the photorealistic style only once its terrain draws
 * the field's cells (the module's remarks): the hand-over's one rule of lane fix-coast-class-map.
 * `false` restores the hand-over as R10.T10.e built it, its pass made at the first hand-over and no
 * floor on its selection, bit for bit.
 */
export const HANDOVER_AT_CELLS = true;

/**
 * The level of a body's terrain patches whose quads are its coarse field's cells: max(L − 6, 0) for
 * a field of level L, 5 to 8, a patch spanning 2⁶ quads a side (the surface crate's
 * `PATCH_SPAN_LEVELS`, and the disc's normal map's band, `discNormals.ts`).
 */
export function cellPatchLevel(fieldLevel: number): number {
  return Math.max(fieldLevel - PATCH_SPAN_LEVELS, 0);
}

/** The quadtree levels a patch spans: its 64 quads a side are cells 6 levels below its own. */
const PATCH_SPAN_LEVELS = 6;

/**
 * Whether a warmed pass's latest draw set draws the field's cells, on which a body hands over under
 * {@link HANDOVER_AT_CELLS}: no selected patch is missing, and none is stood in for by an ancestor
 * coarser than `cellLevel` ({@link cellPatchLevel}), so that every cell it draws is drawn at its own
 * level or finer. A finer patch still streaming may stand in below it, as the terrain's refinement
 * goes on after the hand-over. A set of none, its seen ground all unsurveyed, draws the reference
 * surface before the hand-over and after it alike. `null`, before any selection, draws none.
 *
 * @param viewMask - The views judged, bit k for the selection's view k (`DrawSet.views`): the
 *   patches they see and the selected patches missing for them alone; every view's where absent.
 */
export function drawsCells(
  drawSet: Pick<DrawSet, "count" | "missing" | "patches" | "views" | "missingViews"> | null,
  cellLevel: number,
  viewMask?: number,
): boolean {
  if (drawSet === null) {
    return false;
  }
  if (viewMask === undefined ? drawSet.missing > 0 : (drawSet.missingViews & viewMask) !== 0) {
    return false;
  }
  for (let n = 0; n < drawSet.count; n += 1) {
    const drawn = drawSet.patches[n];
    const seen = viewMask === undefined || ((drawSet.views[n] ?? 0) & viewMask) !== 0;
    if (seen && drawn !== undefined && drawn.standIn && drawn.patch.key.level < cellLevel) {
      return false;
    }
  }
  return true;
}

/**
 * The range ratio, the camera's limb distance over the body's hand-over range, from which a lit
 * terrain draws its class weights as its patches' ancestors of the field's cells' level do
 * ({@link classCellFade}): three quarters of the ratio at which the body hands over, 1 ÷ (1 + h),
 * about 0.68, so that both switches, that one and the hand-back's 1 ÷ (1 − h), fall within it however
 * long the terrain takes to draw the cells (lane fix-coast-class-map, a lean).
 */
export const CLASS_FADE_FAR = 0.75 / (1 + REGIME_HYSTERESIS);

/** The range ratio within which a lit terrain draws its patches' own class weights alone: half {@link CLASS_FADE_FAR}. */
export const CLASS_FADE_NEAR = CLASS_FADE_FAR / 2;

/**
 * The fade of a lit terrain's class weights to those of its patches' ancestors of the field's
 * cells' level, L − 6 (lane fix-coast-class-map; `TerrainFrameInput.classSets`), at the range ratio
 * `rangeRatio` ({@link handoverRangeRatio}): 1 from {@link CLASS_FADE_FAR} out, so that on either
 * side of the switch the terrain draws the field's cells as the disc's class map does from their
 * corners, falling linearly to 0 at {@link CLASS_FADE_NEAR} as the camera nears and the terrain's
 * finer classes resolve. 1 at an infinite ratio, 0 at a ratio that is not a number.
 */
export function classCellFade(rangeRatio: number): number {
  if (!(rangeRatio > CLASS_FADE_NEAR)) {
    return 0;
  }
  return Math.min((rangeRatio - CLASS_FADE_NEAR) / (CLASS_FADE_FAR - CLASS_FADE_NEAR), 1);
}

/** The hand-over's τ in a style on a setting, px (Design note 13). */
export function handoverTauPx(style: RenderStyle, setting: QualitySetting): number {
  let tauPx: number;
  switch (style) {
    case "photorealistic":
      tauPx = SETTINGS[setting].terrain.tauPx;
      break;
    case "wireframe":
      tauPx = WIREFRAME_TERRAIN_TAU_PX;
      break;
  }
  return tauPx;
}

/**
 * Whether a body is handed over to terrain: its relief subtends τ (1 + h) px or more from
 * `limbM`, or τ (1 − h) px or more where it was handed over already, h `REGIME_HYSTERESIS`.
 *
 * @param limbM - The camera's distance to the body's limb ({@link limbDistanceM}), m.
 * @param reliefM - Its relief ({@link handoverReliefM}), or `null`, which never hands over.
 * @param handedOver - Whether it was handed over on the previous frame.
 */
export function handsOver(
  limbM: number,
  reliefM: number | null,
  tauPx: number,
  radPerPx: number,
  handedOver: boolean,
): boolean {
  if (reliefM === null) {
    return false;
  }
  const rangeM = handoverRangeM(reliefM, tauPx, radPerPx);
  const factor = handedOver ? 1 - REGIME_HYSTERESIS : 1 + REGIME_HYSTERESIS;
  // The relief's subtense against τ (1 ± h): in range, d ≤ range ÷ (1 ± h).
  return rangeM > 0 && limbM * factor <= rangeM;
}

/**
 * A view's `FieldCandidate.rangeRatio` for a body (`coarseFieldStore.ts`, Design note 16): the
 * camera's distance to its limb over its hand-over range in the view's style, as the hand-over
 * measures it, or `Infinity` with no relief known (R10.T7's question: until a relief bound reaches
 * the wire, a body not held is wanted only as a local body).
 *
 * @param limbM - The camera's distance to the body's limb ({@link limbDistanceM}), m.
 * @param reliefM - The field header's `realised_relief_m`, or `null` before a header is held.
 */
export function handoverRangeRatio(
  limbM: number,
  reliefM: number | null,
  tauPx: number,
  radPerPx: number,
): number {
  if (reliefM === null || !(reliefM > 0)) {
    return Number.POSITIVE_INFINITY;
  }
  return limbM / handoverRangeM(reliefM, tauPx, radPerPx);
}

/** A lit body as the hand-over sees it: its sphere and its relief. */
export interface HandoverSphere extends LitSphere {
  /** Its relief ({@link handoverReliefM}), or `undefined` where it never hands over. */
  readonly reliefM: number | undefined;
}

/**
 * R07's regimes with each disc whose relief subtends the style's τ handed over: `terrain`, the
 * mesh regime whose ground R05's quadtree draws (Design note 13), before Design note 2's
 * promotion.
 *
 * @remarks
 * A point never hands over: its flux is the point regime's (R10.T10.f) however near. A body that
 * was handed over keeps it through {@link handsOver}'s hysteresis; one past it hands back to the
 * disc.
 *
 * @param previous - Each body's regime on the previous frame.
 */
export function handOverRegimes(
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  bodies: ReadonlyArray<HandoverSphere>,
  tauPx: number,
  radPerPx: number,
  previous: ReadonlyMap<BodyIdHex, LitRegime>,
): Map<BodyIdHex, LitRegime> {
  const handed = new Map(regimes);
  for (const body of bodies) {
    if (handed.get(body.id) !== "disc") {
      continue;
    }
    const wasHanded = previous.get(body.id) === "terrain";
    const limbM = limbDistanceM(norm(body.centreM), body.radiusM);
    if (handsOver(limbM, body.reliefM ?? null, tauPx, radPerPx, wasHanded)) {
      handed.set(body.id, "terrain");
    }
  }
  return handed;
}

/**
 * Where a view takes its bodies' reliefs from for the hand-over: the coarse-field store's header
 * and coverage of each body (`CoarseFieldStore`), through {@link handoverReliefM}.
 */
export interface HandoverReliefSource {
  /** `body`'s relief, m, or `null` where it never hands over. */
  reliefM(body: BodyIdHex): number | null;
}

/** A store's bodies' reliefs, by {@link handoverReliefM} from its header and coverage. */
export function storeReliefSource(store: {
  header(body: BodyIdHex): Pick<SurfaceHeaderDto, "realised_relief_m"> | null;
  coverage(body: BodyIdHex): CoverageView | null;
}): HandoverReliefSource {
  return { reliefM: (body) => handoverReliefM(store.header(body), store.coverage(body)) };
}
