/**
 * The smoke page's helpers (R01.T9): the check record, the shaders and meshes its scenes share,
 * and decoders for read-back texels.
 *
 * @remarks
 * The page asserts properties of read-back frames, never compares a stored picture (the README's
 * "No golden images"). Every shader here is standard WGSL under Design note 23's convention.
 */

import type {
  DrawItem,
  FrameSubmission,
  MaterialHandle,
  MeshHandle,
  PostProcessItem,
  RenderEngine,
  WgslMaterialSpec,
} from "../view/engine/types";
import FRAME_WGSL from "../view/shaders/frame.wgsl?raw";

/** One asserted property, as the harness's main process reads it. */
export interface Check {
  readonly name: string;
  readonly pass: boolean;
  readonly detail: string;
}

/** The checks of one run, in order. */
export class Checks {
  readonly list: Check[] = [];

  /** Records one property. */
  check(name: string, pass: boolean, detail: string): void {
    this.list.push({ name, pass, detail });
  }

  /** Runs a group of checks, recording a throw as a failure of the group. */
  async group(name: string, run: () => Promise<void>): Promise<void> {
    try {
      await run();
    } catch (error: unknown) {
      const message = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
      this.check(name, false, `threw ${message}`);
    }
  }
}

/** The 4 × 4 identity, column-major. */
export const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

/** The near plane of the harness's projection, metres. */
export const NEAR_M = 0.1;

/**
 * A reversed-Z infinite perspective with a 90° vertical field, column-major: clip z is the near
 * distance and w the distance, so depth is `NEAR_M / distance`.
 */
export function projection(aspect: number): Float32Array {
  return new Float32Array([1 / aspect, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, NEAR_M, 0]);
}

/** A material's shared source: the `Frame`, a `Draw` of the offset and a tint, a flat colour. */
export const FLAT_WGSL = `${FRAME_WGSL}
struct Draw {
  offsetFromCameraM : vec3f,
  tint : vec4f,
}
@group(1) @binding(0) var<uniform> draw : Draw;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  let viewSpace = frame.viewRotation * vec4f(position + draw.offsetFromCameraM, 1.0);
  return frame.clipProjection * viewSpace;
}

@fragment fn fragmentMain() -> @location(0) vec4f {
  return draw.tint;
}
`;

/**
 * A flat material of `FLAT_WGSL` with `overrides` on its state, named `TEST FLAT` on the console
 * unless `overrides` names it.
 */
export function flatSpec(
  name: string,
  overrides: Partial<WgslMaterialSpec> = {},
): WgslMaterialSpec {
  return {
    name,
    displayName: "TEST FLAT",
    vertexWgsl: FLAT_WGSL,
    fragmentWgsl: FLAT_WGSL,
    uniforms: [{ name: "tint", type: "vec4f" }],
    samplers: [],
    cullMode: "none",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
    ...overrides,
  };
}

/** A triangle covering the whole of a view of aspect up to 5 at `zM` (negative is ahead). */
export function fullScreenMesh(engine: RenderEngine, name: string, zM = -1): MeshHandle {
  return engine.createMesh({
    name,
    positions: new Float32Array(
      [-5, -5, 1, 15, -5, 1, -5, 15, 1].map((v, i) => (i % 3 === 2 ? zM : v * -zM)),
    ),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  });
}

/** A counter-clockwise triangle in view space, at 1 m ahead unless `zM` says otherwise. */
export function triangleMesh(
  engine: RenderEngine,
  name: string,
  corners: readonly [number, number, number, number, number, number],
  zM = -1,
): MeshHandle {
  const [ax, ay, bx, by, cx, cy] = corners;
  return engine.createMesh({
    name,
    positions: new Float32Array([ax, ay, zM, bx, by, zM, cx, cy, zM]),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  });
}

/** A draw of `mesh` with `material`, tinted, at the camera's origin. */
export function drawOf(
  mesh: MeshHandle,
  material: MaterialHandle,
  tint: readonly [number, number, number, number],
  extra: Partial<DrawItem> = {},
): DrawItem {
  return {
    mesh,
    material,
    offsetFromCameraM: new Float32Array(3),
    uniforms: { tint: new Float32Array(tint) },
    textures: {},
    ...extra,
  };
}

