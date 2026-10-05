// Runs the descent's demand record (R05.T13.a, `just descent-demand`): the fixed-step descent
// through `selectPatches` with height ranges from real bakes of the surface module, in cells of
// {hard, min(hard, 4σ_n)} × {ridges off, on} × {high, low}, and writes the record under
// `docs/measurements/descent-spike/`. Renderer code loads no module and writes no files, so this
// script does both and runs the TypeScript through Vite's module runner, as `placeShip.mjs` does.
//
//   node scripts/descentDemand.mjs [--rules hard,calibrated] [--ridges off,on] [--settings high,low]
//     [--rate 64] [--cap-hours 2] [--wall-cap-hours <h>] [--out <dir>] [--note <text>]...
//     [--write-fixture] [--merge <a.json>,<b.json>,...]
//
// --write-fixture runs only the unit test's windows (ridges off, min(hard, 4σ_n), both settings)
// and writes the ranges the test reads, `src/renderer/src/view/spike/fixtures/descentRanges.txt`,
// printing each window's hash. The cap is shared evenly between the cells and counts selection
// time only, the bakes left out; `--wall-cap-hours` stops the whole run at that wall time, bakes
// included, so that a run under an outer timeout always writes. A cell cut short says so in the
// record, and the record is rewritten after every cell, so a killed run keeps the cells it ended.
// Each --note is a line of the record's notes. --merge runs nothing: it writes one record into
// --out from the records given, their cells in that order, as the cells run one process each.
// Selection is timed on the wall clock and on this thread's CPU clock (`process.threadCpuUsage`):
// under load the first counts the waits for a core, the second the selection's own work.
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

/** Writes a record's JSON and its Markdown summary into `out`, as `stem`. */
function writeRecord(record, out, stem, file) {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, `${stem}.json`), `${JSON.stringify(file, null, 2)}\n`);
  writeFileSync(
    join(out, `${stem}.md`),
    record.demandSummary(file.cells, file.startedAt, file.notes),
  );
  return join(out, stem);
}

const loadAverage = () => readFileSync("/proc/loadavg", "utf8").split(" ").slice(0, 3).map(Number);

const nowMs = () => performance.now();

/**
 * This thread's CPU time, user and system, ms: what the selection itself costs under load. Node
 * before 23.9 lacks `process.threadCpuUsage`; the record's CPU times are then null.
 */
const cpuNowMs =
  typeof process.threadCpuUsage === "function"
    ? () => {
        const { user, system } = process.threadCpuUsage();
        return (user + system) / 1000;
      }
    : undefined;

