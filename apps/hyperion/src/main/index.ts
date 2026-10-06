import { readFileSync } from "node:fs";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { CommanderError } from "commander";
import {
  app,
  BrowserWindow,
  contentTracing,
  dialog,
  ipcMain,
  type IpcMainInvokeEvent,
  screen,
  session,
  shell,
} from "electron";

import type { SpikeLaunch, ViewsCheckLaunch } from "../preload/api";
import { type GraphicsLaunch, graphicsArguments } from "../preload/graphicsLaunch";
import { serverUrlSwitch } from "../preload/serverUrl";
import { spikeSwitch } from "../preload/spikeLaunch";
import { viewsCheckSwitch } from "../preload/viewsCheckLaunch";
import { CdpTracing } from "./cdpTracing";
import { type ClientArgs, parseClientArgs, serverUrlOf, userArgs } from "./cli";
import { parseNvidiaSmi, readDrmMemory, readNvidiaSmi } from "./fdinfo";
import { GpuClockReader } from "./gpuClocks";
import {
  applyGraphicsSwitches,
  type ChromiumSwitch,
  GPU_TIMING_SWITCH,
  launchModeOf,
  SAFE_MODE_SWITCH,
} from "./graphics/switches";
import { isOwnPage } from "./ipcSender";
import { reduceTraceFile } from "./reduceTrace";
import { activeGpu, describeMachine, MemorySampler, nodeMachineSources } from "./results";
import { launchSwitches, registerSpikeHandlers, SpikeTrace } from "./spike";
import { SpikeSession } from "./spikeSession";
import { reduceWindowFile, registerViewsCheckHandlers, ViewsCheckSession } from "./viewsCheck";
import { GpuProcessMonitor } from "./graphics/gpuProcessMonitor";
import { x11RelaunchArgs } from "./graphics/x11Relaunch";
import { isSafeExternalUrl, isSameDocument } from "./navigation";
import { denyPermissionRequests } from "./permissions";

function reportLoadFailure(error: unknown): void {
  console.error("failed to load the renderer:", error);
}

/**
 * The command line's arguments, or `undefined` once the command line has ended the run, as
 * `--help`, `--version` and a usage error do.
 */
function resolveArgs(): ClientArgs | undefined {
  try {
    return parseClientArgs(userArgs(process.argv, app.isPackaged), __APP_VERSION__);
  } catch (error) {
    if (error instanceof CommanderError) {
      // The help, the version or the usage error has been written already.
      app.exit(error.exitCode);
      return undefined;
    }
    throw error;
  }
}

/** The descent spike's window: its canvas size per setting, and whether it is shown (T13.c). */
interface SpikeWindow {
  readonly launch: SpikeLaunch;
  /** A hidden run never shows its window and renders offscreen (`--smoke`, or the recipe's `--hidden`). */
  readonly hidden: boolean;
}

/** The variable by which `just descent-spike --hidden` keeps a full run's window hidden. */
const SPIKE_HIDDEN_ENV = "HYPERION_SPIKE_HIDDEN";

/** The spike window's content size: Design note 21's 1080p for high and 720p for low. */
function spikeSize(launch: SpikeLaunch | ViewsCheckLaunch): {
  readonly width: number;
  readonly height: number;
} {
  return launch.setting === "high" ? { width: 1920, height: 1080 } : { width: 1280, height: 720 };
}

/**
 * The several-views check's window size (R07.T20): 1920 × 1080 or 1280 × 720 device pixels,
 * whatever the display's scale, so that its canvases are R05 Design note 21's 1080p and 720p;
 * a tiling window manager may still size it to its own tile, which the record reads at the end.
 */
function viewsCheckSize(
  launch: ViewsCheckLaunch,
  scaleFactor: number,
): { readonly width: number; readonly height: number } {
  const px = spikeSize(launch);
  const scale = scaleFactor > 0 ? scaleFactor : 1;
  return { width: Math.round(px.width / scale), height: Math.round(px.height / scale) };
}

/** The several-views check's window (R07.T20): shown unless hidden. */
interface ViewsCheckWindowRun {
  readonly launch: ViewsCheckLaunch;
  /** A hidden run never shows its window and renders offscreen (`--smoke`, or the recipe's `--hidden`). */
  readonly hidden: boolean;
}

