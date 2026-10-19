/**
 * The disc's hand-over to terrain (plan R10, R10.T10.e; Design note 13): the range inside which a
 * body's own relief subtends τ pixels of a view, the style's τ, and the decision with R07's
 * hysteresis, by which R07's regime plan hands a body's geometry to R05's quadtree.
 *
 * @remarks
 * - **The range** is the relief over τ pixels' angle, `handoverRangeM` = relief ÷ (τ · rad a
 *   pixel): about 1.8 × 10⁷ m for 10 km of relief at 1 px of 5.454 × 10⁻⁴ rad (Design note 13's
 *   1080p across 60° by width ÷ field), and 4.6 × 10⁶ m at the wireframe's 4 px. A view measures
 *   its pixel as R07's 3 px switch does, at the centre pixel ({@link pixelAngleRad}, the inverse
 *   of `angularDiameterPx`'s scale), so that both switches count the same pixels: 6.01 × 10⁻⁴ rad
 *   at 1080p across 60°, where 10 km hands over at 1.66 × 10⁷ m.
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
 */
import type { BodyIdHex, SurfaceHeaderDto } from "@hyperion/protocol";

import { norm } from "../../geometry/vec3";
import { type LitRegime, type LitSphere, REGIME_HYSTERESIS } from "../bodies/regime";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import type { RenderStyle } from "../camera/state";
import { type QualitySetting, SETTINGS, WIREFRAME_TERRAIN_TAU_PX } from "../quality/qualitySetting";
import type { CoverageView } from "./coverage";

/**
 * The angle of a view's centre pixel, rad: 2 tan(fov ÷ 2) ÷ width, R02's centre-pixel scale, by
 * which R07's regimes measure a body's diameter (`angularDiameterPx`).
 */
export function pixelAngleRad(camera: ProjectionCamera, viewport: Viewport): number {
  return (2 * Math.tan(camera.fovXRad / 2)) / viewport.widthPx;
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
