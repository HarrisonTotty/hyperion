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
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { app, BrowserWindow, ipcMain, nativeImage, session } from "electron";

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

/** How long a run may take before the watchdog ends it. */
const WATCHDOG_MS = 180_000 + soakSeconds * 1000;

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
    const window = new BrowserWindow({
      show,
      width: 1600,
      height: 900,
      webPreferences: {
        // Headless Ozone segfaults without offscreen rendering (Design note 17).
        offscreen: !show,
        preload: join(__dirname, "../preload/smoke.js"),
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
      },
    });
    lockDown(window);
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
      const url = event.senderFrame?.url.split("?")[0];
      if (url !== PAGE_URL) {
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
        const { lines, exitCode } = judgeSmokeRun(
          result,
          cancelled,
          fixture === "broken-wgsl" ? [] : gpuErrors,
        );
        const saved = capturesDir === "" ? [] : saveImages(capturesDir, report);
        finish([...lines, ...saved], exitCode);
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
        gpuTiming,
        soak: String(soakSeconds),
        captures: capturesDir === "" ? "0" : "1",
        ...(video === "" ? {} : { video: pathToFileURL(video).href }),
      },
    });
    return undefined;
  })
  .catch((error: unknown) => {
    finish([`SETUP ${error instanceof Error ? error.message : String(error)}`], SMOKE_EXIT.setup);
  });
