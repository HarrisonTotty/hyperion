import { readFile } from "node:fs/promises";
import { join } from "node:path";

import { describe, expect, it, vi } from "vitest";

import {
  keepsLoadAverage,
  NO_WINDOWS_LOAD_AVERAGE,
  quietOf,
  readLoadAverage,
  recordedLoadAverage,
} from "./machineLoad";
import { measured, missing } from "./measured";

describe("the load average", () => {
  it("is os.loadavg's on Linux and macOS", () => {
    expect(readLoadAverage({ platform: "linux", loadavg: () => [1.5, 1.2, 0.9] })).toEqual(
      measured([1.5, 1.2, 0.9]),
    );
    expect(readLoadAverage({ platform: "darwin", loadavg: () => [0.5, 0.4, 0.3] })).toEqual(
      measured([0.5, 0.4, 0.3]),
    );
  });

  it("is none on Windows, where os.loadavg gives zeros", () => {
    expect(readLoadAverage({ platform: "win32", loadavg: () => [0, 0, 0] })).toEqual(
      missing(NO_WINDOWS_LOAD_AVERAGE),
    );
  });

  it("refuses a reading of fewer than three averages, which Node never gives", () => {
    expect(() => readLoadAverage({ platform: "linux", loadavg: () => [0.5] })).toThrow(
      "os.loadavg() gave 1 of its three averages",
    );
  });

  it("is kept everywhere but Windows, under Node's name or the replayer's", () => {
    expect(["linux", "darwin", "macos", "win32", "windows"].map(keepsLoadAverage)).toEqual([
      true,
      true,
      true,
      false,
      false,
    ]);
  });
});

describe("the quiet-machine rule", () => {
  it("marks a run started at a load of 1 or more provisional", () => {
    expect(quietOf("linux", [1.5, 1.2, 0.9])).toEqual({
      provisional: true,
      note: "load average 1.50 at the start (Design note 27 asks under 1): provisional",
    });
  });

  it("does not mark a run on macOS at a load of 0.5 provisional", () => {
    expect(quietOf("darwin", [0.5, 0.4, 0.3])).toEqual({ provisional: false, note: null });
  });

  it("marks every run on Windows provisional, its rule unchecked", () => {
    expect(quietOf("win32", [0, 0, 0])).toEqual({
      provisional: true,
      note: "Windows keeps no load average: the quiet-machine rule (Design note 27) is unchecked",
    });
  });
});

/** The demand record's script, whose top level runs the record, so it is read rather than imported. */
function demandScript(): Promise<string> {
  return readFile(join(__dirname, "../../scripts/descentDemand.mjs"), "utf8");
}

/** The script's call that records a cell's load average, however Prettier wraps it. */
const RECORDS_THE_LOAD =
  /recordedLoadAverage\(\s*\{\s*platform:\s*process\.platform,\s*loadavg\s*\}\s*\)/u;

/** `/proc` named anywhere, as a whole path segment. */
const NAMES_PROC = /\/proc\b/u;

describe("the demand record's load average (scripts/descentDemand.mjs)", () => {
  it("is read with /proc absent, through the injected os.loadavg alone", () => {
    // The reader is all the path touches, so a machine without /proc (macOS, Windows) runs it.
    const loadavg = vi.fn<() => ReadonlyArray<number>>(() => [0.5, 0.4, 0.3]);
    expect(recordedLoadAverage({ platform: "darwin", loadavg })).toEqual([0.5, 0.4, 0.3]);
    expect(loadavg).toHaveBeenCalledOnce();
  });

  it("is an empty list on Windows, which keeps none", () => {
    expect(recordedLoadAverage({ platform: "win32", loadavg: () => [0, 0, 0] })).toEqual([]);
  });

  it("is what the script records", async () => {
    expect(await demandScript()).toMatch(RECORDS_THE_LOAD);
  });

  it("is read without naming /proc anywhere in the script", async () => {
    expect(await demandScript()).not.toMatch(NAMES_PROC);
  });

  it("would catch the script naming /proc, however it builds the path", () => {
    expect(
      [
        `readFileSync("/proc/loadavg", "utf8")`,
        `join("/proc", "loadavg")`,
        "`/proc/${pid}/status`",
      ].filter((line) => NAMES_PROC.test(line)),
    ).toHaveLength(3);
  });
});
