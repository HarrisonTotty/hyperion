import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it, vi } from "vitest";

import {
  CLIENT_ROOT,
  main,
  PINNED_FILES,
  type PinIo,
  pinDigests,
  pinsOf,
  placePins,
  type RowDigest,
  rewritePins,
  writtenTables,
} from "./pinDigests";

/** A pinned file in small: a stale pin, one a comment repeats, and one the table's `map` makes. */
const WRITTEN = `function digestOf(source: string): string {
  return source;
}

/** Re-pinned by the lane that moved "0f00"; the comments keep their digits. */
const A_DIGESTS: ReadonlyArray<readonly [string, string, string]> = [
  ["first", "x", "0f00"],
  // "0f00", as this comment has it.
  ["second", "y", "0f00"],
  ...[["third", "abc"]].map(([name = "", digest = ""]) => [name, name, digest] as const),
];
`;

const ROWS: ReadonlyArray<RowDigest> = [
  { label: "first", pinned: "0f00", digest: "0f00" },
  { label: "second", pinned: "0f00", digest: "1234abcd" },
  { label: "third", pinned: "abc", digest: "fabc" },
];

/** An I/O that reads `texts` where given and the file otherwise, and records what it writes. */
function recording(texts: ReadonlyMap<string, string>): {
  readonly io: PinIo;
  readonly writes: Map<string, string>;
  readonly reports: string[];
} {
  const writes = new Map<string, string>();
  const reports: string[] = [];
  const io: PinIo = {
    read: (file) => texts.get(file) ?? readFileSync(file, "utf8"),
    write: (file, text) => {
      writes.set(file, text);
    },
    report: (line) => {
      reports.push(line);
    },
  };
  return { io, writes, reports };
}

describe("placing and rewriting the pins", () => {
  it.each([
    ["LF", "\n"],
    ["CRLF", "\r\n"],
  ])(
    "rewrites the stale pins' digits alone, the comments' copies and the %s line ends left",
    (_ends, end) => {
      const written = WRITTEN.replaceAll("\n", end);
      const pins = placePins(
        "a.ts",
        written,
        writtenTables("a.ts", written),
        new Map([["A_DIGESTS", ROWS]]),
      );
      expect(pins.map(({ label, line }) => [label, line])).toEqual([
        ["first", 7],
        ["second", 9],
        ["third", 10],
      ]);
      expect(rewritePins(written, pins)).toBe(
        written
          .replace('["second", "y", "0f00"]', '["second", "y", "1234abcd"]')
          .replace('"abc"]]', '"fabc"]]'),
      );
    },
  );

  it("leaves a file whose pins are current as it was", () => {
    const current = ROWS.map((row) => ({ ...row, pinned: row.digest }));
    const text = rewritePins(
      WRITTEN,
      placePins("a.ts", WRITTEN, writtenTables("a.ts", WRITTEN), new Map([["A_DIGESTS", ROWS]])),
    );
    const pins = placePins(
      "a.ts",
      text,
      writtenTables("a.ts", text),
      new Map([["A_DIGESTS", current]]),
    );
    expect(rewritePins(text, pins)).toBe(text);
  });

  it("refuses a pin that is not one of its table's literals, in the rows' order", () => {
    const tables = writtenTables("a.ts", WRITTEN);
    const reordered = new Map([["A_DIGESTS", [ROWS[2], ROWS[0]].flatMap((row) => row ?? [])]]);
    expect(() => placePins("a.ts", WRITTEN, tables, reordered)).toThrow(
      /A_DIGESTS's first pins 0f00/,
    );
  });

  it("refuses a file with a conflict left in it", () => {
    expect(() => writtenTables("a.ts", `${WRITTEN}<<<<<<< ours\n`)).toThrow(/does not parse/);
  });

  it("refuses a file with no table", () => {
    expect(() => writtenTables("a.ts", WRITTEN.replace("A_DIGESTS", "A_PINS"))).toThrow(
      /declares no top-level `digestOf` and `<NAME>_DIGESTS` table/,
    );
  });

  it("refuses a file with no digestOf", () => {
    expect(() => writtenTables("a.ts", WRITTEN.replace("digestOf", "digest"))).toThrow(
      /declares no top-level `digestOf` and `<NAME>_DIGESTS` table/,
    );
  });
});

describe("the command line (pin-digests)", () => {
  it("fails on an option it does not know, before it loads anything", async () => {
    const reported = vi.spyOn(console, "error").mockImplementation(() => undefined);
    expect(await main(["--nope"])).toBe(1);
    expect(reported).toHaveBeenCalledWith(expect.stringMatching(/^pin-digests: .*--nope/u));
  });
});

// Each `pinDigests` or `pinsOf` call loads a pinned file through Vite's module runner: 1.2 to
// 1.7 s a file in a pod, the file run alone (2026-11-03), and a test makes up to three loads, so
// its limit is 60 s, over ten times theirs.
describe("the pinned files (pin-digests)", () => {
  const files = PINNED_FILES.map((file) => join(CLIENT_ROOT, file));

  it("leaves a clean tree as it is", async () => {
    const { io, writes, reports } = recording(new Map());
    expect(await pinDigests(files, "write", io)).toBe(0);
    expect(writes.size).toBe(0);
    expect(reports).toEqual([expect.stringMatching(/^pin-digests: \d+ pins, all current$/u)]);
  }, 60_000);

  // From the file with every pin current, so that a pin another change left stale moves nothing.
  it.each(PINNED_FILES)(
    "restores a pin of %s edited by hand to a wrong value, and its check flags it",
    async (name) => {
      const file = join(CLIENT_ROOT, name);
      const text = readFileSync(file, "utf8");
      const pins = await pinsOf(file, text);
      const [first, second, ...rest] = pins;
      if (first === undefined || second === undefined) {
        throw new Error(`${file} pins fewer than two rows`);
      }
      const wrong = second.digest === "0123456789abcdef" ? "fedcba9876543210" : "0123456789abcdef";
      const current = rewritePins(text, pins);
      const edited = rewritePins(text, [first, { ...second, digest: wrong }, ...rest]);

      const checked = recording(new Map([[file, edited]]));
      expect(await pinDigests([file], "check", checked.io)).toBe(1);
      expect(checked.writes.size).toBe(0);
      expect(checked.reports).toHaveLength(2);
      expect(checked.reports[0]).toMatch(/^stale /u);
      expect(checked.reports[0]).toContain(
        `${second.table} ${second.label}: ${wrong} -> ${second.digest}`,
      );

      const written = recording(new Map([[file, edited]]));
      expect(await pinDigests([file], "write", written.io)).toBe(0);
      expect([...written.writes]).toEqual([[file, current]]);
    },
    60_000,
  );
});
