import { describe, expect, it } from "vitest";

import {
  compileBudgetFromArgv,
  compileBudgetSwitch,
  parseCompileBudget,
} from "./compileBudgetLaunch";

describe("the compile budget's hand-off (plan R14)", () => {
  it("carries a budget through the renderer's argv", () => {
    for (const budget of [1, 3, 64, "unlimited"] as const) {
      expect(compileBudgetFromArgv(["--a", compileBudgetSwitch(budget)])).toBe(budget);
    }
  });

  it("reads none from an argv that carries none", () => {
    expect(compileBudgetFromArgv(["--hyperion-quality=high"])).toBeNull();
  });

  it.each(["0", "65", "-2", "2.5", "x", "", "Unlimited"])("refuses a budget of %j", (text) => {
    expect(() => parseCompileBudget(text)).toThrow(/a compile budget is unlimited or 1 to 64/u);
  });
});
