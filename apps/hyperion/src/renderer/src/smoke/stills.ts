/**
 * The smoke page's still checks (plan R11, R11.T10.c and T10.b), in part 28, alone: a PNG that
 * Chromium's own encoder made, labelled, saved through the harness's main process (`main/stills.ts`,
 * in the run's own user-data directory) and read back, its label equal to its sidecar's and its
 * pixels to those encoded; then `VIEW`'s still mount, its list and its control, on that main
 * process's listing, its `TAKE STILL` making a small still through the job (`stillJob.ts`) on the
 * page's own device.
 *
 * @remarks
 * The photorealistic renderer's hook is not built (R11.T10.b's remainder), so the job draws its
 * tiles through {@link FlatRenderer}, a stand-in of flat colours into R01's render targets, its
 * pipeline made asynchronously, gated as every still is on the queue's `onSubmittedWorkDone()`, one
 * batch an animation frame, each tile read back through `readTexture`. It draws only into its own
 * targets, alone in its part, so no other part's frame or capture moves.
 */

import { createElement } from "react";
import { createRoot } from "react-dom/client";

import type { StillLabel, StillsApi } from "../../../preload/api";
import type { ViewStillsModel } from "../displays/view/useViewStills";
import { ViewStills } from "../displays/view/ViewStills";
import type {
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  TargetRender,
} from "../view/engine/types";
import { scanPng, withStillLabel } from "../view/stills/pngLabel";
import { stillsOver } from "../view/stills/stillAnswers";
import {
  encodeStillPng,
  type StillDevice,
  type StillRenderer,
  StillSession,
  type StillSource,
} from "../view/stills/stillJob";
import {
  type StillRequest,
  stillLabel,
  stillLabelText,
  STILL_TERRAIN_STREAMING,
} from "../view/stills/stillLabel";
import { planTiles, type StillTile, type StillTiling } from "../view/stills/stillPlan";
import { StillStore } from "../view/stills/stillStore";
import { type Checks, drawOf, flatSpec, frameOf, fullScreenMesh, pause } from "./harness";

/** The test image's size, px: a width that is no multiple of 64, as a still's need not be. */
const SIZE = { widthPx: 40, heightPx: 24 } as const;

/** How long the mount may take to list and draw, ms, polled every {@link POLL_MS}. */
const MOUNT_PATIENCE_MS = 10_000;

const POLL_MS = 10;

/** `value`'s function `name`, called on `value`, or `null` where it has none. */
function callOf(value: object, name: string): ((...args: unknown[]) => Promise<unknown>) | null {
  const call: unknown = Reflect.get(value, name);
  return typeof call === "function"
    ? (...args: unknown[]) => Promise.resolve(Reflect.apply(call, value, args))
    : null;
}

/**
 * The still functions over the smoke's preload's raw still calls (`src/smoke/preload.ts`), each
 * answer read back as the client's preload reads it (`stillAnswers.ts`); `null` where the bridge
 * has none.
 */
export function smokeStills(): StillsApi | null {
  const smoke: unknown = Reflect.get(window, "smoke");
  const calls: unknown =
    typeof smoke === "object" && smoke !== null ? Reflect.get(smoke, "stills") : undefined;
  if (typeof calls !== "object" || calls === null) {
    return null;
  }
  const save = callOf(calls, "save");
  const list = callOf(calls, "list");
  const read = callOf(calls, "read");
  return save === null || list === null || read === null
    ? null
    : stillsOver({
        save: (png, label) => save(png, label),
        list: () => list(),
        read: (id) => read(id),
      });
}

/** An opaque pattern of every channel, so that no premultiplication can round it. */
function testPixels(): Uint8ClampedArray<ArrayBuffer> {
  const pixels = new Uint8ClampedArray(SIZE.widthPx * SIZE.heightPx * 4);
  for (let i = 0; i < SIZE.widthPx * SIZE.heightPx; i += 1) {
    pixels[4 * i] = (i * 37) & 0xff;
    pixels[4 * i + 1] = (i * 101 + 17) & 0xff;
    pixels[4 * i + 2] = 255 - ((i * 13) & 0xff);
    pixels[4 * i + 3] = 255;
  }
  return pixels;
}

