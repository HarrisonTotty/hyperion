/**
 * The device's budget of pipeline compiles in flight (plan R14's Mac measurements): at most its
 * size at once, a draw's ahead of the rest, the rest in the order asked, and `unlimited` asking
 * each at once as before.
 */

import { describe, expect, it } from "vitest";

import { CompileBudget, DEFAULT_COMPILE_BUDGET, isCompileBudgetSize } from "./compileBudget";

/** Compiles a test ends by hand, by name, each started in the order recorded. */
class HeldCompiles {
  readonly started: string[] = [];
  readonly #ends = new Map<string, { resolve: () => void; reject: (error: Error) => void }>();

  /** A compile named `name`, which waits until {@link HeldCompiles.end} or {@link HeldCompiles.fail}. */
  compile(name: string): () => Promise<string> {
    return () =>
      new Promise<string>((resolve, reject) => {
        this.started.push(name);
        this.#ends.set(name, {
          resolve: () => {
            resolve(name);
          },
          reject,
        });
      });
  }

  /** Ends `name`'s compile, and lets the budget's chains settle. */
  async end(name: string): Promise<void> {
    this.#ends.get(name)?.resolve();
    await settle();
  }

  /** Fails `name`'s compile, and lets the budget's chains settle. */
  async fail(name: string): Promise<void> {
    this.#ends.get(name)?.reject(new Error(`${name} could not be compiled`));
    await settle();
  }
}

/** Lets the budget's promise chains settle. */
async function settle(): Promise<void> {
  for (let hop = 0; hop < 10; hop += 1) {
    // Each hop waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await Promise.resolve();
  }
}

describe("the compile budget (plan R14)", () => {
  it("runs three by default, the rest in the order asked as places free", async () => {
    const budget = new CompileBudget();
    const held = new HeldCompiles();
    const results = ["a", "b", "c", "d", "e"].map((name) => budget.run(held.compile(name)));
    const first = [...held.started];
    await held.end("b");
    const second = [...held.started];
    await held.end("a");
    await held.end("c");
    await held.end("d");
    await held.end("e");
    expect([DEFAULT_COMPILE_BUDGET, first, second, budget.mostRunning]).toEqual([
      3,
      ["a", "b", "c"],
      ["a", "b", "c", "d"],
      3,
    ]);
    expect(await Promise.all(results)).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("starts a compile a draw waits for ahead of every compile waiting", async () => {
    const budget = new CompileBudget(1);
    const held = new HeldCompiles();
    void budget.run(held.compile("prepared"));
    void budget.run(held.compile("kernel"));
    void budget.run(held.compile("drawn"), "draw");
    await held.end("prepared");
    await held.end("drawn");
    expect(held.started).toEqual(["prepared", "drawn", "kernel"]);
  });

  it("asks every compile at once when unlimited, as before the budget", () => {
    const budget = new CompileBudget("unlimited");
    const held = new HeldCompiles();
    for (const name of ["a", "b", "c", "d", "e", "f"]) {
      void budget.run(held.compile(name));
    }
    expect([held.started.length, budget.running, budget.waiting]).toEqual([6, 6, 0]);
  });

  it("frees the place of a compile that fails, which the caller hears", async () => {
    const budget = new CompileBudget(1);
    const held = new HeldCompiles();
    const failing = budget.run(held.compile("broken"));
    const next = budget.run(held.compile("next"));
    await held.fail("broken");
    await held.end("next");
    await expect(failing).rejects.toThrow(/broken could not be compiled/u);
    expect([await next, budget.running]).toEqual(["next", 0]);
  });

  it("frees the place of a compile that throws as it is asked", async () => {
    const budget = new CompileBudget(1);
    const thrown = budget.run(() => {
      throw new Error("a descriptor the device refuses");
    });
    await expect(thrown).rejects.toThrow(/refuses/u);
    expect(await budget.run(async () => "after")).toBe("after");
  });

  it.each([0, -1, 2.5, Number.NaN, Number.POSITIVE_INFINITY])("refuses a budget of %s", (size) => {
    expect(isCompileBudgetSize(size)).toBe(false);
    expect(() => new CompileBudget(size)).toThrow(RangeError);
  });
});
