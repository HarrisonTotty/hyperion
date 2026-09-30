import { vi } from "vitest";

function advance(delayMs: number): void {
  vi.advanceTimersByTime(delayMs);
}

/**
 * Fakes animation frames and timeouts for the rest of the test, in a way that Testing Library and
 * `user-event` still run under.
 *
 * @remarks
 * After every `user-event` action Testing Library waits on a `setTimeout` of 0 ms, and advances a
 * fake clock only when it finds Jest's, through a global `jest`: under Vitest's faked `setTimeout`
 * that wait would never end. The stub hands it Vitest's clock under that name, and is removed after
 * the test by `unstubGlobals`. Call `vi.useRealTimers()` in `afterEach`.
 *
 * @returns A function that advances the fake clock, for `userEvent.setup({ advanceTimers })`, so
 *   that user-event's own delays run on it too.
 */
export function fakeFramesAndTimeouts(): (delayMs: number) => void {
  vi.useFakeTimers({
    toFake: ["requestAnimationFrame", "cancelAnimationFrame", "setTimeout", "clearTimeout"],
  });
  vi.stubGlobal("jest", { advanceTimersByTime: advance });
  return advance;
}
