/**
 * The smoke page's blending, material-state, resource and post-process checks (T9.g, T9.i).
 */

import { ENGINE_CHECK_SPLAT } from "../view/engine/catalogue";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { RenderEngine } from "../view/engine/types";
import { DepthSelfSample, Float32BlendUnavailable } from "../view/engine/types";
import FRAME_WGSL from "../view/shaders/frame.wgsl?raw";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  near,
  show,
  texel,
} from "./harness";
import { addCanvas } from "./frames";

/** A small `rgba16float` target with depth. */
function smallTarget(engine: RenderEngine, name: string, sizePx: number, depth = true) {
  return engine.createRenderTarget({
    name,
    size: { widthPx: sizePx, heightPx: sizePx },
    format: "rgba16float",
    mips: 1,
    depth,
    category: "render-targets",
  });
}

/** T9.g: linear-light blending in a view, a compute-written 3D texture sampled, the packed cube. */
export async function checkBlendComputeCube(engine: RenderEngine, checks: Checks): Promise<void> {
  const full = fullScreenMesh(engine, "blend full");
  const additive = engine.createMaterial(
    flatSpec("half white", { blend: "additive", depthWrite: false }),
  );
  const view = engine.createView(addCanvas(), "blend view");
  view.resize({ widthPx: 16, heightPx: 16 });
  view.render(frameOf("blend view", [drawOf(full, additive, [1, 1, 1, 0.5])]));
  const bytes = await view.readBack();
  const red = bytes[(8 * 16 + 8) * 4] ?? Number.NaN;
  checks.check(
    "T9.g white at alpha 0.5 over black reads near 188 (linear-light blending)",
    Math.abs(red - 188) <= 1,
    `red ${red}`,
  );
  view.dispose();

  // A kernel writes a 4 × 4 × 4 volume; a draw reads texel (1, 2, 3) back as (0.25, 0.5, 0.75, 1).
  const volume = engine.createTexture({
    name: "smoke volume",
    size: [4, 4, 4],
    dimension: "3d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING,
    category: "other",
  });
  const fill = await engine.createComputeAsync({
    name: "smoke volume fill",
    readback: "bit-exact",
    subgroup: null,
    reference: `
@group(0) @binding(0) var volume : texture_storage_3d<rgba16float, write>;
@compute @workgroup_size(4, 4, 4) fn main(@builtin(global_invocation_id) id : vec3u) {
  textureStore(volume, id, vec4f(vec3f(id) / 4.0, 1.0));
}`,
  });
  engine.dispatch(
    fill,
    { uniforms: {}, buffers: {}, sampled: {}, storage: { volume: { texture: volume, level: 0 } } },
    [1, 1, 1],
  );
  const sampler = engine.createMaterial({
    ...flatSpec("volume read"),
    textures: [{ name: "volume", binding: 0, viewDimension: "3d" }],
    fragmentWgsl: `${FRAME_WGSL}
@group(2) @binding(0) var volume : texture_3d<f32>;
@fragment fn fragmentMain() -> @location(0) vec4f {
  return textureLoad(volume, vec3i(1, 2, 3), 0);
}`,
  });
  const target = smallTarget(engine, "volume", 4, false);
  target.render(frameOf("volume", [drawOf(full, sampler, [0, 0, 0, 0], { textures: { volume } })]));
  const read = texel(halfTexels(await engine.readTexture(target.colour)), 4, 1, 1);
  checks.check(
    "T9.g a kernel's 3D storage texture is sampled by a draw",
    near(read, [0.25, 0.5, 0.75, 1], 1e-3),
    show(read),
  );
  target.dispose();

  // The packed cube: two levels from the CPU, then from a kernel's buffer, read back each time.
  const cube = engine.createPackedCube(4, 2, "other");
  const levels = [4, 2] as const;
  const expected = levels.map((size, level) =>
    Uint32Array.from({ length: 6 * size * size }, (_, index) => (level + 1) * 100_000 + index),
  );
  expected.forEach((packed, level) => {
    engine.writePackedCubeLevel(cube, level, packed);
  });
  for (const [level] of levels.entries()) {
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const back = new Uint32Array(await engine.readTexture(cube, level));
    const want = expected[level] ?? new Uint32Array();
    checks.check(
      `T9.g packed cube level ${level} written from the CPU reads back`,
      back.length === want.length && back.every((value, index) => value === want[index]),
      `${back.length} texels`,
    );
  }
  const bake = await engine.createComputeAsync({
    name: "smoke cube bake",
    readback: "bit-exact",
    subgroup: null,
    reference: `
struct Level { size : u32, base : u32 }
@group(0) @binding(0) var<uniform> level : Level;
@group(0) @binding(1) var<storage, read_write> packed : array<u32>;
@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id : vec3u) {
  // Rows padded to 256 bytes (64 words); rows of a face, then faces.
  let face = id.z;
  let row = id.y;
  let column = id.x;
  packed[(face * level.size + row) * 64u + column] =
    level.base + (face * level.size + row) * level.size + column;
}`,
  });
  for (const [level, size] of levels.entries()) {
    const buffer = engine.createBuffer({
      name: `smoke cube level ${level}`,
      bytes: 256 * size * 6,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
      category: "other",
    });
    const base = (level + 7) * 100_000;
    engine.dispatch(
      bake,
      {
        uniforms: { level: new Uint32Array([size, base]) },
        buffers: { packed: buffer },
        sampled: {},
        storage: {},
      },
      [size, size, 6],
    );
    engine.writePackedCubeLevelFromBuffer(cube, level, buffer);
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const back = new Uint32Array(await engine.readTexture(cube, level));
    checks.check(
      `T9.g packed cube level ${level} written from a kernel's buffer reads back`,
      back.length === 6 * size * size && back.every((value, index) => value === base + index),
      `${back.length} texels, first ${back[0]}, last ${back.at(-1)}`,
    );
  }
}

