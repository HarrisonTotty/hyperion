/**
 * `just pin-digests`: rewrites the WGSL digest pins of `discReflectance.test.ts` and
 * `thickKernels.test.ts` from the kernels' sources, so that a landing whose lanes both moved a
 * kernel re-pins them with no test run (lane tool-pin-digests).
 *
 * @remarks
 * A pinned file's tables are its top-level `const`s named `<NAME>_DIGESTS`. Each row ends with its
 * pin, the expected digest as a string literal, after the value the file's own `digestOf` digests:
 * a kernel's source, or its kernel pair. The tool loads the file through Vite's module runner,
 * which serves its `?raw` shaders as vitest does, with `vitest` a stub whose `describe` runs
 * nothing, so that only the file's top level runs: its imports, the kernels' sources and the
 * tables. It digests each row by the file's `digestOf` and rewrites a stale pin's hex digits where
 * they stand, a table's rows matched to its string literals in order. Nothing else in the file
 * changes: not the comments (the history of re-pins above a table is the re-pinning lane's to
 * write), the names, the order or the line ends. `--check` writes nothing, names each stale pin
 * and exits 1.
 */

import { readFileSync, realpathSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { normalizePath, parseSync, type Plugin, runnerImport, Visitor } from "vite";

/** The files whose tables the tool pins, from {@link CLIENT_ROOT}. */
export const PINNED_FILES: ReadonlyArray<string> = [
  "src/renderer/src/view/atmosphere/discReflectance.test.ts",
  "src/renderer/src/view/atmosphere/thickKernels.test.ts",
];

/** The client's root, `apps/hyperion`. */
export const CLIENT_ROOT = fileURLToPath(new URL("../..", import.meta.url));

/** A digest table's name. */
const TABLE_NAME = /_DIGESTS$/u;

/** The module the pinned file's `vitest` import is served: no `describe` or `it` runs anything. */
const VITEST_STUB = "\0hyperion:pin-digests-vitest";

const VITEST_STUB_CODE = `const inert = new Proxy(function () {}, {
  get: (_target, key) => (key === "then" ? undefined : inert),
  apply: () => inert,
});
export const describe = inert, it = inert, test = inert, suite = inert, expect = inert, vi = inert;
export const beforeAll = inert, afterAll = inert, beforeEach = inert, afterEach = inert;
`;

/** A string literal's value and where it stands, as UTF-16 offsets of its quotes. */
interface WrittenLiteral {
  readonly value: string;
  readonly start: number;
  readonly end: number;
}

/** A digest table as written: its name and the string literals of its initialiser, in order. */
export interface WrittenTable {
  readonly name: string;
  readonly literals: ReadonlyArray<WrittenLiteral>;
}

/** A table's row as the file's top level made it. */
export interface RowDigest {
  /** The row's name in messages: its first element, or that element's `name`. */
  readonly label: string;
  /** The digest the row expects, as written. */
  readonly pinned: string;
  /** The digest of the row's source now, by the file's `digestOf`. */
  readonly digest: string;
}

/** A row's pin, placed in its file's text. */
export interface Pin extends RowDigest {
  readonly table: string;
  /** The line of the pin's literal, from 1. */
  readonly line: number;
  /** The offset of the pin's first hex digit, past its opening quote. */
  readonly start: number;
  /** The offset of its closing quote. */
  readonly end: number;
}

/** How {@link pinDigests} reads, writes and reports: the file system and stdout, or a test's. */
export interface PinIo {
  readonly read: (file: string) => string;
  readonly write: (file: string, text: string) => void;
  readonly report: (line: string) => void;
}

const NODE_IO: PinIo = {
  read: (file) => readFileSync(file, "utf8"),
  write: (file, text) => writeFileSync(file, text),
  report: (line) => process.stdout.write(`${line}\n`),
};

/**
 * The digest tables `text` declares at its top level, each with its string literals.
 *
 * @throws Error when the text does not parse (a conflict left in it), or declares no table or no
 *   top-level `digestOf`.
 */
export function writtenTables(file: string, text: string): ReadonlyArray<WrittenTable> {
  const parsed = parseSync(file, text, { lang: "ts" });
  const [error] = parsed.errors;
  if (error !== undefined) {
    throw new Error(`${file} does not parse (${error.message}): resolve it by hand first`);
  }
  const spans: Array<{ readonly name: string; readonly start: number; readonly end: number }> = [];
  let digestOf = false;
  for (const statement of parsed.program.body) {
    if (statement.type === "FunctionDeclaration" && statement.id?.name === "digestOf") {
      digestOf = true;
    }
    if (statement.type !== "VariableDeclaration") {
      continue;
    }
    for (const { id, init } of statement.declarations) {
      if (id.type === "Identifier" && init !== null && TABLE_NAME.test(id.name)) {
        spans.push({ name: id.name, start: init.start, end: init.end });
      }
      digestOf ||= id.type === "Identifier" && id.name === "digestOf";
    }
  }
  if (spans.length === 0 || !digestOf) {
    throw new Error(`${file} declares no top-level \`digestOf\` and \`<NAME>_DIGESTS\` table`);
  }
  const literals: WrittenLiteral[] = [];
  new Visitor({
    Literal(node) {
      if (typeof node.value === "string") {
        literals.push({ value: node.value, start: node.start, end: node.end });
      }
    },
  }).visit(parsed.program);
  literals.sort((a, b) => a.start - b.start);
  return spans.map(({ name, start, end }) => ({
    name,
    literals: literals.filter((literal) => literal.start >= start && literal.end <= end),
  }));
}

/**
 * Each table's rows' pins placed in `text`: a row's pin is the first of its table's string
 * literals, after the previous row's, whose value is the row's pinned digest.
 *
 * @throws Error when a row's pin is not written as a literal of its table, in the rows' order.
 */
export function placePins(
  file: string,
  text: string,
  tables: ReadonlyArray<WrittenTable>,
  rows: ReadonlyMap<string, ReadonlyArray<RowDigest>>,
): ReadonlyArray<Pin> {
  return tables.flatMap(({ name, literals }) => {
    let next = 0;
    return (rows.get(name) ?? []).map((row) => {
      const at = literals.findIndex(
        (literal, index) => index >= next && literal.value === row.pinned,
      );
      const literal = literals[at];
      if (literal === undefined || text.slice(literal.start + 1, literal.end - 1) !== row.pinned) {
        throw new Error(
          `${file}: ${name}'s ${row.label} pins ${row.pinned}, which is not a plain string literal ` +
            "of the table after the previous row's pin",
        );
      }
      next = at + 1;
      const line = text.slice(0, literal.start).split("\n").length;
      const { label, pinned, digest } = row;
      return {
        label,
        pinned,
        digest,
        table: name,
        line,
        start: literal.start + 1,
        end: literal.end - 1,
      };
    });
  });
}

/** `text` with each pin's digits replaced by its digest; every other character as it was. */
export function rewritePins(text: string, pins: ReadonlyArray<Pin>): string {
  let rewritten = text;
  for (const pin of pins.toSorted((a, b) => b.start - a.start)) {
    rewritten = rewritten.slice(0, pin.start) + pin.digest + rewritten.slice(pin.end);
  }
  return rewritten;
}

/** Serves the pinned file as `text` with its tables and `digestOf` exported, and `vitest` inert. */
function servedAsWritten(file: string, text: string, tables: ReadonlyArray<string>): Plugin {
  const ids = new Set([normalizePath(file), normalizePath(realpathSync(file))]);
  return {
    name: "hyperion:pin-digests",
    enforce: "pre",
    resolveId: (source) => (source === "vitest" ? VITEST_STUB : null),
    load: (id) => {
      if (id === VITEST_STUB) {
        return VITEST_STUB_CODE;
      }
      return ids.has(id) ? `${text}\nexport { digestOf, ${tables.join(", ")} };\n` : null;
    },
  };
}

function isDigestOf(value: unknown): value is (source: unknown) => unknown {
  return typeof value === "function";
}

/** A row's first element, or that element's `name`. */
function labelOf(row: ReadonlyArray<unknown>, index: number): string {
  const [first] = row;
  if (typeof first === "string") {
    return first;
  }
  if (typeof first === "object" && first !== null && "name" in first) {
    return typeof first.name === "string" ? first.name : `row ${index + 1}`;
  }
  return `row ${index + 1}`;
}

/** A table's rows, each digested by the file's `digestOf`. */
function rowDigests(
  file: string,
  name: string,
  table: unknown,
  digestOf: (source: unknown) => unknown,
): ReadonlyArray<RowDigest> {
  if (!Array.isArray(table)) {
    throw new Error(`${file}: ${name} is not an array`);
  }
  return table.map((row: unknown, index): RowDigest => {
    const pinned: unknown = Array.isArray(row) ? row.at(-1) : undefined;
    if (!Array.isArray(row) || row.length < 2 || typeof pinned !== "string") {
      throw new Error(`${file}: ${name}'s row ${index + 1} does not end with its pin, a string`);
    }
    const digest = digestOf(row.at(-2));
    if (typeof digest !== "string") {
      throw new Error(`${file}: digestOf gave ${name}'s row ${index + 1} no string`);
    }
    return { label: labelOf(row, index), pinned, digest };
  });
}

/**
 * The pins of the file `file`, whose text is `text`, each beside its source's digest now.
 *
 * @throws Error when the file does not load, or its tables are not as {@link writtenTables} and
 *   {@link placePins} require.
 */
export async function pinsOf(file: string, text: string): Promise<ReadonlyArray<Pin>> {
  const tables = writtenTables(file, text);
  const names = tables.map((table) => table.name);
  const { module } = await runnerImport<Record<string, unknown>>(file, {
    configFile: false,
    logLevel: "error",
    // The runner's environment loads every bare import natively; this one it serves, as the stub.
    environments: { inline: { resolve: { noExternal: ["vitest"] } } },
    plugins: [servedAsWritten(file, text, names)],
  });
  const digestOf = module["digestOf"];
  if (!isDigestOf(digestOf)) {
    throw new Error(`${file}: its \`digestOf\` is not a function`);
  }
  const rows = new Map(names.map((name) => [name, rowDigests(file, name, module[name], digestOf)]));
  return placePins(file, text, tables, rows);
}

/**
 * Checks the pins of `files`, or rewrites each stale one. Returns the exit code: 1 when a check
 * finds a stale pin.
 *
 * @throws Error when a file does not read or load, or its tables are not as {@link pinsOf}
 *   requires.
 */
export async function pinDigests(
  files: ReadonlyArray<string>,
  mode: "write" | "check",
  io: PinIo,
): Promise<number> {
  // Loaded side by side: each load waits on its own module runner's reads and transforms.
  const loaded = await Promise.all(
    files.map(async (file) => {
      const text = io.read(file);
      return { file, text, pins: await pinsOf(file, text) };
    }),
  );
  let count = 0;
  let stale = 0;
  for (const { file, text, pins } of loaded) {
    const moved = pins.filter((pin) => pin.pinned !== pin.digest);
    count += pins.length;
    stale += moved.length;
    for (const pin of moved) {
      io.report(
        `${mode === "write" ? "re-pinned" : "stale"} ${relative(process.cwd(), file)}:${pin.line} ` +
          `${pin.table} ${pin.label}: ${pin.pinned} -> ${pin.digest}`,
      );
    }
    if (mode === "write" && moved.length > 0) {
      io.write(file, rewritePins(text, moved));
    }
  }
  if (stale === 0) {
    io.report(`pin-digests: ${count} pins, all current`);
  } else if (mode === "write") {
    io.report(`pin-digests: ${count} pins, ${stale} re-pinned`);
  } else {
    io.report(`pin-digests: ${count} pins, ${stale} stale; run \`just pin-digests\``);
  }
  return mode === "check" && stale > 0 ? 1 : 0;
}

/** The command line: `[--check]`. Returns the exit code: 1 on a stale pin under `--check`, or on any failure. */
export async function main(args: ReadonlyArray<string>): Promise<number> {
  try {
    const { values } = parseArgs({
      args: [...args],
      options: { check: { type: "boolean", default: false } },
    });
    const files = PINNED_FILES.map((file) => join(CLIENT_ROOT, file));
    return await pinDigests(files, values.check ? "check" : "write", NODE_IO);
  } catch (error: unknown) {
    console.error(`pin-digests: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}
