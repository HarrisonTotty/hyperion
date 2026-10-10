import { afterEach, describe, expect, it, vi } from "vitest";

import { blessCommand, blessMode, blessModeOf, expectCommittedTable, tableText } from "./bless";
import probe from "./blessProbe.json" with { type: "json" };

/** The probe file's table, which `blessProbe.json` holds. */
const PROBE = {
  description: "bless.test.ts's probe table",
  rows: [
    [1, 0.5],
    [2, 0.25],
  ],
};

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("the bless mode", () => {
  it.each([
    [undefined, undefined, "compare"],
    ["0", undefined, "compare"],
    [undefined, "true", "compare"],
    ["1", undefined, "bless"],
    ["1", "", "blessForbidden"],
    ["1", "true", "blessForbidden"],
  ])("is golden.rs's for HYPERION_BLESS %s and CI %s: %s", (bless, ci, mode) => {
    expect(blessModeOf(bless, ci)).toBe(mode);
  });
});

describe("the bless command", () => {
  it.each([
    [
      "/home/x/apps/hyperion/src/renderer/src/view/atmosphere/channels.test.ts",
      "src/renderer/src/view/atmosphere/channels.test.ts",
    ],
    ["C:\\w\\apps\\hyperion\\src\\a.test.ts", "src/a.test.ts"],
  ])("runs %s from apps/hyperion", (testFile, relative) => {
    expect(blessCommand(testFile)).toBe(
      `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run ${relative}`,
    );
  });
});

describe("a table's text", () => {
  it("puts arrays of plain values on one line and ends with a newline", () => {
    expect(tableText({ a: [1, 2], b: { c: "d" }, e: [[1], [2]], f: [], g: {} })).toBe(
      '{\n  "a": [1, 2],\n  "b": {\n    "c": "d"\n  },\n  "e": [\n    [1],\n    [2]\n  ],\n  "f": [],\n  "g": {}\n}\n',
    );
  });

  it("reads back to the same doubles", () => {
    const values = [0.1 + 0.2, 1 / 3, 6.02214076e23, 5e-324, -1.5e-300];
    expect(JSON.parse(tableText(values))).toEqual(values);
  });

  it.each([
    ["NaN", { a: Number.NaN }],
    ["an infinity", [Infinity]],
    ["undefined", { a: undefined }],
    ["a typed array", { a: new Float64Array([1, 2]) }],
    ["a map", new Map([["a", 1]])],
  ])("refuses %s", (_, table) => {
    expect(() => tableText(table)).toThrow(TypeError);
  });
});

describe("a committed table", () => {
  it("is the JSON its import reads", () => {
    expect(probe).toEqual(PROBE);
  });

  it("passes when unchanged", async () => {
    await expect(expectCommittedTable(PROBE, probe, "./blessProbe.json")).resolves.toBeUndefined();
  });

  // Under a bless the changed table would be written, so this runs only when comparing.
  it.runIf(blessMode() === "compare")(
    "fails on a changed table and names the command",
    async () => {
      await expect(
        expectCommittedTable({ ...PROBE, rows: [[1, 0.5]] }, probe, "./blessProbe.json"),
      ).rejects.toThrow(
        /blessProbe\.json is not the table the test computes; if the change is intended, run `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run src\/renderer\/src\/test\/bless\.test\.ts`/u,
      );
    },
  );

  it("refuses a bless under CI, before anything is written", async () => {
    vi.stubEnv("HYPERION_BLESS", "1");
    vi.stubEnv("CI", "1");
    await expect(
      expectCommittedTable({ changed: true }, probe, "./blessProbe.json"),
    ).rejects.toThrow(/refusing to bless \.\/blessProbe\.json: HYPERION_BLESS is set under CI/u);
  });
});
