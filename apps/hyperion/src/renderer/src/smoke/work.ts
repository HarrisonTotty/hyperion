/**
 * The smoke page's checks of offscreen chains, asynchronous pipelines, indirect work and pass
 * timing (T9.h), of the engine's rebuild after a forced loss (T9.f), and of a kernel's array
 * bindings of one layer (R08.T0).
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { GraphicsStatusStore } from "../view/engine/status";
import type { ComputeHandle, PassTimes, RenderEngine } from "../view/engine/types";
import FRAME_WGSL from "../view/shaders/frame.wgsl?raw";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfBits,
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

  // An asynchronous material draws on the first frame after it resolves, into the outputs it was
  // prepared for.
  const asyncMaterial = await engine.createMaterialAsync(
    flatSpec("async flat"),
    ["rgba16float"],
    [full],
  );
  first.render(frameOf("chain async", [drawOf(full, asyncMaterial, [0, 0, 1, 1])]));
  const drawn = texel(halfTexels(await engine.readTexture(first.colour)), 8, 3, 3);
  checks.check(
    "T9.h an asynchronous material draws on the first frame after it resolves",
    near(drawn, [0, 0, 1, 1], 0),
    show(drawn),
  );

  // Into an output it was not prepared for, its pipeline is made asynchronously: the frame does
  // not wait for it and leaves its draw out (the red under it shows), and a later frame draws it.
  const unprepared = engine.createRenderTarget({
    name: "chain unprepared",
    size: { widthPx: 8, heightPx: 8 },
    format: "rgba8unorm",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  const pendingFrame = (): ReturnType<typeof frameOf> =>
    frameOf("chain pending", [
      drawOf(full, flat, [1, 0, 0, 1]),
      drawOf(full, asyncMaterial, [0, 0, 1, 1]),
    ]);
  unprepared.render(pendingFrame());
  const leftOut = texel(new Uint8Array(await engine.readTexture(unprepared.colour)), 8, 3, 3);
  checks.check(
    "T9.h a frame whose material's pipeline is still being made leaves that draw out",
    near(leftOut, [255, 0, 0, 255], 0),
    show(leftOut),
  );
  let later = leftOut;
  for (let attempt = 0; attempt < 50 && !near(later, [0, 0, 255, 255], 0); attempt += 1) {
    // Each frame waits for the pipeline the one before it asked for.
    // oxlint-disable-next-line no-await-in-loop
    await pause(20);
    unprepared.render(pendingFrame());
    // Each attempt reads the frame it just rendered before the next.
    // oxlint-disable-next-line no-await-in-loop
    later = texel(new Uint8Array(await engine.readTexture(unprepared.colour)), 8, 3, 3);
  }
  checks.check(
    "T9.h the left-out draw is drawn once its pipeline is ready",
    near(later, [0, 0, 255, 255], 0),
    show(later),
  );
  unprepared.dispose();

  // A target's mips are generated after it renders: level 1 of a flat colour is that colour.
  const mipped = engine.createRenderTarget({
    name: "chain mipped",
    size: { widthPx: 8, heightPx: 8 },
    format: "rgba16float",
    mips: 3,
    depth: false,
    category: "render-targets",
  });
  mipped.render(frameOf("chain mipped", [drawOf(full, flat, [0.5, 0.25, 1, 1])]));
  const level1 = texel(halfTexels(await engine.readTexture(mipped.colour, 1)), 4, 1, 1);
  checks.check(
    "T9.h a 3-mip target's level 1 is generated from level 0",
    near(level1, [0.5, 0.25, 1, 1], 1e-3),
    show(level1),
  );
  mipped.dispose();

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
    flatSpec("indirect add", { blend: "additive", depthWrite: false }),
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
    displayName: "TEST INDIRECT COPY",
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

/** The side, in texels, of each layer {@link checkOneLayerArrays} copies: one workgroup's. */
const LAYER_COPY_TEXELS = 4;

/**
 * R08.T0: a kernel declaring a `texture_2d_array` input and a `texture_storage_2d_array` output
 * copies a one-layer and a three-layer texture layer by layer, and the copy reads back bit for bit.
 *
 * @remarks
 * A one-layer texture binds there only as a one-layer `2d-array` view: at its own `2d`, WebGPU
 * refuses the bind group, and the dispatch's submission with it. Each channel is a quarter of its
 * index, exact in half precision (at most 191/4, well within a half's 11 significant bits).
 */
export async function checkOneLayerArrays(engine: RenderEngine, checks: Checks): Promise<void> {
  const copy = await engine.createComputeAsync({
    name: "smoke layer copy",
    readback: "bit-exact",
    subgroup: null,
    reference: `
@group(0) @binding(0) var source : texture_2d_array<f32>;
@group(0) @binding(1) var copy : texture_storage_2d_array<rgba16float, write>;
@compute @workgroup_size(${LAYER_COPY_TEXELS}, ${LAYER_COPY_TEXELS}, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  textureStore(copy, id.xy, id.z, textureLoad(source, id.xy, id.z, 0));
}`,
  });
  for (const layers of [1, 3]) {
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const { written, read } = await copiedLayers(engine, copy, layers);
    const differing =
      read.length === written.length
        ? written.filter((bits, index) => read[index] !== bits).length
        : written.length;
    checks.check(
      `R08.T0 a kernel declaring arrays copies a ${layers}-layer texture, read back exactly`,
      read.length === written.length && differing === 0,
      `${read.length / 4} of ${written.length / 4} texels read; ${differing} channels differ`,
    );
  }
}

/**
 * The half-float bits {@link checkOneLayerArrays} writes into a texture of `layers` layers, and
 * those `copy` writes from it into another, read back; both textures are released, whatever
 * happens.
 */
async function copiedLayers(
  engine: RenderEngine,
  copy: ComputeHandle,
  layers: number,
): Promise<{ readonly written: Uint16Array; readonly read: Uint16Array }> {
  const size = { width: LAYER_COPY_TEXELS, height: LAYER_COPY_TEXELS, depthOrArrayLayers: layers };
  const source = engine.createTexture({
    name: `layer copy source ${layers}`,
    size,
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  const out = engine.createTexture({
    name: `layer copy ${layers}`,
    size,
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: "other",
  });
  try {
    const written = Uint16Array.from(
      { length: LAYER_COPY_TEXELS * LAYER_COPY_TEXELS * layers * 4 },
      (_, index) => halfBits(index / 4),
    );
    engine.writeTexture(source, [0, 0, 0], size, written);
    engine.dispatch(
      copy,
      {
        uniforms: {},
        buffers: {},
        sampled: { source },
        storage: { copy: { texture: out, level: 0 } },
      },
      [1, 1, layers],
      `layer copy ${layers}`,
    );
    return { written, read: new Uint16Array(await engine.readTexture(out)) };
  } finally {
    engine.releaseTexture(source);
    engine.releaseTexture(out);
  }
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
