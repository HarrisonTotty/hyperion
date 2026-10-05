/**
 * The headless smoke harness's main process (R01.T9, Design note 17): a plain Electron entry that
 * loads `smoke.html` in a hidden offscreen window, refuses every request but `file:` and `data:`,
 * waits for the page's one report, prints it a line a check and exits with 0 (pass), 1 (a property
 * failed, no check ran, a request went out or the page logged an uncaptured GPU error), 2 (a setup
 * error, such as no adapter) or 3 (the watchdog).
 *
 * @remarks
 * Its switches come from the `just test-render` recipe's command line, never from the client's
 * `graphicsSwitches`, which a test keeps free of the unsafe flag and the adapter override. The
 * page's variant and fixture arrive as `--smoke-variant=` and `--smoke-fixture=`;
 * `--smoke-gpu-timing=1` says the run lifted timestamp quantization (the hardware runs of T11),
 * and `--smoke-soak=<seconds>` runs T11's and T12's soak scene instead of the checks, hidden and
 * offscreen unless `--smoke-show=1`. They reach the page in the query string. The main process
 * makes no request itself (Node's `fetch` would bypass Chromium's `webRequest`).
 * `--smoke-captures=<dir>` asks the page for R05.T12.c's atmosphere comparison frames, which are
 * saved there as PNGs, for a person to look at.
 *
 * `--smoke-child=<seconds>` runs R07.T21's child window instead (`childWindow.ts`): on the client's
 * own graphics switches, shown, the child on a second display, refused with a setup error (exit 2)
 * before any window opens when Electron sees one display; `--smoke-child-hidden=1` puts it
 * offscreen on the same display instead, to prove the harness. `--smoke-out=<dir>` writes the
 * run's record there.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { app, BrowserWindow, ipcMain, nativeImage, screen, session } from "electron";

import {
  type CancelledRequest,
  isOfflineUrl,
  isUncapturedGpuError,
  judgeSmokeRun,
  readSmokeImages,
  readSmokeResult,
  SMOKE_EXIT,
  SMOKE_RESULT_CHANNEL,
} from "./result";
import { isOwnPage } from "../main/ipcSender";
import { describeMachine, nodeMachineSources } from "../main/results";
import { applyGraphicsSwitches, graphicsSwitches, launchModeOf } from "../main/graphics/switches";
import {
  CHILD_FRAME_NAME,
  type ChildWindowRun,
  childDisplayOf,
  childOpenHandler,
  childWindowMarkdown,
  type DisplayInfo,
  displayLine,
  machineName,
  recordName,
  refusalLine,
  withGpuExits,
} from "./childWindow";
import { startSoak } from "./soak";

/** The value of `--<name>=value` on the command line, or `fallback`. */
function argument(name: string, fallback: string): string {
  const prefix = `--${name}=`;
  return process.argv.find((arg) => arg.startsWith(prefix))?.slice(prefix.length) ?? fallback;
}

const variant = argument("smoke-variant", "default");
const fixture = argument("smoke-fixture", "none");
const gpuTiming = argument("smoke-gpu-timing", "0");
/** The soak's length in seconds (T11, T12); 0 runs the checks. */
const soakSeconds = Number(argument("smoke-soak", "0"));
const show = argument("smoke-show", "0") === "1";
const resize = argument("smoke-resize", "1") === "1";
const video = argument("smoke-video", "");
const capturePath = argument("smoke-capture", "");
/** Where R05.T12.c's comparison frames are saved, or "" for none. */
const capturesDir = argument("smoke-captures", "");
/** R07.T21's child-window run's length in seconds; 0 runs the checks. */
const childSeconds = Number(argument("smoke-child", "0"));
/** Whether the child is offscreen on the main display: the harness's proof on one display. */
const childHidden = argument("smoke-child-hidden", "0") === "1";
/** Where the child-window run's record goes, or "" for none. */
const outDir = argument("smoke-out", "");
const cancelled: CancelledRequest[] = [];
/** The page's uncaptured GPU errors, as the engine logged them. */
const gpuErrors: string[] = [];
/** The page's own URL, without its query: the only frame allowed to report. */
const PAGE_URL = pathToFileURL(join(__dirname, "../renderer/smoke.html")).href;

/**
 * Saves the report's images as `<name>-<variant>.png` in `dir` (`nativeImage` takes BGRA), a line
 * each; an image that cannot be saved, and the count of malformed ones, are lines too, so that the
 * judged checks are never lost to a capture.
 */
