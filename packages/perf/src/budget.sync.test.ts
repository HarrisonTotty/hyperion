import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { parsePerfRecords } from "./record.ts";
import { writeBudget } from "./table.ts";

const DIR = new URL("../../../docs/measurements/rendering/", import.meta.url);

describe("docs/measurements/rendering", () => {
  it("holds a budget.md whose generated sections agree with runs.v1.jsonl (run `just perf-table`)", () => {
    const budget = readFileSync(new URL("budget.md", DIR), "utf8");
    const records = parsePerfRecords(readFileSync(new URL("runs.v1.jsonl", DIR), "utf8"));
    expect(writeBudget(budget, records)).toBe(budget);
  });
});
