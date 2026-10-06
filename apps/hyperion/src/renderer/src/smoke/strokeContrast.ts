/**
 * The smoke page's check of the view's strokes as drawn (plan R07, R07.T16.d;
 * decision-thin-line-contrast, items 1 and 4): R02's draw list, built by `buildWireframeDrawList`
 * at the strokes of display ratios 0.78125, 1 and 2, drawn by R02's renderer and read back.
 *
 * @remarks
 * At every pixel of length along each stroke, the brightest texel of its cross-section must reach
 * 6.0:1 against `--surface-0`, by WCAG's formula on the 8-bit sRGB texel the canvas would hold:
 * in the wireframe, over a `--surface-0` target, and through the photorealistic overlay over a
 * target loaded with `--text`, where each stroke stands on its casing. Three scenes hold the
 * strokes the ruling names, each on a 256 px square:
 * - a ring seen face-on from a camera rolled 0°, 3° and 45°: its 1 px `--text-muted` edges, and
 *   its radial ticks, which then lie at 0°, 3° and 45° and at every 10° on from each;
 * - a planet seen pole-on, rolled the same, its limb 110 px from the centre: its 1 px limb and
 *   meridians and its 1.5 px prime meridian, read clear of the pole and of the limb's bound;
 * - two open circle symbols in `--text`, the selection with its `--accent` reticle and the
 *   destination with its `--target` reticle, another craft's dashed `--text` path, and a third
 *   craft's `--text` target ticks.
 *
 * A cross-section that another batch's stroke or casing reaches is not read: a later batch's
 * casing cuts an earlier stroke where they cross, by design (a casing covers what lies beneath
 * it). A control, the ring as built before R07.T16.d (1 device px per CSS px, outlines 1.5 px),
 * must read under 6 on its 1 px edges, so that the check is seen to bite.
 */

import { type Vec3, vec3 } from "../geometry/vec3";
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

/** The least points a kind must read over its frames, so that no reading passes empty. */
const MIN_SAMPLES = 20;

/**
 * The least points the target's ticks must read: four ticks about 3 px long each at a ratio of
 * 0.78125, which read a dozen points there over the image, the craft's own cased hull reaching
 * their inner ends.
 */
const MIN_TICK_SAMPLES = 8;

/** A dash's margin, px: a sample this near a dash's end is not read. */
const DASH_MARGIN_PX = 1;

/** The strokes as built before R07.T16.d: device px, the outlines 1.5 px and not moved out. */
const AS_BUILT: ViewStrokes = { strokeScale: 1, markStrokePx: SYMBOL_STROKE_PX, markShiftPx: 0 };

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

/**
 * How many of `strokes` reach each texel of an image: its centre within a stroke's half-width,
 * its casing and the antialiasing fringe of a segment, and a texel's half-diagonal more, so that
 * a texel any of them touches is counted. Each stroke counts once a texel.
 */
export function reachCounts(
  strokes: ReadonlyArray<ScreenStroke>,
  widthPx: number,
  heightPx: number,
): { readonly counts: Uint16Array; readonly masks: ReadonlyArray<Uint8Array> } {
  const counts = new Uint16Array(widthPx * heightPx);
  const masks = strokes.map((stroke) => {
    const mask = new Uint8Array(widthPx * heightPx);
    const reachPx = stroke.widthPx / 2 + stroke.casingWidthPx + 0.5 + Math.SQRT1_2;
    for (const [a, b] of stroke.segments) {
      const left = Math.max(0, Math.floor(Math.min(a.x, b.x) - reachPx));
      const right = Math.min(widthPx - 1, Math.ceil(Math.max(a.x, b.x) + reachPx));
      const top = Math.max(0, Math.floor(Math.min(a.y, b.y) - reachPx));
      const bottom = Math.min(heightPx - 1, Math.ceil(Math.max(a.y, b.y) + reachPx));
      for (let row = top; row <= bottom; row += 1) {
        for (let column = left; column <= right; column += 1) {
          const at = row * widthPx + column;
          if (
            mask[at] === 0 &&
            distanceToSegment({ x: column + 0.5, y: row + 0.5 }, a, b) < reachPx
          ) {
            mask[at] = 1;
            counts[at] = (counts[at] ?? 0) + 1;
          }
        }
      }
    }
    return mask;
  });
  return { counts, masks };
}

/**
 * A stroke as drawn: at every pixel of its length (on a dash, away from its ends; and where
 * `keep` holds, if given), the brightest texel of its cross-section ({@link CROSS_SECTION_ALONG_PX}),
 * scored against the surface's luminance. A cross-section that another batch reaches (`blocked`),
 * or that leaves the image, is not read.
 */