/** T9.i: material state, instancing and storage buffers, post-process inputs, depth reads, alpha, the splat. */
export async function checkMaterialState(engine: RenderEngine, checks: Checks): Promise<void> {
  const full = fullScreenMesh(engine, "state full");
  const behind = fullScreenMesh(engine, "state behind", -2);
  const target = smallTarget(engine, "state", 8);
  const occluder = engine.createMaterial(flatSpec("occluder", { colourWrites: false }));
  const flat = engine.createMaterial(flatSpec("state flat"));
  target.render(
    frameOf("occluder", [drawOf(full, occluder, [1, 1, 1, 1]), drawOf(behind, flat, [0, 1, 0, 1])]),
  );
  const hidden = texel(halfTexels(await engine.readTexture(target.colour)), 8, 4, 4);
  checks.check(
    "T9.i an occluder without colour writes leaves colour and hides what is behind",
    near(hidden, [0, 0, 0, 1], 0),
    show(hidden),
  );

  const noDepth = engine.createMaterial(flatSpec("no depth write", { depthWrite: false }));
  target.render(frameOf("no depth write", [drawOf(full, noDepth, [1, 0, 0, 1])]));
  const depth = new Float32Array(await engine.readTexture(target.depth ?? target.colour));
  checks.check(
    "T9.i a draw without depth writes leaves depth at the clear",
    depth.every((value) => value === 0),
    `depth ${depth[0]}`,
  );

  const additive = engine.createMaterial(
    flatSpec("sprite", { blend: "additive", depthWrite: false }),
  );
  target.render(
    frameOf("two sprites", [
      drawOf(full, additive, [0.25, 0.5, 0, 1]),
      drawOf(full, additive, [0.25, 0.5, 0, 1]),
    ]),
  );
  const summed = texel(halfTexels(await engine.readTexture(target.colour)), 8, 4, 4);
  checks.check(
    "T9.i two additive sprites sum in linear light, alpha kept",
    near(summed, [0.5, 1, 0, 1], 1e-3),
    show(summed),
  );

  // Alpha: over a drawn alpha of 2, each blending mode keeps it.
  for (const blend of ["additive", "premultiplied"] as const) {
    const material = engine.createMaterial(
      flatSpec(`alpha ${blend}`, { blend, depthWrite: false }),
    );
    target.render(
      frameOf(`alpha ${blend}`, [
        drawOf(full, flat, [0.25, 0.5, 1, 2]),
        drawOf(full, material, [0.5, 0.5, 0.5, 0.5]),
      ]),
    );
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const seen = texel(halfTexels(await engine.readTexture(target.colour)), 8, 4, 4);
    const want = blend === "additive" ? [0.5, 0.75, 1.25, 2] : [0.625, 0.75, 1, 2];
    checks.check(
      `T9.i ${blend} keeps alpha 2${blend === "premultiplied" ? ", colour c_src + 0.5 c_dst" : ""}`,
      near(seen, want, 1e-3),
      show(seen),
    );
  }
  target.dispose();

  await checkInstances(engine, checks);
  await checkPostProcessAndDepthRead(engine, checks, full);
  await checkSplat(engine, checks);
}

