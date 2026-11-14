import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import type { PerfRunRecord } from "./record.ts";
import { BUDGET_ROWS, MEMORY_ITEMS_ORDER } from "./rows.ts";
import {
  budgetSections,
  FRAME_ESTIMATES,
  GENERATED_SECTIONS,
  MEMORY_ESTIMATES,
  MEMORY_TOTAL_ESTIMATE,
  recordMarks,
  sectionMarker,
  writeBudget,
} from "./table.ts";
import { sampleRecord } from "./test/records.ts";

const BRAINSTORM = readFileSync(
  new URL("../../../docs/agent/brainstorming/rendering-and-planets.md", import.meta.url),
  "utf8",
);

/** The body rows of the brainstorm's table whose header's first cell is `first`, as cells. */
function brainstormTable(first: string): ReadonlyArray<ReadonlyArray<string>> {
  const lines = BRAINSTORM.split("\n");
  const start = lines.findIndex((line) => new RegExp(`^\\|\\s*${first}\\s*\\|`).test(line));
  if (start < 0) throw new Error(`no table headed ${first}`);
  const body: string[][] = [];
  for (const line of lines.slice(start + 2)) {
    if (!line.startsWith("|")) break;
    body.push(
      line
        .slice(1, -1)
        .split("|")
        .map((cell) => cell.trim().replaceAll("**", "")),
    );
  }
  return body;
}

const NUMBER = /\d+(?:[.,]\d+)*/g;

function figuresIn(text: string): ReadonlyArray<string> {
  return text.match(NUMBER) ?? [];
}

const UHD620 = sampleRecord();

function sectionOf(
  records: ReadonlyArray<PerfRunRecord>,
  name: (typeof GENERATED_SECTIONS)[number],
): string {
  return budgetSections(records)[name];
}

function frameRow(records: ReadonlyArray<PerfRunRecord>, label: string): string {
  const line = sectionOf(records, "frame-time")
    .split("\n")
    .find((l) => l.startsWith(`| ${label} `));
  if (line === undefined) throw new Error(`no row ${label}`);
  return line;
}

describe("the brainstorm's estimates", () => {
  it("are the frame-time table's rows, in its order", () => {
    const rows = brainstormTable("Pass");
    const ours = BUDGET_ROWS.map((r) => FRAME_ESTIMATES[r]).filter((r) => r.brainstorm !== null);
    expect(ours.map((r) => r.brainstorm)).toEqual(rows.map((cells) => cells[0]));
    ours.forEach((estimate, i) => {
      const cells = rows[i] ?? [];
      for (const figure of figuresIn(estimate.discrete?.text ?? ""))
        expect(cells[1]).toContain(figure);
      for (const figure of figuresIn(estimate.uhd620?.text ?? ""))
        expect(cells[2]).toContain(figure);
    });
  });

  it("are the memory table's rows, in its order", () => {
    const rows = brainstormTable("Item");
    const named = [
      ...MEMORY_ITEMS_ORDER.map((item) => MEMORY_ESTIMATES[item]),
      MEMORY_TOTAL_ESTIMATE,
    ].filter((r) => r.brainstorm !== null);
    const distinct = named.filter((r, i) => i === 0 || named[i - 1]?.brainstorm !== r.brainstorm);
    expect(distinct.map((r) => r.brainstorm)).toEqual(rows.map((cells) => cells[0]));
    for (const estimate of named) {
      const cells = rows.find((c) => c[0] === estimate.brainstorm) ?? [];
      for (const figure of figuresIn(estimate.discrete?.text ?? ""))
        expect(cells[1]).toContain(figure);
      for (const figure of figuresIn(estimate.uhd620?.text ?? ""))
        expect(cells[2]).toContain(figure);
    }
  });
});

describe("the frame estimates' upper ends", () => {
  it("are each a figure of its estimate's text", () => {
    for (const r of BUDGET_ROWS) {
      for (const estimate of [FRAME_ESTIMATES[r].discrete, FRAME_ESTIMATES[r].uhd620]) {
        if (estimate === null) continue;
        expect(figuresIn(estimate.text)).toContain(String(estimate.upperMs));
      }
    }
  });
});

