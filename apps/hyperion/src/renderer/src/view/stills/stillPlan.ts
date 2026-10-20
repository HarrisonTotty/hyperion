/**
 * The geometry of a still image job (plan R11, R11.T10.a, Design note 17): the tiles a still is
 * rendered in, each tile's projection M · P and culling frustum, the view its footprints are taken
 * from, and the split that keeps a single dispatch under its bound.
 *
 * @remarks
 * Pure: no GPU, no module state, no platform API. A still of W × H device pixels is rendered in
 * tiles the adapter can hold and read back. Each tile draws through M · P, where P is R02's
 * reversed-Z infinite projection of the whole still and M maps the tile's part of the still's clip
 * space to the tile's, touching only clip x and y, so that a body lands on the same pixel of the
 * still as it would untiled and the depth is P's, bit for bit. Every screen-space footprint
 * (terrain error, mips, decoration fade, the ring profile's mip and hand-over) is taken from the
 * full still's pixel, through {@link footprintView}; only frustum culling is per tile, through
 * {@link tileFrustum}.
 *
 * Rendering the tiles, reading them back and assembling them are R11.T10.b's (`stillJob.ts`).
 */

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { type Mat4F32, NEAR_PLANE_M, perspectiveReversedInfinite } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import type { ViewSize } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import type { Frustum, FrustumCamera, Plane } from "../terrain/cull";
import type { ViewSelectionInput } from "../terrain/select";

/**
 * The largest tile side for each setting, px: 2,048 on the low setting and 4,096 on the target
 * (plan R11, Design note 17, researched 2026-09-29), chosen by memory and provisional until
 * R11.T10.d's soak shows whether 2,048² tiles fit the low setting's memory ceiling (Risks, "Stills'
 * timings").
 *
 * @remarks
 * A tile is the smaller of this and the adapter's `maxTextureDimension2D` ({@link tileLimitPx}).
 */
export const STILL_TILE_MAX_PX: Readonly<Record<QualitySetting, number>> = {
  high: 4096,
  low: 2048,
};

/**
 * The bytes a read-back texel takes: the still is read back as `rgba8unorm`, since its PNG is 8-bit
 * sRGB after R07's tone-mapping pass (Design note 17).
 */
const READBACK_BYTES_PER_TEXEL = 4;

/**
 * The alignment of `bytesPerRow` in a copy between a texture and a buffer, bytes: the WebGPU
 * specification's "validating GPUTexelCopyBufferInfo" (§11.2.2) requires a multiple of 256 for
 * `copyTextureToBuffer` and `copyBufferToTexture` (https://www.w3.org/TR/webgpu/, the Candidate
 * Recommendation Draft of 15 September 2026, re-checked 2026-10-20).
 */
const BYTES_PER_ROW_ALIGNMENT = 256;

/**
 * The step tile widths come in, px: 256 ÷ 4, so that a row of `rgba8unorm` meets
 * {@link BYTES_PER_ROW_ALIGNMENT} with no padding. A row of 8- or 16-byte texels aligns too.
 */
const TILE_WIDTH_STEP_PX = BYTES_PER_ROW_ALIGNMENT / READBACK_BYTES_PER_TEXEL;

/** A rectangle of a still's pixels, [x0Px, x1Px) × [y0Px, y1Px), y down from the top. */
export interface PixelRect {
  readonly x0Px: number;
  readonly y0Px: number;
  readonly x1Px: number;
  readonly y1Px: number;
}

/** One tile of a still: the pixels it contributes, and the texture it is drawn in and read from. */
export interface StillTile {
  /** The tile's column, from 0 at the left. */
  readonly column: number;
  /** The tile's row, from 0 at the top. */
  readonly row: number;
  /** The still's pixels this tile contributes; the tiles' rects cover the still exactly once. */
  readonly rect: PixelRect;
  /**
   * The tile's texture, px: its width a multiple of 64 and at least the rect's, its height the
   * rect's, neither side over the tile limit. Its pixel (u, v) is the still's (x0Px + u, y0Px + v);
   * the columns from the rect's width on lie beyond the still's right edge and are cropped at
   * assembly, on the CPU.
   */
  readonly texture: ViewSize;
  /** The read-back's bytes per row: 4 × the texture's width (`rgba8unorm`), a multiple of 256. */
  readonly bytesPerRow: number;
}

