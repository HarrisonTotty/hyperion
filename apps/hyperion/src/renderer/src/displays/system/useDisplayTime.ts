import type { UniverseTime } from "@hyperion/protocol";
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";

import { usePrefersReducedMotion } from "../../lib/usePrefersReducedMotion";
import { createRedrawScheduler, type RedrawScheduler } from "../../spatial/redraw";
import {
  type ClockLimit,
  clockLimitAt,
  DEFAULT_RUN_RATE,
  DEFAULT_TIME_STEP,
  READOUT_PERIOD_MS,
  RUN_RATES,
  stepTime,
  TIME_STEPS,
} from "./displayTime";

/**
 * The display time as it stands on each frame while the display runs, for the one component that
 * draws at frame rate (the orbit map): an external store, so that nothing else renders per frame.
 */
export interface FrameTime {
  /** Registers a listener for each new frame's time; returns what removes it. */
  readonly subscribe: (onChange: () => void) => () => void;
  /** The time of the latest frame while running, or `null` while held. */
  readonly getSnapshot: () => UniverseTime | null;
}

/** The display time and the ways to move it. */
export interface DisplayTime {
  /**
   * The display's own universe time: held, stepped, or while running the time at the latest
   * quarter second, which every numeric readout and every request reads (the guide's 4 Hz).
   */
  readonly time: UniverseTime;
  /** The time at every frame while running, which the orbit map draws. */
  readonly frameTime: FrameTime;
  /** The time the display was opened at, to which `RESET` returns. */
  readonly openedAt: UniverseTime;
  /** The index in `TIME_STEPS` of the step chosen. */
  readonly stepIndex: number;
  /** The index in `RUN_RATES` of the rate chosen. */
  readonly rateIndex: number;
  /** Whether the display time is running, rather than held. */
  readonly running: boolean;
  /** Why `RUN` is held back, other than the clock window's edge; `null` when it is not. */
  readonly runHeldBy: string | null;
  /** The edge of the clock window the time stands at, if either. */
  readonly limit: ClockLimit;
  /** Chooses the step the time moves by. */
  readonly chooseStep: (index: number) => void;
  /** Chooses the rate `RUN` advances the time at, at once if it is running. */
  readonly chooseRate: (index: number) => void;
  /**
   * Moves the time one chosen step back (`-1`) or ahead (`+1`) on the next frame, with any other
   * steps asked for before it, held to the clock window; a running display is held first.
   */
  readonly step: (direction: -1 | 1) => void;
  /** Returns the time to the opening time on the next frame; a running display is held first. */
  readonly reset: () => void;
  /** Starts the display time advancing at the chosen rate (`RUN`). */
  readonly run: () => void;
  /** Stops it where it stands (`HOLD`). */
  readonly hold: () => void;
}

/** What is to happen on the next frame: steps to add, or a return to the opening time first. */
interface Pending {
  readonly reset: boolean;
  readonly steps: ReadonlyArray<number>;
}

const NOTHING_PENDING: Pending = { reset: false, steps: [] };

/** A run in progress: the time it counts from, and its rate. */
interface Run {
  readonly from: UniverseTime;
  readonly rateIndex: number;
}

/** The frame-time store, and where the run stands for the commands that stop or re-rate it. */
interface RunClock extends FrameTime {
  /** Sets the time the orbit map draws, and tells it; `null` hands it back to the held time. */
  readonly publish: (time: UniverseTime | null) => void;
  /** The latest time the run reached, readout or frame, or `null` with no run. */
  readonly latest: () => UniverseTime | null;
  /** Records the latest time the run reached; `null` once it has stopped. */
  readonly reach: (time: UniverseTime | null) => void;
}

function createRunClock(): RunClock {
  let frame: UniverseTime | null = null;
  let latest: UniverseTime | null = null;
  const listeners = new Set<() => void>();
  return {
    subscribe(onChange) {
      listeners.add(onChange);
      return () => {
        listeners.delete(onChange);
      };
    },
    getSnapshot: () => frame,
    publish(time) {
      if (time === frame) {
        return;
      }
      frame = time;
      for (const listener of listeners) {
        listener();
      }
    },
    latest: () => latest,
    reach(time) {
      latest = time;
    },
  };
}

/**
 * The frame-rate display time while running, or `null` while held: the orbit map's subscription,
 * so that the map alone renders per frame.
 */
export function useFrameTime(frameTime: FrameTime): UniverseTime | null {
  return useSyncExternalStore(frameTime.subscribe, frameTime.getSnapshot);
}

/**
 * Owns a display's time, which opens held at `openedAt` and moves only when the operator steps it
 * or runs it (plan 14, D24 and P14.T44.a–b).
 *
 * @remarks
 * A step changes what the display shows and nothing on the ship. Steps and `RESET` are gathered
 * through plan 05's redraw scheduler into one state update on the next frame, so a held key that
 * repeats faster than frames moves the time once a frame and redraws the map once, and nothing
 * moves between steps: there is no loop and no transition. Each step is applied in turn, held to
 * the clock window, so that stepping against `+H` and back again lands where the operator expects.
 * The scheduler is disposed when the display is hidden or unmounted, which drops a step not yet
 * applied.
 *
 * `RUN` advances the time at the chosen rate, from where it stands, and `HOLD` stops it there. The
 * display opens held and never runs on its own. While it runs, one animation frame at a time asks
 * for the next: each frame's time, whole seconds counted from the run's start by the frame's own
 * timestamp, goes to the orbit map alone through {@link FrameTime}, and the display time that the
 * readouts and the requests read moves once a quarter second, the guide's 4 Hz, without tweening.
 * Under `prefers-reduced-motion` no frame is asked for: the time steps every quarter second, a
 * quarter of a second's worth at a time, and each step is one paint. A run drops to `HOLD` at `+H`,
 * when `runHeldBy` names a reason (the link lost), and when the display is hidden or unmounted; a
 * step or `RESET` holds it first, so that the operator's step lands where the map stands. Holding
 * keeps the time the run reached, the frame's where the map had drawn one. Nothing is left running:
 * each run's frame or timeout is released when it stops.
 *
 * @param runHeldBy - Why the display may not run, in the console's words (`NO CARRIER`), or `null`.
 */
