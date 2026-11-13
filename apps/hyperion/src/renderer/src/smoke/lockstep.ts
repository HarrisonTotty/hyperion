/**
 * The terrain checks' wait for a pass's streaming to settle (R10.T16's frame's passes in parts 15
 * and 20, R14.T3.c's grounded craft in part 16, R10.T16's shared pass in part 21 and R10.T9.b's
 * cascades in part 7): the pass stepped in lockstep with its workers, each step selecting only once every bake the
 * steps before asked for has landed, until a step meets the check's terms, and then stepped again
 * at once, with no bake landing between, until a step's morph bands rest and it draws what the step
 * before drew.
 *
 * @remarks
 * Selection refines only baked patches, and where the budget binds it holds the last selection's
 * splits past its margin (`terrainPass.ts`' `BUDGET_HOLD_MARGIN`), so which patches it splits
 * there depends on what had landed at each step before. Stepped every 50 ms whatever had landed,
 * part 15's frame settled on 77 patches in some runs and on 80 in others, by whether one level-9
 * patch was split before the budget bound, and its R14 captures with them: 46 or 49 interior
 * patches, digest 93e51627 or 615542ff (2026-11-03, on a pod beside another run). In lockstep each
 * selection reads the whole of the last one's bakes, as it would with workers that baked within a
 * frame, whatever their pace or the machine's load.
 *
 * And a step's frame is not yet the pass's last where its terms hold: the draw set's stand-in and
 * missing counts are the resolved set's (`timeMorph.ts`' `TimeMorph.present`), while the time morph
 * presents a refine of several levels a level a step and a coarsen's patch at the step after its
 * fade ends; and the morph bands go to the selection's τ′ at 2% a step (`terrainPass.ts`'
 * `slewedTauPx`), whatever lands. In lockstep few steps are taken, and part 15's first frame drew
 * f71db21f where the next ones drew 93e51627. The steps taken at once read the same bakes, so what
 * changes between them is the time morph's and the bands' alone, and the frame a check takes is the
 * first that both leave as it was. A grazing view's far ground streams on past its check's terms,
 * so no wait for the workers to have nothing left to bake would end.
 *
 * So that same far ground does not move the frames a check draws after its settle, each step of
 * them stepping the pass again, a pass's lane is held from the settle's end ({@link LaneHold}): its
 * bakes wait there, and land at the next settle's start. Stepped on as they landed, the line views
 * over grazing ground and R14.T1.e's redraws drew 7 and 8 digests in 8 runs. Part 7's cascades,
 * the budget binding (50 patches), settled on 43 patches or on other sets under load, R14.T1.d's
 * and R14.T6.b's frames with them.
 */

import type { DrawSet } from "../view/terrain/cache";
import { patchKeyString } from "../view/terrain/patchKey";
import type { PatchRequest } from "../view/terrain/select";
import type { TerrainPool } from "../view/terrain/terrainPass";
import type { RefineRequest } from "../view/terrain/workers/pool";
import { pause } from "./harness";

/** How often the wait looks at the workers again, milliseconds: the event loop's, between polls. */
const POLL_MS = 20;

/**
 * The most steps taken at once after a step that meets the terms: a refine's levels a step each (13
 * at most, `MAX_SUB_PATCH_LEVELS`), a coarsen's swap, and the bands' slew at 2% a step, which 64
 * steps carry over a τ′ 3.5 times theirs; past them, the wait goes on.
 */
const AT_ONCE_STEPS = 64;

/** One step of the pass, as {@link settleInLockstep} takes it. */
export interface LockstepStep<F> {
  /** Whether the step meets the check's terms. */
  readonly done: boolean;
  /** What the check reads of the step. */
  readonly frame: F;
  /**
   * The draw set the step presents, or `undefined` before the pass has one: the pass's own, which
   * the next step rewrites in place (`TerrainPass.lastStep`), so it is read before that step.
   */
  readonly drawSet: DrawSet | undefined;
  /**
   * Whether the view's morph bands are at the selection's τ′ (`TerrainPass.bandsAtTarget`), so
   * that the next step moves them no further.
   */
  readonly resting: boolean;
}

/** How {@link settleInLockstep} ended. */
export interface Settled<F> {
  /** Whether a step met the check's terms, its bands resting, drawing what the step before drew. */
  readonly done: boolean;
  /** The last step's frame. */
  readonly frame: F;
  /** How many steps were taken. */
  readonly steps: number;
}

/**
 * Each patch `drawSet` presents, in its order, by its key, the area it draws and its views: two
 * steps that draw the same patches over the same areas give the same string.
 */
export function drawnKey(drawSet: DrawSet | undefined): string {
  if (drawSet === undefined) {
    return "";
  }
  const parts: string[] = [];
  for (let n = 0; n < drawSet.count; n += 1) {
    const drawn = drawSet.patches[n];
    if (drawn !== undefined) {
      const area = drawn.within === null ? "" : patchKeyString(drawn.within);
      parts.push(
        `${drawn.keyString}<${area}${drawn.standIn ? "*" : ""}@${String(drawSet.views[n] ?? 0)}`,
      );
    }
  }
  return parts.join(" ");
}

