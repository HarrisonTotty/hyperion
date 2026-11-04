// Plays the seamless probe's camera strides (plan R14, R14.T3.c;
// `src/renderer/src/view/terrain/seamlessProbe.wasm.test.ts`) over TERRAIN TEST's script in
// segments, and accepts them over the whole script from the segments' figures. A whole strided run
// took 4.4 h on high on the pod, past the lane rules' 4 h for a run that cannot resume
// (2026-11-01). Here each segment is a vitest run of its own, which plays the script from its start
// and measures the strides over the segment alone (`SEAM_STRIDES_FROM_S`, `SEAM_STRIDES_TO_S`), and
// its figures, a few kB of sums, counts and largest values, go to a file of their own as it ends.
// A stopped run resumes at the segments with no file, and the merge of the segments' figures is
// the whole run's bit for bit (the probe's "measures the camera's strides over a run's segments as
// over the whole run, bit for bit"). Renderer code reads no files, so this script keeps the files
// and the probe reads its segments' figures from `SEAM_SEGMENTS`. CPU only, by hand; not part of
// `ci`.
//
//   node scripts/seamSegments.mjs --setting high|low [--dir <dir>] [--segment-s 20] [--jobs 1]
//                                 [--to-s 240] [--timeout-s 7200]
//
// --dir holds the segments' files, `<setting>-<from>-<to>.json`, and the merge's,
// `<setting>-whole.json` (default target/seam-segments/); each vitest run's own report and output
// go to its `work/`. --segment-s is a segment's span of the script, s; --jobs how many segments
// play at once; --to-s the script's span, past which the last segment plays to the script's end;
// --timeout-s a segment's limit. The probe's other knobs (SEAM_COST_MS and the rest) pass through
// from the environment and a segment's file keeps them: the merge refuses a segment of other knobs,
// so clear the directory after changing one. It exits 0 where the strides pass over the whole
// script, 1 where they fail, and 2 where a segment did not end or the merge could not run.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");
const PROBE = "src/renderer/src/view/terrain/seamlessProbe.wasm.test.ts";

function option(args, name, fallback) {
  const at = args.indexOf(`--${name}`);
  return at >= 0 && at + 1 < args.length ? args[at + 1] : fallback;
}

function positive(args, name, fallback) {
  const value = Number(option(args, name, String(fallback)));
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`--${name} must be a positive number`);
  }
  return value;
}

/** The script's segments: `segmentS` long from 0, the last open to the script's end (`toS` null). */
function segmentsOf(spanS, segmentS) {
  const out = [];
  for (let k = 0; k * segmentS < spanS; k += 1) {
    const toS = (k + 1) * segmentS;
    out.push({ fromS: k * segmentS, toS: toS >= spanS ? null : toS });
  }
  return out;
}

function nameOf(setting, segment) {
  return `${setting}-${segment.fromS}-${segment.toS ?? "end"}`;
}

/** Writes `text` to `file` whole or not at all. */
function writeWhole(file, text) {
  writeFileSync(`${file}.partial`, text);
  renameSync(`${file}.partial`, file);
}

/** The children running, stopped with this script. */
const running = new Set();

/** Runs the probe's tests named `testName` with `env`, its report in `report`; the exit code. */
function runProbe(testName, env, report, log, timeoutS) {
  return new Promise((done) => {
    const child = spawn(
      "pnpm",
      [
        "exec",
        "vitest",
        "run",
        PROBE,
        "-t",
        testName,
        "--reporter=json",
        `--outputFile.json=${report}`,
      ],
      { cwd: app, env, stdio: ["ignore", "pipe", "pipe"] },
    );
    running.add(child);
    const chunks = [];
    child.stdout.on("data", (chunk) => chunks.push(chunk));
    child.stderr.on("data", (chunk) => chunks.push(chunk));
    const timer = setTimeout(() => child.kill("SIGTERM"), 1000 * timeoutS);
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      running.delete(child);
      writeFileSync(log, Buffer.concat(chunks));
      done(code ?? (signal === null ? 1 : 128));
    });
  });
}

/** The figures a probe report's test titled `prefix…` left, with its status, or null. */
function figuresIn(report, prefix) {
  if (!existsSync(report)) {
    return null;
  }
  const results = JSON.parse(readFileSync(report, "utf8")).testResults ?? [];
  for (const file of results) {
    for (const test of file.assertionResults ?? []) {
      const seamless = test.meta?.seamless;
      if (String(test.title).startsWith(prefix) && seamless?.strides !== undefined) {
        return { status: test.status, seamless };
      }
    }
  }
  return null;
}