function createWindow(
  serverUrl: string,
  graphics: GraphicsLaunch,
  spike: SpikeWindow | null,
  check: ViewsCheckWindowRun | null = null,
): BrowserWindow {
  const run = spike ?? check;
  const size =
    check !== null
      ? // Offscreen rendering draws at a device-pixel ratio of 1, whatever the display's.
        viewsCheckSize(check.launch, check.hidden ? 1 : screen.getPrimaryDisplay().scaleFactor)
      : run === null
        ? { width: 1600, height: 900 }
        : spikeSize(run.launch);
  const window = new BrowserWindow({
    ...size,
    useContentSize: run !== null,
    // The spike holds its size; the check's may meet a smaller display, and records what it got.
    minWidth: spike === null ? 1024 : size.width,
    minHeight: spike === null ? 640 : size.height,
    show: false,
    backgroundColor: "#05080d",
    autoHideMenuBar: true,
    webPreferences: {
      preload: join(__dirname, "../preload/index.js"),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      // The sandboxed preload has no way to read the command line, so the URL and the graphics
      // launch ride in its argv.
      additionalArguments: [
        serverUrlSwitch(serverUrl),
        ...graphicsArguments(graphics.launchMode, graphics.gpuTiming),
        ...(spike === null ? [] : [spikeSwitch(spike.launch)]),
        ...(check === null ? [] : [viewsCheckSwitch(check.launch)]),
      ],
      // A hidden spike run renders offscreen, as the smoke harness's does: a hidden window on a
      // hardware adapter draws nothing otherwise, and headless Ozone's GPU process exits there.
      ...(run?.hidden === true ? { offscreen: true, backgroundThrottling: false } : {}),
    },
  });

  if (run?.hidden !== true) {
    window.once("ready-to-show", () => {
      window.show();
    });
  }

  // Never open foreign pages inside the bridge; hand web links to the OS browser.
  window.webContents.setWindowOpenHandler(({ url }) => {
    if (isSafeExternalUrl(url)) {
      shell.openExternal(url).catch((error: unknown) => {
        console.error("failed to open external link:", error);
      });
    } else {
      console.warn("blocked attempt to open unsafe url:", url);
    }
    return { action: "deny" };
  });

  // The bridge is a single page: anything but a reload would replace it with foreign content.
  window.webContents.on("will-navigate", (event, url) => {
    if (!isSameDocument(url, window.webContents.getURL())) {
      event.preventDefault();
      console.warn("blocked navigation to:", url);
    }
  });

  // electron-vite serves the renderer over HTTP in dev and from disk in production.
  const devServerUrl = process.env["ELECTRON_RENDERER_URL"];
  if (!app.isPackaged && devServerUrl !== undefined) {
    window.loadURL(devServerUrl).catch(reportLoadFailure);
  } else {
    window.loadFile(join(__dirname, "../renderer/index.html")).catch(reportLoadFailure);
  }
  return window;
}

/** The variable naming the `nvidia-smi -q -x` reading the recipe took before the launch. */
const SPIKE_NVIDIA_BASELINE_ENV = "HYPERION_SPIKE_NVIDIA_BASELINE";

/**
 * The device's memory before the client held any, bytes, which the results subtract (Design note
 * 18): the recipe reads `nvidia-smi` before Electron starts, since the GPU process can start
 * before the main process could; `null` without a reading (off NVIDIA, or not the recipe).
 */
function nvidiaBaseline(): number | null {
  const path = process.env[SPIKE_NVIDIA_BASELINE_ENV];
  if (path === undefined) {
    return null;
  }
  try {
    const reading = parseNvidiaSmi(readFileSync(path, "utf8"));
    return reading.kind === "nvidia" ? (reading.gpus[0]?.usedBytes ?? null) : null;
  } catch (error: unknown) {
    console.error("descent spike: the nvidia-smi baseline could not be read:", error);
    return null;
  }
}

/** The page the window loads, against which the spike's handlers check their sender. */
function pageUrl(): string {
  const devServerUrl = process.env["ELECTRON_RENDERER_URL"];
  return !app.isPackaged && devServerUrl !== undefined
    ? devServerUrl
    : pathToFileURL(join(__dirname, "../renderer/index.html")).href;
}

/**
 * How long a spike run may take before the main process ends it with status 3: the smoke's 10 s
 * and the descent's 20.5 minutes, each with room for building the scene and writing the results.
 */
function spikeWatchdogMs(launch: SpikeLaunch): number {
  return launch.smoke ? 180_000 : 45 * 60_000;
}

