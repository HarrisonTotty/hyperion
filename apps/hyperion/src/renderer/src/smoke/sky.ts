/**
 * The sky's harness checks (plan R06): the band's HDR draw on a target the check makes itself,
 * since R07.T7 makes the views' scene target (decision record item 1 of 2026-10-02).
 */

import type { HostDiscDto, SkyBand } from "@hyperion/protocol";

import { normalise, vec3 } from "../geometry/vec3";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  viewRotation4,
} from "../view/camera/projection";
import { lookAlong, rotate } from "../view/camera/quaternion";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { AllocationEvent } from "../view/engine/memory";
import type { RenderEngine } from "../view/engine/types";
import {
  BAKE_CLEAR_KERNEL,
  BAKE_PACK_KERNEL,
  BAKE_SPLAT,
  type BakeInput,
  bakeSkyCube,
  bakeSkyCubeOnCpu,
  paddedRowTexels,
  releaseBakedCube,
} from "../view/sky/bake";
import { BandLayer } from "../view/sky/band";
import { METER_CLASS } from "../view/post/meter";
import { HostDiscLayer, rasteriseHostDisc } from "../view/sky/disc";
import { packRgb9e5, unpackRgb9e5 } from "../view/sky/pack";
import { SPLAT_POINT_FLOATS, splatCpu } from "../view/sky/splatCpu";
import { type Checks, frameOf, halfTexels, show, texel } from "./harness";

/** The band's face side in the check, texels. */
const FACE = 4;

/** A white band whose face k (WebGPU's layer order) holds (k + 1) × 10⁻⁴ cd/m². */
function steppedBand(): SkyBand {
  const count = 6 * FACE * FACE;
  return {
    count,
    luminanceCdM2: Float32Array.from(
      { length: count },
      (_, i) => (Math.floor(i / (FACE * FACE)) + 1) * 1e-4,
    ),
    chroma: new Float32Array(count * 2).fill(1 / 3),
    eyeLimitMag: new Float32Array(count).fill(Number.NaN),
    spRatio: new Float32Array(count).fill(2.26),
  };
}

/**
 * R06.T13.d: the band drawn first into an `rgba16float` target, at the view's direction's face,
 * pre-exposed, the target's alpha (R07's meter class) kept.
 */
