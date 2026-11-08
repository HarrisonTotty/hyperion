// The owner's Mac batch (plans R14, R08 and R10; `src/main/macProfile.ts`): every pending Mac run as
// one command, which a stop does not lose. It builds the client and a release server, starts the
// server, places the ship at body 41ffecb200000002.0400 with the scene's clock paused, warms each
// setting's sky, then runs `viewsCheck.sh --inspection --timing --hidden --hyperion-gpu-timing` over
// the matrix (each setting at DPR 1 and 2; each lever pair at high as A B A B; the shading pairs,
// alone and with the occlusion, at both ratios; then the counted runs), then the height-worker
// sweep (`workerSweep.mjs`), and writes one summary. Run it by hand on a quiet machine; not part of
// `ci`.
//
//   node --no-warnings scripts/macProfile.mjs --machine <text> [--body <id>] [--out <dir>]
//       [--load-bound <n>] [--settings high,low|none] [--dprs 1,2] [--levers <names>|all|none] [--pair-setting high]
//       [--rounds 2] [--no-count] [--sweep-workers 3,4,6,8 | --no-sweep] [--port 7880] [--software] [--summary]
//
// --machine names the machine in the summary and the sweep's table ("Apple M5, 16 GiB"). --body
// names the served body every launch inspects, 41ffecb200000002.0400 by default; a batch of another
// body, such as a routed one's (a medium within the thick or the haze route's range) for the thick
// read's limb-rays and march-sources pairs, takes an --out of its own. The second batch, after the
// first, is the decked air's, the served world 42002cae00000004.0300 (plan R08,
// fix-r08-decked-air-route), whose air alone the decked-air-surface pair's sides draw apart. The
// first batch leaves that pair out (`MAC_PROFILE_BODY_LEVERS`: the default body has no deck, so its
// sides draw alike); this batch names it:
//
//   node --no-warnings scripts/macProfile.mjs --machine <text> --body 42002cae00000004.0300
//       --out target/mac-profile-decked --settings none --no-sweep
//       --levers surface-lobes,decked-air-surface,limb-rays,march-sources
//
// --out
// (default target/mac-profile/; a relative path is the shell's, made absolute once as the batch
// starts, so the server's data and every launch's files land under the one directory) holds the batch: each kept run under runs/<id>/ (its timing file and
// summary, its captures, the views check's output and run.json, its samples), a run that wrote no
// timing file under failed/, the warm-ups under warm-up/, the server's log and data, the sweep under
// worker-sweep/ and summary.md. A run is kept once its timing file is moved into place, its record
// written and its directory renamed to its ID, whatever the views check's exit, which the record
// and the summary give with its FAIL line: a check failing after the windows would fail again. So a
// batch stopped at any point (Ctrl-C, a crash, a restart) resumes at its first run not kept when
// the same command is given again: nothing kept is run again, and the run that was stopped is run
// again whole; delete runs/<id>/ to make a kept run again. The batch's tree is recorded as it
// starts, and a resume on another commit is refused, since its runs would not pair with those
// kept. --summary writes the summary from the runs kept, running nothing.
//
// The sky: the server cancels a connection's requests when it closes, and keeps a sky's reply for
// its next asking only once the reply is final, which for a camera near the Sun takes about an hour
// of an M5's four workers, far longer than a launch. So the ship is placed with the clock paused
// (`--rate 0`), every launch then asking its setting's sky at the same time and place, and for each
// setting with runs to make, a warm-up launch (the inspection without its profile) connects through
// a relay that holds its connection open after it leaves (`src/main/holdingRelay.ts`), until the
// server logs that sky's final reply. Every measured launch's sky is then answered from the kept
// reply, and the server is quiet beside its windows; run.json counts the skies each run computed.
// Before each run the batch waits, never stopping, until no sky is in flight and the 1-minute load is
// under --load-bound (default 4), saying every minute what it waits on. The server keeps its final
// skies under its --data-dir (R06.T11.n), the batch's `server-data/`, so a resumed batch's warm-ups
// find them there. Its sky's later jobs run at `--sky-qos utility`: on macOS the default,
// `background`, puts them on the efficiency cores, and the warm-ups wait for the final reply. The
// measured runs read the kept skies, so the class moves no figure.
//
// Each run's sample, before and after: the 1-minute load, the server's `ps` %CPU and CPU time (the
// summary gives its mean over the run) and `pmset -g therm` on macOS. The sweep's runs are sampled
// too, without the server, which is stopped first: each descent starts its own. --levers takes the
// names of `MAC_PROFILE_LEVERS` (limb-rays, march-sources, surface-lobes, decked-air-surface,
// lit-per-fragment, composite-per-pixel, occlusion, strides, far-cascades, shading,
// shading-occlusion, pacing-120); with none given it runs every pair but decked-air-surface, and
// --levers all runs every pair; --settings none and --levers none drop those runs. pacing-120
// (R07.T19) runs both sides on a 120 Hz offscreen frame clock (`--frame-rate 120`), frames counted
// (`--frame-pacing display`) against each view at its own rate (`rate`), each window's callbacks,
// draws and measured vsync in the windows' table.
// --software (Linux only) runs every launch on SwiftShader, as a pod's dry run does. In a pod the
// 1-minute load is the node's, not the pod's, so give it --load-bound 10000; a dry run is one pair
// at low, once (macProfile.test.ts's DRY_RUN):
//
//   node --no-warnings scripts/macProfile.mjs --machine pod --software --load-bound 10000
//       --settings none --levers limb-rays --pair-setting low --rounds 1 --no-count --no-sweep
//
// --port is the batch's server's (7880 by default, apart from a developer's server and the
// descent spike's).
//
// The counted runs (R14.T2.e) come after the pairs, apart from them so that the count costs their
// times nothing: each setting of --settings its own way, and each side of the pairs that split a
// pass (occlusion, shading and both together) at the pair setting where --levers holds them, all at
// DPR 1 with `--count-gpu-calls`, which counts every render pass's attachments and the bytes a
// tile-based GPU loads and stores for them. The summary gives their bytes by pass beside those
// pairs' deltas, and every window's atmosphere read (thin, thick, deck, beneath a deck, or a
// thermal form), from the kernels its frames dispatched. --no-count drops the counted runs.
import { spawn, spawnSync } from "node:child_process";
import {
  cpSync,
  createWriteStream,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { loadavg } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");
const repo = join(app, "..", "..");

/** The address of the batch's server, and of its relay. */
const LOCAL = "127.0.0.1";

/** The body every launch inspects unless --body names another: the owner's profile's since 11-01. */
const DEFAULT_BODY = "41ffecb200000002.0400";

/** Where the views check writes an inspection's timing files and its captures. */
const TIMING_DIR = join(repo, "target", "views-check", "timing");
const CAPTURE_DIR = join(repo, "target", "views-check", "inspection");

/** How the views check begins the line that says why a launch failed. */
const FAIL_PREFIX = "views check: FAIL ";

/** How often a wait looks again, and how often it says what it waits on, ms. */
const POLL_MS = 5_000;
const REPORT_MS = 60_000;

/** The longest the server may take to listen, and a stopped child to end before it is killed, ms. */
const LISTEN_MS = 120_000;
const STOP_MS = 10_000;

/**
 * The value after `--<name>` in `args`, or `fallback`.
 *
 * @param {ReadonlyArray<string>} args
 * @param {string} name
 * @param {string} fallback
 * @returns {string}
 */
function option(args, name, fallback) {
  const at = args.indexOf(`--${name}`);
  return (at >= 0 ? args.at(at + 1) : undefined) ?? fallback;
}

/**
 * A comma-separated list, or none for `none`.
 *
 * @param {string} text
 * @returns {string[]}
 */
function listOf(text) {
  return text === "none" ? [] : text.split(",").filter((each) => each.length > 0);
}

/**
 * Says something on standard error, stamped with the time.
 *
 * @param {string} text
 */
function say(text) {
  console.error(`mac profile ${new Date().toISOString().slice(11, 19)}: ${text}`);
}

/**
 * Looks at `check` now and every `everyMs` until it gives other than `undefined`.
 *
 * @template T
 * @param {() => T | undefined} check Throws to stop looking.
 * @param {number} everyMs
 * @returns {Promise<T>} What `check` gave.
 */
function poll(check, everyMs) {
  return new Promise((resolve, reject) => {
    const timer = setInterval(look, everyMs);
    function look() {
      try {
        const found = check();
        if (found !== undefined) {
          clearInterval(timer);
          resolve(found);
        }
      } catch (error) {
        clearInterval(timer);
        reject(error);
      }
    }
    look();
  });
}

/**
 * Calls `step` on each of `items` in turn, each once the one before it has settled: the batch runs
 * one launch at a time.
 *
 * @template T
 * @param {ReadonlyArray<T>} items
 * @param {(item: T) => Promise<void>} step
 * @returns {Promise<void>}
 */
function inTurn(items, step) {
  return items.reduce((before, item) => before.then(() => step(item)), Promise.resolve());
}

/** Whether the batch is stopping: once it is, no step goes on past the launch it stopped. */
let stopping = false;

/** Never settles: where a step waits, once the batch is stopping, for the stop to end the process. */
const HALTED = new Promise(() => {});

/** The children running, each its own process group, stopped with the batch. */
const children = new Set();

/**
 * Starts `command` in a process group of its own, so that the batch's stop ends it and all it
 * started, and the terminal's Ctrl-C reaches the batch alone.
 *
 * @param {string} command
 * @param {ReadonlyArray<string>} args
 * @param {import("node:child_process").SpawnOptions} options
 */
function start(command, args, options) {
  const child = spawn(command, args, { ...options, detached: true });
  children.add(child);
  child.on("exit", () => children.delete(child));
  return child;
}

/**
 * Runs `command` to its end, its output appended to `log`.
 *
 * @param {string} command
 * @param {ReadonlyArray<string>} args
 * @param {string} log
 * @param {string} cwd
 * @returns {Promise<number>} Its exit status, 128 and more where a signal ended it.
 */
async function run(command, args, log, cwd) {
  const out = createWriteStream(log, { flags: "a" });
  const child = start(command, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
  child.stdout?.pipe(out, { end: false });
  child.stderr?.pipe(out, { end: false });
  const status = await new Promise((resolve) => {
    child.on("error", () => resolve(127));
    child.on("close", (code, signal) => resolve(code ?? (signal === "SIGKILL" ? 137 : 143)));
  });
  await new Promise((resolve) => out.end(resolve));
  if (stopping) {
    await HALTED;
  }
  return status;
}

/**
 * Ends every child's process group: TERM, then KILL for those left after {@link STOP_MS}.
 *
 * @returns {Promise<void>}
 */
async function stopChildren() {
  const signal = (name) => {
    for (const child of children) {
      try {
        process.kill(-child.pid, name);
      } catch {
        // The group has gone already.
      }
    }
  };
  signal("SIGTERM");
  const until = Date.now() + STOP_MS;
  await poll(() => (children.size === 0 || Date.now() > until ? true : undefined), 200);
  signal("SIGKILL");
}

/**
 * Writes `value` as JSON to `path`, through a file beside it, so that a stop never leaves half.
 *
 * @param {string} path
 * @param {unknown} value
 */
function writeJson(path, value) {
  writeFileSync(`${path}.tmp`, `${JSON.stringify(value, null, 2)}\n`);
  renameSync(`${path}.tmp`, path);
}

/**
 * The files and directories of `dir` by name with their modification times, none where it does not
 * exist: an inspection launch writes its timing files, and its captures in a directory of their own.
 *
 * @param {string} dir
 * @returns {Map<string, number>}
 */
function entriesIn(dir) {
  if (!existsSync(dir)) {
    return new Map();
  }
  return new Map(
    readdirSync(dir, { withFileTypes: true })
      .filter((entry) => entry.isFile() || entry.isDirectory())
      .map((entry) => [entry.name, statSync(join(dir, entry.name)).mtimeMs]),
  );
}

/**
 * Moves a file or directory, copying it where `to` is on another volume than `from`.
 *
 * @param {string} from
 * @param {string} to
 */
function moveEntry(from, to) {
  try {
    renameSync(from, to);
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "EXDEV")) {
      throw error;
    }
    cpSync(from, to, { recursive: true });
    rmSync(from, { recursive: true, force: true });
  }
}

