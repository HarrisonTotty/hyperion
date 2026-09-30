import { useEffect, useState } from "react";

/** What a throttled value shows, and whether a change shown lately holds the next back. */
interface Throttle<T> {
  readonly shown: T;
  /** Whether a change was shown less than the interval ago, so that the next must wait. */
  readonly holding: boolean;
  /** How many holds there have been, so that each is timed afresh. */
  readonly holds: number;
}

/**
 * A value as a readout should show it: changed at most once per `intervalMs`, so that it can be
 * read however fast the value moves (the guide's "about 4 Hz").
 *
 * @remarks
 * A throttle with both edges. The first value is shown at once, and so is the first change after
 * a quiet interval; the changes that follow it within `intervalMs` are held, and the latest of them
 * is shown when the interval ends, so the readout always comes to rest on the final value. Values
 * are compared with `Object.is`, so a value that keeps its identity starts nothing. A hold is one
 * timeout: nothing runs while the value is still.
 *
 * @param intervalMs - The shortest time between two changes of what is shown, in milliseconds.
 */
export function useThrottledValue<T>(value: T, intervalMs: number): T {
  const [throttle, setThrottle] = useState<Throttle<T>>({ shown: value, holding: false, holds: 0 });

  // Outside a hold a change is shown at once, and starts a hold: the state is adjusted during
  // render, as React allows for state that follows a prop, and the render is redone before
  // anything is committed.
  if (!throttle.holding && !Object.is(value, throttle.shown)) {
    setThrottle({ shown: value, holding: true, holds: throttle.holds + 1 });
  }

  // Each hold is timed on its own, so that changes during it do not put its end off; a hold that
  // follows another at once is a new one, and is timed again. When it ends, the render above shows
  // the latest value, if it differs, and holds again.
  useEffect(() => {
    if (!throttle.holding) {
      return undefined;
    }
    const hold = throttle.holds;
    const timer = window.setTimeout(() => {
      // Ends this hold alone: a later one is timed by its own timeout.
      setThrottle((current) => (current.holds === hold ? { ...current, holding: false } : current));
    }, intervalMs);
    return () => {
      window.clearTimeout(timer);
    };
  }, [throttle.holding, throttle.holds, intervalMs]);

  return throttle.shown;
}
