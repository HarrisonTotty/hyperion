/**
 * R07.T14.b's smoke checks: the bloom chain on the device against its CPU twin, energy kept across
 * the clamp (stored and bloomed on the device, injected in closed form), and the glare sources'
 * veil in WGSL against its TypeScript twin at pinned pixels.
 *
 * @remarks
 * The HDR input is the check's own texture (decision 2026-10-02, item 1). The composite here is
 * the tone-mapping pass's bloom and glare steps without AgX, writing linear light to an
 * `rgba16float` target so that its energy can be summed.
 */

import FRAME_WGSL from "../view/shaders/frame.wgsl?raw";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { RenderEngine, TextureHandle, ViewSize, WgslMaterialSpec } from "../view/engine/types";
import type { TargetRounding } from "../view/engine/status";
import { HALF_FLOAT_MAX } from "../view/photometry/toneCurve";
import {
  bloomChain,
  bloomEnergy,
  bloomExcess,
  bloomImage,
  bloomKernel,
  bloomThreshold,
  levelWeight,
} from "../view/post/bloom";
import {
  BloomChain,
  fullScreenTriangle,
  packGlareSources,
  packGlareTerms,
} from "../view/post/bloomChain";
import {
  glareSourceSolidAngleSr,
  glareSourceVeilOf,
  glareSpreadTerms,
  type GlareSource,
} from "../view/post/glare";
import GLARE_WGSL from "../view/post/glare.wgsl?raw";
import { type Checks, halfTexels, halfToNumber, show } from "./harness";
import { halfBits } from "./histogram";

/** The check's frame: large enough that the chain's widest level keeps its energy inside it. */
const SIZE: ViewSize = { widthPx: 512, heightPx: 512 };

/** One pixel's angle, rad: 60° across 1920 px, the scale the kernel is fitted at. */
const RAD_PER_PX = (60 * Math.PI) / 180 / 1920;

/** The disc's radius, px, and its true luminance in the target's units. */
const DISC_PX = 10;
const DISC_LUMINANCE = 1.5e5;
const SKY = 1e-3;

const EYE = { ageYears: 25, pigmentation: 0.5 };

const COMPOSITE_WGSL = `${FRAME_WGSL}
${GLARE_WGSL}
struct Draw {
  offsetFromCameraM : vec3f,
  threshold : f32,
  levelZeroWeight : f32,
  levelOneWeight : f32,
  sourceCount : u32,
  glarePoisson0 : vec4f,
  glarePoisson1 : vec4f,
  glarePoisson2 : vec4f,
  glareBroad : vec4f,
  glareMisc : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var hdrColour : texture_2d<f32>;
@group(2) @binding(1) var bloomUp : texture_2d<f32>;
@group(2) @binding(2) var<storage, read> glareSources : array<GlareSourceGpu>;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 0.0, 1.0);
}

@fragment fn fragmentMain(@builtin(position) fragment : vec4f) -> @location(0) vec4f {
  let hdr = textureLoad(hdrColour, vec2i(fragment.xy), 0).rgb;
  var light = min(hdr, vec3f(draw.threshold));
  light += draw.levelZeroWeight * bloom_excess(hdr, draw.threshold);
  light += draw.levelOneWeight * bloom_tent(bloomUp, fragment.xy, frame.viewport.xy);
  let terms = GlareTerms(
    draw.glarePoisson0, draw.glarePoisson1, draw.glarePoisson2, draw.glareBroad, draw.glareMisc,
  );
  let pixel = glare_pixel_direction(fragment.xy, frame.viewport, frame.clipProjection);
  for (var i = 0u; i < draw.sourceCount; i++) {
    var source = glareSources[i];
    source.direction = normalize((frame.viewRotation * vec4f(source.direction, 0.0)).xyz);
    light += glare_veil(source, pixel, terms);
  }
  return vec4f(light, 1.0);
}
`;

