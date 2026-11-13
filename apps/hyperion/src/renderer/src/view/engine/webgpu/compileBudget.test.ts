/**
 * The device's budget of pipeline compiles in flight (plan R14's Mac measurements): at most its
 * size at once, a draw's ahead of the rest, the rest in the order asked, and `unlimited` asking
 * each at once as before.
 */

import { describe, expect, it, vi } from "vitest";

import type { CompileEvent } from "../types";
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
    const results = ["a", "b", "c", "d", "e"].map((name) => budget.run(name, held.compile(name)));
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
    void budget.run("prepared", held.compile("prepared"));
    void budget.run("kernel", held.compile("kernel"));
    void budget.run("drawn", held.compile("drawn"), "draw");
    await held.end("prepared");
    await held.end("drawn");
    expect(held.started).toEqual(["prepared", "drawn", "kernel"]);
  });

  it("asks every compile at once when unlimited, as before the budget", () => {
    const budget = new CompileBudget("unlimited");
    const held = new HeldCompiles();
    for (const name of ["a", "b", "c", "d", "e", "f"]) {
      void budget.run(name, held.compile(name));
    }
    expect([held.started.length, budget.running, budget.waiting]).toEqual([6, 6, 0]);
  });

  it("frees the place of a compile that fails, which the caller hears", async () => {
    const budget = new CompileBudget(1);
    const held = new HeldCompiles();
    const failing = budget.run("broken", held.compile("broken"));
    const next = budget.run("next", held.compile("next"));
    await held.fail("broken");
    await held.end("next");
    await expect(failing).rejects.toThrow(/broken could not be compiled/u);
    expect([await next, budget.running]).toEqual(["next", 0]);
  });

  it("frees the place of a compile that throws as it is asked", async () => {
    const budget = new CompileBudget(1);
    const thrown = budget.run("refused", () => {
      throw new Error("a descriptor the device refuses");
    });
    await expect(thrown).rejects.toThrow(/refuses/u);
    expect(await budget.run("after", async () => "after")).toBe("after");
  });

  it("times each compile from its create call to its pipeline's being ready, and its wait for a place, by name and lane (R08)", async () => {
    let nowMs = 1000;
    const budget = new CompileBudget(1, () => nowMs);
    const held = new HeldCompiles();
    const heard: CompileEvent[] = [];
    budget.onCompile((event) => heard.push(event));
    const runs = [
      budget.run("composite", held.compile("composite")),
      budget.run("ray march", held.compile("ray march")),
      budget.run("material", held.compile("material"), "draw"),
    ];
    nowMs = 1250;
    await held.end("composite");
    nowMs = 1300;
    await held.fail("material");
    nowMs = 2000;
    await held.end("ray march");
    await Promise.allSettled(runs);
    expect(heard).toEqual([
      { name: "composite", lane: "ahead", waitedMs: 0, ms: 250 },
      { name: "material", lane: "draw", waitedMs: 250, ms: 50 },
      { name: "ray march", lane: "ahead", waitedMs: 300, ms: 700 },
    ]);
  });

  it("stops telling a listener of compiles once it is removed, or the budget's listeners are cleared (R08)", async () => {
    const budget = new CompileBudget(1, () => 0);
    const heard: string[] = [];
    const stop = budget.onCompile((event) => heard.push(`removed: ${event.name}`));
    budget.onCompile((event) => heard.push(`cleared: ${event.name}`));
    await budget.run("before", async () => "pipeline");
    stop();
    await budget.run("removed", async () => "pipeline");
    budget.clearListeners();
    await budget.run("cleared", async () => "pipeline");
    await settle();
    expect(heard).toEqual(["removed: before", "cleared: before", "cleared: removed"]);
  });

  it("hears no compile whose turn made nothing (R08)", async () => {
    const budget = new CompileBudget(1, () => 0);
    const heard: string[] = [];
    budget.onCompile((event) => heard.push(event.name));
    await budget.run("made by a draw first", async () => null);
    await budget.run("material", async () => "pipeline");
    await settle();
    expect(heard).toEqual(["material"]);
  });

  it("gives a pipeline to its caller though a listener throws, and says the listener failed (R08)", async () => {
    const budget = new CompileBudget(1, () => 0);
    budget.onCompile(() => {
      throw new Error("a listener that breaks");
    });
    const failure = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const made = await budget.run("material", async () => "pipeline");
    await settle();
    expect([made, failure.mock.calls.length]).toEqual(["pipeline", 1]);
  });

  it.each([0, -1, 2.5, Number.NaN, Number.POSITIVE_INFINITY])("refuses a budget of %s", (size) => {
    expect(isCompileBudgetSize(size)).toBe(false);
    expect(() => new CompileBudget(size)).toThrow(RangeError);
  });
});