/** A request as a view holds it, for a still of `size`. */
function testRequest(size: { readonly widthPx: number; readonly heightPx: number }): StillRequest {
  return {
    time: { seconds: 412_345_678, nanos: 250_000_000 },
    camera: {
      frame: { kind: "body", body: "0200080020000000.0100" },
      positionM: [6_771_000, -12_500.25, 340.5],
      orientation: { w: 1, x: 0, y: 0, z: 0 },
      fovXDeg: 60,
    },
    exposure: { kind: "manual", ev100: 12 },
    readings: [
      { label: "FRAME", value: "BODY TEST EARTH" },
      { label: "TIME", value: "UT +13 yr 024/06:27:58" },
      { label: "FOV", value: "60°" },
      { label: "EXPOSURE", value: "EV100 12.0 MAN" },
    ],
    setting: "high",
    decoration: true,
    survey: [
      {
        body: "0200080020000000.0100",
        name: "TEST EARTH",
        level: 7,
        surveyedFraction: 0.5,
        bestResolutionM: 1200,
      },
    ],
    annunciations: [],
    size,
  };
}

/** The test still's label, its frustum not streamed in time, so that it says so. */
function testLabel(): StillLabel {
  return stillLabel(testRequest(SIZE), false);
}

/** The pixels Chromium decodes `png` to, unconverted and unpremultiplied. */
async function decoded(png: Uint8Array<ArrayBuffer>): Promise<Uint8ClampedArray> {
  const bitmap = await createImageBitmap(new Blob([png], { type: "image/png" }), {
    colorSpaceConversion: "none",
    premultiplyAlpha: "none",
  });
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
  const context = canvas.getContext("2d");
  if (context === null) {
    throw new Error("an OffscreenCanvas gave no 2D context");
  }
  context.drawImage(bitmap, 0, 0);
  bitmap.close();
  return context.getImageData(0, 0, canvas.width, canvas.height).data;
}

/** Whether two byte arrays are equal, byte for byte. */
function sameBytes(a: ArrayLike<number>, b: ArrayLike<number>): boolean {
  if (a.length !== b.length) {
    return false;
  }
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) {
      return false;
    }
  }
  return true;
}

/** Waits until `ready` holds, polling, or the patience runs out; whether it held. */
async function until(ready: () => boolean): Promise<boolean> {
  for (let waited = 0; waited < MOUNT_PATIENCE_MS; waited += POLL_MS) {
    if (ready()) {
      return true;
    }
    // A poll: each wait follows the last look, by design.
    // oxlint-disable-next-line no-await-in-loop
    await pause(POLL_MS);
  }
  return ready();
}

/** The PNG round trip through the main process: save, read back, decode. */
async function checkRoundTrip(
  checks: Checks,
  stills: StillsApi,
  encoded: Uint8Array<ArrayBuffer>,
  pixels: Uint8ClampedArray,
  label: StillLabel,
): Promise<void> {
  const store = new StillStore(stills);
  const answer = await store.save(encoded, label);
  checks.check(
    "R11.T10.c the main process saves a labelled still",
    answer.kind === "saved",
    JSON.stringify(answer.kind === "saved" ? { kind: answer.kind, id: answer.still.id } : answer),
  );
  if (answer.kind !== "saved") {
    return;
  }
  const read = await store.read(answer.still.id);
  checks.check(
    "R11.T10.c the still's PNG label parses back equal to its sidecar",
    read.kind === "read" && stillLabelText(read.still.label) === stillLabelText(label),
    read.kind === "read" ? stillLabelText(read.still.label) : JSON.stringify(read),
  );
  if (read.kind !== "read") {
    return;
  }
  const sent = withStillLabel(encoded, label);
  checks.check(
    "R11.T10.c the still's PNG reads back byte for byte as it was sent",
    sameBytes(read.still.png, sent),
    `${String(read.still.png.byteLength)} bytes read, ${String(sent.byteLength)} sent`,
  );
  const back = await decoded(new Uint8Array(read.still.png));
  checks.check(
    "R11.T10.c Chromium decodes the labelled still to the pixels it encoded",
    sameBytes(back, pixels),
    `${String(back.length)} bytes decoded, ${String(pixels.length)} encoded`,
  );
  const refused = await stills.saveStill(encoded, label);
  checks.check(
    "R11.T10.c the main process refuses a PNG whose chunk is not its sidecar's label",
    refused.kind === "refused" && refused.reason === "label-mismatch",
    JSON.stringify(refused),
  );
}

