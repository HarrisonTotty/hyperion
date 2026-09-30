import { useLayoutEffect, useState } from "react";

/** Where a scrolling list stands, as measured from the DOM. */
export interface ScrollMetrics {
  /** The list's scroll offset. */
  readonly scrollTopPx: number;
  /** The height of the list's visible area. */
  readonly viewportPx: number;
  /** Pixels in one `rem` at the current interface scale. */
  readonly remPx: number;
}

/** The browser's default root font size, used when none can be read. */
const DEFAULT_REM_PX = 16;

const UNMEASURED: ScrollMetrics = { scrollTopPx: 0, viewportPx: 0, remPx: DEFAULT_REM_PX };

function rootRemPx(): number {
  const remPx = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(remPx) && remPx > 0 ? remPx : DEFAULT_REM_PX;
}

function measure(element: HTMLElement): ScrollMetrics {
  return { scrollTopPx: element.scrollTop, viewportPx: element.clientHeight, remPx: rootRemPx() };
}

function sameMetrics(a: ScrollMetrics, b: ScrollMetrics): boolean {
  return a.scrollTopPx === b.scrollTopPx && a.viewportPx === b.viewportPx && a.remPx === b.remPx;
}

/**
 * Measures a scrolling list's offset and visible height, for its position readout.
 *
 * @remarks
 * Rows are sized in `rem`, so the root font size is measured too, and the interface scale can
 * change without breaking the readout. Measured when the element mounts, when it scrolls, when it
 * is resized (by its own rows or its neighbours) and when the window resizes, which is also what a
 * change of interface scale does. Until the element
 * mounts the list reads as scrolled to the top with no height, which `windowRange` reports as its
 * first row.
 *
 * @returns A callback ref for the scrolling element, and its latest metrics.
 */
export function useScrollMetrics(): {
  readonly ref: (element: HTMLElement | null) => void;
  readonly metrics: ScrollMetrics;
} {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const [metrics, setMetrics] = useState<ScrollMetrics>(UNMEASURED);

  useLayoutEffect(() => {
    if (element === null) {
      return undefined;
    }
    const update = (): void => {
      const measured = measure(element);
      setMetrics((previous) => (sameMetrics(previous, measured) ? previous : measured));
    };
    update();
    // The list's height changes with its own rows and its neighbours' as well as with the window.
    const observer = new ResizeObserver(update);
    observer.observe(element);
    element.addEventListener("scroll", update, { passive: true });
    window.addEventListener("resize", update);
    return () => {
      observer.disconnect();
      element.removeEventListener("scroll", update);
      window.removeEventListener("resize", update);
    };
  }, [element]);

  return { ref: setElement, metrics };
}
