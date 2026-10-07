/**
 * The smoke page's check of the spatial displays' strokes as drawn (plan R07, R07.T16.f;
 * decision-thin-line-contrast, items 1 and 4; decision-r07-t16d-followups, items 2 and (b)): P05's
 * painter, `paint`, drawing draw lists on a 2D canvas whose backing store stands for the device at
 * ratios 0.78125, 1 and 2, read back.
 *
 * @remarks
 * `paint` takes the ratio as an argument, so the check needs no forced device scale factor and no
 * display: each frame's canvas is the view's size in CSS px times the ratio, as `SpatialView`
 * backs its own. At every device pixel of length along each stroke, the brightest pixel of its
 * cross-section must reach 6.0:1 against `--surface-0`, by WCAG's formula on the 8-bit sRGB pixel
 * the canvas holds. The cross-section is `checkStrokeContrast`'s since R07.T16.g: the pixels whose
 * centres lie within half a pixel's diagonal of a point along the stroke and within its half-width
 * and antialiased edge across it, less those another stroke lights, with the points where another
 * stroke crosses it left out. Canvas 2D's coverage is a pixel's area, so its edge reaches half a
 * pixel's diagonal ({@link CANVAS_FRINGE_PX}), and since `paint` draws no casing, lighting is all a
 * neighbour does. Its strokes end flat (butt caps), so a cut end, which is not where a stroke falls
 * between pixels, is kept a pixel clear, as a dash's ends are.
 *
 * What it reads is what `paint` drew: the canvas's path calls are recorded as they are made, so the
 * widths and the outlines' outward shifts are the painter's own. Three frames hold the strokes the
 * ruling names, each painted in the tokens and again in the stale tokens:
 * - a 1 px `--text-muted` circle, a 1 px `--text` data edge with its ticks, and a 2 px `--text`
 *   polyline, a selected orbit;
 * - 1 px `--text-muted` lines at 0°, 3° and 45°;
 * - an open class-0 circle in `--accent`, the selection and the destination, with its `--accent`
 *   bracket and the `--target` reticle at the least gap outside it, and an open class-2 ringed
 *   circle in `--accent`, the destination alone, with its `--target` reticle; each mark centred on
 *   a device pixel, whose pixel must read `--surface-0`, the hole kept. Between the pair's arms a
 *   pixel must read `--surface-0` too, since on a canvas, which draws no casing, the least gap is
 *   there for separation (decision-r07-t16d-followups, item 2).
 *
 * The data edge's ticks are a kind of their own, since the rule leaves little of them: below a ratio
 * of 1 a 0.25 rem tick lies mostly in its crossing with the circle and its cut end's margin, so
 * there they need read only 8 points, as T16.d's target ticks do.
 *
 * A control, a 1 CSS px circle stroked directly at 0.78125, must read under 6, so that the check is
 * seen to bite.
 */

import { localFrameAt } from "../geometry/frame";
import { scale, vec3 } from "../geometry/vec3";
import { lineScale, markStrokeDevicePx, minReticleGapDevicePx } from "../lib/strokes";
import type { Camera, Viewport } from "../spatial/camera";
import {
  buildDrawList,
  type DrawOp,
  type ReticleOp,
  type ScreenPoint,
  type SymbolOp,
} from "../spatial/drawList";
import type { PointMark, SpatialScene } from "../spatial/marks";
import { type ColourTokens, paint, readTokens, staleTokens } from "../spatial/paint";
import { type Checks, texel } from "./harness";
import {
  CUT_END_MARGIN_PX,
  neighbourOf,
  type PointPx,
  type ReadImage,
  readStroke,
  REQUIRED_RATIO,
  type ScreenStroke,
  srgb8,
  tokenLuminance,
  wcagLuminance,
} from "./strokeContrast";

/** The ratios the check paints at: the development machine's and the UHD 620's, 100% and 2. */
export const SPATIAL_RATIOS = [0.78125, 1, 2] as const;

/** The view each frame paints, CSS px, at 100% (a rem of 16 px). */
const VIEWPORT: Viewport = { widthPx: 240, heightPx: 240, remPx: 16 };

/** The view's centre, CSS px. */
const CENTRE: ScreenPoint = { xPx: 120, yPx: 120 };

