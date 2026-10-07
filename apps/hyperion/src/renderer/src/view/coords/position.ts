import {
  type BodyIdHex,
  type GalacticPosition,
  METRES_PER_LIGHT_YEAR,
  parseBodyId,
  type SystemIdHex,
} from "@hyperion/protocol";

import { add, sub, type Vec3 } from "../../geometry/vec3";
import { IDENTITY_ROTATION, type Rotation3, rotateToBody, rotateToBodyFixed } from "./rotation";

/**
 * A position the view draws, tagged with its frame: the client's mirror of the simulation's
 * position types (`coords::GalacticPosition`, `SystemPosition`, `BodyPosition` and
 * `BodyFixedPosition`).
 *
 * @remarks
 * `m` is `f64` metres from the frame's origin: along the galactic axes for `system` and `body`,
 * along the body's rotating axes for `body_fixed`. Every position reaches the GPU only through
 * {@link relativeToCamera} and `narrow` (plan R02, Design note 1).
 */
export type ViewPosition =
  | { readonly kind: "galactic"; readonly position: GalacticPosition }
  | { readonly kind: "system"; readonly system: SystemIdHex; readonly m: Vec3 }
  | { readonly kind: "body"; readonly body: BodyIdHex; readonly m: Vec3 }
  | { readonly kind: "body_fixed"; readonly body: BodyIdHex; readonly m: Vec3 };

/** A frame a {@link ViewPosition} can be expressed in: its kind and whose origin it is. */
export type ViewFrame =
  | { readonly kind: "galactic" }
  | { readonly kind: "system"; readonly system: SystemIdHex }
  | { readonly kind: "body"; readonly body: BodyIdHex }
  | { readonly kind: "body_fixed"; readonly body: BodyIdHex };

/**
 * Where each frame's origin is at the frame time: what the view's scene supplies so that positions
 * in different frames can be differenced.
 */
export interface FrameOrigins {
  /** The system's barycentre in the galactic frame. */
  systemBarycentre(system: SystemIdHex): GalacticPosition;
  /** The body's centre in its system's frame, metres from the barycentre along the galactic axes. */
  bodyCentreM(body: BodyIdHex): Vec3;
  /**
   * The body's rotation from its body-fixed axes to its body frame, or `null` where the rotation
   * is not modelled (plan R02, Design note 14): the body-fixed axes are then taken as the body
   * frame's, and, for a body other than a star with a radius, the view says
   * `ROTATION: NOT YET MODELLED`.
   */
  bodyFixedRotation(body: BodyIdHex): Rotation3 | null;
}

/** One component of `to − from` in metres, whole cells subtracted before offsets. */
function axisDeltaM(from: GalacticPosition, to: GalacticPosition, axis: 0 | 1 | 2): number {
  const cellsM = (to.cell_ly[axis] - from.cell_ly[axis]) * METRES_PER_LIGHT_YEAR;
  const offsetsM = to.offset_m[axis] - from.offset_m[axis];
  return cellsM + offsetsM;
}

/**
 * The vector from `from` to `to` in metres, along the galactic axes.
 *
 * @remarks
 * `galacticDeltaLy` of `@hyperion/protocol` in metres: cells are subtracted before offsets, so the
 * result carries the galactic frame's 2 m resolution anywhere in the galaxy however far both
 * points are from its centre.
 */
export function galacticDeltaM(from: GalacticPosition, to: GalacticPosition): Vec3 {
  return { x: axisDeltaM(from, to, 0), y: axisDeltaM(from, to, 1), z: axisDeltaM(from, to, 2) };
}

/** The frame `p` is expressed in. */
export function frameOf(p: ViewPosition): ViewFrame {
  let frame: ViewFrame;
  switch (p.kind) {
    case "galactic":
      frame = { kind: "galactic" };
      break;
    case "system":
      frame = { kind: "system", system: p.system };
      break;
    case "body":
      frame = { kind: "body", body: p.body };
      break;
    case "body_fixed":
      frame = { kind: "body_fixed", body: p.body };
      break;
  }
  return frame;
}

