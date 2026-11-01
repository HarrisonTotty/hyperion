// Runs plan R14's counting harness (Design note 18; R14.T1.f): triangles a frame by pass over R05's
// descent on R10.T4.b's sample worlds, today's path against each lever as built
// (`src/renderer/src/view/clusters/clusterCounts.ts`, on `levelBoundsCounts.ts`). Renderer code
// reads no files, so this script reads the fixture, runs the cells through Vite's module runner, as
// `levelCounts.mjs` does, and writes the record and its summary. CPU only, by hand; not part of
// `ci` and no justfile recipe (which would plan the full `just ci`).
//
//   node --no-warnings scripts/clusterCounts.mjs [--setting high|low] [--worlds a,b] [--out <dir>]
//                                                [--note <text>]...
//
// --setting counts one setting (both by default); --worlds the fixture's worlds named (default the
// research's three: earth_like, mars_like, moon_like); --out is where `<stamp>-cluster-counts.json`
// and `.md` go (default target/cluster-counts/); each --note is a line of the summary's notes. The
// record is rewritten after each cell, so a killed run keeps the cells it ended.
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
  const runner = { configFile: false, logLevel: "error" };
  const { module: counts } = await runnerImport(
    join(app, "src/renderer/src/view/clusters/clusterCounts.ts"),
    runner,
  );
  const { module: levels } = await runnerImport(
    join(app, "src/renderer/src/view/terrain/levelBoundsCounts.ts"),
    runner,
  );
  const fixture = JSON.parse(
    readFileSync(join(app, "src/renderer/src/view/terrain/fixtures/level-bounds.json"), "utf8"),
  );
  const out = option(args, "out", join(repo, "target/cluster-counts"));
  const wanted = option(args, "worlds", "earth_like,mars_like,moon_like").split(",");
  const setting = option(args, "setting", null);
  if (setting !== null && setting !== "high" && setting !== "low") {
    process.stderr.write("cluster counts: --setting is high or low\n");
    return 2;
  }
  const settings = setting === null ? ["high", "low"] : [setting];
  const worlds = fixture.worlds
    .filter((world) => wanted.includes(world.name))
    .map((world) => levels.sampleWorld(world));
  if (worlds.length !== wanted.length) {
    process.stderr.write(
      `cluster counts: the fixture has ${fixture.worlds.map((world) => world.name).join(", ")}\n`,
    );
    return 2;
  }
  const startedAt = new Date().toISOString();
  const stem = `${startedAt.replaceAll(":", "-").replace(/\.\d+Z$/u, "Z")}-cluster-counts`;
  const notes = options(args, "note");
  const cells = [];
  mkdirSync(out, { recursive: true });
  for (const world of worlds) {
    for (const each of settings) {
      const cellStart = performance.now();
      cells.push(counts.countClusterCell({ world, setting: each }));
      const record = counts.clusterCountsRecord({
        startedAt,
        generatorVersion: fixture.generator_version,
        cells,
        notes,
      });
      writeFileSync(join(out, `${stem}.json`), `${JSON.stringify(record)}\n`);
      writeFileSync(join(out, `${stem}.md`), counts.clusterCountsMarkdown(record));
      process.stdout.write(
        `${world.name} ${each}: ${((performance.now() - cellStart) / 1000).toFixed(1)} s\n`,
      );
    }
  }
  process.stdout.write(`record: ${join(out, stem)}.{json,md}\n`);
  return 0;
}

process.exitCode = await main(process.argv.slice(2));
