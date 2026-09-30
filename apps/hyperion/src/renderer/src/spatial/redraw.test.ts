import { describe, expect, it, vi } from "vitest";

import { createRedrawScheduler } from "./redraw";

/** Frame functions driven by hand, standing in for `requestAnimationFrame`. */
function fakeFrames(): {
  requestFrame: (callback: (timeMs: number) => void) => number;
  cancelFrame: (handle: number) => void;
  fire: (timeMs: number) => void;
  requested: () => number;
  pending: () => number;
} {
  let nextHandle = 1;
  let requested = 0;
  const waiting = new Map<number, (timeMs: number) => void>();
  return {
    requestFrame(callback) {
      requested += 1;
      const handle = nextHandle;
      nextHandle += 1;
      waiting.set(handle, callback);
      return handle;
    },
    cancelFrame(handle) {
      waiting.delete(handle);
    },
    fire(timeMs) {
      const callbacks = [...waiting.values()];
      waiting.clear();
      for (const callback of callbacks) {
        callback(timeMs);
      }
    },
    requested: () => requested,
    pending: () => waiting.size,
  };
}

describe("createRedrawScheduler", () => {
  it("runs three requests made before a frame as one frame with the latest callback", () => {
    const frames = fakeFrames();
    const scheduler = createRedrawScheduler(frames.requestFrame, frames.cancelFrame);
    const first = vi.fn<(timeMs: number) => void>();
    const second = vi.fn<(timeMs: number) => void>();
    const third = vi.fn<(timeMs: number) => void>();

    scheduler.request(first);
    scheduler.request(second);
    scheduler.request(third);
    frames.fire(16);

    expect(frames.requested()).toBe(1);
    expect(first).not.toHaveBeenCalled();
    expect(second).not.toHaveBeenCalled();
    expect(third).toHaveBeenCalledExactlyOnceWith(16);
  });

  it("asks for no frame without a request, even after a frame has run", () => {
    const frames = fakeFrames();
    const scheduler = createRedrawScheduler(frames.requestFrame, frames.cancelFrame);

    expect(frames.requested()).toBe(0);
    scheduler.request(() => {});
    frames.fire(16);
    frames.fire(32);

    expect(frames.requested()).toBe(1);
    expect(frames.pending()).toBe(0);
  });

  it("asks for a new frame when a callback requests another", () => {
    const frames = fakeFrames();
    const scheduler = createRedrawScheduler(frames.requestFrame, frames.cancelFrame);
    const times: number[] = [];
    const step = (timeMs: number): void => {
      times.push(timeMs);
      if (times.length < 3) {
        scheduler.request(step);
      }
    };

    scheduler.request(step);
    frames.fire(10);
    frames.fire(20);
    frames.fire(30);
    frames.fire(40);

    expect(times).toEqual([10, 20, 30]);
    expect(frames.requested()).toBe(3);
  });

  it("cancels a pending frame on dispose and ignores later requests", () => {
    const frames = fakeFrames();
    const scheduler = createRedrawScheduler(frames.requestFrame, frames.cancelFrame);
    const callback = vi.fn<(timeMs: number) => void>();

    scheduler.request(callback);
    scheduler.dispose();
    frames.fire(16);
    scheduler.request(callback);
    frames.fire(32);

    expect(callback).not.toHaveBeenCalled();
    expect(frames.pending()).toBe(0);
    expect(frames.requested()).toBe(1);
  });
});
