/**
 * Where a system's stars are, from plan 11's hierarchy: each star placed about its pair's
 * barycentre by the members' masses, and the paths they trace (plan 14, design note D18 and
 * P14.T42.a).
 *
 * @remarks
 * Pure functions of the model. Positions are in metres in the galactic axes about the system's
 * barycentre, as `lib/orbit.ts` gives them.
 */
import type { BodyPlacement, KeplerOrbit } from "../orbit";
import { type Vec3, vec3 } from "../../spatial/vec3";
import type { HierarchyNode, HostBody } from "./model";

/** The placement key of a hierarchy node: a star's body ID, or a pair's node index. */
function pairKey(index: number): string {
  return `pair:${index}`;
}

/** One member of one pair: where it sits about the pair's barycentre, and on which orbit. */
export interface PairMember {
  /** The member's placement key: a star's body ID, or an inner pair's `pair:<node>`. */
  readonly key: string;
  /** The body ID of the member when it is a star, which a selection names; `null` for a pair. */
  readonly hostId: string | null;
  /** The placement key of the pair it is a member of. */
  readonly pairKey: string;
  /** The pair's relative orbit: the outer member's barycentre about the inner member's. */
  readonly orbit: KeplerOrbit;
  /** Its share of the relative orbit: −M₂ ÷ M for the inner member, +M₁ ÷ M for the outer. */
  readonly share: number;
  /** Whether it is the pair's outer member, the one the pair's orbit brought in. */
  readonly outer: boolean;
}

/** What a hierarchy says about where every star is. */
export interface HierarchyLayout {
  /** Every star's and every pair's placement, for `composePosition`. */
  readonly placements: ReadonlyMap<string, BodyPlacement>;
  /** Every member of every pair, in the node order of the hierarchy. */
  readonly members: ReadonlyArray<PairMember>;
  /**
   * The largest distance from the system's barycentre that each star's chain of orbits allows, m:
   * the sum of its share of each orbit's apoapsis distance; 0 for a single star.
   */
  readonly reachM: ReadonlyMap<string, number>;
  /**
   * The unit normal of the orbit of the innermost pair that holds the primary, along its angular
   * momentum; `null` for a single star.
   */
  readonly primaryPairNormal: Vec3 | null;
  /**
   * The relative orbit each star keys, by the star's body ID: the orbit of the pair whose outer
   * member begins with it, the orbit that brought it in (plan 11, design note 5). The primary keys
   * none.
   */
  readonly keyedOrbits: ReadonlyMap<string, KeplerOrbit>;
}

/**
 * The unit normal of an orbit's plane, along its angular momentum, in the axes the orbit is given
 * in: (sin i sin Ω, −sin i cos Ω, cos i).
 */
export function orbitNormal(orbit: KeplerOrbit): Vec3 {
  const sinI = Math.sin(orbit.inclinationRad);
  return vec3(
    sinI * Math.sin(orbit.ascendingNodeRad),
    -sinI * Math.cos(orbit.ascendingNodeRad),
    Math.cos(orbit.inclinationRad),
  );
}

/**
 * Lays out a system's stars from its hierarchy.
 *
 * @remarks
 * The root, node 0, is at the barycentre. A pair's inner member sits at −(M₂ ÷ M) and its outer
 * member at +(M₁ ÷ M) of the pair's relative orbit about the pair's barycentre, where M₁ and M₂ are
 * the members' masses, each the sum of its stars' (`HierarchyDto`'s own description). The hierarchy
 * is the one `toSystemModel` accepted: members after their pair, every node once.
 *
 * @throws Error naming the node when a pair's member or a star's host is missing, which a model
 *   that `toSystemModel` accepted never has.
 */
export function layoutHierarchy(
  nodes: ReadonlyArray<HierarchyNode>,
  hosts: ReadonlyArray<HostBody>,
): HierarchyLayout {
  const hostIdOf = new Map(hosts.map((host) => [host.bodyIndex, host.id]));
  const keyOf = (index: number): string => {
    const node = nodes[index];
    if (node === undefined) {
      throw new Error(`the hierarchy has no node ${index}`);
    }
    if (node.kind === "pair") {
      return pairKey(index);
    }
    const id = hostIdOf.get(node.bodyIndex);
    if (id === undefined) {
      throw new Error(`hierarchy node ${index} names no star ${node.bodyIndex}`);
    }
    return id;
  };
  const massMemo = new Map<number, number>();
  const massOf = (index: number): number => {
    const known = massMemo.get(index);
    if (known !== undefined) {
      return known;
    }
    const node = nodes[index];
    if (node === undefined) {
      throw new Error(`the hierarchy has no node ${index}`);
    }
    const mass = node.kind === "star" ? node.massMsun : massOf(node.inner) + massOf(node.outer);
    massMemo.set(index, mass);
    return mass;
  };

  // A member's first star: its inner members down to a star, since the inner holds the lower
  // indices.
  const firstStarOf = (index: number): string => {
    let at = index;
    for (let node = nodes[at]; node?.kind === "pair"; node = nodes[at]) {
      at = node.inner;
    }
    return keyOf(at);
  };

  const placements = new Map<string, BodyPlacement>();
  const reachM = new Map<string, number>();
  const keyedOrbits = new Map<string, KeplerOrbit>();
  const members: PairMember[] = [];
  if (nodes.length > 0) {
    placements.set(keyOf(0), { kind: "origin" });
    reachM.set(keyOf(0), 0);
  }
  for (const [index, node] of nodes.entries()) {
    if (node.kind === "star") {
      continue;
    }
    const parent = keyOf(index);
    const innerMass = massOf(node.inner);
    const outerMass = massOf(node.outer);
    const total = innerMass + outerMass;
    const parentReach = reachM.get(parent) ?? 0;
    const apoapsisM = node.orbit.semiMajorAxisM * (1 + node.orbit.eccentricity);
    keyedOrbits.set(firstStarOf(node.outer), node.orbit);
    for (const [child, share, outer] of [
      [node.inner, -outerMass / total, false],
      [node.outer, innerMass / total, true],
    ] as const) {
      const key = keyOf(child);
      placements.set(key, { kind: "member", parentId: parent, orbit: node.orbit, share });
      reachM.set(key, parentReach + Math.abs(share) * apoapsisM);
      members.push({
        key,
        hostId: nodes[child]?.kind === "star" ? key : null,
        pairKey: parent,
        orbit: node.orbit,
        share,
        outer,
      });
    }
  }

  // The primary is always in a pair's inner member, which holds the lower-indexed stars.
  let primaryPairNormal: Vec3 | null = null;
  let at = nodes[0];
  while (at?.kind === "pair") {
    primaryPairNormal = orbitNormal(at.orbit);
    at = nodes[at.inner];
  }
  return { placements, members, reachM, primaryPairNormal, keyedOrbits };
}
