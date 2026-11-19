/**
 * Pass times as a serial timer stamps them, for fakes and tests (lane fix-mac-gpu-time): each pass
 * beginning where the one before it ended, so that the union of their spans is their sum.
 */

import type { PassTimes } from "../view/engine/types";

/** One pass's times: its span `ns` long from `beginNs`, ns from its resolve's earliest stamp. */
export function timedPass(label: string, ns: number, beginNs = 0): PassTimes["passes"][number] {
  return { label, ns, beginNs, endNs: beginNs + ns, bracketed: false };
}

/** A resolve's passes, each a label and its time, ns, laid end to end from 0 in their order. */
export function serialPasses(
  passes: ReadonlyArray<readonly [label: string, ns: number]>,
): PassTimes["passes"] {
  let atNs = 0;
  return passes.map(([label, ns]) => {
    const pass = timedPass(label, ns, atNs);
    atNs = Math.max(atNs, pass.endNs);
    return pass;
  });
}

/**
 * A resolve's passes shaped as the owner's M5 stamps a photorealistic frame (the 2026-11-08 Mac
 * batch): `restNs` of other passes, then the atmosphere's march, `marchNs`, and its composite,
 * whose span begins with the march's, holding its wait on it, and ends `compositeOwnNs` after it.
 * Their spans sum to `restNs + 2 marchNs + compositeOwnNs`; their union is `marchNs` less.
 */
export function overlappedPasses(
  restNs: number,
  marchNs: number,
  compositeOwnNs: number,
): PassTimes["passes"] {
  return [
    timedPass("bodies", restNs),
    timedPass("atmosphere view", marchNs, restNs),
    timedPass("atmosphere composite", marchNs + compositeOwnNs, restNs),
  ];
}
