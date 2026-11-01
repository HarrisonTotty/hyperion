/**
 * The smoke page's captures of the view's light with the host star in frame (fix-view-light, the
 * owner's white frame, 2026-11-01): the kept phase scene (`view/scenes/phaseScene.ts`) as the view's
 * photorealistic frame assembles it, at 1920 × 1080, exposed by the view's own controller
 * (`AutoExposure`) from the histograms the renderer reads back, each framing a new scene that the
 * controller cuts to.
 *
 * @remarks
 * Four framings, each settled and captured with its verdict:
 *
 * - (a) `LIT` on the crescent planet with the Sun at the frame's left edge: the disc saturated
 *   (sRGB 250 or more), the sky beyond 30° from the Sun at a median below code 32; and the same
 *   framing turned 1.1° so that the Sun is just outside, whose exposure is within 0.3 EV of it.
 * - (b) The first framing under `AVG`: the frame's median below code 128, the disc saturated.
 * - (c) The Sun alone in a dark field under `AVG`: the frame's median below code 64, the white
 *   frame's regression capture.
 *
 * The meter classes the scene target holds tell the disc, the lit planet and the sky apart.
 */

import { photorealFrame } from "../displays/view/photorealFrame";
import { startRun } from "../displays/view/viewRun";
import { type Vec3, vec3 } from "../geometry/vec3";
import type { CameraPose } from "../view/camera/pose";
import type { Viewport } from "../view/camera/projection";
import { conjugate, lookAlong, rotate } from "../view/camera/quaternion";
import type { RenderEngine, RenderView } from "../view/engine/types";
import {
  controlEv100,
  type ExposureControl,
  exposureScale,
  VIEW_CAMERA,
} from "../view/photometry/exposure";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import { AutoExposure } from "../view/post/autoExposure";
import type { Histogram } from "../view/post/histogram";
import { METER_CLASS, type MeterMode } from "../view/post/meter";
import { viewId } from "../view/camera/state";
import { phasePlanetDirection, phaseScene } from "../view/scenes/phaseScene";
import { HostDiscLayer } from "../view/sky/disc";
import { emptyDrawList, viewStrokesAt } from "../view/wireframe/drawList";
import { base64Of, type CapturedImage } from "./atmosphere";
import { addCanvas } from "./frames";
import { type Checks, halfTexels } from "./harness";

/** The captures' size, px: the view's at 1080p. */
const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };

/** The field across the frame, deg. */
const FOV_DEG = 60;

/** No wireframe draw list: the stars are not under test. */
const NO_LIST = emptyDrawList(viewStrokesAt(1));

/** The controller's step, s: a quarter second, so that a settled exposure takes few frames. */
const STEP_S = 0.25;

/** The most frames a framing is drawn while its exposure settles. */
const MAX_FRAMES = 12;

/** Settled: two frames in a row whose applied EV100 differ by under this. */
const SETTLED_EV = 0.02;

/** The Sun's direction from the phase scene's camera: the star lies along −x. */
const TO_STAR = vec3(-1, 0, 0);

const RAD_PER_DEG = Math.PI / 180;

/** The direction in the scene's xz plane at `angleDeg` from the Sun's, towards the crescent. */
function fromStar(angleDeg: number): Vec3 {
  return phasePlanetDirection(180 - angleDeg);
}

/** One framing: its name, the camera looking along `forward` with `up`, and its meter. */
interface Framing {
  readonly name: string;
  readonly forward: Vec3;
  readonly up: Vec3;
  readonly meter: MeterMode;
}

/** The crescent 30° from the Sun; looking 29.5° from the Sun puts its disc just inside the left edge. */
const SUN_IN_FRAME = { forward: fromStar(29.5), up: vec3(0, 1, 0) };

/** The same, turned 1.1° further: the Sun's disc, 0.27° in radius, just outside the frame. */
const SUN_JUST_OUTSIDE = { forward: fromStar(30.6), up: vec3(0, 1, 0) };

/** The Sun at the centre, the frame turned so that the crescent, 30° off, lies above it. */
const SUN_ALONE = { forward: TO_STAR, up: vec3(0, 0, 1) };

const FRAMINGS: ReadonlyArray<Framing> = [
  { name: "a-lit-sun-in-frame", ...SUN_IN_FRAME, meter: "lit" },
  { name: "a-lit-sun-just-outside", ...SUN_JUST_OUTSIDE, meter: "lit" },
  { name: "b-avg-sun-in-frame", ...SUN_IN_FRAME, meter: "average" },
  { name: "c-avg-sun-alone", ...SUN_ALONE, meter: "average" },
];

