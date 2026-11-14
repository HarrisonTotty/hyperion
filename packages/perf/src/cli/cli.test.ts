import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { serialisePerfRecord } from "../record.ts";
import { GENERATED_SECTIONS, sectionMarker } from "../table.ts";
import { metricsOf, sampleRecord } from "../test/records.ts";

/** Runs a tool as `just` does, under Node's type stripping. */
function run(
  tool: "record" | "table" | "compare",
  args: ReadonlyArray<string>,
): { status: number | null; err: string } {
  const script = fileURLToPath(new URL(`./${tool}.ts`, import.meta.url));
  const result = spawnSync(process.execPath, [script, ...args], { encoding: "utf8" });
  return { status: result.status, err: result.stderr };
}

const BUDGET = GENERATED_SECTIONS.map(
  (s) => `## ${s}\n\n${sectionMarker(s)}\n${sectionMarker(s)}\n`,
).join("\n");

let dir = "";
let store = "";
let budget = "";

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "perf-cli-"));
  store = join(dir, "runs.v1.jsonl");
  budget = join(dir, "budget.md");
  writeFileSync(store, "");
  writeFileSync(budget, BUDGET);
});

afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe("the command-line tools", () => {
  it("perf-table --check exits 1 on a stale table and writes nothing", () => {
    expect(run("table", ["--check", "--store", store, "--budget", budget]).status).toBe(1);
    expect(readFileSync(budget, "utf8")).toBe(BUDGET);
  });

  it("perf-table regenerates, after which --check passes", () => {
    expect(run("table", ["--store", store, "--budget", budget]).status).toBe(0);
    expect(run("table", ["--check", "--store", store, "--budget", budget]).status).toBe(0);
  });

  it("exits 2 on a usage error", () => {
    expect(run("table", ["--store"]).status).toBe(2);
    expect(run("record", []).status).toBe(2);
    expect(run("compare", []).status).toBe(2);
  });

  it("perf-record appends a record, and refuses it a second time with exit 1", () => {
    const file = join(dir, "record.json");
    writeFileSync(file, JSON.stringify(sampleRecord()));
    expect(run("record", [file, "--store", store, "--budget", budget]).status).toBe(0);
    const again = run("record", [file, "--store", store, "--budget", budget]);
    expect(again.status).toBe(1);
    expect(again.err).toContain("already in the store, as 2026-11-13-uhd620-cockpit-descent-1");
  });

  it("perf-compare exits 0 on a regression", () => {
    const regressed = sampleRecord(
      metricsOf([30, 30, 30, 30, 30], [8, 8, 8, 8, 8], [0.01, 0.01, 0.01, 0.01, 0.01]),
      {
        id: "2026-11-13-uhd620-cockpit-descent-2",
      },
    );
    writeFileSync(
      store,
      `${serialisePerfRecord(sampleRecord())}\n${serialisePerfRecord(regressed)}\n`,
    );
    expect(run("compare", [regressed.id, "--store", store]).status).toBe(0);
  });
});
