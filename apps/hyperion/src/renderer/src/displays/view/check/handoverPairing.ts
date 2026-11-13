/**
 * The hand-over check's bookkeeping of a look (plan R10, R10.T10.g), apart from the page: which
 * primary frames are read back, which two of those read are compared across each switch, how the
 * two sit among the frames drawn, and whether the exposure settled and its lock held.
 *
 * @remarks
 * - **The read-backs** ({@link readBackWanted}): on the way in from {@link HANDOVER_IN_WINDOW} of
 *   the range, and through the hold, until the terrain's first draw has been paired; on the way out
 *   from {@link HANDOVER_OUT_WINDOW}, once it has, until the hand-back has been paired.
 * - **The pairs** ({@link SwitchPairing}): each frame read, in the order drawn, against the one read
 *   before it. At the first change from the disc (to the figure, or to the terrain where its first
 *   mesh frame drew it) the regime's switch, `disc-to-figure`, pairs the disc's last frame read
 *   with it; at the first terrain frame, `disc-to-terrain` pairs that disc frame with it; and at the
 *   first disc frame after that, `terrain-to-disc` pairs the terrain's last frame read with it. A
 *   look's first frame read must be the disc's.
 * - **Adjacency** ({@link pairAdjacency}): how many frames were drawn from one to the other, and
 *   whether each was its regime's last or first; a read-back skipped shows there, recorded, not
 *   failed.
 * - **The exposure** ({@link SettleWatch}, {@link LockWatch}): settled once `AUTO` stands within a
 *   step for a while, and held while every reading after the lock is the locked one.
 */

import type { HandoverRegime, HandoverSwitchName } from "../../../../../preload/api";

/** The camera's distance to the limb over the range at which the way in's read-backs begin. */
export const HANDOVER_IN_WINDOW = 0.97;

/** The same at which the way out's begin. */
export const HANDOVER_OUT_WINDOW = 1.04;

/** A step of `EXPOSURE`'s reading, EV, with a little over for its rounding. */
export const EV100_STEP = 0.15;

/** The part of a look's line a frame was drawn on (`approachPart`). */
export type LinePart = "in" | "hold" | "out";

/**
 * Whether a frame drawn on `part` of the line at `ratio` of the range is read back, the switches
 * `paired` so far.
 */
export function readBackWanted(
  part: LinePart,
  ratio: number,
  paired: ReadonlySet<HandoverSwitchName>,
): boolean {
  const inDone = paired.has("disc-to-terrain");
  let wanted: boolean;
  switch (part) {
    case "in":
      wanted = ratio <= HANDOVER_IN_WINDOW && !inDone;
      break;
    case "hold":
      wanted = !inDone;
      break;
    case "out":
      wanted = ratio >= HANDOVER_OUT_WINDOW && inDone && !paired.has("terrain-to-disc");
      break;
  }
  return wanted;
}

/** A frame read back, as the pairing sees it. */
export interface PairedFrame {
  /** Its place among the look's frames drawn. */
  readonly index: number;
  readonly regime: HandoverRegime;
  /** The script's time it drew, s. */
  readonly scriptS: number;
}

/** Two frames read either side of a switch. */
export interface FramePair<T extends PairedFrame> {
  readonly name: HandoverSwitchName;
  readonly before: T;
  readonly after: T;
}

/** A look's frames read back, paired across its switches: see the module's remarks. */
export class SwitchPairing<T extends PairedFrame> {
  readonly #paired = new Set<HandoverSwitchName>();
  /** Why the frames read do not pair as a look's should. */
  readonly #failures: string[] = [];
  #previous: T | null = null;
  #discLast: T | null = null;
  #lastTerrain: T | null = null;
  #first = true;

  /** The switches paired so far. */
  get paired(): ReadonlySet<HandoverSwitchName> {
    return this.#paired;
  }

  /** Why the frames read did not pair as a look's should, so far. */
  get failures(): ReadonlyArray<string> {
    return this.#failures;
  }

