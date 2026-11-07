/**
 * The device's one budget of pipeline compiles in flight (plan R14's Mac measurements; a lean,
 * 2026-11-03): every `createRenderPipelineAsync` and `createComputePipelineAsync` the engine asks,
 * the bodies' preparations, the post passes, the composite, the atmosphere's kernels and the
 * terrain's materials among them, waits here for one of {@link DEFAULT_COMPILE_BUDGET} places.
 *
 * @remarks
 * - **Why.** A session's start asked some twenty compiles at once, and the GPU process ran them on
 *   as many threads (pod, SwiftShader, 2026-11-03), which Metal's compiler takes as wide: the load
 *   the owner's Mac saw at each new terrain session. A budget of `unlimited` asks each at once, as
 *   before, for an A/B (`--compile-budget`, `preload/compileBudgetLaunch.ts`).
 * - **The order is the order of first use.** A pipeline a draw is waiting for now (an asynchronous
 *   material's at its first draw) goes ahead of the rest; the rest go in the order asked, which is
 *   the order their owners need them: `VIEW`'s preparations as its views open, then a terrain
 *   session's materials in their own order (`TerrainMaterialSet`), then the atmosphere's kernels.
 * - **Presentation only.** A pipeline is the same whenever it is made; only when it is ready moves.
 *   The device's rounding probe, made once before any view draws, stands outside it.
 * - **Timed.** Each compile's wait for a place and its create-to-ready time go to its listeners as
 *   it settles ({@link CompileBudget.onCompile}), for an inspection profile's timing file (plan
 *   R08, fix-r08-decked-air-route): SwiftShader's one-time compiles told apart from its frames.
 */

import type { CompileBudgetSize, CompileEvent, CompileLane } from "../types";

/** The compiles a device runs at once by default (a lean, 2026-11-03; the brief's 2 to 3). */
export const DEFAULT_COMPILE_BUDGET = 3;

/**
 * Whether `size` is a budget a device can run: `unlimited`, or a whole number from 1.
 */
export function isCompileBudgetSize(size: unknown): size is CompileBudgetSize {
  return size === "unlimited" || (typeof size === "number" && Number.isInteger(size) && size >= 1);
}

/** One device's compiles in flight and waiting (see the module's remarks). */
export class CompileBudget {
  /** The compiles it runs at once; `Infinity` for `unlimited`. */
  readonly #size: number;
  readonly #nowMs: () => number;
  readonly #listeners = new Set<(event: CompileEvent) => void>();
  readonly #draws: Array<() => void> = [];
  readonly #ahead: Array<() => void> = [];
  #running = 0;
  #most = 0;

  /**
   * @param nowMs - The wall clock its compiles are timed on, ms: the page's, `performance.now()`.
   * @throws RangeError for a size that is not {@link isCompileBudgetSize}.
   */
  constructor(
    size: CompileBudgetSize = DEFAULT_COMPILE_BUDGET,
    nowMs: () => number = () => performance.now(),
  ) {
    if (!isCompileBudgetSize(size)) {
      throw new RangeError(
        `a compile budget is unlimited or a whole number from 1, not ${String(size)}`,
      );
    }
    this.#size = size === "unlimited" ? Number.POSITIVE_INFINITY : size;
    this.#nowMs = nowMs;
  }

  /** The compiles running now. */
  get running(): number {
    return this.#running;
  }

  /** The compiles waiting for a place. */
  get waiting(): number {
    return this.#draws.length + this.#ahead.length;
  }

  /** The most compiles that have run at once. */
  get mostRunning(): number {
    return this.#most;
  }

  /**
   * Tells `listener` of each compile as it settles, from now on, until the returned function is
   * called.
   */
  onCompile(listener: (event: CompileEvent) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /**
   * Runs `compile`, the pipeline of `name`, once a place is free, in `lane`'s order, and settles as
   * it settles.
   *
   * @remarks
   * `compile` is called at most once, and not before its turn; its rejection is the result's. A
   * compile that resolves to `null` made nothing (its turn found the pipeline made) and is not
   * heard ({@link CompileBudget.onCompile}); every other is, failed or not.
   */
  run<T>(name: string, compile: () => Promise<T>, lane: CompileLane = "ahead"): Promise<T> {
    const askedMs = this.#nowMs();
    return new Promise<T>((resolve, reject) => {
      const start = (): void => {
        this.#running += 1;
        this.#most = Math.max(this.#most, this.#running);
        const startMs = this.#nowMs();
        let compiling: Promise<T>;
        try {
          compiling = compile();
        } catch (error: unknown) {
          compiling = Promise.reject(error);
        }
        // Its caller hears first, so that a listener's throw cannot hold its pipeline back.
        const heard = (made: boolean, readyMs: number): void => {
          if (made) {
            this.#heard({ name, lane, waitedMs: startMs - askedMs, ms: readyMs - startMs });
          }
        };
        void compiling
          .then(
            (result): void => {
              const readyMs = this.#nowMs();
              resolve(result);
              heard(result !== null, readyMs);
              return undefined;
            },
            (error: unknown): void => {
              const readyMs = this.#nowMs();
              reject(error);
              heard(true, readyMs);
              return undefined;
            },
          )
          .finally(() => {
            this.#running -= 1;
            this.#startWaiting();
          })
          .catch((error: unknown) => {
            // The compile's own outcome went to its caller; only the budget's step or a listener
            // can land here.
            console.error("the compile budget could not go on:", error);
          });
      };
      (lane === "draw" ? this.#draws : this.#ahead).push(start);
      this.#startWaiting();
    });
  }

  /** Tells no listener of its compiles from now on: its engine's disposal. */
  clearListeners(): void {
    this.#listeners.clear();
  }

  /** Tells the listeners of a settled compile. */
  #heard(event: CompileEvent): void {
    for (const listener of this.#listeners) {
      listener(event);
    }
  }

  /** Starts the waiting compiles, the draws' first, while places are free. */
  #startWaiting(): void {
    while (this.#running < this.#size) {
      const next = this.#draws.shift() ?? this.#ahead.shift();
      if (next === undefined) {
        return;
      }
      next();
    }
  }
}
