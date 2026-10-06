/**
 * R07.T21's scene: an instrument view in a same-origin child window, beside the main view in the
 * opener, both drawn by the opener's one engine from the opener's animation frames (R01.T13's
 * prototype, rebuilt; `src/smoke/childWindow.ts` places the child on the second display).
 *
 * @remarks
 * The opener draws its full-window `cockpit` canvas and the child's `child instrument` canvas
 * every animation frame, each with a marker in its top-left quadrant and a one-pass post-process,
 * as R01.T11's soak does. The child's own `requestAnimationFrame` is timed apart from the
 * opener's: on a second display Chromium should pace it from that display's vsync. Halfway the
 * child is resized; at the end it is closed, and R01.T13's rule is kept: its view is dropped on
 * its `pagehide` (the record says how many of the opener's frames came first; the main process
 * counts any uncaptured GPU error against the run), and the opener draws on for
 * {@link CLOSE_TAIL_MS}. Not part of `just test-render`: it needs a display and a
 * real GPU, and its figures are R07's record.
 */

import { loadRenderEngine } from "../view/engine/loadEngine";
import { requestAdapterOutcome } from "../view/engine/platform";
import type { GraphicsStatusStore } from "../view/engine/status";
import type { PassTimes, RenderEngine, RenderView, ViewSize } from "../view/engine/types";
import { type Checks, drawOf, flatSpec, frameOf, fullScreenMesh, triangleMesh } from "./harness";
import { percentile } from "./soak";

/** How long the opener draws on after the child closes, ms. */
export const CLOSE_TAIL_MS = 2_000;

/** The child's size after its halfway resize, CSS px. */
export const CHILD_RESIZED = { width: 800, height: 600 } as const;

/** How far a median interval may stand from a display's period and still be paced by it. */
export const PACING_TOLERANCE = 0.03;

/**
 * What the main process tells the page: the displays' rates, whether the child is offscreen, and
 * the target name its window-open handler allows.
 */
export interface ChildWindowRates {
  /** The child's `window.open` target name (`src/smoke/childWindow.ts`'s `CHILD_FRAME_NAME`). */
  readonly frameName: string;
  /** The opener's display's refresh rate, Hz; 0 when unknown. */
  readonly mainHz: number;
  /** The child's display's refresh rate, Hz; 0 when unknown. */
  readonly childHz: number;
  /** An offscreen child on the opener's display: the harness's proof, paced offscreen. */
  readonly hidden: boolean;
}

/**
 * Whether the child's frames are paced by its own display: its median interval within
 * {@link PACING_TOLERANCE} of that display's period, and the reading of it.
 *
 * @remarks
 * Two displays at one rate cannot tell the child's pacing from the opener's, which the detail
 * says; a hidden child is paced by offscreen rendering, not by a display.
 */
export function childPacing(
  childIntervalsMs: ReadonlyArray<number>,
  openerIntervalsMs: ReadonlyArray<number>,
  rates: ChildWindowRates,
): { readonly pass: boolean; readonly detail: string } {
  const child = percentile(childIntervalsMs, 0.5);
  const opener = percentile(openerIntervalsMs, 0.5);
  const childPeriodMs = rates.childHz > 0 ? 1000 / rates.childHz : Number.NaN;
  const mainPeriodMs = rates.mainHz > 0 ? 1000 / rates.mainHz : Number.NaN;
  const paced = Math.abs(child - childPeriodMs) <= PACING_TOLERANCE * childPeriodMs;
  const figures = `child p50 ${child.toFixed(2)} ms (p95 ${percentile(childIntervalsMs, 0.95).toFixed(2)}, n ${String(childIntervalsMs.length)}) against its display's ${childPeriodMs.toFixed(2)} ms; opener p50 ${opener.toFixed(2)} ms against its display's ${mainPeriodMs.toFixed(2)} ms`;
  if (rates.hidden) {
    return {
      pass: childIntervalsMs.length > 0,
      detail: `${figures}; hidden: offscreen pacing, not a display's`,
    };
  }
  // Apart only where the band about the child's period leaves the opener's period out.
  const apart = Math.abs(childPeriodMs - mainPeriodMs) > PACING_TOLERANCE * childPeriodMs;
  return {
    pass: childIntervalsMs.length > 0 && paced,
    detail: `${figures}; ${apart ? "the displays' rates differ, so the child's own display paces it" : "both displays run at one rate, so this cannot tell the child's display from the opener's"}`,
  };
}

/**
 * Whether the child's view was dropped on its `pagehide`, and how many of the opener's frames
 * came between the close and the `pagehide`, from the opener's frame count at each (`null` where
 * no `pagehide` came).
 *
 * @remarks
 * R01.T13's rule drops the view on `pagehide` and expected that to come before the opener's next
 * frame. A frame between still draws the closing child; whether any of it reached a closed
 * context is the uncaptured-error count the main process judges, so the frames between are a
 * finding beside the release, not its failure.
 */
