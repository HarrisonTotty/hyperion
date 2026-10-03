/**
 * R07.T15's smoke checks: the tone-mapping pass on a canvas through its own non-sRGB view against
 * its TypeScript twin at pinned inputs, the dither within one code and unbiased, the upscale of a
 * flat field, and a following pass that loads what it drew (decision 2026-10-02, item 6).
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { RenderEngine, TextureHandle, ViewSize } from "../view/engine/types";
import type { Rgb } from "../view/photometry/toneCurve";
import { fullScreenTriangle, GLARE_SOURCE_BYTES } from "../view/post/bloomChain";
import { BLUE_NOISE_SIDE, blueNoiseTile } from "../view/post/blueNoise";
import { glareSpreadTerms } from "../view/post/glare";
import { TONEMAP_MATERIAL, tonemapDraw, tonemapTexel } from "../view/post/tonemap";
import { addCanvas } from "./frames";
import { type Checks, halfBits, halfToNumber, show } from "./harness";

/** The canvas's size, and the HDR input's at full and half resolution. */
const CANVAS: ViewSize = { widthPx: 32, heightPx: 16 };
const HALF: ViewSize = { widthPx: 16, heightPx: 8 };

/** The 4 × 4 identity, column-major. */
const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

/** An `rgba16float` input texture holding `light(x, y)` per texel, and what it holds. */
function hdrInput(
  engine: RenderEngine,
  name: string,
  size: ViewSize,
  light: (x: number, y: number) => Rgb,
): { readonly texture: TextureHandle; readonly held: (x: number, y: number) => Rgb } {
  const bits = new Uint16Array(4 * size.widthPx * size.heightPx);
  for (let y = 0; y < size.heightPx; y += 1) {
    for (let x = 0; x < size.widthPx; x += 1) {
      const [r, g, b] = light(x, y);
      bits.set([halfBits(r), halfBits(g), halfBits(b), halfBits(1)], 4 * (y * size.widthPx + x));
    }
  }
  const texture = engine.createTexture({
    name,
    size: [size.widthPx, size.heightPx],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    texture,
    { x: 0, y: 0 },
    { width: size.widthPx, height: size.heightPx },
    bits,
  );
  const held = (x: number, y: number): Rgb => {
    const at = 4 * (y * size.widthPx + x);
    return [
      halfToNumber(bits[at] ?? 0),
      halfToNumber(bits[at + 1] ?? 0),
      halfToNumber(bits[at + 2] ?? 0),
    ];
  };
  return { texture, held };
}

/** The pinned inputs: a grey ramp over 20 stops along x, and three colours along y. */
function pinned(x: number, y: number): Rgb {
  const grey = 1e-4 * 2 ** ((20 * x) / (CANVAS.widthPx - 1));
  switch (y % 4) {
    case 0:
      return [grey, grey, grey];
    case 1:
      return [grey, 0.3 * grey, 0.05 * grey];
    case 2:
      return [0.1 * grey, 0.6 * grey, grey];
    default:
      return [0.5 * grey, grey, 0.2 * grey];
  }
}

/** R07.T15: the pass, its dither, its upscale, and a pass that loads over it. */
export async function checkTonemap(engine: RenderEngine, checks: Checks): Promise<void> {
  const mesh = fullScreenTriangle(engine, "smoke tonemap triangle");
  const material = await engine.createMaterialAsync(TONEMAP_MATERIAL, ["canvas-in-pass"], [mesh]);
  const noiseBits = Uint16Array.from(blueNoiseTile(), (v) => halfBits(v));
  const blueNoise = engine.createTexture({
    name: "smoke blue noise",
    size: [BLUE_NOISE_SIDE, BLUE_NOISE_SIDE],
    dimension: "2d",
    format: "r16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    blueNoise,
    { x: 0, y: 0 },
    { width: BLUE_NOISE_SIDE, height: BLUE_NOISE_SIDE },
    noiseBits,
  );
  const bloomUp = engine.createTexture({
    name: "smoke tonemap bloom",
    size: [1, 1],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  const glareSources = engine.createBuffer({
    name: "smoke tonemap sources",
    bytes: GLARE_SOURCE_BYTES,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  const terms = glareSpreadTerms("eye", { ageYears: 25, pigmentation: 0.5 });
  const view = engine.createView(addCanvas(), "smoke tonemap view");
  view.resize(CANVAS);

  const draw = (hdrColour: TextureHandle, dither: number): ReturnType<typeof tonemapDraw> =>
    tonemapDraw({
      mesh,
      material,
      hdrColour,
      bloomUp,
      blueNoise,
      glareSources,
      uniforms: {
        exposure: 1,
        threshold: 1e9,
        levelZeroWeight: 0,
        levelOneWeight: 0,
        sourceCount: 0,
        terms,
        dither: dither === 1,
      },
    });
  const frame = (hdr: TextureHandle, dither: number): Parameters<typeof view.render>[0] => ({
    label: "tonemap",
    viewRotation: IDENTITY,
    projection: IDENTITY,
    draws: [draw(hdr, dither)],
    postProcesses: [],
    encoding: "in-pass",
  });
  const codeAt = (bytes: Uint8Array | Float32Array, x: number, y: number, c: number): number =>
    bytes[4 * (y * CANVAS.widthPx + x) + c] ?? Number.NaN;

  // The pass against its twin, undithered.
  const input = hdrInput(engine, "smoke tonemap input", CANVAS, pinned);
  view.render(frame(input.texture, 0));
  const plain = await view.readBack();
  const misses: string[] = [];
  for (let y = 0; y < CANVAS.heightPx; y += 1) {
    for (let x = 0; x < CANVAS.widthPx; x += 1) {
      const expected = tonemapTexel(input.held(x, y), 1, null);
      for (const c of [0, 1, 2] as const) {
        const code = codeAt(plain, x, y, c);
        if (!(Math.abs(code - expected[c] * 255) <= 1)) {
          misses.push(`(${x},${y},${c}) ${code} against ${(expected[c] * 255).toFixed(2)}`);
        }
      }
    }
  }
  checks.check(
    "R07.T15 the pass in WGSL equals its twin within one code at pinned inputs",
    misses.length === 0,
    misses.length === 0
      ? `${CANVAS.widthPx * CANVAS.heightPx} texels`
      : misses.slice(0, 4).join("; "),
  );

  // The dither: within one code of the undithered pass, and unbiased over a flat field whose
  // encoded value falls half-way between two codes.
  view.render(frame(input.texture, 1));
  const dithered = await view.readBack();
  let worst = 0;
  for (let i = 0; i < CANVAS.widthPx * CANVAS.heightPx * 4; i += 1) {
    if (i % 4 !== 3) {
      worst = Math.max(worst, Math.abs((dithered[i] ?? 0) - (plain[i] ?? 0)));
    }
  }
  checks.check(
    "R07.T15 the dither moves no texel by more than one code",
    worst <= 1,
    `largest move ${worst}`,
  );
  let level = 0.18;
  for (let step = 0; step < 40; step += 1) {
    const code = tonemapTexel([level, level, level], 1, null)[1] * 255;
    level *= code % 1 < 0.5 ? 1.002 : 0.998;
  }
  const flat = hdrInput(engine, "smoke tonemap flat", CANVAS, () => [level, level, level]);
  view.render(frame(flat.texture, 1));
  const flatBytes = await view.readBack();
  const target = tonemapTexel(flat.held(0, 0), 1, null)[1] * 255;
  let sum = 0;
  for (let i = 0; i < CANVAS.widthPx * CANVAS.heightPx; i += 1) {
    sum += flatBytes[4 * i + 1] ?? 0;
  }
  const mean = sum / (CANVAS.widthPx * CANVAS.heightPx);
  checks.check(
    "R07.T15 the dither is unbiased over a flat field",
    Math.abs(mean - target) < 0.15,
    `mean code ${mean.toFixed(3)} against ${target.toFixed(3)}`,
  );

  // The upscale: a flat field at half resolution fills the canvas evenly.
  const half = hdrInput(engine, "smoke tonemap half", HALF, () => [0.18, 0.18, 0.18]);
  view.render(frame(half.texture, 0));
  const upscaled = await view.readBack();
  const first = codeAt(upscaled, 0, 0, 1);
  let even = true;
  for (let i = 0; i < CANVAS.widthPx * CANVAS.heightPx; i += 1) {
    even &&= (upscaled[4 * i + 1] ?? -1) === first;
  }
  checks.check(
    "R07.T15 the pass upscales a half-resolution flat field evenly",
    even && Math.abs(first - tonemapTexel(half.held(0, 0), 1, null)[1] * 255) <= 1,
    `code ${first}`,
  );

  // A following pass that loads keeps the tone-mapped image (T16's symbology draws in it).
  view.render(frame(input.texture, 0));
  view.render({
    label: "symbology",
    viewRotation: IDENTITY,
    projection: IDENTITY,
    draws: [],
    postProcesses: [],
    colourLoad: "load",
  });
  const loaded = await view.readBack();
  let same = true;
  for (let i = 0; i < loaded.length; i += 1) {
    same &&= loaded[i] === plain[i];
  }
  checks.check(
    "R07.T15 a following pass with colourLoad load keeps the image",
    same,
    `first texel ${show(loaded.subarray(0, 4))}`,
  );
  view.dispose();
}
