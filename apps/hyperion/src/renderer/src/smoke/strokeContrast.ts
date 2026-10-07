/**
 * The smoke page's check of the view's strokes as drawn (plan R07, R07.T16.d and T16.g;
 * decision-thin-line-contrast, items 1 and 4; decision-r07-t16d-followups, items 2 and (b)): R02's
 * draw list, built by `buildWireframeDrawList` at the strokes of display ratios 0.78125, 1 and 2,
 * drawn by R02's renderer and read back.
 *
 * @remarks
 * At every pixel of length along each stroke, the brightest texel of its cross-section must reach
 * 6.0:1 against `--surface-0`, by WCAG's formula on the 8-bit sRGB texel the canvas would hold:
 * in the wireframe, over a `--surface-0` target, and through the photorealistic overlay over a
 * target loaded with `--text`, where each stroke stands on its casing. Four scenes hold the
 * strokes the rulings name, each on a 256 px square:
 * - a ring seen face-on from a camera rolled 0°, 3° and 45°: its 1 px `--text-muted` edges, and
 *   its radial ticks, which then lie at 0°, 3° and 45° and at every 10° on from each;
 * - a planet seen pole-on, rolled the same, its limb 110 px from the centre: its 1 px limb and
 *   meridians and its 1.5 px prime meridian, read clear of the pole and of the limb's bound;
 * - two open circle symbols in `--text`, the selection with its `--accent` reticle and the
 *   destination with its `--target` reticle, another craft's dashed `--text` path, and a third
 *   craft's `--text` target ticks;
 * - an open circle symbol that is both the selection and the destination, its `--target` reticle
 *   at the least gap outside its `--accent` one, at interface scales of 100% and 80% (T16.g).
 *
 * What another batch does to a stroke's pixels is left out of its reading (T16.g). Where another
 * batch crosses the stroke, the points about the crossing that it reaches are not read: a later
 * batch's casing cuts an earlier stroke there by design (a casing covers what lies beneath it).
 * Elsewhere only the texels another batch lights, or a later batch covers, are left out, and the
 * rest of the cross-section is read, so that a parallel neighbour's light never stands in for a
 * stroke's own and its casing's cover counts against it. Two controls must read under 6, so that
 * the check is seen to bite: the ring as built before R07.T16.d (1 device px per CSS px, outlines
 * 1.5 px) on its 1 px edges, and the pair of reticles with no least gap at 80% and a ratio of
 * 0.78125, on its bracket.
 */