/**
 * How far beyond its half-width Canvas 2D's antialiasing lights a pixel's centre, px: half a
 * pixel's diagonal, since its coverage is the area of the pixel the stroke covers, and a pixel's
 * corner reaches that far towards an edge at 45°.
 */
export const CANVAS_FRINGE_PX = Math.SQRT1_2;

/** The least points a kind must read over its frames, so that no reading passes empty. */
const MIN_SAMPLES = 20;

/**
 * The least points the data edge's ticks must read at a ratio: 8 below a ratio of 1, as T16.d's
 * target ticks must. A tick is 0.25 rem long, 3.1 device px at 0.78125, and starts on the circle's
 * centreline, so up to 2.4 px of it lie in the crossing with the circle, which the reading leaves
 * out, and its last pixel is its cut end's margin: only where a tick's sampled points fall short
 * of the margin is one read (10 at 0.78125). At 1 a tick reads about one point, and at 2 about five.
 */
function minTickSamples(ratio: number): number {
  return ratio < 1 ? 8 : MIN_SAMPLES;
}

/** The longest chord of an arc the check reads, device px: it then lies within 0.002 px of a 60 px arc. */
const ARC_CHORD_PX = 1;

/** The fewest chords a full circle is read as, however small. */
const MIN_ARC_CHORDS = 12;

/** A camera straight down on the plane, a scene unit to the CSS pixel. */
const TOP_DOWN: Camera = { azimuthDeg: 0, elevationDeg: 90, pxPerUnit: 1 };

/** The scenes' frame, the star chart's at the Sun's radius. */
const FRAME = localFrameAt(vec3(26_000, 0, 0));

/** Where each kind of mark is centred, CSS px, before it is moved onto a device pixel's centre. */
const MARK_AT = {
  pair: { xPx: 80, yPx: 120 },
  ringed: { xPx: 170, yPx: 120 },
} as const;

/** One op of a frame, and the kind its reading is reported under. */
export interface FrameOp {
  readonly op: DrawOp;
  readonly kind: string;
}

/** A frame of the check: its ops, and the points, CSS px, whose pixel must read `--surface-0`. */
export interface SpatialFrame {
  readonly name: string;
  readonly ops: ReadonlyArray<FrameOp>;
  readonly holes: ReadonlyArray<ScreenPoint>;
}

/** The kinds the check reads, as the ruling names them. */
export const SPATIAL_KINDS = {
  circle: "1 px circle, --text-muted",
  dataEdge: "data edge, 1 px --text",
  dataEdgeTicks: "data edge's ticks, 1 px --text",
  selectedOrbit: "selected orbit, 2 px --text",
  lines: "1 px lines at 0°, 3° and 45°, --text-muted",
  openCircle: "open class-0 circle, --accent",
  ringedCircle: "open class-2 ringed circle, --accent",
  pairBracket: "--accent reticle, the destination about it",
  pairDestination: "--target reticle about the selection",
  loneDestination: "--target reticle alone",
} as const;

/** A scene of nothing but these marks and spheres, its plane bare. */
function sceneOf(
  points: ReadonlyArray<PointMark>,
  overrides: Partial<SpatialScene> = {},
): SpatialScene {
  return {
    frame: FRAME,
    points,
    spheres: [],
    plane: { spacing: 10, extent: 0, rings: [] },
    selectedId: null,
    destinationId: null,
    ...overrides,
  };
}

/** An available mark below the plane, so that it is open and in `--accent`. */
function openMark(id: string, shape: PointMark["shape"], sizeClass: PointMark["sizeClass"]) {
  const mark: PointMark = {
    id,
    position: scale(FRAME.north, -1),
    shape,
    sizeClass,
    status: "available",
    label: id,
    labelPriority: 1,
  };
  return mark;
}

/** `at` moved to the centre of the device pixel it falls in at `ratio`, CSS px. */
function onPixelCentre(at: ScreenPoint, ratio: number): ScreenPoint {
  return {
    xPx: (Math.floor(at.xPx * ratio) + 0.5) / ratio,
    yPx: (Math.floor(at.yPx * ratio) + 0.5) / ratio,
  };
}