/**
 * Moves the files and directories of `dir` written since `before` into `into`.
 *
 * @param {string} dir
 * @param {Map<string, number>} before
 * @param {string} into
 * @returns {string[]} The names moved.
 */
function moveWritten(dir, before, into) {
  const moved = [...entriesIn(dir)].filter(([name, mtimeMs]) => before.get(name) !== mtimeMs);
  for (const [name] of moved) {
    moveEntry(join(dir, name), join(into, name));
  }
  return moved.map(([name]) => name);
}

/**
 * The machine now: the load, the server's CPU by `ps` and the thermal state by `pmset`.
 *
 * @param {number | null} serverPid
 * @param {(text: string) => number | null} cpuTimeSeconds
 */
function sample(serverPid, cpuTimeSeconds) {
  let serverCpuPercent = null;
  let serverCpuS = null;
  if (serverPid !== null) {
    // Each column its own -o, since procps reads a header after `=` to the argument's end.
    const ps = spawnSync("ps", ["-o", "%cpu=", "-o", "time=", "-p", String(serverPid)], {
      encoding: "utf8",
    });
    const fields = ps.error === undefined ? ps.stdout.trim().split(/\s+/u) : [];
    const percent = Number(fields.at(0));
    const time = fields.at(1);
    serverCpuPercent = Number.isFinite(percent) ? percent : null;
    serverCpuS = time === undefined ? null : cpuTimeSeconds(time);
  }
  const therm =
    process.platform === "darwin"
      ? spawnSync("pmset", ["-g", "therm"], { encoding: "utf8" })
      : null;
  return {
    at: new Date().toISOString(),
    load1: loadavg().at(0) ?? 0,
    serverCpuPercent,
    serverCpuS,
    thermal: therm === null || therm.error !== undefined ? null : therm.stdout,
  };
}

