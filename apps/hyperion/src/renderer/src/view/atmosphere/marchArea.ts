/**
 * The ray march's area of a frame (plan R08, R08.T11.d; `decision-r08-t11-frame.md` §2): the tiles
 * of the march's target that the air's top shell can reach, so that the march is dispatched over
 * them alone and the composite reads no air outside them.
 *
 * @remarks
 * The area is exact. A texel outside it marches a ray that misses the top shell, the quadric
 * (a + H ÷ s_e, c + H ÷ s_p) the march clips its air by, and its far-limb footprint
 * (`farLimb.wgsl`) cannot blend, so the march would write it (0, 0, 0, 1) and the composite's
 * transmittance from above is 1 there. That holds by three margins over the shell's exact
 * projection:
 * - the shell is widened by {@link MARCH_AREA_WIDENING} of the camera's distance on each
 *   semi-axis, against the kernels' `f32` rounding of the tangent heights and of the quadric's
 *   discriminant, about 10⁻⁷ of the distance (`rayMarch.wgsl`'s Lagrange form beyond h0 = a ÷ 2,
 *   so that no difference of squares of the distance is formed), and their shell's radii in `f32`;
 * - the projection's bounds are taken exactly, in `f64`, from the widened shell's dual quadric:
 *   a plane through the camera is tangent to the ellipsoid of semi-axes (A, A, C) centred at m
 *   from the camera iff nᵀ diag(A², A², C²) n = (n · m)², so the planes holding the view's up
 *   (or right) axis and tangent to it give the forward rays' bounds along the other, the
 *   ellipsoid's shadow along that axis seen from the camera (`forwardSlopes`), even where the
 *   shell reaches behind the camera's plane, as from orbit over the limb;
 * - {@link MARCH_AREA_MARGIN_TEXELS} more on each side, over a footprint's reach (its nodes lie
 *   within 0.71 of a texel of its centre, its span's rays within half of one) and low's
 *   half-resolution read (the composite's four candidates of a pixel, half a texel apart).
 *
 * The area is then held to whole tiles of {@link MARCH_TILE_TEXELS} texels on the target's own grid,
 * the march's workgroups, which the tile cull classifies (`rayMarchCentre.wgsl`). A camera inside
 * the shell's shadow along both axes, inside the shell among those, takes the whole target, and a
 * shell no forward ray of the field meets none.
 */

import { dot, norm, type Vec3 } from "../../geometry/vec3";
import { rotate } from "../camera/quaternion";
import type { AtmosphereCamera } from "./hillaire";

/** The side of a tile of the ray march's target, texels: its workgroups' 8 × 8 (R08.T11.d). */
export const MARCH_TILE_TEXELS = 8;

/**
 * The texels added on each side of the shell's projection (R08.T11.d): over a footprint's 0.71 of
 * a texel and low's half-resolution candidates, half a texel each way.
 */
export const MARCH_AREA_MARGIN_TEXELS = 2;

/**
 * The widening of the shell's semi-axes, and of the tile cull's bounds, as a share of the camera's
 * distance from the centre (or of the shell's radius, where larger) (R08.T11.d): about 160 times
 * the kernels' `f32` rounding of a tangent height, a few times 2⁻²⁴ of the distance.
 */
export const MARCH_AREA_WIDENING = 1e-5;

/** The air's top shell, m: the quadric (a + H ÷ s_e, c + H ÷ s_p) the ray march clips its air by. */
export interface MarchShell {
  readonly equatorialM: number;
  readonly polarM: number;
}

/**
 * The tiles of a frame's ray-march target the march is dispatched over (R08.T11.d), on the target's
 * own grid of {@link MARCH_TILE_TEXELS}: the first tile, and how many across and down; none where
 * either count is 0.
 */