/** How a still is cut into tiles. */
export interface StillTiling {
  /** The still's size, px. */
  readonly size: ViewSize;
  /** L, the most px a tile's side may have: {@link tileLimitPx}. */
  readonly tileLimitPx: number;
  readonly columns: number;
  readonly rows: number;
  /** The tiles, row by row from the top left. */
  readonly tiles: ReadonlyArray<StillTile>;
}

function isPositiveInteger(n: number): boolean {
  return Number.isSafeInteger(n) && n > 0;
}

/**
 * L, the most px a tile's side may have: the smaller of the adapter's `maxTextureDimension2D` and
 * {@link STILL_TILE_MAX_PX} for the setting (Design note 17).
 *
 * @remarks
 * The adapter's limit is read, never assumed: WebGPU's default is 8,192 (the specification's Limits
 * table) and the UHD 620 gives 16,384 (Design note 17, by probe), so either way L is the setting's
 * figure today.
 *
 * @param maxTextureDimension2D - The device's `GpuCapabilities.maxTextureDimension2D`, px.
 * @throws RangeError if the limit is not a positive integer.
 */
export function tileLimitPx(maxTextureDimension2D: number, setting: QualitySetting): number {
  if (!isPositiveInteger(maxTextureDimension2D)) {
    throw new RangeError("a texture limit must be a positive integer");
  }
  return Math.min(maxTextureDimension2D, STILL_TILE_MAX_PX[setting]);
}

/** `total` split into `parts` integers as equal as possible, the larger first. */
function balancedParts(total: number, parts: number): number[] {
  const base = Math.floor(total / parts);
  const extra = total - base * parts;
  return Array.from({ length: parts }, (_, k) => (k < extra ? base + 1 : base));
}

/**
 * Cuts a still of `size` into tiles the adapter can hold (Design note 17).
 *
 * @remarks
 * The tiles form a grid of as few columns and rows as the tile limit L allows, as equal as the
 * rules let them be, so that no batch renders a sliver. Every texture width is a multiple of 64 px,
 * so that its `rgba8unorm` read-back's `bytesPerRow` (4 × the width) is a multiple of 256; a limit
 * that is not a multiple of 64 floors to the multiple below it for widths, and holds for heights as
 * it is. A still whose width is not a multiple of 64 pads its last column's texture to the next
 * multiple (under 64 px), and assembly crops it on the CPU: a narrower read-back rect would bring
 * the row padding back.
 *
 * @param size - The still's size, px, each side a positive integer.
 * @param maxTextureDimension2D - The device's `GpuCapabilities.maxTextureDimension2D`, px.
 * @throws RangeError if a side or the limit is not a positive integer, or if the limit is under
 *   64 px, which holds no aligned row.
 */
export function planTiles(
  size: ViewSize,
  maxTextureDimension2D: number,
  setting: QualitySetting,
): StillTiling {
  const { widthPx, heightPx } = size;
  if (!isPositiveInteger(widthPx) || !isPositiveInteger(heightPx)) {
    throw new RangeError("a still's sides must be positive integers");
  }
  const limitPx = tileLimitPx(maxTextureDimension2D, setting);
  const widthSteps = Math.floor(limitPx / TILE_WIDTH_STEP_PX);
  if (widthSteps < 1) {
    throw new RangeError(
      `a tile limit of ${limitPx} px is under the ${TILE_WIDTH_STEP_PX} px step`,
    );
  }
  // Widths in steps of 64 px: the last column's texture reaches past the still by under one step.
  const stillSteps = Math.ceil(widthPx / TILE_WIDTH_STEP_PX);
  const columns = Math.ceil(stillSteps / widthSteps);
  const rows = Math.ceil(heightPx / limitPx);
  const columnSteps = balancedParts(stillSteps, columns);
  const rowHeights = balancedParts(heightPx, rows);

  const tiles: StillTile[] = [];
  let y0Px = 0;
  for (const [row, height] of rowHeights.entries()) {
    let x0Px = 0;
    for (const [column, steps] of columnSteps.entries()) {
      const textureWidthPx = steps * TILE_WIDTH_STEP_PX;
      tiles.push({
        column,
        row,
        rect: { x0Px, y0Px, x1Px: Math.min(x0Px + textureWidthPx, widthPx), y1Px: y0Px + height },
        texture: { widthPx: textureWidthPx, heightPx: height },
        bytesPerRow: textureWidthPx * READBACK_BYTES_PER_TEXEL,
      });
      x0Px += textureWidthPx;
    }
    y0Px += height;
  }
  return { size: { widthPx, heightPx }, tileLimitPx: limitPx, columns, rows, tiles };
}

