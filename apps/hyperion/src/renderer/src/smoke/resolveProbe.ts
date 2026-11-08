/**
 * Plan R14, R14.T2.d's barycentric twin on the device: the resolve's place of each pixel, from its
 * triangle's three refetched vertices and its barycentrics, against the place the rasteriser
 * interpolates there for the lit cluster form, axis by axis, as `f32` bits.
 *
 * @remarks
 * Two probe forms write one axis of the lit fragment's `fromCameraBf` (the vertex less the camera,
 * body-fixed, metres) as the bits of an `r32uint`: the lit cluster form's fragment stage replaced,
 * drawn with the frame's opaque buckets into an ID target with its own depth, so that each pixel
 * holds its nearest fragment's interpolant; and the resolve's, its shading replaced, drawn over the
 * IDs the raster wrote. Both draw the same triangles to the bit (the raster's clip position is the
 * lit form's, `@invariant`), so that a pixel's two values differ only by the resolve's
 * reconstruction and the hardware's interpolation, counted in units in the last place of the
 * larger. Neither form is catalogued: they are this check's alone.
 */

import { withVisibilityIds } from "../view/clusters/resolve";
import { ID_NONE } from "../view/engine/idTarget";
import type {
  DrawItem,
  RenderEngine,
  RenderTarget,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../view/engine/types";

/** The lit cluster form's fragment stage's head, which the forward probe takes as a function. */
const LIT_FRAGMENT_HEAD = "@fragment\nfn fragmentMain(in : LitVertexOut) -> @location(0) vec4f {";

/**
 * The resolve's fragment stage's head, its shading, formed only where the pixel is kept, and its
 * return after the discard, which its probe replaces: the place's bits returned where the shading
 * stood, and 0 at the end, which a kept pixel never reaches.
 */
const RESOLVE_HEAD = "fn fragmentMain(@builtin(position) pixel : vec4f) -> @location(0) vec4f {";
const RESOLVE_SHADING =
  "colour = lit_fragment(in, at[1].fromCameraBf - at[0].fromCameraBf, at[2].fromCameraBf - at[0].fromCameraBf);";
const RESOLVE_RETURN = "  return colour;\n}";

/** `source` with `from` replaced by `to`, which it holds exactly once. */
function once(source: string, from: string, to: string): string {
  const parts = source.split(from);
  if (parts.length !== 2) {
    throw new Error(
      `a probe needs ${JSON.stringify(from)} once, found ${String(parts.length - 1)}`,
    );
  }
  return parts.join(to);
}

/** `spec` writing into `r32uint` from `source`, named for its probe of `axis`. */
function probeOf(spec: WgslMaterialSpec, source: string, axis: number): WgslMaterialSpec {
  return {
    ...spec,
    name: `${spec.name} place probe ${String(axis)}`,
    displayName: "PLACE PROBE",
    vertexWgsl: source,
    fragmentWgsl: source,
    colourWrites: true,
    blend: "none",
  };
}

/** The lit cluster form `lit` writing axis `axis` of each fragment's interpolated place's bits. */
export function forwardPlaceProbe(lit: WgslMaterialSpec, axis: number): WgslMaterialSpec {
  const source = `${once(lit.fragmentWgsl, LIT_FRAGMENT_HEAD, "fn unprobed_fragment(in : LitVertexOut) -> vec4f {")}
@fragment
fn fragmentMain(in : LitVertexOut) -> @location(0) u32 {
  return bitcast<u32>(in.fromCameraBf[${String(axis)}]);
}
`;
  return probeOf(lit, source, axis);
}

/** The resolve `resolve` writing axis `axis` of each pixel's reconstructed place's bits. */
export function resolvePlaceProbe(resolve: WgslMaterialSpec, axis: number): WgslMaterialSpec {
  const head = once(
    resolve.fragmentWgsl,
    RESOLVE_HEAD,
    "fn fragmentMain(@builtin(position) pixel : vec4f) -> @location(0) u32 {",
  );
  return probeOf(
    resolve,
    once(
      once(head, RESOLVE_SHADING, `return bitcast<u32>(in.fromCameraBf[${String(axis)}]);`),
      RESOLVE_RETURN,
      "  return 0u;\n}",
    ),
    axis,
  );
}

/** How two probes' places compare over the pixels both drew. */
export interface PlaceAgreement {
  /** The pixels both drew, an axis each. */
  readonly compared: number;
  /** The pixels one drew and the other did not. */
  readonly unmatched: number;
  /**
   * The units in the last place of the place's distance from the camera apart, an axis each (an
   * axis near 0 interpolates its corners' larger values): the median, the 99th percentile and the
   * largest.
   */
  readonly medianUlps: number;
  readonly p99Ulps: number;
  readonly largestUlps: number;
  /** The largest difference as a share of the place's distance from the camera. */
  readonly largestRelative: number;
}

/** The `f32` of bits `bits`. */
function f32Of(bits: number): number {
  return new Float32Array(Uint32Array.of(bits).buffer)[0] ?? Number.NaN;
}

/** One unit in the last place of `f32` `x`'s binade. */
export function ulpOf(x: number): number {
  const magnitude = Math.max(Math.abs(x), 2 ** -126);
  return 2 ** (Math.floor(Math.log2(magnitude)) - 23);
}

/**
 * How the rasteriser's places (`forward`, an axis a read-back of bits) and the resolve's
 * (`resolved`) agree over the pixels both drew, every axis together.
 */
export function placeAgreement(
  forward: ReadonlyArray<Uint32Array>,
  resolved: ReadonlyArray<Uint32Array>,
): PlaceAgreement {
  const ulps: number[] = [];
  let unmatched = 0;
  let largestRelative = 0;
  const pixels = forward[0]?.length ?? 0;
  for (let p = 0; p < pixels; p += 1) {
    const a = forward.map((axis) => axis[p] ?? ID_NONE);
    const b = resolved.map((axis) => axis[p] ?? ID_NONE);
    const drawnA = a.every((bits) => bits !== ID_NONE);
    const drawnB = b.every((bits) => bits !== ID_NONE);
    if (drawnA !== drawnB) {
      unmatched += 1;
      continue;
    }
    if (!drawnA) {
      continue;
    }
    const place = a.map(f32Of);
    const distance = Math.hypot(...place);
    const unit = ulpOf(distance);
    for (let axis = 0; axis < a.length; axis += 1) {
      const apart = Math.abs((place[axis] ?? Number.NaN) - f32Of(b[axis] ?? ID_NONE));
      ulps.push(apart / unit);
      largestRelative = Math.max(largestRelative, apart / Math.max(distance, Number.MIN_VALUE));
    }
  }
  ulps.sort((x, y) => x - y);
  const at = (q: number): number =>
    ulps[Math.min(ulps.length - 1, Math.floor(q * ulps.length))] ?? 0;
  return {
    compared: ulps.length,
    unmatched,
    medianUlps: at(0.5),
    p99Ulps: at(0.99),
    largestUlps: ulps.at(-1) ?? 0,
    largestRelative,
  };
}

/** What the probe draws with: the engine, the frame's matrices, its size and its draws. */
export interface PlaceProbeFrame {
  readonly engine: RenderEngine;
  readonly frame: {
    readonly viewRotation: Float32Array;
    readonly projection: Float32Array;
    readonly postProcesses: [];
  };
  readonly size: ViewSize;
  /** The frame's opaque buckets by the lit cluster form, the raster's own triangles. */
  readonly opaque: ReadonlyArray<DrawItem>;
  /** The lit cluster form's spec, of the opaque draws' material, and the resolve's. */
  readonly lit: WgslMaterialSpec;
  readonly resolve: WgslMaterialSpec;
  /** The resolve's draw, but the IDs, and the IDs the raster wrote. */
  readonly resolveDraw: DrawItem;
  readonly ids: TextureHandle;
}

/** An `r32uint` probe target of `size`, with a depth of its own where `depth`. */
function probeTarget(
  engine: RenderEngine,
  name: string,
  size: ViewSize,
  depth: boolean,
): RenderTarget {
  return engine.createRenderTarget({
    name,
    size,
    format: "r32uint",
    mips: 1,
    depth,
    category: "render-targets",
  });
}

/**
 * The two probes' places over `of`'s frame, axis by axis: the rasteriser's by the forward probe, the
 * resolve's over the raster's IDs ({@link placeAgreement}).
 */
export async function probePlaces(of: PlaceProbeFrame): Promise<PlaceAgreement> {
  const { engine } = of;
  const axes = [0, 1, 2] as const;
  const litMesh = of.opaque[0]?.mesh;
  if (litMesh === undefined) {
    throw new Error("the frame draws no opaque bucket to probe");
  }
  const targets: RenderTarget[] = [];
  const settled = await Promise.allSettled(
    axes.flatMap((axis) => [
      engine.createMaterialAsync(forwardPlaceProbe(of.lit, axis), ["r32uint"], [litMesh]),
      engine.createMaterialAsync(
        resolvePlaceProbe(of.resolve, axis),
        ["r32uint"],
        [of.resolveDraw.mesh],
      ),
    ]),
  );
  const made = settled.flatMap((each) => (each.status === "fulfilled" ? [each.value] : []));
  try {
    const refused = settled.find((each) => each.status === "rejected");
    if (refused !== undefined) {
      throw new Error("a place probe was refused", { cause: refused.reason });
    }
    const reads = axes.map((axis) => {
      const litProbe = made[2 * axis];
      const resolveProbe = made[2 * axis + 1];
      if (litProbe === undefined || resolveProbe === undefined) {
        throw new Error(`axis ${String(axis)}'s probes were not made`);
      }
      const rasterised = probeTarget(engine, `place probe ${String(axis)} forward`, of.size, true);
      const reconstructed = probeTarget(
        engine,
        `place probe ${String(axis)} resolve`,
        of.size,
        false,
      );
      targets.push(rasterised, reconstructed);
      rasterised.render({
        ...of.frame,
        label: "place probe forward",
        draws: of.opaque.map((draw) => ({ ...draw, material: litProbe })),
      });
      reconstructed.render({
        ...of.frame,
        label: "place probe resolve",
        draws: [withVisibilityIds({ ...of.resolveDraw, material: resolveProbe }, of.ids)],
      });
      return Promise.all([
        engine.readTexture(rasterised.colour),
        engine.readTexture(reconstructed.colour),
      ]);
    });
    const read = await Promise.all(reads);
    return placeAgreement(
      read.map(([a]) => new Uint32Array(a)),
      read.map(([, b]) => new Uint32Array(b)),
    );
  } finally {
    for (const target of targets) {
      target.dispose();
    }
    for (const material of made) {
      engine.releaseMaterial(material);
    }
  }
}