/** The system whose frame, or whose body's frame, `frame` is; `null` for the galactic frame. */
export function systemOfFrame(frame: ViewFrame): SystemIdHex | null {
  let system: SystemIdHex | null;
  switch (frame.kind) {
    case "galactic":
      system = null;
      break;
    case "system":
      system = frame.system;
      break;
    case "body":
    case "body_fixed":
      system = parseBodyId(frame.body).system;
      break;
  }
  return system;
}

/** A position in a system's frame or one of its bodies': every kind but `galactic`. */
type InSystem = Exclude<ViewPosition, { kind: "galactic" }>;

/** The system whose frame, or whose body's frame, `p` is in. */
function systemOf(p: InSystem): SystemIdHex {
  return p.kind === "system" ? p.system : parseBodyId(p.body).system;
}

function rotationOf(body: BodyIdHex, origins: FrameOrigins): Rotation3 {
  return origins.bodyFixedRotation(body) ?? IDENTITY_ROTATION;
}

/** `p`'s metres from its body's centre along the galactic axes, for a body or body-fixed position. */
function bodyFrameM(
  p: Extract<ViewPosition, { kind: "body" | "body_fixed" }>,
  origins: FrameOrigins,
): Vec3 {
  return p.kind === "body" ? p.m : rotateToBody(rotationOf(p.body, origins), p.m);
}

/** `p`'s metres from its system's barycentre along the galactic axes, for a non-galactic position. */
function systemFrameM(p: InSystem, origins: FrameOrigins): Vec3 {
  return p.kind === "system" ? p.m : add(origins.bodyCentreM(p.body), bodyFrameM(p, origins));
}

/**
 * `a − b` in metres along the galactic axes, computed in the innermost frame the two share.
 *
 * @remarks
 * Two positions in one frame subtract their own metres, so that the difference of two nearby
 * points is exact whatever their distance from the frame's origin; two in one body's frames
 * (non-rotating and fixed) meet in that body's frame; two in one system meet in its frame; and
 * anything else meets through the galactic frame, barycentres differenced cells first. A
 * body-fixed difference is rotated into the galactic axes.
 */
export function differenceM(a: ViewPosition, b: ViewPosition, origins: FrameOrigins): Vec3 {
  if (a.kind === "galactic" && b.kind === "galactic") {
    return galacticDeltaM(b.position, a.position);
  }
  if (a.kind !== "galactic" && b.kind !== "galactic") {
    if (a.kind === "body_fixed" && b.kind === "body_fixed" && a.body === b.body) {
      return rotateToBody(rotationOf(a.body, origins), sub(a.m, b.m));
    }
    if (a.kind !== "system" && b.kind !== "system" && a.body === b.body) {
      return sub(bodyFrameM(a, origins), bodyFrameM(b, origins));
    }
    if (a.kind === "system" && b.kind === "system" && a.system === b.system) {
      return sub(a.m, b.m);
    }
    if (systemOf(a) === systemOf(b)) {
      return sub(systemFrameM(a, origins), systemFrameM(b, origins));
    }
  }
  return sub(galacticOffsetM(a, b, origins), galacticOffsetM(b, b, origins));
}

/**
 * `p`'s position as metres from a reference galactic point along the galactic axes: the reference
 * is `anchor`'s barycentre, or `anchor` itself when it is galactic.
 */
function galacticOffsetM(p: ViewPosition, anchor: ViewPosition, origins: FrameOrigins): Vec3 {
  const reference = anchorPosition(anchor, origins);
  if (p.kind === "galactic") {
    return galacticDeltaM(reference, p.position);
  }
  return add(
    galacticDeltaM(reference, origins.systemBarycentre(systemOf(p))),
    systemFrameM(p, origins),
  );
}

function anchorPosition(anchor: ViewPosition, origins: FrameOrigins): GalacticPosition {
  if (anchor.kind === "galactic") {
    return anchor.position;
  }
  return origins.systemBarycentre(systemOf(anchor));
}

