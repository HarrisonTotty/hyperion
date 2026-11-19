/**
 * A perf run's timed passes to the budget's rows (plan R12, R12.T4.c; Design note 4): each pass
 * keyed by its view's role and its label through `@hyperion/perf`'s `PASS_ROWS`, and each of R01's
 * resolves given the role of the view that made it.
 *
 * @remarks
 * R01's `PassTimes` carry no view: one resolve is one view's `renderFrame`, with the dispatches
 * that ride it. So a resolve's role is read from its labels: one holding a pass that only a
 * wireframe view submits is the scene's wireframe view's (an `instrument` in the cockpit, the
 * `station` in the station's scene), one holding a pass that only a photorealistic view submits is
 * the `primary`, and one holding only passes both styles submit is the scene's first view's. A
 * resolve holding passes of both styles, as a dispatch of one view riding another's resolve can
 * make, gives each pass the role its own style names.
 */

import {
  type BudgetRow,
  PASS_ROW_PREFIXES,
  PASS_ROWS,
  type PassKey,
  type ViewRole,
} from "@hyperion/perf";

import type { RenderStyle } from "../camera/state";
import { passOwnNs } from "../engine/passSpans";
import type { PassTimes } from "../engine/types";

/**
 * A timed pass with no row in `PASS_ROWS` for its role: a pass a later plan added without its
 * row, which ends the run rather than leave its time out of the table (Design note 4).
 */
export class UnmappedPass extends Error {
  override readonly name = "UnmappedPass";
  readonly key: PassKey;

  constructor(key: PassKey) {
    super(`the timed pass ${key} has no row in PASS_ROWS (Design note 4)`);
    this.key = key;
  }
}

/**
 * The budget row of the pass `label` timed on a view of `role`: its key's in `PASS_ROWS`, else the
 * row of the label's prefix in `PASS_ROW_PREFIXES` (the terrain's cluster counters, which carry the
 * view's ID, R12.T0's record).
 *
 * @throws {@link UnmappedPass} naming the key where neither has it.
 */
export function passRowOf(role: ViewRole, label: string): BudgetRow {
  const key: PassKey = `${role}:${label}`;
  const row = Object.hasOwn(PASS_ROWS, key) ? PASS_ROWS[key] : undefined;
  if (row !== undefined) {
    return row;
  }
  const prefixed = prefixedRows(label);
  if (prefixed !== undefined) {
    return prefixed[role];
  }
  throw new UnmappedPass(key);
}

/** The rows by role of a label `PASS_ROW_PREFIXES` keys by its prefix, or `undefined`. */
function prefixedRows(label: string): Readonly<Record<ViewRole, BudgetRow>> | undefined {
  return Object.entries(PASS_ROW_PREFIXES).find(([prefix]) => label.startsWith(prefix))?.[1];
}

/** The labels with a row on `role` in `PASS_ROWS`. */
function labelsOn(role: ViewRole): ReadonlySet<string> {
  const prefix = `${role}:`;
  return new Set(
    Object.keys(PASS_ROWS)
      .filter((key) => key.startsWith(prefix))
      .map((key) => key.slice(prefix.length)),
  );
}

const PRIMARY_LABELS = labelsOn("primary");
const WIREFRAME_LABELS = new Set([...labelsOn("instrument"), ...labelsOn("station")]);

/** Which style submits a pass: one alone, or both; `null` for a label `PASS_ROWS` does not name. */
export type PassStyle = RenderStyle | "both";

/**
 * The style that submits the pass `label`, as `PASS_ROWS` maps it, both for a label keyed by its
 * prefix; `null` where neither names it.
 */
export function passStyleOf(label: string): PassStyle | null {
  if (prefixedRows(label) !== undefined) {
    return "both";
  }
  const photorealistic = PRIMARY_LABELS.has(label);
  const wireframe = WIREFRAME_LABELS.has(label);
  if (photorealistic && wireframe) {
    return "both";
  }
  if (photorealistic) {
    return "photorealistic";
  }
  return wireframe ? "wireframe" : null;
}

/** The roles a scene's resolves are given (Design note 4): its first view's and its wireframe's. */
export interface PassRoles {
  /** The scene's first view's role: a resolve of shared passes alone. */
  readonly first: ViewRole;
  /** The scene's first wireframe view's role, the first view's where it has none. */
  readonly wireframe: ViewRole;
}

/** The roles of a scene's views, in its order: the primary or the station, then the instruments. */
export function passRolesOf(
  views: ReadonlyArray<{ readonly role: ViewRole; readonly style: RenderStyle }>,
): PassRoles {
  const first = views[0];
  if (first === undefined) {
    throw new Error("a perf scene draws at least one view");
  }
  const wireframe = views.find(({ style }) => style === "wireframe") ?? first;
  return { first: first.role, wireframe: wireframe.role };
}

/** One pass of a resolve, keyed and given its row. */
export interface AttributedPass {
  readonly key: PassKey;
  readonly row: BudgetRow;
  /** Its GPU time, ns: its span, from its first timestamp to its last. */
  readonly ns: number;
  /**
   * Its own time, ns (`passOwnNs`; lane fix-floor-calm): its span less what the resolve's passes
   * ending before it covered, so that the resolve's own times add to the union of its spans; its
   * span on a serial timer.
   */
  readonly ownNs: number;
}

/**
 * The passes of one resolve (`PassTimes`), each keyed by the role its resolve or its own style
 * gives it and its label, and given its row (the module's remarks).
 *
 * @throws {@link UnmappedPass} for the first pass with no row on its role.
 */
export function attributePasses(
  times: Pick<PassTimes, "passes">,
  roles: PassRoles,
): ReadonlyArray<AttributedPass> {
  const styles = times.passes.map(({ label }) => passStyleOf(label));
  const wireframe = styles.includes("wireframe");
  const photorealistic = styles.includes("photorealistic");
  const resolveRole: ViewRole = wireframe
    ? roles.wireframe
    : photorealistic
      ? "primary"
      : roles.first;
  const own = passOwnNs(times.passes);
  return times.passes.map(({ label, ns }, i) => {
    let role = resolveRole;
    if (wireframe && photorealistic) {
      const style = styles[i] ?? null;
      role =
        style === "wireframe"
          ? roles.wireframe
          : style === "photorealistic"
            ? "primary"
            : roles.first;
    }
    return { key: `${role}:${label}`, row: passRowOf(role, label), ns, ownNs: own[i] ?? 0 };
  });
}