/** Writes one line of a spike run's log to stdout, where the recipe reads it. */
function logLine(line: string): void {
  process.stdout.write(`${line}\n`);
}

/**
 * Wires a spike run's handlers to its window (T13.c): the trace, over the window's own debugger
 * (T14.i), the memory sampler and the results file, each call checked against the window's own
 * page.
 */
function startSpikeSession(
  window: BrowserWindow,
  launch: SpikeLaunch,
  graphics: GraphicsLaunch,
  switches: ReadonlyArray<ChromiumSwitch>,
  hidden: boolean,
  nvidiaBaselineBytes: number | null,
): void {
  const trace = new SpikeTrace(
    new CdpTracing(window.webContents.debugger, {
      log: logLine,
      nowMs: () => performance.now(),
    }),
    { profiled: launch.traceProfile === "on" },
  );
  const startedAt = new Date();
  /** The renderer's last private-memory reading, bytes, for the sampler. */
  let rendererBytes: number | null = null;
  const clocks = new GpuClockReader({
    platform: process.platform,
    gpu: () => app.getGPUInfo("basic").then(activeGpu),
  });
  const memory = new MemorySampler(
    {
      appMetrics: () => app.getAppMetrics(),
      rendererPrivateBytes: () => Promise.resolve(rendererBytes),
      drm: (pid) => readDrmMemory(pid, process.platform),
      nvidia: () => readNvidiaSmi(),
      clocks: (nvidia) => clocks.read(nvidia),
      nowMs: () => performance.now(),
    },
    (error: unknown) => {
      console.error("descent spike: a memory sample failed:", error);
    },
  );
  const spikeSession = new SpikeSession({
    launch,
    describe: async () => ({
      startedAt,
      machine: await describeMachine(nodeMachineSources(() => app.getGPUInfo("basic"))),
      versions: {
        app: __APP_VERSION__,
        electron: process.versions.electron,
        chromium: process.versions.chrome,
        node: process.versions.node,
        v8: process.versions.v8,
      },
      platform: process.platform,
      launchMode: graphics.launchMode,
      setting: launch.setting,
      seed: launch.seed,
      options: Object.fromEntries(
        Object.entries(launch).filter(
          (entry): entry is [string, string | number | boolean] => entry[1] !== null,
        ),
      ),
      switches: switches.map(({ name, value }) =>
        value === undefined ? `--${name}` : `--${name}=${value}`,
      ),
      shown: !hidden,
      displayHz: hidden ? null : screen.getDisplayMatching(window.getBounds()).displayFrequency,
      nvidiaBaselineBytes,
    }),
    trace,
    traceDir: app.getPath("userData"),
    reduce: (path, categories) => reduceTraceFile(path, { categories }),
    memory,
    outDir: launch.out ?? resolve(process.cwd(), "docs/measurements/descent-spike"),
    exit: (code) => {
      app.exit(code);
    },
    log: logLine,
  });
  const page = pageUrl();
  registerSpikeHandlers<IpcMainInvokeEvent>({
    handle: (channel, listener) => {
      ipcMain.handle(channel, listener);
    },
    isSender: (event) => isOwnPage(event.senderFrame, page, window.webContents.mainFrame),
    ...spikeSession.operations(),
    rendererMemory: (bytes) => {
      rendererBytes = bytes;
    },
  });
  const watchdog = setTimeout(() => {
    process.stdout.write("descent spike: WATCHDOG the run did not end\n");
    app.exit(3);
  }, spikeWatchdogMs(launch));
  app.once("will-quit", () => {
    clearTimeout(watchdog);
  });
  window.webContents.once("render-process-gone", (_event, details) => {
    process.stdout.write(`descent spike: FAIL the renderer exited (${details.reason})\n`);
    app.exit(1);
  });
  // A window closed before the run ended is a failed run, not the ordinary quit's 0.
  window.once("closed", () => {
    if (!spikeSession.ended) {
      process.stdout.write("descent spike: FAIL the window was closed before the run ended\n");
      app.exit(1);
    }
  });
}

/** The variable by which `just views-check --hidden` keeps a full check's window hidden. */
const VIEWS_CHECK_HIDDEN_ENV = "HYPERION_VIEWS_CHECK_HIDDEN";

/** How long the person at a shown check has to answer before the run carries on unanswered. */
const RIGHT_WAY_UP_TIMEOUT_MS = 180_000;

