/**
 * The client's bless helper (plan R08, R08.T4.a): a vitest that computes a committed table compares
 * it with the file, and rewrites the file only when asked to, as the Rust testkit's golden files
 * are (`crates/hyperion-testkit/src/golden.rs`).
 *
 * @remarks
 * Renderer code, tests included, reads no files through `node:fs` and has no Node types
 * (R04.T10.c), so a table's consumers read it as a JSON import, and this helper hands the file to
 * vitest's `toMatchFileSnapshot`, which reads and writes it in vitest's own process. The modes are
 * golden.rs's, from the same two variables:
 *
 * - `HYPERION_BLESS` unset, or set to anything but `1`: compare. A table that differs from its file
 *   fails, naming the command that rewrites it.
 * - `HYPERION_BLESS=1`: bless. `vitest.config.mts` turns vitest's snapshot update mode on, and the
 *   file is rewritten.
 * - `HYPERION_BLESS=1` under `CI` (set to anything): refused, so that CI can never paper over a
 *   change. The update mode stays off.
 *
 * Under vitest, `import.meta.env` reads the process's environment.
 */

import { expect } from "vitest";

/** The variable that asks for a bless, the Rust testkit's `BLESS_VAR`. */
export const BLESS_VARIABLE = "HYPERION_BLESS";

/** The variable whose presence forbids a bless, the Rust testkit's `CI_VAR`. */
export const CI_VARIABLE = "CI";

/** What a bless-style test does with its table. */
export type BlessMode = "compare" | "bless" | "blessForbidden";

/**
 * The mode for the two variables' values, `undefined` where unset: golden.rs's `Mode::from_vars`.
 * `vitest.config.mts` applies the same rule to its update mode.
 */
export function blessModeOf(bless: unknown, ci: unknown): BlessMode {
  if (bless !== "1") {
    return "compare";
  }
  return ci === undefined ? "bless" : "blessForbidden";
}

/** The mode of this test run, from its environment. */
export function blessMode(): BlessMode {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return blessModeOf(env[BLESS_VARIABLE], env[CI_VARIABLE]);
}

/**
 * The command that rewrites a test file's committed tables.
 *
 * @param testFile - The test file, as vitest reports it (absolute) or relative to `apps/hyperion`;
 *   an absolute path is cut to the part after its last `apps/hyperion/`.
 */
export function blessCommand(testFile: string): string {
  const unixPath = testFile.replaceAll("\\", "/");
  const marker = "apps/hyperion/";
  const at = unixPath.lastIndexOf(marker);
  const relative = at < 0 ? unixPath : unixPath.slice(at + marker.length);
  return `${BLESS_VARIABLE}=1 pnpm --filter hyperion exec vitest run ${relative}`;
}

/**
 * A table as the text its file holds: JSON with two-space indentation, an array of numbers,
 * strings, booleans or nulls on one line, and a newline at the end.
 *
 * @remarks
 * The text is compared byte for byte, so the committed files are not reformatted by Prettier
 * (`.prettierignore`). A number is written as JavaScript's shortest round-trip form, so the file
 * reads back to the same doubles.
 *
 * @throws TypeError for a value JSON cannot hold as written: `undefined`, a function, a symbol, a
 *   bigint, a number that is not finite, or an object that is neither an array nor a plain object
 *   (a typed array, a `Map`, a `Set`, a `Date`), which a caller turns into an array or a record
 *   first.
 */
export function tableText(table: unknown): string {
  return `${textOf(table, "")}\n`;
}

function textOf(value: unknown, indent: string): string {
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError(`a table holds finite numbers, not ${value}`);
    }
    return JSON.stringify(value);
  }
  if (typeof value !== "object") {
    throw new TypeError(`a table holds JSON values, not a ${typeof value}`);
  }
  const prototype: unknown = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(
      `a table holds arrays and plain objects, not a ${value.constructor.name}: convert it first`,
    );
  }
  const inner = `${indent}  `;
  if (Array.isArray(value)) {
    const items: readonly unknown[] = value;
    if (items.length === 0) {
      return "[]";
    }
    if (items.every((item) => item === null || typeof item !== "object")) {
      return `[${items.map((item) => textOf(item, inner)).join(", ")}]`;
    }
    return `[\n${items.map((item) => `${inner}${textOf(item, inner)}`).join(",\n")}\n${indent}]`;
  }
  const entries = Object.entries(value);
  if (entries.length === 0) {
    return "{}";
  }
  const fields = entries.map(
    ([key, item]) => `${inner}${JSON.stringify(key)}: ${textOf(item, inner)}`,
  );
  return `{\n${fields.join(",\n")}\n${indent}}`;
}

/**
 * Compares `table` with its committed file, or rewrites the file under a bless.
 *
 * @remarks
 * When comparing, the values are checked first against `committed`, the file as its JSON import
 * reads it, so that a changed table fails with the command and leaves vitest's snapshot state
 * alone; the text is then checked byte for byte by `toMatchFileSnapshot`, which also catches a
 * file reformatted by hand.
 *
 * Vitest writes a blessed file when the test file finishes, whatever else failed, so a check that
 * should stop a bless (such as a refit that must not worsen its record) runs before this call, in
 * the same test.
 *
 * A new table starts as a committed placeholder (`{}`), which its JSON import needs before the
 * first run, with an entry in `.prettierignore`; a bless then writes it.
 *
 * @example
 * ```ts
 * import recorded from "./channels.json" with { type: "json" };
 *
 * it("is written to channels.json under a bless, and checked unchanged otherwise", async () => {
 *   await expect(expectCommittedTable(table, recorded, "./channels.json")).resolves.toBeUndefined();
 * });
 * ```
 *
 * @param table - The table as the test computes it.
 * @param committed - The committed file's JSON import.
 * @param file - The file, relative to the calling test file.
 * @throws Error, through the returned promise, when the table differs from the file, by value or
 *   by byte (naming {@link blessCommand}), or when a bless is asked for under `CI`.
 */
export async function expectCommittedTable(
  table: unknown,
  committed: unknown,
  file: string,
): Promise<void> {
  const command = blessCommand(expect.getState().testPath ?? "<the test file>");
  const message =
    `${file} is not the table the test computes; if the change is intended, run \`${command}\` ` +
    "and commit the file";
  const text = tableText(table);
  const mode = blessMode();
  switch (mode) {
    case "blessForbidden":
      throw new Error(
        `refusing to bless ${file}: ${BLESS_VARIABLE} is set under ${CI_VARIABLE}; committed ` +
          "tables are blessed on a developer machine and committed",
      );
    case "compare":
      if (tableText(committed) !== text) {
        throw new Error(message);
      }
      break;
    case "bless":
      break;
  }
  try {
    await expect(text).toMatchFileSnapshot(file);
  } catch (error: unknown) {
    throw new Error(message, { cause: error });
  }
}
