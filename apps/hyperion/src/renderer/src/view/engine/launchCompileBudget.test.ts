import { describe, expect, it, vi } from "vitest";

import { launchCompileBudget } from "./launchCompileBudget";

describe("the launch's compile budget (plan R14)", () => {
  it("gives the engine the budget the preload carries", () => {
    vi.stubGlobal("hyperion", { compileBudget: "unlimited" });
    expect(launchCompileBudget()).toEqual({ compileBudget: "unlimited" });
    vi.stubGlobal("hyperion", { compileBudget: 2 });
    expect(launchCompileBudget()).toEqual({ compileBudget: 2 });
  });

  it("gives none where the launch gave none, or no preload runs", () => {
    vi.stubGlobal("hyperion", {});
    expect(launchCompileBudget()).toEqual({});
    vi.stubGlobal("hyperion", undefined);
    expect(launchCompileBudget()).toEqual({});
  });
});
