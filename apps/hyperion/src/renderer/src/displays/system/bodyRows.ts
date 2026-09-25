/**
 * The rows of the `SYSTEM` display's body list (plan 14, P14.T43.a).
 */
import { formatBodyId, type SystemIdHex } from "@hyperion/protocol";

import { type FormattedDistance, formatBodyDistance } from "../../lib/format";
import { bodyKindLabel, bodyStateLabel } from "../../lib/system/bodyWords";
import type { HierarchyLayout } from "../../lib/system/hierarchy";
import type { HostBody, SystemBody } from "../../lib/system/model";
import { objectKindLabel } from "../../lib/system/words";

/** One row of the body list: a body, where it sits in the tree, and what the row reads. */
export interface BodyRow {
  /** The body's ID, which a selection names. */
  readonly id: string;
  /** The row it sits under, or `null` at the top of the tree, where the hosts are. */
  readonly parentId: string | null;
  /** Its depth in the tree, from 1 for a host. */
  readonly level: number;
  readonly designation: string;
  /** What it is, in words. */
  readonly kind: string;
  /**
   * The semi-major axis of the orbit it is listed with: a companion star's is the relative orbit
   * that brought it in, the pair it keys, and a body's its own; `null` for the primary, which keys
   * no orbit, and for a body whose orbit is withheld or does not apply.
   */
  readonly semiMajorAxis: FormattedDistance | null;
  /** What has become of it in words when it is not present: `DESTROYED`; `null` when present. */
  readonly state: string | null;
}

/** Kilometres in a metre. */
const KM_PER_M = 1e-3;

/**
 * The body list's rows for the hosts, in body-index order, each at the top of the tree (plan 14,
 * P14.T43.a).
 *
 * @remarks
 * Each row reads the host's designation, its kind in words and, for a companion, the semi-major
 * axis of the relative orbit it keys, in `formatBodyDistance`'s units; that orbit does not move
 * with the display time, so it needs no unit held from before.
 */
export function hostRows(
  hosts: ReadonlyArray<HostBody>,
  layout: HierarchyLayout,
): ReadonlyArray<BodyRow> {
  return hosts.map((host) => {
    const orbit = layout.keyedOrbits.get(host.id);
    return {
      id: host.id,
      parentId: null,
      level: 1,
      designation: host.designation,
      kind: objectKindLabel(host.kind),
      semiMajorAxis:
        orbit === undefined ? null : formatBodyDistance(orbit.semiMajorAxisM * KM_PER_M, null),
      state: null,
    };
  });
}

/** A body's semi-major axis in metres, or `null` when its orbit is not on show. */
function semiMajorAxisM(body: SystemBody): number | null {
  return body.orbit.state === "ok" ? body.orbit.value.orbit.semiMajorAxisM : null;
}

/**
 * How far from its parent a body is listed at: its semi-major axis, or a ring's, a belt's or the
 * halo's inner edge, so that a belt stands among the planets where it lies and a ring among the
 * moons; `null` for a body with neither on show.
 */
function listedDistanceM(body: SystemBody): number | null {
  const axisM = semiMajorAxisM(body);
  if (axisM !== null) {
    return axisM;
  }
  return body.population.state === "ok" ? body.population.value.innerEdgeM : null;
}

/** Bodies nearest their parent first, then those with no distance on show, each by body index. */
function bySemiMajorAxis(a: SystemBody, b: SystemBody): number {
  const aM = listedDistanceM(a);
  const bM = listedDistanceM(b);
  if (aM !== null && bM !== null && aM !== bM) {
    return aM - bM;
  }
  if ((aM === null) !== (bM === null)) {
    return aM === null ? 1 : -1;
  }
  return a.bodyIndex - b.bodyIndex;
}

/** A body's row, under `parentId` at `level`. */
function bodyRow(body: SystemBody, parentId: string | null, level: number): BodyRow {
  const axisM = semiMajorAxisM(body);
  return {
    id: body.id,
    parentId,
    level,
    designation: body.designation,
    kind: bodyKindLabel(body.kind),
    semiMajorAxis: axisM === null ? null : formatBodyDistance(axisM * KM_PER_M, null),
    state: body.state.kind === "present" ? null : bodyStateLabel(body.state),
  };
}

/**
 * The whole body list: the hosts, and under each the bodies that orbit it, by semi-major axis, with
 * the bodies of each body under it, as moons and rings under their planet and a belt's members
 * under the belt (plan 14, P14.T43.a).
 *
 * @remarks
 * A body sits under the row of what it belongs to: its star, or the body it orbits. What orbits a
 * pair or the whole system, as a circumbinary planet or the cometary halo, has no row to sit under
 * and follows the hosts at the top of the tree. A belt's members sit under the belt, though they
 * orbit its star. Siblings run nearest first, by semi-major axis, or by inner edge for a ring, a
 * belt or the halo; those with neither on show (withheld, not applicable, or gone with the body)
 * follow in body-index order. Every body is listed, drawn or not: a destroyed, unbound or unformed body reads its state
 * in words.
 */
export function systemRows(
  system: SystemIdHex,
  hosts: ReadonlyArray<HostBody>,
  layout: HierarchyLayout,
  bodies: ReadonlyArray<SystemBody>,
): ReadonlyArray<BodyRow> {
  const ids = new Set(bodies.map((body) => body.id));
  const hostIds = new Set(hosts.map((host) => host.id));
  const rowParentOf = (body: SystemBody): string | null => {
    const parent = body.parent;
    if (parent?.kind === "star") {
      const id = formatBodyId({ system, bodyIndex: parent.bodyIndex });
      return hostIds.has(id) ? id : null;
    }
    return parent?.kind === "body" && ids.has(parent.id) ? parent.id : null;
  };
  const children = new Map<string | null, SystemBody[]>();
  for (const body of bodies) {
    const parentId = rowParentOf(body);
    children.set(parentId, [...(children.get(parentId) ?? []), body]);
  }

  const rows: BodyRow[] = [];
  const visit = (parentId: string, level: number): void => {
    for (const body of (children.get(parentId) ?? []).toSorted(bySemiMajorAxis)) {
      rows.push(bodyRow(body, parentId, level));
      visit(body.id, level + 1);
    }
  };
  for (const row of hostRows(hosts, layout)) {
    rows.push(row);
    visit(row.id, 2);
  }
  for (const body of (children.get(null) ?? []).toSorted(bySemiMajorAxis)) {
    rows.push(bodyRow(body, null, 1));
    visit(body.id, 2);
  }
  return rows;
}
