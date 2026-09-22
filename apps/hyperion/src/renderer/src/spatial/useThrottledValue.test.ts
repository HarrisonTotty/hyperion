import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useThrottledValue } from "./useThrottledValue";

function setup(initial: number) {
  return renderHook(({ value }) => useThrottledValue(value, 250), {
    initialProps: { value: initial },
  });
}

/** Lets `ms` of fake time pass. */
function wait(ms: number): void {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

describe("useThrottledValue", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows the first value at once", () => {
    const { result } = setup(30);

    expect(result.current).toBe(30);
  });

  it("shows the first change after a quiet interval at once", () => {
    const { result, rerender } = setup(30);

    rerender({ value: 35 });

    expect(result.current).toBe(35);
  });

  it("holds the changes that follow within 250 ms", () => {
    const { result, rerender } = setup(30);
    rerender({ value: 35 });

    wait(100);
    rerender({ value: 40 });
    wait(100);
    rerender({ value: 45 });
    wait(49);

    expect(result.current).toBe(35);
  });

  it("shows the latest value when the interval ends", () => {
    const { result, rerender } = setup(30);
    rerender({ value: 35 });
    rerender({ value: 40 });
    rerender({ value: 45 });

    wait(250);

    expect(result.current).toBe(45);
  });

  it("changes at most once in each 250 ms while the value keeps moving", () => {
    const { result, rerender } = setup(0);
    const shown: number[] = [result.current];

    // A new value every 10 ms for a second.
    for (let step = 1; step <= 100; step += 1) {
      rerender({ value: step });
      wait(10);
      if (shown.at(-1) !== result.current) {
        shown.push(result.current);
      }
    }

    // The first change at once, then the latest value every 250 ms.
    expect(shown).toEqual([0, 1, 25, 50, 75, 100]);
  });

  it("starts no timer while the value keeps its identity", () => {
    const value = { azimuthDeg: 30 };
    const { rerender } = renderHook(({ current }) => useThrottledValue(current, 250), {
      initialProps: { current: value },
    });

    rerender({ current: value });

    expect(vi.getTimerCount()).toBe(0);
  });

  it("stops its timer when it is unmounted", () => {
    const { rerender, unmount } = setup(30);
    rerender({ value: 35 });
    expect(vi.getTimerCount()).toBe(1);

    unmount();

    expect(vi.getTimerCount()).toBe(0);
  });
});
