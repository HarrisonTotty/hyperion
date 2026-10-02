/**
 * The by-hand scene of R01.T11 and T12: three canvases on one device, drawn every animation frame
 * for a set time while the main process resizes the window, with a post-process on each view, a
 * looping video and a panel of DOM text beside them (the consoles' stand-in), and a record of
 * frame intervals, per-pass GPU time, device losses and gaps in the frame heartbeat.
 *
 * @remarks
 * Not part of `just test-render`: it needs a display and a real GPU, and its figures are the
 * as-built notes'. The page reports its record as checks whose details carry the figures.
 */

import { loadRenderEngine } from "../view/engine/loadEngine";
import { requestAdapterOutcome } from "../view/engine/platform";
import type { GraphicsStatusStore } from "../view/engine/status";
import type { PassTimes, RenderEngine, RenderView } from "../view/engine/types";
import { type Checks, drawOf, flatSpec, frameOf, fullScreenMesh, triangleMesh } from "./harness";

/** The percentile `p` (0 to 1) of `values`, by nearest rank. */
export function percentile(values: ReadonlyArray<number>, p: number): number {
  if (values.length === 0) {
    return Number.NaN;
  }
  const sorted = values.toSorted((a, b) => a - b);
  return (
    sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))] ?? Number.NaN
  );
}

/** One view of the scene, with the canvas it draws into. */
interface SoakView {
  readonly name: string;
  readonly canvas: HTMLCanvasElement;
  readonly view: RenderView;
}

/** A canvas placed in the page by CSS. */
function placedCanvas(style: string): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.setAttribute("style", style);
  document.body.append(canvas);
  return canvas;
}

/** A looping video of moving bars, from the `file:` URL the main process hands over, if any. */
function addVideo(source: string | null): void {
  if (source === null) {
    return;
  }
  const video = document.createElement("video");
  video.src = source;
  video.loop = true;
  video.muted = true;
  // Stacked above the full-window cockpit canvas, appended later, so that the compositor draws it
  // (T12's soak needs the video and the DOM panel on screen, not occluded).
  video.setAttribute(
    "style",
    "position:fixed;z-index:1;left:16px;bottom:16px;width:320px;height:180px;",
  );
  document.body.append(video);
  void video.play().catch((error: unknown) => {
    console.error("the soak's video did not play:", error);
  });
}

/** The panel of DOM text the main process compares between captures. */
function addPanel(): HTMLElement {
  const panel = document.createElement("output");
  panel.id = "soak-panel";
  panel.setAttribute(
    "style",
    "position:fixed;z-index:1;right:16px;bottom:16px;width:320px;height:120px;background:#000;color:#0f0;font:16px monospace;",
  );
  document.body.append(panel);
  return panel;
}

