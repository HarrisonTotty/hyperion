import type { MapView } from "@hyperion/protocol";

/**
 * What a stale view of the galaxy map waits on while its stale picture stands (plan 07,
 * P07.T11.d; decision-gg-port-a2 §2): `map`, a map of the quantity shown, which reads plain, or
 * `extinction`, only the extinction map the dust overlay adds, which carries its subject.
 */
export type StaleWait = "map" | "extinction";

/** Each view's stale wait, or `null` for a view that has none. */
export type ViewWaits = Readonly<Record<MapView, StaleWait | null>>;

const NO_WAITS: ViewWaits = { face_on: null, edge_on: null };

/**
 * The stale waits of a `GALAXY MAP` page's two views, which each view reports and the page states
 * once, where the link's state stands.
 *
 * @remarks
 * The page holds no request (plan 07, P07.T11.b), so a view's wait is known to the view alone, and
 * the ruling has the view report it to the page (P07.T11.d: "The panel still holds no request: a
 * view reports its stale wait to the page"; decision-gg-port-a2 §2). It is reported here: a store
 * owned by the page for its lifetime, which each view keeps in step with its own wait from a
 * layout effect, and which the page reads through `useSyncExternalStore`. A layout effect's change
 * re-renders the page before the browser paints, so that the page's line and the stale pictures
 * appear together. The snapshot keeps its identity until a view's wait changes.
 *
 * This is a recorded exception to the TypeScript rule that no effect notifies a parent
 * (`.claude/rules/typescript-dev.md`, React; decision-p07-t11d-stale-waits, and plan 07's
 * P07.T11.d record). The rule's remedy, lifting both views' requests, decodes and kept pictures
 * into the page, is the structure P07.T11.b and P07.T11.d rule out ("the panel holds no
 * request"), and `RETRY` would no longer mount the view afresh (decision-gg-port-a2 §2.2). It
 * holds only within these bounds:
 * - the wait is computed during the view's render, and the effect does nothing but report it;
 * - the store belongs to one page, created with it, and is given to that page's two views alone;
 * - a report that changes nothing tells no one, so the page renders again once per change;
 * - the effect's cleanup reports `null`, so a view unmounted by `RETRY`, or hidden under
 *   `Activity`, leaves no wait behind.
 *
 * It is not a pattern for other displays. It goes if the map requests ever leave
 * `GalaxyMapView`: the page then derives the wait during render.
 */
export class StaleWaits {
  #waits: ViewWaits = NO_WAITS;
  readonly #listeners = new Set<() => void>();

  /**
   * Registers `listener`, called after every change.
   *
   * @returns Its removal.
   */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** Each view's stale wait now; the same object until one changes. */
  readonly getSnapshot = (): ViewWaits => this.#waits;

  /** Sets `view`'s stale wait, `null` for none, and tells the subscribers if it changed. */
  report(view: MapView, wait: StaleWait | null): void {
    if (this.#waits[view] === wait) {
      return;
    }
    this.#waits = { ...this.#waits, [view]: wait };
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/**
 * How the page states its views' stale waits, once for both: `PENDING` while either waits on a
 * map of the quantity shown, `EXTINCTION PENDING` while they wait only on the extinction map the
 * overlay adds, as a view's own status names that map (decision-gg-port-a2 §6), and `null` while
 * neither waits.
 */
export function staleWaitText(waits: ViewWaits): "PENDING" | "EXTINCTION PENDING" | null {
  const either = (wait: StaleWait): boolean => waits.face_on === wait || waits.edge_on === wait;
  if (either("map")) {
    return "PENDING";
  }
  return either("extinction") ? "EXTINCTION PENDING" : null;
}