/** Waits for `quiet`, polling every {@link POLL_MS}, until `deadlineMs`; whether it came. */
async function untilQuiet(quiet: () => boolean, deadlineMs: number): Promise<boolean> {
  for (;;) {
    // The workers answer on the event loop: give it them before each look.
    // oxlint-disable-next-line no-await-in-loop
    await pause(POLL_MS);
    if (quiet()) {
      return true;
    }
    if (performance.now() > deadlineMs) {
      return false;
    }
  }
}

/**
 * Steps a pass by `step` until a step is `done`, and a step taken at once after it is `done` and
 * `resting` and draws what the one before it drew, or `timeoutMs` passes. Every other step waits
 * for `streamed`, every bake the pass asked for landed (none queued, in flight or waiting, and none
 * refining), so that what each selection reads is the same whatever the workers' pace.
 */
export async function settleInLockstep<F>(
  step: () => LockstepStep<F>,
  streamed: () => boolean,
  timeoutMs: number,
): Promise<Settled<F>> {
  const deadlineMs = performance.now() + timeoutMs;
  let steps = 0;
  for (;;) {
    // Each step waits for the bakes the one before asked for: one after another, by design.
    // oxlint-disable-next-line no-await-in-loop
    const quiet = await untilQuiet(streamed, deadlineMs);
    let stepped = step();
    steps += 1;
    if (quiet) {
      // With no await between them, no bake lands between these steps.
      for (let again = 0; stepped.done && again < AT_ONCE_STEPS; again += 1) {
        const drew = drawnKey(stepped.drawSet);
        stepped = step();
        steps += 1;
        if (stepped.done && stepped.resting && drawnKey(stepped.drawSet) === drew) {
          return { done: true, frame: stepped.frame, steps };
        }
      }
    }
    if (performance.now() > deadlineMs) {
      return { done: false, frame: stepped.frame, steps };
    }
  }
}

/**
 * The hold on a pass's lanes ({@link LaneHold.wrap}): while held, what the pass asks still goes to
 * the workers, and what they answer waits here until {@link LaneHold.release} gives it to the
 * pass, in the order it came.
 */
export class LaneHold {
  #held = false;
  readonly #kept: Array<() => void> = [];

  /**
   * `lane` with its arrivals (a bake, a failure, refined normals) kept while held: the pool to give
   * the pass, with the parts `lane` has and no more, so that the pass refines drafts where `lane`
   * refines them. Its `refiningCount` is the lane's, which leaves out refined normals kept here:
   * the pass reads it after a release. A listener stopped gets none of the arrivals kept for it.
   */
  wrap(lane: TerrainPool): TerrainPool {
    const onFailed = lane.onFailed?.bind(lane);
    const refine = lane.refine?.bind(lane);
    const dropRefinement = lane.dropRefinement?.bind(lane);
    const onRefined = lane.onRefined?.bind(lane);
    return {
      reprioritise: (demand: ReadonlyArray<PatchRequest>) => {
        lane.reprioritise(demand);
      },
      onBaked: (cb) => this.#listen(cb, (kept) => lane.onBaked(kept)),
      terminate: () => {
        lane.terminate();
      },
      get unavailable(): boolean {
        return lane.unavailable === true;
      },
      get refiningCount(): number {
        return lane.refiningCount ?? 0;
      },
      ...(onFailed === undefined
        ? {}
        : { onFailed: (cb) => this.#listen(cb, (kept) => onFailed(kept)) }),
      ...(refine === undefined ? {} : { refine: (r: RefineRequest) => refine(r) }),
      ...(dropRefinement === undefined
        ? {}
        : { dropRefinement: (keyString: string) => dropRefinement(keyString) }),
      ...(onRefined === undefined
        ? {}
        : { onRefined: (cb) => this.#listen(cb, (kept) => onRefined(kept)) }),
    };
  }

  /** From now on, keeps each arrival from the pass. */
  hold(): void {
    this.#held = true;
  }

  /** Gives the pass every arrival kept, in the order they came, and keeps none from now on. */
  release(): void {
    this.#held = false;
    for (const arrival of this.#kept.splice(0)) {
      arrival();
    }
  }

  /**
   * Listens with `cb` by `listen`, its calls kept while held; the returned stop also drops what is
   * kept for it.
   */
  #listen<T>(
    cb: (arrival: T) => void,
    listen: (kept: (arrival: T) => void) => () => void,
  ): () => void {
    let live = true;
    const stop = listen((arrival) => {
      if (this.#held) {
        this.#kept.push(() => {
          if (live) {
            cb(arrival);
          }
        });
      } else {
        cb(arrival);
      }
    });
    return () => {
      live = false;
      stop();
    };
  }
}
