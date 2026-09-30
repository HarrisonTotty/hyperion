import type { BodyIdHex } from "@hyperion/protocol";

import { norm, scale, type Vec3 } from "../../geometry/vec3";

/**
 * The ratio of distance to Hill radius at or below which the camera enters a body's frame: 0.9.
 *
 * @remarks
 * The simulation's `planetary::body_frame::BODY_FRAME_ENTRY` (plan R02, Design note 6). The camera
 * leaves a frame above 1, so the band on each sphere's boundary keeps a camera from flickering
 * between frames: 5.8 × 10⁶ m for the Moon, 1.5 × 10⁸ m for the Earth.
 */
export const BODY_FRAME_ENTRY = 0.9;

/**
 * How much smaller a sibling's ratio must be before it takes the camera from its current frame:
 * a tenth, the galaxy's `FRAME_HYSTERESIS` (plan 03; brainstorm, "Coordinates").
 */
export const FRAME_HYSTERESIS = 0.1;

/**
 * A body near the camera as the frame rule sees it: the TypeScript mirror of the simulation's
 * `BodyFrameCandidate`.
 *
 * @remarks
 * Only planets, dwarf planets and moons are candidates. The distance is geometric and present,
 * from system-frame positions at the frame time, never an apparent one. Build one with
 * {@link cameraFrameCandidate}, which refuses what the simulation refuses.
 */
export interface CameraFrameCandidate {
  /** The body. */
  readonly id: BodyIdHex;
  /** The body it orbits (a planet's star, a moon's planet), or `null`. */
  readonly parent: BodyIdHex | null;
  /** The camera's distance from the body's centre, m, finite and not negative. */
  readonly distanceM: number;
  /** The body's Hill radius at pericentre, m, finite and positive. */
  readonly hillRadiusM: number;
}

/**
 * Builds a {@link CameraFrameCandidate}, as the simulation's `BodyFrameCandidate::new` does.
 *
 * @remarks
 * A distance of −0 is stored as +0, so that the ratio orders by its value.
 *
 * @throws RangeError if the distance is not finite and non-negative, if the Hill radius is not
 * finite and positive, or if the body names itself as its parent.
 */
export function cameraFrameCandidate(
  id: BodyIdHex,
  parent: BodyIdHex | null,
  distanceM: number,
  hillRadiusM: number,
): CameraFrameCandidate {
  if (!(Number.isFinite(distanceM) && distanceM >= 0)) {
    throw new RangeError("a body-frame candidate's distance must be finite and non-negative");
  }
  if (!(Number.isFinite(hillRadiusM) && hillRadiusM > 0)) {
    throw new RangeError("a body-frame candidate's Hill radius must be finite and positive");
  }
  if (parent === id) {
    throw new RangeError("a body-frame candidate cannot be its own parent");
  }
  return { id, parent, distanceM: distanceM + 0, hillRadiusM };
}

function ratio(c: CameraFrameCandidate): number {
  return c.distanceM / c.hillRadiusM;
}

/** Orders two IDs as the simulation's `BodyId` does: their fixed-width wire forms sort the same. */
function compareIds(a: BodyIdHex | null, b: BodyIdHex | null): number {
  if (a === b) {
    return 0;
  }
  if (a === null) {
    return -1;
  }
  if (b === null) {
    return 1;
  }
  return a < b ? -1 : 1;
}

/** By ratio, then by ID: the simulation's `rank`. */
function rank(a: CameraFrameCandidate, b: CameraFrameCandidate): number {
  const ra = ratio(a);
  const rb = ratio(b);
  if (ra !== rb) {
    return ra < rb ? -1 : 1;
  }
  return compareIds(a.id, b.id);
}

/**
 * The body frame the camera is in, given the bodies near it and the frame it was in: a body's ID,
 * or `null` for the system frame. The TypeScript twin of the simulation's `select_body_frame`
 * (plan R02, Design note 6), held to it line by line by `frame/body_frames.golden`.
 *
 * @remarks
 * While the camera is inside `current`'s sphere (`current` is among the candidates with a ratio of
 * at most 1), `current` and its ancestors among the candidates form the chain. A candidate is
 * eligible when its ratio of distance to Hill radius is at most {@link BODY_FRAME_ENTRY}, or at
 * most 1 when it is in the chain. Of the eligible, the deepest in the parent chain wins (a moon over
 * its planet); among eligible bodies at that depth the smallest ratio wins, the lower ID on an exact
 * tie, except that the one in the chain keeps the camera until a rival's ratio is at most
 * (1 − {@link FRAME_HYSTERESIS}) times its own. Once the camera has left `current`'s sphere, or
 * `current` is not a candidate, the rule runs as if there were no current frame.
 *
 * Depth counts `parent` links among the candidates only, so callers pass whole parent chains. A
 * repeated ID keeps its smallest-ratio entry, and the answer does not depend on the candidates'
 * order. R03's `sceneAt` names the ship's local body with it (R03.T13), and the free camera
 * re-selects its own frame with it each step (R02.T9.b).
 */
