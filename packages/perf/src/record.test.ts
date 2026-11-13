import { describe, expect, it } from "vitest";

import {
  parsePerfRecord,
  parsePerfRecords,
  PerfRecordError,
  serialisePerfRecord,
  validatePerfRecord,
} from "./record.ts";
import { sampleRecord } from "./test/records.ts";

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

function plain(): Json {
  const value: unknown = JSON.parse(serialisePerfRecord(sampleRecord()));
  return toJson(value);
}

function toJson(value: unknown): Json {
  if (value === null || typeof value === "boolean" || typeof value === "number") return value;
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map(toJson);
  if (typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, toJson(v)]));
  }
  throw new Error(`not JSON: ${typeof value}`);
}

/** The open maps, whose keys are data rather than fields: a key may be absent and any value holds. */
const OPEN_MAPS = [
  "setting.values",
  "machine.capabilities",
  "machine.kernelPaths",
  /^runs\[\d+\]\.memory\.itemPeakBytes$/,
];

function isOpenMap(path: string): boolean {
  return OPEN_MAPS.some((m) => (typeof m === "string" ? m === path : m.test(path)));
}

/** Every field's path in the record, with the container and key that hold it. */
function fields(
  value: Json,
  path: string,
  out: Array<{ path: string; holder: Json[] | { [key: string]: Json }; key: string | number }>,
): typeof out {
  if (Array.isArray(value)) {
    value.forEach((item, i) => {
      const p = `${path}[${String(i)}]`;
      out.push({ path: p, holder: value, key: i });
      fields(item, p, out);
    });
  } else if (value !== null && typeof value === "object" && !isOpenMap(path)) {
    for (const [key, item] of Object.entries(value)) {
      const p = path.length > 0 ? `${path}.${key}` : key;
      out.push({ path: p, holder: value, key });
      fields(item, p, out);
    }
  }
  return out;
}

function errorOf(run: () => unknown): PerfRecordError {
  try {
    run();
  } catch (error) {
    if (error instanceof PerfRecordError) return error;
    throw error;
  }
  throw new Error("expected a PerfRecordError, and nothing was thrown");
}

function mistyped(value: Json): Json {
  if (typeof value === "string") return 7;
  if (typeof value === "number" || typeof value === "boolean") return "seven";
  if (value === null || typeof value === "object") return Array.isArray(value) ? "seven" : [7];
  return null;
}

describe("the run record", () => {
  it("round-trips through its line", () => {
    const record = sampleRecord();
    expect(parsePerfRecord(serialisePerfRecord(record))).toEqual(record);
  });

  it("names each missing field", () => {
    const paths = fields(plain(), "", []).filter((f) => !Array.isArray(f.holder));
    expect(paths.length).toBeGreaterThan(150);
    for (const { path } of paths) {
      const value = plain();
      const target = fields(value, "", []).find((f) => f.path === path);
      if (target === undefined || Array.isArray(target.holder) || typeof target.key !== "string") {
        throw new Error(`no field ${path}`);
      }
      delete target.holder[target.key];
      const error = errorOf(() => validatePerfRecord(value));
      expect({ path, field: error.field, reason: error.reason }).toEqual({
        path,
        field: path,
        reason: "missing",
      });
    }
  });

  it("names each mistyped field", () => {
    for (const { path } of fields(plain(), "", [])) {
      const value = plain();
      const target = fields(value, "", []).find((f) => f.path === path);
      if (target === undefined) throw new Error(`no field ${path}`);
      if (Array.isArray(target.holder) && typeof target.key === "number") {
        target.holder[target.key] = mistyped(target.holder[target.key] ?? null);
      } else if (!Array.isArray(target.holder) && typeof target.key === "string") {
        target.holder[target.key] = mistyped(target.holder[target.key] ?? null);
      }
      const error = errorOf(() => validatePerfRecord(value));
      expect({ path, field: error.field }).toEqual({ path, field: path });
    }
  });

  it("refuses an unknown version", () => {
    const error = errorOf(() => validatePerfRecord({ ...sampleRecord(), version: 2 }));
    expect(error.field).toBe("version");
    expect(error.reason).toContain("unknown version 2");
  });

  it("refuses a field that is not the version's", () => {
    const error = errorOf(() => validatePerfRecord({ ...sampleRecord(), extra: true }));
    expect(error.field).toBe("extra");
  });

  it("refuses an ID that is not its date, machine, scene and number", () => {
    const error = errorOf(() =>
      validatePerfRecord(sampleRecord(undefined, { id: "2026-11-12-uhd620-cockpit-descent-1" })),
    );
    expect(error.field).toBe("id");
  });

  it("refuses a perf-run record without its commit or generator version", () => {
    expect(errorOf(() => validatePerfRecord(sampleRecord(undefined, { commit: null }))).field).toBe(
      "commit",
    );
    expect(
      errorOf(() => validatePerfRecord(sampleRecord(undefined, { generatorVersion: null }))).field,
    ).toBe("generatorVersion");
  });

  it("accepts a fold's record without its commit", () => {
    const record = sampleRecord(undefined, {
      source: "descent-spike",
      commit: null,
      generatorVersion: null,
      scene: { id: "cockpit-descent", version: 0, seed: "7", world: "terrain-test" },
    });
    expect(validatePerfRecord(record)).toEqual(record);
  });

  it("refuses repetitions that disagree with their count", () => {
    const record = sampleRecord();
    const error = errorOf(() => validatePerfRecord({ ...record, runs: record.runs.slice(1) }));
    expect(error.field).toBe("runs");
  });

  it("refuses a forced record without its reasons", () => {
    const record = sampleRecord();
    const error = errorOf(() =>
      validatePerfRecord({ ...record, harness: { ...record.harness, forced: true } }),
    );
    expect(error.field).toBe("harness.forcedReasons");
  });

  it("names the store's line and refuses a repeated ID", () => {
    const line = serialisePerfRecord(sampleRecord());
    const bad = errorOf(() => parsePerfRecords(`${line}\n\n{"version":1}\n`));
    expect(bad.line).toBe(3);
    const twice = errorOf(() => parsePerfRecords(`${line}\n${line}\n`));
    expect({ line: twice.line, field: twice.field }).toEqual({ line: 2, field: "id" });
    expect(parsePerfRecords(`${line}\n`)).toHaveLength(1);
  });

  it("refuses a line that is not JSON", () => {
    const error = errorOf(() => parsePerfRecord("{", 4));
    expect(error.line).toBe(4);
    expect(error.reason).toMatch(/^not JSON/);
  });
});