/** Whether an op is a mark's symbol or reticle, which a mark's centre places. */
function isMarkOp(op: DrawOp): op is SymbolOp | ReticleOp {
  return op.kind === "symbol" || op.kind === "reticle";
}

/** A mark's symbol and reticles from a draw list, centred on `at`. */
function marksAt(ops: ReadonlyArray<DrawOp>, at: ScreenPoint): DrawOp[] {
  const marks: DrawOp[] = [];
  for (const op of ops) {
    if (isMarkOp(op)) {
      marks.push({ ...op, centre: at });
    }
  }
  return marks;
}

/** The kind a mark's symbol or reticle is read as. */
function markKind(op: DrawOp, alone: boolean): string {
  if (op.kind === "symbol") {
    return op.shape === "ringed-circle" ? SPATIAL_KINDS.ringedCircle : SPATIAL_KINDS.openCircle;
  }
  if (op.kind === "reticle" && op.stroke === "accent") {
    return SPATIAL_KINDS.pairBracket;
  }
  return alone ? SPATIAL_KINDS.loneDestination : SPATIAL_KINDS.pairDestination;
}

/** An ellipse about `centre`, closed, as a polyline of `points` points. */
function ellipse(centre: ScreenPoint, aPx: number, bPx: number, points: number): ScreenPoint[] {
  return Array.from({ length: points + 1 }, (_, i) => {
    const angle = (2 * Math.PI * (i % points)) / points;
    return { xPx: centre.xPx + aPx * Math.cos(angle), yPx: centre.yPx + bPx * Math.sin(angle) };
  });
}

/** A 1 px `--text-muted` line from `from`, `lengthPx` long at `angleDeg` below the horizontal. */
function lineAt(from: ScreenPoint, angleDeg: number, lengthPx: number): DrawOp {
  const angle = (angleDeg * Math.PI) / 180;
  return {
    kind: "line",
    from,
    to: { xPx: from.xPx + lengthPx * Math.cos(angle), yPx: from.yPx + lengthPx * Math.sin(angle) },
    stroke: "textMuted",
    widthPx: 1,
    markId: null,
  };
}

/**
 * The check's frames at a device-pixel ratio: the curves, the lines and the marks, built as the
 * spatial displays build theirs, `buildDrawList` at the ratio's least reticle gap for the data
 * edge and the marks.
 */
export function spatialFrames(ratio: number): SpatialFrame[] {
  const minGapPx = minReticleGapDevicePx(ratio) / ratio;
  const dataEdge = buildDrawList(
    sceneOf([], { spheres: [{ radius: 60, role: "data_edge", label: "" }] }),
    TOP_DOWN,
    VIEWPORT,
    minGapPx,
  ).ops.filter((op) => op.kind === "circle" || op.kind === "ticks");
  const pairAt = onPixelCentre(MARK_AT.pair, ratio);
  const ringedAt = onPixelCentre(MARK_AT.ringed, ratio);
  const pair = marksAt(
    buildDrawList(
      sceneOf([openMark("pair", "circle", 0)], { selectedId: "pair", destinationId: "pair" }),
      TOP_DOWN,
      VIEWPORT,
      minGapPx,
    ).ops,
    pairAt,
  );
  const ringed = marksAt(
    buildDrawList(
      sceneOf([openMark("ringed", "ringed-circle", 2)], { destinationId: "ringed" }),
      TOP_DOWN,
      VIEWPORT,
      minGapPx,
    ).ops,
    ringedAt,
  );
  return [
    {
      name: "curves",
      ops: [
        {
          op: { kind: "circle", centre: CENTRE, radiusPx: 100, stroke: "textMuted", widthPx: 1 },
          kind: SPATIAL_KINDS.circle,
        },
        ...dataEdge.map((op) => ({
          op,
          kind: op.kind === "ticks" ? SPATIAL_KINDS.dataEdgeTicks : SPATIAL_KINDS.dataEdge,
        })),
        {
          op: {
            kind: "polyline",
            points: ellipse(CENTRE, 44, 26, 120),
            stroke: "text",
            widthPx: 2,
          },
          kind: SPATIAL_KINDS.selectedOrbit,
        },
      ],
      holes: [],
    },
    {
      name: "lines",
      ops: [
        lineAt({ xPx: 20, yPx: 30 }, 0, 200),
        lineAt({ xPx: 20, yPx: 80 }, 3, 200),
        lineAt({ xPx: 30, yPx: 120 }, 45, 140),
      ].map((op) => ({ op, kind: SPATIAL_KINDS.lines })),
      holes: [],
    },
    {
      name: "marks",
      ops: [
        ...pair.map((op) => ({ op, kind: markKind(op, false) })),
        ...ringed.map((op) => ({ op, kind: markKind(op, true) })),
      ],
      holes: [pairAt, ringedAt],
    },
  ];
}

