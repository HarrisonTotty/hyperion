import { useLayoutEffect, useState } from "react";

/** An element's size as laid out, with what it takes to draw at that size. */
export interface ElementSize {
  /** The element's width as laid out, in CSS pixels, fractions kept. */
  readonly widthPx: number;
  /** The element's height as laid out, in CSS pixels, fractions kept. */
  readonly heightPx: number;
  /** Device pixels in one CSS pixel. */
  readonly devicePixelRatio: number;
  /** CSS pixels in one `rem` at the current interface scale. */
  readonly remPx: number;
}

/** The browser's default root font size, used when none can be read. */
const DEFAULT_REM_PX = 16;

function rootRemPx(): number {
  const remPx = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(remPx) && remPx > 0 ? remPx : DEFAULT_REM_PX;
}

function sameSize(a: ElementSize | null, b: ElementSize): boolean {
  return (
    a !== null &&
    a.widthPx === b.widthPx &&
    a.heightPx === b.heightPx &&
    a.devicePixelRatio === b.devicePixelRatio &&
    a.remPx === b.remPx
  );
}

/**
 * Measures an element as its layout, the device pixel ratio and the interface scale change.
 *
 * @remarks
 * Measured when the element mounts, on each `ResizeObserver` callback and on each window resize,
 * which is also what a change of interface scale fires. Sizes come from `getBoundingClientRect`,
 * so the observer's entries are not needed, and keep the fractions that `clientWidth` rounds away:
 * two grid rows laid out 2:1 measure 2:1. Meant for elements without borders or transforms. An
 * unchanged size keeps its identity, so a consumer re-renders only when something changed. An
 * element measured 0 × 0 is not laid out, as when it is hidden, and keeps the size it last had, so
 * that what depends on it, such as the resolution of a map, survives a page being hidden.
 *
 * @returns A callback ref for the element, and its size once measured, or `null` before.
 */
export function useElementSize(): {
  readonly ref: (element: HTMLElement | null) => void;
  readonly size: ElementSize | null;
} {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const [size, setSize] = useState<ElementSize | null>(null);
  useLayoutEffect(() => {
    if (element === null) {
      return undefined;
    }
    const update = (): void => {
      const box = element.getBoundingClientRect();
      if (box.width === 0 && box.height === 0) {
        // Not laid out: hidden, as a page behind its tab is. It keeps the size it had.
        return;
      }
      const measured: ElementSize = {
        widthPx: box.width,
        heightPx: box.height,
        devicePixelRatio: window.devicePixelRatio,
        remPx: rootRemPx(),
      };
      setSize((previous) => (sameSize(previous, measured) ? previous : measured));
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    window.addEventListener("resize", update);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", update);
    };
  }, [element]);
  return { ref: setElement, size };
}