export function readStroke(
  image: ReadImage,
  stroke: ScreenStroke,
  blocked: (column: number, row: number) => boolean,
  surfaceLuminance: number,
  keep: ((p: PointPx) => boolean) | null,
): StrokeReading {
  const halfPx = stroke.widthPx / 2 + 0.5;
  let samples = 0;
  let worst = Number.POSITIVE_INFINITY;
  let at: PointPx | null = null;
  stroke.segments.forEach(([a, b], index) => {
    const lengthPx = Math.hypot(b.x - a.x, b.y - a.y);
    if (!(lengthPx > 0)) {
      return;
    }
    const u = { x: (b.x - a.x) / lengthPx, y: (b.y - a.y) / lengthPx };
    const steps = Math.max(1, Math.floor(lengthPx));
    for (let i = 0; i <= steps; i += 1) {
      const s = (lengthPx * i) / steps;
      const p = { x: a.x + u.x * s, y: a.y + u.y * s };
      if (stroke.dash !== null) {
        const period = stroke.dash.onPx + stroke.dash.offPx;
        const phase = ((((stroke.phases[index] ?? 0) + s) % period) + period) % period;
        if (phase < DASH_MARGIN_PX || phase > stroke.dash.onPx - DASH_MARGIN_PX) {
          continue;
        }
      }
      if (keep !== null && !keep(p)) {
        continue;
      }
      const section = crossSection(p, u, halfPx, image);
      if (section === null || section.some(([column, row]) => blocked(column, row))) {
        continue;
      }
      const brightest = Math.max(
        ...section.map(([column, row]) => texelLuminance(image, column, row)),
      );
      samples += 1;
      const ratio = contrastRatio(brightest, surfaceLuminance);
      if (ratio < worst) {
        worst = ratio;
        at = p;
      }
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

/** One kind's least contrast over a frame's batches of that kind, and its samples. */
interface KindReading {
  readonly kind: string;
  readonly samples: number;
  readonly worst: number;
  readonly at: PointPx | null;
}

/** Reads every kind of `readings` in a drawn frame, each batch clear of every other batch. */
function readFrame(
  image: ReadImage,
  strokes: ReadonlyArray<ScreenStroke>,
  readings: ReadonlyArray<Reading>,
  surfaceLuminance: number,
): KindReading[] {
  const { counts, masks } = reachCounts(strokes, image.widthPx, image.heightPx);
  return readings.map((reading) => {
    let samples = 0;
    let worst = Number.POSITIVE_INFINITY;
    let at: PointPx | null = null;
    strokes.forEach((stroke, index) => {
      if (!reading.of(stroke.name)) {
        return;
      }
      const own = masks[index];
      // A texel another batch reaches: counted by more batches than this one's own reach.
      const blocked = (column: number, row: number): boolean => {
        const texelAt = row * image.widthPx + column;
        return (counts[texelAt] ?? 0) - (own?.[texelAt] ?? 0) > 0;
      };
      const read = readStroke(image, stroke, blocked, surfaceLuminance, reading.keep);
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

/** A frame of the check: its scene, camera, readings, and whether it marks its two bodies. */
interface CheckFrame {
  readonly scene: ViewScene;
  readonly camera: DrawCamera;
  readonly readings: ReadonlyArray<Reading>;
  readonly marked: boolean;
}

/** Every frame the check draws at each ratio, in each style. */
const FRAMES: ReadonlyArray<CheckFrame> = [
  ...ROLLS_DEG.map((roll) => ({
    scene: ringScene(),
    camera: cameraRolled(roll),
    readings: RING_READINGS,
    marked: false,
  })),
  ...ROLLS_DEG.map((roll) => ({
    scene: planetScene(),
    camera: cameraRolled(roll),
    readings: PLANET_READINGS,
    marked: false,
  })),
  { scene: marksScene(), camera: cameraRolled(0), readings: MARK_READINGS, marked: true },
];

/** Every kind a style's frames read, once, with the least points it must read. */
const KINDS = new Map(
  FRAMES.flatMap((frame) =>
    frame.readings.map((reading): [string, number] => [
      reading.kind,
      reading.minSamples ?? MIN_SAMPLES,
    ]),
  ),
);

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

/**
 * R07.T16.d: the view's strokes reach 6:1 as drawn at ratios of 0.78125, 1 and 2, in the
 * wireframe and over the image; the control, as built, does not.
 */
export async function checkStrokeContrast(engine: RenderEngine, checks: Checks): Promise<void> {
  const tokens: ColourTokens = readTokens(document.documentElement);
  const surface = tokenLuminance(tokens.surface0);
  const renderer = new WireframeRenderer(engine);
  for (const ratio of RATIOS) {
    const strokes = viewStrokesAt(ratio);
    const base = { lowSetting: false, ev100: 0, remPx: 16 * ratio } as const;
    for (const style of ["wireframe", "overlay"] as const) {
      const fill = style === "wireframe" ? tokens.surface0 : tokens.text;
      const readings: KindReading[] = [];
      for (const frame of FRAMES) {
        const built = buildWireframeDrawList(frame.scene, frame.camera, VIEWPORT, tokens, {
          ...base,
          ...strokes,
          selection: frame.marked ? { kind: "body", body: FIXTURE_MOON } : null,
          destination: frame.marked ? { kind: "body", body: SECOND_MOON } : null,
        });
        const list = style === "overlay" ? overlayDrawList(built) : built;
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
        `R07.T16.d every stroke reaches 6:1 as drawn at a ratio of ${String(ratio)}, ${style === "wireframe" ? "in the wireframe" : "over the image"} (lines ${String(strokes.strokeScale)} px per CSS px, outlines ${String(strokes.markStrokePx)} px)`,
        kinds.length === KINDS.size &&
          kinds.every(
            (kind) =>
              kind.samples >= (KINDS.get(kind.kind) ?? MIN_SAMPLES) && kind.worst >= REQUIRED_RATIO,
          ),
        kinds.map(shown).join("; "),
      );
    }
    // The control: the ring as built before R07.T16.d, its 1 px edges under 6:1.
    const camera = cameraRolled(0);
    const asBuilt = buildWireframeDrawList(ringScene(), camera, VIEWPORT, tokens, {
      ...base,
      ...AS_BUILT,
      selection: null,
      destination: null,
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