/**
 * The batch's options, or why they are refused.
 *
 * @param {ReadonlyArray<string>} args
 * @param {typeof import("../src/main/macProfile")} profile
 */
function optionsOf(args, profile) {
  const machine = option(args, "machine", "");
  if (machine.length === 0) {
    return { problem: 'name the machine with --machine, such as "Apple M5, 16 GiB"' };
  }
  const settings = listOf(option(args, "settings", "high,low"));
  const pairSetting = option(args, "pair-setting", "high");
  if (![...settings, pairSetting].every((each) => each === "high" || each === "low")) {
    return { problem: "--settings and --pair-setting take high and low" };
  }
  const ratios = listOf(option(args, "dprs", "1,2")).map(Number);
  if (!ratios.every((each) => each === 1 || each === 2)) {
    return { problem: "--dprs takes 1 and 2" };
  }
  const levers = profile.leversOf(option(args, "levers", ""));
  const unknown = levers.filter((name) => profile.leverNamed(name) === undefined);
  if (unknown.length > 0) {
    const known = profile.leversOf("all").join(", ");
    return { problem: `no lever is named ${unknown.join(", ")}: the levers are ${known}` };
  }
  const body = option(args, "body", DEFAULT_BODY);
  if (!/^[0-9a-f]{16}\.[0-9a-f]{4}$/u.test(body)) {
    return { problem: `--body takes a body's ID, such as ${DEFAULT_BODY}, not ${body}` };
  }
  const rounds = Number(option(args, "rounds", "2"));
  const loadBound = Number(option(args, "load-bound", "4"));
  const port = Number(option(args, "port", "7880"));
  const sweepWorkers = args.includes("--no-sweep")
    ? []
    : listOf(option(args, "sweep-workers", "3,4,6,8")).map(Number);
  if (!Number.isInteger(rounds) || rounds < 1 || !(loadBound > 0) || !Number.isInteger(port)) {
    return { problem: "--rounds takes a count, --load-bound a load and --port a port" };
  }
  if (!sweepWorkers.every((n) => Number.isInteger(n) && n >= 1 && n <= 64)) {
    return { problem: "--sweep-workers takes counts from 1 to 64, such as 3,4,6,8" };
  }
  return {
    machine,
    body,
    out: profile.batchOut(option(args, "out", join(repo, "target", "mac-profile")), process.cwd()),
    matrix: {
      settings,
      ratios,
      levers,
      pairSetting,
      rounds,
      counted: !args.includes("--no-count"),
    },
    loadBound,
    port,
    sweepWorkers,
    software: args.includes("--software"),
    summaryOnly: args.includes("--summary"),
  };
}