  /** Whether a frame drawn on `part` of the line at `ratio` of the range is read back. */
  wants(part: LinePart, ratio: number): boolean {
    return readBackWanted(part, ratio, this.#paired);
  }

  /** Takes the next frame read back, in the order drawn; returns the pairs it completes. */
  take(frame: T): ReadonlyArray<FramePair<T>> {
    const previous = this.#previous;
    this.#previous = frame;
    const { regime } = frame;
    if (this.#first) {
      this.#first = false;
      if (regime !== "disc") {
        this.#failures.push(
          `the first frame read on the way in, at ${frame.scriptS.toFixed(2)} s of the script, drew the ${regime}, not the disc`,
        );
      }
    }
    if (regime === "terrain") {
      this.#lastTerrain = frame;
    }
    if (previous === null || previous.regime === regime) {
      return [];
    }
    const pairs: Array<FramePair<T>> = [];
    const pair = (name: HandoverSwitchName, before: T): void => {
      this.#paired.add(name);
      pairs.push({ name, before, after: frame });
    };
    if (
      !this.#paired.has("disc-to-figure") &&
      previous.regime === "disc" &&
      (regime === "figure" || regime === "terrain")
    ) {
      this.#discLast = previous;
      pair("disc-to-figure", previous);
    }
    const discLast = this.#discLast;
    if (!this.#paired.has("disc-to-terrain") && regime === "terrain" && discLast !== null) {
      pair("disc-to-terrain", discLast);
    }
    const lastTerrain = this.#lastTerrain;
    if (
      this.#paired.has("disc-to-terrain") &&
      !this.#paired.has("terrain-to-disc") &&
      regime === "disc" &&
      lastTerrain !== null
    ) {
      pair("terrain-to-disc", lastTerrain);
    }
    return pairs;
  }
}

/** How a pair's two frames sit among the frames drawn. */
export interface PairAdjacency {
  /** The frames drawn from one to the other: 1 where they are consecutive. */
  readonly framesApart: number;
  /** Whether the frame before was its regime's last drawn, and the frame after its own's first. */
  readonly beforeWasLast: boolean;
  readonly afterWasFirst: boolean;
}

/** How `before` and `after` sit among the frames `drawn`, by their indices there. */
export function pairAdjacency(
  drawn: ReadonlyArray<{ readonly regime: HandoverRegime }>,
  before: PairedFrame,
  after: PairedFrame,
): PairAdjacency {
  const next = drawn[before.index + 1];
  const previous = drawn[after.index - 1];
  return {
    framesApart: after.index - before.index,
    beforeWasLast: next === undefined || next.regime !== before.regime,
    afterWasFirst: previous === undefined || previous.regime !== after.regime,
  };
}

/** `EXPOSURE`'s reading taken apart (`parseExposureReading`). */
export interface ExposureReading {
  readonly ev100: number;
  readonly level: string;
}

/**
 * `AUTO`'s settling: settled once its EV100 has stood within {@link EV100_STEP} of where it stood
 * for `settleMs`, so that a controller dithering by one step of the reading settles; any reading
 * that is not `AUTO`'s starts it again.
 */
export class SettleWatch {
  readonly #settleMs: number;
  #standing: { readonly ev100: number; readonly sinceMs: number } | null = null;

  constructor(settleMs: number) {
    this.#settleMs = settleMs;
  }

  /** Takes a reading at `nowMs`, ms; returns whether the exposure has settled. */
  note(reading: ExposureReading | null, nowMs: number): boolean {
    if (reading === null || reading.level !== "AUTO") {
      this.#standing = null;
      return false;
    }
    const standing =
      this.#standing === null || Math.abs(this.#standing.ev100 - reading.ev100) > EV100_STEP
        ? { ev100: reading.ev100, sinceMs: nowMs }
        : this.#standing;
    this.#standing = standing;
    return nowMs - standing.sinceMs >= this.#settleMs;
  }
}

/**
 * The exposure's lock watched to the look's end: held while every reading after it is the locked
 * one, never again once one is not; a lock never taken (`null`) is never held.
 */
export class LockWatch {
  readonly #locked: string | null;
  #held: boolean;
  #endReading: string | null;

  constructor(locked: string | null) {
    this.#locked = locked;
    this.#held = locked !== null;
    this.#endReading = locked;
  }

  /** Takes `EXPOSURE`'s reading now. */
  note(reading: string | null): void {
    if (this.#locked === null) {
      return;
    }
    this.#endReading = reading;
    this.#held &&= reading === this.#locked;
  }

  /** Whether the lock has held so far. */
  get held(): boolean {
    return this.#held;
  }

  /** The last reading taken, or the locked one before any. */
  get endReading(): string | null {
    return this.#endReading;
  }
}
