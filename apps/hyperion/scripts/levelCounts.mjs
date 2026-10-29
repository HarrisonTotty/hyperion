// Runs R10.T4.b's counts of the selected patches (plan R10; signoff-12.md item 30), the harness its
// record kept by hand, rebuilt for R10.T15.b (decision-r10-item30 §6: item 30 at generator version
// 24 re-runs it on the re-blessed `level-bounds.json`). It reads the fixture, counts every world,
// rule, style and boxes over R05's descent (`src/renderer/src/view/terrain/levelBoundsCounts.ts`),
// and writes the record and its summary. Renderer code reads no files, so this script reads the
// fixture and writes the record, and runs the TypeScript through Vite's module runner, as
// `descentDemand.mjs` does. CPU only, by hand; not part of `ci` (about 6 minutes on the desktop at
// generator version 23, R10.T4.b's record).
//
//   node --no-warnings scripts/levelCounts.mjs [--out <dir>] [--worlds a,b] [--note <text>]...
//
// --out is where `<stamp>-level-counts.json` and `.md` go (default target/level-counts/); --worlds
// counts only the worlds named (the fixture's names, such as mars_like); each --note is a line of
// the summary's notes. The record is rewritten after each world, so a killed run keeps the worlds
// it ended.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");
const repo = join(app, "..", "..");

function option(args, name, fallback) {
  const at = args.indexOf(`--${name}`);
  return at >= 0 && at + 1 < args.length ? args[at + 1] : fallback;
}

/** Every value given to a repeatable option, in order. */
function options(args, name) {
  return args.flatMap((arg, at) =>
    arg === `--${name}` && at + 1 < args.length ? [args[at + 1]] : [],
  );
}

async function main(args) {
  const { module: counts } = await runnerImport(
    join(app, "src/renderer/src/view/terrain/levelBoundsCounts.ts"),
    { configFile: false, logLevel: "error" },
  );
  const fixture = JSON.parse(
    readFileSync(join(app, "src/renderer/src/view/terrain/fixtures/level-bounds.json"), "utf8"),
  );
  const out = option(args, "out", join(repo, "target/level-counts"));
  const named = option(args, "worlds", null);
  const wanted = named === null ? null : named.split(",");
  const worlds = fixture.worlds
    .filter((world) => wanted === null || wanted.includes(world.name))
    .map((world) => counts.sampleWorld(world));
  if (wanted !== null && worlds.length !== wanted.length) {
    process.stderr.write(
      `level counts: the fixture has ${fixture.worlds.map((world) => world.name).join(", ")}\n`,
    );
    return 2;
  }
  const startedAt = new Date().toISOString();
  const stem = `${startedAt.replaceAll(":", "-").replace(/\.\d+Z$/u, "Z")}-level-counts`;
  const notes = options(args, "note");
  const cells = [];
  mkdirSync(out, { recursive: true });
  const write = (done) => {
    const record = counts.levelCountsRecord({
      startedAt,
      generatorVersion: fixture.generator_version,
      worlds: done,
      cells,
      notes,
    });
    writeFileSync(join(out, `${stem}.json`), `${JSON.stringify(record)}\n`);
    writeFileSync(join(out, `${stem}.md`), counts.levelCountsMarkdown(record));
  };
  const done = [];
  for (const world of worlds) {
    const worldStart = performance.now();
    for (const rule of counts.COUNT_RULES) {
      for (const style of counts.COUNT_STYLES) {
        for (const boxes of counts.COUNT_BOXES) {
          const cellStart = performance.now();
          cells.push(counts.countCell({ world, rule, style, boxes }));
          process.stdout.write(
            `${world.name} ${rule} ${style.name} ${boxes}: ` +
              `${((performance.now() - cellStart) / 1000).toFixed(1)} s\n`,
          );
        }
      }
    }
    done.push(world);
    write(done);
    process.stdout.write(
      `${world.name}: ${((performance.now() - worldStart) / 1000).toFixed(0)} s\n`,
    );
  }
  process.stdout.write(`record: ${join(out, stem)}.{json,md}\n`);
  return 0;
}

process.exitCode = await main(process.argv.slice(2));