export function useDisplayTime(openedAt: UniverseTime, runHeldBy: string | null): DisplayTime {
  const [time, setTime] = useState(openedAt);
  const [stepIndex, setStepIndex] = useState(DEFAULT_TIME_STEP);
  const [rateIndex, setRateIndex] = useState(DEFAULT_RUN_RATE);
  const [run, setRun] = useState<Run | null>(null);
  const [clock] = useState(createRunClock);
  const reducedMotion = usePrefersReducedMotion();
  const pendingRef = useRef<Pending>(NOTHING_PENDING);
  const schedulerRef = useRef<RedrawScheduler | null>(null);

  // A reason to hold, as the link lost, stops a run in the render it arrives in.
  if (runHeldBy !== null && run !== null) {
    setRun(null);
  }

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
      // Leaving the display, hidden or unmounted, holds it: it never runs on its own on return.
      setRun(null);
    };
  }, []);

  useEffect(() => {
    const rate = RUN_RATES[run?.rateIndex ?? -1];
    if (run === null || rate === undefined) {
      return undefined;
    }
    const { from } = run;
    clock.reach(from);
    const reach = (next: UniverseTime): boolean => {
      clock.reach(next);
      if (clockLimitAt(next) !== "end") {
        return false;
      }
      setTime(next);
      setRun(null);
      return true;
    };

    let stop: () => void;
    if (reducedMotion) {
      const secondsPerTick = (rate.secondsPerSecond * READOUT_PERIOD_MS) / 1_000;
      let ticks = 0;
      let timeout: number | null = null;
      const onTick = (): void => {
        timeout = null;
        ticks += 1;
        const next = stepTime(from, ticks * secondsPerTick);
        if (reach(next)) {
          return;
        }
        setTime(next);
        timeout = window.setTimeout(onTick, READOUT_PERIOD_MS);
      };
      timeout = window.setTimeout(onTick, READOUT_PERIOD_MS);
      stop = () => {
        if (timeout !== null) {
          window.clearTimeout(timeout);
        }
      };
    } else {
      let startMs: number | null = null;
      let readout = 0;
      let handle: number | null = null;
      const onFrame = (nowMs: number): void => {
        handle = null;
        startMs ??= nowMs;
        const elapsedMs = Math.max(0, nowMs - startMs);
        const next = stepTime(from, Math.floor((rate.secondsPerSecond * elapsedMs) / 1_000));
        const drawn = clock.getSnapshot();
        if (drawn === null || drawn.seconds !== next.seconds) {
          clock.publish(next);
        }
        if (reach(next)) {
          return;
        }
        const quarter = Math.floor(elapsedMs / READOUT_PERIOD_MS);
        if (quarter !== readout) {
          readout = quarter;
          setTime(next);
        }
        handle = window.requestAnimationFrame(onFrame);
      };
      handle = window.requestAnimationFrame(onFrame);
      stop = () => {
        if (handle !== null) {
          window.cancelAnimationFrame(handle);
        }
      };
    }

    return () => {
      stop();
      const reached = clock.latest() ?? from;
      clock.reach(null);
      clock.publish(null);
      setTime(reached);
      // A run restarted by another cause than a command, as a change of the motion setting, goes
      // on from where it reached rather than from where it began.
      setRun((current) => (current === run ? { ...current, from: reached } : current));
    };
  }, [run, reducedMotion, clock]);

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
      setRun(null);
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
    setRun(null);
    pendingRef.current = { reset: true, steps: [] };
    scheduler.request(apply);
  }, [apply]);

  const limit = clockLimitAt(time);
  // The time a run starts from, read when `RUN` is pressed rather than a dependency of the command,
  // so that the command, and the key listener that calls it, keep their identity while the time
  // moves four times a second.
  const timeRef = useRef(time);
  useEffect(() => {
    timeRef.current = time;
  }, [time]);
  const startRun = useCallback((): void => {
    if (runHeldBy !== null || limit === "end") {
      return;
    }
    const from = timeRef.current;
    setRun((current) => current ?? { from, rateIndex });
  }, [runHeldBy, limit, rateIndex]);

  const hold = useCallback((): void => {
    setRun(null);
  }, []);

  const chooseRate = useCallback(
    (index: number): void => {
      setRateIndex(index);
      const reached = clock.latest();
      setRun((current) =>
        current === null || current.rateIndex === index
          ? current
          : { from: reached ?? current.from, rateIndex: index },
      );
    },
    [clock],
  );

  return {
    time,
    frameTime: clock,
    openedAt,
    stepIndex,
    rateIndex,
    running: run !== null,
    runHeldBy,
    limit,
    chooseStep: setStepIndex,
    chooseRate,
    step,
    reset,
    run: startRun,
    hold,
  };
}
