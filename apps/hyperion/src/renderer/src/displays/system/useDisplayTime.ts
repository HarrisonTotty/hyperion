import type { UniverseTime } from "@hyperion/protocol";
import { useCallback, useEffect, useRef, useState } from "react";

import { createRedrawScheduler, type RedrawScheduler } from "../../spatial/redraw";
import {
  type ClockLimit,
  clockLimitAt,
  DEFAULT_TIME_STEP,
  stepTime,
  TIME_STEPS,
} from "./displayTime";

/** The display time and the ways to move it. */
export interface DisplayTime {
  /** The display's own universe time. */
  readonly time: UniverseTime;
  /** The time the display was opened at, to which `RESET` returns. */
  readonly openedAt: UniverseTime;
  /** The index in `TIME_STEPS` of the step chosen. */
  readonly stepIndex: number;
  /** The edge of the clock window the time stands at, if either. */
  readonly limit: ClockLimit;
  /** Chooses the step the time moves by. */
  readonly chooseStep: (index: number) => void;
  /**
   * Moves the time one chosen step back (`-1`) or ahead (`+1`) on the next frame, with any other
   * steps asked for before it, held to the clock window.
   */
  readonly step: (direction: -1 | 1) => void;
  /** Returns the time to the opening time on the next frame. */
  readonly reset: () => void;
}

/** What is to happen on the next frame: steps to add, or a return to the opening time first. */
interface Pending {
  readonly reset: boolean;
  readonly steps: ReadonlyArray<number>;
}

const NOTHING_PENDING: Pending = { reset: false, steps: [] };

/**
 * Owns a display's time, which opens held at `openedAt` and moves only when the operator steps it
 * (plan 14, D24 and P14.T44.a).
 *
 * @remarks
 * A step changes what the display shows and nothing on the ship. Steps and `RESET` are gathered
 * through plan 05's redraw scheduler into one state update on the next frame, so a held key that
 * repeats faster than frames moves the time once a frame and redraws the map once, and nothing
 * moves between steps: there is no loop and no transition. Each step is applied in turn, held to
 * the clock window, so that stepping against `+H` and back again lands where the operator expects.
 * The scheduler is disposed when the display is hidden or unmounted, which drops a step not yet
 * applied.
 */
export function useDisplayTime(openedAt: UniverseTime): DisplayTime {
  const [time, setTime] = useState(openedAt);
  const [stepIndex, setStepIndex] = useState(DEFAULT_TIME_STEP);
  const pendingRef = useRef<Pending>(NOTHING_PENDING);
  const schedulerRef = useRef<RedrawScheduler | null>(null);

  useEffect(() => {
    const scheduler = createRedrawScheduler(
      (callback) => window.requestAnimationFrame(callback),
      (handle) => {
        window.cancelAnimationFrame(handle);
      },
    );
    schedulerRef.current = scheduler;
    return () => {
      scheduler.dispose();
      schedulerRef.current = null;
      pendingRef.current = NOTHING_PENDING;
    };
  }, []);

  const apply = useCallback((): void => {
    const { reset, steps } = pendingRef.current;
    pendingRef.current = NOTHING_PENDING;
    setTime((previous) => {
      const next = steps.reduce(
        (current, deltaS) => stepTime(current, deltaS),
        reset ? openedAt : previous,
      );
      // A step against the window's edge, or a reset where the time already is, keeps the state
      // itself, so that nothing is drawn again.
      return next.seconds === previous.seconds && next.nanos === previous.nanos ? previous : next;
    });
  }, [openedAt]);

  const step = useCallback(
    (direction: -1 | 1): void => {
      const chosen = TIME_STEPS[stepIndex];
      const scheduler = schedulerRef.current;
      if (chosen === undefined || scheduler === null) {
        return;
      }
      const pending = pendingRef.current;
      pendingRef.current = { ...pending, steps: [...pending.steps, direction * chosen.seconds] };
      scheduler.request(apply);
    },
    [stepIndex, apply],
  );

  const reset = useCallback((): void => {
    const scheduler = schedulerRef.current;
    if (scheduler === null) {
      return;
    }
    pendingRef.current = { reset: true, steps: [] };
    scheduler.request(apply);
  }, [apply]);

  return {
    time,
    openedAt,
    stepIndex,
    limit: clockLimitAt(time),
    chooseStep: setStepIndex,
    step,
    reset,
  };
}