/** The view's exposure as a new view opens, the controller's start: `AUTO` at `MAN`'s −1. */
const START: ExposureControl = { kind: "auto", ev100: -1 };

/** What one settled framing gave. */
interface Settled {
  readonly ev100: number;
  readonly frames: number;
  readonly srgb: Uint8Array;
  readonly hdr: Float32Array;
}

/** Waits for the renderer's next histogram, read back after its frame's copy has settled. */
async function nextHistogram(renderer: PhotorealRenderer): Promise<Histogram | undefined> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const histogram = renderer.takeHistogram();
    if (histogram !== undefined) {
      return histogram;
    }
    // The read settles on the device's timeline: each look waits for the one before.
    // oxlint-disable-next-line no-await-in-loop
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 10);
    });
  }
  return undefined;
}

/** Draws `framing` until the controller's exposure settles, and reads its last frame back. */
async function settle(
  engine: RenderEngine,
  renderer: PhotorealRenderer,
  view: RenderView,
  discs: HostDiscLayer,
  framing: Framing,
): Promise<Settled> {
  const start = startRun(phaseScene());
  const pose: CameraPose = {
    ...start.camera.pose,
    orientation: lookAlong(framing.forward, framing.up),
  };
  const run = {
    ...start,
    camera: { ...start.camera, pose, fovDeg: FOV_DEG, style: "photorealistic" as const },
  };
  const exposure = new AutoExposure({
    source: viewId("smoke"),
    program: VIEW_CAMERA,
    control: START,
    meter: framing.meter,
  });
  exposure.cut();
  exposure.noteImage(true);
  let histogram: Histogram | undefined;
  let last = Number.NaN;
  let frames = 0;
  let srgb: Uint8Array = new Uint8Array(0);
  for (; frames < MAX_FRAMES; frames += 1) {
    const ev100 = controlEv100(exposure.step(histogram, STEP_S).control);
    const frame = photorealFrame({
      run,
      pose,
      viewport: VIEWPORT,
      presentedPx: VIEWPORT,
      setting: "high",
      exposureScale: exposureScale(ev100),
      list: NO_LIST,
      sky: null,
      band: null,
      discs,
      cube: null,
      previousRegimes: new Map(),
      overlay: null,
      meter: framing.meter,
    });
    renderer.render(view, frame);
    // The canvas is read back in the task that drew it, each frame before the next.
    // oxlint-disable-next-line no-await-in-loop
    const bytes = await view.readBack();
    srgb = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
    // The next frame's exposure is metered from this frame's histogram, as the view's loop does.
    // oxlint-disable-next-line no-await-in-loop
    histogram = await nextHistogram(renderer);
    if (Math.abs(ev100 - last) < SETTLED_EV) {
      break;
    }
    last = ev100;
  }
  const colours = renderer.frameColours;
  if (colours === null) {
    throw new Error("the photorealistic renderer drew no frame");
  }
  return {
    ev100: last,
    frames: frames + 1,
    srgb,
    hdr: halfTexels(await engine.readTexture(colours.drawn)),
  };
}

/** The median of `values`, or `NaN` for none. */
function median(values: number[]): number {
  if (values.length === 0) {
    return Number.NaN;
  }
  values.sort((a, b) => a - b);
  return values[Math.floor(values.length / 2)] ?? Number.NaN;
}

/** The view-space direction through pixel (x, y)'s centre, unit. */
function pixelDirection(x: number, y: number): Vec3 {
  const t = Math.tan((FOV_DEG * RAD_PER_DEG) / 2);
  const v = vec3(
    (((x + 0.5) / VIEWPORT.widthPx) * 2 - 1) * t,
    (1 - ((y + 0.5) / VIEWPORT.heightPx) * 2) * (t * (VIEWPORT.heightPx / VIEWPORT.widthPx)),
    -1,
  );
  const n = Math.hypot(v.x, v.y, v.z);
  return vec3(v.x / n, v.y / n, v.z / n);
}

/** A settled frame's figures: its medians by class and its disc's least code. */
interface Figures {
  /** The frame's median, the green channel's code. */
  readonly frame: number;
  /** The median of the sky's pixels beyond 30° from the Sun. */
  readonly farSky: number;
  /** The median of the lit planet's pixels. */
  readonly lit: number;
  /** The least code over the disc's pixels, its brightest channel's; `NaN` with none in frame. */
  readonly disc: number;
}