/**
 * Starts the server, its log written and its skies counted as it runs, and waits for it to listen.
 *
 * @param {string} binary
 * @param {string} out
 * @param {number} port
 * @param {typeof import("../src/main/macProfile")} profile
 */
async function startServer(binary, out, port, profile) {
  const log = createWriteStream(join(out, "server.log"), { flags: "a" });
  log.write(`--- the batch's server, started ${new Date().toISOString()}\n`);
  // A sky's census after its first reply on the performance cores: see the header's sky.
  const args = [
    "--port",
    String(port),
    "--data-dir",
    join(out, "server-data"),
    "--sky-qos",
    "utility",
  ];
  const child = start(binary, args, {
    cwd: repo,
    env: { ...process.env, NO_COLOR: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const server = {
    pid: child.pid ?? null,
    skies: profile.NO_SKIES,
    listening: false,
    exited: false,
  };
  for (const stream of [child.stdout, child.stderr]) {
    if (stream !== null) {
      createInterface({ input: stream }).on("line", (line) => {
        log.write(`${line}\n`);
        const event = profile.skyEventOf(line);
        if (event !== null) {
          server.skies = profile.tallied(server.skies, event);
        }
        server.listening ||= line.includes(" listening ");
      });
    }
  }
  child.on("exit", () => {
    server.exited = true;
  });
  const until = Date.now() + LISTEN_MS;
  await poll(() => {
    if (server.exited || Date.now() > until) {
      throw new Error(`the server did not listen on port ${String(port)}: see ${out}/server.log`);
    }
    return server.listening ? true : undefined;
  }, 200);
  return server;
}

/**
 * Waits, never stopping, until `waitingOn` says nothing is left to wait on, saying every minute
 * what it waits on; it throws only if the server it waits beside exits.
 *
 * @param {string} what
 * @param {() => string | null} waitingOn Why it must wait still, or `null` once it need not.
 * @param {{ readonly exited: boolean } | null} server
 * @returns {Promise<void>}
 */
async function waitUntil(what, waitingOn, server) {
  let reported = Date.now();
  await poll(() => {
    if (server?.exited === true) {
      throw new Error(`the server exited while the batch waited ${what}`);
    }
    const why = waitingOn();
    if (why === null) {
      return true;
    }
    if (Date.now() - reported >= REPORT_MS) {
      say(`waiting ${what}: ${why}`);
      reported = Date.now();
    }
    return undefined;
  }, POLL_MS);
}

/**
 * A count of skies, in words.
 *
 * @param {number} count
 */
function skiesText(count) {
  return `${String(count)} sk${count === 1 ? "y" : "ies"}`;
}

/**
 * Why a run must wait still for a quiet machine, or `null` once it need not.
 *
 * @param {{ readonly skies: import("../src/main/macProfile").SkyTally } | null} server
 * @param {number} loadBound
 * @param {typeof import("../src/main/macProfile")} profile
 * @returns {string | null}
 */
function notQuiet(server, loadBound, profile) {
  const flying = server === null ? 0 : profile.skiesInFlight(server.skies);
  const load = loadavg().at(0) ?? 0;
  if (flying > 0) {
    return `${skiesText(flying)} in flight on the server, the load ${load.toFixed(2)}`;
  }
  return load < loadBound
    ? null
    : `the 1-minute load ${load.toFixed(2)}, not under ${String(loadBound)}`;
}

/**
 * Writes the summary from the runs kept and the sweep's table and samples.
 *
 * @param {Exclude<ReturnType<typeof optionsOf>, { problem: string }>} options
 * @param {string} tree
 * @param {typeof import("../src/main/macProfile")} profile
 * @returns {number} The runs kept.
 */
function writeSummary(options, tree, profile) {
  const kept = profile.keptRunsIn(
    join(options.out, "runs"),
    profile.macProfileMatrix(options.matrix),
  );
  const lines = profile.macProfileSummaryLines({
    machine: options.machine,
    tree,
    body: options.body,
    options: options.matrix,
    kept,
    sweep: profile.sweepIn(join(options.out, "worker-sweep"), options.sweepWorkers.length > 0),
  });
  writeFileSync(join(options.out, "summary.md"), `${lines.join("\n")}\n`);
  return kept.length;
}

/**
 * Builds what the batch launches: the surface module, the client and the release server.
 *
 * @param {string} log
 * @param {typeof import("../src/main/macProfile")} profile
 * @returns {Promise<string>} The server's binary.
 */
async function build(log, profile) {
  await inTurn(
    [
      { command: "just", args: ["gen-surface"] },
      { command: "pnpm", args: ["--filter", "hyperion", "build"] },
    ],
    async ({ command, args }) => {
      if ((await run(command, args, log, repo)) !== 0) {
        throw new Error(`${command} ${args.join(" ")} failed: see ${log}`);
      }
    },
  );
  const cargo = spawnSync(
    "cargo",
    ["build", "--release", "-p", "hyperion-server", "--message-format=json-render-diagnostics"],
    { cwd: repo, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"], maxBuffer: 1 << 28 },
  );
  const binary =
    cargo.status === 0 ? profile.cargoExecutable(cargo.stdout, "hyperion-server") : null;
  if (binary === null) {
    throw new Error("the release server did not build");
  }
  return binary;
}

/**
 * Places the ship at `body` with the clock paused, on the server at `port`.
 *
 * @param {number} port
 * @param {string} body
 * @returns {Promise<string>} The placement, its line of JSON.
 */
function placeShip(port, body) {
  return new Promise((resolve, reject) => {
    const child = start(
      "node",
      [
        join(here, "placeShip.mjs"),
        "--address",
        LOCAL,
        "--port",
        String(port),
        "--look-at",
        body,
        "--rate",
        "0",
        "--json",
      ],
      { cwd: app, stdio: ["ignore", "pipe", "inherit"] },
    );
    let text = "";
    child.stdout?.on("data", (chunk) => {
      text += String(chunk);
    });
    child.on("close", (code) => {
      if (code === 0) {
        resolve(text);
      } else {
        reject(new Error(`place-ship ended ${String(code)}`));
      }
    });
  });
}

/**
 * Runs the batch's runs not kept and its sweep's counts not flown, and writes the summary.
 *
 * @param {ReadonlyArray<string>} args
 * @returns {Promise<number>} The exit status: 0 with every run kept and flown, 1 otherwise, 2 refused.
 */
async function main(args) {
  const load = (path) => runnerImport(join(app, path), { configFile: false, logLevel: "error" });
  /** @type {typeof import("../src/main/macProfile")} */
  const profile = (await load("src/main/macProfile.ts")).module;
  const options = optionsOf(args, profile);
  if ("problem" in options) {
    console.error(`macProfile: ${options.problem}`);
    return 2;
  }
  if (options.software && process.platform !== "linux") {
    console.error("macProfile: --software runs on Linux only");
    return 2;
  }
  mkdirSync(options.out, { recursive: true });
  const tree = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repo,
    encoding: "utf8",
  }).stdout.trim();
  const started = profile.batchIn(options.out);
  if (started !== null && started.tree !== tree) {
    console.error(
      `macProfile: ${options.out} holds runs made on ${started.tree}, not ${tree}: resume on that ` +
        "commit, or give another --out",
    );
    return 2;
  }
  if (started !== null && (started.body ?? DEFAULT_BODY) !== options.body) {
    console.error(
      `macProfile: ${options.out} holds runs of ${started.body ?? DEFAULT_BODY}, not ${options.body}: ` +
        "give another --out",
    );
    return 2;
  }
  if (started === null) {
    writeJson(join(options.out, "batch.json"), {
      tree,
      body: options.body,
      startedAt: new Date().toISOString(),
    });
  }
  if (options.summaryOnly) {
    writeSummary(options, tree, profile);
    return 0;
  }

  const matrix = profile.macProfileMatrix(options.matrix);
  const runs = join(options.out, "runs");
  mkdirSync(runs, { recursive: true });
  for (const name of readdirSync(runs).filter((each) => each.endsWith(".partial"))) {
    // A run stopped part way, run again whole.
    rmSync(join(runs, name), { recursive: true, force: true });
  }
  const todo = matrix.filter((each) => !existsSync(join(runs, each.id, "run.json")));
  say(
    `${String(matrix.length - todo.length)} of ${String(matrix.length)} runs kept, ${String(todo.length)} to run`,
  );
  let relay = null;
  const stop = async (code) => {
    stopping = true;
    await relay?.close();
    await stopChildren();
    process.exit(code);
  };
  process.on("SIGINT", () => void stop(130));
  process.on("SIGTERM", () => void stop(143));
  process.on("SIGHUP", () => void stop(129));
  const binary = await build(join(options.out, "build.log"), profile);

  const common = ["--hidden", ...(options.software ? ["--software"] : [])];
  if (todo.length > 0) {
    const server = await startServer(binary, options.out, options.port, profile);
    const placement = await placeShip(options.port, options.body);
    writeFileSync(join(options.out, "placement.json"), placement);
    const universe = String(JSON.parse(placement).universe.id);
    say(`the ship is placed at ${options.body} in universe ${universe}, its clock paused`);
    /** @type {typeof import("../src/main/holdingRelay")} */
    const relays = (await load("src/main/holdingRelay.ts")).module;
    // The address given, so that a developer's HYPERION_SERVER_ADDR leads no launch elsewhere.
    const inspection = [
      "--inspection",
      "--universe",
      universe,
      "--body",
      options.body,
      "--address",
      LOCAL,
    ];

    // Each setting's sky, held to its final reply before the setting's first run.
    const warmDir = join(options.out, "warm-up");
    await inTurn([...new Set(todo.map((each) => each.setting))], async (setting) => {
      await waitUntil(
        `to warm the ${setting} sky`,
        () => notQuiet(server, options.loadBound, profile),
        server,
      );
      const into = join(warmDir, setting);
      mkdirSync(into, { recursive: true });
      relay = await relays.startHoldingRelay(options.port);
      const held = relay;
      const before = server.skies;
      const startedAt = Date.now();
      const captures = entriesIn(CAPTURE_DIR);
      say(`warming the ${setting} sky: the inspection, through the holding relay`);
      const exit = await run(
        "bash",
        [
          join(here, "viewsCheck.sh"),
          ...inspection,
          ...common,
          "--setting",
          setting,
          "--port",
          String(held.port),
        ],
        join(into, "views-check.log"),
        repo,
      );
      moveWritten(CAPTURE_DIR, captures, into);
      await waitUntil(
        `for the ${setting} sky's final reply`,
        () => {
          const flying = profile.skiesInFlight(server.skies);
          return flying === 0
            ? null
            : `${skiesText(flying)} in flight, ${String(held.held())} connection held`;
        },
        server,
      );
      await held.close();
      relay = null;
      const skies = profile.skiesBetween(before, server.skies);
      writeJson(join(into, "warm-up.json"), {
        setting,
        exit,
        wallS: (Date.now() - startedAt) / 1000,
        skies,
      });
      say(
        skies.started === 0
          ? `the ${setting} warm-up asked no sky (exit ${String(exit)})`
          : `the ${setting} sky is warm: ${String(skies.final)} computed to the final reply, ${String(skies.kept)} kept already (exit ${String(exit)})`,
      );
    });

    await inTurn(todo, async (each) => {
      await waitUntil(
        `to run ${each.id}`,
        () => notQuiet(server, options.loadBound, profile),
        server,
      );
      const dir = join(runs, `${each.id}.partial`);
      mkdirSync(dir, { recursive: true });
      const timings = entriesIn(TIMING_DIR);
      const captures = entriesIn(CAPTURE_DIR);
      const argv = [
        ...inspection,
        "--timing",
        ...common,
        "--hyperion-gpu-timing",
        "--setting",
        each.setting,
        "--port",
        String(options.port),
        ...each.args,
      ];
      const skies = server.skies;
      const before = sample(server.pid, profile.cpuTimeSeconds);
      say(`running ${each.id}`);
      const exit = await run(
        "bash",
        [join(here, "viewsCheck.sh"), ...argv],
        join(dir, "views-check.log"),
        repo,
      );
      const after = sample(server.pid, profile.cpuTimeSeconds);
      const timing =
        moveWritten(TIMING_DIR, timings, dir).find((name) => name.endsWith(".json")) ?? null;
      moveWritten(CAPTURE_DIR, captures, dir);
      const failed =
        readFileSync(join(dir, "views-check.log"), "utf8")
          .split("\n")
          .toReversed()
          .find((line) => line.startsWith(FAIL_PREFIX)) ?? null;
      const failure = failed === null ? null : failed.slice(FAIL_PREFIX.length);
      writeJson(join(dir, "run.json"), {
        schema: profile.MAC_PROFILE_RUN_SCHEMA,
        version: 1,
        id: each.id,
        argv,
        exit,
        before,
        after,
        skies: profile.skiesBetween(skies, server.skies),
        timing,
        failure: exit === 0 ? null : failure,
        files: [...readdirSync(dir), "run.json"]
          .filter((name, i, all) => all.indexOf(name) === i)
          .toSorted(),
      });
      // A run whose timing file was written is kept whatever its exit: a check failing after its
      // windows (as the 4 km step at low did in the pod) fails again on a resume, and would keep
      // the windows it measured out of the summary; the record and summary give the exit.
      if (timing !== null) {
        rmSync(join(runs, each.id), { recursive: true, force: true });
        renameSync(dir, join(runs, each.id));
        say(
          `kept ${each.id}${exit === 0 ? "" : `, which ended ${String(exit)}: ${failure ?? "no FAIL line"}`}`,
        );
      } else {
        const failedDir = join(options.out, "failed");
        mkdirSync(failedDir, { recursive: true });
        renameSync(dir, join(failedDir, `${each.id}-${before.at.replaceAll(":", "")}`));
        say(
          `${each.id} ended ${String(exit)} with no timing file: not kept, to run again on a resume`,
        );
      }
      writeSummary(options, tree, profile);
    });
    // The sweep's descents start servers of their own.
    await stopChildren();
  }

  // The sweep, a count at a time so that each descent starts on a quiet machine and is sampled;
  // workerSweep.mjs flies a count only where its directory holds no results.
  const sweepDir = join(options.out, "worker-sweep");
  const samplesFile = join(sweepDir, "samples.json");
  const configuration = [...common, "--world", "served", "--sizes", "on", "--setting", "high"];
  const sweep = [
    "--no-warnings",
    join(here, "workerSweep.mjs"),
    "--machine",
    options.machine,
    "--out",
    sweepDir,
  ];
  const flown = (n) =>
    [...entriesIn(join(sweepDir, `workers-${String(n)}`)).keys()].some(
      (name) => name.endsWith(".json") && !name.endsWith("-sizes-rejected.json"),
    );
  await inTurn(
    options.sweepWorkers.filter((n) => !flown(n)),
    async (n) => {
      mkdirSync(sweepDir, { recursive: true });
      await waitUntil(
        `to fly the sweep at ${String(n)} workers`,
        () => notQuiet(null, options.loadBound, profile),
        null,
      );
      const before = sample(null, profile.cpuTimeSeconds);
      say(`flying the sweep's descent at ${String(n)} workers`);
      const exit = await run(
        "node",
        [...sweep, "--workers", String(n), "--", ...configuration],
        join(sweepDir, `workers-${String(n)}.log`),
        app,
      );
      const after = sample(null, profile.cpuTimeSeconds);
      const { runs: sampled } = profile.sweepIn(sweepDir, true);
      writeJson(samplesFile, [
        ...sampled.filter((each) => each.workers !== n),
        { workers: n, exit, before, after },
      ]);
    },
  );
  const swept = options.sweepWorkers.every(flown);
  if (options.sweepWorkers.length > 0 && swept) {
    await run(
      "node",
      [...sweep, "--workers", options.sweepWorkers.join(","), "--table"],
      join(sweepDir, "table.log"),
      app,
    );
  }
  const keptCount = writeSummary(options, tree, profile);
  say(
    `${String(keptCount)} of ${String(matrix.length)} runs kept; the summary is ${join(options.out, "summary.md")}`,
  );
  return keptCount === matrix.length && swept ? 0 : 1;
}

process.exitCode = await main(process.argv.slice(2));