export async function checkSkyBand(engine: RenderEngine, checks: Checks): Promise<void> {
  const layer = new BandLayer(engine);
  layer.update(steppedBand(), FACE, new Float64Array(6 * FACE * FACE * 3));
  const target = engine.createRenderTarget({
    name: "sky band check",
    size: { widthPx: 8, heightPx: 8 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const draw = layer.draw(1_000);
  if (draw === null) {
    throw new Error("the band layer gave no draw after its update");
  }
  // The identity view looks along −z, face 5, which holds 6 × 10⁻⁴ cd/m²: 0.6 at 1,000.
  target.render(frameOf("sky band check", [draw]));
  const texels = halfTexels(await engine.readTexture(target.colour));
  const centre = texel(texels, 8, 4, 4);
  checks.check(
    "R06.T13.d the band reads its face's luminance, pre-exposed, at the view's centre",
    Math.abs(centre[0] - 0.6) < 0.01 && Math.abs(centre[1] - 0.6) < 0.01,
    show(centre),
  );
  checks.check(
    "R06.T13.d the band keeps the target's alpha, the meter class",
    centre[3] === 1,
    show(centre),
  );
  checks.check(
    "R06.T13.d every band texel is finite",
    texels.every(Number.isFinite),
    `${texels.length / 4} texels`,
  );
  target.dispose();
  layer.dispose();
}

/** A host whose centre is `centralRgb` cd/m² in red, green and blue, darkened by c 0.5, α 1. */
function testHost(centralRgb: readonly [number, number, number]): HostDiscDto {
  const law = { c: 0.5, alpha: 1 };
  // The law's disc average is 1 − cα ÷ (α + 2) = 5 ÷ 6.
  const mean = 5 / 6;
  return {
    star: 0,
    radius_m: Math.sin((15 * Math.PI) / 180),
    teff_k: 5_772,
    log_g: 4.438,
    mean_luminance_cd_m2: [centralRgb[2] * mean, centralRgb[1] * mean, centralRgb[0] * mean],
    central_luminance_cd_m2: [centralRgb[2], centralRgb[1], centralRgb[0]],
    limb: [law, law, law],
    chroma: [1, 1],
    lux_per_v0: 1,
    bake_spectrum: [
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
    ],
  };
}

/**
 * R06.T13.e: a host disc 30° across lights its pixels by its law, pre-exposed and clamped at
 * 65,504, writes R07's meter class `hostDisc` (0) in their alpha, and the band drawn over it
 * keeps that class.
 */
export async function checkSkyDisc(engine: RenderEngine, checks: Checks): Promise<void> {
  const discs = new HostDiscLayer(engine);
  const band = new BandLayer(engine);
  band.update(steppedBand(), FACE, new Float64Array(6 * FACE * FACE * 3));
  const target = engine.createRenderTarget({
    name: "sky disc check",
    size: { widthPx: 64, heightPx: 64 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const look = { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 2 };
  const viewport = { widthPx: 64, heightPx: 64 };
  const ahead = { x: 0, y: 0, z: -1 };
  const frame = discs.frame(
    [{ host: testHost([1, 2, 3]), direction: ahead, distanceM: 1 }],
    look,
    viewport,
    1_000,
  );
  const bandDraw = band.draw(1);
  if (frame.draws.length !== 1 || bandDraw === null) {
    throw new Error("the disc check made no disc draw or no band draw");
  }
  target.render(frameOf("sky disc check", [...frame.draws.map((d) => d.item), bandDraw]));
  const texels = halfTexels(await engine.readTexture(target.colour));
  const centre = texel(texels, 64, 32, 32);
  // Inside the disc's quad (x and y 21 to 43 since R07.T19.e), 13 px from the centre, outside the
  // disc's 8.6 px: the shader's own discard, not the quad's edge, leaves it unlit.
  const outside = texel(texels, 64, 22, 22);
  // The centre: μ ≈ 1, (1, 2, 3) × 1,000 pre-exposed, plus the band's 6 × 10⁻⁴.
  checks.check(
    "R06.T13.e the disc's centre is its central luminance, pre-exposed",
    Math.abs(centre[0] - 1_000) < 15 &&
      Math.abs(centre[1] - 2_000) < 30 &&
      Math.abs(centre[2] - 3_000) < 45,
    show(centre),
  );
  checks.check(
    "R06.T13.e the disc's pixels carry the meter class hostDisc, kept under the band drawn over it",
    centre[3] === 0 && outside[3] === 1,
    `centre ${show(centre)}, outside ${show(outside)}`,
  );
  checks.check("R06.T13.e the disc lights nothing outside it", outside[0] < 1e-2, show(outside));
  const bright = discs.frame(
    [{ host: testHost([1e9, 1e9, 1e9]), direction: ahead, distanceM: 1 }],
    look,
    viewport,
    1,
  );
  target.render(
    frameOf(
      "sky disc clamp check",
      bright.draws.map((d) => d.item),
    ),
  );
  const clamped = texel(halfTexels(await engine.readTexture(target.colour)), 64, 32, 32);
  checks.check(
    "R06.T13.e a disc above a half's range is clamped at 65,504",
    clamped[0] === 65_504 && clamped.every(Number.isFinite),
    show(clamped),
  );
  target.dispose();
  band.dispose();
  await checkSkyDiscQuad(engine, discs, checks);
}

/** The disc quad check's view: 60° across 96 × 64, the camera turned. */
const QUAD_VIEWPORT = { widthPx: 96, heightPx: 64 };
const QUAD_CAMERA = {
  orientation: lookAlong(normalise(vec3(0.3, -0.2, -1)), vec3(0, 1, 0)),
  fovXRad: Math.PI / 3,
};

/** The agreement of a disc's texel with its twin: `rgba16float`'s rounding and `f32`'s shading. */
const DISC_TEXEL_RELATIVE = 4e-3;

/**
 * R07.T19.e: a host disc drawn over its screen rectangle lights the texels its twin lights, with
 * the twin's light, off the view's centre and cut by the view's edges.
 */
async function checkSkyDiscQuad(
  engine: RenderEngine,
  discs: HostDiscLayer,
  checks: Checks,
): Promise<void> {
  const tanHalf = Math.tan(QUAD_CAMERA.fovXRad / 2);
  const tanY = (tanHalf * QUAD_VIEWPORT.heightPx) / QUAD_VIEWPORT.widthPx;
  // Directions in view axes, and distances, m, for the test host of radius sin 15° m.
  const cases = [
    { name: "cut by the right edge", view: vec3(tanHalf, 0.1, -1), distanceM: 2 },
    { name: "over the top left corner", view: vec3(-tanHalf, tanY, -1), distanceM: 2 },
    { name: "inside, off the centre", view: vec3(0.2, -0.15, -1), distanceM: 4 },
  ];
  const target = engine.createRenderTarget({
    name: "sky disc quad check",
    size: QUAD_VIEWPORT,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const results: string[] = [];
  let pass = true;
  try {
    for (const { name, view, distanceM } of cases) {
      const direction = rotate(QUAD_CAMERA.orientation, normalise(view));
      const frame = discs.frame(
        [{ host: testHost([1, 2, 3]), direction, distanceM }],
        QUAD_CAMERA,
        QUAD_VIEWPORT,
        1_000,
      );
      const draw = frame.draws[0];
      if (draw === undefined) {
        throw new Error(`the disc quad check made no draw ${name}`);
      }
      target.render({
        label: "sky disc quad check",
        viewRotation: viewRotation4(QUAD_CAMERA.orientation),
        projection: perspectiveReversedInfinite(
          QUAD_CAMERA.fovXRad,
          QUAD_VIEWPORT.widthPx / QUAD_VIEWPORT.heightPx,
          NEAR_PLANE_M,
        ),
        draws: [draw.item],
        postProcesses: [],
      });
      // Each case is read back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = halfTexels(await engine.readTexture(target.colour));
      const twin = new Map(
        rasteriseHostDisc(draw.record, QUAD_CAMERA, QUAD_VIEWPORT).map((p) => [
          p.yPx * QUAD_VIEWPORT.widthPx + p.xPx,
          p,
        ]),
      );
      let lit = 0;
      let mismatched = 0;
      let worst = 0;
      for (let index = 0; index < QUAD_VIEWPORT.widthPx * QUAD_VIEWPORT.heightPx; index += 1) {
        const gpuLit = texels[index * 4 + 3] === METER_CLASS.hostDisc;
        const pixel = twin.get(index);
        lit += gpuLit ? 1 : 0;
        mismatched += gpuLit === (pixel !== undefined) ? 0 : 1;
        if (gpuLit && pixel !== undefined) {
          for (const c of [0, 1, 2] as const) {
            const gpu = texels[index * 4 + c] ?? Number.NaN;
            const error = Math.abs(gpu - pixel.rgb[c]) / (DISC_TEXEL_RELATIVE * pixel.rgb[c]);
            worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
          }
        }
      }
      const { rect } = draw.record;
      const bounded =
        (rect.rightPx - rect.leftPx) * (rect.bottomPx - rect.topPx) <
        QUAD_VIEWPORT.widthPx * QUAD_VIEWPORT.heightPx;
      pass &&= lit > 20 && mismatched === 0 && worst <= 1 && bounded;
      results.push(
        `${name}: ${String(lit)} texels, ${String(mismatched)} not the twin's, colours within ${worst.toFixed(3)} of the tolerance, over (${String(rect.leftPx)}, ${String(rect.topPx)})–(${String(rect.rightPx)}, ${String(rect.bottomPx)})`,
      );
    }
  } finally {
    target.dispose();
  }
  checks.check(
    "R07.T19.e a host disc drawn over its screen rectangle lights its twin's texels, off the centre and cut by the view's edges",
    pass,
    results.join("; "),
  );
}

/** A seeded generator in [0, 1), the same sequence on every run. */
function lcg(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    return state / 2_147_483_648;
  };
}

/** `count` stars in seeded directions with light spread over four decades, as a bake takes them. */
function bakeStars(count: number, faceSizePx: number): BakeInput {
  const next = lcg(97);
  const directions = Float32Array.from({ length: count * 3 }, () => next() * 2 - 1);
  const illuminanceLx = Float32Array.from({ length: count * 3 }, () => 10 ** (next() * 4 - 9));
  return { directions, illuminanceLx, faceSizePx, name: "sky cube: smoke" };
}

/**
 * R06.T13.g: the bake on the GPU against its TypeScript references: the packer bit for bit, the
 * splat to 10⁻⁶, the whole bake to one mantissa step of the CPU bake, every texel finite, and both
 * memory categories in the allocation events.
 */
export async function checkSkyBake(engine: RenderEngine, checks: Checks): Promise<void> {
  if (!engine.capabilities.float32Blendable) {
    checks.check("R06.T13.g the GPU bake needs float32-blendable", true, "not run on this device");
    return;
  }
  // The packer: 10⁴ texels at a scale of 1 (a peak of 2¹⁵ gives k = 0), against `packRgb9e5`.
  const side = 100;
  const next = lcg(5);
  const texels = Float32Array.from({ length: side * side * 4 }, (_, i) =>
    i % 4 === 3 ? 1 : Math.fround(10 ** (next() * 14 - 9)),
  );
  const level = engine.createTexture({
    name: "sky pack check",
    size: [side, side],
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "sky-scratch",
  });
  engine.writeTexture(level, [0, 0, 0], [side, side, 1], texels);
  const peak = engine.createBuffer({
    name: "sky pack check peak",
    bytes: 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "sky-scratch",
  });
  engine.writeBuffer(peak, 0, new Uint32Array(Float32Array.of(2 ** 15).buffer));
  const stride = paddedRowTexels(side);
  // Level 0's solid angles are not read above level 0; the binding needs a buffer all the same.
  const omega = engine.createBuffer({
    name: "sky pack check solid angles",
    bytes: 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "sky-scratch",
  });
  engine.writeBuffer(omega, 0, Float32Array.of(1));
  const packed = engine.createBuffer({
    name: "sky pack check out",
    bytes: stride * side * 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "sky-scratch",
  });
  engine.dispatch(
    engine.createCompute(BAKE_PACK_KERNEL),
    {
      uniforms: { params: Uint32Array.of(side, stride, 0, 0) },
      buffers: { peak, omega, packed },
      sampled: {},
      storage: { level: { texture: level, level: 0 } },
    },
    [Math.ceil(side / 8), Math.ceil(side / 8), 1],
  );
  const gpuPacked = new Uint32Array(await engine.readBuffer(packed));
  let packWrong = 0;
  for (let y = 0; y < side; y += 1) {
    for (let x = 0; x < side; x += 1) {
      const at = (y * side + x) * 4;
      const want = packRgb9e5(texels[at] ?? 0, texels[at + 1] ?? 0, texels[at + 2] ?? 0);
      if (gpuPacked[y * stride + x] !== want) {
        packWrong += 1;
      }
    }
  }
  checks.check(
    "R06.T13.g the WGSL packer equals the TypeScript packer for 10⁴ texels",
    packWrong === 0,
    `${packWrong} texels differ`,
  );
  engine.releaseTexture(level);
  engine.releaseBuffer(peak);
  engine.releaseBuffer(omega);
  engine.releaseBuffer(packed);

  // The splat: 2,000 seeded stars on each face of 32², against `splatCpu`.
  const size = 32;
  const input = bakeStars(2_000, size);
  const count = input.directions.length / 3;
  const points = new Float32Array(4 + count * SPLAT_POINT_FLOATS);
  points[1] = size;
  for (let star = 0; star < count; star += 1) {
    const at = 4 + star * SPLAT_POINT_FLOATS;
    points.set(input.directions.subarray(star * 3, star * 3 + 3), at);
    points.set(input.illuminanceLx.subarray(star * 3, star * 3 + 3), at + 4);
    points[at + 7] = 1;
  }
  const cpuFaces = splatCpu(points.subarray(4), size);
  const pointBuffer = engine.createBuffer({
    name: "sky splat check points",
    bytes: points.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "sky-scratch",
  });
  engine.writeBuffer(pointBuffer, 0, points);
  const scratch = engine.createTexture({
    name: "sky splat check face",
    size: [size, size],
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage:
      TEXTURE_USAGE.RENDER_ATTACHMENT |
      TEXTURE_USAGE.STORAGE_BINDING |
      TEXTURE_USAGE.COPY_SRC |
      TEXTURE_USAGE.COPY_DST,
    category: "sky-scratch",
  });
  const clear = engine.createCompute(BAKE_CLEAR_KERNEL);
  const splat = engine.createPointSplat(BAKE_SPLAT);
  let worst = 0;
  for (let face = 0; face < 6; face += 1) {
    engine.dispatch(
      clear,
      {
        uniforms: { params: Uint32Array.of(size, 0, 0, 0) },
        buffers: {},
        sampled: {},
        storage: { face: { texture: scratch, level: 0 } },
      },
      [size / 8, size / 8, 1],
    );
    engine.writeBuffer(pointBuffer, 0, Float32Array.of(face, size, 0, 0));
    splat.draw(scratch, pointBuffer, count);
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const gpuFace = new Float32Array(await engine.readTexture(scratch));
    const cpuFace = cpuFaces[face] ?? new Float32Array(0);
    gpuFace.forEach((value, i) => {
      const want = cpuFace[i] ?? 0;
      const scale = Math.max(Math.abs(want), 1e-30);
      worst = Math.max(worst, Math.abs(value - want) / scale);
    });
  }
  checks.check(
    "R06.T13.g the GPU splat equals the CPU splat to 10⁻⁶ relative",
    worst < 1e-6,
    `worst ${worst.toExponential(2)}`,
  );
  splat.dispose();
  engine.releaseBuffer(pointBuffer);
  engine.releaseTexture(scratch);

  // The whole bake, on the GPU and on the CPU, with the allocation events it raises.
  const events: AllocationEvent[] = [];
  const unlisten = engine.onAllocation((event) => events.push(event));
  const gpu = bakeSkyCube(engine, input);
  unlisten();
  const cpu = bakeSkyCubeOnCpu(engine, { ...input, name: "sky cube: smoke reference" });
  const categories = new Set(
    events.flatMap((event) => ("category" in event ? [event.category] : [])),
  );
  checks.check(
    "R06.T13.g the bake's allocation events name sky-cube and sky-scratch",
    categories.has("sky-cube") && categories.has("sky-scratch"),
    [...categories].join(", "),
  );
  let finite = true;
  let steps = 0;
  // Each cube's own scale is undone before they are compared: the two may choose powers of two a
  // factor of two apart when the brightest texel lies at one.
  const scaleOf = async (peakBuffer: typeof gpu.peak): Promise<number> => {
    const bits = new Float32Array(await engine.readBuffer(peakBuffer, "tolerance"))[0] ?? 0;
    const exponent = bits > 0 ? 15 - Math.ceil(Math.log2(bits)) : 0;
    return 2 ** -exponent;
  };
  const [gpuScale, cpuScale] = await Promise.all([scaleOf(gpu.peak), scaleOf(cpu.peak)]);
  const levels = await Promise.all(
    [0, 5].flatMap((levelIndex) => [
      engine.readTexture(gpu.cube, levelIndex),
      engine.readTexture(cpu.cube, levelIndex),
    ]),
  );
  for (let pair = 0; pair < levels.length; pair += 2) {
    const gpuLevel = new Uint32Array(levels[pair] ?? new ArrayBuffer(0));
    const cpuLevel = new Uint32Array(levels[pair + 1] ?? new ArrayBuffer(0));
    gpuLevel.forEach((bits, i) => {
      const a = unpackRgb9e5(bits).map((value) => value * gpuScale);
      const b = unpackRgb9e5(cpuLevel[i] ?? 0).map((value) => value * cpuScale);
      finite = finite && a.every(Number.isFinite);
      a.forEach((value, channel) => {
        const reference = b[channel] ?? 0;
        // One mantissa step of the shared exponent: 2⁻⁸ of the largest channel.
        const step = Math.max(...b) / 256;
        if (Math.abs(value - reference) > step + 1e-30) {
          steps += 1;
        }
      });
    });
  }
  checks.check("R06.T13.g every texel of the baked cube read back is finite", finite, gpu.path);
  checks.check(
    "R06.T13.g the GPU bake equals the CPU bake to one mantissa step, levels 0 and 5",
    steps === 0,
    `${steps} channels differ by more`,
  );
  releaseBakedCube(engine, gpu);
  releaseBakedCube(engine, cpu);
}
