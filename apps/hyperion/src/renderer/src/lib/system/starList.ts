/**
 * The rows of a system's star list: each star under its component letter, and each orbit named by
 * the letters it joins (plan 11, P11.T14).
 *
 * @remarks
 * Pure functions of the model. The letters follow the hierarchy's own order, depth first from the
 * root with each pair's inner member first, which is also the stars' body-index order (plan 11's
 * `HierarchyDto`): `A` is the primary, `B` the next star in, and so on.
 */
import type { KeplerOrbit } from "../orbit";
import type { HierarchyNode, HostBody, SystemModel } from "./model";

/** One star of the list: the star, and its component letter. */
export interface StarListStar {
  /** `A` for the primary, then `B`, `C` in hierarchy order. */
  readonly letter: string;
  readonly host: HostBody;
}

/** One orbit of the list: a pair's relative orbit, named by the stars it joins. */
export interface StarListOrbit {
  /** The pair's node in the hierarchy, `pair:<index>`, unique within the system. */
  readonly id: string;
  /** The inner member's letters, an en dash, and the outer member's: `A–B`, `AB–C`. */
  readonly label: string;
  readonly orbit: KeplerOrbit;
}

/** A system's star list. */
export interface StarListRows {
  readonly stars: ReadonlyArray<StarListStar>;
  /** Every pair's orbit, in hierarchy order: the outermost first. */
  readonly orbits: ReadonlyArray<StarListOrbit>;
}

/** The letter of the `position`-th star in hierarchy order, from 0: `A`, `B`, … */
function letterAt(position: number): string {
  return String.fromCodePoint(0x41 + position);
}

/** The body indices of the stars under node `index`, in hierarchy order. */
function starsUnder(nodes: ReadonlyArray<HierarchyNode>, index: number): ReadonlyArray<number> {
  const node = nodes[index];
  if (node === undefined) {
    return [];
  }
  return node.kind === "star"
    ? [node.bodyIndex]
    : [...starsUnder(nodes, node.inner), ...starsUnder(nodes, node.outer)];
}

/**
 * The star list of a system: every star with its letter, in hierarchy order, and every pair's orbit
 * named by the letters of the stars it joins.
 *
 * @remarks
 * A star the hierarchy does not name, which a model `toSystemModel` accepted never has, follows the
 * named ones in body-index order, so that no star is left out of the list.
 */
export function starListRows(model: SystemModel): StarListRows {
  const nodes = model.hierarchy;
  const order = nodes.length === 0 ? [] : starsUnder(nodes, 0);
  const unnamed = model.hosts
    .map((host) => host.bodyIndex)
    .filter((bodyIndex) => !order.includes(bodyIndex));
  const letters = new Map<number, string>(
    [...order, ...unnamed].map((bodyIndex, position) => [bodyIndex, letterAt(position)]),
  );
  const stars = [...order, ...unnamed].flatMap((bodyIndex) => {
    const host = model.hosts.find((candidate) => candidate.bodyIndex === bodyIndex);
    const letter = letters.get(bodyIndex);
    return host === undefined || letter === undefined ? [] : [{ letter, host }];
  });
  const lettersOf = (index: number): string =>
    starsUnder(nodes, index)
      .map((bodyIndex) => letters.get(bodyIndex) ?? "")
      .join("");
  const orbits = nodes.flatMap((node, index) =>
    node.kind === "pair"
      ? [
          {
            id: `pair:${index}`,
            label: `${lettersOf(node.inner)}–${lettersOf(node.outer)}`,
            orbit: node.orbit,
          },
        ]
      : [],
  );
  return { stars, orbits };
}
