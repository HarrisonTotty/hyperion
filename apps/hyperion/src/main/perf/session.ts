/**
 * A perf run's main-process session (plan R12, R12.T4.b): the runner over this process's sources,
 * the run window's own debugger, Node's file system and a local `hyperion-server`.
 *
 * @remarks
 * The recipe (`just perf-run`, `scripts/perfRun.sh`) states the build in the environment: the
 * commit and whether the tree was dirty, `hyperion-base`'s `GENERATOR_VERSION`, the server binary
 * it built, the NVIDIA baseline it read before Electron started (the GPU process can start before
 * this process could read it, as R05's recipe found), and whether the window is hidden. A run
 * without them is refused, since a perf record states its commit and generator version.
 */

import { execFile } from "node:child_process";
import { cpus, loadavg } from "node:os";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

import {
  PERF_PINS,
  PERF_SCENE_HARNESS,
  PERF_UNIVERSE_NAME,
  type PerfPlacement,
} from "@hyperion/perf";

import type { PerfArrival, PerfLaunch } from "../../preload/api";
import type { GraphicsLaunch } from "../../preload/graphicsLaunch";
import { CdpTracing } from "../cdpTracing";
import { measured, missing } from "../measured";
import { reduceTraceFile } from "../reduceTrace";
import { activeGpu } from "../results";
import { SpikeTrace, type TraceWindowStop } from "../spike";
import { WindowedFrameTrace, TRACE_WINDOW_MS, type WindowTrace } from "./frameTrace";
import { machineSlugOf } from "./launch";
import { describeMachine, nodeMachineSources, type DisplayLike } from "./machine";
import { MEMORY_SAMPLE_INTERVAL_MS, PerfMemorySampler } from "./memory";
import { checkQuiet } from "./preconditions";
import { PerfRunner } from "./runner";
import {
  drmDriversOf,
  findDisplayServerPid,
  GpuClockSampler,
  NODE_EVERY,
  nodeMemorySources,
  nodeSamplerSources,
  NvidiaClockStream,
  PERF_SAMPLE_INTERVAL_MS,
  PerfSampler,
  sampleDisplayServer,
  sampleGpuMemory,
  sampleNvidia,
  SPAWN_LINES,
} from "./samplers";
import { LocalServer, NODE_SPAWN_SERVER, NODE_WAIT, pinnedToOneCore } from "./server";

/** The variables the recipe states the run's build and window in. */
export const PERF_RUN_ENV = {
  commit: "HYPERION_PERF_COMMIT",
  dirty: "HYPERION_PERF_DIRTY",
  generatorVersion: "HYPERION_PERF_GENERATOR_VERSION",
  server: "HYPERION_PERF_SERVER",
  nvidiaBaseline: "HYPERION_PERF_NVIDIA_BASELINE",
  hidden: "HYPERION_PERF_HIDDEN",
  /** The Node that runs `just place-ship`, `node` on the path without it. */
  node: "HYPERION_PERF_NODE",
} as const;

/** Where `--perf-out` appends without the option, from the working directory. */
export const DEFAULT_PERF_OUT = "target/perf-run/runs.v1.jsonl";

/** NVIDIA's PCI vendor ID. */
const NVIDIA_VENDOR_ID = 0x10_de;

/** The build a run's record states, from the recipe's variables, or why it cannot be read. */
export function runBuildOf(
  env: Readonly<Record<string, string | undefined>>,
):
  { readonly commit: string; readonly dirty: boolean; readonly generatorVersion: number } | string {
  const commit = env[PERF_RUN_ENV.commit] ?? "";
  const version = Number(env[PERF_RUN_ENV.generatorVersion] ?? "");
  if (!/^[0-9a-f]{40}$/.test(commit)) {
    return `${PERF_RUN_ENV.commit} is not a commit; run through \`just perf-run\`, which states it`;
  }
  if (!Number.isInteger(version) || version < 1) {
    return `${PERF_RUN_ENV.generatorVersion} is not a generator version; run through \`just perf-run\``;
  }
  return { commit, dirty: env[PERF_RUN_ENV.dirty] === "1", generatorVersion: version };
}

