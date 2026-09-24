import { useCallback, useLayoutEffect, useRef, useState } from "react";

/** Where one item of a scrolling region lies, in pixels from the top of the region's content. */
export interface ItemExtent {
  readonly topPx: number;
  readonly heightPx: number;
}

/** The items of a scrolling region at least partly in view, 0-based and inclusive. */
export interface ItemsInView {
  readonly firstVisible: number;
  readonly lastVisible: number;
  readonly total: number;
}

// Keeps an item whose edge lands a hair inside the viewport's edge, at fractional interface scales,
// from counting as visible.
const EDGE_TOLERANCE_PX = 0.5;

/**
 * Works out which items of a scrolling region of items of any height are in view, for the guide's
 * position readout (`12-24 of 87`).
 *
 * @remarks
 * The counterpart of `windowRange` for a region whose items are not one height, such as a readout
 * whose readings stand two to a line with some a line of their own. An item is in view when any of
 * it is; an item of no height, when its top is. A viewport of no height, or one that falls between
 * items, still names the first item at or below its top, so that the readout always names one.
 *
 * @param items - The items, in document order.
 * @param scrollTopPx - The region's scroll offset.
 * @param viewportPx - The height of the region's visible area.
 * @returns The range, or `null` when there are no items.
 */
export function itemsInView(
  items: ReadonlyArray<ItemExtent>,
  scrollTopPx: number,
  viewportPx: number,
): ItemsInView | null {
  if (items.length === 0) {
    return null;
  }
  const startPx = scrollTopPx;
  const endPx = scrollTopPx + Math.max(0, viewportPx);
  let firstVisible = -1;
  let lastVisible = -1;
  for (const [index, item] of items.entries()) {
    const bottomPx = item.topPx + Math.max(0, item.heightPx);
    const visible =
      item.heightPx > 0
        ? bottomPx > startPx + EDGE_TOLERANCE_PX && item.topPx < endPx - EDGE_TOLERANCE_PX
        : item.topPx >= startPx - EDGE_TOLERANCE_PX && item.topPx <= endPx + EDGE_TOLERANCE_PX;
    if (visible) {
      if (firstVisible < 0) {
        firstVisible = index;
      }
      lastVisible = index;
    }
  }
  if (firstVisible < 0) {
    const below = items.findIndex((item) => item.topPx >= startPx);
    firstVisible = below < 0 ? items.length - 1 : below;
    lastVisible = firstVisible;
  }
  return { firstVisible, lastVisible, total: items.length };
}

function sameRange(a: ItemsInView | null, b: ItemsInView | null): boolean {
  return (
    a === b ||
    (a !== null &&
      b !== null &&
      a.firstVisible === b.firstVisible &&
      a.lastVisible === b.lastVisible &&
      a.total === b.total)
  );
}

/**
 * Measures which items of a scrolling region are in view, for its position readout.
 *
 * @remarks
 * The items are the region's descendants that match `selector`, measured by their offsets, so the
 * region must be their offset parent (`position: relative`). Measured when the region mounts, when
 * it scrolls, when it is resized and when the window is, and whenever `contentKey` changes, which
 * the caller changes with what the region holds. Whenever `topKey` changes the region first returns
 * to its top, as for a new subject, and is then measured there.
 *
 * @param selector - Selects the items among the region's descendants, such as `dt`.
 * @param contentKey - Changes whenever the region's items may have changed.
 * @param topKey - Changes whenever the region should return to its top.
 * @returns A callback ref for the scrolling element, and the items in view.
 */
export function useItemsInView(
  selector: string,
  contentKey: string,
  topKey: string,
): {
  readonly ref: (element: HTMLElement | null) => void;
  readonly range: ItemsInView | null;
} {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const [range, setRange] = useState<ItemsInView | null>(null);
  // The element as the browser holds it, whose scroll offset is the browser's state, not React's.
  const elementRef = useRef<HTMLElement | null>(null);
  const shownTopKey = useRef<string | null>(null);
  const measuredContentKey = useRef<string | null>(null);
  const ref = useCallback((node: HTMLElement | null): void => {
    elementRef.current = node;
    setElement(node);
  }, []);

  useLayoutEffect(() => {
    const node = elementRef.current;
    if (element === null || node === null) {
      return undefined;
    }
    if (shownTopKey.current !== topKey) {
      shownTopKey.current = topKey;
      node.scrollTop = 0;
    }
    // Measured afresh for new content as well as on the events below.
    measuredContentKey.current = contentKey;
    const update = (): void => {
      const items = [...node.querySelectorAll<HTMLElement>(selector)].map((item) => ({
        topPx: item.offsetTop,
        heightPx: item.offsetHeight,
      }));
      const measured = itemsInView(items, node.scrollTop, node.clientHeight);
      setRange((previous) => (sameRange(previous, measured) ? previous : measured));
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    node.addEventListener("scroll", update, { passive: true });
    window.addEventListener("resize", update);
    return () => {
      observer.disconnect();
      node.removeEventListener("scroll", update);
      window.removeEventListener("resize", update);
    };
  }, [element, selector, contentKey, topKey]);

  return { ref, range };
}