/**
 * A tile's projection M · P, sixteen `f32` column-major, R01's and R02's layout (Design note 17).
 *
 * @remarks
 * P is the full still's `perspectiveReversedInfinite(fovXRad, W ÷ H, NEAR_PLANE_M)`. M maps the
 * tile's texture, the still's pixels [x₀, x₀ + w) × [y₀, y₀ + h) for a texture of w × h, onto clip
 * space's [−1, 1]², touching only clip x and y:
 *
 * - x′_clip = aₓ x_clip + bₓ w_clip, with aₓ = W ÷ w and bₓ = (W − 2x₀ − w) ÷ w;
 * - y′_clip = a_y y_clip + b_y w_clip, with a_y = H ÷ h and b_y = (2y₀ + h − H) ÷ h.
 *
 * So M · P's rows 0 and 1 are (aₓ s, 0, −bₓ, 0) and (0, a_y s a, −b_y, 0), with
 * s = 1 ÷ tan(fovX ÷ 2) and a = W ÷ H, and its rows 2 and 3 are P's, bit for bit: the reversed-Z
 * depth is untouched. The entries are formed in `f64` and rounded to `f32` once. A still in one
 * tile whose width is a multiple of 64 projects through P itself, bit for bit.
 *
 * @param fovXRad - The still's horizontal field of view, rad, in (0, π).
 * @param still - The still's size, px.
 * @param tile - One of `planTiles(still, …)`'s tiles.
 * @throws RangeError for a field of view or size {@link perspectiveReversedInfinite} refuses.
 */
export function tileProjection(fovXRad: number, still: ViewSize, tile: StillTile): Mat4F32 {
  const { widthPx, heightPx } = still;
  const aspect = widthPx / heightPx;
  // P validates the arguments and gives rows 2 and 3, which M leaves as they are.
  const m = perspectiveReversedInfinite(fovXRad, aspect, NEAR_PLANE_M);
  const s = 1 / Math.tan(fovXRad / 2);
  const tileWidthPx = tile.texture.widthPx;
  const tileHeightPx = tile.texture.heightPx;
  const x0Px = tile.rect.x0Px;
  const y0Px = tile.rect.y0Px;
  // Column-major: [0] is row 0 of column 0, [5] row 1 of column 1, [8] and [9] rows 0 and 1 of
  // column 2. −bₓ and −b_y are formed directly, so that an untiled still gives +0 there, as P has.
  m[0] = (widthPx / tileWidthPx) * s;
  m[5] = (heightPx / tileHeightPx) * (s * aspect);
  m[8] = (2 * x0Px + tileWidthPx - widthPx) / tileWidthPx;
  m[9] = (heightPx - 2 * y0Px - tileHeightPx) / tileHeightPx;
  return m;
}

/** A plane through the camera, its inward normal (x, y, z) in view axes, rotated by `camera`. */
function sidePlane(camera: FrustumCamera, x: number, y: number, z: number): Plane {
  return { normal: rotate(camera.orientation, normalise(vec3(x, y, z))), offsetM: 0 };
}