export interface MarchArea {
  readonly firstTileX: number;
  readonly firstTileY: number;
  readonly tilesAcross: number;
  readonly tilesDown: number;
  /** The target's size at the setting's scale, texels: ⌈viewport × scale⌉. */
  readonly widthTexels: number;
  readonly heightTexels: number;
  /** The widening of the shell and of the tile cull's bounds, m ({@link MARCH_AREA_WIDENING}). */
  readonly widenM: number;
  /** Whether it is the whole target. */
  readonly whole: boolean;
}

/** The texels a {@link MarchArea} covers, [x0, x1) × [y0, y1), held to the target. */
export interface MarchAreaTexels {
  readonly x0: number;
  readonly y0: number;
  readonly x1: number;
  readonly y1: number;
}

/** The texels `area` covers, its last tiles cut at the target's edge. */
export function marchAreaTexels(area: MarchArea): MarchAreaTexels {
  const x0 = area.firstTileX * MARCH_TILE_TEXELS;
  const y0 = area.firstTileY * MARCH_TILE_TEXELS;
  return {
    x0,
    y0,
    x1: Math.min(x0 + area.tilesAcross * MARCH_TILE_TEXELS, area.widthTexels),
    y1: Math.min(y0 + area.tilesDown * MARCH_TILE_TEXELS, area.heightTexels),
  };
}

/** The whole of a target of `widthTexels` × `heightTexels`, as an area. */
function wholeArea(widthTexels: number, heightTexels: number, widenM: number): MarchArea {
  return {
    firstTileX: 0,
    firstTileY: 0,
    tilesAcross: Math.ceil(widthTexels / MARCH_TILE_TEXELS),
    tilesDown: Math.ceil(heightTexels / MARCH_TILE_TEXELS),
    widthTexels,
    heightTexels,
    widenM,
    whole: true,
  };
}

/** None of a target, as an area. */
function noArea(widthTexels: number, heightTexels: number, widenM: number): MarchArea {
  return {
    firstTileX: 0,
    firstTileY: 0,
    tilesAcross: 0,
    tilesDown: 0,
    widthTexels,
    heightTexels,
    widenM,
    whole: false,
  };
}

/** An angle taken to (−π, π]. */
function wrapped(angleRad: number): number {
  const turns = Math.round(angleRad / (2 * Math.PI));
  const angle = angleRad - turns * 2 * Math.PI;
  return angle <= -Math.PI ? angle + 2 * Math.PI : angle;
}

/**
 * The forward rays' slopes along `axis`, tan ψ for the ray forward cos ψ + axis sin ψ, that meet
 * the ellipsoid, ψ within a hair of the half turn about forward: the ellipsoid seen along the
 * view's other axis, the ellipse of its shadow on the plane of `axis` and `forward` (both unit and
 * perpendicular), whose dual shape there is `shape`'s restriction, M₂ = [[M_aa, M_af], [M_af,
 * M_ff]], about its centre m₂ = (m · axis, m · forward) from the camera. A ray of the frame meets
 * the ellipsoid only if its shadow meets the shadow's ellipse, and every shadow ray that meets it
 * forward is a shadow of one that does. `whole` where the camera's own shadow lies in the ellipse,
 * every direction meeting it; `null` where no forward ray does. The bounds are not held to the
 * field here: a texel at the field's edge has rays beyond it (its footprint's nodes reach about a
 * tenth of a texel past the edge), so the caller adds its margin before the target's edge holds
 * them (R08.T11.d's review: a limb a tenth of a texel outside the field from 10⁹ m blended there).
 *
 * @remarks
 * The ellipse's tangents through the camera are the directions ψ whose normal n = (cos ψ,
 * −sin ψ) has (n · m₂)² = nᵀ M₂ n: with C_aa = m_a² − M_aa, C_ff = m_f² − M_ff and
 * C_af = m_a m_f − M_af, P + Q cos 2ψ + R sin 2ψ = 0 for P = (C_aa + C_ff) ÷ 2,
 * Q = (C_aa − C_ff) ÷ 2 and R = −C_af, so 2ψ = atan2(R, Q) ± acos(−P ÷ hypot(Q, R)). Each
 * tangent touches on the ray of its line whose tangent point s* = tᵀ M₂⁻¹ m₂ ÷ tᵀ M₂⁻¹ t lies
 * ahead, and the ellipse's rays are the arc between the two that holds its centre's.
 */
