/** The live status each role carries without an `aria-live` attribute, per WAI-ARIA 1.2. */
const IMPLICIT_LIVE: Readonly<Record<string, string>> = {
  alert: "assertive",
  log: "polite",
  marquee: "off",
  status: "polite",
  timer: "off",
};

/**
 * The element whose live status decides whether a change to `node` is announced, or `null` where
 * nothing announces it: the nearest ancestor that carries one, which is where a screen reader's
 * event generator stops walking up, and `null` where that one is turned off.
 *
 * @remarks
 * An `output` is a `status` by itself, so it counts without a `role`; an explicit `aria-live`
 * overrides what a role implies.
 */
export function announcingRegion(node: Node): HTMLElement | null {
  let element = node instanceof HTMLElement ? node : node.parentElement;
  while (element !== null) {
    const role = element.tagName === "OUTPUT" ? "status" : element.getAttribute("role");
    const live = element.getAttribute("aria-live") ?? (role === null ? null : IMPLICIT_LIVE[role]);
    if (live !== undefined && live !== null) {
      return live === "off" ? null : element;
    }
    element = element.parentElement;
  }
  return null;
}

/**
 * The live regions that `change` announces from, one entry per region however many of its nodes
 * changed, so that a key press which announces twice can be told from one that announces once.
 *
 * @remarks
 * It watches the whole document for changes to text and to children, as a screen reader's live
 * region events are raised for; attributes are not watched, since none of the regions announces
 * one. Records are delivered in a microtask, which `change` may await, or still wait in the queue
 * when it resolves, so both are taken.
 */
export async function announcements(
  change: () => Promise<void>,
): Promise<ReadonlyArray<HTMLElement>> {
  const changed: MutationRecord[] = [];
  const observer = new MutationObserver((records) => changed.push(...records));
  observer.observe(document.body, { subtree: true, childList: true, characterData: true });
  try {
    await change();
    changed.push(...observer.takeRecords());
  } finally {
    // Also when the change throws: the suite shares its document across files (`isolate: false`).
    observer.disconnect();
  }
  const regions = new Set<HTMLElement>();
  for (const record of changed) {
    const region = announcingRegion(record.target);
    if (region !== null) {
      regions.add(region);
    }
  }
  return [...regions];
}
