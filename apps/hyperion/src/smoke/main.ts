/**
 * The headless smoke harness's main process (R01.T9, Design note 17): a plain Electron entry that
 * loads `smoke.html` in a hidden offscreen window, refuses every request but `file:` and `data:`,
 * waits for the page's one report, prints it a line a check and exits with 0 (pass), 1 (a property
 * failed or a request went out), 2 (a setup error, such as no adapter) or 3 (the watchdog).
 *
 * @remarks
 * Its switches come from the `just test-render` recipe's command line, never from the client's
 * `graphicsSwitches`, which a test keeps free of the unsafe flag and the adapter override. The
 * page's variant and fixture arrive as `--smoke-variant=` and `--smoke-fixture=` and reach it in
 * the query string. The main process makes no request itself (Node's `fetch` would bypass
 * Chromium's `webRequest`).
 */

import { join } from "node:path";

import { app, BrowserWindow, ipcMain, session } from "electron";

import {
  type CancelledRequest,
  isOfflineUrl,
  judgeSmokeRun,
  readSmokeResult,
  SMOKE_EXIT,
} from "./result";

/** The one channel the page reports on. */
export const SMOKE_RESULT_CHANNEL = "smoke:result";

/** How long a run may take before the watchdog ends it. */
const WATCHDOG_MS = 180_000;

/** The value of `--<name>=value` on the command line, or `fallback`. */
function argument(name: string, fallback: string): string {
  const prefix = `--${name}=`;
  return process.argv.find((arg) => arg.startsWith(prefix))?.slice(prefix.length) ?? fallback;
}

const variant = argument("smoke-variant", "default");
const fixture = argument("smoke-fixture", "none");
const cancelled: CancelledRequest[] = [];

function finish(lines: ReadonlyArray<string>, exitCode: number): void {
  for (const line of lines) {
    process.stdout.write(`${line}\n`);
  }
  app.exit(exitCode);
}

app.on("window-all-closed", () => {
  // The run ends by its report or the watchdog, never by a closed window.
});

void app
  .whenReady()
  .then(async (): Promise<void> => {
    const watchdog = setTimeout(() => {
      finish([`WATCHDOG no report after ${WATCHDOG_MS} ms`], SMOKE_EXIT.watchdog);
    }, WATCHDOG_MS);
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
      show: false,
      webPreferences: {
        // Headless Ozone segfaults without offscreen rendering (Design note 17).
        offscreen: true,
        preload: join(__dirname, "../preload/smoke.js"),
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
      },
    });
    ipcMain.handle(SMOKE_RESULT_CHANNEL, (event, report: unknown) => {
      if (event.senderFrame?.url.startsWith("file:") !== true) {
        return;
      }
      clearTimeout(watchdog);
      const result = readSmokeResult(report);
      if (result === null) {
        finish(["SETUP the page's report is malformed"], SMOKE_EXIT.setup);
        return;
      }
      const { lines, exitCode } = judgeSmokeRun(result, cancelled);
      finish(lines, exitCode);
    });
    window.webContents.on("console-message", (details) => {
      if (details.level === "error" || details.level === "warning") {
        process.stderr.write(`page ${details.level}: ${details.message}\n`);
      }
    });
    await window.loadFile(join(__dirname, "../renderer/smoke.html"), {
      query: { variant, fixture },
    });
    return undefined;
  })
  .catch((error: unknown) => {
    finish([`SETUP ${error instanceof Error ? error.message : String(error)}`], SMOKE_EXIT.setup);
  });
