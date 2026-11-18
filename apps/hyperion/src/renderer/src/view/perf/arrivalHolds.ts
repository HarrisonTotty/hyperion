/**
 * `arrival-memory`'s two holds (plan R12, R12.T4.c; Design notes 8 and 13): the descent's first
 * pose held while the arrival's surveyed field reaches every height worker, then its deepest held
 * until its patches are resident and for {@link ARRIVAL_HELD_S} after, the span the run measures
 * the resident memory's peak over.
 *
 * @remarks
 * A pure state machine over the repetition's frames, told each drawn frame's time from the span's
 * first frame and what the views found then; the descent spike draws the pose it names
 * ({@link ArrivalHolds.deep}). The field is resident once the store holds the field's answer, its
 * header, and every height worker reports the same copy of it, so that it is whole in each. The
 * deepest pose's patches are resident once its frames have drawn patches for
 * {@link ARRIVAL_SETTLE_S} with no `TERRAIN: STREAMING` and no patch made resident: the line shows
 * a condition held for 250 ms (`terrain/annunciation.ts`), so a selection still waiting on patches
 * after the jump raises it within the window, and a pose whose drafts are still refined makes
 * patches resident without it (a pod's first run did so through its held minute). A pose that has
 * not settled {@link ARRIVAL_SETTLE_LIMIT_S} after the jump is held from then, so that a run
 * cannot wait on it for ever. The span's segments are the three stretches as they ran: `arrival`,
 * `deepest` until resident, `held`.
 */

import type { MeasuredSpan } from "./runPerfScene";
import { ARRIVAL_HELD_S } from "./scripts";

/**
 * How long the deepest pose's frames must draw patches with no `TERRAIN: STREAMING` and no patch
 * made resident before its patches count as resident, s: 2, eight times the line's 250 ms onset (a
 * lean, 2026-11-18).
 */
export const ARRIVAL_SETTLE_S = 2;

/**
 * The longest the deepest pose waits to settle before it is held all the same, s: 120, twice the
 * held minute (a lean, 2026-11-18).
 */
export const ARRIVAL_SETTLE_LIMIT_S = 120;

/** Where the holds stand. */
export type ArrivalStage = "arrival" | "deepest" | "held" | "flown";

/** What one drawn frame of the span found. */
export interface ArrivalFrame {
  /** Its time from the span's first frame, s. */
  readonly spanS: number;
  /** Whether the world's field is whole in every height worker. */
  readonly fieldResident: boolean;
  /** Whether `TERRAIN: STREAMING` showed. */
  readonly streaming: boolean;
  /** The patches the views' caches hold. */
  readonly patches: number;
  /** When the latest patch was made resident, s from the span's first frame, or `null` for none. */
  readonly lastResidentS: number | null;
}

/** The holds of one repetition of `arrival-memory`. */
export class ArrivalHolds {
  readonly #heldS: number;
  readonly #settleS: number;
  #stage: ArrivalStage = "arrival";
  #deepestS = 0;
  #settledFromS: number | null = null;
  #heldFromS = 0;
  #endS = 0;

  /**
   * @param heldS - How long the deepest pose is held once resident, s.
   * @param settleS - How long its frames must draw patches without streaming, s.
   */
  constructor(heldS: number = ARRIVAL_HELD_S, settleS: number = ARRIVAL_SETTLE_S) {
    this.#heldS = heldS;
    this.#settleS = settleS;
  }

  /** Where the holds stand. */
  get stage(): ArrivalStage {
    return this.#stage;
  }

  /** Whether the views stand at the descent's deepest pose: from the frame after the field's. */
  get deep(): boolean {
    return this.#stage !== "arrival";
  }

  /** Steps the holds at a frame drawn; returns where they stand after it. */
  frame(frame: ArrivalFrame): ArrivalStage {
    switch (this.#stage) {
      case "arrival":
        if (frame.fieldResident) {
          this.#stage = "deepest";
          this.#deepestS = frame.spanS;
        }
        break;
      case "deepest": {
        // The frame the field was found in drew the first pose; the next is the deepest's.
        if (frame.spanS > this.#deepestS && !frame.streaming && frame.patches > 0) {
          this.#settledFromS ??= frame.spanS;
        } else {
          this.#settledFromS = null;
        }
        const quietFromS =
          this.#settledFromS === null
            ? null
            : Math.max(this.#settledFromS, frame.lastResidentS ?? Number.NEGATIVE_INFINITY);
        if (
          (quietFromS !== null && frame.spanS - quietFromS >= this.#settleS) ||
          frame.spanS - this.#deepestS >= ARRIVAL_SETTLE_LIMIT_S
        ) {
          this.#stage = "held";
          this.#heldFromS = frame.spanS;
        }
        break;
      }
      case "held":
        if (frame.spanS - this.#heldFromS >= this.#heldS) {
          this.#stage = "flown";
          this.#endS = frame.spanS;
        }
        break;
      case "flown":
        break;
    }
    return this.#stage;
  }

  /**
   * The span flown and its three stretches, s from its first frame.
   *
   * @throws Error before the holds have flown.
   */
  span(): MeasuredSpan {
    if (this.#stage !== "flown") {
      throw new Error("arrival-memory's span is not flown yet");
    }
    return {
      spanS: this.#endS,
      segments: [
        { name: "arrival", startS: 0, endS: this.#deepestS },
        { name: "deepest", startS: this.#deepestS, endS: this.#heldFromS },
        { name: "held", startS: this.#heldFromS, endS: this.#endS },
      ],
    };
  }
}
