/**
 * How the adapter rounds a colour-attachment write, probed rather than assumed (R01 Design note
 * 22).
 *
 * @remarks
 * R07's probe of 2026-09-29 found that Gen9's colour-attachment writes round toward zero, a bias of
 * −0.78% to −1.56% a write in `rg11b10ufloat` and −0.05% in `rgba16float`; R07's bloom keeps
 * `rgba16float` unless the adapter rounds to nearest. So once the device exists, one full-screen
 * triangle is drawn into a 4 × 1 target of each format (`rg11b10ufloat` only where it is
 * renderable), writing constants three quarters of the way between neighbouring values of the
 * format: 1 + 0.75 × 2⁻¹⁰ for `rgba16float` (10 mantissa bits), and 1 + 0.75 × 2⁻⁶ in red and green
 * and 1 + 0.75 × 2⁻⁵ in blue for `rg11b10ufloat` (6 and 5 mantissa bits). The upper neighbour
 * read back means `nearest`, the lower `toward-zero`, anything else or a failed read `unknown`.
 * One value a format is read, so an adapter that rounds differently by magnitude is classed by
 * that one value (the plan's Risks).
 */

import { TEXTURE_USAGE } from "../gpuFlags";
import type { TextureSpec } from "../memory";
import type { GpuCapabilities } from "../platform";
import type { ProbedTargetFormat, TargetRounding } from "../status";
import type { TextureHandle } from "../types";

/** The texels of the probe's target, on a side. */
export const PROBE_WIDTH_PX = 4;

/** The value the probe writes in each channel: three quarters of a step above 1. */
export const PROBE_VALUES: Readonly<Record<ProbedTargetFormat, readonly [number, number, number]>> =
  {
    rgba16float: [1 + 0.75 * 2 ** -10, 1 + 0.75 * 2 ** -10, 1 + 0.75 * 2 ** -10],
    rg11b10ufloat: [1 + 0.75 * 2 ** -6, 1 + 0.75 * 2 ** -6, 1 + 0.75 * 2 ** -5],
  };

/** The neighbours of each probe value: the step below and the step above, per channel. */
const NEIGHBOURS: Readonly<
  Record<ProbedTargetFormat, ReadonlyArray<{ readonly lower: number; readonly upper: number }>>
> = {
  rgba16float: [
    { lower: 1, upper: 1 + 2 ** -10 },
    { lower: 1, upper: 1 + 2 ** -10 },
    { lower: 1, upper: 1 + 2 ** -10 },
  ],
  rg11b10ufloat: [
    { lower: 1, upper: 1 + 2 ** -6 },
    { lower: 1, upper: 1 + 2 ** -6 },
    { lower: 1, upper: 1 + 2 ** -5 },
  ],
};

/** An IEEE half float from its 16 bits. */
export function halfToNumber(bits: number): number {
  const exponent = (bits >> 10) & 0x1f;
  const mantissa = bits & 0x3ff;
  const sign = (bits & 0x8000) === 0 ? 1 : -1;
  if (exponent === 0) {
    return sign * mantissa * 2 ** -24;
  }
  if (exponent === 0x1f) {
    return mantissa === 0 ? sign * Infinity : Number.NaN;
  }
  return sign * (1 + mantissa / 1024) * 2 ** (exponent - 15);
}

/** An unsigned small float of `mantissaBits` and a 5-bit exponent, as `rg11b10ufloat` packs. */
function smallFloat(bits: number, mantissaBits: number): number {
  const exponent = bits >> mantissaBits;
  const mantissa = bits & ((1 << mantissaBits) - 1);
  if (exponent === 0) {
    return mantissa * 2 ** (-14 - mantissaBits);
  }
  if (exponent === 0x1f) {
    return mantissa === 0 ? Infinity : Number.NaN;
  }
  return (1 + mantissa / (1 << mantissaBits)) * 2 ** (exponent - 15);
}

/** The red, green and blue of an `rg11b10ufloat` texel's 32 bits. */
export function unpackRg11b10(bits: number): readonly [number, number, number] {
  return [
    smallFloat(bits & 0x7ff, 6),
    smallFloat((bits >>> 11) & 0x7ff, 6),
    smallFloat(bits >>> 22, 5),
  ];
}

/** The first texel's red, green and blue from a read-back of `format`. */
export function firstTexel(
  format: ProbedTargetFormat,
  bytes: ArrayBuffer,
): readonly [number, number, number] {
  if (format === "rgba16float") {
    const halves = new Uint16Array(bytes, 0, 3);
    return [
      halfToNumber(halves[0] ?? 0),
      halfToNumber(halves[1] ?? 0),
      halfToNumber(halves[2] ?? 0),
    ];
  }
  return unpackRg11b10(new Uint32Array(bytes, 0, 1)[0] ?? 0);
}

