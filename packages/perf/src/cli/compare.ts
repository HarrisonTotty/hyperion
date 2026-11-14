/**
 * `just perf-compare <id>` (plan R12, R12.T6): states a record against the last comparable one in
 * `runs.v1.jsonl`, or against `--against <id>`, metric by metric, with its effect size.
 *
 * @remarks
 * Run by Node's type stripping: `node packages/perf/src/cli/compare.ts <id> [--against <id>]
 * [--store <path>]`. Exits 0 for a comparison whatever its verdicts, a regression included, and for
 * a scene's first record; 1 for a refusal, a record not in the store or a store it cannot read; 2
 * on a usage error.
 */

import { readFileSync } from "node:fs";

import { perfCompare } from "../compare.ts";
import { isFileError, STORE_PATH, takeOption } from "./paths.ts";

function main(argv: ReadonlyArray<string>): number {
  const args = [...argv];
  let store: string;
  let against: string;
  try {
    store = takeOption(args, "--store", STORE_PATH);
    against = takeOption(args, "--against", "");
  } catch (error) {
    process.stderr.write(
      `perf-compare: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    return 2;
  }
  const [id, ...rest] = args;
  if (id === undefined || rest.length > 0) {
    process.stderr.write("usage: perf-compare <id> [--against <id>] [--store <runs.v1.jsonl>]\n");
    return 2;
  }
  let text: string;
  try {
    text = readFileSync(store, "utf8");
  } catch (error) {
    if (!isFileError(error)) throw error;
    process.stderr.write(`perf-compare: ${error.message}\n`);
    return 1;
  }
  const outcome = perfCompare(text, id, against.length > 0 ? against : null);
  (outcome.exitCode === 0 ? process.stdout : process.stderr).write(outcome.text);
  return outcome.exitCode;
}

process.exitCode = main(process.argv.slice(2));
