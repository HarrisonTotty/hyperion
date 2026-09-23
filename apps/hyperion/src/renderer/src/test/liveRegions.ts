/** The live status each role carries without an `aria-live` attribute, per WAI-ARIA 1.2. */
const IMPLICIT_LIVE: Readonly<Record<string, string>> = {
  alert: "assertive",
  log: "polite",
  marquee: "off",
  status: "polite",
  timer: "off",
};

/** What a region announces when it has no `aria-relevant`, per WAI-ARIA 1.2. */
const DEFAULT_RELEVANT = "additions text";

/**
 * Whether `element` is out of the accessibility tree, and so announces nothing: it or an ancestor
 * is `hidden`, `aria-hidden`, inert, or not displayed, as `Activity` hides a display with an inline
 * `display: none`.
 */
function outOfTree(element: HTMLElement): boolean {
  for (let each: HTMLElement | null = element; each !== null; each = each.parentElement) {
    if (
      each.hasAttribute("hidden") ||
      each.getAttribute("aria-hidden") === "true" ||
      each.hasAttribute("inert") ||
      each.style.display === "none"
    ) {
      return true;
    }
  }
  return false;
}

/**
 * The element whose live status decides whether a change to `node` is announced, or `null` where
 * nothing announces it: the nearest ancestor that carries one, which is where a screen reader's
 * event generator stops walking up, and `null` where that one is turned off, or where the node is
 * no longer in the document or is out of the accessibility tree.
 *
 * @remarks
 * An `output` is a `status` by itself, so it counts without a `role`; an explicit `aria-live`
 * overrides what a role implies.
 */
export function announcingRegion(node: Node): HTMLElement | null {
  const start = node instanceof HTMLElement ? node : node.parentElement;
  if (start === null || !start.isConnected || outOfTree(start)) {
    return null;
  }
  for (let element: HTMLElement | null = start; element !== null; element = element.parentElement) {
    const role = element.tagName === "OUTPUT" ? "status" : element.getAttribute("role");
    const live = element.getAttribute("aria-live") ?? (role === null ? null : IMPLICIT_LIVE[role]);
    if (live !== undefined && live !== null) {
      return live === "off" ? null : element;
    }
  }
  return null;
}

/** Whether a node added to the page can be heard: text, or an element in the accessibility tree. */
function audible(node: Node): boolean {
  return node instanceof HTMLElement ? !outOfTree(node) : node.nodeType === Node.TEXT_NODE;
}

/**
 * The region a change announces from, or `null`: a change of text, or nodes added, as a region
 * announces by default, and nodes removed only where its `aria-relevant` asks for removals.
 */
function regionAnnouncing(record: MutationRecord): HTMLElement | null {
  const region = announcingRegion(record.target);
  if (region === null) {
    return null;
  }
  const relevant = (region.getAttribute("aria-relevant") ?? DEFAULT_RELEVANT).split(/\s+/u);
  const all = relevant.includes("all");
  if (record.type === "characterData") {
    return all || relevant.includes("text") ? region : null;
  }
  const added = [...record.addedNodes].some(audible);
  const removed = record.removedNodes.length > 0;
  const announced =
    (added && (all || relevant.includes("additions") || relevant.includes("text"))) ||
    (removed && (all || relevant.includes("removals")));
  return announced ? region : null;
}

/** The regions a delivery of changes announces from, each once however many nodes changed. */
function regionsOf(records: ReadonlyArray<MutationRecord>): ReadonlyArray<HTMLElement> {
  const regions = new Set<HTMLElement>();
  for (const record of records) {
    const region = regionAnnouncing(record);
    if (region !== null) {
      regions.add(region);
    }
  }
  return [...regions];
}

/**
 * The announcements that `change` makes: the live region each comes from, once for every update of
 * the page in which the region changed, so that a key press which announces twice, from two regions
 * or from one region twice, can be told from one that announces once.
 *
 * @remarks
 * It watches the whole document for changes to text and to children, as a screen reader's live
 * region events are raised for; attributes are not watched, since none of the regions announces
 * one. An update is one delivery of changes to the observer, which takes every change made before
 * the next microtask checkpoint. Several changes to one region in one update are one announcement,
 * as a screen reader reads the region as it then stands. Deliveries come at least as often as a
 * screen reader can be told of a change, so the count errs high: two in one frame may be heard as
 * one, and a count of one is therefore a bound. Which regions a delivery announces from is decided
 * when it arrives, since a region hidden or removed later was heard all the same. Records still
 * waiting in the queue when `change` resolves are the last delivery.
 */
export async function announcements(
  change: () => Promise<void>,
): Promise<ReadonlyArray<HTMLElement>> {
  const announced: HTMLElement[] = [];
  const observer = new MutationObserver((records) => {
    announced.push(...regionsOf(records));
  });
  observer.observe(document.body, { subtree: true, childList: true, characterData: true });
  try {
    await change();
    announced.push(...regionsOf(observer.takeRecords()));
  } finally {
    // Also when the change throws: the suite shares its document across files (`isolate: false`).
    observer.disconnect();
  }
  return announced;
}
