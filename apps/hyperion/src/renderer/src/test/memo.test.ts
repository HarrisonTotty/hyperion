import { describe, expect, it } from "vitest";

import { memoised } from "./memo";

describe("memoised", () => {
  it("computes each key's result once, and hands every later caller that same result", () => {
    const asked: number[] = [];
    const squares = memoised(
      (n: number): ReadonlyArray<number> => {
        asked.push(n);
        return [n * n];
      },
      (n) => String(n),
    );
    const first = squares(3);
    expect([first, squares(3), squares(4), asked]).toEqual([[9], [9], [16], [3, 4]]);
    expect(squares(3)).toBe(first);
  });

  it("computes again for a key that keyOf tells apart, and only then", () => {
    const asked: string[] = [];
    const differences = memoised(
      ([a, b]: readonly [number, number]) => {
        asked.push(`${String(a)}-${String(b)}`);
        return a - b;
      },
      ([a, b]) => `${String(a)} ${String(b)}`,
    );
    expect([differences([1, 2]), differences([2, 1]), differences([1, 2])]).toEqual([-1, 1, -1]);
    expect(asked).toEqual(["1-2", "2-1"]);
  });

  it("keeps a result that is undefined rather than computing it again", () => {
    const asked: string[] = [];
    const nothing = memoised(
      (key: string): undefined => {
        asked.push(key);
        return undefined;
      },
      (key) => key,
    );
    expect([nothing("a"), nothing("a"), asked]).toEqual([undefined, undefined, ["a"]]);
  });
});