function forwardSlopes(
  axis: Vec3,
  forward: Vec3,
  centre: Vec3,
  shape: (u: Vec3, v: Vec3) => number,
): "whole" | readonly [number, number] | null {
  const ma = dot(axis, centre);
  const mf = dot(forward, centre);
  const maa = shape(axis, axis);
  const maf = shape(axis, forward);
  const mff = shape(forward, forward);
  // M₂⁻¹ = [[M_ff, −M_af], [−M_af, M_aa]] ÷ det.
  const det = maa * mff - maf * maf;
  const inverse = (ta: number, tf: number, ua: number, uf: number): number =>
    (ta * (mff * ua - maf * uf) + tf * (maa * uf - maf * ua)) / det;
  if (!(inverse(ma, mf, ma, mf) > 1)) {
    return "whole";
  }
  const caa = ma * ma - maa;
  const cff = mf * mf - mff;
  const caf = ma * mf - maf;
  const p = (caa + cff) / 2;
  const q = (caa - cff) / 2;
  const r = -caf;
  const s = Math.hypot(q, r);
  if (!(s > 0) || !(Math.abs(p) <= s)) {
    return "whole";
  }
  const beta = Math.atan2(r, q);
  const spread = Math.acos(-p / s);
  const centreRad = Math.atan2(ma, mf);
  const offsets = [(beta - spread) / 2, (beta + spread) / 2].map((lineRad) => {
    const ta = Math.sin(lineRad);
    const tf = Math.cos(lineRad);
    const ahead = inverse(ta, tf, ma, mf) / inverse(ta, tf, ta, tf) > 0;
    return wrapped((ahead ? lineRad : lineRad + Math.PI) - centreRad);
  });
  const low = centreRad + Math.min(...offsets);
  const high = centreRad + Math.max(...offsets);
  // The arc, under a half turn, against the forward half turn, a hair short of it so that the
  // slopes stay finite: at most one of its turns meets it.
  const forwardRad = (Math.PI / 2) * (1 - 1e-9);
  for (const turn of [-2 * Math.PI, 0, 2 * Math.PI]) {
    const from = Math.max(low + turn, -forwardRad);
    const to = Math.min(high + turn, forwardRad);
    if (to >= from) {
      return [Math.tan(from), Math.tan(to)];
    }
  }
  return null;
}

/**
 * A frame's ray-march area (R08.T11.d): the tiles of its target, at the setting's `scale` of the
 * view's size, that the air's top `shell`, widened, can reach, with {@link MARCH_AREA_MARGIN_TEXELS}
 * on each side ({@link MarchArea}).
 *
 * @param view - The atmosphere's camera, body-fixed, as the kernels take it (`hillaire.ts`'s view
 *   uniform: right × tan(fov_x ÷ 2), up × tan(fov_y ÷ 2), forward).
 * @param scale - The ray march's scale of the output, 1 or 0.5 (`TableSizes.rayMarchScale`).
 */
