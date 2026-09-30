import type { BodyIdHex } from "@hyperion/protocol";

/** The depth buffer's format: 32-bit float, for reversed-Z (plan R02, Design note 4). */
export const DEPTH_FORMAT = "depth32float";

/** The depth the buffer is cleared to: 0, infinitely far under reversed-Z. */
export const DEPTH_CLEAR = 0;

/** The depth test: nearer is larger under reversed-Z (R01's `DepthPolicy` `"reversed-z-float"`). */
export const DEPTH_COMPARE = "greater-equal";

/**
 * The separation along the view, as a fraction of the distance, above which two surfaces order
 * reliably: 10⁻⁶ (plan R02, Design note 5; the brainstorm's "Depth").
 *
 * @remarks
 * A reversed `depth32float` buffer resolves about 1.2 × 10⁻⁷ of the distance at every distance
 * (Reed 2015, "Depth Precision Visualized"), and the depth itself errs by one `f32` division, 6 ×
 * 10⁻⁸ relative, so the bound is conservative.
 */
export const ORDERING_RELATIVE_SEPARATION = 1e-6;

/**
 * Whether two surfaces at `aM` and `bM` along the view, near `distanceM` from the camera, order
 * reliably in the depth buffer.
 *
 * @param aM - One surface's distance along the view, m.
 * @param bM - The other's, m.
 * @param distanceM - The distance at which they are compared, m, positive.
 */
export function separable(aM: number, bM: number, distanceM: number): boolean {
  return Math.abs(aM - bM) >= ORDERING_RELATIVE_SEPARATION * distanceM;
}

/**
 * The margin by which a body's depth-only occluder sits inside its own graticule, as a fraction of
 * the distance: four times {@link ORDERING_RELATIVE_SEPARATION} (plan R02, Design note 5).
 */
export const OCCLUDER_MARGIN = 4 * ORDERING_RELATIVE_SEPARATION;

/**
 * The radius of a body's depth-only occluder sphere, m: r − 4 × 10⁻⁶ × d, never below r ÷ 2
 * (plan R02, Design note 5).
 *
 * @remarks
 * The front hemisphere of the body's own graticule then lies in front of the occluder by four
 * times the ordering bound, while other bodies, orbits and hulls behind it are hidden.
 *
 * @param radiusM - The body's radius, m.
 * @param distanceM - The camera's distance from the body's centre, m.
 */
export function occluderRadius(radiusM: number, distanceM: number): number {
  return Math.max(radiusM - OCCLUDER_MARGIN * distanceM, radiusM / 2);
}

/**
 * A layer the view draws, as the ordering of transparent layers sees it.
 *
 * @remarks
 * `opaque` layers write depth and are drawn first; a `shell` is a transparent sphere about a body
 * (an atmosphere, a cloud deck) of radius `radiusM`; a `plane` is a body's transparent plane (its
 * rings).
 */
export type DepthLayer =
  | { readonly kind: "opaque"; readonly id: string }
  | {
      readonly kind: "shell";
      readonly id: string;
      readonly body: BodyIdHex;
      readonly radiusM: number;
    }
  | { readonly kind: "plane"; readonly id: string; readonly body: BodyIdHex };

/** What ordering transparent layers needs of the camera: its distance from each body's centre. */
export interface LayerCamera {
  /** The camera's distance from `body`'s centre, m. */
  bodyDistanceM(body: BodyIdHex): number;
}

/** A layer's place within its body: 0 below the camera, 1 the plane, 2 above. */
function withinBody(layer: Exclude<DepthLayer, { kind: "opaque" }>, distanceM: number): number {
  if (layer.kind === "plane") {
    return 1;
  }
  return layer.radiusM < distanceM ? 0 : 2;
}

function compareLayers(a: DepthLayer, b: DepthLayer, camera: LayerCamera): number {
  if (a.kind === "opaque" || b.kind === "opaque") {
    if (a.kind === "opaque" && b.kind === "opaque") {
      return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
    }
    return a.kind === "opaque" ? -1 : 1;
  }
  const da = camera.bodyDistanceM(a.body);
  const db = camera.bodyDistanceM(b.body);
  if (a.body !== b.body) {
    // Back to front by the distance of the body's centre; the lower ID on a tie.
    if (da !== db) {
      return da > db ? -1 : 1;
    }
    return a.body < b.body ? -1 : 1;
  }
  const pa = withinBody(a, da);
  const pb = withinBody(b, db);
  if (pa !== pb) {
    return pa - pb;
  }
  if (a.kind === "shell" && b.kind === "shell" && a.radiusM !== b.radiusM) {
    // Below the camera by ascending altitude; above it by descending altitude.
    return pa === 0 ? a.radiusM - b.radiusM : b.radiusM - a.radiusM;
  }
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/**
 * The order in which to draw a view's layers (plan R02, R02.T7.b; the brainstorm's "Depth").
 *
 * @remarks
 * Opaque layers first; then, per body, back to front by the distance of the body's centre; within
 * a body, the shells below the camera by ascending altitude, then the planes (rings), then the
 * shells above the camera by descending altitude. Every transparent layer tests depth and writes
 * none. The order is a function of the layers and the camera alone, whatever order they come in.
 * R08 and R11 consume it.
 */
export function transparentLayerOrder<L extends DepthLayer>(
  layers: ReadonlyArray<L>,
  camera: LayerCamera,
): L[] {
  return layers.toSorted((a, b) => compareLayers(a, b, camera));
}