/** The job's still: four tiles at a 64 px tile limit, its last column padded. */
const JOB_SIZE = { widthPx: 100, heightPx: 70 } as const;

/** The tile limit the job's still is cut at, px: R11.T10.a's least, 64. */
const JOB_TILE_LIMIT_PX = 64;

/** The passes a tile of the job's still takes, each a flat colour of its own over the tile. */
const JOB_PASSES = ["base", "tint"] as const;

/** The colour pass `pass` of tile `tile` draws, 8-bit: the last pass's is the tile's. */
function clearColour(tile: number, pass: number): readonly [number, number, number] {
  return [(40 * (tile + 1)) & 0xff, (60 * (pass + 1)) & 0xff, 200 - 30 * tile];
}

/**
 * A still renderer on the page's engine, standing in for the photorealistic renderer's hook: each
 * tile a render target of R01's `createRenderTarget`, each unit a draw of a flat colour over the
 * whole tile, a batch one `renderTargets` submission, each tile read back through `readTexture`.
 * Its one pipeline is made by `createMaterialAsync` in `prepare()`, as every still's are.
 */
class FlatRenderer implements StillRenderer {
  readonly tiling: StillTiling;
  readonly units: ReadonlyArray<string>;
  readonly #engine: RenderEngine;
  readonly #mesh: MeshHandle;
  readonly #targets: ReadonlyArray<RenderTarget>;
  #material: MaterialHandle | null = null;

