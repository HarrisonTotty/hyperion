/**
 * The stage a perf run's driver shows (plan R12, R12.T4.c): an external store `PerfApp`
 * subscribes to, set by the driver at each mount and cleared at each unmount, so that a
 * repetition's views are new under their own `key` (Design note 13's resets).
 */

/** A stage of type `S`, or none, which `PerfApp` renders. */
export class PerfStages<S> {
  #stage: S | null = null;
  readonly #listeners = new Set<() => void>();

  /** The stage now, or `null` between mounts. */
  readonly current = (): S | null => this.#stage;

  /** Listens for a change of stage; returns the unsubscribe. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** Shows `stage`, or none. */
  set(stage: S | null): void {
    this.#stage = stage;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}
