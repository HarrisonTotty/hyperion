import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import type { PerfRunRecord } from "./record.ts";
import { parsePerfRecords, PerfRecordError, serialisePerfRecord } from "./record.ts";
import { appendRecords, recordsOfFile } from "./store.ts";
import { sampleRecord } from "./test/records.ts";

function appended(
  store: string,
  records: ReadonlyArray<PerfRunRecord>,
): { text: string; ids: ReadonlyArray<string> } {
  const outcome = appendRecords(store, records);
  if (outcome.kind !== "appended") throw new Error(`refused as a duplicate of ${outcome.existing}`);
  return outcome;
}

describe("the store", () => {
  it("appends a record under the first free number of its date, machine and scene", () => {
    const once = appended("", [sampleRecord()]);
    const twice = appended(once.text, [sampleRecord(undefined, { notes: "a second run" })]);
    expect(twice.ids).toEqual(["2026-11-13-uhd620-cockpit-descent-2"]);
    expect(parsePerfRecords(twice.text).map((r) => r.id)).toEqual([
      "2026-11-13-uhd620-cockpit-descent-1",
      "2026-11-13-uhd620-cockpit-descent-2",
    ]);
  });

  it("only ever appends", () => {
    const once = appended("", [sampleRecord()]);
    const twice = appended(once.text, [sampleRecord(undefined, { notes: "a second run" })]);
    expect(twice.text.startsWith(once.text)).toBe(true);
  });

  it("refuses a record already in the store under any number", () => {
    const once = appended("", [sampleRecord()]);
    const again = sampleRecord(undefined, { id: "2026-11-13-uhd620-cockpit-descent-7" });
    expect(appendRecords(once.text, [again])).toEqual({
      kind: "duplicate",
      existing: "2026-11-13-uhd620-cockpit-descent-1",
    });
  });
});

describe("a file given to perf-record", () => {
  it("is read as one record", () => {
    const record = sampleRecord();
    expect(recordsOfFile(JSON.stringify(record, null, 2))).toEqual([record]);
  });

  it("is read as records in JSON lines", () => {
    const record = sampleRecord();
    const other = sampleRecord(undefined, {
      id: "2026-11-13-uhd620-cockpit-descent-2",
      notes: "two",
    });
    expect(
      recordsOfFile(`${serialisePerfRecord(record)}\n${serialisePerfRecord(other)}\n`),
    ).toEqual([record, other]);
  });

  it("is folded when it is R05's results file", () => {
    const spike = readFileSync(
      new URL("../fixtures/descent-spike-v6.json", import.meta.url),
      "utf8",
    );
    expect(recordsOfFile(spike).map((r) => r.source)).toEqual(["descent-spike"]);
  });

  it("is refused naming the field at fault", () => {
    const { notes: _notes, ...withoutNotes } = sampleRecord();
    expect(() => recordsOfFile(JSON.stringify(withoutNotes))).toThrow(PerfRecordError);
    expect(() => recordsOfFile(JSON.stringify(withoutNotes))).toThrow(/notes: missing/);
  });
});