export function marchAreaOf(view: AtmosphereCamera, shell: MarchShell, scale: number): MarchArea {
  const widthTexels = Math.max(1, Math.ceil(view.viewport.widthPx * scale));
  const heightTexels = Math.max(1, Math.ceil(view.viewport.heightPx * scale));
  const p = view.positionM;
  const widenM = MARCH_AREA_WIDENING * Math.max(norm(p), shell.equatorialM, shell.polarM);
  const a = shell.equatorialM + widenM;
  const c = shell.polarM + widenM;
  // The ellipsoid's centre from the camera, and its dual quadric's shape diag(A², A², C²).
  const centre = { x: -p.x, y: -p.y, z: -p.z };
  const shape = (u: Vec3, v: Vec3): number => a * a * (u.x * v.x + u.y * v.y) + c * c * u.z * v.z;
  const forward = rotate(view.orientation, { x: 0, y: 0, z: -1 });
  const tanX = Math.tan(view.fovXRad / 2);
  const tanY = tanX / (view.viewport.widthPx / view.viewport.heightPx);
  const across = forwardSlopes(
    rotate(view.orientation, { x: 1, y: 0, z: 0 }),
    forward,
    centre,
    shape,
  );
  const down = forwardSlopes(
    rotate(view.orientation, { x: 0, y: 1, z: 0 }),
    forward,
    centre,
    shape,
  );
  if (across === null || down === null) {
    return noArea(widthTexels, heightTexels, widenM);
  }
  if (across === "whole" && down === "whole") {
    return wholeArea(widthTexels, heightTexels, widenM);
  }
  // The texels' coordinates of the bounds: x = (ndc_x + 1) ÷ 2 × width, y = (1 − ndc_y) ÷ 2 ×
  // height, ndc the slope over the half-field's tangent; then the margin, the target's edges and
  // the tiles.
  const margin = MARCH_AREA_MARGIN_TEXELS;
  const [xLow, xHigh] = across === "whole" ? [-tanX, tanX] : across;
  const [yLow, yHigh] = down === "whole" ? [-tanY, tanY] : down;
  const x0 = Math.max(0, Math.floor(((xLow / tanX + 1) / 2) * widthTexels - margin));
  const x1 = Math.min(widthTexels, Math.ceil(((xHigh / tanX + 1) / 2) * widthTexels + margin));
  const y0 = Math.max(0, Math.floor(((1 - yHigh / tanY) / 2) * heightTexels - margin));
  const y1 = Math.min(heightTexels, Math.ceil(((1 - yLow / tanY) / 2) * heightTexels + margin));
  if (!(x1 > x0) || !(y1 > y0)) {
    return noArea(widthTexels, heightTexels, widenM);
  }
  const firstTileX = Math.floor(x0 / MARCH_TILE_TEXELS);
  const firstTileY = Math.floor(y0 / MARCH_TILE_TEXELS);
  const tilesAcross = Math.ceil(x1 / MARCH_TILE_TEXELS) - firstTileX;
  const tilesDown = Math.ceil(y1 / MARCH_TILE_TEXELS) - firstTileY;
  const whole = wholeArea(widthTexels, heightTexels, widenM);
  return {
    firstTileX,
    firstTileY,
    tilesAcross,
    tilesDown,
    widthTexels,
    heightTexels,
    widenM,
    whole: tilesAcross === whole.tilesAcross && tilesDown === whole.tilesDown,
  };
}

/**
 * The area of every tile of the target (R08.T11.d's reference path): the march over the whole
 * frame, as before the area.
 */
export function wholeMarchArea(
  view: AtmosphereCamera,
  shell: MarchShell,
  scale: number,
): MarchArea {
  const area = marchAreaOf(view, shell, scale);
  return wholeArea(area.widthTexels, area.heightTexels, area.widenM);
}

/**
 * A tile's kind under the tile cull (R08.T11.d; `marchArea.wgsl`'s `MARCH_TILE_*`): every ray of
 * its texels misses the top shell and no footprint can blend (`miss`, written (0, 0, 0, 1)); no
 * footprint can blend (`centre`, its centre rays marched); or a footprint may (`footprint`).
 */
export type MarchTileKind = "miss" | "centre" | "footprint";

/**
 * Each {@link MarchTileKind}'s code in the tile cull's buffer, as `marchArea.wgsl`'s
 * `MARCH_TILE_MISS`, `MARCH_TILE_CENTRE` and `MARCH_TILE_FOOTPRINT` write it (R08.T11.d).
 */
export const MARCH_TILE_CODES: Readonly<Record<MarchTileKind, number>> = {
  miss: 0,
  centre: 1,
  footprint: 2,
};

