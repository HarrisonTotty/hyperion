/**
 * `just perf-record <file>` (plan R12, R12.T1.b): validates a perf run's record, or folds one of
 * R05's descent-spike results files, appends it to `runs.v1.jsonl` and regenerates `budget.md`.
 *
 * @remarks
 * Run by Node's type stripping: `node packages/perf/src/cli/record.ts <file> [--store <path>]
 * [--budget <path>]`. Exits 0 with the IDs appended, 1 naming what was refused, 2 on a usage error.
 */

import { readFileSync, writeFileSync } from "node:fs";

import { PerfRecordError, parsePerfRecords } from "../record.ts";
import { appendRecords, recordsOfFile } from "../store.ts";
import { BudgetMarkerError, writeBudget } from "../table.ts";
import { BUDGET_PATH, isFileError, STORE_PATH, takeOption } from "./paths.ts";

function main(argv: ReadonlyArray<string>): number {
  const args = [...argv];
  let store: string;
  let budget: string;
  try {
    store = takeOption(args, "--store", STORE_PATH);
    budget = takeOption(args, "--budget", BUDGET_PATH);
  } catch (error) {
    process.stderr.write(
      `perf-record: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    return 2;
  }
  const [file, ...rest] = args;
  if (file === undefined || rest.length > 0) {
    process.stderr.write(
      "usage: perf-record <file> [--store <runs.v1.jsonl>] [--budget <budget.md>]\n",
    );
    return 2;
  }
  try {
    const records = recordsOfFile(readFileSync(file, "utf8"));
    const appended = appendRecords(readFileSync(store, "utf8"), records);
    if (appended.kind === "duplicate") {
      process.stderr.write(
        `perf-record: ${file} is already in the store, as ${appended.existing}\n`,
      );
      return 1;
    }
    const table = writeBudget(readFileSync(budget, "utf8"), parsePerfRecords(appended.text));
    writeFileSync(store, appended.text);
    writeFileSync(budget, table);
    process.stdout.write(
      `appended ${appended.ids.join(", ")} to ${store}; regenerated ${budget}\n`,
    );
    return 0;
  } catch (error) {
    if (error instanceof PerfRecordError) {
      process.stderr.write(`perf-record: refused: ${error.message}\n`);
      return 1;
    }
    if (error instanceof BudgetMarkerError || isFileError(error)) {
      process.stderr.write(`perf-record: ${error.message}\n`);
      return 1;
    }
    throw error;
  }
}

process.exitCode = main(process.argv.slice(2));