import { type Vec3, vec3 } from "../geometry/vec3";
import type { ColourToken } from "../spatial/drawList";
import { type ColourTokens, readTokens } from "../spatial/paint";
import { SYMBOL_STROKE_PX } from "../spatial/symbols";
import {
  aBody,
  aViewCraft,
  aViewScene,
  FIXTURE_MOON,
  FIXTURE_PLANET,
  FIXTURE_SYSTEM,
  NO_TURN,
} from "../test/viewFixtures";
import { project, type ProjectionCamera, type Viewport } from "../view/camera/projection";
import { quaternionFromAxisAngle } from "../view/camera/quaternion";
import type { RenderEngine } from "../view/engine/types";
import { overlayDrawList } from "../view/photoreal/overlay";
import type { CraftPose, ViewBody, ViewScene } from "../view/scene/model";
import {
  buildWireframeDrawList,
  type DrawCamera,
  type DrawOptions,
  emptyDrawList,
  type LineBatch,
  type ViewStrokes,
  viewStrokesAt,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import { linearColour, WireframeRenderer } from "../view/wireframe/submit";
import { type Checks, halfTexels, texel } from "./harness";

/** The guide's least contrast for a stroke that carries meaning, against its surface. */
const REQUIRED_RATIO = 6;

/**
 * How far under its colour pair's ratio a stroke that must read its pair may read: 1%, within which
 * a stroke of 2 device px scores its pair (decision-thin-line-contrast, item 2), and which covers
 * the half float's rounding before the 8-bit code.
 */
const PAIR_TOLERANCE = 0.01;

/** The square target the checks draw into, device px. */
const SIDE_PX = 256;

/** The target's size. */
const VIEWPORT: Viewport = { widthPx: SIDE_PX, heightPx: SIDE_PX };

/** The camera's horizontal field of view: 60°, the view's default. */
const FOV_X_RAD = Math.PI / 3;

/** The focal length, px: the distance at which a metre across is a pixel, per metre away. */
const FOCAL_PX = SIDE_PX / 2 / Math.tan(FOV_X_RAD / 2);

/** How far ahead the scenes lie, m. */
const DISTANCE_M = 1e9;

/**
 * The planet's limb's radius on the target, px: 110, so that its graticule is drawn at 15° and its
 * prime meridian runs clear of the parallels crossing it for 15 to 25 px at a time.
 */
const PLANET_RADIUS_PX = 110;

/** The ring's inner and outer edges on the target, px from its centre. */
const RING_PX = { inner: 75, outer: 114 } as const;

/** The ratios the check draws at: the development machine's and the UHD 620's, 100% and 2. */
const RATIOS = [0.78125, 1, 2] as const;

/** The rolls of the ring's camera: its ticks then lie at 0°, 3° and 45°, and 10° on from each. */
const ROLLS_DEG = [0, 3, 45] as const;

/** The interface's rem at 100%, CSS px. */
const REM_CSS_PX = 16;

/**
 * The interface scale of the pair's second frame and of its control: 80%, where 0.25 rem is least
 * at a ratio of 0.78125 (2.5 device px, under the least gap's 4).
 */
const SMALL_INTERFACE = 0.8;

/** The least points a kind must read over its frames, so that no reading passes empty. */
const MIN_SAMPLES = 20;

/**
 * The least points the target's ticks must read: four ticks about 3 px long each at a ratio of
 * 0.78125.
 */
const MIN_TICK_SAMPLES = 8;

/** A dash's margin, px: a sample this near a dash's end is not read. */
const DASH_MARGIN_PX = 1;

/**
 * A cut end's margin, px, for a reading that keeps clear of its segments' ends: within a pixel of
 * one, a stroke's round cap falls off from its full coverage, so a cut end is not where it falls
 * between pixels (as a dash's ends, {@link DASH_MARGIN_PX}).
 */
const CUT_END_MARGIN_PX = 1;

/**
 * How far a stroke lights a texel beyond its half-width, px: `lines.wgsl`'s coverage,
 * clamp(w ÷ 2 + 0.5 − d, 0, 1), is above zero only at a texel centre nearer than w ÷ 2 + 0.5.
 */
const ANTIALIAS_EDGE_PX = 0.5;

/** The strokes as built before R07.T16.d: device px, the outlines 1.5 px, not moved, no least gap. */
const AS_BUILT: ViewStrokes = {
  strokeScale: 1,
  markStrokePx: SYMBOL_STROKE_PX,
  markShiftPx: 0,
  minReticleGapPx: 0,
};

/** A point on the target, px from its top left. */
export interface PointPx {
  readonly x: number;
  readonly y: number;
}

/** A batch's strokes on the target, as the check reads them. */
export interface ScreenStroke {
  /** The batch's name (`ring:…`, `mark:selection:3`, …). */
  readonly name: string;
  /** Its segments' ends on the target, px. */
  readonly segments: ReadonlyArray<readonly [PointPx, PointPx]>;
  /** The dash's phase at each segment's first end, px, as the renderer packs it. */
  readonly phases: ReadonlyArray<number>;
  /** Its width, device px. */
  readonly widthPx: number;
  /** Its casing's width on each side, device px. */
  readonly casingWidthPx: number;
  /** Its dash, device px, or `null` for a solid stroke. */
  readonly dash: LineBatch["dash"];
}

/** A target read back: its linear RGBA, four `f32` per texel, and its size. */
export interface ReadImage {
  readonly colour: Float32Array;
  readonly widthPx: number;
  readonly heightPx: number;
}

/** What a stroke reads, as drawn. */
export interface StrokeReading {
  /** The points of its length read. */
  readonly samples: number;
  /** The least contrast of its brightest texel across, against the surface; ∞ with no sample. */
  readonly worst: number;
  /** Where the least was read, or `null`. */
  readonly at: PointPx | null;
}

/**
 * Another batch of a frame as a stroke's reading takes it: its strokes, and the texels whose
 * pixels it changes for that stroke ({@link neighbourOf}).
 */
export interface Neighbour {
  /** Its strokes on the target. */
  readonly stroke: ScreenStroke;
  /** One byte a texel of the image, row by row: 1 where it lights or covers the texel. */
  readonly region: Uint8Array;
}

/** The 8-bit sRGB code a canvas stores for a linear value (IEC 61966-2-1), clamped to [0, 1]. */
export function srgb8(linear: number): number {
  const c = Math.min(Math.max(linear, 0), 1);
  const encoded = c <= 0.003_130_8 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;
  return Math.round(encoded * 255);
}

/** An 8-bit sRGB code's linear value, as WCAG 2.2 decodes it. */
function wcagLinear(code: number): number {
  const c = code / 255;
  return c <= 0.040_45 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** WCAG 2.2's relative luminance of an 8-bit sRGB colour. */
export function wcagLuminance(r8: number, g8: number, b8: number): number {
  return 0.2126 * wcagLinear(r8) + 0.7152 * wcagLinear(g8) + 0.0722 * wcagLinear(b8);
}

/** WCAG's contrast ratio between two relative luminances. */
export function contrastRatio(a: number, b: number): number {
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** A colour token's WCAG luminance, through the 8-bit code a canvas would store for it. */
export function tokenLuminance(css: string): number {
  const [r = 0, g = 0, b = 0] = linearColour(css);
  return wcagLuminance(srgb8(r), srgb8(g), srgb8(b));
}

/** The WCAG luminance of a read-back texel, through its 8-bit sRGB code. */
function texelLuminance(image: ReadImage, column: number, row: number): number {
  const [r, g, b] = texel(image.colour, image.widthPx, column, row);
  return wcagLuminance(srgb8(r), srgb8(g), srgb8(b));
}

/** The distance from `p` to the segment from `a` to `b`, px. */
function distanceToSegment(p: PointPx, a: PointPx, b: PointPx): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lengthSq = dx * dx + dy * dy;
  const t =
    lengthSq > 0 ? Math.min(1, Math.max(0, ((p.x - a.x) * dx + (p.y - a.y) * dy) / lengthSq)) : 0;
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/** The distance from `p` to the nearest of a stroke's segments, px; ∞ with none. */
function distanceToStroke(p: PointPx, stroke: ScreenStroke): number {
  let nearest = Number.POSITIVE_INFINITY;
  for (const [a, b] of stroke.segments) {
    nearest = Math.min(nearest, distanceToSegment(p, a, b));
  }
  return nearest;
}

/**
 * Another batch as a stroke's reading takes it: the texels whose centres lie within its
 * antialiased edge, where it lights them, and, where it is drawn after the stroke (`later`),
 * within its casing's antialiased edge, where it covers them. Its dash's gaps are counted as lit,
 * which can only leave more out.
 */
export function neighbourOf(
  stroke: ScreenStroke,
  later: boolean,
  widthPx: number,
  heightPx: number,
): Neighbour {
  const region = new Uint8Array(widthPx * heightPx);
  const reachPx = stroke.widthPx / 2 + (later ? stroke.casingWidthPx : 0) + ANTIALIAS_EDGE_PX;
  for (const [a, b] of stroke.segments) {
    const left = Math.max(0, Math.floor(Math.min(a.x, b.x) - reachPx));
    const right = Math.min(widthPx - 1, Math.ceil(Math.max(a.x, b.x) + reachPx));
    const top = Math.max(0, Math.floor(Math.min(a.y, b.y) - reachPx));
    const bottom = Math.min(heightPx - 1, Math.ceil(Math.max(a.y, b.y) + reachPx));
    for (let row = top; row <= bottom; row += 1) {
      for (let column = left; column <= right; column += 1) {
        const at = row * widthPx + column;
        if (
          region[at] === 0 &&
          distanceToSegment({ x: column + 0.5, y: row + 0.5 }, a, b) < reachPx
        ) {
          region[at] = 1;
        }
      }
    }
  }
  return { stroke, region };
}

/** A point of a stroke's length, with its cross-section and whether it joins the point before. */
interface SectionPoint {
  readonly p: PointPx;
  /** The texels of its cross-section, or `null` where it reaches off the image. */
  readonly section: ReadonlyArray<readonly [number, number]> | null;
  /** Whether it is read: on a dash away from its ends, and kept. */
  readonly read: boolean;
  /** Whether it runs on from the point before it, along one segment or across a joint. */
  readonly joined: boolean;
}

/**
 * The points of a stroke's length, a pixel apart along each segment, with their cross-sections;
 * those within `endMarginPx` of a segment's end are not read.
 */
function sectionPoints(
  image: ReadImage,
  stroke: ScreenStroke,
  keep: ((p: PointPx) => boolean) | null,
  endMarginPx: number,
): SectionPoint[] {
  const halfPx = stroke.widthPx / 2 + ANTIALIAS_EDGE_PX;
  const points: SectionPoint[] = [];
  let lastEnd: PointPx | null = null;
  stroke.segments.forEach(([a, b], index) => {
    const lengthPx = Math.hypot(b.x - a.x, b.y - a.y);
    if (!(lengthPx > 0)) {
      return;
    }
    const continues = lastEnd !== null && lastEnd.x === a.x && lastEnd.y === a.y;
    lastEnd = b;
    const u = { x: (b.x - a.x) / lengthPx, y: (b.y - a.y) / lengthPx };
    const steps = Math.max(1, Math.floor(lengthPx));
    for (let i = 0; i <= steps; i += 1) {
      const s = (lengthPx * i) / steps;
      const p = { x: a.x + u.x * s, y: a.y + u.y * s };
      let onDash = true;
      if (stroke.dash !== null) {
        const period = stroke.dash.onPx + stroke.dash.offPx;
        const phase = ((((stroke.phases[index] ?? 0) + s) % period) + period) % period;
        onDash = phase >= DASH_MARGIN_PX && phase <= stroke.dash.onPx - DASH_MARGIN_PX;
      }
      points.push({
        p,
        section: crossSection(p, u, halfPx, image),
        read:
          onDash && s >= endMarginPx && s <= lengthPx - endMarginPx && (keep === null || keep(p)),
        joined: i > 0 || continues,
      });
    }
  });
  return points;
}

/** Whether a neighbour lights or covers any texel of a cross-section. */
function touches(
  neighbour: Neighbour,
  section: ReadonlyArray<readonly [number, number]> | null,
  widthPx: number,
): boolean {
  return (
    section !== null &&
    section.some(([column, row]) => neighbour.region[row * widthPx + column] === 1)
  );
}

/**
 * Which of a stroke's points lie at a crossing with a neighbour: from each point where the
 * neighbour's centreline comes within the stroke's half-width of the stroke's (it crosses it, or
 * ends on it), the run of joined points on either side whose cross-sections the neighbour touches.
 */
function crossingPoints(
  points: ReadonlyArray<SectionPoint>,
  stroke: ScreenStroke,
  neighbour: Neighbour,
  widthPx: number,
): Uint8Array {
  const touched = points.map((point) => touches(neighbour, point.section, widthPx));
  const crossing = new Uint8Array(points.length);
  points.forEach((point, i) => {
    if (
      crossing[i] === 1 ||
      touched[i] !== true ||
      !(distanceToStroke(point.p, neighbour.stroke) < stroke.widthPx / 2)
    ) {
      return;
    }
    crossing[i] = 1;
    for (
      let j = i + 1;
      j < points.length && points[j]?.joined === true && touched[j] === true;
      j += 1
    ) {
      crossing[j] = 1;
    }
    for (let j = i; j > 0 && points[j]?.joined === true && touched[j - 1] === true; j -= 1) {
      crossing[j - 1] = 1;
    }
  });
  return crossing;
}

/**
 * A stroke as drawn (R07.T16.g): at every pixel of its length (on a dash, away from its ends; and
 * where `keep` holds, if given), the brightest texel of its cross-section
 * ({@link CROSS_SECTION_ALONG_PX}), scored against the surface's luminance.
 *
 * @remarks
 * A point where a neighbour crosses the stroke, or a run of points about it that the neighbour
 * touches, is not read. Elsewhere the texels a neighbour lights or covers are left out of the
 * cross-section, and the rest are read. A cross-section left with no texel, or one that leaves the
 * image, is not read.
 *
 * @param neighbours - The frame's other batches, as {@link neighbourOf} takes each for this one.
 * @param endMarginPx - How near a segment's end a point is not read, px: none by default.
 */
export function readStroke(
  image: ReadImage,
  stroke: ScreenStroke,
  neighbours: ReadonlyArray<Neighbour>,
  surfaceLuminance: number,
  keep: ((p: PointPx) => boolean) | null,
  endMarginPx = 0,
): StrokeReading {
  const points = sectionPoints(image, stroke, keep, endMarginPx);
  const crossings = neighbours.map((neighbour) =>
    crossingPoints(points, stroke, neighbour, image.widthPx),
  );
  let samples = 0;
  let worst = Number.POSITIVE_INFINITY;
  let at: PointPx | null = null;
  points.forEach((point, i) => {
    if (!point.read || point.section === null || crossings.some((each) => each[i] === 1)) {
      return;
    }
    const own = point.section.filter(([column, row]) =>
      neighbours.every((neighbour) => neighbour.region[row * image.widthPx + column] === 0),
    );
    if (own.length === 0) {
      return;
    }
    const brightest = Math.max(...own.map(([column, row]) => texelLuminance(image, column, row)));
    samples += 1;
    const ratio = contrastRatio(brightest, surfaceLuminance);
    if (ratio < worst) {
      worst = ratio;
      at = point.p;
    }
  });
  return { samples, worst, at };
}

/**
 * How far along a stroke a texel's centre may lie from a point and be of its cross-section, px:
 * half a texel's diagonal. It is the cross-section the ruling's figures take: with it a stroke up
 * to 2 device px wide peaks at w ÷ 2 of its colour at its worst angle and phase, and a 2 px one at
 * all of it at every angle and phase (decision-thin-line-contrast, item 1; with half a pixel, a 2 px
 * line at 45° would find no texel nearer its centreline than 0.71 px at a corner of the grid).
 */
const CROSS_SECTION_ALONG_PX = Math.SQRT1_2;

/**
 * A stroke's cross-section at `p`: the texels whose centres lie within
 * {@link CROSS_SECTION_ALONG_PX} of it along the stroke (`u`) and within `halfPx` across; `null`
 * where it reaches off the image.
 */
function crossSection(
  p: PointPx,
  u: PointPx,
  halfPx: number,
  image: ReadImage,
): Array<readonly [number, number]> | null {
  const span = halfPx + 1;
  const texels: Array<readonly [number, number]> = [];
  for (let row = Math.floor(p.y - span); row <= Math.ceil(p.y + span); row += 1) {
    for (let column = Math.floor(p.x - span); column <= Math.ceil(p.x + span); column += 1) {
      const dx = column + 0.5 - p.x;
      const dy = row + 0.5 - p.y;
      const along = dx * u.x + dy * u.y;
      const across = -dx * u.y + dy * u.x;
      if (Math.abs(along) <= CROSS_SECTION_ALONG_PX && Math.abs(across) <= halfPx) {
        if (column < 0 || row < 0 || column >= image.widthPx || row >= image.heightPx) {
          return null;
        }
        texels.push([column, row]);
      }
    }
  }
  return texels.length > 0 ? texels : null;
}

/** A list's line batches on the target: view-space ends projected, each segment's dash phase. */
export function screenStrokes(
  list: WireframeDrawList,
  camera: DrawCamera,
  viewport: Viewport,
): ScreenStroke[] {
  const projection: ProjectionCamera = {
    orientation: camera.pose.orientation,
    fovXRad: camera.fovXRad,
  };
  return list.lines.map((batch) => {
    const end = (at: number): PointPx | null => {
      const p = batch.segments;
      const x = p[at] ?? 0;
      const y = p[at + 1] ?? 0;
      const z = p[at + 2] ?? 0;
      if (batch.space === "screen") {
        return { x, y };
      }
      const o = batch.originF32;
      const projected = project(
        vec3((o[0] ?? 0) + x, (o[1] ?? 0) + y, (o[2] ?? 0) + z),
        projection,
        viewport,
      );
      return projected.inFront ? { x: projected.xPx, y: projected.yPx } : null;
    };
    const segments: Array<readonly [PointPx, PointPx]> = [];
    const phases: number[] = [];
    let phase = 0;
    let last: PointPx | null = null;
    for (let i = 0; i < batch.segments.length / 6; i += 1) {
      const a = end(i * 6);
      const b = end(i * 6 + 3);
      if (a === null || b === null) {
        last = null;
        phase = 0;
        continue;
      }
      // A dash runs on across a joint, and starts again at a break, as `packWireframe` packs it.
      if (last === null || last.x !== a.x || last.y !== a.y) {
        phase = 0;
      }
      segments.push([a, b]);
      phases.push(phase);
      phase += Math.hypot(b.x - a.x, b.y - a.y);
      last = b;
    }
    return {
      name: batch.id,
      segments,
      phases,
      widthPx: batch.widthPx,
      casingWidthPx: batch.casingWidthPx,
      dash: batch.dash,
    };
  });
}

/** A point `xPx` right of and `yPx` below the target's centre, `DISTANCE_M` ahead, system m. */
function aheadAt(xPx: number, yPx: number): Vec3 {
  return vec3((xPx / FOCAL_PX) * DISTANCE_M, (-yPx / FOCAL_PX) * DISTANCE_M, -DISTANCE_M);
}

/** A length on the target, px, as metres `DISTANCE_M` ahead. */
function metresAt(px: number): number {
  return (px / FOCAL_PX) * DISTANCE_M;
}

/** The camera at the system's barycentre, looking down −z, rolled `rollDeg` about its axis. */
function cameraRolled(rollDeg: number): DrawCamera {
  return {
    pose: {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(0, 0, 0),
      orientation:
        rollDeg === 0 ? NO_TURN : quaternionFromAxisAngle(vec3(0, 0, 1), (rollDeg * Math.PI) / 180),
    },
    fovXRad: FOV_X_RAD,
  };
}

/** A second small body's ID, for the destination. */
const SECOND_MOON = `${FIXTURE_SYSTEM}.0105`;

/** A small body, under 3 px across, so that it is drawn as its symbol: an open circle, class 2. */
function smallBody(id: string, xPx: number, yPx: number): ViewBody {
  return aBody({
    id,
    kind: "moon",
    designation: "TEST MOON",
    centreM: aheadAt(xPx, yPx),
    radiusM: 1e3,
    hillRadiusM: 1e6,
    symbol: { shape: "circle", sizeClass: 2 },
  });
}

/** A scene of the fixtures' system with these bodies and rings, and no craft or star. */
function sceneOf(overrides: Partial<ViewScene>): ViewScene {
  return aViewScene({ bodies: [], craft: [], stars: [], ownShip: null, ...overrides });
}

/**
 * The ring scene: a ring seen face-on, its 1 px `--text-muted` edges 75 and 114 px from the
 * centre and its radial ticks every 10° between them, about a small body drawn as its symbol.
 */
function ringScene(): ViewScene {
  return sceneOf({
    bodies: [smallBody(FIXTURE_PLANET, 0, 0)],
    rings: [
      {
        body: FIXTURE_PLANET,
        innerRadiusM: metresAt(RING_PX.inner),
        outerRadiusM: metresAt(RING_PX.outer),
        normal: vec3(0, 0, 1),
      },
    ],
  });
}

/**
 * The planet scene: a planet straight ahead, its pole towards the camera, so that its meridians,
 * 1 px, and its prime meridian, 1.5 px, run straight out from the pole to its 1 px limb.
 */
function planetScene(): ViewScene {
  // Its limb, the silhouette's circle of tangency, lies at asin(r ÷ d) from its centre: that angle's
  // tangent is the limb's radius on the target over the focal length.
  const radiusM = DISTANCE_M * Math.sin(Math.atan(PLANET_RADIUS_PX / FOCAL_PX));
  return sceneOf({
    bodies: [
      aBody({
        centreM: aheadAt(0, 0),
        radiusM,
        orbitNormal: vec3(0, 0, 1),
        hillRadiusM: 1e3 * radiusM,
      }),
    ],
  });
}

/** Another craft's pose `xPx`, `yPx` from the centre, ahead. */
function craftAt(xPx: number, yPx: number): CraftPose {
  return {
    position: { kind: "system", system: FIXTURE_SYSTEM, m: aheadAt(xPx, yPx) },
    attitude: NO_TURN,
  };
}

/**
 * The marks scene: two small bodies drawn as open circles in `--text`, the first the selection
 * (its `--accent` reticle) and the second the destination (its `--target` reticle); another
 * craft whose dashed `--text` predicted path crosses the right of the target, the craft itself
 * beyond the target's edge; and a third craft, in view, with its target's ticks in `--text`.
 */
function marksScene(): ViewScene {
  return sceneOf({
    bodies: [smallBody(FIXTURE_MOON, -60, -50), smallBody(SECOND_MOON, -60, 50)],
    craft: [
      aViewCraft({
        id: "other",
        designation: "OTHER",
        pose: craftAt(170, -150),
        predictedPath: [craftAt(170, -150), craftAt(40, -10), craftAt(90, 100)],
      }),
      aViewCraft({ id: "third", designation: "THIRD", pose: craftAt(100, 30) }),
    ],
  });
}

/**
 * The pair's scene: one small body drawn as an open circle, a whole number of pixels from the
 * target's centre, so that at 80% and a ratio of 0.78125 every arm of its bracket keeps a texel of
 * its inner edge that the control's destination does not cover.
 */
function pairScene(): ViewScene {
  return sceneOf({ bodies: [smallBody(FIXTURE_MOON, -20, 10)] });
}

/** Renders a list into a fresh target first filled with `fill`, and reads its colour. */
async function drawOver(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  name: string,
  fill: string,
  list: WireframeDrawList,
  camera: DrawCamera,
): Promise<ReadImage> {
  const target = engine.createRenderTarget({
    name,
    size: VIEWPORT,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  // The fill: one screen-space stroke wider than the target, uncased.
  const flood: LineBatch = {
    id: "fill",
    space: "screen",
    originF32: new Float32Array(3),
    segments: new Float32Array([-SIDE_PX, SIDE_PX / 2, 0, 2 * SIDE_PX, SIDE_PX / 2, 0]),
    // The token names the fill's role only; its colour is the fill's.
    token: "text",
    colour: fill,
    widthPx: 4 * SIDE_PX,
    casingWidthPx: 0,
    casingColour: fill,
    dash: null,
  };
  target.render(renderer.frame({ ...emptyDrawList(AS_BUILT), lines: [flood] }, camera, VIEWPORT));
  target.render({ ...renderer.frame(list, camera, VIEWPORT), colourLoad: "load" });
  const colour = halfTexels(await engine.readTexture(target.colour));
  target.dispose();
  return { colour, widthPx: SIDE_PX, heightPx: SIDE_PX };
}

/** Which strokes of a frame are read, and the points of each. */
interface Reading {
  /** The kind the reading is reported under. */
  readonly kind: string;
  /** Whether a batch is of this kind. */
  readonly of: (name: string) => boolean;
  /** Which of its points are read, or `null` for all. */
  readonly keep: ((p: PointPx) => boolean) | null;
  /** The least points it must read over its frames, if not {@link MIN_SAMPLES}. */
  readonly minSamples?: number;
  /**
   * The token whose pair with `--surface-0` it must read, to within {@link PAIR_TOLERANCE}, where
   * 6:1 is not enough: the pair of reticles, whose least gap keeps each one's full-coverage core
   * (decision-r07-t16d-followups, item 2). Such a reading keeps {@link CUT_END_MARGIN_PX} clear of
   * its segments' ends.
   */
  readonly pair?: ColourToken;
}

/** The distance of `p` from the target's centre, px. */
function fromCentre(p: PointPx): number {
  return Math.hypot(p.x - SIDE_PX / 2, p.y - SIDE_PX / 2);
}

/** The ring's readings: its edges and ticks, all of them. */
const RING_READINGS: ReadonlyArray<Reading> = [
  { kind: "ring edges and ticks, 1 px", of: (name) => name.startsWith("ring:"), keep: null },
];

/** Whether a point lies on the planet clear of its pole and limb, or on its limb. */
function onGraticule(p: PointPx, withLimb: boolean): boolean {
  const r = fromCentre(p) / PLANET_RADIUS_PX;
  // Within 5 px of the limb a line may meet the occluder's limb bound (Design note 5).
  return (r > 0.3 && r < 0.95) || (withLimb && r > 0.98 && r < 1.02);
}

/** The planet's readings: its limb and meridians, and its prime meridian, clear of the pole. */
const PLANET_READINGS: ReadonlyArray<Reading> = [
  {
    kind: "limb and meridians, 1 px",
    of: (name) => name.endsWith(":graticule"),
    keep: (p) => onGraticule(p, true),
  },
  {
    kind: "prime meridian, 1.5 px",
    of: (name) => name.endsWith(":major"),
    keep: (p) => onGraticule(p, false),
  },
];

/** The marks' readings: the symbols, the two reticles and the dashed path. */
const MARK_READINGS: ReadonlyArray<Reading> = [
  { kind: "body symbols", of: (name) => name.startsWith("mark:body_symbol:"), keep: null },
  { kind: "--accent reticle", of: (name) => name.startsWith("mark:selection:"), keep: null },
  { kind: "--target reticle", of: (name) => name.startsWith("mark:destination:"), keep: null },
  { kind: "predicted path, dashed", of: (name) => name.startsWith("predicted:"), keep: null },
  {
    kind: "target ticks",
    of: (name) => name.startsWith("mark:target:"),
    keep: null,
    minSamples: MIN_TICK_SAMPLES,
  },
];

/** The bracket about the destination on the selection. */
const PAIR_BRACKET: Reading = {
  kind: "--accent reticle, the destination about it",
  of: (name) => name.startsWith("mark:selection:"),
  keep: null,
  pair: "accent",
};

/** The pair's readings: its symbol, its bracket, and the destination's reticle about it. */
const PAIR_READINGS: ReadonlyArray<Reading> = [
  { kind: "body symbols", of: (name) => name.startsWith("mark:body_symbol:"), keep: null },
  PAIR_BRACKET,
  {
    kind: "--target reticle about the selection",
    of: (name) => name.startsWith("mark:destination:"),
    keep: null,
    pair: "target",
  },
];

/** One kind's least contrast over a frame's batches of that kind, and its samples. */
interface KindReading {
  readonly kind: string;
  readonly samples: number;
  readonly worst: number;
  readonly at: PointPx | null;
}

/**
 * Reads every kind of `readings` in a drawn frame, each batch with every other batch as its
 * neighbour: lighting it, and covering it where drawn after it, in the list's order.
 */
function readFrame(
  image: ReadImage,
  strokes: ReadonlyArray<ScreenStroke>,
  readings: ReadonlyArray<Reading>,
  surfaceLuminance: number,
): KindReading[] {
  const lighting = strokes.map((stroke) =>
    neighbourOf(stroke, false, image.widthPx, image.heightPx),
  );
  const covering = strokes.map((stroke) =>
    neighbourOf(stroke, true, image.widthPx, image.heightPx),
  );
  return readings.map((reading) => {
    let samples = 0;
    let worst = Number.POSITIVE_INFINITY;
    let at: PointPx | null = null;
    strokes.forEach((stroke, index) => {
      if (!reading.of(stroke.name)) {
        return;
      }
      // A batch drawn before this one lights it; one drawn after covers it with its casing too.
      const neighbours = strokes.flatMap((_, other) => {
        const neighbour = other < index ? lighting[other] : covering[other];
        return other === index || neighbour === undefined ? [] : [neighbour];
      });
      // A reading held to its pair keeps clear of its arms' cut ends, whose caps fall off.
      const read = readStroke(
        image,
        stroke,
        neighbours,
        surfaceLuminance,
        reading.keep,
        reading.pair === undefined ? 0 : CUT_END_MARGIN_PX,
      );
      samples += read.samples;
      if (read.worst < worst) {
        ({ worst, at } = read);
      }
    });
    return { kind: reading.kind, samples, worst, at };
  });
}

/** A kind's reading in words: its least contrast, where, and its samples. */
function shown(reading: KindReading): string {
  const where =
    reading.at === null ? "" : ` at (${reading.at.x.toFixed(1)}, ${reading.at.y.toFixed(1)})`;
  return `${reading.kind} ${reading.worst.toFixed(2)}:1${where}, ${String(reading.samples)} points`;
}

/** What a frame of the check marks: nothing, two bodies, or one body as both. */
type FrameMarks = "none" | "apart" | "pair";

/** A frame of the check: its scene, camera, readings, marks and interface scale. */
interface CheckFrame {
  readonly scene: ViewScene;
  readonly camera: DrawCamera;
  readonly readings: ReadonlyArray<Reading>;
  readonly marks: FrameMarks;
  /** The interface scale, which the rem follows: 1 at 100%. */
  readonly interfaceScale: number;
}

/** Every frame the check draws at each ratio, in each style. */
const FRAMES: ReadonlyArray<CheckFrame> = [
  ...ROLLS_DEG.map((roll) => ({
    scene: ringScene(),
    camera: cameraRolled(roll),
    readings: RING_READINGS,
    marks: "none" as const,
    interfaceScale: 1,
  })),
  ...ROLLS_DEG.map((roll) => ({
    scene: planetScene(),
    camera: cameraRolled(roll),
    readings: PLANET_READINGS,
    marks: "none" as const,
    interfaceScale: 1,
  })),
  {
    scene: marksScene(),
    camera: cameraRolled(0),
    readings: MARK_READINGS,
    marks: "apart",
    interfaceScale: 1,
  },
  ...[1, SMALL_INTERFACE].map((interfaceScale) => ({
    scene: pairScene(),
    camera: cameraRolled(0),
    readings: PAIR_READINGS,
    marks: "pair" as const,
    interfaceScale,
  })),
];

/** Every kind a style's frames read, once, with the reading that names it. */
const KINDS = new Map(
  FRAMES.flatMap((frame) =>
    frame.readings.map((reading): [string, Reading] => [reading.kind, reading]),
  ),
);

/** The least contrast a kind must read: its token's pair, less the tolerance, or 6:1. */
function requiredRatio(
  reading: Reading | undefined,
  tokens: ColourTokens,
  surface: number,
): number {
  return reading?.pair === undefined
    ? REQUIRED_RATIO
    : contrastRatio(tokenLuminance(tokens[reading.pair]), surface) * (1 - PAIR_TOLERANCE);
}

/** Each kind's least over the frames it was read in, and its points summed. */
function byKind(readings: ReadonlyArray<KindReading>): KindReading[] {
  const kinds = new Map<string, KindReading>();
  for (const reading of readings) {
    const before = kinds.get(reading.kind);
    const keepBefore = before !== undefined && before.worst <= reading.worst;
    kinds.set(reading.kind, {
      kind: reading.kind,
      samples: (before?.samples ?? 0) + reading.samples,
      worst: keepBefore ? before.worst : reading.worst,
      at: keepBefore ? before.at : reading.at,
    });
  }
  return [...kinds.values()];
}

/** The selection and the destination each kind of frame marks. */
const MARKED: Readonly<Record<FrameMarks, Pick<DrawOptions, "selection" | "destination">>> = {
  none: { selection: null, destination: null },
  apart: {
    selection: { kind: "body", body: FIXTURE_MOON },
    destination: { kind: "body", body: SECOND_MOON },
  },
  pair: {
    selection: { kind: "body", body: FIXTURE_MOON },
    destination: { kind: "body", body: FIXTURE_MOON },
  },
};

/**
 * R07.T16.d and T16.g: the view's strokes reach 6:1 as drawn at ratios of 0.78125, 1 and 2, in
 * the wireframe and over the image, the destination's reticle on the selection's among them; the
 * controls, as built, do not.
 */
export async function checkStrokeContrast(engine: RenderEngine, checks: Checks): Promise<void> {
  const tokens: ColourTokens = readTokens(document.documentElement);
  const surface = tokenLuminance(tokens.surface0);
  const renderer = new WireframeRenderer(engine);
  for (const ratio of RATIOS) {
    const strokes = viewStrokesAt(ratio);
    const base = { lowSetting: false, ev100: 0 } as const;
    for (const style of ["wireframe", "overlay"] as const) {
      const fill = style === "wireframe" ? tokens.surface0 : tokens.text;
      const readings: KindReading[] = [];
      for (const frame of FRAMES) {
        const built = buildWireframeDrawList(frame.scene, frame.camera, VIEWPORT, tokens, {
          ...base,
          ...strokes,
          ...MARKED[frame.marks],
          remPx: REM_CSS_PX * frame.interfaceScale * ratio,
        });
        const list = style === "overlay" ? overlayDrawList(built, tokens) : built;
        // The checks run in order: each reads the GPU back before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        const image = await drawOver(
          engine,
          renderer,
          `R07 stroke contrast`,
          fill,
          list,
          frame.camera,
        );
        const drawn = screenStrokes(list, frame.camera, VIEWPORT);
        readings.push(...readFrame(image, drawn, frame.readings, surface));
      }
      const kinds = byKind(readings);
      checks.check(
        `R07.T16.d every stroke reaches 6:1 as drawn, and the pair of reticles its tokens' ratios (T16.g), at a ratio of ${String(ratio)}, ${style === "wireframe" ? "in the wireframe" : "over the image"} (lines ${String(strokes.strokeScale)} px per CSS px, outlines ${String(strokes.markStrokePx)} px, reticles ${String(strokes.minReticleGapPx)} px apart at least)`,
        kinds.length === KINDS.size &&
          kinds.every(
            (kind) =>
              kind.samples >= (KINDS.get(kind.kind)?.minSamples ?? MIN_SAMPLES) &&
              kind.worst >= requiredRatio(KINDS.get(kind.kind), tokens, surface),
          ),
        kinds.map(shown).join("; "),
      );
      if (ratio === RATIOS[0]) {
        // The pair's control: the destination a bare 0.25 rem outside the bracket, as built
        // before R07.T16.d, its casing over the bracket's core at 80%.
        const camera = cameraRolled(0);
        const tight = buildWireframeDrawList(pairScene(), camera, VIEWPORT, tokens, {
          ...base,
          ...strokes,
          ...MARKED.pair,
          minReticleGapPx: 0,
          remPx: REM_CSS_PX * SMALL_INTERFACE * ratio,
        });
        const list = style === "overlay" ? overlayDrawList(tight, tokens) : tight;
        // The checks run in order: each reads the GPU back before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        const image = await drawOver(
          engine,
          renderer,
          "R07 stroke contrast pair control",
          fill,
          list,
          camera,
        );
        const [control] = readFrame(
          image,
          screenStrokes(list, camera, VIEWPORT),
          [PAIR_BRACKET],
          surface,
        );
        checks.check(
          `R07.T16.g the pair's control, the destination 0.25 rem outside the bracket at 80% and a ratio of ${String(ratio)}, ${style === "wireframe" ? "in the wireframe" : "over the image"}, reads the bracket under 6:1`,
          control !== undefined && control.samples >= MIN_SAMPLES && control.worst < REQUIRED_RATIO,
          control === undefined ? "no reading" : shown(control),
        );
      }
    }
    // The control: the ring as built before R07.T16.d, its 1 px edges under 6:1.
    const camera = cameraRolled(0);
    const asBuilt = buildWireframeDrawList(ringScene(), camera, VIEWPORT, tokens, {
      ...base,
      ...AS_BUILT,
      ...MARKED.none,
      remPx: REM_CSS_PX * ratio,
    });
    // The checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const image = await drawOver(
      engine,
      renderer,
      "R07 stroke contrast control",
      tokens.surface0,
      asBuilt,
      camera,
    );
    const edges: Reading = {
      kind: "ring edges, 1 device px",
      of: (name) => name.startsWith("ring:"),
      keep: (p) => [RING_PX.inner, RING_PX.outer].some((r) => Math.abs(fromCentre(p) - r) < 1),
    };
    const [control] = readFrame(image, screenStrokes(asBuilt, camera, VIEWPORT), [edges], surface);
    checks.check(
      `R07.T16.d the control, drawn as built at a ratio of ${String(ratio)} (1 device px per CSS px), reads under 6:1 on its 1 px circles`,
      control !== undefined && control.samples >= MIN_SAMPLES && control.worst < REQUIRED_RATIO,
      control === undefined ? "no reading" : shown(control),
    );
  }
  renderer.dispose();
}