/** The bounds of the `i32` that carries each cell coordinate on the wire. */
const CELL_MIN_LY = -(2 ** 31);
const CELL_MAX_LY = 2 ** 31 - 1;

/**
 * `position` moved by `deltaM` metres along the galactic axes, each offset kept in `[0, 1 ly)`.
 *
 * @throws RangeError if a component of `deltaM` is not finite or the result leaves the galactic
 * frame's `i32` cells, as `galacticPositionFromLy` refuses them.
 */
export function galacticTranslated(position: GalacticPosition, deltaM: Vec3): GalacticPosition {
  const cell: [number, number, number] = [0, 0, 0];
  const offset: [number, number, number] = [0, 0, 0];
  const delta = [deltaM.x, deltaM.y, deltaM.z] as const;
  for (const axis of [0, 1, 2] as const) {
    const totalM = position.offset_m[axis] + delta[axis];
    const carryLy = Math.floor(totalM / METRES_PER_LIGHT_YEAR);
    let offsetM = totalM - carryLy * METRES_PER_LIGHT_YEAR;
    let cellLy = position.cell_ly[axis] + carryLy;
    // Rounding can leave the offset at a whole light-year; carry it, as the wire form requires.
    if (offsetM >= METRES_PER_LIGHT_YEAR) {
      offsetM -= METRES_PER_LIGHT_YEAR;
      cellLy += 1;
    }
    if (offsetM < 0) {
      offsetM += METRES_PER_LIGHT_YEAR;
      cellLy -= 1;
    }
    if (!Number.isFinite(offsetM) || cellLy < CELL_MIN_LY || cellLy > CELL_MAX_LY) {
      throw new RangeError(`a translation by ${String(delta[axis])} m leaves the galactic frame`);
    }
    cell[axis] = cellLy === 0 ? 0 : cellLy;
    offset[axis] = offsetM;
  }
  return { cell_ly: cell, offset_m: offset };
}

/**
 * `p` expressed in `frame`, given where each frame's origin is.
 *
 * @remarks
 * Exact where `p` is already in `frame`; otherwise one {@link differenceM} from the frame's origin,
 * rotated into the body-fixed axes where `frame` is body-fixed.
 */
export function expressIn(p: ViewPosition, frame: ViewFrame, origins: FrameOrigins): ViewPosition {
  let expressed: ViewPosition;
  switch (frame.kind) {
    case "galactic": {
      if (p.kind === "galactic") {
        expressed = p;
        break;
      }
      expressed = {
        kind: "galactic",
        position: galacticTranslated(
          origins.systemBarycentre(systemOf(p)),
          systemFrameM(p, origins),
        ),
      };
      break;
    }
    case "system": {
      if (p.kind === "system" && p.system === frame.system) {
        expressed = p;
        break;
      }
      const origin: ViewPosition = {
        kind: "system",
        system: frame.system,
        m: { x: 0, y: 0, z: 0 },
      };
      expressed = { kind: "system", system: frame.system, m: differenceM(p, origin, origins) };
      break;
    }
    case "body": {
      if (p.kind === "body" && p.body === frame.body) {
        expressed = p;
        break;
      }
      const origin: ViewPosition = { kind: "body", body: frame.body, m: { x: 0, y: 0, z: 0 } };
      expressed = { kind: "body", body: frame.body, m: differenceM(p, origin, origins) };
      break;
    }
    case "body_fixed": {
      if (p.kind === "body_fixed" && p.body === frame.body) {
        expressed = p;
        break;
      }
      const origin: ViewPosition = { kind: "body", body: frame.body, m: { x: 0, y: 0, z: 0 } };
      const bodyM = differenceM(p, origin, origins);
      expressed = {
        kind: "body_fixed",
        body: frame.body,
        m: rotateToBodyFixed(rotationOf(frame.body, origins), bodyM),
      };
      break;
    }
  }
  return expressed;
}