/**
 * A tile's culling frustum, in `cull.ts`'s form, so that `inFrustum` culls per tile (Design note
 * 17: "frustum culling is per tile").
 *
 * @remarks
 * The tile's rect on the still is the NDC rect [x_l, x_r] × [y_b, y_t]. Looking down −z with +y up,
 * a point is inside the side planes where s x + x_l z ≥ 0, −s x − x_r z ≥ 0, s a y + y_b z ≥ 0 and
 * −s a y − y_t z ≥ 0, with s = 1 ÷ tan(fovX ÷ 2) and a = W ÷ H, so their inward normals lie along
 * (s, 0, x_l), (−s, 0, −x_r), (0, s a, y_b) and (0, −s a, −y_t). They are normalised, since
 * `inFrustum`'s sphere test needs unit normals, and rotated into the body-fixed axes as `cull.ts`'
 * `frustumOf` does, in its order, with its near plane. With the full still's rect they are
 * `frustumOf`'s planes, to rounding. Culling against the tile's rect, not its padded texture, drops
 * only what the cropped columns alone would show.
 *
 * @param camera - The still's camera, its `viewport` the still's size (W × H), never the tile's.
 * @param tile - One of `planTiles(camera.viewport, …)`'s tiles.
 * @param marginPx - How far each side is moved outwards, px of the still, at least 0; 0, the
 *   default, is the tile's own rect. R11.T10.b's sub-pixel jitter widens it by the jitter's reach.
 */
export function tileFrustum(camera: FrustumCamera, tile: StillTile, marginPx = 0): Frustum {
  const { widthPx: w, heightPx: h } = camera.viewport;
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const sa = s * (w / h);
  const xl = (2 * (tile.rect.x0Px - marginPx)) / w - 1;
  const xr = (2 * (tile.rect.x1Px + marginPx)) / w - 1;
  const yt = 1 - (2 * (tile.rect.y0Px - marginPx)) / h;
  const yb = 1 - (2 * (tile.rect.y1Px + marginPx)) / h;
  const forward: Vec3 = rotate(camera.orientation, vec3(0, 0, -1));
  return {
    planes: [
      sidePlane(camera, s, 0, xl),
      sidePlane(camera, -s, 0, -xr),
      sidePlane(camera, 0, sa, yb),
      sidePlane(camera, 0, -sa, -yt),
      { normal: forward, offsetM: -NEAR_PLANE_M },
    ],
  };
}

/**
 * The view a still's terrain selects with, whichever tile is drawn: the still's camera, field of
 * view, weight and tolerance, with the full still's size as its viewport (Design note 17: every
 * footprint is taken "from the full still's pixel").
 *
 * @remarks
 * `selectPatches` reads the viewport as the presented size: ρ grows with its width, so a tile-sized
 * viewport would choose coarser levels than the still's pixels warrant. A still selects once with
 * this view, then culls the selection per tile with {@link tileFrustum}. Its camera is frozen, so
 * it needs no motion margin.
 *
 * @param view - The still's view; its own `viewport` is replaced.
 * @param still - The still's size, px.
 */
export function footprintView(view: ViewSelectionInput, still: ViewSize): ViewSelectionInput {
  return { ...view, viewport: { widthPx: still.widthPx, heightPx: still.heightPx } };
}

/**
 * The most GPU time a single dispatch or draw may be planned at, ns: about 50 ms, by construction
 * from the measured cost per workgroup (Design note 17, researched 2026-09-29).
 *
 * @remarks
 * The brainstorm's reasons: i915 resets the GPU when a batch cannot be preempted within 640 ms (the
 * default of `DRM_I915_PREEMPT_TIMEOUT` in the Linux kernel's
 * `drivers/gpu/drm/i915/Kconfig.profile`, confirmed on the UHD 620 laptop), Gen9 preempts only
 * between draws and between workgroups, and Windows' default `TdrDelay` is 2 s (Microsoft, "TDR
 * registry keys",
 * https://learn.microsoft.com/en-us/windows-hardware/drivers/display/tdr-registry-keys). Gen9's
 * granularity was re-checked for Linux only (2026-10-20): i915's Gen9 workarounds and Mesa's ANV
 * preempt compute between thread groups and 3D work between draws, never inside one; Intel's
 * Windows driver is not checked. It holds on any machine, so nothing keys it to a platform.
 */
