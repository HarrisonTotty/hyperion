/**
 * The main process's side of the soak (R01.T11 and T12): it resizes the window in a loop, opens a
 * second window, and logs every GPU-process event, a changed GPU process, and any change of a
 * known DOM region between captures every 30 s.
 *
 * @remarks
 * Not part of `just test-render`. Hidden and offscreen by default, so that a soak never takes the
 * owner's display; `--smoke-show=1` shows both windows for the by-hand checks that need a
 * presenting window. Every line it prints starts `SOAK`.
 */

import { writeFileSync } from "node:fs";

import { app, BrowserWindow, type NativeImage } from "electron";

/** What the soak needs of the harness's main process. */
export interface SoakOptions {
  readonly window: BrowserWindow;
  readonly capturePath: string | null;
  /** The soak's length; the full capture is taken halfway, while frames are drawn. */
  readonly seconds: number;
  /** Whether the windows are shown and present, rather than hidden and offscreen. */
  readonly show: boolean;
}

/** A line of the soak's log, with the time since it began. */
function log(start: number, line: string): void {
  process.stdout.write(`SOAK ${((Date.now() - start) / 1000).toFixed(1)} s ${line}\n`);
}

/** The GPU process's pid in `app.getAppMetrics()`, or `null`. */
function gpuPid(): number | null {
  return app.getAppMetrics().find((metric) => metric.type === "GPU")?.pid ?? null;
}

/** The static label's region: 320 × 48 at the window's top right, inset 16 px. */
function labelRegion(window: BrowserWindow): Electron.Rectangle {
  const [width = 0] = window.getContentSize();
  return { x: width - 336, y: 16, width: 320, height: 48 };
}

/** Starts the soak's monitors and window loops; returns what stops them all. */
export function startSoak(options: SoakOptions): () => Promise<void> {
  const start = Date.now();
  const { window } = options;
  const onGone = (_event: Electron.Event, details: Electron.Details): void => {
    log(start, `child-process-gone ${JSON.stringify(details)}`);
  };
  const onInfo = (): void => {
    log(start, `gpu-info-update ${JSON.stringify(app.getGPUFeatureStatus())}`);
  };
  app.on("child-process-gone", onGone);
  app.on("gpu-info-update", onInfo);
  log(start, `feature status ${JSON.stringify(app.getGPUFeatureStatus())}`);
  let pid = gpuPid();
  log(start, `gpu pid ${pid}`);
  const sizes: ReadonlyArray<readonly [number, number]> = [
    [1600, 900],
    [1280, 720],
    [1920, 1080],
  ];
  let resizes = 0;
  const resize = setInterval(() => {
    resizes += 1;
    const [width, height] = sizes[resizes % sizes.length] ?? [1280, 720];
    window.setContentSize(width, height);
  }, 7_000);
  const second = new BrowserWindow({
    width: 400,
    height: 300,
    show: options.show,
    webPreferences: {
      offscreen: !options.show,
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  second.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  second
    .loadURL("data:text/html,<body style='background:%23123'><h1>second window</h1></body>")
    .catch((error: unknown) => {
      log(start, `the second window did not load: ${String(error)}`);
    });
  const poll = setInterval(() => {
    const now = gpuPid();
    if (now !== pid) {
      log(start, `gpu pid changed ${pid} -> ${now}`);
      pid = now;
    }
  }, 2_000);
  let reference: Buffer | null = null;
  let mismatches = 0;
  let captures = 0;
  const compare = setInterval(() => {
    window.webContents
      .capturePage(labelRegion(window))
      .then((image: NativeImage): void => {
        captures += 1;
        const bitmap = image.toBitmap();
        if (reference === null) {
          reference = bitmap;
        } else if (!bitmap.equals(reference)) {
          mismatches += 1;
          log(start, `DOM region mismatch at capture ${captures}`);
        }
        return undefined;
      })
      .catch((error: unknown) => {
        log(start, `capture failed: ${String(error)}`);
      });
  }, 30_000);
  const halfway = setTimeout(
    () => {
      const path = options.capturePath;
      if (path === null) {
        return;
      }
      window.webContents
        .capturePage()
        .then((image): void => {
          writeFileSync(path, image.toPNG());
          log(start, `capture saved to ${path} at ${window.getContentSize().join(" × ")} DIP`);
          return undefined;
        })
        .catch((error: unknown) => {
          log(start, `the halfway capture failed: ${String(error)}`);
        });
    },
    (options.seconds * 1000) / 2,
  );
  return () => {
    clearTimeout(halfway);
    clearInterval(resize);
    clearInterval(poll);
    clearInterval(compare);
    app.off("child-process-gone", onGone);
    app.off("gpu-info-update", onInfo);
    log(
      start,
      `resizes ${resizes}, captures ${captures}, DOM mismatches ${mismatches}, gpu pid ${pid}`,
    );
    second.destroy();
    return Promise.resolve();
  };
}
