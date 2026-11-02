// Runs plan R14, T5.b's acceptance by hand: a scripted fly-by of 100 instances of
// `tools/meshbuild`'s procedural test meshes (the icosphere, the torus knot and the terrain patch;
// no authored mesh), each frame's cut checked for holes, overlaps and cracks over the bytes the
// page pool wrote, at the high and low settings' `pageBytes` and at a pool too small for every
// page wanted (`src/renderer/src/test/meshFlyby.ts`). Renderer code reads no files, so
// this script builds the meshes' files with the tool, reads them, runs the fly-by through Vite's
// module runner, as `clusterCounts.mjs` does, and writes the record and its summary. CPU only, by
// hand; not part of `ci` and no justfile recipe (which would plan the full `just ci`).
//
//   node --no-warnings scripts/meshFlyby.mjs [--frames <n>] [--pages <dir>] [--out <dir>]
//
// --frames is the flight's frames (default 300); --pages is where the mesh files are built
// (default target/mesh-flyby/pages); --out is where `<stamp>-mesh-flyby.json` and `.md` go
// (default target/mesh-flyby/).
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");
const repo = join(app, "..", "..");

const MESHES = ["icosphere", "torus-knot", "terrain"];
const MIB = 2 ** 20;

function option(args, name, fallback) {
  const at = args.indexOf(`--${name}`);
  return at >= 0 && at + 1 < args.length ? args[at + 1] : fallback;
}

/** Builds each test mesh's file with the tool, as `just _meshbuild-cargo` runs it. */
function buildMeshes(pages) {
  mkdirSync(pages, { recursive: true });
  return MESHES.map((name) => {
    const file = join(pages, `${name}.hmsh`);
    execFileSync(
      "cargo",
      [
        "run",
        "--locked",
        "--release",
        "--quiet",
        "--manifest-path",
        join(repo, "tools/meshbuild/Cargo.toml"),
        "--target-dir",
        join(repo, "target/tools-meshbuild"),
        "--",
        `procedural:${name}`,
        file,
      ],
      { stdio: ["ignore", "inherit", "inherit"] },
    );
    const bytes = readFileSync(file);
    return {
      name,
      bytes: bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
    };
  });
}

function markdown(record) {
  const rows = record.runs.map(
    (r) =>
      `| ${r.pool} | ${(r.poolBytes / MIB).toFixed(0)} MiB | ${r.rootSlots} + ${r.pageSlots} | ${(r.peakPoolBytes / MIB).toFixed(2)} MiB | ${r.holes} | ${r.overlaps} | ${r.cracks} | ${r.streaming} | ${r.fetched} | ${r.stored} | ${r.evicted} | ${r.refused} | ${r.trianglesP50} (${r.trianglesMax}) | ${r.seconds.toFixed(1)} s |`,
  );
  return [
    "# Mesh pages' fly-by (plan R14, R14.T5.b)",
    "",
    `Started ${record.startedAt}; ${record.instances} instances of ${record.meshes.map((m) => `${m.name} (${m.bytes} B, ${m.pages} pages)`).join(", ")}; ${record.frames} frames at τ = 1 px of 1920 across 60°, pages arriving ${record.latencyFrames} frames after they are asked for.`,
    "",
    "Holes, overlaps and cracks count instance-frames; streaming counts instance-frames with a wanted page not yet resident, drawn by its stand-in.",
    "",
    "| Pool | Bytes | Slots (root + page) | Peak bytes | Holes | Overlaps | Cracks | Streaming | Fetched | Stored | Evicted | Refused | Triangles a frame, median (largest) | Time |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...rows,
    "",
  ].join("\n");
}

async function main(args) {
  const frames = Number(option(args, "frames", "300"));
  if (!Number.isInteger(frames) || frames < 2) {
    process.stderr.write("mesh fly-by: --frames is a whole number of at least 2\n");
    return 2;
  }
  const out = option(args, "out", join(repo, "target/mesh-flyby"));
  const meshes = buildMeshes(option(args, "pages", join(out, "pages")));
  const runner = { configFile: false, logLevel: "error" };
  const { module: flyby } = await runnerImport(
    join(app, "src/renderer/src/test/meshFlyby.ts"),
    runner,
  );
  const startedAt = new Date().toISOString();
  const stem = `${startedAt.replaceAll(":", "-").replace(/\.\d+Z$/u, "Z")}-mesh-flyby`;
  const latencyFrames = 2;
  const record = {
    startedAt,
    instances: 100,
    frames,
    latencyFrames,
    meshes: meshes.map((m) => {
      const read = flyby.readMeshFile(m.bytes);
      return {
        name: m.name,
        bytes: m.bytes.byteLength,
        pages: read.kind === "read" ? read.value.index.pages.length : 0,
      };
    }),
    runs: [],
  };
  mkdirSync(out, { recursive: true });
  for (const [pool, poolBytes, layout] of [
    ["high", 128 * MIB, "alternating"],
    ["low", 32 * MIB, "alternating"],
    ["pressed", 4 * MIB, "alternating"],
    ["pressed, each mesh in a run", 4 * MIB, "runs"],
  ]) {
    const start = performance.now();
    // The runs are timed one after another, not side by side.
    // oxlint-disable-next-line eslint/no-await-in-loop
    const result = await flyby.runMeshFlyby({
      meshes,
      instances: record.instances,
      frames,
      poolBytes,
      toleranceRad: (2 * Math.tan(Math.PI / 6)) / 1920,
      latencyFrames,
      spacingM: 30,
      passM: 20,
      cracks: true,
      layout,
    });
    record.runs.push({ pool, ...result, seconds: (performance.now() - start) / 1000 });
    writeFileSync(join(out, `${stem}.json`), `${JSON.stringify(record, null, 2)}\n`);
    writeFileSync(join(out, `${stem}.md`), markdown(record));
    process.stdout.write(
      `${pool}: ${result.holes} holes, ${result.overlaps} overlaps, ${result.cracks} cracks, peak ${result.peakPoolBytes} of ${poolBytes} B\n`,
    );
  }
  process.stdout.write(`record: ${join(out, stem)}.{json,md}\n`);
  const whole = record.runs.every(
    (r) => r.holes === 0 && r.overlaps === 0 && r.cracks === 0 && r.peakPoolBytes <= r.poolBytes,
  );
  return whole ? 0 : 1;
}

process.exitCode = await main(process.argv.slice(2));
