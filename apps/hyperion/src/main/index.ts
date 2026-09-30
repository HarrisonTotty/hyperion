import { join } from "node:path";

import { CommanderError } from "commander";
import { app, BrowserWindow, shell } from "electron";

import { type GraphicsLaunch, graphicsArguments } from "../preload/graphicsLaunch";
import { serverUrlSwitch } from "../preload/serverUrl";
import { parseClientArgs, serverUrlOf, userArgs } from "./cli";
import {
  applyGraphicsSwitches,
  GPU_TIMING_SWITCH,
  graphicsSwitches,
  launchModeOf,
  SAFE_MODE_SWITCH,
} from "./graphics/switches";
import { GpuProcessMonitor } from "./graphics/gpuProcessMonitor";
import { x11RelaunchArgs } from "./graphics/x11Relaunch";
import { isSafeExternalUrl, isSameDocument } from "./navigation";

function reportLoadFailure(error: unknown): void {
  console.error("failed to load the renderer:", error);
}

/**
 * The server URL from the command line, or `undefined` once the command line has ended the run, as
 * `--help`, `--version` and a usage error do.
 */
function resolveServerUrl(): string | undefined {
  try {
    return serverUrlOf(parseClientArgs(userArgs(process.argv, app.isPackaged), app.getVersion()));
  } catch (error) {
    if (error instanceof CommanderError) {
      // The help, the version or the usage error has been written already.
      app.exit(error.exitCode);
      return undefined;
    }
    throw error;
  }
}

function createWindow(serverUrl: string, graphics: GraphicsLaunch): void {
  const window = new BrowserWindow({
    width: 1600,
    height: 900,
    minWidth: 1024,
    minHeight: 640,
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
      ],
    },
  });

  window.once("ready-to-show", () => {
    window.show();
  });

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
}

/**
 * Sets up the GPU before `ready`: relaunches a Wayland session through XWayland, or puts the
 * launch's graphics switches on the command line.
 *
 * @returns The launch's graphics set-up, or `undefined` once a relaunch has been asked for and this
 * process is exiting.
 */
function prepareGraphics(): GraphicsLaunch | undefined {
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
  const gpuTiming = app.commandLine.hasSwitch(GPU_TIMING_SWITCH);
  applyGraphicsSwitches(
    app.commandLine,
    graphicsSwitches({ platform: process.platform, mode, gpuTiming }),
  );
  return { launchMode: mode, gpuTiming };
}

async function main(): Promise<void> {
  // Before the app is ready, so that `--help` and a usage error answer without a window appearing.
  const serverUrl = resolveServerUrl();
  if (serverUrl === undefined) {
    return;
  }
  // Also before `ready`: Chromium reads its switches when the GPU process starts.
  const graphics = prepareGraphics();
  if (graphics === undefined) {
    return;
  }
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
  createWindow(serverUrl, graphics);

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow(serverUrl, graphics);
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