/**
 * Asks the person at a shown check whether every view is the right way up (R07.T20).
 *
 * @remarks
 * Not modal, so that the views can be flown while it stands: the measured phases are over, and
 * the scene's bodies lie on the horizon line, where only turning a camera shows a view upside down.
 */
async function askRightWayUp(): Promise<"yes" | "no" | null> {
  const { response } = await dialog.showMessageBox({
    type: "question",
    title: "HYPERION views check",
    message: "Is every view the right way up?",
    detail:
      "The measured phases are done. Three views are open: PRIMARY, photorealistic, filling the " +
      "stage, and INSTRUMENT 1 and INSTRUMENT 2, wireframes at its right edge. Click each in turn " +
      "and hold an arrow key a moment to turn its camera: as a camera turns up the bodies move " +
      "down, and as it turns right they move left, in every view; nothing is upside down or " +
      "mirrored. Then answer. Skip ends the run without an answer; a picture of each phase is " +
      "saved under target/views-check/.",
    buttons: ["Yes", "No", "Skip"],
    defaultId: 0,
    cancelId: 2,
    noLink: true,
    signal: AbortSignal.timeout(RIGHT_WAY_UP_TIMEOUT_MS),
  });
  return response === 0 ? "yes" : response === 1 ? "no" : null;
}

/**
 * Wires a several-views check's handlers to its window (R07.T20): a trace window and a capture a
 * phase, the question at a shown run, the console's pass-timer warnings and the GPU process's
 * exits, and the results file, each call checked against the window's own page.
 */
function startViewsCheckSession(
  window: BrowserWindow,
  launch: ViewsCheckLaunch,
  graphics: GraphicsLaunch,
  switches: ReadonlyArray<ChromiumSwitch>,
  hidden: boolean,
): void {
  const startedAt = new Date();
  const stamp = startedAt.toISOString().replaceAll(/[-:]/g, "").slice(0, 15);
  const repo = process.cwd();
  const checkSession = new ViewsCheckSession({
    launch,
    describe: async () => ({
      startedAt,
      machine: await describeMachine(nodeMachineSources(() => app.getGPUInfo("basic"))),
      versions: {
        app: __APP_VERSION__,
        electron: process.versions.electron,
        chromium: process.versions.chrome,
        node: process.versions.node,
        v8: process.versions.v8,
      },
      platform: process.platform,
      launchMode: graphics.launchMode,
      setting: launch.setting,
      smoke: launch.smoke,
      switches: switches.map(({ name, value }) =>
        value === undefined ? `--${name}` : `--${name}=${value}`,
      ),
      shown: !hidden,
      displayHz: hidden ? null : screen.getDisplayMatching(window.getBounds()).displayFrequency,
    }),
    windowSize: () => {
      const [widthDip = 0, heightDip = 0] = window.getContentSize();
      return { widthDip, heightDip };
    },
    tracing: contentTracing,
    tracePath: (phase) => join(app.getPath("userData"), `views-check-${phase}.json`),
    reduce: reduceWindowFile,
    removeFile: (path) => rm(path, { force: true }),
    capturesDir: resolve(repo, "target/views-check", `captures-${stamp}`),
    capture: async (path) => {
      await mkdir(dirname(path), { recursive: true });
      const image = await window.webContents.capturePage();
      await writeFile(path, image.toPNG());
    },
    ask: hidden ? null : askRightWayUp,
    outDir:
      launch.out ??
      resolve(repo, launch.smoke ? "target/views-check" : "docs/measurements/several-views"),
    exit: (code) => {
      app.exit(code);
    },
    log: (line) => {
      process.stdout.write(`${line}\n`);
    },
  });
  const page = pageUrl();
  registerViewsCheckHandlers<IpcMainInvokeEvent>({
    handle: (channel, listener) => {
      ipcMain.handle(channel, listener);
    },
    isSender: (event) => isOwnPage(event.senderFrame, page, window.webContents.mainFrame),
    session: checkSession,
  });
  window.webContents.on("console-message", (details) => {
    checkSession.consoleMessage(details.message);
  });
  app.on("child-process-gone", (_event, details) => {
    if (details.type === "GPU") {
      checkSession.gpuProcessGone();
    }
  });
  const watchdog = setTimeout(
    () => {
      process.stdout.write("views check: WATCHDOG the run did not end\n");
      app.exit(3);
    },
    launch.smoke ? 240_000 : 20 * 60_000,
  );
  app.once("will-quit", () => {
    clearTimeout(watchdog);
  });
  window.webContents.once("render-process-gone", (_event, details) => {
    process.stdout.write(`views check: FAIL the renderer exited (${details.reason})\n`);
    app.exit(1);
  });
  window.once("closed", () => {
    if (!checkSession.ended) {
      process.stdout.write("views check: FAIL the window was closed before the run ended\n");
      app.exit(1);
    }
  });
}