async function main(args) {
  const surface = await import(join(app, "src/renderer/src/generated/surface/hyperion_surface.js"));
  surface.initSync({
    module: readFileSync(join(app, "src/renderer/src/generated/surface/hyperion_surface_bg.wasm")),
  });
  const { module: record } = await runnerImport(
    join(app, "src/renderer/src/view/spike/demandRecord.ts"),
    { configFile: false, logLevel: "error" },
  );
  const notes = options(args, "note");
  const out = option(args, "out", join(repo, "docs/measurements/descent-spike"));
  const merge = option(args, "merge", null);
  if (merge !== null) {
    const files = merge.split(",").map((path) => JSON.parse(readFileSync(path, "utf8")));
    const merged = record.mergeRecords(files, notes);
    const stem = record.recordStem(merged.startedAt, record.recordRules(merged));
    const path = writeRecord(record, out, stem, merged);
    process.stdout.write(`merged: ${path}.{json,md}\n`);
    return 0;
  }
  const ridgesOf = { off: surface.Ridges.Off, on: surface.Ridges.On };
  const sourceOf = (ridges) => {
    const r = ridgesOf[ridges];
    const baked = record.memoised((key) => {
      const bake = surface.bakePatch(
        key.face,
        key.level,
        key.i,
        key.j,
        surface.VertexPath.FaceDifferences,
        surface.NormalScale.Mesh,
        r,
        0,
      );
      const range = bake.heightRangeM();
      bake.free();
      return [range[0], range[1]];
    });
    return {
      baked,
      source: {
        levelTable: surface.levelTable(r),
        omittedSigmaM: Array.from({ length: 25 }, (_, level) => surface.omittedSigmaM(level, r)),
        rangeOf: baked.rangeOf,
        surfaceHeightM: ([x, y, z]) => surface.surfaceHeightM(x, y, z, r),
      },
    };
  };

  if (args.includes("--write-fixture") === true) {
    const { baked, source } = sourceOf("off");
    const profile = record.recordProfile();
    for (const settingView of record.SETTING_VIEWS) {
      for (const window of record.testWindows(profile)) {
        const cell = record.runCell({
          rule: "calibrated",
          ridges: "off",
          settingView,
          source,
          rateHz: 64,
          fromS: window.fromS,
          toS: window.toS,
          measureFromS: window.measureFromS,
          nowMs,
          ...(cpuNowMs === undefined ? {} : { cpuNowMs }),
          deadlineMs: Infinity,
        });
        process.stdout.write(
          `${settingView.setting}\t${window.segment}\t${cell.hash}\t${JSON.stringify(cell.segments)}\n`,
        );
      }
    }
    const path = join(app, "src/renderer/src/view/spike/fixtures/descentRanges.txt");
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(
      path,
      record.formatRanges({
        levelTable: source.levelTable,
        omittedSigmaM: source.omittedSigmaM,
        siteHeightM: record.measureTerrain(record.hardPlanet(source.levelTable), source)
          .siteHeightM,
        ranges: baked.ranges,
      }),
    );
    process.stdout.write(`fixture: ${path} (${baked.ranges.size} keys)\n`);
    return 0;
  }

  const rules = option(args, "rules", "hard,calibrated").split(",");
  const ridgesList = option(args, "ridges", "off,on").split(",");
  const settings = option(args, "settings", "high,low").split(",");
  const rateHz = Number(option(args, "rate", "64"));
  const capMs = Number(option(args, "cap-hours", "2")) * 3_600_000;
  const startedAt = new Date().toISOString();
  const cellCount = rules.length * ridgesList.length * settings.length;
  const shareMs = capMs / cellCount;
  const wallCapHours = Number(option(args, "wall-cap-hours", "Infinity"));
  const wallDeadlineMs = nowMs() + wallCapHours * 3_600_000;
  const cells = [];
  const stem = record.recordStem(startedAt, rules);
  let path = "";
  const write = () => {
    path = writeRecord(record, out, stem, {
      schema: record.DEMAND_RECORD_SCHEMA,
      version: record.DEMAND_RECORD_VERSION,
      startedAt,
      seed: String(record.RECORD_SEED),
      testPlanetVersion: surface.testPlanetVersion(),
      notes,
      cells,
    });
  };
  for (const ridges of ridgesList) {
    // One memo a ridges value: the bakes are the same patches whatever the rule and setting.
    const { source } = sourceOf(ridges);
    const durationS = record.recordProfile().durationS;
    for (const rule of rules) {
      for (const settingView of record.SETTING_VIEWS.filter((s) => settings.includes(s.setting))) {
        const cellStart = nowMs();
        const cell = record.runCell({
          rule,
          ridges,
          settingView,
          source,
          rateHz,
          fromS: 0,
          toS: durationS,
          measureFromS: 1,
          nowMs,
          ...(cpuNowMs === undefined ? {} : { cpuNowMs }),
          deadlineMs: cellStart + shareMs,
          wallDeadlineMs,
        });
        const wallS = (nowMs() - cellStart) / 1000;
        cells.push({
          ...cell,
          capHours: shareMs / 3_600_000,
          wallCapHours: Number.isFinite(wallCapHours) ? wallCapHours : null,
          wallS,
          loadAverage: loadAverage(),
        });
        write();
        process.stdout.write(
          `${rule} ridges ${ridges} ${settingView.setting}: ${cell.frames} frames${cell.truncated === true ? " (truncated)" : ""} in ${wallS.toFixed(0)} s\n`,
        );
      }
    }
  }
  write();
  process.stdout.write(`record: ${path}.{json,md}\n`);
  return 0;
}

process.exitCode = await main(process.argv.slice(2));