export const MAX_DISPATCH_NS = 50_000_000;

/**
 * The workgroups a dispatch's first piece holds before any cost per workgroup is known: one, the
 * smallest piece, so that a probe cannot exceed the bound unless a single workgroup does.
 */
export const PROBE_WORKGROUPS = 1;

/** A run of a dispatch's workgroups, dispatched together, from `firstWorkgroup` on. */
export interface DispatchPiece {
  readonly firstWorkgroup: number;
  readonly workgroups: number;
}

/**
 * How a dispatch is split ({@link splitDispatch}):
 *
 * - `probe`: no cost is known yet; dispatch `piece` alone, measure it, and split the rest by its
 *   cost;
 * - `split`: the pieces, in order, covering every workgroup once, each predicted at no more than
 *   {@link MAX_DISPATCH_NS} (`largestPredictedNs`, the largest piece's);
 * - `over-bound`: one workgroup alone is predicted over the bound, so no split can meet it.
 */
export type DispatchSplit =
  | { readonly kind: "probe"; readonly piece: DispatchPiece }
  | {
      readonly kind: "split";
      readonly pieces: ReadonlyArray<DispatchPiece>;
      readonly largestPredictedNs: number;
    }
  | { readonly kind: "over-bound"; readonly workgroupNs: number };

/**
 * Splits a dispatch of `workgroups` into pieces each predicted at no more than
 * {@link MAX_DISPATCH_NS} at the measured cost per workgroup (Design note 17).
 *
 * @remarks
 * The pieces are as few as the bound allows and as equal as it lets them be, the larger first.
 * Without a cost the first piece is a probe of {@link PROBE_WORKGROUPS}; the caller measures it and
 * splits the remaining workgroups again with the cost it gives. The cost must bound the true cost:
 * under a quantised timer, `passTimeBoundNs` of the measured time divided by the workgroups
 * measured (`batchController.ts`). A cost measured on a small piece over-states a large one's,
 * since a GPU runs many workgroups at once, so later splits are coarser. The same split serves a
 * draw's instances.
 *
 * @param workgroups - The dispatch's workgroups, a positive integer.
 * @param costPerWorkgroupNs - The measured cost of one workgroup, ns, positive and finite, or
 *   `null` before any is measured.
 * @throws RangeError if `workgroups` is not a positive integer or the cost is not positive and
 *   finite.
 */
export function splitDispatch(
  workgroups: number,
  costPerWorkgroupNs: number | null,
): DispatchSplit {
  if (!isPositiveInteger(workgroups)) {
    throw new RangeError("a dispatch's workgroups must be a positive integer");
  }
  if (costPerWorkgroupNs === null) {
    return {
      kind: "probe",
      piece: { firstWorkgroup: 0, workgroups: Math.min(PROBE_WORKGROUPS, workgroups) },
    };
  }
  if (!(Number.isFinite(costPerWorkgroupNs) && costPerWorkgroupNs > 0)) {
    throw new RangeError("a cost per workgroup must be positive and finite");
  }
  let perPiece = Math.floor(MAX_DISPATCH_NS / costPerWorkgroupNs);
  // The division can round up onto an integer: keep the product within the bound.
  if (perPiece * costPerWorkgroupNs > MAX_DISPATCH_NS) {
    perPiece -= 1;
  }
  if (perPiece < 1) {
    return { kind: "over-bound", workgroupNs: costPerWorkgroupNs };
  }
  const count = Math.ceil(workgroups / perPiece);
  const sizes = balancedParts(workgroups, count);
  const pieces: DispatchPiece[] = [];
  let firstWorkgroup = 0;
  for (const size of sizes) {
    pieces.push({ firstWorkgroup, workgroups: size });
    firstWorkgroup += size;
  }
  return {
    kind: "split",
    pieces,
    largestPredictedNs: (sizes[0] ?? 0) * costPerWorkgroupNs,
  };
}