function saveImages(dir: string, report: unknown): string[] {
  const { images, rejected } = readSmokeImages(report);
  const lines = rejected > 0 ? [`CAPTURE REJECTED ${rejected} malformed images`] : [];
  try {
    mkdirSync(dir, { recursive: true });
  } catch (error: unknown) {
    return [...lines, `CAPTURE FAILED ${dir}: ${String(error)}`];
  }
  for (const image of images) {
    const path = join(dir, `${image.name}-${variant}.png`);
    try {
      const rgba = Buffer.from(image.rgba, "base64");
      const bgra = Buffer.alloc(rgba.length);
      for (let i = 0; i + 3 < rgba.length; i += 4) {
        bgra[i] = rgba[i + 2] ?? 0;
        bgra[i + 1] = rgba[i + 1] ?? 0;
        bgra[i + 2] = rgba[i] ?? 0;
        bgra[i + 3] = rgba[i + 3] ?? 255;
      }
      const png = nativeImage
        .createFromBitmap(bgra, { width: image.width, height: image.height })
        .toPNG();
      writeFileSync(path, png);
      lines.push(`CAPTURE ${path}`);
    } catch (error: unknown) {
      lines.push(`CAPTURE FAILED ${path}: ${String(error)}`);
    }
  }
  return lines;
}

function finish(lines: ReadonlyArray<string>, exitCode: number): void {
  for (const line of lines) {
    process.stdout.write(`${line}\n`);
  }
  app.exit(exitCode);
}

/** A harness window refuses every other window and every navigation. */
function lockDown(window: BrowserWindow): void {
  window.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  window.webContents.on("will-navigate", (event) => {
    event.preventDefault();
  });
}

app.on("window-all-closed", () => {
  // The run ends by its report or the watchdog, never by a closed window.
});

if (!Number.isFinite(soakSeconds) || soakSeconds < 0) {
  finish([`SETUP --smoke-soak must be a number of seconds, not ${soakSeconds}`], SMOKE_EXIT.setup);
}
if (!Number.isFinite(childSeconds) || childSeconds < 0) {
  finish(
    [`SETUP --smoke-child must be a number of seconds, not ${String(childSeconds)}`],
    SMOKE_EXIT.setup,
  );
}

/** The client's own switches, for the child-window run: T21 asks of the client's graphics. */
const childSwitches =
  childSeconds > 0
    ? graphicsSwitches({
        platform: process.platform,
        mode: launchModeOf(process.platform, false),
        gpuTiming: true,
      })
    : [];
// Before `ready`: Chromium reads its switches when the GPU process starts.
applyGraphicsSwitches(app.commandLine, childSwitches);

/** How long a run may take before the watchdog ends it. */
const WATCHDOG_MS = 180_000 + (soakSeconds + childSeconds) * 1000;

/** A display as the child-window run records it. */
function displayInfo(display: Electron.Display): DisplayInfo {
  return {
    id: display.id,
    label: display.label,
    bounds: display.bounds,
    workArea: display.workArea,
    displayFrequency: display.displayFrequency,
    scaleFactor: display.scaleFactor,
  };
}

/** The child-window run's displays: the opener's (the primary) and the child's, or a refusal. */
function childDisplays():
  | {
      readonly kind: "ready";
      readonly all: DisplayInfo[];
      readonly main: DisplayInfo;
      readonly child: DisplayInfo;
    }
  | { readonly kind: "refused"; readonly line: string } {
  const all = screen.getAllDisplays().map(displayInfo);
  const main = displayInfo(screen.getPrimaryDisplay());
  const child = childDisplayOf(all, main.id) ?? (childHidden ? main : null);
  return child === null
    ? { kind: "refused", line: refusalLine(all) }
    : { kind: "ready", all, main, child };
}

/** Writes the child-window run's record into `--smoke-out`, a line saying where (or why not). */
function writeChildRecord(run: ChildWindowRun, lines: ReadonlyArray<string>): string[] {
  if (outDir === "") {
    return [];
  }
  const machine = machineName();
  const path = join(outDir, recordName(run, machine));
  try {
    mkdirSync(outDir, { recursive: true });
    writeFileSync(path, childWindowMarkdown(run, machine, lines));
    return [`RECORD ${path}`];
  } catch (error: unknown) {
    return [`RECORD FAILED ${path}: ${String(error)}`];
  }
}