export function pagehideRelease(
  atClose: number,
  atPagehide: number | null,
): { readonly pass: boolean; readonly detail: string } {
  if (atPagehide === null) {
    return { pass: false, detail: "no pagehide reached the opener" };
  }
  const after = atPagehide - atClose;
  return after === 0
    ? { pass: true, detail: "before the opener's next frame after the close" }
    : {
        pass: true,
        detail: `${String(after)} of the opener's frames after the close, each still drawing the closing child (see the uncaptured GPU errors)`,
      };
}

/**
 * Whether the child drew at a new size after its resize: some size drawn after the first
 * `before` of `sizes` differs from the last one drawn before it.
 */
export function resizedFrom(sizes: ReadonlyArray<string>, before: number): boolean {
  const last = sizes[before - 1];
  return last !== undefined && sizes.slice(before).some((size) => size !== last);
}

/** The median and 95th percentile of a pass's times, µs, and its count. */
function passFigures(values: ReadonlyArray<number>): string {
  return `${(percentile(values, 0.5) / 1000).toFixed(1)} µs (p95 ${(percentile(values, 0.95) / 1000).toFixed(1)}, n ${String(values.length)})`;
}

/** A view's canvas size in device pixels, for a window's inner size. */
function windowSize(target: Window): ViewSize {
  const ratio = target.devicePixelRatio;
  return {
    widthPx: Math.max(1, Math.round(target.innerWidth * ratio)),
    heightPx: Math.max(1, Math.round(target.innerHeight * ratio)),
  };
}

/** A canvas filling `target`'s document. */
function fillingCanvas(target: Window): HTMLCanvasElement {
  const doc = target.document;
  doc.body.setAttribute("style", "margin:0;background:#000;overflow:hidden;");
  const canvas = doc.createElement("canvas");
  canvas.setAttribute("style", "position:fixed;inset:0;width:100vw;height:100vh;");
  doc.body.append(canvas);
  return canvas;
}

/** The post-process each view runs: an exposure scale standing in for R07's tone mapping. */
function makeTone(engine: RenderEngine): ReturnType<RenderEngine["createPostProcess"]> {
  return engine.createPostProcess({
    name: "tone",
    displayName: "T21 TONE",
    uniforms: [{ name: "exposure", type: "f32" }],
    fragmentWgsl: `
struct Draw { exposure : f32 }
@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var colour : texture_2d<f32>;
@group(2) @binding(1) var colourSampler : sampler;
@fragment fn fragmentMain(@location(0) uv : vec2f) -> @location(0) vec4f {
  let c = textureSample(colour, colourSampler, uv).rgb * draw.exposure;
  return vec4f(c / (1.0 + c), 1.0);
}`,
  });
}