/** The tone-mapping pass's bloom and glare steps, linear, for the harness alone. */
const COMPOSITE_MATERIAL: WgslMaterialSpec = {
  name: "smoke bloom composite",
  displayName: "GLARE COMPOSITE",
  vertexWgsl: COMPOSITE_WGSL,
  fragmentWgsl: COMPOSITE_WGSL,
  uniforms: [
    { name: "threshold", type: "f32" },
    { name: "levelZeroWeight", type: "f32" },
    { name: "levelOneWeight", type: "f32" },
    { name: "sourceCount", type: "u32" },
    { name: "glarePoisson0", type: "vec4f" },
    { name: "glarePoisson1", type: "vec4f" },
    { name: "glarePoisson2", type: "vec4f" },
    { name: "glareBroad", type: "vec4f" },
    { name: "glareMisc", type: "vec4f" },
  ],
  samplers: [],
  textures: [
    { name: "hdrColour", binding: 0 },
    { name: "bloomUp", binding: 1 },
  ],
  storageBuffers: [{ name: "glareSources", binding: 2 }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The 4 × 4 identity, column-major. */
const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

/** A projection with the field `RAD_PER_PX` gives the frame, reversed-Z infinite, column-major. */
function projection(): Float32Array {
  const fx = 1 / Math.tan((SIZE.widthPx * RAD_PER_PX) / 2);
  const fy = 1 / Math.tan((SIZE.heightPx * RAD_PER_PX) / 2);
  return new Float32Array([fx, 0, 0, 0, 0, fy, 0, 0, 0, 0, 0, -1, 0, 0, 0.1, 0]);
}

/** The view-space direction through pixel (x, y), as `glare_pixel_direction` computes it. */
function pixelDirection(x: number, y: number): readonly [number, number, number] {
  const p = projection();
  const ndcX = ((x + 0.5) / SIZE.widthPx) * 2 - 1;
  const ndcY = 1 - ((y + 0.5) / SIZE.heightPx) * 2;
  const v: [number, number, number] = [ndcX / (p[0] ?? 1), ndcY / (p[5] ?? 1), -1];
  const n = Math.hypot(...v);
  return [v[0] / n, v[1] / n, v[2] / n];
}

/** The synthetic HDR frame, red channel only: the clamped disc at the centre over a dark sky. */
function hdrFrame(): {
  readonly bits: Uint16Array;
  readonly red: Float64Array;
  readonly discPixels: number;
} {
  const n = SIZE.widthPx * SIZE.heightPx;
  const bits = new Uint16Array(4 * n);
  const red = new Float64Array(n);
  let discPixels = 0;
  const centre = SIZE.widthPx / 2;
  const sky = halfBits(SKY);
  const disc = halfBits(HALF_FLOAT_MAX);
  for (let y = 0; y < SIZE.heightPx; y += 1) {
    for (let x = 0; x < SIZE.widthPx; x += 1) {
      const inDisc = Math.hypot(x - centre, y - centre) <= DISC_PX;
      discPixels += inDisc ? 1 : 0;
      const value = inDisc ? disc : sky;
      bits.set([value, 0, 0, halfBits(1)], 4 * (y * SIZE.widthPx + x));
      red[y * SIZE.widthPx + x] = halfToNumber(value);
    }
  }
  return { bits, red, discPixels };
}

/** R07.T14.b: the chain, the energy and the veil on the device. */
export async function checkBloom(
  engine: RenderEngine,
  rounding: TargetRounding,
  checks: Checks,
): Promise<void> {
  const frame = hdrFrame();
  const hdr: TextureHandle = engine.createTexture({
    name: "smoke bloom input",
    size: [SIZE.widthPx, SIZE.heightPx],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    hdr,
    { x: 0, y: 0 },
    { width: SIZE.widthPx, height: SIZE.heightPx },
    frame.bits,
  );
  const kernel = bloomKernel("high", "eye", RAD_PER_PX, EYE);
  const threshold = bloomThreshold(1, 1);
  const chain = await BloomChain.create(engine, "smoke", SIZE, kernel);
  chain.run(hdr, threshold);

  const mesh = fullScreenTriangle(engine, "smoke composite triangle");
  const material = await engine.createMaterialAsync(COMPOSITE_MATERIAL, ["rgba16float"], [mesh]);
  const terms = glareSpreadTerms("eye", EYE);
  const centre = SIZE.widthPx / 2;
  const source: GlareSource = {
    direction: (() => {
      const [x, y, z] = pixelDirection(centre, centre);
      return { x, y, z };
    })(),
    angularRadiusRad: Math.sqrt(frame.discPixels / Math.PI) * RAD_PER_PX,
    excessLuminance: [DISC_LUMINANCE - HALF_FLOAT_MAX, 0, 0],
  };
  const sources = engine.createBuffer({
    name: "smoke glare sources",
    bytes: 48,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(sources, 0, packGlareSources([source], 1, terms));
  const composite = async (sourceCount: number): Promise<Float32Array> => {
    const target = engine.createRenderTarget({
      name: `smoke composite ${sourceCount}`,
      size: SIZE,
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    target.render({
      label: "tonemap",
      viewRotation: IDENTITY,
      projection: projection(),
      draws: [
        {
          mesh,
          material,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            threshold: new Float32Array([threshold]),
            levelZeroWeight: new Float32Array([levelWeight(kernel, 0)]),
            levelOneWeight: new Float32Array([chain.levelOneWeight]),
            sourceCount: new Float32Array([sourceCount]),
            ...packGlareTerms(terms),
          },
          textures: { hdrColour: hdr, bloomUp: chain.levelOne },
          storageBuffers: { glareSources: sources },
        },
      ],
      postProcesses: [],
    });
    const texels = halfTexels(await engine.readTexture(target.colour));
    target.dispose();
    return texels;
  };

  // The chain against its twin: the stored disc's light, bloomed, summed over the frame.
  const plain = await composite(0);
  const gpuRed = Float64Array.from(
    { length: SIZE.widthPx * SIZE.heightPx },
    (_, i) => plain[4 * i] ?? 0,
  );
  const excess = bloomImage(SIZE.widthPx, SIZE.heightPx);
  const shown = bloomImage(SIZE.widthPx, SIZE.heightPx);
  frame.red.forEach((value, i) => {
    excess.values[i] = bloomExcess(value, threshold);
    shown.values[i] = Math.min(value, threshold);
  });
  const twin = bloomEnergy(shown) + bloomEnergy(bloomChain(excess, kernel, rounding));
  const gpu = gpuRed.reduce((sum, v) => sum + v, 0);
  checks.check(
    "R07.T14.b the chain's light on the device equals its CPU twin's to 0.5%",
    Math.abs(gpu / twin - 1) < 0.005,
    `device ${gpu.toPrecision(7)}, twin ${twin.toPrecision(7)} (${rounding} rounding)`,
  );

  // Stored and bloomed on the device, plus the injected veil in closed form (L_ex Ω over the
  // sphere), against the unclamped light.
  const injected = (source.excessLuminance[0] * glareSourceSolidAngleSr(source)) / RAD_PER_PX ** 2;
  const unclamped =
    frame.discPixels * DISC_LUMINANCE +
    (SIZE.widthPx * SIZE.heightPx - frame.discPixels) * (frame.red[0] ?? 0);
  checks.check(
    "R07.T14.b stored plus injected energy equals the unclamped energy to 1.5%",
    Math.abs((gpu + injected) / unclamped - 1) < 0.015,
    `stored ${gpu.toPrecision(6)} + injected ${injected.toPrecision(6)} against ${unclamped.toPrecision(6)}`,
  );

  // The veil in WGSL against its twin, at pixels beyond the limb.
  const veiled = await composite(1);
  const misses: string[] = [];
  for (const beyondPx of [2, 5, 20, 80, 200]) {
    const x = Math.round(centre + DISC_PX + beyondPx);
    const y = Math.round(centre);
    const i = 4 * (y * SIZE.widthPx + x);
    const device = (veiled[i] ?? 0) - (plain[i] ?? 0);
    const [dx, dy, dz] = pixelDirection(x, y);
    const d = source.direction;
    const cross = Math.hypot(dy * d.z - dz * d.y, dz * d.x - dx * d.z, dx * d.y - dy * d.x);
    const theta = Math.atan2(cross, dx * d.x + dy * d.y + dz * d.z);
    const expected = glareSourceVeilOf(source, theta, terms)[0];
    if (!(Math.abs(device / expected - 1) < 0.01)) {
      misses.push(
        `${beyondPx} px: device ${device.toPrecision(5)}, twin ${expected.toPrecision(5)}`,
      );
    }
  }
  checks.check(
    "R07.T14.b the veil in WGSL equals its twin to 1% beyond the limb",
    misses.length === 0,
    misses.length === 0 ? `5 pixels, first ${show(veiled.subarray(0, 4))}` : misses.join("; "),
  );
  chain.dispose();
}