/** How long an arrival may take, ms: a universe made, the ship placed and a world surveyed. */
export const PLACE_TIMEOUT_MS = 300_000;

/**
 * `just place-ship`'s arguments for a scene's arrival on the server at `serverUrl` (its universe
 * by the pins' name and seed, made where a fresh server has none, the clock at the epoch): beside
 * the body, surveyed first where the scene flies its terrain, or beside the system's barycentre.
 */
export function placeShipArgs(serverUrl: string, placement: PerfPlacement): ReadonlyArray<string> {
  const url = new URL(serverUrl);
  const host = url.hostname.startsWith("[") ? url.hostname.slice(1, -1) : url.hostname;
  const target =
    placement.kind === "body"
      ? [
          "--look-at",
          placement.body,
          ...(placement.survey === null
            ? []
            : ["--survey", placement.survey.source, String(placement.survey.resolutionM)]),
        ]
      : ["--look-at", "barycentre", "--system", placement.system];
  return [
    "--address",
    host,
    "--port",
    url.port,
    "--json",
    "--universe",
    PERF_UNIVERSE_NAME,
    "--seed",
    PERF_PINS.seed,
    "--time",
    "0",
    ...target,
  ];
}

/** The arrival `just place-ship --json`'s last line states, or `null` where it is not one. */
export function arrivalOf(stdout: string): PerfArrival | null {
  const line = stdout.trim().split("\n").at(-1) ?? "";
  let parsed: unknown;
  try {
    parsed = JSON.parse(line);
  } catch {
    return null;
  }
  if (typeof parsed !== "object" || parsed === null) {
    return null;
  }
  const universe: unknown = Reflect.get(parsed, "universe");
  const id: unknown =
    typeof universe === "object" && universe !== null ? Reflect.get(universe, "id") : undefined;
  const system: unknown = Reflect.get(parsed, "system");
  const body: unknown = Reflect.get(parsed, "body");
  return typeof id === "string" &&
    typeof system === "string" &&
    (body === null || typeof body === "string")
    ? { universe: id, system, body }
    : null;
}

/** Whether `address` is this machine's: the loopback's names and addresses. */
export function isLoopback(address: string): boolean {
  return address === "localhost" || address === "::1" || /^127\.\d+\.\d+\.\d+$/.test(address);
}

/** A trace whose window's debugger is attached once the window exists. */
export class LateTrace implements WindowTrace {
  #trace: SpikeTrace | null = null;

  /** Attaches the run window's trace. */
  attach(trace: SpikeTrace): void {
    this.#trace = trace;
  }

  /** The trace's categories, for reducing its windows; empty before it is attached. */
  get categories(): ReadonlyArray<string> {
    return this.#trace?.settings.categories ?? [];
  }

  #attached(): SpikeTrace {
    if (this.#trace === null) {
      throw new Error("the run window's trace is not attached");
    }
    return this.#trace;
  }

  start(): Promise<void> {
    return this.#attached().start();
  }

  stop(path: string): Promise<TraceWindowStop> {
    return this.#attached().stop(path);
  }

  cycle(path: string, stopped: (stop: TraceWindowStop) => void): Promise<void> {
    return this.#attached().cycle(path, stopped);
  }
}

/** What the session needs of Electron and the launch. */
export interface PerfSessionInputs {
  readonly launch: PerfLaunch;
  readonly graphics: GraphicsLaunch;
  /** Every switch in force, as Chromium's command line writes them. */
  readonly switches: ReadonlyArray<string>;
  readonly hidden: boolean;
  /** The remote server's URL where `--address` names another machine, else `null`. */
  readonly remoteUrl: string | null;
  /** How many times longer the watchdog waits: the software mode's factor, else 1. */
  readonly patience: number;
  readonly env: Readonly<Record<string, string | undefined>>;
  /** The repository's root, from the built main process's place in it. */
  readonly repoRoot: string;
  readonly userData: string;
  readonly appVersion: string;
  gpuInfo(kind: "basic" | "complete"): Promise<unknown>;
  appMetrics(): unknown;
  /** The run window's display, once there is a window. */
  display(): DisplayLike | null;
  /** The NVIDIA baseline the recipe read, bytes, or `null`. */
  readonly nvidiaBaselineBytes: number | null;
  readonly log: (line: string) => void;
  readonly exit: (code: number) => void;
}

