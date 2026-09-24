/**
 * The rows of the `SYSTEM` display's body list (plan 14, P14.T43.a).
 */
import { type FormattedDistance, formatBodyDistance } from "../../lib/format";
import type { HierarchyLayout } from "../../lib/system/hierarchy";
import type { HostBody } from "../../lib/system/model";
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
   * that brought it in, the pair it keys; `null` for the primary, which keys no orbit.
   */
  readonly semiMajorAxis: FormattedDistance | null;
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
 * with the display time, so it needs no unit held from before. The planets join under their hosts
 * with `system_bodies`, by semi-major axis, and their moons and rings under them.
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
    };
  });
}