/**
 * Sets up the GPU before `ready`: relaunches a Wayland session through XWayland, or puts the
 * launch's graphics switches on the command line.
 *
 * @returns The launch's graphics set-up, or `undefined` once a relaunch has been asked for and this
 * process is exiting.
 */
function prepareGraphics(
  spike: SpikeLaunch | undefined,
  viewsCheck: boolean,
):
  | { readonly graphics: GraphicsLaunch; readonly switches: ReadonlyArray<ChromiumSwitch> }
  | undefined {
  // `process.argv.slice(1)`: Electron supplies the executable itself (R01 Design note 3).
  const relaunchArgs = x11RelaunchArgs(process.argv.slice(1), process.env, process.platform);
  if (relaunchArgs !== undefined) {
    app.relaunch({ args: [...relaunchArgs] });
    app.exit(0);
    return undefined;
  }
  // Without it Chromium blocked WebGPU for the page after the second GPU-process crash, so the
  // client never got the chance to report and recover (R01 Design note 6).
  app.disableDomainBlockingFor3DAPIs();
  const mode = launchModeOf(process.platform, app.commandLine.hasSwitch(SAFE_MODE_SWITCH));
  // The timing toggle is one of the forced path's switches: nothing else lifts the quantization.
  // A spike run always asks for it (T14.b's `launchSwitches`), and so does a views check (R07.T20).
  const gpuTiming =
    mode === "vulkan" &&
    (spike !== undefined || viewsCheck || app.commandLine.hasSwitch(GPU_TIMING_SWITCH));
  const switches = launchSwitches(
    { platform: process.platform, mode, gpuTiming },
    spike === undefined ? undefined : { dawnSafety: spike.dawnSafety },
  );
  applyGraphicsSwitches(app.commandLine, switches);
  return { graphics: { launchMode: mode, gpuTiming }, switches };
}

async function main(): Promise<void> {
  // Before the app is ready, so that `--help` and a usage error answer without a window appearing.
  const args = resolveArgs();
  if (args === undefined) {
    return;
  }
  const serverUrl = serverUrlOf(args);
  // Also before `ready`: Chromium reads its switches when the GPU process starts.
  const prepared = prepareGraphics(args.spike, args.viewsCheck !== undefined);
  if (prepared === undefined) {
    return;
  }
  const { graphics, switches } = prepared;
  const nvidiaBaselineBytes = args.spike === undefined ? null : nvidiaBaseline();
  // Watching from before `ready`, so that no crash of the GPU process goes uncounted.
  const gpuMonitor = new GpuProcessMonitor({
    app,
    windows: () => BrowserWindow.getAllWindows(),
    mode: graphics.launchMode,
    args: process.argv.slice(1),
    nowMs: () => performance.now(),
  });
  app.once("will-quit", () => {
    gpuMonitor.dispose();
  });

  await app.whenReady();
  denyPermissionRequests(session.defaultSession);
  const spike = args.spike;
  if (spike !== undefined) {
    const hidden = spike.smoke || process.env[SPIKE_HIDDEN_ENV] === "1";
    const window = createWindow(serverUrl, graphics, { launch: spike, hidden });
    startSpikeSession(window, spike, graphics, switches, hidden, nvidiaBaselineBytes);
    return;
  }
  const viewsCheck = args.viewsCheck;
  if (viewsCheck !== undefined) {
    const hidden = viewsCheck.smoke || process.env[VIEWS_CHECK_HIDDEN_ENV] === "1";
    const window = createWindow(serverUrl, graphics, null, { launch: viewsCheck, hidden });
    startViewsCheckSession(window, viewsCheck, graphics, switches, hidden);
    return;
  }
  createWindow(serverUrl, graphics, null);

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow(serverUrl, graphics, null);
    }
  });
}

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

main().catch((error: unknown) => {
  console.error("failed to start:", error);
  app.exit(1);
});