void app
  .whenReady()
  .then(async (): Promise<void> => {
    const watchdog = setTimeout(() => {
      finish([`WATCHDOG no report after ${WATCHDOG_MS} ms`], SMOKE_EXIT.watchdog);
    }, WATCHDOG_MS);
    session.defaultSession.setPermissionRequestHandler((_contents, _permission, decide) => {
      decide(false);
    });
    session.defaultSession.webRequest.onBeforeRequest(
      { urls: ["<all_urls>"] },
      (details, reply) => {
        if (isOfflineUrl(details.url)) {
          reply({});
          return;
        }
        cancelled.push({ url: details.url, resourceType: details.resourceType });
        reply({ cancel: true });
      },
    );
    const child = childSeconds > 0 ? childDisplays() : null;
    if (child?.kind === "refused") {
      clearTimeout(watchdog);
      finish([child.line], SMOKE_EXIT.setup);
      return;
    }
    const childStartedAt = new Date();
    // Read at the start, for the load average then.
    const childMachine =
      child === null
        ? null
        : await describeMachine(nodeMachineSources(() => app.getGPUInfo("basic")));
    // The child-window run is on screen, as T21 asks, unless it proves the harness hidden.
    const shown = show || (child !== null && !childHidden);
    const window = new BrowserWindow({
      show: shown,
      width: 1600,
      height: 900,
      webPreferences: {
        // Headless Ozone segfaults without offscreen rendering (Design note 17).
        offscreen: !shown,
        preload: join(__dirname, "../preload/smoke.js"),
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
      },
    });
    lockDown(window);
    const childLines: string[] = [];
    let gpuExits = 0;
    if (child !== null) {
      childLines.push(
        `T21 opener on ${displayLine(child.main)}`,
        `T21 child asked on ${displayLine(child.child)}`,
      );
      // The one window the page may open, once: its child, on the second display.
      window.webContents.setWindowOpenHandler(childOpenHandler(child.child, childHidden));
      window.webContents.on("did-create-window", (opened) => {
        // The child opens nothing and goes nowhere, as the harness's own window does not.
        lockDown(opened);
        const on = screen.getDisplayMatching(opened.getBounds());
        childLines.push(`T21 child opened on ${displayLine(displayInfo(on))}`);
        opened.once("closed", () => {
          childLines.push("T21 child window closed");
        });
      });
      // A shown child under the Vulkan surface is R07's on-screen check of a GPU-process restart.
      app.on("child-process-gone", (_event, details) => {
        if (details.type === "GPU") {
          gpuExits += 1;
          childLines.push(`T21 GPU process gone: ${details.reason}`);
        }
      });
    }
    const stopSoak =
      soakSeconds > 0
        ? startSoak({
            window,
            capturePath: capturePath === "" ? null : capturePath,
            seconds: soakSeconds,
            show,
            resize,
          })
        : (): Promise<void> => Promise.resolve();
    ipcMain.handle(SMOKE_RESULT_CHANNEL, async (event, report: unknown) => {
      if (!isOwnPage(event.senderFrame, PAGE_URL)) {
        return;
      }
      try {
        await stopSoak();
        const result = readSmokeResult(report);
        if (result === null) {
          finish(["SETUP the page's report is malformed"], SMOKE_EXIT.setup);
          return;
        }
        // The broken fixture's shader raises its own validation errors; its check already fails.
        const judged = judgeSmokeRun(result, cancelled, fixture === "broken-wgsl" ? [] : gpuErrors);
        const exits = child === null ? null : withGpuExits(judged.exitCode, gpuExits, SMOKE_EXIT);
        const lines = exits === null ? judged.lines : [...judged.lines, exits.line];
        const exitCode = exits?.exitCode ?? judged.exitCode;
        const saved = capturesDir === "" ? [] : saveImages(capturesDir, report);
        const recorded =
          child === null || childMachine === null
            ? []
            : writeChildRecord(
                {
                  startedAt: childStartedAt,
                  hidden: childHidden,
                  displays: child.all,
                  mainDisplayId: child.main.id,
                  childDisplayId: child.child.id,
                  versions: {
                    electron: process.versions.electron,
                    chromium: process.versions.chrome,
                  },
                  switches: [
                    ...process.argv.filter(
                      (arg) =>
                        arg.startsWith("--") &&
                        !arg.startsWith("--smoke-") &&
                        !arg.startsWith("--user-data-dir"),
                    ),
                    ...childSwitches.map(({ name, value }) =>
                      value === undefined ? `--${name}` : `--${name}=${value}`,
                    ),
                  ],
                  machine: childMachine,
                  exitCode,
                },
                [...childLines, ...lines],
              );
        finish([...childLines, ...lines, ...saved, ...recorded], exitCode);
      } catch (error: unknown) {
        finish([`SETUP the report could not be judged: ${String(error)}`], SMOKE_EXIT.setup);
      } finally {
        clearTimeout(watchdog);
      }
    });
    window.webContents.on("console-message", (details) => {
      if (details.level === "error" || details.level === "warning") {
        process.stderr.write(`page ${details.level}: ${details.message}\n`);
      }
      if (details.level === "error" && isUncapturedGpuError(details.message)) {
        gpuErrors.push(details.message);
      }
    });
    await window.loadFile(join(__dirname, "../renderer/smoke.html"), {
      query: {
        variant,
        fixture,
        // The child-window run lifts the quantization with the client's own timing switch.
        gpuTiming: child === null ? gpuTiming : "1",
        soak: String(soakSeconds),
        child: String(childSeconds),
        ...(child === null
          ? {}
          : {
              childFrame: CHILD_FRAME_NAME,
              childHz: String(child.child.displayFrequency),
              mainHz: String(child.main.displayFrequency),
              childHidden: childHidden ? "1" : "0",
            }),
        captures: capturesDir === "" ? "0" : "1",
        ...(video === "" ? {} : { video: pathToFileURL(video).href }),
      },
    });
    return undefined;
  })
  .catch((error: unknown) => {
    finish([`SETUP ${error instanceof Error ? error.message : String(error)}`], SMOKE_EXIT.setup);
  });
