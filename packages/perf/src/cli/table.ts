/**
 * `just perf-table` (plan R12, R12.T1.b): regenerates `budget.md`'s generated sections from
 * `runs.v1.jsonl`, leaving its prose as it is.
 *
 * @remarks
 * Run by Node's type stripping: `node packages/perf/src/cli/table.ts [--check] [--store <path>]
 * [--budget <path>]`. `--check` writes nothing and exits 1 if the table is out of date. Exits 1
 * naming a store or a table it cannot read, 2 on a usage error.
 */

import { readFileSync, writeFileSync } from "node:fs";

import { parsePerfRecords } from "../record.ts";
import { writeBudget } from "../table.ts";
import { BUDGET_PATH, STORE_PATH, takeFlag, takeOption } from "./paths.ts";

function main(argv: ReadonlyArray<string>): number {
  const args = [...argv];
  const check = takeFlag(args, "--check");
  let store: string;
  let budget: string;
  try {
    store = takeOption(args, "--store", STORE_PATH);
    budget = takeOption(args, "--budget", BUDGET_PATH);
  } catch (error) {
    process.stderr.write(`perf-table: ${error instanceof Error ? error.message : String(error)}\n`);
    return 2;
  }
  if (args.length > 0) {
    process.stderr.write(
      "usage: perf-table [--check] [--store <runs.v1.jsonl>] [--budget <budget.md>]\n",
    );
    return 2;
  }
  try {
    const before = readFileSync(budget, "utf8");
    const after = writeBudget(before, parsePerfRecords(readFileSync(store, "utf8")));
    if (check) {
      if (after === before) return 0;
      process.stderr.write(
        `perf-table: ${budget} is out of date with ${store}: run \`just perf-table\`\n`,
      );
      return 1;
    }
    if (after !== before) writeFileSync(budget, after);
    process.stdout.write(`${after === before ? "unchanged" : "regenerated"}: ${budget}\n`);
    return 0;
  } catch (error) {
    if (error instanceof Error) {
      process.stderr.write(`perf-table: ${error.message}\n`);
      return 1;
    }
    throw error;
  }
}

process.exitCode = main(process.argv.slice(2));
