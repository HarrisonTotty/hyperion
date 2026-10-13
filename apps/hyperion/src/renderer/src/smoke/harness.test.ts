import { describe, expect, it, vi } from "vitest";

import { Checks } from "./harness";

describe("Checks", () => {
  it("times each group, a group that throws among them, and logs each time as it ends", async () => {
    vi.spyOn(performance, "now")
      .mockReturnValueOnce(0)
      .mockReturnValueOnce(1_500)
      .mockReturnValueOnce(2_000)
      .mockReturnValueOnce(6_000);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const checks = new Checks();
    await checks.group("quick", () => Promise.resolve());
    await checks.group("slow", () => Promise.reject(new Error("broke")));
    expect(checks.times).toEqual([
      { name: "quick", ms: 1_500 },
      { name: "slow", ms: 4_000 },
    ]);
    expect(warn.mock.calls).toEqual([["smoke group quick: 1.5 s"], ["smoke group slow: 4.0 s"]]);
    expect(checks.list).toEqual([{ name: "slow", pass: false, detail: "threw Error: broke" }]);
  });

  it("records the groups' times as one passing check, slowest first, with their sum", async () => {
    vi.spyOn(performance, "now")
      .mockReturnValueOnce(0)
      .mockReturnValueOnce(1_500)
      .mockReturnValueOnce(2_000)
      .mockReturnValueOnce(6_000);
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const checks = new Checks();
    await checks.group("quick", () => Promise.resolve());
    await checks.group("slow", () => Promise.resolve());
    checks.recordTimes();
    expect(checks.list).toEqual([
      {
        name: "the page's groups' wall times",
        pass: true,
        detail: "5.5 s in all: slow 4.0 s; quick 1.5 s",
      },
    ]);
  });
});