/** 64 instances of a quad, x from a per-instance attribute, y from a storage buffer. */
async function checkInstances(engine: RenderEngine, checks: Checks): Promise<void> {
  const offsets = Float32Array.from({ length: 64 }, (_, index) => -1 + 0.25 * (index % 8));
  const quad = engine.createMesh({
    name: "instance quad",
    positions: new Float32Array([
      0, 0, -1, 0.25, 0, -1, 0.25, 0.25, -1, 0, 0, -1, 0.25, 0.25, -1, 0, 0.25, -1,
    ]),
    indices: null,
    topology: "triangle-list",
    attributes: {},
    instanceAttributes: { column: { data: offsets, size: 1 } },
  });
  const rows = engine.createBuffer({
    name: "instance rows",
    bytes: 64 * 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(
    rows,
    0,
    Float32Array.from({ length: 64 }, (_, index) => 0.75 - 0.25 * Math.floor(index / 8)),
  );
  const source = `${FRAME_WGSL}
@group(2) @binding(0) var<storage, read> rows : array<f32>;
struct Out { @builtin(position) position : vec4f, @location(0) shade : f32 }
@vertex fn vertexMain(@location(0) corner : vec3f, @location(1) column : f32, @builtin(instance_index) instance : u32) -> Out {
  var out : Out;
  out.position = frame.clipProjection * vec4f(corner.x + column, corner.y + rows[instance], corner.z, 1.0);
  out.shade = f32(instance) / 64.0;
  return out;
}
@fragment fn fragmentMain(in : Out) -> @location(0) vec4f { return vec4f(1.0, in.shade, 0.0, 1.0); }`;
  const material = engine.createMaterial({
    ...flatSpec("instances"),
    uniforms: [],
    vertexWgsl: source,
    fragmentWgsl: source,
    storageBuffers: [{ name: "rows", binding: 0 }],
  });
  const target = smallTarget(engine, "instances", 16);
  target.render(
    frameOf("instances", [
      {
        mesh: quad,
        material,
        offsetFromCameraM: new Float32Array(3),
        uniforms: {},
        textures: {},
        instanceCount: 64,
        storageBuffers: { rows },
      },
    ]),
  );
  const texels = halfTexels(await engine.readTexture(target.colour));
  const misplaced: number[] = [];
  for (let index = 0; index < 64; index += 1) {
    const seen = texel(texels, 16, 2 * (index % 8) + 1, 2 * Math.floor(index / 8) + 1);
    if (!near(seen, [1, index / 64, 0, 1], 2e-3)) {
      misplaced.push(index);
    }
  }
  checks.check(
    "T9.i 64 instances land at their 64 positions",
    misplaced.length === 0,
    misplaced.length === 0 ? "all 64" : `misplaced ${misplaced.join(", ")}`,
  );
  target.dispose();
}

/** A post-process with a uniform; a full-screen draw reading another target's depth, and refused on its own. */
async function checkPostProcessAndDepthRead(
  engine: RenderEngine,
  checks: Checks,
  full: ReturnType<RenderEngine["createMesh"]>,
): Promise<void> {
  const flat = engine.createMaterial(flatSpec("pp flat"));
  const gain = engine.createPostProcess({
    name: "gain",
    displayName: "TEST GAIN",
    uniforms: [{ name: "gain", type: "f32" }],
    fragmentWgsl: `
struct Draw { gain : f32 }
@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var colour : texture_2d<f32>;
@group(2) @binding(1) var colourSampler : sampler;
@fragment fn fragmentMain(@location(0) uv : vec2f) -> @location(0) vec4f {
  return textureSample(colour, colourSampler, uv) * draw.gain + vec4f(0.25, 0.0, 0.0, 0.0);
}`,
  });
  const target = smallTarget(engine, "post", 8);
  for (const value of [2, 3]) {
    target.render(
      frameOf(`gain ${value}`, [drawOf(full, flat, [0.25, 0.5, 1, 1])], 1, [
        { postProcess: gain, uniforms: { gain: new Float32Array([value]) } },
      ]),
    );
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const seen = texel(halfTexels(await engine.readTexture(target.colour)), 8, 4, 4);
    checks.check(
      `T9.i a post-process reads hdr-colour and its uniform (gain ${value})`,
      near(seen, [0.25 * value + 0.25, 0.5 * value, value, value], 1e-3),
      show(seen),
    );
  }

  // The scene 2 m ahead in one target; a full-screen draw in another reads its depth.
  const scene = smallTarget(engine, "depth source", 8);
  const behind = fullScreenMesh(engine, "depth source mesh", -2);
  scene.render(frameOf("depth source", [drawOf(behind, flat, [1, 1, 1, 1])]));
  const depthHandle = scene.depth;
  if (depthHandle === null) {
    throw new Error("the depth source has no depth");
  }
  const reader = engine.createMaterial({
    ...flatSpec("depth reader", { depthWrite: false }),
    textures: [{ name: "sceneDepth", binding: 0, sampleType: "depth" }],
    fragmentWgsl: `${FRAME_WGSL}
@group(2) @binding(0) var sceneDepth : texture_depth_2d;
@fragment fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  return vec4f(textureLoad(sceneDepth, vec2i(position.xy), 0) * 10.0, 0.0, 0.0, 1.0);
}`,
  });
  const readerDraw = drawOf(full, reader, [0, 0, 0, 0], { textures: { sceneDepth: depthHandle } });
  const output = smallTarget(engine, "depth output", 8, false);
  output.render(frameOf("depth read", [readerDraw]));
  const seen = texel(halfTexels(await engine.readTexture(output.colour)), 8, 4, 4);
  checks.check(
    "T9.i a full-screen draw reads another target's depth as texture_depth_2d",
    near(seen, [0.5, 0, 0, 1], 1e-3),
    show(seen),
  );
  let refused = "nothing thrown";
  try {
    scene.render(frameOf("depth self", [readerDraw]));
  } catch (error: unknown) {
    refused =
      error instanceof DepthSelfSample ? `DepthSelfSample: ${error.message}` : String(error);
  }
  checks.check(
    "T9.i the same draw into its own depth's target throws DepthSelfSample",
    refused.startsWith("DepthSelfSample"),
    refused,
  );
  for (const disposable of [target, scene, output]) {
    disposable.dispose();
  }
}

/** 1,000 unit points onto a 64 × 64 `rgba32float` face, summing per texel. */
async function checkSplat(engine: RenderEngine, checks: Checks): Promise<void> {
  if (!engine.capabilities.float32Blendable) {
    checks.check("T9.i the splat is skipped without float32-blendable", true, "withheld");
    return;
  }
  const count = 1000;
  const points = Float32Array.from({ length: count * 2 }, (_, index) => {
    const point = Math.floor(index / 2);
    return index % 2 === 0
      ? (((point * 37) % 64) + 0.5) / 32 - 1
      : 1 - (((point * 11) % 64) + 0.5) / 32;
  });
  const expected = new Uint32Array(64 * 64);
  for (let point = 0; point < count; point += 1) {
    const at = ((point * 11) % 64) * 64 + ((point * 37) % 64);
    expected[at] = (expected[at] ?? 0) + 1;
  }
  const buffer = engine.createBuffer({
    name: "splat points",
    bytes: points.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(buffer, 0, points);
  const face = engine.createTexture({
    name: "splat face",
    size: [64, 64],
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: "other",
  });
  const splat = engine.createPointSplat(ENGINE_CHECK_SPLAT);
  splat.draw(face, buffer, count);
  const sums = new Float32Array(await engine.readTexture(face));
  let wrong = 0;
  for (let index = 0; index < 64 * 64; index += 1) {
    if (sums[index * 4] !== expected[index]) {
      wrong += 1;
    }
  }
  checks.check(
    "T9.i 1,000 splatted points sum per texel to the count landing there",
    wrong === 0,
    `${wrong} texels differ`,
  );
  splat.dispose();
}

/** T9.i: `createPointSplat` throws without `float32-blendable`. */
export function checkSplatRefused(engine: RenderEngine, checks: Checks): void {
  let refused = "nothing thrown";
  try {
    engine.createPointSplat({
      name: "refused splat",
      format: "rgba32float",
      blend: "additive",
      vertexWgsl: "",
      fragmentWgsl: "",
    });
  } catch (error: unknown) {
    refused = error instanceof Float32BlendUnavailable ? "Float32BlendUnavailable" : String(error);
  }
  checks.check(
    "T9.i without float32-blendable createPointSplat throws Float32BlendUnavailable",
    refused === "Float32BlendUnavailable",
    refused,
  );
}