/** The environment of a run of the probe: this one's, its knobs of the run's own set. */
function probeEnv(extra) {
  const env = { ...process.env, HYPERION_SLOW_TESTS: "1", SEAM_STRIDES: "measured", ...extra };
  for (const name of ["SEAM_STRIDES_FROM_S", "SEAM_STRIDES_TO_S", "SEAM_TO_S", "SEAM_SEGMENTS"]) {
    if (!(name in extra)) {
      delete env[name];
    }
  }
  return env;
}

/** Plays `segment` unless its file is there; whether its file is there after. */
async function playSegment(setting, segment, dir, work, timeoutS) {
  const name = nameOf(setting, segment);
  const file = join(dir, `${name}.json`);
  if (existsSync(file)) {
    return true;
  }
  const report = join(work, `${name}.vitest.json`);
  rmSync(report, { force: true });
  const started = Date.now();
  console.warn(`seamSegments: ${name} playing`);
  const code = await runProbe(
    "draws no collapse",
    probeEnv({
      SEAM_SETTING: setting,
      SEAM_STRIDES_FROM_S: String(segment.fromS),
      ...(segment.toS === null
        ? {}
        : { SEAM_STRIDES_TO_S: String(segment.toS), SEAM_TO_S: String(segment.toS) }),
    }),
    report,
    join(work, `${name}.log`),
    timeoutS,
  );
  const figures = figuresIn(report, "draws no collapse");
  const wallS = Math.round((Date.now() - started) / 1000);
  if (figures === null || figures.seamless.setting !== setting) {
    console.warn(`seamSegments: ${name} did not end (exit ${code} after ${wallS} s)`);
    return false;
  }
  const { knobs, strides } = figures.seamless;
  writeWhole(
    file,
    `${JSON.stringify({ setting, knobs, fromS: segment.fromS, toS: segment.toS, status: figures.status, wallS, strides }, null, 2)}\n`,
  );
  console.warn(
    `seamSegments: ${name} ${figures.status} after ${wallS} s: ${strides.frames} frames, moves ${strides.movePxMax.toFixed(3)} px, normals ${strides.normalDegMax.toFixed(3)} deg`,
  );
  return true;
}

async function main(args) {
  const setting = option(args, "setting", "");
  if (setting !== "high" && setting !== "low") {
    throw new Error("--setting must be high or low");
  }
  if (process.env["SEAM_FROM_S"] !== undefined) {
    throw new Error("SEAM_FROM_S starts the script cold, so its segments would not merge");
  }
  const dir = resolve(option(args, "dir", join(app, "target", "seam-segments")));
  const work = join(dir, "work");
  mkdirSync(work, { recursive: true });
  const segmentS = positive(args, "segment-s", 20);
  const jobs = Math.round(positive(args, "jobs", 1));
  const spanS = positive(args, "to-s", 240);
  const timeoutS = positive(args, "timeout-s", 7200);
  // The latest first: each plays the script from its start, so they take the longest.
  const queue = segmentsOf(spanS, segmentS).toReversed();
  const all = [...queue];
  const ends = [];
  await Promise.all(
    Array.from({ length: Math.min(jobs, queue.length) }, async () => {
      for (let segment = queue.shift(); segment !== undefined; segment = queue.shift()) {
        // One segment after another in each lane.
        // oxlint-disable-next-line no-await-in-loop
        ends.push(await playSegment(setting, segment, dir, work, timeoutS));
      }
    }),
  );
  if (ends.includes(false)) {
    console.warn("seamSegments: a segment did not end; run again to resume");
    return 2;
  }
  const segments = all.toReversed().map((segment) => {
    const { knobs, strides } = JSON.parse(
      readFileSync(join(dir, `${nameOf(setting, segment)}.json`), "utf8"),
    );
    return { setting, knobs, strides };
  });
  const report = join(work, `${setting}-whole.vitest.json`);
  rmSync(report, { force: true });
  const code = await runProbe(
    "from its segments",
    probeEnv({ SEAM_SETTING: setting, SEAM_SEGMENTS: JSON.stringify(segments) }),
    report,
    join(work, `${setting}-whole.log`),
    600,
  );
  const figures = figuresIn(report, "accepts the camera's strides");
  if (figures === null) {
    console.warn(`seamSegments: the merge did not run (exit ${code})`);
    return 2;
  }
  writeWhole(
    join(dir, `${setting}-whole.json`),
    `${JSON.stringify({ status: figures.status, ...figures.seamless }, null, 2)}\n`,
  );
  const { strides } = figures.seamless;
  console.warn(
    `seamSegments: ${setting} ${figures.status} over ${segments.length} segments: moves ${strides.movePxMax} px, normals ${strides.normalDegMax} deg, lit triangles ${strides.litTriangleShare}`,
  );
  return figures.status === "passed" ? 0 : 1;
}

function stopAll() {
  for (const child of running) {
    child.kill("SIGTERM");
  }
  process.exit(130);
}
process.on("SIGINT", stopAll);
process.on("SIGTERM", stopAll);

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(2);
  },
);
