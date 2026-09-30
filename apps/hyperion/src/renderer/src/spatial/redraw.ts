/** Asks the browser for a frame, as `requestAnimationFrame` does. */
export type RequestFrame = (callback: (timeMs: number) => void) => number;

/** Cancels a frame asked for with a {@link RequestFrame}, as `cancelAnimationFrame` does. */
export type CancelFrame = (handle: number) => void;

/** Coalesces redraw requests into at most one frame at a time. */
export interface RedrawScheduler {
  /**
   * Asks for `callback` to run on the next frame. Requests made before the frame fires share it,
   * and only the latest callback runs.
   */
  request(callback: (timeMs: number) => void): void;
  /** Cancels a pending frame; later requests do nothing. */
  dispose(): void;
}

/**
 * A scheduler that redraws on demand and never on a loop.
 *
 * @remarks
 * Any number of requests before the frame fires produce one frame that runs the latest callback
 * once. With no request there is no frame: the scheduler never re-arms itself, so an idle view
 * costs nothing (the brainstorm's "redraws happen on demand"). The frame functions are injected so
 * that tests can drive frames by hand.
 */
export function createRedrawScheduler(
  requestFrame: RequestFrame,
  cancelFrame: CancelFrame,
): RedrawScheduler {
  let handle: number | null = null;
  let pending: ((timeMs: number) => void) | null = null;
  let disposed = false;

  const run = (timeMs: number): void => {
    handle = null;
    const callback = pending;
    pending = null;
    callback?.(timeMs);
  };

  return {
    request(callback) {
      if (disposed) {
        return;
      }
      pending = callback;
      handle ??= requestFrame(run);
    },
    dispose() {
      disposed = true;
      pending = null;
      if (handle !== null) {
        cancelFrame(handle);
        handle = null;
      }
    },
  };
}
