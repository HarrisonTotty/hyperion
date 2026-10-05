import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { CommanderError } from "commander";
import {
  app,
  BrowserWindow,
  contentTracing,
  ipcMain,
  type IpcMainInvokeEvent,
  screen,
  session,
  shell,
} from "electron";

import type { SpikeLaunch } from "../preload/api";
import { type GraphicsLaunch, graphicsArguments } from "../preload/graphicsLaunch";
import { serverUrlSwitch } from "../preload/serverUrl";
import { spikeSwitch } from "../preload/spikeLaunch";
import { type ClientArgs, parseClientArgs, serverUrlOf, userArgs } from "./cli";
import { parseNvidiaSmi, readDrmMemory, readNvidiaSmi } from "./fdinfo";
import {
  applyGraphicsSwitches,
  type ChromiumSwitch,
  GPU_TIMING_SWITCH,
  launchModeOf,
  SAFE_MODE_SWITCH,
} from "./graphics/switches";
import { isOwnPage } from "./ipcSender";
import { reduceTraceFile } from "./reduceTrace";
import { describeMachine, MemorySampler, nodeMachineSources } from "./results";
import { launchSwitches, registerSpikeHandlers, SpikeTrace } from "./spike";
import { SpikeSession } from "./spikeSession";
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
function spikeSize(launch: SpikeLaunch): { readonly width: number; readonly height: number } {
  return launch.setting === "high" ? { width: 1920, height: 1080 } : { width: 1280, height: 720 };
}

function createWindow(
  serverUrl: string,
  graphics: GraphicsLaunch,
  spike: SpikeWindow | null,
): BrowserWindow {
  const size = spike === null ? { width: 1600, height: 900 } : spikeSize(spike.launch);
  const window = new BrowserWindow({
    ...size,
    useContentSize: spike !== null,
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
      ],
      // A hidden spike run renders offscreen, as the smoke harness's does: a hidden window on a
      // hardware adapter draws nothing otherwise, and headless Ozone's GPU process exits there.
      ...(spike?.hidden === true ? { offscreen: true, backgroundThrottling: false } : {}),
    },
  });

  if (spike?.hidden !== true) {
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

/**
 * Wires a spike run's handlers to its window (T13.c): the trace, the memory sampler and the
 * results file, each call checked against the window's own page.
 */
function startSpikeSession(
  window: BrowserWindow,
  launch: SpikeLaunch,
  graphics: GraphicsLaunch,
  switches: ReadonlyArray<ChromiumSwitch>,
  hidden: boolean,
  nvidiaBaselineBytes: number | null,
): void {
  const trace = new SpikeTrace(contentTracing, { profiled: launch.traceProfile === "on" });
  const startedAt = new Date();
  /** The renderer's last private-memory reading, bytes, for the sampler. */
  let rendererBytes: number | null = null;
  const memory = new MemorySampler(
    {
      appMetrics: () => app.getAppMetrics(),
      rendererPrivateBytes: () => Promise.resolve(rendererBytes),
      drm: (pid) => readDrmMemory(pid, process.platform),
      nvidia: () => readNvidiaSmi(),
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
    log: (line) => {
      process.stdout.write(`${line}\n`);
    },
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

/**
 * Sets up the GPU before `ready`: relaunches a Wayland session through XWayland, or puts the
 * launch's graphics switches on the command line.
 *
 * @returns The launch's graphics set-up, or `undefined` once a relaunch has been asked for and this
 * process is exiting.
 */
function prepareGraphics(
  spike: SpikeLaunch | undefined,
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
  // A spike run always asks for it (T14.b's `launchSwitches`).
  const gpuTiming =
    mode === "vulkan" && (spike !== undefined || app.commandLine.hasSwitch(GPU_TIMING_SWITCH));
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
  const prepared = prepareGraphics(args.spike);
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