/** A prepared session: its runner, and the trace to attach once the window is made. */
export interface PerfSession {
  readonly runner: PerfRunner;
  readonly trace: LateTrace;
  /** Attaches the trace to the run window's debugger (`webContents.debugger`). */
  attachWindow(debuggee: ConstructorParameters<typeof CdpTracing>[0]): void;
}

/** The epoch's milliseconds now, on the high-resolution clock. */
function epochNowMs(): number {
  return performance.timeOrigin + performance.now();
}

/**
 * Makes and prepares the run's session: its preconditions checked and its server and samplers
 * started.
 *
 * @returns The session, or `null` where the run is refused (said in the log), nothing running.
 */
export async function preparePerfSession(inputs: PerfSessionInputs): Promise<PerfSession | null> {
  const { launch, env, log } = inputs;
  const build = runBuildOf(env);
  if (typeof build === "string") {
    log(`perf run: REFUSED ${build}`);
    return null;
  }
  const harness = PERF_SCENE_HARNESS[launch.scene];
  const gpu = activeGpu(await inputs.gpuInfo("basic"));
  const identity = gpu.value === null ? missing(gpu.reason) : measured(gpu.value);
  const vendorId = gpu.value?.vendorId ?? null;
  const samplerSources = nodeSamplerSources();
  const nvidia =
    vendorId === NVIDIA_VENDOR_ID
      ? new NvidiaClockStream({
          spawn: SPAWN_LINES,
          intervalMs: PERF_SAMPLE_INTERVAL_MS,
          gpuIndex: 0,
          now: epochNowMs,
        })
      : null;
  const sampler = new PerfSampler({
    sources: samplerSources,
    gpu: new GpuClockSampler({ sources: samplerSources, gpu: identity, nvidia }),
    intervalMs: PERF_SAMPLE_INTERVAL_MS,
    now: epochNowMs,
    every: NODE_EVERY,
  });
  const memorySources = nodeMemorySources();
  const drivers = vendorId === null ? null : drmDriversOf(vendorId);
  const xorgPid = await findDisplayServerPid(memorySources);
  const memory = new PerfMemorySampler({
    sources: {
      appMetrics: () => inputs.appMetrics(),
      gpu: (pid, atMs) => sampleGpuMemory(pid, drivers, memorySources, atMs),
      nvidia:
        vendorId === NVIDIA_VENDOR_ID
          ? (gpuPid, atMs) => sampleNvidia(gpuPid, inputs.nvidiaBaselineBytes, memorySources, atMs)
          : null,
      displayServer: (atMs) => sampleDisplayServer(xorgPid, drivers, memorySources, atMs),
    },
    intervalMs: MEMORY_SAMPLE_INTERVAL_MS,
    now: epochNowMs,
    every: NODE_EVERY,
  });
  const serverLaunch = {
    binary:
      env[PERF_RUN_ENV.server] ??
      join(
        inputs.repoRoot,
        "target",
        "release",
        process.platform === "win32" ? "hyperion-server.exe" : "hyperion-server",
      ),
    numWorkers: harness.server.numWorkers,
    placement: harness.server.placement,
    platform: process.platform,
  };
  const server =
    inputs.remoteUrl === null
      ? new LocalServer({
          launch: serverLaunch,
          spawn: NODE_SPAWN_SERVER,
          wait: NODE_WAIT,
          log: (line) => {
            log(line);
          },
        })
      : null;
  const late = new LateTrace();
  const runDir = join(inputs.userData, "perf-run");
  const outPath = resolve(launch.out ?? DEFAULT_PERF_OUT);
  const slug = machineSlugOf(gpu.value, process.platform, cpus()[0]?.model ?? "");
  const ozone = inputs.switches.find((each) => each.startsWith("--ozone-platform="));
  const runner = new PerfRunner({
    launch,
    harness,
    graphics: {
      launchMode: inputs.graphics.launchMode,
      gpuTiming: inputs.graphics.gpuTiming,
      switches: inputs.switches,
    },
    build,
    hidden: inputs.hidden,
    remoteUrl: inputs.remoteUrl,
    server,
    pinned: pinnedToOneCore(serverLaunch),
    place: (serverUrl) =>
      new Promise((resolvePlace, rejectPlace) => {
        // place-ship under Node, as `descentSpike.sh` runs it, from the client's directory.
        const script = join(inputs.repoRoot, "apps", "hyperion", "scripts", "placeShip.mjs");
        execFile(
          env[PERF_RUN_ENV.node] ?? "node",
          [script, ...placeShipArgs(serverUrl, harness.placement)],
          { cwd: join(inputs.repoRoot, "apps", "hyperion"), timeout: PLACE_TIMEOUT_MS },
          (error, stdout, stderr) => {
            const arrival = error === null ? arrivalOf(stdout) : null;
            if (arrival === null) {
              rejectPlace(
                new Error(`place-ship did not arrive: ${error?.message ?? ""} ${stderr}`.trim()),
              );
            } else {
              log(
                `perf run: arrived in universe ${arrival.universe} at ${arrival.body ?? arrival.system}`,
              );
              resolvePlace(arrival);
            }
          },
        );
      }),
    freshDataDir: async (n) => {
      const dir = join(runDir, `server-data-${String(n)}`);
      await rm(dir, { recursive: true, force: true });
      await mkdir(dir, { recursive: true });
      return dir;
    },
    sampler,
    memory,
    trace: new WindowedFrameTrace({
      trace: late,
      reduce: (path) => reduceTraceFile(path, { categories: late.categories }),
      path: (repetition, n) => join(runDir, `trace-${String(repetition)}-${String(n)}.pftrace`),
      removeFile: (path) => rm(path, { force: true }),
      every: NODE_EVERY,
      windowMs: TRACE_WINDOW_MS,
      log,
    }),
    checkQuiet: () =>
      checkQuiet({
        ...samplerSources,
        loadavg: () => loadavg(),
        sleep: (ms) =>
          new Promise((done) => {
            setTimeout(done, ms);
          }),
      }),
    describeMachine: (description) =>
      describeMachine(
        {
          name: slug,
          adapter: description.adapter,
          capabilities: description.capabilities,
          kernelPaths: description.kernelPaths,
          card: null,
          ozonePlatform: ozone === undefined ? null : ozone.slice("--ozone-platform=".length),
        },
        nodeMachineSources({
          gpuInfo: () => inputs.gpuInfo("complete"),
          display: () => inputs.display(),
          appVersion: inputs.appVersion,
        }),
      ),
    vsyncPeriodMs: () => {
      const hz = inputs.display()?.displayFrequency ?? 0;
      return hz > 0 ? 1000 / hz : null;
    },
    now: epochNowMs,
    wait: NODE_WAIT,
    patience: inputs.patience,
    readOut: async () => {
      try {
        return await readFile(outPath, "utf8");
      } catch (error: unknown) {
        if (error instanceof Error && "code" in error && error.code === "ENOENT") {
          return "";
        }
        throw error;
      }
    },
    writeOut: async (text) => {
      await mkdir(dirname(outPath), { recursive: true });
      await writeFile(outPath, text);
    },
    outPath,
    log,
    exit: (code) => {
      nvidia?.stop();
      inputs.exit(code);
    },
  });
  nvidia?.start();
  if (!(await runner.prepare())) {
    nvidia?.stop();
    return null;
  }
  return {
    runner,
    trace: late,
    attachWindow: (debuggee) => {
      late.attach(
        new SpikeTrace(new CdpTracing(debuggee, { log, nowMs: () => performance.now() }), {
          profiled: false,
        }),
      );
    },
  };
}
