import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { parsePerfRecords, PerfRecordError, serialisePerfRecord } from "./record.ts";
import { appendRecords, recordsOfFile } from "./store.ts";
import { sampleRecord } from "./test/records.ts";

describe("the store", () => {
  it("appends a record under the first free number of its date, machine and scene", () => {
    const first = sampleRecord();
    const once = appendRecords("", [first]);
    expect(once.ids).toEqual(["2026-11-13-uhd620-cockpit-descent-1"]);
    const second = sampleRecord(undefined, { notes: "a second run" });
    const twice = appendRecords(once.text, [second]);
    expect(twice.ids).toEqual(["2026-11-13-uhd620-cockpit-descent-2"]);
    expect(parsePerfRecords(twice.text).map((r) => r.id)).toEqual([...once.ids, ...twice.ids]);
    expect(twice.text.startsWith(once.text)).toBe(true);
  });

  it("refuses a record already in the store under any number", () => {
    const once = appendRecords("", [sampleRecord()]);
    expect(() =>
      appendRecords(once.text, [
        sampleRecord(undefined, { id: "2026-11-13-uhd620-cockpit-descent-7" }),
      ]),
    ).toThrow(/already in the store, as 2026-11-13-uhd620-cockpit-descent-1/);
  });

  it("reads a record, records as JSON lines and R05's results file", () => {
    const record = sampleRecord();
    expect(recordsOfFile(JSON.stringify(record, null, 2))).toEqual([record]);
    const other = sampleRecord(undefined, {
      id: "2026-11-13-uhd620-cockpit-descent-2",
      notes: "two",
    });
    expect(
      recordsOfFile(`${serialisePerfRecord(record)}\n${serialisePerfRecord(other)}\n`),
    ).toEqual([record, other]);
    const spike = readFileSync(
      new URL("../fixtures/descent-spike-v6.json", import.meta.url),
      "utf8",
    );
    expect(recordsOfFile(spike).map((r) => r.source)).toEqual(["descent-spike"]);
  });

  it("names the field of a record it refuses", () => {
    const { notes: _notes, ...withoutNotes } = sampleRecord();
    expect(() => recordsOfFile(JSON.stringify(withoutNotes))).toThrow(PerfRecordError);
    expect(() => recordsOfFile(JSON.stringify(withoutNotes))).toThrow(/notes: missing/);
  });
});