/** A subpath as `paint` traced it, device px, and whether it was closed. */
interface Subpath {
  readonly points: PointPx[];
  closed: boolean;
}

/** A stroke `paint` made, as the canvas drew it: its subpaths and its width, device px. */
export interface PaintedStroke {
  readonly subpaths: ReadonlyArray<{
    readonly points: ReadonlyArray<PointPx>;
    readonly closed: boolean;
  }>;
  readonly widthPx: number;
}

/**
 * The points of an arc of the canvas's, device px: at most {@link ARC_CHORD_PX} apart, and for a
 * full turn ending exactly where it starts.
 */
export function arcPoints(
  centre: PointPx,
  radiusPx: number,
  fromRad: number,
  toRad: number,
): PointPx[] {
  const sweep = toRad - fromRad;
  const fullTurn = Math.abs(sweep) >= 2 * Math.PI;
  const chords = Math.max(MIN_ARC_CHORDS, Math.ceil((Math.abs(sweep) * radiusPx) / ARC_CHORD_PX));
  return Array.from({ length: chords + 1 }, (_, i) => {
    const angle = fromRad + (sweep * (fullTurn && i === chords ? 0 : i)) / chords;
    return { x: centre.x + radiusPx * Math.cos(angle), y: centre.y + radiusPx * Math.sin(angle) };
  });
}

/**
 * A 2D context that draws as `context` does and records each stroke made with it into `strokes`,
 * its path in device px at `ratio` (the transform `paint` sets) and its width.
 *
 * @remarks
 * Only the path calls `paint` makes are traced: `beginPath`, `moveTo`, `lineTo`, `arc` (a line
 * from the current point to its start, as the canvas draws it) and `closePath`. Every other call
 * and every property goes to `context` unchanged, on `context` itself, as a host object needs.
 */
export function recordingContext(
  context: CanvasRenderingContext2D,
  ratio: number,
  strokes: PaintedStroke[],
): CanvasRenderingContext2D {
  let subpaths: Subpath[] = [];
  const device = (args: ReadonlyArray<unknown>, at: number): PointPx => ({
    x: Number(args[at]) * ratio,
    y: Number(args[at + 1]) * ratio,
  });
  const extend = (points: ReadonlyArray<PointPx>): void => {
    const current = subpaths.at(-1);
    if (current === undefined || current.closed) {
      subpaths.push({ points: [...points], closed: false });
    } else {
      current.points.push(...points);
    }
  };
  const hooks: Readonly<Record<string, (args: ReadonlyArray<unknown>) => void>> = {
    beginPath: () => {
      subpaths = [];
    },
    moveTo: (args) => {
      subpaths.push({ points: [device(args, 0)], closed: false });
    },
    lineTo: (args) => {
      extend([device(args, 0)]);
    },
    arc: (args) => {
      extend(arcPoints(device(args, 0), Number(args[2]) * ratio, Number(args[3]), Number(args[4])));
    },
    closePath: () => {
      const current = subpaths.at(-1);
      const first = current?.points[0];
      if (current !== undefined && first !== undefined) {
        current.points.push(first);
        current.closed = true;
      }
    },
    stroke: () => {
      strokes.push({
        subpaths: subpaths.map((subpath) => ({
          points: [...subpath.points],
          closed: subpath.closed,
        })),
        widthPx: context.lineWidth * ratio,
      });
    },
  };
  return new Proxy(context, {
    get(target, key): unknown {
      const value: unknown = Reflect.get(target, key);
      if (typeof value !== "function") {
        return value;
      }
      const hook = typeof key === "string" ? hooks[key] : undefined;
      return (...args: unknown[]): unknown => {
        hook?.(args);
        return Reflect.apply(value, target, args);
      };
    },
    set(target, key, value: unknown): boolean {
      return Reflect.set(target, key, value);
    },
  });
}

