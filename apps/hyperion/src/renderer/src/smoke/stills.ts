/**
 * The smoke page's still checks (plan R11, R11.T10.c), in part 28, alone: a PNG that Chromium's own
 * encoder made, labelled, saved through the harness's main process (`main/stills.ts`, in the run's
 * own user-data directory) and read back, its label equal to its sidecar's and its pixels to those
 * encoded; and `VIEW`'s still mount, its list and its control, on that main process's listing.
 *
 * @remarks
 * Nothing here draws on the GPU or touches the engine, so no other part's frame or capture moves.
 */

import { createElement } from "react";
import { createRoot } from "react-dom/client";

import type { StillLabel, StillsApi } from "../../../preload/api";
import { STILL_RENDERING_NOT_BUILT, ViewStills } from "../displays/view/ViewStills";
import { scanPng, withStillLabel } from "../view/stills/pngLabel";
import { stillsOver } from "../view/stills/stillAnswers";
import { stillLabel, stillLabelText, STILL_TERRAIN_STREAMING } from "../view/stills/stillLabel";
import { StillStore } from "../view/stills/stillStore";
import { type Checks, pause } from "./harness";

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

/** The test still's label, its frustum not streamed in time, so that it says so. */
function testLabel(): StillLabel {
  return stillLabel(
    {
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
      size: SIZE,
    },
    false,
  );
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

/** `VIEW`'s still mount on the main process's listing: the list, a label shown, the control. */
async function checkMount(checks: Checks, stills: StillsApi, label: StillLabel): Promise<void> {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    root.render(createElement(ViewStills, { designator: "PRIMARY", stills }));
    const entry = (): HTMLButtonElement | null =>
      host.querySelector<HTMLButtonElement>(".still-list__still");
    const listed = await until(() => entry() !== null);
    checks.check(
      "R11.T10.c VIEW's still list shows the saved still",
      listed,
      host.querySelector(".still-list, .still-list__empty")?.textContent ?? "nothing drawn",
    );
    const button = entry();
    if (button === null) {
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
    const reasonId = take?.getAttribute("aria-describedby") ?? "";
    checks.check(
      "R11.T10.c TAKE STILL is held back, saying why, until the renderer draws a still",
      take?.getAttribute("aria-disabled") === "true" &&
        document.getElementById(reasonId)?.textContent === STILL_RENDERING_NOT_BUILT,
      document.getElementById(reasonId)?.textContent ?? "no reason",
    );
  } finally {
    root.unmount();
    host.remove();
  }
}

/** Part 28's checks (R11.T10.c): the PNG round trip, then the mount. */
export async function checkStills(checks: Checks, stills: StillsApi | null): Promise<void> {
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
  await checkMount(checks, stills, label);
}
