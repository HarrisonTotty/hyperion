/**
 * When R05.T13.b's spike captures stop waiting for a shot's patches: once every selected patch is
 * drawn as itself, once streaming has gone quiet, or at the settle limit.
 *
 * @remarks
 * Quiet is a while and a number of frames with none of the shot's terrain counts changing, not the
 * drawn less the standing-in count alone, and not a while alone. Selection refines only below baked
 * patches, so each frame after a bake selects anew. Between two frames the drawn less the
 * standing-in count can stay the same while patches land: 4 drawn and 5 standing in, then 6 and 7,
 * with five patches made resident between them. And each frame waits for its read-back. On
 * SwiftShader the spike's frame took 5 to 6 s on high and its first 3.2 s on low (2026-10-22, its
 * atmosphere's kernels; 0.6 s and 0.16 s on 2026-10-10), so one such frame outlasted a 3 s window
 * by itself and ended the wait two frames in, with every patch asked for baked and resident.
 */

/** The terrain counts of one frame of a shot (`SpikeTerrainReading`'s). */
export interface ShotTerrainCounts {
  readonly selected: number;
  readonly drawn: number;
  readonly standingIn: number;
  readonly missing: number;
}

/** The wait's limits. */
export interface SettleLimits {
  /** The longest a shot waits, ms. */
  readonly settleMs: number;
  /** How long with no count changing counts as quiet, ms. */
  readonly quietMs: number;
  /**
   * How many frames in a row with no count changing count as quiet, besides
   * {@link SettleLimits.quietMs}.
   */
  readonly quietFrames: number;
}

/** The spike captures' limits: 120 s, and quiet after 3 s and three frames with nothing changed. */
export const SPIKE_SETTLE_LIMITS: SettleLimits = {
  settleMs: 120_000,
  quietMs: 3_000,
  quietFrames: 3,
};

/** One shot's wait, told each frame's counts. */
export class StreamSettle {
  readonly #limits: SettleLimits;
  readonly #startMs: number;
  #last: ShotTerrainCounts | null = null;
  #changedMs: number;
  /** Frames in a row whose counts were the frame's before. */
  #quietFrames = 0;

  /** Starts the wait at `startMs`, `performance.now()`'s clock. */
  constructor(startMs: number, limits: SettleLimits = SPIKE_SETTLE_LIMITS) {
    this.#limits = limits;
    this.#startMs = startMs;
    this.#changedMs = startMs;
  }

  /**
   * Takes one frame's counts at `nowMs` and returns whether the wait ends: every selected patch
   * drawn as itself (none standing in or missing), quiet for the limits' while and frames, or past
   * the settle limit.
   */
  done(counts: ShotTerrainCounts, nowMs: number): boolean {
    const last = this.#last;
    if (
      last === null ||
      last.selected !== counts.selected ||
      last.drawn !== counts.drawn ||
      last.standingIn !== counts.standingIn ||
      last.missing !== counts.missing
    ) {
      const { selected, drawn, standingIn, missing } = counts;
      this.#last = { selected, drawn, standingIn, missing };
      this.#changedMs = nowMs;
      this.#quietFrames = 0;
    } else {
      this.#quietFrames += 1;
    }
    const { settleMs, quietMs, quietFrames } = this.#limits;
    return (
      (counts.standingIn === 0 && counts.missing === 0) ||
      (nowMs - this.#changedMs > quietMs && this.#quietFrames >= quietFrames) ||
      nowMs - this.#startMs > settleMs
    );
  }
}