  constructor(engine: RenderEngine) {
    this.#engine = engine;
    this.tiling = planTiles(JOB_SIZE, JOB_TILE_LIMIT_PX, "low");
    this.units = this.tiling.tiles.flatMap((_, tile) =>
      JOB_PASSES.map((pass) => `${pass} ${String(tile)}`),
    );
    this.#mesh = fullScreenMesh(engine, "smoke still triangle");
    this.#targets = this.tiling.tiles.map((tile, index) =>
      engine.createRenderTarget({
        name: `smoke still tile ${String(index)}`,
        size: tile.texture,
        format: "rgba8unorm",
        mips: 1,
        depth: true,
        category: "render-targets",
      }),
    );
  }

  async prepare(): Promise<void> {
    this.#material = await this.#engine.createMaterialAsync(
      flatSpec("smoke still flat"),
      ["rgba8unorm"],
      [this.#mesh],
    );
  }

  streamed(): boolean {
    return true;
  }

  submit(first: number, count: number): void {
    const material = this.#material;
    if (material === null) {
      throw new Error("a still's batch was submitted before its pipelines were made");
    }
    const renders: TargetRender[] = [];
    for (let unit = first; unit < first + count; unit += 1) {
      const tile = Math.floor(unit / JOB_PASSES.length);
      const target = this.#targets[tile];
      if (target === undefined) {
        throw new Error(`unit ${String(unit)} has no tile`);
      }
      const [r, g, b] = clearColour(tile, unit % JOB_PASSES.length);
      renders.push({
        target,
        frame: frameOf(this.units[unit] ?? "smoke still unit", [
          drawOf(this.#mesh, material, [r / 255, g / 255, b / 255, 1]),
        ]),
      });
    }
    this.#engine.renderTargets(renders);
  }

  async readTile(tile: StillTile): Promise<Uint8Array> {
    const target = this.#targets[this.tiling.tiles.indexOf(tile)];
    if (target === undefined) {
      throw new Error("a tile that is not the still's was read back");
    }
    return new Uint8Array(await this.#engine.readTexture(target.colour));
  }

  dispose(): void {
    for (const target of this.#targets) {
      target.dispose();
    }
    if (this.#material !== null) {
      this.#engine.releaseMaterial(this.#material);
    }
    this.#engine.releaseMesh(this.#mesh);
  }
}

/**
 * The job's device on the page's own: the queue's `onSubmittedWorkDone()` and empty submits, the
 * animation frame, the clock and the device's loss; no timestamps, so the batches are sized from
 * round trips.
 */
function pageStillDevice(device: GPUDevice): StillDevice {
  return {
    timer: "absent",
    submittedWorkDone: () => device.queue.onSubmittedWorkDone(),
    submitEmpty: () => {
      device.queue.submit([]);
    },
    nextFrame: () =>
      new Promise((resolve) => {
        requestAnimationFrame(() => {
          resolve();
        });
      }),
    nowNs: () => performance.now() * 1e6,
    onPassTimes: () => () => undefined,
    onLoss: (listener) => {
      let live = true;
      device.lost
        .then(() => {
          if (live) {
            listener();
          }
          return undefined;
        })
        .catch((error: unknown) => {
          console.error("the device's loss could not be watched:", error);
        });
      return () => {
        live = false;
      };
    },
  };
}

/** A still source over the page's engine and device: the test request, drawn by {@link FlatRenderer}. */
function pageStillSource(engine: RenderEngine, device: GPUDevice): StillSource {
  return {
    device: pageStillDevice(device),
    setting: "low",
    frameIntervalNs: () => 16_683_350,
    snapshot: () => ({ request: testRequest(JOB_SIZE) }),
    renderer: () => new FlatRenderer(engine),
  };
}

/** The tile of the job's still that the pixel (x, y) lies in, row by row from the top left. */
function jobTileOf(tiling: StillTiling, x: number, y: number): number {
  return tiling.tiles.findIndex(
    ({ rect }) => x >= rect.x0Px && x < rect.x1Px && y >= rect.y0Px && y < rect.y1Px,
  );
}

/** How many of the still's pixels are not their tile's last pass's colour: 0 where every one is. */
function wrongPixels(pixels: ArrayLike<number>): number {
  const tiling = planTiles(JOB_SIZE, JOB_TILE_LIMIT_PX, "low");
  let wrong = 0;
  for (let y = 0; y < JOB_SIZE.heightPx; y += 1) {
    for (let x = 0; x < JOB_SIZE.widthPx; x += 1) {
      const at = (y * JOB_SIZE.widthPx + x) * 4;
      const expected = [...clearColour(jobTileOf(tiling, x, y), JOB_PASSES.length - 1), 255];
      if (!expected.every((channel, k) => pixels[at + k] === channel)) {
        wrong += 1;
      }
    }
  }
  return wrong;
}

/**
 * `VIEW`'s still mount on the main process's listing: the list and a label shown, then `TAKE STILL`
 * making a still through the job on the page's device, saved, listed and read back.
 */
async function checkMount(
  checks: Checks,
  stills: StillsApi,
  label: StillLabel,
  engine: RenderEngine,
  device: GPUDevice | undefined,
): Promise<void> {
  if (device === undefined) {
    checks.check("R11.T10.b the page has its device for a still", false, "no device recorded");
    return;
  }
  const store = new StillStore(stills);
  const model: ViewStillsModel = {
    store,
    session: new StillSession({
      source: pageStillSource(engine, device),
      encode: encodeStillPng,
      save: (png, still) => store.save(png, still),
      nowMs: () => performance.now(),
      reportError: (error) => {
        console.error("a still could not be made:", error);
      },
    }),
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    root.render(createElement(ViewStills, { designator: "PRIMARY", model }));
    const entries = (): HTMLButtonElement[] => [
      ...host.querySelectorAll<HTMLButtonElement>(".still-list__still"),
    ];
    const listed = await until(() => entries().length === 1);
    checks.check(
      "R11.T10.c VIEW's still list shows the saved still",
      listed,
      host.querySelector(".still-list, .still-list__empty")?.textContent ?? "nothing drawn",
    );
    const [button] = entries();
    if (button === undefined) {
      return;
    }
    button.click();
    const shown = await until(() => button.getAttribute("aria-expanded") === "true");
    const details = document.getElementById(button.getAttribute("aria-controls") ?? "");
    const text = details?.textContent ?? "";
    const size = `${String(label.size.widthPx)} × ${String(label.size.heightPx)} px`;
    checks.check(
      "R11.T10.c the still's label opens with its fields, TERRAIN: STREAMING among them",
      shown &&
        details?.hidden === false &&
        text.includes(STILL_TERRAIN_STREAMING) &&
        text.includes(size),
      text,
    );
    const take = [...host.querySelectorAll("button")].find(
      (each) => each.textContent === "TAKE STILL",
    );
    const status = (): string => host.querySelector(".still-control__status")?.textContent ?? "";
    const offered = take !== undefined && take.getAttribute("aria-disabled") === null;
    take?.click();
    const saved = await until(() => status() === "STILL SAVED");
    checks.check(
      "R11.T10.b TAKE STILL makes a still through the job, one batch in flight, and saves it",
      offered && saved,
      status(),
    );
    const listedBoth = await until(() => entries().length === 2);
    const [latest] = await stills.listStills();
    const read = latest === undefined ? null : await store.read(latest.id);
    const pixels = read?.kind === "read" ? await decoded(new Uint8Array(read.still.png)) : null;
    const wrong = pixels === null ? null : wrongPixels(pixels);
    checks.check(
      "R11.T10.b the still is listed and reads back, every texel its tile's last pass's",
      listedBoth && wrong === 0,
      wrong === null
        ? `read back: ${read?.kind ?? "nothing"}`
        : `${String(wrong)} of ${String(JOB_SIZE.widthPx * JOB_SIZE.heightPx)} pixels wrong`,
    );
  } finally {
    root.unmount();
    host.remove();
  }
}

/** Part 28's checks (R11.T10.c, T10.b): the PNG round trip, then the mount and the job. */
export async function checkStills(
  checks: Checks,
  stills: StillsApi | null,
  engine: RenderEngine,
  device: GPUDevice | undefined,
): Promise<void> {
  checks.check(
    "R11.T10.c the page has the still functions",
    stills !== null,
    stills === null ? "window.smoke.stills is absent" : "saveStill, listStills, readStill",
  );
  if (stills === null) {
    return;
  }
  const pixels = testPixels();
  const canvas = new OffscreenCanvas(SIZE.widthPx, SIZE.heightPx);
  const context = canvas.getContext("2d");
  if (context === null) {
    throw new Error("an OffscreenCanvas gave no 2D context");
  }
  context.putImageData(new ImageData(pixels, SIZE.widthPx, SIZE.heightPx), 0, 0);
  const encoded = new Uint8Array(
    await (await canvas.convertToBlob({ type: "image/png" })).arrayBuffer(),
  );
  const scan = scanPng(encoded);
  checks.check(
    "R11.T10.c convertToBlob gives a PNG of the size drawn",
    scan.kind === "png" &&
      scan.size.widthPx === SIZE.widthPx &&
      scan.size.heightPx === SIZE.heightPx,
    scan.kind === "png"
      ? `${String(scan.size.widthPx)} × ${String(scan.size.heightPx)} px, ${String(encoded.byteLength)} bytes`
      : scan.reason,
  );
  const label = testLabel();
  await checkRoundTrip(checks, stills, encoded, pixels, label);
  await checkMount(checks, stills, label, engine, device);
}