/** Runs the scene for `seconds` and records its figures as checks. */
export async function runSoak(
  status: GraphicsStatusStore,
  checks: Checks,
  seconds: number,
  video: string | null,
): Promise<void> {
  document.body.setAttribute("style", "margin:0;background:#000;overflow:hidden;");
  const outcome = await requestAdapterOutcome(navigator.gpu);
  if (outcome.kind !== "adapter") {
    throw new Error(`no adapter: ${outcome.kind}`);
  }
  const engine: RenderEngine = await loadRenderEngine(outcome, status);
  const times = new Map<string, number[]>();
  const labelsSeen = new Set<string>();
  const stopTimes = engine.onPassTimes((frame: PassTimes) => {
    for (const pass of frame.passes) {
      labelsSeen.add(pass.label);
      const list = times.get(pass.label) ?? [];
      list.push(pass.ns);
      times.set(pass.label, list);
    }
  });
  let losses = 0;
  const stopFaults = engine.onFault((fault) => {
    if (fault.kind === "device-lost") {
      losses += 1;
    }
  });
  addVideo(video);
  const panel = addPanel();
  const views: SoakView[] = [
    ["cockpit", "position:fixed;inset:0;width:100vw;height:100vh;"],
    ["instrument 1", "position:fixed;right:16px;top:80px;"],
    ["instrument 2", "position:fixed;right:560px;top:80px;"],
  ].map(([name = "", style = ""]) => {
    const canvas = placedCanvas(style);
    return { name, canvas, view: engine.createView(canvas, name) };
  });
  const soakLabel = document.createElement("output");
  soakLabel.id = "soak-label";
  soakLabel.textContent = "HYPERION SOAK";
  soakLabel.setAttribute(
    "style",
    "position:fixed;right:16px;top:16px;width:320px;height:48px;background:#fff;color:#000;font:24px monospace;",
  );
  document.body.append(soakLabel);
  let material = engine.createMaterial(flatSpec("soak marker"));
  let background = engine.createMaterial(flatSpec("soak background"));
  let full = fullScreenMesh(engine, "soak full", -3);
  const markers = new Map<number, ReturnType<RenderEngine["createMesh"]>>();
  let tone = makeTone(engine);
  const stopRestores = engine.onRestored(() => {
    try {
      material = engine.createMaterial(flatSpec("soak marker"));
      background = engine.createMaterial(flatSpec("soak background"));
      full = fullScreenMesh(engine, "soak full", -3);
      markers.clear();
      tone = makeTone(engine);
    } catch (error: unknown) {
      // Lost again before the handles were remade: the next restore remakes them.
      console.warn("the soak's handles were not remade after a restore:", error);
    }
  });
  const markerFor = (aspect: number): ReturnType<RenderEngine["createMesh"]> => {
    const key = Math.round(aspect * 1000);
    let mesh = markers.get(key);
    if (mesh === undefined) {
      mesh = triangleMesh(engine, `soak marker ${key}`, [
        -0.8 * aspect,
        0.2,
        -0.2 * aspect,
        0.2,
        -0.8 * aspect,
        0.8,
      ]);
      markers.set(key, mesh);
    }
    return mesh;
  };

  const intervals: number[] = [];
  let gaps = 0;
  let last = performance.now();
  const start = last;
  let frames = 0;
  await new Promise<void>((resolve) => {
    const tick = (now: number): void => {
      const interval = now - last;
      last = now;
      if (frames > 0) {
        intervals.push(interval);
        if (interval > 1000) {
          gaps += 1;
        }
      }
      frames += 1;
      const ratio = window.devicePixelRatio;
      const instrument =
        Math.floor((now - start) / 10_000) % 2 === 0
          ? { widthPx: 320, heightPx: 240 }
          : { widthPx: 400, heightPx: 300 };
      for (const { name, canvas, view } of views) {
        const size =
          name === "cockpit"
            ? {
                widthPx: Math.max(1, Math.round(window.innerWidth * ratio)),
                heightPx: Math.max(1, Math.round(window.innerHeight * ratio)),
              }
            : instrument;
        if (name !== "cockpit") {
          canvas.style.width = `${size.widthPx / ratio}px`;
          canvas.style.height = `${size.heightPx / ratio}px`;
        }
        view.resize(size);
        const aspect = size.widthPx / size.heightPx;
        const shade = 0.5 + 0.5 * Math.sin(now / 1000);
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
      }
      panel.textContent = `frame ${frames}\nlosses ${losses}\ngaps ${gaps}`;
      if (now - start < seconds * 1000) {
        requestAnimationFrame(tick);
      } else {
        resolve();
      }
    };
    requestAnimationFrame(tick);
  });

  stopTimes();
  stopFaults();
  stopRestores();
  for (const { view } of views) {
    view.dispose();
  }
  const snapshotAtEnd = status.getSnapshot();
  engine.dispose();
  const median = (values: ReadonlyArray<number>): number => percentile(values, 0.5);
  checks.check(
    "T11 frame interval",
    intervals.length > 0,
    `${frames} frames over ${seconds} s; p50 ${percentile(intervals, 0.5).toFixed(2)} ms, p95 ${percentile(intervals, 0.95).toFixed(2)} ms, max ${intervals.reduce((a, b) => Math.max(a, b), 0).toFixed(1)} ms; gaps over 1 s ${gaps}`,
  );
  checks.check(
    "T11 GPU time per pass (median, µs)",
    times.size > 0,
    [...times]
      .map(
        ([label, values]) =>
          `${label} ${(median(values) / 1000).toFixed(1)} (p95 ${(percentile(values, 0.95) / 1000).toFixed(1)}, n ${values.length})`,
      )
      .join("; "),
  );
  checks.check(
    "T11 the passes are the views' own and their post-processes, no copy",
    [...labelsSeen].every((label) => /^(cockpit|instrument [12])( tone)?$/u.test(label)),
    [...labelsSeen].join(", "),
  );
  const snapshot = snapshotAtEnd;
  checks.check(
    "T11 status",
    true,
    `timer ${snapshot.timer}, targetRounding ${JSON.stringify(snapshot.targetRounding)}, device losses ${snapshot.deviceLosses}, fault ${JSON.stringify(snapshot.fault)}, engine-seen losses ${losses}`,
  );
}

/** The post-process each view runs: an exposure scale, standing in for R07's tone mapping. */
function makeTone(engine: RenderEngine): ReturnType<RenderEngine["createPostProcess"]> {
  return engine.createPostProcess({
    name: "tone",
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