/**
 * How `format` rounds, from the first texel read back after the probe wrote its values.
 *
 * @returns `nearest` when every channel holds the upper neighbour, `toward-zero` when every one
 * holds the lower, `unknown` otherwise.
 */
export function classifyRounding(
  format: ProbedTargetFormat,
  texel: readonly [number, number, number],
): TargetRounding {
  const neighbours = NEIGHBOURS[format];
  const all = (pick: "lower" | "upper"): boolean =>
    texel.every((value, channel) => value === neighbours[channel]?.[pick]);
  if (all("upper")) {
    return "nearest";
  }
  return all("lower") ? "toward-zero" : "unknown";
}

/** The formats the probe draws into on a device. */
export function probedFormats(capabilities: GpuCapabilities): ReadonlyArray<ProbedTargetFormat> {
  return capabilities.rg11b10Renderable ? ["rgba16float", "rg11b10ufloat"] : ["rgba16float"];
}

/** The probe's shaders: a full-screen triangle writing one constant colour. */
export function probeWgsl(format: ProbedTargetFormat): string {
  const [red, green, blue] = PROBE_VALUES[format];
  return `
@vertex fn vertexMain(@builtin(vertex_index) index : u32) -> @builtin(position) vec4f {
  let corner = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
  return vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment fn fragmentMain() -> @location(0) vec4f {
  return vec4f(${red}, ${green}, ${blue}, 1.0);
}
`;
}

/**
 * Probes every format the device can render, reading each through `readFormat`.
 *
 * @param readFormat - Draws the probe into a target of the format and reads it back.
 * @returns Each format's rounding; a format not probed, or whose probe failed, is `unknown`, and a
 * failure is no fault.
 */
export async function probeTargetRounding(
  formats: ReadonlyArray<ProbedTargetFormat>,
  readFormat: (format: ProbedTargetFormat) => Promise<ArrayBuffer>,
): Promise<Readonly<Record<ProbedTargetFormat, TargetRounding>>> {
  const rounding: Record<ProbedTargetFormat, TargetRounding> = {
    rgba16float: "unknown",
    rg11b10ufloat: "unknown",
  };
  for (const format of formats) {
    try {
      // One small read a format, in turn, off the frame path.
      // oxlint-disable-next-line no-await-in-loop
      rounding[format] = classifyRounding(format, firstTexel(format, await readFormat(format)));
    } catch (error: unknown) {
      console.warn(`the ${format} rounding probe failed, so its rounding is unknown:`, error);
    }
  }
  return rounding;
}

/** What the probe needs of the engine: its device and its one creation, submission and read path. */
export interface ProbeHost {
  readonly device: GPUDevice;
  createTexture(spec: TextureSpec): TextureHandle;
  gpuTextureOf(handle: TextureHandle): GPUTexture;
  /** Submits the encoded work after what the engine has recorded so far. */
  submit(label: string, encode: (encoder: GPUCommandEncoder) => void): void;
  readTexture(handle: TextureHandle): Promise<ArrayBuffer>;
  destroyTexture(handle: TextureHandle): void;
}

/** The probe's 4 × 1 target of `format`, made through the engine's one creation path. */
export function probeTargetSpec(format: ProbedTargetFormat): TextureSpec {
  return {
    name: `rounding probe ${format}`,
    size: { width: PROBE_WIDTH_PX, height: 1 },
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.COPY_SRC,
    category: "other",
  };
}

/**
 * Draws the probe into a fresh target of `format` on the host's device, reads it back and
 * destroys the target.
 *
 * @remarks
 * A raw pipeline of the adapter's own, encoded like Design note 19's raw passes: the probe needs
 * no mesh or material of Babylon's.
 */
export async function drawAndReadProbe(
  host: ProbeHost,
  format: ProbedTargetFormat,
): Promise<ArrayBuffer> {
  const module = host.device.createShaderModule({
    label: `rounding probe ${format}`,
    code: probeWgsl(format),
  });
  const pipeline = await host.device.createRenderPipelineAsync({
    label: `rounding probe ${format}`,
    layout: "auto",
    vertex: { module, entryPoint: "vertexMain" },
    fragment: { module, entryPoint: "fragmentMain", targets: [{ format }] },
    primitive: { topology: "triangle-list" },
  });
  const target = host.createTexture(probeTargetSpec(format));
  try {
    const view = host.gpuTextureOf(target).createView();
    host.submit(`rounding probe ${format}`, (encoder) => {
      const pass = encoder.beginRenderPass({
        label: `rounding probe ${format}`,
        colorAttachments: [
          { view, loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 0 } },
        ],
      });
      pass.setPipeline(pipeline);
      pass.draw(3);
      pass.end();
    });
    return await host.readTexture(target);
  } finally {
    host.destroyTexture(target);
  }
}
