// The height-worker sweep (plan R10, R10.T14.b's "Bakes under load", decision-r10-t14b-measure
// 6(c); R05 Design note 11's worker count): flies the descent spike once a worker count, each run
// with the sweep's one configuration and its own --workers, then writes the bake clock's table
// (`src/main/workerSweep.ts`: bakes a second, post to reply p50 and p95, never resident). Run it
// after `just build`, by hand; not part of `ci`. Each run is a whole descent, half an hour or more
// on a served world, so a run whose directory holds its results already is not flown again: a
// sweep stopped part way picks up at its next count, and `--table` writes the table from the runs
// held, flying none.
//
//   node --no-warnings scripts/workerSweep.mjs --workers 2,3,4,6 --machine <text> [--out <dir>]
//       [--table] -- <descentSpike.sh options...>
//
// --machine names the machine in the table's heading ("pod, 16 vCPU", or the target's). --out
// (default target/worker-sweep/) holds each run under workers-<n>/ and the table, worker-sweep.md.
// The options after `--` are the configuration, given to descentSpike.sh for every run with
// `--workers <n> --out <dir>/workers-<n>`, e.g. `--hidden --world served --sizes on --setting
// high`; --workers and --out are refused there. A sweep whose runs' configurations differ is
// refused when its table is written.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");
const repo = join(app, "..", "..");

/**
 * The value after `--<name>` in `args`, or `fallback`.
 *
 * @param {ReadonlyArray<string>} args
 * @param {string} name
 * @param {string | null} fallback
 * @returns {string | null}
 */
function option(args, name, fallback) {
  const at = args.indexOf(`--${name}`);
  return at >= 0 && at + 1 < args.length ? args[at + 1] : fallback;
}

/**
 * The results file a run's directory holds, the newest if several, or `null` for none.
 *
 * @param {string} dir
 * @returns {string | null}
 */
function resultsIn(dir) {
  let names;
  try {
    names = readdirSync(dir);
  } catch {
    // No directory: the run has not been flown.
    return null;
  }
  const files = names
    .filter((name) => name.endsWith(".json") && !name.endsWith("-sizes-rejected.json"))
    .map((name) => join(dir, name))
    .toSorted((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return files[0] ?? null;
}

/**
 * Flies the sweep's runs it does not hold, unless `--table`, and writes its table.
 *
 * @param {ReadonlyArray<string>} args
 * @returns {Promise<number>} The exit status.
 */
async function main(args) {
  const split = args.indexOf("--");
  const own = split >= 0 ? args.slice(0, split) : args;
  const configuration = split >= 0 ? args.slice(split + 1) : [];
  const workers = (option(own, "workers", "2,3,4,6") ?? "").split(",").map(Number);
  if (workers.some((n) => !Number.isInteger(n) || n < 1 || n > 64)) {
    console.error("workerSweep: --workers takes counts from 1 to 64, such as 2,3,4,6");
    return 2;
  }
  const machine = option(own, "machine", null);
  if (machine === null) {
    console.error('workerSweep: --machine names the machine, such as "pod, 16 vCPU"');
    return 2;
  }
  if (configuration.some((arg) => /^--(workers|out)(=|$)/.test(arg))) {
    console.error("workerSweep: --workers and --out are the sweep's own, not the configuration's");
    return 2;
  }
  const out = option(own, "out", join(repo, "target", "worker-sweep"));
  const table = own.includes("--table");
  const files = [];
  for (const n of workers) {
    const dir = join(out, `workers-${String(n)}`);
    let file = resultsIn(dir);
    if (file === null && !table) {
      mkdirSync(dir, { recursive: true });
      const run = spawnSync(
        "bash",
        [join(here, "descentSpike.sh"), ...configuration, "--workers", String(n), "--out", dir],
        { stdio: "inherit" },
      );
      if (run.status !== 0) {
        console.error(`workerSweep: the run at ${String(n)} workers ended ${String(run.status)}`);
        return 1;
      }
      file = resultsIn(dir);
    }
    if (file === null) {
      console.error(`workerSweep: no results at ${String(n)} workers in ${dir}`);
      return 1;
    }
    files.push(file);
  }
  const { module: sweep } = await runnerImport(join(app, "src/main/workerSweep.ts"), {
    configFile: false,
    logLevel: "error",
  });
  const swept = sweep.sweepOf(files.map((file) => JSON.parse(readFileSync(file, "utf8"))));
  if (swept.ok !== true) {
    console.error(`workerSweep: ${swept.problem}`);
    return 1;
  }
  const lines = [
    ...sweep.sweepTableLines(swept.rows, machine),
    "",
    "Runs, under the sweep's directory:",
    ...files.map((file) => `- ${relative(out, file)}`),
  ];
  writeFileSync(join(out, "worker-sweep.md"), `${lines.join("\n")}\n`);
  process.stdout.write(`${lines.join("\n")}\n`);
  return 0;
}

process.exitCode = await main(process.argv.slice(2));