export function selectCameraFrame(
  candidates: ReadonlyArray<CameraFrameCandidate>,
  current: BodyIdHex | null,
): BodyIdHex | null {
  // One entry per ID, its smallest-ratio candidate: nothing below depends on the input's order.
  const byId = new Map<BodyIdHex, CameraFrameCandidate>();
  for (const candidate of candidates) {
    const kept = byId.get(candidate.id);
    if (
      kept === undefined ||
      rank(candidate, kept) < 0 ||
      (rank(candidate, kept) === 0 && compareIds(candidate.parent, kept.parent) < 0)
    ) {
      byId.set(candidate.id, candidate);
    }
  }
  // The chain from `id` up through its candidate ancestors, cut after as many steps as there are
  // candidates so that a malformed cycle of parents cannot loop.
  const ancestors = (id: BodyIdHex): BodyIdHex[] => {
    const chain: BodyIdHex[] = [];
    let next = byId.get(id);
    while (next !== undefined && chain.length < byId.size) {
      chain.push(next.id);
      next = next.parent === null ? undefined : byId.get(next.parent);
    }
    return chain;
  };
  const depths = new Map<BodyIdHex, number>();
  for (const id of byId.keys()) {
    depths.set(id, ancestors(id).length);
  }
  const depth = (id: BodyIdHex): number => depths.get(id) ?? 0;
  const held = current === null ? undefined : byId.get(current);
  const chain = held !== undefined && ratio(held) <= 1 ? ancestors(held.id) : [];
  // In ID order, as the simulation's `BTreeMap` iterates, so that even a malformed cycle of
  // parents gives the simulation's answer.
  const eligible = [...byId.values()]
    .toSorted((a, b) => compareIds(a.id, b.id))
    .filter((c) => ratio(c) <= BODY_FRAME_ENTRY || (chain.includes(c.id) && ratio(c) <= 1));
  if (eligible.length === 0) {
    return null;
  }
  const deepest = Math.max(...eligible.map((c) => depth(c.id)));
  const atDepth = eligible.filter((c) => depth(c.id) === deepest);
  let best: CameraFrameCandidate | undefined;
  for (const c of atDepth) {
    if (best === undefined || rank(c, best) < 0) {
      best = c;
    }
  }
  if (best === undefined) {
    return null;
  }
  const incumbent = atDepth.find((c) => chain.includes(c.id));
  if (incumbent === undefined) {
    return best.id;
  }
  return ratio(best) <= (1 - FRAME_HYSTERESIS) * ratio(incumbent) ? best.id : incumbent.id;
}

/**
 * The camera's position in its system's frame, held inside the system's sphere of influence.
 *
 * @remarks
 * A free camera's reach is the scene's system (plan R02, Design note 7): it cannot cross into the
 * galactic frame except where the scene itself is galactic. A position farther than
 * `tidalRadiusM` from the barycentre is pulled back along its own direction to that radius; any
 * other is returned unchanged.
 *
 * @param positionM - The camera's position, m from the system's barycentre along the galactic axes.
 * @param tidalRadiusM - The system's tidal radius, m: a kept scene's, or the arrival's
 * `tidal_radius_m` (R03.T7.a).
 * @throws RangeError if `tidalRadiusM` is not finite and positive.
 */
export function clampToTidalRadius(positionM: Vec3, tidalRadiusM: number): Vec3 {
  if (!(Number.isFinite(tidalRadiusM) && tidalRadiusM > 0)) {
    throw new RangeError(`a tidal radius must be finite and positive, got ${String(tidalRadiusM)}`);
  }
  const distanceM = norm(positionM);
  if (!(distanceM > tidalRadiusM)) {
    return positionM;
  }
  return scale(positionM, tidalRadiusM / distanceM);
}