/** A frame of `draws` under the identity rotation and the harness's projection. */
export function frameOf(
  label: string,
  draws: ReadonlyArray<DrawItem>,
  aspect = 1,
  postProcesses: ReadonlyArray<PostProcessItem> = [],
): FrameSubmission {
  return { label, viewRotation: IDENTITY, projection: projection(aspect), draws, postProcesses };
}

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

/** The texels of an `rgba16float` read-back, four numbers each. */
export function halfTexels(bytes: ArrayBuffer): Float32Array {
  return Float32Array.from(new Uint16Array(bytes), halfToNumber);
}

/** A half's place on the number line, so that neighbouring halves differ by 1 and ±0 is 0. */
function halfOrdinal(bits: number): number {
  return (bits & 0x8000) === 0 ? bits : -(bits & 0x7fff);
}

/**
 * How far apart two `rgba16float` read-backs of one size lie: the most units in the last place
 * between a texel's channel in one and in the other, every channel and alpha included (R07.T8.d's
 * G11), the channels that differ at all, and whether every alpha (a scene target's meter class) is
 * equal.
 *
 * @returns An `ulps` of +∞ where the sizes differ or a channel is NaN in either.
 */
export function halfUlpsApart(
  a: ArrayBuffer,
  b: ArrayBuffer,
): { readonly ulps: number; readonly differing: number; readonly alphasEqual: boolean } {
  const x = new Uint16Array(a);
  const y = new Uint16Array(b);
  if (x.length !== y.length) {
    return { ulps: Number.POSITIVE_INFINITY, differing: x.length, alphasEqual: false };
  }
  let ulps = 0;
  let differing = 0;
  let alphasEqual = true;
  for (let i = 0; i < x.length; i += 1) {
    const p = x[i] ?? 0;
    const q = y[i] ?? 0;
    if (Number.isNaN(halfToNumber(p)) || Number.isNaN(halfToNumber(q))) {
      return { ulps: Number.POSITIVE_INFINITY, differing: differing + 1, alphasEqual: false };
    }
    const apart = Math.abs(halfOrdinal(p) - halfOrdinal(q));
    ulps = Math.max(ulps, apart);
    differing += apart > 0 ? 1 : 0;
    alphasEqual &&= i % 4 !== 3 || apart === 0;
  }
  return { ulps, differing, alphasEqual };
}

/** The four channels of texel (x, y) of an image `widthTexels` wide. */
export function texel(
  texels: ArrayLike<number>,
  widthTexels: number,
  x: number,
  y: number,
): readonly [number, number, number, number] {
  const at = (y * widthTexels + x) * 4;
  return [
    texels[at] ?? Number.NaN,
    texels[at + 1] ?? Number.NaN,
    texels[at + 2] ?? Number.NaN,
    texels[at + 3] ?? Number.NaN,
  ];
}

/** Whether every channel of `actual` is within `tolerance` of `expected`. */
export function near(
  actual: ReadonlyArray<number>,
  expected: ReadonlyArray<number>,
  tolerance: number,
): boolean {
  return (
    actual.length === expected.length &&
    actual.every((value, index) => Math.abs(value - (expected[index] ?? Number.NaN)) <= tolerance)
  );
}

/** A list of numbers for a log line, to four significant figures. */
export function show(values: ArrayLike<number>): string {
  return Array.from(values, (value) => Number(value.toPrecision(4))).join(", ");
}

/** Resolves after `ms` milliseconds, letting the GPU process and asynchronous compiles run. */
export function pause(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

/**
 * The 16 bits of the half float at or just below a non-negative `value`: 0 for zero, a subnormal
 * below 2⁻¹⁴.
 */
export function halfBits(value: number): number {
  if (!(value > 0)) {
    return 0;
  }
  if (value < 2 ** -14) {
    return Math.floor(value / 2 ** -24);
  }
  const exponent = Math.floor(Math.log2(value));
  const mantissa = Math.floor((value / 2 ** exponent - 1) * 1024);
  return ((exponent + 15) << 10) | mantissa;
}
