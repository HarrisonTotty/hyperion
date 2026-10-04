// Runs the descent's demand record (R05.T13.a, `just descent-demand`): the fixed-step descent
// through `selectPatches` with height ranges from real bakes of the surface module, in cells of
// {hard, min(hard, 4σ_n)} × {ridges off, on} × {high, low}, and writes the record under
// `docs/measurements/descent-spike/`. Renderer code loads no module and writes no files, so this
// script does both and runs the TypeScript through Vite's module runner, as `placeShip.mjs` does.
//
//   node scripts/descentDemand.mjs [--rules hard,calibrated] [--ridges off,on] [--settings high,low]
//     [--rate 64] [--cap-hours 2] [--wall-cap-hours <h>] [--out <dir>] [--write-fixture]
//
// --write-fixture runs only the unit test's windows (ridges off, min(hard, 4σ_n), both settings)
// and writes the ranges the test reads, `src/renderer/src/view/spike/fixtures/descentRanges.txt`,
// printing each window's hash. The cap is shared evenly between the cells and counts selection
// time only, the bakes left out; `--wall-cap-hours` stops the whole run at that wall time, bakes
// included, so that a run under an outer timeout always writes. A cell cut short says so in the
// record, and the record is rewritten after every cell, so a killed run keeps the cells it ended.
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

const nowMs = () => performance.now();

async function main(args) {
  const surface = await import(join(app, "src/renderer/src/generated/surface/hyperion_surface.js"));
  surface.initSync({
    module: readFileSync(join(app, "src/renderer/src/generated/surface/hyperion_surface_bg.wasm")),
  });
  const { module: record } = await runnerImport(
    join(app, "src/renderer/src/view/spike/demandRecord.ts"),
    { configFile: false, logLevel: "error" },
  );
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
  const out = option(args, "out", join(repo, "docs/measurements/descent-spike"));
  const startedAt = new Date().toISOString();
  const cellCount = rules.length * ridgesList.length * settings.length;
  const shareMs = capMs / cellCount;
  const wallMs = Number(option(args, "wall-cap-hours", "Infinity")) * 3_600_000;
  const wallDeadlineMs = nowMs() + wallMs;
  const cells = [];
  mkdirSync(out, { recursive: true });
  const stem = `${startedAt.slice(0, 10)}-demand-${rules.join("+")}`;
  const write = () => {
    const json = {
      schema: record.DEMAND_RECORD_SCHEMA,
      version: record.DEMAND_RECORD_VERSION,
      startedAt,
      seed: String(record.RECORD_SEED),
      testPlanetVersion: surface.testPlanetVersion(),
      rateHz,
      capHours: capMs / 3_600_000,
      loadAverage: readFileSync("/proc/loadavg", "utf8").split(" ").slice(0, 3).map(Number),
      cells,
    };
    writeFileSync(join(out, `${stem}.json`), `${JSON.stringify(json, null, 2)}\n`);
    writeFileSync(join(out, `${stem}.md`), record.demandSummary(cells, startedAt));
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
          deadlineMs: cellStart + shareMs,
          wallDeadlineMs,
        });
        cells.push(cell);
        write();
        process.stdout.write(
          `${rule} ridges ${ridges} ${settingView.setting}: ${cell.frames} frames${cell.truncated === true ? " (truncated)" : ""} in ${((nowMs() - cellStart) / 1000).toFixed(0)} s\n`,
        );
      }
    }
  }
  write();
  process.stdout.write(`record: ${join(out, stem)}.{json,md}\n`);
  return 0;
}

process.exitCode = await main(process.argv.slice(2));