describe("the table writer", () => {
  it("is a pure function of the records", () => {
    const records = [UHD620];
    const copy = structuredClone(records);
    expect(budgetSections(records)).toEqual(budgetSections(copy));
    expect(records).toEqual(copy);
  });

  it("orders the frame-time rows as the brainstorm's table, then the measured-only rows", () => {
    const labels = sectionOf([], "frame-time")
      .split("\n")
      .filter((l) => l.startsWith("| ") && !l.startsWith("| Pass") && !l.startsWith("| ---"))
      .map((l) => l.slice(2).split(" | ")[0]?.trim());
    expect(labels).toEqual(BUDGET_ROWS.map((r) => FRAME_ESTIMATES[r].label));
  });

  it("sets a record's medians in its machine's column, with its ID", () => {
    expect(frameRow([UHD620], "Terrain geometry and patches")).toContain(
      "9.00 / 11.20 / 12.50 (2026-11-13-uhd620-cockpit-descent-1)",
    );
    expect(sectionOf([UHD620], "memory")).toContain(
      "64.0 MB (2026-11-13-uhd620-cockpit-descent-1)",
    );
  });

  it("leaves out a virtual record and lists it with the runs", () => {
    const hidden = sampleRecord(undefined, { harness: { ...UHD620.harness, display: "virtual" } });
    expect(frameRow([hidden], "Terrain geometry and patches")).not.toContain(hidden.id);
    expect(sectionOf([hidden], "memory")).not.toContain(hidden.id);
    expect(sectionOf([hidden], "runs")).toContain("virtual: left out of the tables");
  });

  it("marks a forced record", () => {
    const forced = sampleRecord(undefined, {
      harness: { ...UHD620.harness, forced: true, forcedReasons: ["load average 2.1"] },
    });
    expect(frameRow([forced], "Terrain geometry and patches")).toContain(`(${forced.id}) (forced)`);
    expect(recordMarks(forced)).toContain("FORCED: load average 2.1");
  });

  it("reads QUANTISED where the timer was quantised, preferring a record with the full timer", () => {
    const quantised = sampleRecord(undefined, {
      id: "2026-11-14-uhd620-cockpit-descent-1",
      date: "2026-11-14T10:00:00.000Z",
      harness: { ...UHD620.harness, timer: "quantized", gpuTiming: false },
    });
    expect(frameRow([quantised], "Terrain geometry and patches")).toContain(
      `QUANTISED (${quantised.id})`,
    );
    expect(frameRow([UHD620, quantised], "Terrain geometry and patches")).toContain(
      `(${UHD620.id})`,
    );
  });

  it("shows a co-resident station run but never counts it as the station's figure", () => {
    const station = (placement: "co-resident" | "remote", n: number): PerfRunRecord =>
      sampleRecord(undefined, {
        id: `2026-11-13-uhd620-station-wireframe-descent-${String(n)}`,
        date: `2026-11-13T1${String(n)}:00:00.000Z`,
        scene: { ...UHD620.scene, id: "station-wireframe-descent" },
        harness: {
          ...UHD620.harness,
          server: { placement, numWorkers: 1, restartedPerRepetition: true },
        },
        runs: UHD620.runs.map((run) =>
          Object.assign({}, run, {
            rows: [
              { row: "station-wireframe", frames: 1000, meanMs: 5, p50Ms: 5, p95Ms: 6, p99Ms: 7 },
            ],
          }),
        ),
      });
    const coResident = station("co-resident", 2);
    expect(frameRow([coResident], "Station wireframe view")).toContain(
      `(${coResident.id}) (co-resident: not the station's figure)`,
    );
    const remote = station("remote", 1);
    expect(frameRow([remote, coResident], "Station wireframe view")).toContain(`(${remote.id})`);
    expect(frameRow([remote, coResident], "Station wireframe view")).not.toContain(coResident.id);
  });

  it("reports each record's percentiles with their ranges and the pooled p99", () => {
    expect(sectionOf([UHD620], "frames")).toContain(
      "| UHD 620, low | 2026-11-13-uhd620-cockpit-descent-1 | 180000 | 33.30 [33.30, 33.30]",
    );
  });

  it("shows the M5 Mac's columns only once it has a record", () => {
    expect(sectionOf([UHD620], "frame-time")).not.toContain("Apple M5");
    const mac = sampleRecord(undefined, {
      id: "2026-11-13-m5-air-cockpit-descent-1",
      machine: {
        ...UHD620.machine,
        name: "m5-air",
        platform: "darwin",
        cpu: { model: "Apple M5", logicalCores: 10 },
        gpu: { vendorId: 0x106b, deviceId: 0, driverVersion: null, description: null },
      },
    });
    expect(frameRow([UHD620, mac], "Terrain geometry and patches")).toContain(`(${mac.id})`);
    expect(sectionOf([mac], "frame-time")).toContain("Apple M5, low (lean)");
  });

  it("flags a monotone trend over five repetitions and the throttling apart from the median", () => {
    const record = sampleRecord();
    const trending: PerfRunRecord = {
      ...record,
      runs: record.runs.map((run, i) => ({
        ...run,
        frames: { ...run.frames, meanMs: 33 + i * 0.1 },
        throttle: { ...run.throttle, throttleFraction: i === 3 ? 0.5 : 0.05 },
      })),
    };
    expect(recordMarks(trending)).toEqual(["TREND", "throttling apart: repetitions 4"]);
    expect(recordMarks(record)).toEqual([]);
  });

  it("writes each section between its markers and keeps the prose", () => {
    const prose = GENERATED_SECTIONS.map(
      (s) => `## ${s}\n\nprose ${s}\n\n${sectionMarker(s)}\nold\n${sectionMarker(s)}\n`,
    ).join("\n");
    const written = writeBudget(prose, [UHD620]);
    for (const section of GENERATED_SECTIONS) {
      expect(written).toContain(`prose ${section}`);
      expect(written).toContain(`${sectionMarker(section)}\n<!-- prettier-ignore-start -->\n`);
    }
    expect(written).not.toContain("\nold\n");
    expect(writeBudget(written, [UHD620])).toBe(written);
  });

  it("refuses a table whose section has lost a marker", () => {
    expect(() => writeBudget(`${sectionMarker("frame-time")}\n`, [])).toThrow(/frame-time/);
  });
});