/** A painted stroke as the reader takes it, and its cut ends, device px. */
export function screenStrokeOf(
  name: string,
  painted: PaintedStroke,
): { readonly stroke: ScreenStroke; readonly cutEnds: ReadonlyArray<PointPx> } {
  const segments: Array<readonly [PointPx, PointPx]> = [];
  const cutEnds: PointPx[] = [];
  for (const subpath of painted.subpaths) {
    const { points } = subpath;
    for (let i = 1; i < points.length; i += 1) {
      const a = points[i - 1];
      const b = points[i];
      if (a !== undefined && b !== undefined) {
        segments.push([a, b]);
      }
    }
    const first = points[0];
    const last = points.at(-1);
    if (
      !subpath.closed &&
      first !== undefined &&
      last !== undefined &&
      (first.x !== last.x || first.y !== last.y)
    ) {
      cutEnds.push(first, last);
    }
  }
  return {
    stroke: {
      name,
      segments,
      phases: segments.map(() => 0),
      widthPx: painted.widthPx,
      casingWidthPx: 0,
      dash: null,
    },
    cutEnds,
  };
}

/** An 8-bit sRGB code's linear value (IEC 61966-2-1), the inverse of {@link srgb8}. */
export function linearOfCode(code: number): number {
  const c = code / 255;
  return c <= 0.040_45 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** A canvas read back as the reader takes an image: linear RGBA, from its 8-bit sRGB codes. */
function canvasImage(context: CanvasRenderingContext2D): ReadImage {
  const { width, height } = context.canvas;
  const { data } = context.getImageData(0, 0, width, height);
  const colour = new Float32Array(data.length);
  for (let i = 0; i < data.length; i += 1) {
    const code = data[i] ?? 0;
    colour[i] = i % 4 === 3 ? code / 255 : linearOfCode(code);
  }
  return { colour, widthPx: width, heightPx: height };
}

/** A fresh canvas the view's size at `ratio`, and its 2D context. */
function canvasAt(ratio: number): CanvasRenderingContext2D {
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(VIEWPORT.widthPx * ratio);
  canvas.height = Math.round(VIEWPORT.heightPx * ratio);
  const context = canvas.getContext("2d");
  if (context === null) {
    throw new Error("the smoke page gave no 2D context");
  }
  return context;
}

/** One kind's least contrast over the strokes of that kind, and its points. */
interface KindReading {
  readonly kind: string;
  readonly samples: number;
  readonly worst: number;
  readonly at: PointPx | null;
}

/** Folds a stroke's reading into its kind's. */
function fold(
  kinds: Map<string, KindReading>,
  kind: string,
  read: { readonly samples: number; readonly worst: number; readonly at: PointPx | null },
): void {
  const before = kinds.get(kind);
  const keepBefore = before !== undefined && before.worst <= read.worst;
  kinds.set(kind, {
    kind,
    samples: (before?.samples ?? 0) + read.samples,
    worst: keepBefore ? before.worst : read.worst,
    at: keepBefore ? before.at : read.at,
  });
}

/**
 * Reads each painted stroke of a frame, with every other stroke as its neighbour and its cut ends
 * kept a pixel clear, into `kinds`.
 */
function readPainted(
  image: ReadImage,
  painted: ReadonlyArray<PaintedStroke>,
  kindOf: (index: number) => string,
  surfaceLuminance: number,
  kinds: Map<string, KindReading>,
): void {
  const strokes = painted.map((stroke, index) => screenStrokeOf(`stroke ${String(index)}`, stroke));
  const lighting = strokes.map(({ stroke }) =>
    neighbourOf(stroke, false, image.widthPx, image.heightPx, CANVAS_FRINGE_PX),
  );
  strokes.forEach(({ stroke, cutEnds }, index) => {
    const neighbours = lighting.filter((_, other) => other !== index);
    const clearOfEnds = (p: PointPx): boolean =>
      cutEnds.every((end) => Math.hypot(p.x - end.x, p.y - end.y) >= CUT_END_MARGIN_PX);
    fold(
      kinds,
      kindOf(index),
      readStroke(image, stroke, neighbours, surfaceLuminance, clearOfEnds, 0, CANVAS_FRINGE_PX),
    );
  });
}

/** The least points a kind must read at a ratio. */
function floorOf(kind: string, ratio: number): number {
  return kind.startsWith(SPATIAL_KINDS.dataEdgeTicks) ? minTickSamples(ratio) : MIN_SAMPLES;
}

/** A kind's reading in words: its least contrast, where, and its points. */
function shown(reading: KindReading): string {
  const where =
    reading.at === null ? "" : ` at (${reading.at.x.toFixed(1)}, ${reading.at.y.toFixed(1)})`;
  return `${reading.kind} ${reading.worst.toFixed(2)}:1${where}, ${String(reading.samples)} points`;
}

/** The luminance of the device pixel a CSS point falls in, through its 8-bit sRGB code. */
function pixelLuminance(image: ReadImage, at: ScreenPoint, ratio: number): number {
  const [r, g, b] = texel(
    image.colour,
    image.widthPx,
    Math.floor(at.xPx * ratio),
    Math.floor(at.yPx * ratio),
  );
  return wcagLuminance(srgb8(r), srgb8(g), srgb8(b));
}

/**
 * R07.T16.f: the spatial displays' strokes reach 6:1 as drawn at ratios of 0.78125, 1 and 2, in
 * their tokens and in the stale tokens, and the open marks keep their holes; the control, as built,
 * does not reach it. A 2D canvas draws and reads back at once, so the check runs synchronously.
 */
export function checkSpatialStrokeContrast(checks: Checks): void {
  const tokens: ColourTokens = readTokens(document.documentElement);
  const surface = tokenLuminance(tokens.surface0);
  for (const ratio of SPATIAL_RATIOS) {
    const kinds = new Map<string, KindReading>();
    const holes: string[] = [];
    const apart: Array<{ readonly style: string; readonly clear: number }> = [];
    const mismatched: string[] = [];
    const widths = new Set<number>();
    for (const [style, painted] of [
      ["", tokens],
      [", stale", staleTokens(tokens)],
    ] as const) {
      for (const frame of spatialFrames(ratio)) {
        const context = canvasAt(ratio);
        const strokes: PaintedStroke[] = [];
        paint(
          recordingContext(context, ratio, strokes),
          { ops: frame.ops.map(({ op }) => op), anchors: [], curveLabels: [] },
          painted,
          ratio,
        );
        // `paint` strokes each op once, in order, so the strokes and the ops align.
        if (strokes.length !== frame.ops.length) {
          mismatched.push(
            `${frame.name}${style}: ${String(strokes.length)} strokes for ${String(frame.ops.length)} ops`,
          );
          continue;
        }
        for (const stroke of strokes) {
          widths.add(Math.round(stroke.widthPx * 1000) / 1000);
        }
        const image = canvasImage(context);
        readPainted(
          image,
          strokes,
          (index) => `${frame.ops[index]?.kind ?? "?"}${style}`,
          surface,
          kinds,
        );
        for (const hole of frame.holes) {
          const clear = pixelLuminance(image, hole, ratio) === surface;
          holes.push(
            `(${hole.xPx.toFixed(2)}, ${hole.yPx.toFixed(2)})${style} ${clear ? "--surface-0" : "lit"}`,
          );
        }
        const bracket =
          strokes[frame.ops.findIndex(({ kind }) => kind === SPATIAL_KINDS.pairBracket)];
        const destination =
          strokes[frame.ops.findIndex(({ kind }) => kind === SPATIAL_KINDS.pairDestination)];
        if (bracket !== undefined && destination !== undefined) {
          apart.push({ style, clear: clearBetween(image, bracket, destination, surface) });
        }
      }
    }
    const expected = Object.values(SPATIAL_KINDS).flatMap((kind) => [kind, `${kind}, stale`]);
    const readings = expected.map(
      (kind): KindReading =>
        kinds.get(kind) ?? { kind, samples: 0, worst: Number.POSITIVE_INFINITY, at: null },
    );
    checks.check(
      `R07.T16.f every spatial stroke reaches 6:1 as drawn at a ratio of ${String(ratio)}, in its tokens and the stale tokens (lines ${String(lineScale(ratio))} px per CSS px, outlines ${String(markStrokeDevicePx(ratio))} px, reticles ${String(minReticleGapDevicePx(ratio))} px apart at least)`,
      mismatched.length === 0 &&
        readings.every(
          (reading) =>
            reading.samples >= floorOf(reading.kind, ratio) && reading.worst >= REQUIRED_RATIO,
        ),
      [
        ...mismatched,
        `widths drawn ${[...widths].toSorted((a, b) => a - b).join(", ")} device px`,
        ...readings.map(shown),
      ].join("; "),
    );
    checks.check(
      `R07.T16.f the open class-0 circle's centre and the ringed circle's disc centre read --surface-0 at a ratio of ${String(ratio)}`,
      holes.length === 4 && holes.every((hole) => hole.endsWith("--surface-0")),
      holes.join("; "),
    );
    checks.check(
      `R07.T16.f the destination's reticle about the selection stands apart from the bracket, --surface-0 between their arms, at a ratio of ${String(ratio)}`,
      apart.length === 2 && apart.every(({ clear }) => clear >= 1),
      apart.map(({ style, clear }) => `${String(clear)} px clear${style}`).join("; "),
    );
  }
  checkControl(checks, tokens, surface);
}

/**
 * The pixels reading `--surface-0` between the pair's left arms, along the row through the middle
 * of the bracket's upper left arm (decision-r07-t16d-followups, item 2): the least gap is there
 * for separation, so that the two reticles never read as one two-coloured bracket.
 *
 * @remarks
 * Each reticle's first traced subpath is its upper left corner: the arm's free end below the
 * corner, the corner, the other arm's end. The destination's arm, a third of a larger side, spans
 * the bracket's arm's middle row.
 */
function clearBetween(
  image: ReadImage,
  bracket: PaintedStroke,
  destination: PaintedStroke,
  surface: number,
): number {
  const [armEnd, corner] = bracket.subpaths[0]?.points ?? [];
  const outer = destination.subpaths[0]?.points[1];
  if (armEnd === undefined || corner === undefined || outer === undefined) {
    return 0;
  }
  const row = Math.floor((armEnd.y + corner.y) / 2);
  let clear = 0;
  for (let column = Math.floor(outer.x); column <= Math.floor(corner.x); column += 1) {
    const [r, g, b] = texel(image.colour, image.widthPx, column, row);
    if (wcagLuminance(srgb8(r), srgb8(g), srgb8(b)) === surface) {
      clear += 1;
    }
  }
  return clear;
}

/** The control: a 1 CSS px `--text-muted` circle stroked directly at 0.78125, as P05 built it. */
function checkControl(checks: Checks, tokens: ColourTokens, surface: number): void {
  const [ratio] = SPATIAL_RATIOS;
  const context = canvasAt(ratio);
  const strokes: PaintedStroke[] = [];
  const recorder = recordingContext(context, ratio, strokes);
  recorder.setTransform(1, 0, 0, 1, 0, 0);
  recorder.fillStyle = tokens.surface0;
  recorder.fillRect(0, 0, context.canvas.width, context.canvas.height);
  recorder.setTransform(ratio, 0, 0, ratio, 0, 0);
  recorder.beginPath();
  recorder.arc(CENTRE.xPx, CENTRE.yPx, 100, 0, 2 * Math.PI);
  recorder.strokeStyle = tokens.textMuted;
  recorder.lineWidth = 1;
  recorder.stroke();
  const kinds = new Map<string, KindReading>();
  const kind = "1 CSS px circle as built, --text-muted";
  readPainted(canvasImage(context), strokes, () => kind, surface, kinds);
  const control = kinds.get(kind);
  checks.check(
    `R07.T16.f the control, a 1 CSS px circle stroked directly at a ratio of ${String(ratio)} (0.78 device px), reads under 6:1`,
    control !== undefined && control.samples >= MIN_SAMPLES && control.worst < REQUIRED_RATIO,
    control === undefined ? "no reading" : shown(control),
  );
}