/** The figures of `settled`, framed as `framing`. */
function figures(settled: Settled, framing: Framing): Figures {
  const toStar = rotateInto(framing, TO_STAR);
  const all: number[] = [];
  const farSky: number[] = [];
  const lit: number[] = [];
  let disc = Number.POSITIVE_INFINITY;
  for (let y = 0; y < VIEWPORT.heightPx; y += 1) {
    for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
      const i = y * VIEWPORT.widthPx + x;
      const g = settled.srgb[4 * i + 1] ?? 0;
      const brightest = Math.max(settled.srgb[4 * i] ?? 0, g, settled.srgb[4 * i + 2] ?? 0);
      const meterClass = Math.round(settled.hdr[4 * i + 3] ?? 1);
      all.push(g);
      if (meterClass === METER_CLASS.hostDisc) {
        disc = Math.min(disc, brightest);
      } else if (meterClass === METER_CLASS.litBody) {
        lit.push(g);
      } else if (meterClass === METER_CLASS.other) {
        const d = pixelDirection(x, y);
        const cos = d.x * toStar.x + d.y * toStar.y + d.z * toStar.z;
        if (cos < Math.cos(30 * RAD_PER_DEG)) {
          farSky.push(g);
        }
      }
    }
  }
  return {
    frame: median(all),
    farSky: median(farSky),
    lit: median(lit),
    disc: Number.isFinite(disc) ? disc : Number.NaN,
  };
}

/** A scene direction in `framing`'s view axes. */
function rotateInto(framing: Framing, v: Vec3): Vec3 {
  return rotate(conjugate(lookAlong(framing.forward, framing.up)), v);
}

/**
 * The four framings, settled, captured and judged: the captures are named
 * `fix-view-light-<framing>`.
 */
export async function captureViewLight(
  engine: RenderEngine,
  checks: Checks,
): Promise<CapturedImage[]> {
  const view = engine.createView(addCanvas(), "smoke view light");
  view.resize(VIEWPORT);
  const renderer = new PhotorealRenderer(engine, "smoke view light");
  const discs = new HostDiscLayer(engine);
  const images: CapturedImage[] = [];
  const results = new Map<string, { readonly settled: Settled; readonly figures: Figures }>();
  try {
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: lookAlong(SUN_IN_FRAME.forward, SUN_IN_FRAME.up),
      fovXRad: FOV_DEG * RAD_PER_DEG,
    });
    for (const framing of FRAMINGS) {
      // Each framing settles before the next, as the views' frames do.
      // oxlint-disable-next-line no-await-in-loop
      const settled = await settle(engine, renderer, view, discs, framing);
      results.set(framing.name, { settled, figures: figures(settled, framing) });
      images.push({
        name: `fix-view-light-${framing.name}`,
        width: VIEWPORT.widthPx,
        height: VIEWPORT.heightPx,
        rgba: base64Of(settled.srgb),
      });
    }
  } finally {
    renderer.dispose();
    view.dispose();
  }
  const said = (name: string): string => {
    const result = results.get(name);
    if (result === undefined) {
      return `${name}: not drawn`;
    }
    const { settled, figures: f } = result;
    return `${name}: EV100 ${settled.ev100.toFixed(2)} after ${String(settled.frames)} frames; median frame ${String(f.frame)}, sky beyond 30° ${String(f.farSky)}, lit ${String(f.lit)}, disc least ${String(f.disc)}`;
  };
  const a = results.get("a-lit-sun-in-frame");
  const outside = results.get("a-lit-sun-just-outside");
  const b = results.get("b-avg-sun-in-frame");
  const c = results.get("c-avg-sun-alone");
  checks.check(
    "fix-view-light (a) LIT with the Sun in frame: the disc saturated, the sky beyond 30° below code 32",
    a !== undefined && a.figures.disc >= 250 && a.figures.farSky < 32,
    said("a-lit-sun-in-frame"),
  );
  checks.check(
    "fix-view-light (a) LIT's exposure within 0.3 EV of the same framing with the Sun just outside",
    a !== undefined &&
      outside !== undefined &&
      Math.abs(a.settled.ev100 - outside.settled.ev100) <= 0.3,
    said("a-lit-sun-just-outside"),
  );
  checks.check(
    "fix-view-light (b) AVG with the Sun in frame: the frame's median below code 128, the disc saturated",
    b !== undefined && b.figures.frame < 128 && b.figures.disc >= 250,
    said("b-avg-sun-in-frame"),
  );
  checks.check(
    "fix-view-light (c) the Sun alone in a dark field under AVG: the frame's median below code 64",
    c !== undefined && c.figures.frame < 64,
    said("c-avg-sun-alone"),
  );
  return images;
}
