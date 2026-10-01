/**
 * The smoke page's checks of offscreen chains, asynchronous pipelines, indirect work and pass
 * timing (T9.h), and of the engine's rebuild after a forced loss (T9.f).
 */

import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { GraphicsStatusStore } from "../view/engine/status";
import type { PassTimes, RenderEngine } from "../view/engine/types";
import FRAME_WGSL from "../view/shaders/frame.wgsl?raw";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  near,
  pause,
  show,
  texel,
} from "./harness";

/** The labels of `wanted` not in `seen`, or `none`. */
function missingText(wanted: ReadonlyArray<string>, seen: ReadonlySet<string>): string {
  const missing = wanted.filter((label) => !seen.has(label));
  return missing.length > 0 ? missing.join(", ") : "none";
}

/** T9.h: a two-target chain, an asynchronous material, GPU-written counts and per-pass time. */
export async function checkTargetsAsyncIndirectTiming(
  engine: RenderEngine,
  checks: Checks,
): Promise<void> {
  const times: PassTimes[] = [];
  const stopTimes = engine.onPassTimes((frameTimes) => {
    times.push(frameTimes);
  });
  const full = fullScreenMesh(engine, "chain full");
  const flat = engine.createMaterial(flatSpec("chain flat"));
  const first = engine.createRenderTarget({
    name: "chain first",
    size: { widthPx: 8, heightPx: 8 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const second = engine.createRenderTarget({
    name: "chain second",
    size: { widthPx: 8, heightPx: 8 },
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  const halve = engine.createMaterial({
    ...flatSpec("chain halve", { depthWrite: false }),
    textures: [{ name: "previous", binding: 0 }],
    fragmentWgsl: `${FRAME_WGSL}
@group(2) @binding(0) var previous : texture_2d<f32>;
@fragment fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  return textureLoad(previous, vec2i(position.xy), 0) * 0.5;
}`,
  });
  first.render(frameOf("chain first", [drawOf(full, flat, [2, 0.25, 0, 1])]));
  second.render(
    frameOf("chain second", [
      drawOf(full, halve, [0, 0, 0, 0], { textures: { previous: first.colour } }),
    ]),
  );
  const chained = texel(halfTexels(await engine.readTexture(second.colour)), 8, 3, 3);
  checks.check(
    "T9.h a second target samples the first",
    near(chained, [1, 0.125, 0, 0.5], 1e-3),
    show(chained),
  );

  // An asynchronous material: a frame submitted meanwhile does not wait, and it draws on the first
  // frame after it resolves.
  let resolved = false;
  const pending = engine
    .createMaterialAsync(flatSpec("async flat"), ["rgba16float"], [full])
    .then((handle) => {
      resolved = true;
      return handle;
    });
  first.render(frameOf("chain while compiling", [drawOf(full, flat, [0, 1, 0, 1])]));
  checks.check(
    "T9.h a frame submitted while a material compiles does not wait for it",
    !resolved,
    `resolved before the frame returned: ${String(resolved)}`,
  );
  const asyncMaterial = await pending;
  first.render(frameOf("chain async", [drawOf(full, asyncMaterial, [0, 0, 1, 1])]));
  const drawn = texel(halfTexels(await engine.readTexture(first.colour)), 8, 3, 3);
  checks.check(
    "T9.h an asynchronous material draws on the first frame after it resolves",
    near(drawn, [0, 0, 1, 1], 0),
    show(drawn),
  );

  // A kernel writes a draw's and a dispatch's counts; each instance adds 0.25 of red.
  const args = engine.createBuffer({
    name: "indirect args",
    bytes: 32,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.INDIRECT | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  const writeArgs = await engine.createComputeAsync({
    name: "smoke write args",
    readback: "bit-exact",
    subgroup: null,
    reference: `
@group(0) @binding(0) var<storage, read_write> args : array<u32, 8>;
@compute @workgroup_size(1) fn main() {
  args[0] = 3u; args[1] = 4u; args[2] = 0u; args[3] = 0u;
  args[4] = 2u; args[5] = 1u; args[6] = 1u; args[7] = 0u;
}`,
  });
  engine.dispatch(
    writeArgs,
    { uniforms: {}, buffers: { args }, sampled: {}, storage: {} },
    [1, 1, 1],
    "write args",
  );
  const counter = engine.createBuffer({
    name: "indirect counter",
    bytes: 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  const count = await engine.createComputeAsync({
    name: "smoke count",
    readback: "bit-exact",
    subgroup: null,
    reference: `
@group(0) @binding(0) var<storage, read_write> counter : array<atomic<u32>, 1>;
@compute @workgroup_size(1) fn main() { atomicAdd(&counter[0], 1u); }`,
  });
  engine.dispatch(
    count,
    { uniforms: {}, buffers: { counter }, sampled: {}, storage: {} },
    { buffer: args, offsetBytes: 16 },
    "indirect dispatch",
  );
  const counted = new Uint32Array(await engine.readBuffer(counter))[0];
  checks.check(
    "T9.h an indirect dispatch runs the workgroups a kernel wrote",
    counted === 2,
    `counter ${counted}`,
  );
  const additive = engine.createMaterial(
    flatSpec("indirect add", { blend: "additive", depthWrite: false, transparent: true }),
  );
  first.render(
    frameOf("indirect draw", [
      drawOf(full, additive, [0.25, 0, 0, 1], { indirect: { buffer: args, offsetBytes: 0 } }),
    ]),
  );
  const instances = texel(halfTexels(await engine.readTexture(first.colour)), 8, 3, 3);
  checks.check(
    "T9.h an indirect draw draws the instance count a kernel wrote (4 × 0.25)",
    near(instances, [1, 0, 0, 1], 1e-3),
    show(instances),
  );

  // A frame with both an indirect draw and a post-process (decision item 12).
  const copy = engine.createPostProcess({
    name: "indirect copy",
    uniforms: [],
    fragmentWgsl: `
@group(2) @binding(0) var colour : texture_2d<f32>;
@fragment fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  return textureLoad(colour, vec2i(position.xy), 0);
}`,
  });
  first.render(
    frameOf(
      "indirect and post",
      [drawOf(full, additive, [0.25, 0, 0, 1], { indirect: { buffer: args, offsetBytes: 0 } })],
      1,
      [{ postProcess: copy, uniforms: {} }],
    ),
  );
  const both = texel(halfTexels(await engine.readTexture(first.colour)), 8, 3, 3);
  checks.check(
    "T9.h a frame holds indirect draws and post-processes together",
    near(both, [1, 0, 0, 1], 1e-3),
    show(both),
  );

  if (engine.capabilities.timestampQuery) {
    for (
      let attempt = 0;
      attempt < 50 &&
      !times.some((frame) =>
        frame.passes.some((pass) => pass.label === "indirect and post indirect copy"),
      );
      attempt += 1
    ) {
      // The harness's checks run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      await pause(20);
    }
    const passes = times.flatMap((frame) => frame.passes);
    const labels = new Set(passes.map((pass) => pass.label));
    const wanted: ReadonlyArray<string> = [
      "chain first",
      "chain second",
      "write args",
      "indirect dispatch",
      "indirect draw",
      "indirect and post",
      "indirect and post indirect copy",
    ];
    checks.check(
      "T9.h onPassTimes reports one entry per labelled pass, none bracketed, finite and non-negative",
      wanted.every((label) => labels.has(label)) &&
        passes.every((pass) => !pass.bracketed && Number.isFinite(pass.ns) && pass.ns >= 0),
      `${passes.length} passes; timer ${times[0]?.timer ?? "none"}; ${passes.filter((pass) => pass.ns % 65_536 !== 0).length} not multiples of 65,536 ns; missing ${missingText(wanted, labels)}`,
    );
  } else {
    checks.check(
      "T9.h no pass times without timestamp-query",
      times.length === 0,
      `${times.length} frames`,
    );
  }
  stopTimes();
  first.dispose();
  second.dispose();
}

/**
 * T9.f's integration check of T8.e: the engine's device destroyed under it is a loss; the engine
 * rebuilds on a fresh adapter and its view and a new target render again at their sizes.
 */
export async function checkForcedLoss(
  engine: RenderEngine,
  status: GraphicsStatusStore,
  device: GPUDevice,
  checks: Checks,
  canvas: HTMLCanvasElement,
): Promise<void> {
  const view = engine.createView(canvas, "loss view");
  view.resize({ widthPx: 48, heightPx: 24 });
  let restored = 0;
  const stop = engine.onRestored(() => {
    restored += 1;
  });
  device.destroy();
  // The flag is set by the engine's listener while the loop waits.
  // oxlint-disable-next-line no-unmodified-loop-condition
  for (let attempt = 0; attempt < 250 && restored === 0; attempt += 1) {
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    await pause(20);
  }
  stop();
  const snapshot = status.getSnapshot();
  checks.check(
    "T9.f a destroyed device is a loss, and the engine rebuilds",
    restored === 1 && snapshot.deviceLosses === 1 && snapshot.fault === null,
    `restored ${restored}, losses ${snapshot.deviceLosses}, fault ${JSON.stringify(snapshot.fault)}`,
  );
  const material = engine.createMaterial(flatSpec("after loss"));
  const full = fullScreenMesh(engine, "after loss full");
  view.render(frameOf("after loss view", [drawOf(full, material, [0, 1, 0, 1])], 2));
  const bytes = await view.readBack();
  checks.check(
    "T9.f the view renders again at its size after the rebuild",
    bytes.length === 48 * 24 * 4 && near(Array.from(texel(bytes, 48, 24, 12)), [0, 255, 0, 255], 0),
    `${bytes.length / 4} pixels, centre ${show(texel(bytes, 48, 24, 12))}`,
  );
  const target = engine.createRenderTarget({
    name: "after loss target",
    size: { widthPx: 4, heightPx: 4 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  target.render(frameOf("after loss target", [drawOf(full, material, [0, 0.5, 0, 1])]));
  const seen = texel(halfTexels(await engine.readTexture(target.colour)), 4, 2, 2);
  checks.check(
    "T9.f a target made after the rebuild renders",
    near(seen, [0, 0.5, 0, 1], 1e-3),
    show(seen),
  );
  target.dispose();
  view.dispose();
}