/** Runs the scene for `seconds` and records its figures as checks. */
export async function runChildWindow(
  status: GraphicsStatusStore,
  checks: Checks,
  seconds: number,
  rates: ChildWindowRates,
): Promise<void> {
  document.body.setAttribute("style", "margin:0;background:#000;overflow:hidden;");
  const outcome = await requestAdapterOutcome(navigator.gpu);
  if (outcome.kind !== "adapter") {
    throw new Error(`no adapter: ${outcome.kind}`);
  }
  const engine: RenderEngine = await loadRenderEngine(outcome, status);
  const times = new Map<string, number[]>();
  const stopTimes = engine.onPassTimes((frame: PassTimes) => {
    for (const pass of frame.passes) {
      times.set(pass.label, [...(times.get(pass.label) ?? []), pass.ns]);
    }
  });
  let losses = 0;
  const stopFaults = engine.onFault((fault) => {
    if (fault.kind === "device-lost") {
      losses += 1;
    }
  });
  const material = engine.createMaterial(flatSpec("t21 marker"));
  const background = engine.createMaterial(flatSpec("t21 background"));
  const full = fullScreenMesh(engine, "t21 full", -3);
  const tone = makeTone(engine);
  const markers = new Map<number, ReturnType<RenderEngine["createMesh"]>>();
  const markerFor = (aspect: number): ReturnType<RenderEngine["createMesh"]> => {
    const key = Math.round(aspect * 1000);
    const known = markers.get(key);
    if (known !== undefined) {
      return known;
    }
    const mesh = triangleMesh(engine, `t21 marker ${String(key)}`, [
      -0.8 * aspect,
      0.2,
      -0.2 * aspect,
      0.2,
      -0.8 * aspect,
      0.8,
    ]);
    markers.set(key, mesh);
    return mesh;
  };
  const draw = (view: RenderView, name: string, size: ViewSize, nowMs: number): void => {
    view.resize(size);
    const aspect = size.widthPx / size.heightPx;
    const shade = 0.5 + 0.5 * Math.sin(nowMs / 1000);
    view.render(
      frameOf(
        name,
        [
          drawOf(full, background, [0.02, 0.02, 0.05 * shade, 1]),
          drawOf(markerFor(aspect), material, [0, 1, 0, 1]),
        ],
        aspect,
        [{ postProcess: tone, uniforms: { exposure: new Float32Array([1]) } }],
      ),
    );
  };

  const cockpit = engine.createView(fillingCanvas(window), "cockpit");
  const child = window.open("", rates.frameName, `popup,width=640,height=480`);
  checks.check(
    "T21 the child window opened",
    child !== null,
    child === null
      ? "window.open gave no window"
      : `a same-origin child, ${child.innerWidth} × ${child.innerHeight} CSS px`,
  );
  if (child === null) {
    cockpit.dispose();
    engine.dispose();
    return;
  }
  let childView: RenderView | null = engine.createView(fillingCanvas(child), "child instrument");
  // The opener's frames counted, so that the release is placed against the close.
  const frames: { drawn: number; atClose: number; atPagehide: number | null } = {
    drawn: 0,
    atClose: 0,
    atPagehide: null,
  };
  child.addEventListener("pagehide", () => {
    childView?.dispose();
    childView = null;
    frames.atPagehide = frames.drawn;
  });

  const childIntervals: number[] = [];
  let childLast: number | null = null;
  let childOpen = true;
  const childTick = (nowMs: number): void => {
    if (childLast !== null) {
      childIntervals.push(nowMs - childLast);
    }
    childLast = nowMs;
    if (childOpen) {
      child.requestAnimationFrame(childTick);
    }
  };
  child.requestAnimationFrame(childTick);

  const openerIntervals: number[] = [];
  const childSizes: string[] = [];
  let last: number | null = null;
  let resized = false;
  /** The child's sizes, as drawn, before its resize was asked for. */
  let sizesBeforeResize = 0;
  let closedAtMs: number | null = null;
  let framesAfterClose = 0;
  const start = performance.now();
  await new Promise<void>((resolve) => {
    const tick = (nowMs: number): void => {
      if (last !== null) {
        openerIntervals.push(nowMs - last);
      }
      last = nowMs;
      frames.drawn += 1;
      draw(cockpit, "cockpit", windowSize(window), nowMs);
      const view = childView;
      if (view !== null) {
        const size = windowSize(child);
        const text = `${String(size.widthPx)} × ${String(size.heightPx)}`;
        if (childSizes.at(-1) !== text) {
          childSizes.push(text);
        }
        draw(view, "child instrument", size, nowMs);
      }
      const elapsedMs = nowMs - start;
      if (!resized && elapsedMs >= (seconds * 1000) / 2) {
        resized = true;
        sizesBeforeResize = childSizes.length;
        child.resizeTo(CHILD_RESIZED.width, CHILD_RESIZED.height);
      }
      if (closedAtMs === null && elapsedMs >= seconds * 1000) {
        closedAtMs = nowMs;
        childOpen = false;
        frames.atClose = frames.drawn;
        child.close();
      } else if (closedAtMs !== null) {
        framesAfterClose += 1;
      }
      if (closedAtMs !== null && nowMs - closedAtMs >= CLOSE_TAIL_MS) {
        resolve();
        return;
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });

  stopTimes();
  stopFaults();
  const snapshot = status.getSnapshot();
  const release = pagehideRelease(frames.atClose, frames.atPagehide);
  checks.check("T21 the child's view was dropped on its pagehide", release.pass, release.detail);
  const pacing = childPacing(childIntervals, openerIntervals, rates);
  checks.check("T21 the child is paced by its own display", pacing.pass, pacing.detail);
  const cockpitTimes = times.get("cockpit") ?? [];
  const childTimes = times.get("child instrument") ?? [];
  checks.check(
    "T21 frame times beside the main view's",
    openerIntervals.length > 0 && cockpitTimes.length > 0 && childTimes.length > 0,
    `opener frame p50 ${percentile(openerIntervals, 0.5).toFixed(2)} ms, p95 ${percentile(openerIntervals, 0.95).toFixed(2)} ms; GPU: cockpit ${passFigures(cockpitTimes)}, child instrument ${passFigures(childTimes)}, their post-processes ${passFigures(times.get("cockpit tone") ?? [])} and ${passFigures(times.get("child instrument tone") ?? [])}; timer ${snapshot.timer}`,
  );
  checks.check(
    "T21 the child followed its resize",
    rates.hidden || resizedFrom(childSizes, sizesBeforeResize),
    `child canvas ${childSizes.join(" → ")} device px${rates.hidden ? "; hidden: an offscreen child has no size of its own (R01.T13), so its resize is the shown run's" : ""}`,
  );
  checks.check(
    "T21 the opener drew on after the child closed",
    framesAfterClose > 0 && losses === 0,
    `${String(framesAfterClose)} frames in ${String(CLOSE_TAIL_MS)} ms after the close; device losses ${String(losses)}, fault ${JSON.stringify(snapshot.fault)}`,
  );
  cockpit.dispose();
  engine.dispose();
}
