import { describe, expect, it } from "vitest";

import { overlappedPasses, serialPasses, timedPass } from "../../test/passTimes";
import { passOwnNs, passUnionNs } from "./passSpans";

/**
 * Resolve `n` of a sweep: 1 to 9 spans of whole ns from a fixed pattern, nested, staggered,
 * touching and apart.
 */
function overlappingResolve(n: number): ReturnType<typeof timedPass>[] {
  const passes: ReturnType<typeof timedPass>[] = [];
  for (let k = 0; k < 1 + (n % 9); k += 1) {
    passes.push(
      timedPass(`pass ${String(k)}`, 1 + ((n * 37 + k * 101) % 5_000), (n * 13 + k * 977) % 6_000),
    );
  }
  return passes;
}

describe("a resolve's pass spans", () => {
  it("count the composite's wait on the march once, as the M5 stamps them", () => {
    const passes = overlappedPasses(3_000_000, 12_000_000, 500_000);
    expect([
      passes.reduce((sum, pass) => sum + pass.ns, 0),
      passUnionNs(passes),
      passOwnNs(passes),
    ]).toEqual([27_500_000, 15_500_000, [3_000_000, 12_000_000, 500_000]]);
  });

  it("give each pass its span as its own time on serial stamps, their union their sum", () => {
    const passes = serialPasses([
      ["sky", 65_536],
      ["discs", 131_072],
      ["tonemap", 1_000_000],
    ]);
    expect([passOwnNs(passes), passUnionNs(passes)]).toEqual([
      [65_536, 131_072, 1_000_000],
      1_196_608,
    ]);
  });

  it("take the passes in their stamps' order, not the order they were encoded in", () => {
    const passes = [timedPass("late", 2_000, 5_000), timedPass("early", 6_000, 0)];
    expect([passOwnNs(passes), passUnionNs(passes)]).toEqual([[1_000, 6_000], 7_000]);
  });

  it("give the march its dispatches' time where the composite's span holds them all, the composite only its tail", () => {
    // The composite is stamped from before the first dispatch begins, waiting on all of them.
    const passes = [
      timedPass("bodies", 3e6),
      timedPass("atmosphere view", 1e6, 3e6),
      timedPass("atmosphere view", 10e6, 4e6),
      timedPass("atmosphere behind", 1e6, 14e6),
      timedPass("atmosphere composite", 12.501e6, 2.999e6),
    ];
    expect([passOwnNs(passes), passUnionNs(passes)]).toEqual([
      [3e6, 1e6, 10e6, 1e6, 0.5e6],
      15.5e6,
    ]);
  });

  it("give a pass within another's span its own time, and the other the rest of its span", () => {
    const passes = [timedPass("outer", 10_000, 0), timedPass("inner", 2_000, 3_000)];
    expect([passOwnNs(passes), passUnionNs(passes)]).toEqual([[8_000, 2_000], 10_000]);
  });

  it("sum each resolve's own times to its union, however its spans overlap", () => {
    const resolves = Array.from({ length: 300 }, (_, n) => overlappingResolve(n));
    expect(
      resolves.filter(
        (passes) => passOwnNs(passes).reduce((sum, ns) => sum + ns, 0) !== passUnionNs(passes),
      ),
    ).toEqual([]);
  });

  it("skip a span whose end precedes its begin, an empty one and one not finite", () => {
    const passes = [
      timedPass("reset", -4_000, 9_000),
      timedPass("empty", 0, 2_000),
      { ...timedPass("unread", 1_000, 0), endNs: Number.NaN },
      timedPass("kept", 3_000, 1_000),
    ];
    expect([passOwnNs(passes), passUnionNs(passes)]).toEqual([[0, 0, 0, 3_000], 3_000]);
  });

  it("take no time from a resolve of no passes", () => {
    expect([passOwnNs([]), passUnionNs([])]).toEqual([[], 0]);
  });
});
