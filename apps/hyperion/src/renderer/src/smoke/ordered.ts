/**
 * Checks recorded in the order their work began, while the work itself overlaps: a group draws and
 * reads back its GPU frames one after another, and each frame's comparison with its CPU twin waits
 * for the twin (`twinPool.ts`) without holding up the next frame (perf-smoke, 2026-10-13).
 *
 * @remarks
 * A group's checks, and how it fails, are as they were in sequence. Each piece of work records into
 * a slot of its own, and the slots are appended to the group's checks in the order they were begun.
 * Should a piece throw, the checks of the slots before it and its own checks up to the throw are
 * recorded, the later slots' are dropped, and the group fails with that throw, as the group in
 * sequence would have stopped there. A throw in the group's own sequence (a GPU step) comes after
 * every slot begun before it, so an earlier slot's throw is the one the group fails with. Once a
 * slot has failed, the group's next step that begins a slot throws, so that the group draws no more
 * frames whose checks would be dropped.
 */

import { Checks } from "./harness";

/** One piece of work, its checks, and its end. */
interface Slot {
  readonly checks: Checks;
  readonly done: Promise<unknown>;
}

/** The slots of one group's work, in the order begun. */
export class OrderedChecks {
  readonly #slots: Slot[] = [];
  /** Whether a slot has failed, after which the group begins no more work. */
  #failed = false;

  /**
   * Records checks that need nothing more, in a slot of their own.
   *
   * @throws Error once a slot has failed, so that the group's sequence stops there.
   */
  now(record: (checks: Checks) => void): void {
    this.#goOn();
    const checks = new Checks();
    record(checks);
    this.#slots.push({ checks, done: Promise.resolve() });
  }

  /**
   * Begins `work` at once, its checks recorded in a slot of its own; it runs until it awaits, and
   * the group's sequence goes on meanwhile.
   *
   * @returns The work's result, marked handled ({@link handled}): its failure is the slot's, which
   *   {@link OrderedChecks.settle} rethrows in order, so a caller with no use for the result
   *   discards it with `void`.
   * @throws Error once a slot has failed, so that the group's sequence stops there: the slots after
   *   the failed one are dropped, so the work would be wasted.
   */
  later<T>(work: (checks: Checks) => Promise<T>): Promise<T> {
    this.#goOn();
    const checks = new Checks();
    const done = work(checks);
    // `settle` awaits it and rethrows its failure in order.
    void done.catch(() => {
      this.#failed = true;
    });
    this.#slots.push({ checks, done });
    return done;
  }

  /** Stops the group's sequence once a slot has failed. */
  #goOn(): void {
    if (this.#failed) {
      throw new Error("the group stopped: one of its checks' work failed before this step");
    }
  }

  /**
   * Waits for every slot in order, appending its checks to `checks`.
   *
   * @throws The first slot's failure in order, once that slot's checks are appended.
   */
  async settle(checks: Checks): Promise<void> {
    for (const slot of this.#slots) {
      let failure: { readonly error: unknown } | null = null;
      try {
        // The slots are read in the order begun, each after the one before it.
        // oxlint-disable-next-line no-await-in-loop
        await slot.done;
      } catch (error: unknown) {
        failure = { error };
      }
      for (const check of slot.checks.list) {
        checks.check(check.name, check.pass, check.detail);
      }
      if (failure !== null) {
        throw failure.error;
      }
    }
  }
}

/**
 * `promise`, marked handled: work begun ahead of its turn whose failure is read where it is
 * awaited, so that a group that fails before then leaves no unhandled rejection.
 */
export function handled<T>(promise: Promise<T>): Promise<T> {
  // Marked handled here, not reported: the caller that awaits it sees the rejection.
  void promise.catch(() => undefined);
  return promise;
}

/** `compute()` at once, its throw a rejection read where the promise is awaited ({@link handled}). */
export function attempt<T>(compute: () => T): Promise<T> {
  return handled(
    new Promise<T>((resolve) => {
      resolve(compute());
    }),
  );
}

/**
 * Runs a group's `body` with its checks in order ({@link OrderedChecks}), then records them into
 * `checks`.
 *
 * @throws The first failure in the order of the group's sequence: a slot's, or the body's own.
 */
export async function inOrder(
  checks: Checks,
  body: (ordered: OrderedChecks) => Promise<void>,
): Promise<void> {
  const ordered = new OrderedChecks();
  let failure: { readonly error: unknown } | null = null;
  try {
    await body(ordered);
  } catch (error: unknown) {
    failure = { error };
  }
  await ordered.settle(checks);
  if (failure !== null) {
    throw failure.error;
  }
}
