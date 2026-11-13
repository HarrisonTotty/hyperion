/**
 * The guard of a terrain computation a view's loop runs every frame (plan R10, R10.T16.e;
 * decision-r10-t16-leans §8; the batch review's S5): the terrain session's step in
 * `stepTerrainSession`, a view's survey edges in its drawer, and the terrain readout's queries,
 * whose answers come later (R10.T12; decision-r10-t12-leans §5), through
 * {@link TerrainFailureGuard.succeeded} and {@link TerrainFailureGuard.failed}.
 *
 * @remarks
 * No throw of a guarded call stops the loop. A device loss (`EngineUnavailable`) passes in silence:
 * the call is made again at the restore. Any other throw, a bug, is said once with
 * `console.error` until the call succeeds again; and once its failures have lasted
 * {@link ANNUNCIATION_ONSET_MS} on end, the guard faults the session's terrain
 * (`TerrainSession.failComputation`), as lost workers do, so that no view draws on with a stopped
 * terrain beside a moving camera. A single failed frame raises nothing.
 */

import { EngineUnavailable } from "../engine/resilientEngine";
import { ANNUNCIATION_ONSET_MS } from "./annunciation";

/** What a guarded call is said and faulted by. */
export interface GuardedCall {
  /** The `console.error`'s message, said once for a run of failures, the error after it. */
  readonly describe: string;
  /**
   * Faults the terrain (`TerrainSession.failComputation`), once a run of failures has lasted
   * {@link ANNUNCIATION_ONSET_MS}: called once a run.
   */
  readonly onFault: () => void;
}

/** One guarded call's failures (see the module's remarks): one a call a loop makes, a session. */
export class TerrainFailureGuard {
  /** When the run of failures standing began, ms; `null` while the call succeeds. */
  #failingSinceMs: number | null = null;
  /** Whether the run standing has faulted the terrain. */
  #faulted = false;

  /** Whether the call's last outcome but a loss was a failure: a run of failures stands. */
  get failing(): boolean {
    return this.#failingSinceMs !== null;
  }

  /**
   * Makes `call` and returns its value, or `fallback` where it throws.
   *
   * @remarks
   * A success ends the run of failures, so that the next failure is said again and its onset
   * counted afresh; a loss neither starts nor ends one.
   *
   * @param nowMs - The frame's monotonic time, ms, by which a run's length is counted.
   */
  run<T>(call: () => T, fallback: T, nowMs: number, guarded: GuardedCall): T {
    let value: T;
    try {
      value = call();
    } catch (error: unknown) {
      this.failed(error, nowMs, guarded);
      return fallback;
    }
    this.succeeded();
    return value;
  }

  /**
   * Takes a success of a call whose outcome comes later than its making (an answer that arrived):
   * it ends the run of failures, as a call `run` makes that returns does.
   */
  succeeded(): void {
    this.#failingSinceMs = null;
    this.#faulted = false;
  }

  /**
   * Takes a failure of a call whose outcome comes later than its making (an answer refused), as a
   * call `run` makes that throws: a loss (`EngineUnavailable`) in silence; anything else said once
   * for a run of failures, and the terrain faulted once the run has lasted
   * {@link ANNUNCIATION_ONSET_MS}.
   *
   * @param nowMs - The time the failure is taken at, the frame's monotonic time, ms.
   */
  failed(error: unknown, nowMs: number, guarded: GuardedCall): void {
    if (error instanceof EngineUnavailable) {
      return;
    }
    if (this.#failingSinceMs === null) {
      this.#failingSinceMs = nowMs;
      console.error(guarded.describe, error);
    }
    if (!this.#faulted && nowMs - this.#failingSinceMs >= ANNUNCIATION_ONSET_MS) {
      this.#faulted = true;
      guarded.onFault();
    }
  }
}