/**
 * The texels about a tile's centre its texels' rays reach, per axis (`rayMarchCentre.wgsl`'s
 * `MARCH_TILE_REACH_TEXELS`): their centres within 3.5, their footprints' nodes within 0.71 of a
 * texel of those, and their spans' rays at their edges' midpoints within 0.5.
 */
export const MARCH_TILE_REACH_TEXELS = 4.5;

/** The camera and its ray basis as the tile cull takes them: `TwinView`'s, body-fixed, m. */
export interface MarchTileView {
  readonly cameraM: Vec3;
  /** Right × tan(fov_x ÷ 2), up × tan(fov_y ÷ 2), and forward, unit. */
  readonly right: Vec3;
  readonly upRay: Vec3;
  readonly forward: Vec3;
  /** Whether the camera's h\* is at or above the medium's top: the footprint's own test. */
  readonly above: boolean;
}

/** The datum's and the top shell's radii the tile cull takes, m. */
export interface MarchTileShells {
  readonly groundEquatorialM: number;
  readonly groundPolarM: number;
  readonly topEquatorialM: number;
  readonly topPolarM: number;
}

/** The tangent height over the spheroid (a, a, c) of the line from `o` along unit `d`, m. */
function tangentHeightM(o: Vec3, d: Vec3, a: number, c: number): number {
  const k = a / c;
  const oz = o.z * k;
  const dz = d.z * k;
  const cx = o.y * dz - oz * d.y;
  const cy = oz * d.x - o.x * dz;
  const cz = o.x * d.y - o.y * d.x;
  const qa = d.x * d.x + d.y * d.y + dz * dz;
  return Math.sqrt((cx * cx + cy * cy + cz * cz) / qa) - a;
}

/**
 * The kind of the tile (`tileX`, `tileY`) of `area`'s target under the tile cull, as
 * `rayMarchCentre.wgsl`'s `marchTileKindOf` forms it in `f32`, here in `f64` (R08.T11.d): the
 * datum's and the top's tangent heights of the line through the tile's centre, and the bound
 * |o| k² 2E (|right| ÷ width + |upRay| ÷ height), widened by 1% and by the area's widening, on how
 * far any of its texels' rays, footprints and spans reach from them.
 */
export function marchTileKindTwin(
  view: MarchTileView,
  shells: MarchTileShells,
  area: MarchArea,
  tileX: number,
  tileY: number,
): MarchTileKind {
  const half = MARCH_TILE_TEXELS / 2;
  const nx = ((tileX * MARCH_TILE_TEXELS + half) / area.widthTexels) * 2 - 1;
  const ny = 1 - ((tileY * MARCH_TILE_TEXELS + half) / area.heightTexels) * 2;
  const { right: r, upRay: u, forward: f } = view;
  const w = {
    x: f.x + nx * r.x + ny * u.x,
    y: f.y + nx * r.y + ny * u.y,
    z: f.z + nx * r.z + ny * u.z,
  };
  const length = norm(w);
  const d = { x: w.x / length, y: w.y / length, z: w.z / length };
  const o = view.cameraM;
  const a = shells.groundEquatorialM;
  const c = shells.groundPolarM;
  const topA = shells.topEquatorialM;
  const topC = shells.topPolarM;
  const groundM = tangentHeightM(o, d, a, c);
  const topHeightM = tangentHeightM(o, d, topA, topC);
  const k = Math.max(a / c, c / a, topA / topC, topC / topA);
  const reachM =
    norm(o) *
    k *
    k *
    2 *
    MARCH_TILE_REACH_TEXELS *
    (norm(r) / area.widthTexels + norm(u) / area.heightTexels);
  const boundM = 1.01 * reachM + area.widenM;
  if (topHeightM - boundM > 0) {
    return "miss";
  }
  if (view.above && groundM + boundM > 0) {
    return "footprint";
  }
  return "centre";
}
