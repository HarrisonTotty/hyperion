/**
 * The smoke page's wireframe checks (plan R02, R02.T14.c): R02's shaders, submitted through
 * `WireframeRenderer` as the `VIEW` display submits them, read back on the run's adapter.
 *
 * @remarks
 * Properties only, never a stored picture: the first frame of each kept scene is finite with depth
 * 0 where nothing was drawn; a cased stroke's neighbouring rows are `--surface-0`; the tone curve's
 * WGSL at 64 luminances equals its TypeScript twin within 10⁻⁵; a face at the near plane reads depth
 * near 1 in the upper half of the view (no half-Z conversion, no Y flip); a hull face shows its
 * own edge and hides one 4 × 10⁻⁵ of the distance behind it, at 1 m and at 10⁸ m (Design note 5); a
 * sphere occluder's depth is pushed by its slope's magnitude where the slope is diagonal; a
 * star sprite peaks in its own pixel, lights nothing outside its quad and sums to the tone curve
 * of its PSF-weighted colour (both added in RM1 validation, m2 and m3); a hull edge cased as
 * the photorealistic overlay draws it stays whole over its own receding face (R07.T16.a), on a
 * slope along an axis and at 45° to the axes, at stroke scales of 1 and 2; a hull face's depth is
 * pushed in its fragment by its slope's magnitude (R07.T16.d); and over a loaded colour a hull's
 * silhouette reads `--surface-0` inside its faces and leaves the image beyond them and behind a
 * body in front, its own cased edges stay whole over it, and an edge behind a window draws as with
 * no window, in both styles (R07.T16.e). Both occluders' slope term is the list's
 * `occluderSlopePx`, a uniform that follows the stroke scale.
 */

import "../styles.css";

import { cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { DEFAULT_FOV_DEG, NEAR_PLANE_M, project } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { KernelPair } from "../view/engine/kernels";
import type { RenderEngine, RenderTarget } from "../view/engine/types";
import { controlEv100, DEFAULT_EXPOSURE } from "../view/photometry/exposure";
import { erf, PSF_QUAD_PX, PSF_SIGMA_PX } from "../view/photometry/magnitude";
import { type Rgb, spriteToneCurve, toneCurve } from "../view/photometry/toneCurve";
import toneCurveWgsl from "../view/shaders/toneCurve.wgsl?raw";
import { overlayDrawList } from "../view/photoreal/overlay";
import { SYMBOL_STROKE_PX } from "../spatial/symbols";
import {
  buildWireframeDrawList,
  CASING_PX,
  type DrawCamera,
  emptyDrawList,
  HULL_OCCLUDER_DEPTH_FRACTION,
  type LineBatch,
  type OccluderMesh,
  type StarSprite,
  STROKE_PX,
  viewStrokesAt,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import { linearColour, WireframeRenderer } from "../view/wireframe/submit";
import { type ColourTokens, readTokens } from "../spatial/paint";
import { runPose, SCENE_OPTIONS, startRun } from "../displays/view/viewRun";
import { aViewCraft, aViewScene, FIXTURE_SYSTEM, NO_TURN } from "../test/viewFixtures";
import { type HullOutline, hullOutline } from "../view/scene/hull";
import { type Checks, halfTexels, show, texel } from "./harness";

/** The side of the square targets the checks draw into, px. */
const SIDE_PX = 64;

/** The kept scenes' target, px: the display's 16 : 9 at a small size. */
const SCENE_SIZE = { widthPx: 640, heightPx: 360 } as const;

/** The hidden edge's distance behind the face, as a fraction of the face's distance (T14.c). */
const HIDDEN_EDGE_FRACTION = 4e-5;

/** The tone curve's agreement with its twin (T14.c). */
const TONE_TOLERANCE = 1e-5;

/** The luminances the tone curve is compared at: 64, log-spaced from 10⁻⁶ to 10³. */
const TONE_INPUTS = Float32Array.from({ length: 64 }, (_, i) => 10 ** (-6 + (9 * i) / 63));

/** The tone curve as a compute kernel: `agx` of a grey of each input, its first channel. */
const TONE_KERNEL: KernelPair = {
  name: "R02 tone curve twin",
  reference: `${toneCurveWgsl}
struct Params { count : u32 }
@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var<storage, read> inputs : array<f32>;
@group(0) @binding(2) var<storage, read_write> outputs : array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x < params.count) {
    outputs[id.x] = agx(vec3f(inputs[id.x])).x;
  }
}
`,
  subgroup: null,
  readback: "bit-exact",
};

/** A square target with depth. */
function squareTarget(engine: RenderEngine, name: string): RenderTarget {
  return engine.createRenderTarget({
    name,
    size: { widthPx: SIDE_PX, heightPx: SIDE_PX },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
}

/** A camera at the system's origin, looking down −z with +y up, its field 90° across. */
const AXIS_CAMERA: DrawCamera = {
  pose: {
    frame: { kind: "system", system: "0200080020000000" },
    positionM: vec3(0, 0, 0),
    orientation: IDENTITY_QUATERNION,
  },
  fovXRad: Math.PI / 2,
};

/** The view-space height, as a fraction of the distance, of row `row`'s centre in the square. */
function rowHeight(row: number): number {
  return 1 - (2 * (row + 0.5)) / SIDE_PX;
}

/** A line batch of `segments` in view space, uncased unless `casingPx` is given. */
function batch(
  id: string,
  segments: ReadonlyArray<number>,
  colour: string,
  widthPx: number,
  casing: { readonly px: number; readonly colour: string } | null = null,
): LineBatch {
  return {
    id,
    space: "view",
    originF32: new Float32Array(3),
    segments: new Float32Array(segments),
    token: "text",
    colour,
    widthPx,
    casingWidthPx: casing?.px ?? 0,
    casingColour: casing?.colour ?? colour,
    dash: null,
  };
}

/**
 * An empty draw list at a stroke scale: its `occluderSlopePx` is what the occluders read (3 at a
 * scale of 1, 5 at 2).
 */
function nothingAt(strokeScale: number): WireframeDrawList {
  return emptyDrawList({ strokeScale, markStrokePx: SYMBOL_STROKE_PX * strokeScale });
}

/** An empty draw list, as a view at a ratio of 1 draws: lines at 2 device px per CSS px. */
const NOTHING: WireframeDrawList = emptyDrawList(viewStrokesAt(1));

/**
 * Renders `list` from the axis camera into a fresh square target and reads colour and depth: onto
 * the cleared target, or with `under` over a target first filled with that colour and loaded, as
 * the symbology's pass loads the tone-mapped image (R07.T16.e).
 */
async function drawSquare(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  name: string,
  list: WireframeDrawList,
  under: string | null = null,
): Promise<{ readonly colour: Float32Array; readonly depth: Float32Array }> {
  const target = squareTarget(engine, name);
  const square = { widthPx: SIDE_PX, heightPx: SIDE_PX };
  if (under === null) {
    target.render(renderer.frame(list, AXIS_CAMERA, square));
  } else {
    // The image's stand-in: one uncased screen-space stroke wider than the square.
    const image: LineBatch = {
      ...batch(
        "image",
        [-SIDE_PX, SIDE_PX / 2, 0, 2 * SIDE_PX, SIDE_PX / 2, 0],
        under,
        4 * SIDE_PX,
      ),
      space: "screen",
    };
    target.render(renderer.frame({ ...NOTHING, lines: [image] }, AXIS_CAMERA, square));
    target.render({ ...renderer.frame(list, AXIS_CAMERA, square), colourLoad: "load" });
  }
  const colour = halfTexels(await engine.readTexture(target.colour));
  const depth = new Float32Array(await engine.readTexture(target.depth ?? target.colour));
  target.dispose();
  return { colour, depth };
}

/** Two triangles of the square of half-side `halfM` at distance `distanceM` down −z. */
function face(distanceM: number, halfM: number, bottomM = -halfM): Float32Array {
  const z = -distanceM;
  return new Float32Array([
    -halfM,
    bottomM,
    z,
    halfM,
    bottomM,
    z,
    halfM,
    halfM,
    z,
    -halfM,
    bottomM,
    z,
    halfM,
    halfM,
    z,
    -halfM,
    halfM,
    z,
  ]);
}

/** Whether `seen` matches `want` in its colour channels, within half-float rounding of a token. */
function sameColour(seen: ReadonlyArray<number>, want: Float32Array): boolean {
  return [0, 1, 2].every((c) => Math.abs((seen[c] ?? Number.NaN) - (want[c] ?? 0)) < 2e-3);
}

/** R02.T14.c on the run's adapter. */
export async function checkWireframe(engine: RenderEngine, checks: Checks): Promise<void> {
  const tokens = readTokens(document.documentElement);
  const renderer = new WireframeRenderer(engine);

  // The first frame of each kept scene, as the display draws it.
  for (const option of SCENE_OPTIONS) {
    const kept = option.make();
    const run = startRun(kept);
    const camera: DrawCamera = { pose: runPose(run), fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180 };
    const list = buildWireframeDrawList(run.scene, camera, SCENE_SIZE, tokens, {
      lowSetting: false,
      ev100: controlEv100(DEFAULT_EXPOSURE),
      selection: null,
      destination: null,
      remPx: 16,
      ...viewStrokesAt(1),
    });
    const target = engine.createRenderTarget({
      name: `R02 ${option.name}`,
      size: SCENE_SIZE,
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    target.render(renderer.frame(list, camera, SCENE_SIZE));
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const colour = halfTexels(await engine.readTexture(target.colour));
    // oxlint-disable-next-line no-await-in-loop
    const depth = new Float32Array(await engine.readTexture(target.depth ?? target.colour));
    target.dispose();
    const lit = colour.filter((value, index) => index % 4 !== 3 && value > 0).length;
    checks.check(
      `R02.T14.c ${option.name}: every texel of the first frame finite`,
      colour.every(Number.isFinite) && depth.every(Number.isFinite) && lit > 0,
      `${String(lit)} lit channels of ${String(colour.length)}`,
    );
    checks.check(
      `R02.T14.c ${option.name}: depth 0 where nothing was drawn`,
      depth[0] === 0,
      `corner depth ${String(depth[0])}`,
    );
  }

  // A cased 1 px stroke along row 24: its own row the stroke, the rows either side the casing.
  const strokeY = rowHeight(24);
  const cased = await drawSquare(engine, renderer, "R02 casing", {
    ...NOTHING,
    lines: [
      batch("cased", [-0.5, strokeY, -1, 0.5, strokeY, -1], tokens.text, 1, {
        px: 1,
        colour: tokens.surface0,
      }),
    ],
  });
  const surface = linearColour(tokens.surface0);
  const text = linearColour(tokens.text);
  const above = texel(cased.colour, SIDE_PX, 32, 23);
  const on = texel(cased.colour, SIDE_PX, 32, 24);
  const below = texel(cased.colour, SIDE_PX, 32, 25);
  checks.check(
    "R02.T14.c a cased stroke's neighbouring rows are --surface-0, its own --text",
    sameColour(above, surface) && sameColour(below, surface) && sameColour(on, text),
    `above ${show(above)}, on ${show(on)}, below ${show(below)}; surface ${show(surface)}, text ${show(text)}`,
  );

  // A face at 1.0001 × the near plane over the upper half of the view only.
  const nearM = 0.1 * 1.0001;
  const upper = await drawSquare(engine, renderer, "R02 near face", {
    ...NOTHING,
    occluderMeshes: [
      {
        id: "near face",
        originF32: new Float32Array(3),
        triangles: face(nearM, nearM, 0),
        twoSided: true,
        fill: null,
      },
    ],
  });
  const upperDepth = upper.depth[16 * SIDE_PX + 32] ?? Number.NaN;
  const lowerDepth = upper.depth[48 * SIDE_PX + 32] ?? Number.NaN;
  checks.check(
    "R02.T14.c a face at the near plane reads depth near 1 in the upper half, +y up, no half-Z",
    upperDepth > 0.999 && upperDepth <= 1 && lowerDepth === 0,
    `upper ${String(upperDepth)}, lower ${String(lowerDepth)}`,
  );

  // The tone curve's WGSL against its TypeScript twin at 64 luminances.
  const kernel = await engine.createComputeAsync(TONE_KERNEL);
  const inputs = engine.createBuffer({
    name: "R02 tone inputs",
    bytes: TONE_INPUTS.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(inputs, 0, TONE_INPUTS);
  const outputs = engine.createBuffer({
    name: "R02 tone outputs",
    bytes: TONE_INPUTS.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  engine.dispatch(
    kernel,
    {
      uniforms: { params: new Uint32Array([TONE_INPUTS.length]) },
      buffers: { inputs, outputs },
      sampled: {},
      storage: {},
    },
    [1, 1, 1],
    TONE_KERNEL.name,
  );
  const gpu = new Float32Array(await engine.readBuffer(outputs));
  const worst = Math.max(
    ...Array.from(TONE_INPUTS, (l, i) =>
      Math.abs((gpu[i] ?? Number.NaN) - toneCurve([l, l, l])[0]),
    ),
  );
  checks.check(
    "R02.T14.c the tone curve's WGSL equals its twin at 64 luminances within 1e-5",
    worst <= TONE_TOLERANCE,
    `largest difference ${worst.toExponential(2)}`,
  );

  // A hull face with its own edge, and an edge 4 × 10⁻⁵ of the distance behind it.
  for (const distanceM of [1, 1e8]) {
    const ownY = rowHeight(24) * distanceM;
    const behindM = distanceM * (1 + HIDDEN_EDGE_FRACTION);
    const behindY = rowHeight(40) * behindM;
    const half = 0.25 * distanceM;
    const lines = [
      batch("own edge", [-half, ownY, -distanceM, half, ownY, -distanceM], "#ff0000", 2),
      batch("hidden edge", [-half, behindY, -behindM, half, behindY, -behindM], "#ff0000", 2),
    ];
    // oxlint-disable-next-line no-await-in-loop
    const edges = await drawSquare(engine, renderer, `R02 edges ${String(distanceM)}`, {
      ...NOTHING,
      occluderMeshes: [
        {
          id: "hull face",
          originF32: new Float32Array(3),
          triangles: face(distanceM, 0.5 * distanceM),
          twoSided: true,
          fill: null,
        },
      ],
      lines,
    });
    // The control: with no face, the edge behind is drawn where the check reads it.
    // oxlint-disable-next-line no-await-in-loop
    const bare = await drawSquare(engine, renderer, `R02 bare edges ${String(distanceM)}`, {
      ...NOTHING,
      lines,
    });
    const own = texel(edges.colour, SIDE_PX, 32, 24);
    const hidden = texel(edges.colour, SIDE_PX, 32, 40);
    const shown = texel(bare.colour, SIDE_PX, 32, 40);
    checks.check(
      `R02.T14.c at ${distanceM.toExponential(0)} m a face shows its own edge and hides one 4e-5 behind`,
      own[0] > 0.5 && hidden[0] === 0 && shown[0] > 0.5,
      `own ${show(own)}, behind ${show(hidden)}, behind with no face ${show(shown)}`,
    );
  }

  for (const strokeScale of [1, 2]) {
    // The checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    await checkSphereSlope(engine, renderer, checks, strokeScale);
    // The checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    await checkHullSlope(engine, renderer, checks, strokeScale);
  }
  await checkCasedHullEdge(engine, renderer, tokens, checks);
  for (const strokeScale of [1, 2]) {
    // The checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    await checkDiagonalCasedHullEdge(engine, renderer, tokens, checks, strokeScale);
  }
  await checkSilhouette(engine, renderer, tokens, checks);
  await checkWindow(engine, renderer, tokens, checks);
  await checkStarSprite(engine, renderer, checks);
  renderer.dispose();
}

/**
 * The cased hull edge's check (R07.T16.a): where its edge falls on the view, px down from the top,
 * a little above a row's centre so that the casing's last partial texel lies 2.15 px below it
 * (the cased 3.5 px stroke's coverage reaches 2.25 px), past the 2 px the hull faces' old slope
 * scale covered; and the face's near edge, px, at half the distance.
 */
const CASED_EDGE = { edgeYPx: 24.35, nearYPx: 40, farM: 1, nearM: 0.5 } as const;

/**
 * How much farther the cased edge checks' control face stands than the face itself: a power of
 * two, so that every corner, scaled about the camera, and its projection are exact in `f32`, and
 * the control's silhouette fills the very texels the face's does, behind the edge (R07.T16.e).
 */
const BEHIND_FACTOR = 4;

/**
 * A face's triangles moved {@link BEHIND_FACTOR} times as far from the axis camera: the same
 * silhouette on the square, standing well behind anything drawn at the face's own depth.
 */
function behind(mesh: OccluderMesh): OccluderMesh {
  return {
    ...mesh,
    id: `${mesh.id} behind`,
    triangles: mesh.triangles.map((value) => value * BEHIND_FACTOR),
  };
}

/**
 * R07.T16.a: a hull edge cased by the photorealistic overlay (`overlayDrawList`), on the far edge
 * of its own face as the face recedes towards it, draws every texel it draws with the face behind
 * it, its casing's outer texel included: the hull faces' slope term, 3 px at a stroke scale of 1,
 * covers the cased edge's 2.25 px of coverage where the depth's slope runs along the screen's
 * axes, as here (Design note 5's w ÷ 2 + 1, the UX decisions, item 12). The face's depth changes
 * only down the view, so one column reads for all. R07.T16.d's {@link checkDiagonalCasedHullEdge}
 * turns it 45°.
 *
 * @remarks
 * Since R07.T16.e the overlay fills the face, a `--surface-0` silhouette, over a loaded colour, as
 * the symbology's pass draws over the image. So the control is the same face {@link behind} the
 * edge, whose silhouette fills the same texels, rather than no face, whose texels the casing's
 * partial coverage would blend over the loaded colour instead.
 */
async function checkCasedHullEdge(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  tokens: ColourTokens,
  checks: Checks,
): Promise<void> {
  const { edgeYPx, nearYPx, farM, nearM } = CASED_EDGE;
  // The height over the distance of a point `yPx` down from the top of the square view.
  const heightOverDistance = (yPx: number): number => 1 - (2 * yPx) / SIDE_PX;
  const farYM = heightOverDistance(edgeYPx) * farM;
  const nearYM = heightOverDistance(nearYPx) * nearM;
  const farHalfM = 0.25 * farM;
  const nearHalfM = 0.25 * nearM;
  // Two triangles of the face, from its far edge, the hull's edge, to its near edge.
  const triangles = new Float32Array([
    -farHalfM,
    farYM,
    -farM,
    farHalfM,
    farYM,
    -farM,
    nearHalfM,
    nearYM,
    -nearM,
    -farHalfM,
    farYM,
    -farM,
    nearHalfM,
    nearYM,
    -nearM,
    -nearHalfM,
    nearYM,
    -nearM,
  ]);
  const edgeHalfM = 0.2 * farM;
  // A hull's edge as the wireframe's list has it, uncased at the heavy stroke, cased by the
  // overlay; its casing here in `--accent`, not `--surface-0`, so that its faint outer texel reads.
  const edge = {
    ...batch(
      "cased hull edge",
      [-edgeHalfM, farYM, -farM, edgeHalfM, farYM, -farM],
      tokens.text,
      STROKE_PX.heavy,
    ),
    casingColour: tokens.accent,
  };
  const recedingFace: OccluderMesh = {
    id: "receding face",
    originF32: new Float32Array(3),
    triangles,
    twoSided: true,
    fill: null,
  };
  const atOne = nothingAt(1);
  // Over the image's stand-in, a colour neither the edge, its casing nor the silhouette takes.
  const under = tokens.textMuted;
  const onFace = await drawSquare(
    engine,
    renderer,
    "R07 cased hull edge",
    overlayDrawList({ ...atOne, occluderMeshes: [recedingFace], lines: [edge] }, tokens),
    under,
  );
  const bare = await drawSquare(
    engine,
    renderer,
    "R07 cased hull edge, the face behind it",
    overlayDrawList({ ...atOne, occluderMeshes: [behind(recedingFace)], lines: [edge] }, tokens),
    under,
  );
  // The column through the edge's middle, from 4 px above the edge to 4 px below its casing.
  const rows = Array.from({ length: 10 }, (_, i) => 20 + i);
  // A texel that could not be read differs, so that a short readback fails the check.
  const differs = rows.filter((row) => {
    const seen = texel(onFace.colour, SIDE_PX, 32, row);
    const want = texel(bare.colour, SIDE_PX, 32, row);
    return [0, 1, 2].some((c) => !(Math.abs((seen[c] ?? Number.NaN) - (want[c] ?? 0)) <= 2e-3));
  });
  // The control: the casing's outer texel, 2.15 px below the edge, is drawn with the face behind.
  const outer = texel(bare.colour, SIDE_PX, 32, 26);
  checks.check(
    "R07.T16.a a cased hull edge on its own receding face, filled over the image, draws every texel it draws with the face behind it (R07.T16.e)",
    differs.length === 0 && outer[1] > 0.02,
    `rows that differ ${differs.length === 0 ? "none" : differs.join(", ")}; the casing's outer texel with the face ${show(texel(onFace.colour, SIDE_PX, 32, 26))}, with it behind ${show(outer)}`,
  );
}

/** The sphere occluder's slope check: its centre straight ahead, m, and its radius, m. */
const SLOPE_SPHERE = { distanceM: 10, radiusM: 6 } as const;

/** A texel off both axes, where the depth's screen gradient runs diagonally (column, row). */
const DIAGONAL_TEXEL = [44, 20] as const;

/**
 * The depth `occluderSphere.wgsl` should write at a texel of the axis camera's square view: the
 * exact ray-sphere depth less `slopePx` (the list's `occluderSlopePx`) px of the depth's screen
 * slope, its magnitude or, for the old form, its larger component.
 */
function sphereDepth(
  column: number,
  row: number,
  slope: "length" | "max",
  slopePx: number,
): number {
  const n = NEAR_PLANE_M;
  // The axis camera's 90° field: s = 1 in both axes of the square view.
  const ndcX = ((column + 0.5) / SIDE_PX) * 2 - 1;
  const ndcY = 1 - ((row + 0.5) / SIDE_PX) * 2;
  const ray = normalise(vec3(ndcX, ndcY, -1));
  const centre = vec3(0, 0, -SLOPE_SPHERE.distanceM);
  const along = dot(centre, ray);
  const perp = sub(centre, scale(ray, along));
  const h2 = SLOPE_SPHERE.radiusM ** 2 - dot(perp, perp);
  const t = along - Math.sqrt(h2);
  const depth = n / (t * -ray.z);
  const hit = scale(ray, t);
  const normal = scale(sub(hit, centre), 1 / SLOPE_SPHERE.radiusM);
  const facing = Math.abs(dot(normal, hit));
  const slopeX = (n * Math.abs(normal.x) * 2) / (SIDE_PX * facing);
  const slopeY = (n * Math.abs(normal.y) * 2) / (SIDE_PX * facing);
  return (
    depth - slopePx * (slope === "length" ? Math.hypot(slopeX, slopeY) : Math.max(slopeX, slopeY))
  );
}

/**
 * RM1 validation m2: a body's occluder sphere pushes its depth away by the magnitude of its screen
 * slope, so that a graticule stroke's whole cased width stays in front where the slope runs
 * diagonally (Design note 5); by the list's `occluderSlopePx`, a uniform, at each stroke scale
 * (R07.T16.d).
 */
async function checkSphereSlope(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  checks: Checks,
  strokeScale: number,
): Promise<void> {
  const { distanceM, radiusM } = SLOPE_SPHERE;
  const list = nothingAt(strokeScale);
  const drawn = await drawSquare(engine, renderer, `R02 sphere slope ${String(strokeScale)}`, {
    ...list,
    occluderSpheres: [
      {
        id: "sphere",
        centreF32: new Float32Array([0, 0, -distanceM]),
        radiusM,
        altitudeM: distanceM - radiusM,
      },
    ],
  });
  const [column, row] = DIAGONAL_TEXEL;
  const seen = drawn.depth[row * SIDE_PX + column] ?? Number.NaN;
  const want = sphereDepth(column, row, "length", list.occluderSlopePx);
  const old = sphereDepth(column, row, "max", list.occluderSlopePx);
  checks.check(
    `R02.T14 a sphere occluder's depth is pushed by ${String(list.occluderSlopePx)} px of its slope's magnitude where the slope is diagonal (stroke scale ${String(strokeScale)})`,
    Math.abs(seen - want) < 0.1 * Math.abs(old - want),
    `depth ${seen.toPrecision(7)}, by the magnitude ${want.toPrecision(7)}, by the larger component ${old.toPrecision(7)}`,
  );
}

/** A point turned by `angleRad` about the view axis (−z): the picture turned on the screen. */
function turned(p: Vec3, angleRad: number): Vec3 {
  const c = Math.cos(angleRad);
  const s = Math.sin(angleRad);
  return vec3(p.x * c - p.y * s, p.x * s + p.y * c, p.z);
}

/** The angle the diagonal checks turn their face by: 45°, so that its slope runs diagonally. */
const DIAGONAL_RAD = Math.PI / 4;

/**
 * Where the turned face's far edge lies before the turn, px down the view. A 45° line meets the
 * texel centres at one sub-pixel phase along its length, 1 ÷ √2 px apart across it, so this places
 * one 2.190 px from the edge on the face's side, inside the band a larger-component push loses at a
 * stroke scale of 1 (2.121 to 2.25 px, the casing's coverage 0.06 there), and so one 3.604 px from
 * it, inside the band at 2 (3.536 to 4.0 px, coverage 0.40).
 */
const DIAGONAL_EDGE_Y_PX = 24.153;

/**
 * R07.T16.a's receding face turned by `angleRad`: its far edge, the hull's, at 1 m, `edgeYPx` down
 * the view before the turn, and its near edge 40 px down at 0.5 m, so that its depth's gradient
 * runs at `angleRad` to the screen's vertical; as two triangles, with the hull's edge on its far
 * side, 0.2 m each way.
 */
function recedingFaceAt(
  angleRad: number,
  edgeYPx: number,
): {
  readonly triangles: Float32Array;
  /** Its far edge's two corners, then its near edge's, turned. */
  readonly corners: readonly [Vec3, Vec3, Vec3, Vec3];
  readonly edge: readonly [Vec3, Vec3];
} {
  const { nearYPx, farM, nearM } = CASED_EDGE;
  const heightOverDistance = (yPx: number): number => 1 - (2 * yPx) / SIDE_PX;
  const farYM = heightOverDistance(edgeYPx) * farM;
  const nearYM = heightOverDistance(nearYPx) * nearM;
  const corners = [
    turned(vec3(-0.25 * farM, farYM, -farM), angleRad),
    turned(vec3(0.25 * farM, farYM, -farM), angleRad),
    turned(vec3(0.25 * nearM, nearYM, -nearM), angleRad),
    turned(vec3(-0.25 * nearM, nearYM, -nearM), angleRad),
  ] as const;
  const [a, b, c, d] = corners;
  const triangles = new Float32Array([a, b, c, a, c, d].flatMap((p) => [p.x, p.y, p.z]));
  const edge = [
    turned(vec3(-0.2 * farM, farYM, -farM), angleRad),
    turned(vec3(0.2 * farM, farYM, -farM), angleRad),
  ] as const;
  return { triangles, corners, edge };
}

/** The axis camera's projection, for the square view. */
const SQUARE_PROJECTION = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 2 } as const;

/** The square view's size. */
const SQUARE = { widthPx: SIDE_PX, heightPx: SIDE_PX } as const;

/** A point's place on the square view, px from its top left. */
function onSquare(p: Vec3): { readonly x: number; readonly y: number } {
  const at = project(p, SQUARE_PROJECTION, SQUARE);
  return { x: at.xPx, y: at.yPx };
}

/**
 * The reversed-Z depth, `f64`, where the axis camera's ray through the square view's point
 * (`xPx`, `yPx`) meets the plane through `corners`: n ÷ the distance along −z.
 */
function planeDepth(corners: ReadonlyArray<Vec3>, xPx: number, yPx: number): number {
  const [a, b, c] = corners;
  if (a === undefined || b === undefined || c === undefined) {
    throw new Error("a plane needs three corners");
  }
  const normal = cross(sub(b, a), sub(c, a));
  // The 90° field: one unit across the view's half-width at unit distance.
  const ray = vec3((2 * xPx) / SIDE_PX - 1, 1 - (2 * yPx) / SIDE_PX, -1);
  const t = dot(normal, a) / dot(normal, ray);
  return NEAR_PLANE_M / t;
}

/**
 * R07.T16.d: a hull face's depth, which `occluder.wgsl` writes in its fragment, is its rasterised
 * depth times 1 − 2⁻¹⁶ less `occluderSlopePx` px of its screen slope's magnitude, computed here in
 * `f64` at a texel of a face whose slope runs at 45° to the axes; within a tenth of its difference
 * from the push by the slope's larger component, which a hardware bias may take.
 */
async function checkHullSlope(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  checks: Checks,
  strokeScale: number,
): Promise<void> {
  const list = nothingAt(strokeScale);
  const turnedFace = recedingFaceAt(DIAGONAL_RAD, DIAGONAL_EDGE_Y_PX);
  const drawn = await drawSquare(engine, renderer, `R07 hull slope ${String(strokeScale)}`, {
    ...list,
    occluderMeshes: [
      {
        id: "diagonal face",
        originF32: new Float32Array(3),
        triangles: turnedFace.triangles,
        twoSided: true,
        fill: null,
      },
    ],
  });
  // The texel at the face's middle, well inside it.
  const middle = onSquare(
    scale(
      turnedFace.corners.reduce(
        (sum, p) => vec3(sum.x + p.x, sum.y + p.y, sum.z + p.z),
        vec3(0, 0, 0),
      ),
      0.25,
    ),
  );
  const column = Math.floor(middle.x);
  const row = Math.floor(middle.y);
  const at = (dx: number, dy: number): number =>
    planeDepth(turnedFace.corners, column + 0.5 + dx, row + 0.5 + dy);
  // Depth is affine in the screen across a plane, so a central difference is its exact slope.
  const slopeX = (at(1, 0) - at(-1, 0)) / 2;
  const slopeY = (at(0, 1) - at(0, -1)) / 2;
  const kept = at(0, 0) * (1 - HULL_OCCLUDER_DEPTH_FRACTION);
  const want = kept - list.occluderSlopePx * Math.hypot(slopeX, slopeY);
  const old = kept - list.occluderSlopePx * Math.max(Math.abs(slopeX), Math.abs(slopeY));
  const seen = drawn.depth[row * SIDE_PX + column] ?? Number.NaN;
  checks.check(
    `R07.T16.d a hull face's depth is pushed by ${String(list.occluderSlopePx)} px of its slope's magnitude where the slope is diagonal (stroke scale ${String(strokeScale)})`,
    Math.abs(seen - want) < 0.1 * Math.abs(old - want),
    `texel (${String(column)}, ${String(row)}): depth ${seen.toPrecision(7)}, by the magnitude ${want.toPrecision(7)}, by the larger component ${old.toPrecision(7)}`,
  );
}

/**
 * R07.T16.d: T16.a's cased hull edge on its own receding face, turned 45° so that the face's depth
 * gradient runs diagonally on the screen, at a stroke scale: it draws every texel of the square that
 * it draws with the face behind it, the texels that a push by the slope's larger component would
 * lose included, those whose centres lie between `occluderSlopePx` ÷ √2 and the casing's reach,
 * w ÷ 2 + 0.5, from the edge on the face's side (0 to 0.13 of the casing at a scale of 1, 0 to
 * 0.46 at 2). The casing is in `--accent` here, not `--surface-0`, so that those texels read.
 * Since R07.T16.e the face is filled over a loaded colour, so the control is the same face
 * {@link behind} the edge rather than no face, as in {@link checkCasedHullEdge}.
 */
async function checkDiagonalCasedHullEdge(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  tokens: ColourTokens,
  checks: Checks,
  strokeScale: number,
): Promise<void> {
  const list = nothingAt(strokeScale);
  const turnedFace = recedingFaceAt(DIAGONAL_RAD, DIAGONAL_EDGE_Y_PX);
  const [a, b] = turnedFace.edge;
  const edge = {
    ...batch(
      "diagonal cased hull edge",
      [a.x, a.y, a.z, b.x, b.y, b.z],
      tokens.text,
      STROKE_PX.heavy * strokeScale,
    ),
    casingColour: tokens.accent,
  };
  const mesh: OccluderMesh = {
    id: "diagonal receding face",
    originF32: new Float32Array(3),
    triangles: turnedFace.triangles,
    twoSided: true,
    fill: null,
  };
  const name = `R07 diagonal cased hull edge ${String(strokeScale)}`;
  const under = tokens.textMuted;
  const onFace = await drawSquare(
    engine,
    renderer,
    name,
    overlayDrawList({ ...list, occluderMeshes: [mesh], lines: [edge] }, tokens),
    under,
  );
  const bare = await drawSquare(
    engine,
    renderer,
    `${name}, the face behind it`,
    overlayDrawList({ ...list, occluderMeshes: [behind(mesh)], lines: [edge] }, tokens),
    under,
  );
  // Every texel of the square, a short readback included, as the axis check reads its column.
  const differs: string[] = [];
  for (let row = 0; row < SIDE_PX; row += 1) {
    for (let column = 0; column < SIDE_PX; column += 1) {
      const seen = texel(onFace.colour, SIDE_PX, column, row);
      const want = texel(bare.colour, SIDE_PX, column, row);
      if ([0, 1, 2].some((c) => !(Math.abs((seen[c] ?? Number.NaN) - (want[c] ?? 0)) <= 2e-3))) {
        differs.push(`(${String(column)}, ${String(row)})`);
      }
    }
  }
  // The texels at risk: on the face's side of the edge, within its middle, between the larger
  // component's push and the casing's reach, each with the casing's coverage there.
  const from = onSquare(a);
  const to = onSquare(b);
  const lengthPx = Math.hypot(to.x - from.x, to.y - from.y);
  const along = { x: (to.x - from.x) / lengthPx, y: (to.y - from.y) / lengthPx };
  const [, , nearRight, nearLeft] = turnedFace.corners;
  const nearEdge = onSquare(
    scale(vec3(nearRight.x + nearLeft.x, nearRight.y + nearLeft.y, nearRight.z + nearLeft.z), 0.5),
  );
  const side = Math.sign(-along.y * (nearEdge.x - from.x) + along.x * (nearEdge.y - from.y));
  const normal = { x: -along.y * side, y: along.x * side };
  const reachPx = ((STROKE_PX.heavy + 2 * CASING_PX) * strokeScale) / 2 + 0.5;
  const fromPx = list.occluderSlopePx / Math.SQRT2;
  const accent = linearColour(tokens.accent)[1] ?? Number.NaN;
  const atRisk: string[] = [];
  const unread: string[] = [];
  for (let row = 0; row < SIDE_PX; row += 1) {
    for (let column = 0; column < SIDE_PX; column += 1) {
      const dx = column + 0.5 - from.x;
      const dy = row + 0.5 - from.y;
      const t = dx * along.x + dy * along.y;
      const d = dx * normal.x + dy * normal.y;
      if (t > 0.2 * lengthPx && t < 0.8 * lengthPx && d > fromPx && d < reachPx) {
        const coverage = reachPx - d;
        atRisk.push(`(${String(column)}, ${String(row)}) ${coverage.toFixed(3)}`);
        // A texel with a casing worth reading must read it with the face behind.
        const green = texel(bare.colour, SIDE_PX, column, row)[1];
        if (coverage >= 0.03 && !(green >= 0.5 * coverage * accent)) {
          unread.push(`(${String(column)}, ${String(row)}) ${show([green])}`);
        }
      }
    }
  }
  checks.check(
    `R07.T16.d a cased hull edge on its own face, its slope at 45° to the axes, filled over the image, draws every texel it draws with the face behind it (stroke scale ${String(strokeScale)}, slope term ${String(list.occluderSlopePx)} px; R07.T16.e)`,
    differs.length === 0 && atRisk.length >= 3 && unread.length === 0,
    `texels that differ ${differs.length === 0 ? "none" : differs.slice(0, 12).join(", ")}; texels a larger-component push would lose ${String(atRisk.length)} (${atRisk.slice(0, 6).join(", ")}); unread with the face behind ${unread.length === 0 ? "none" : unread.join(", ")}`,
  );
}

/**
 * R07.T16.e's silhouette: a hull face, its distance and half-side, m, over texels 16 to 48 of the
 * axis camera's square each way; and a body's occluder sphere in front of the face's upper right,
 * its centre from the camera and its radius, m, about 4.4 px in radius on the square.
 */
const SILHOUETTE = {
  face: { distanceM: 4, halfM: 2 },
  sphere: { centreM: vec3(0.6, 0.6, -2), radiusM: 0.3 },
} as const;

/** How far a texel's centre must lie from an aliased edge for the silhouette check to read it, px. */
const EDGE_CLEARANCE_PX = 2;

/**
 * How far the ray through texel (`column`, `row`) of the axis camera's square passes outside the
 * sphere's limb, px on the square: negative on its disc. Its angle from the limb times half the
 * square's side, which is a unit of tangent across the 90° field: no more than the distance on
 * the square off the axis, so that a clearance in it is a clearance on the square.
 */
function pastLimbPx(column: number, row: number, centreM: Vec3, radiusM: number): number {
  const ray = normalise(
    vec3((2 * (column + 0.5)) / SIDE_PX - 1, 1 - (2 * (row + 0.5)) / SIDE_PX, -1),
  );
  const along = dot(centreM, ray);
  const perpM = norm(sub(centreM, scale(ray, along)));
  return ((perpM - radiusM) / along) * (SIDE_PX / 2);
}

/** What the silhouette check reads at a texel: the kind of place, and the colour it must be. */
interface SilhouetteReading {
  readonly kind: "inside" | "behindBody" | "beyond";
  readonly want: Float32Array;
}

/**
 * What texel (`column`, `row`) must read in the silhouette check, or `null` for one too near an
 * edge or the limb: the image on the body's disc and beyond the face, `--surface-0` inside it.
 */
function silhouetteReading(
  column: number,
  row: number,
  colours: { readonly surface: Float32Array; readonly image: Float32Array },
): SilhouetteReading | null {
  const { face: square, sphere } = SILHOUETTE;
  // The face's edges on the square, px: its half-side over its distance, half the side a unit.
  const halfPx = (square.halfM / square.distanceM) * (SIDE_PX / 2);
  const from = SIDE_PX / 2 - halfPx;
  const to = SIDE_PX / 2 + halfPx;
  const centres = [column + 0.5, row + 0.5];
  const inFace = centres.every((c) => c > from + EDGE_CLEARANCE_PX && c < to - EDGE_CLEARANCE_PX);
  const offFace = centres.some((c) => c < from - EDGE_CLEARANCE_PX || c > to + EDGE_CLEARANCE_PX);
  const limbPx = pastLimbPx(column, row, sphere.centreM, sphere.radiusM);
  let reading: SilhouetteReading | null = null;
  if (limbPx < -EDGE_CLEARANCE_PX) {
    reading = { kind: "behindBody", want: colours.image };
  } else if (inFace && limbPx > EDGE_CLEARANCE_PX) {
    reading = { kind: "inside", want: colours.surface };
  } else if (offFace) {
    reading = { kind: "beyond", want: colours.image };
  }
  return reading;
}

/**
 * R07.T16.e: over a loaded colour, the overlay's silhouette of a hull face reads `--surface-0` at
 * its interior texels, and leaves the loaded colour beyond its edges and where a body's occluder
 * sphere stands in front of it (decision-r07-t16a, item 3): the spheres are drawn first, and the
 * silhouette is depth-tested against them. Its own edges are aliased, and lie under its cased
 * outline in a view, so texels within {@link EDGE_CLEARANCE_PX} of an edge or the limb are not
 * read.
 */
async function checkSilhouette(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  tokens: ColourTokens,
  checks: Checks,
): Promise<void> {
  const { face: square, sphere } = SILHOUETTE;
  const mesh: OccluderMesh = {
    id: "silhouette",
    originF32: new Float32Array(3),
    triangles: face(square.distanceM, square.halfM),
    twoSided: true,
    fill: null,
  };
  const { x, y, z } = sphere.centreM;
  const distanceM = Math.hypot(x, y, z);
  const list = overlayDrawList(
    {
      ...NOTHING,
      occluderSpheres: [
        {
          id: "body in front",
          centreF32: new Float32Array([x, y, z]),
          radiusM: sphere.radiusM,
          altitudeM: distanceM - sphere.radiusM,
        },
      ],
      occluderMeshes: [mesh],
    },
    tokens,
  );
  const under = tokens.text;
  const drawn = await drawSquare(engine, renderer, "R07 silhouette", list, under);
  const surface = linearColour(tokens.surface0);
  const image = linearColour(under);
  const tally = { inside: 0, behindBody: 0, beyond: 0 };
  const wrong: string[] = [];
  for (let row = 0; row < SIDE_PX; row += 1) {
    for (let column = 0; column < SIDE_PX; column += 1) {
      const reading = silhouetteReading(column, row, { surface, image });
      if (reading === null) {
        continue;
      }
      tally[reading.kind] += 1;
      const seen = texel(drawn.colour, SIDE_PX, column, row);
      if (!sameColour(seen, reading.want)) {
        wrong.push(`${reading.kind} (${String(column)}, ${String(row)}) ${show(seen)}`);
      }
    }
  }
  checks.check(
    "R07.T16.e over the image a hull face's silhouette reads --surface-0 inside it, and leaves the image beyond its edges and behind a body in front of it",
    wrong.length === 0 && tally.inside > 400 && tally.behindBody >= 10 && tally.beyond > 1000,
    `texels read inside ${String(tally.inside)}, behind the body ${String(tally.behindBody)}, beyond ${String(tally.beyond)}; wrong ${wrong.length === 0 ? "none" : wrong.slice(0, 8).join(", ")}; --surface-0 ${show(surface)}, the image ${show(image)}`,
  );
}

/**
 * R07.T16.e's window hull, at the axis camera and unturned: a window 0.5 m square 1 m ahead, over
 * texels 24 to 40 of the square each way; an opaque panel beside it, over texels 3 to 16 across;
 * and an edge 2 m ahead behind the window, across its middle at 31.2 px down, from 25.6 to 38.4 px
 * across. The window's two faces are its first.
 */
const WINDOW_HULL: HullOutline = hullOutline({
  name: "WINDOW TEST",
  vertices: [
    vec3(-0.25, -0.25, -1),
    vec3(0.25, -0.25, -1),
    vec3(0.25, 0.25, -1),
    vec3(-0.25, 0.25, -1),
    vec3(-0.9, -0.25, -1),
    vec3(-0.5, -0.25, -1),
    vec3(-0.5, 0.25, -1),
    vec3(-0.9, 0.25, -1),
    vec3(-0.4, 0.05, -2),
    vec3(0.4, 0.05, -2),
  ],
  edges: [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [8, 9],
  ],
  faces: [
    [0, 1, 2],
    [0, 2, 3],
    [4, 5, 6],
    [4, 6, 7],
  ],
  windows: [0, 1],
  eyePointM: vec3(0, 0, 0),
  lengthM: 2,
});

/** The texels at the core of the window hull's edge behind its window: its middle row, clear of its ends. */
const BEHIND_WINDOW_CORE = Array.from({ length: 8 }, (_, i) => [28 + i, 31] as const);

/**
 * The window hull's draw list, from the axis camera, through `buildWireframeDrawList`, as a view
 * at a ratio of 1 draws it: the hull the only thing in its scene.
 */
function windowList(hull: HullOutline, tokens: ColourTokens): WireframeDrawList {
  const scene = aViewScene({
    bodies: [],
    stars: [],
    ownShip: null,
    craft: [
      aViewCraft({
        id: "window test",
        designation: hull.name,
        hull,
        pose: {
          position: { kind: "system", system: FIXTURE_SYSTEM, m: vec3(0, 0, 0) },
          attitude: NO_TURN,
        },
      }),
    ],
  });
  return buildWireframeDrawList(scene, AXIS_CAMERA, SQUARE, tokens, {
    lowSetting: false,
    ev100: 0,
    selection: null,
    destination: null,
    remPx: 16,
    ...viewStrokesAt(1),
  });
}

/**
 * R07.T16.e: a hull's cased edge behind its window draws as with no window, in each style: the
 * window's faces are in no mesh, so they hide nothing, and every texel of the square is as it is
 * with those faces taken out of the hull (decision-r07-t16a, item 3). Over the image, through the
 * overlay over a loaded colour; in the wireframe, on the cleared target. The control makes the
 * window an opaque face, which hides the edge's core.
 */
async function checkWindow(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  tokens: ColourTokens,
  checks: Checks,
): Promise<void> {
  const noWindow: HullOutline = { ...WINDOW_HULL, faces: WINDOW_HULL.faces.slice(2), windows: [] };
  const opaque: HullOutline = { ...WINDOW_HULL, windows: [] };
  const stroke = linearColour(tokens.text);
  for (const style of ["overlay", "wireframe"] as const) {
    const listOf = (hull: HullOutline): WireframeDrawList =>
      style === "overlay"
        ? overlayDrawList(windowList(hull, tokens), tokens)
        : windowList(hull, tokens);
    const under = style === "overlay" ? tokens.textMuted : null;
    const name = `R07 window ${style}`;
    // The checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const glazed = await drawSquare(engine, renderer, name, listOf(WINDOW_HULL), under);
    // The checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const bare = await drawSquare(engine, renderer, `${name}, none`, listOf(noWindow), under);
    // The checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const shut = await drawSquare(engine, renderer, `${name}, opaque`, listOf(opaque), under);
    let differs = 0;
    for (let row = 0; row < SIDE_PX; row += 1) {
      for (let column = 0; column < SIDE_PX; column += 1) {
        const seen = texel(glazed.colour, SIDE_PX, column, row);
        const want = texel(bare.colour, SIDE_PX, column, row);
        if ([0, 1, 2].some((c) => !(Math.abs((seen[c] ?? Number.NaN) - (want[c] ?? 0)) <= 2e-3))) {
          differs += 1;
        }
      }
    }
    const drawnCore = BEHIND_WINDOW_CORE.filter(([column, row]) =>
      sameColour(texel(glazed.colour, SIDE_PX, column, row), stroke),
    ).length;
    const hiddenCore = BEHIND_WINDOW_CORE.filter(
      ([column, row]) => !sameColour(texel(shut.colour, SIDE_PX, column, row), stroke),
    ).length;
    const [column, row] = BEHIND_WINDOW_CORE[0] ?? [0, 0];
    checks.check(
      `R07.T16.e a hull's ${style === "overlay" ? "cased " : ""}edge behind its window draws as with no window, ${style === "overlay" ? "over the image" : "in the wireframe"}`,
      differs === 0 &&
        drawnCore === BEHIND_WINDOW_CORE.length &&
        hiddenCore === BEHIND_WINDOW_CORE.length,
      `texels that differ from no window ${String(differs)}; the edge's core drawn at ${String(drawnCore)} of ${String(BEHIND_WINDOW_CORE.length)} texels, hidden by an opaque window at ${String(hiddenCore)}; (${String(column)}, ${String(row)}) ${show(texel(glazed.colour, SIDE_PX, column, row))}, opaque ${show(texel(shut.colour, SIDE_PX, column, row))}`,
    );
  }
}

/** The sprite check's star: where it falls, px, off its pixel's centre, and its colour per weight. */
const SPRITE = { xPx: 32.3, yPx: 20.8, exposedRgb: [2, 1.2, 0.8] } as const satisfies {
  readonly xPx: number;
  readonly yPx: number;
  readonly exposedRgb: Rgb;
};

/** The share of a unit Gaussian's light, centred at `centrePx`, in the pixel whose centre is `p`. */
function pixelShare(p: number, centrePx: number): number {
  const k = 1 / (PSF_SIGMA_PX * Math.SQRT2);
  return (erf((p + 0.5 - centrePx) * k) - erf((p - 0.5 - centrePx) * k)) / 2;
}

/**
 * RM1 validation m3: one star sprite at a sub-pixel position, drawn on the GPU: its peak in its
 * own pixel, nothing outside its 7 × 7 quad, and each texel the sprite's tone curve of its
 * PSF-weighted colour, summed over the quad within half-float rounding.
 */
async function checkStarSprite(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  checks: Checks,
): Promise<void> {
  const sprite: StarSprite = {
    id: "star",
    directionF32: new Float32Array([0, 0, -1]),
    xPx: SPRITE.xPx,
    yPx: SPRITE.yPx,
    exposedRgb: SPRITE.exposedRgb,
    illuminanceLx: 1,
  };
  const bare = await drawSquare(engine, renderer, "R02 no sprite", NOTHING);
  const lit = await drawSquare(engine, renderer, "R02 sprite", { ...NOTHING, sprites: [sprite] });
  const ownColumn = Math.floor(SPRITE.xPx);
  const ownRow = Math.floor(SPRITE.yPx);
  const half = (PSF_QUAD_PX - 1) / 2;
  let peak = { value: -1, column: -1, row: -1 };
  let outside = 0;
  const seenSum = [0, 0, 0];
  const wantSum = [0, 0, 0];
  for (let row = 0; row < SIDE_PX; row += 1) {
    for (let column = 0; column < SIDE_PX; column += 1) {
      const a = texel(lit.colour, SIDE_PX, column, row);
      const b = texel(bare.colour, SIDE_PX, column, row);
      const added = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
      const inQuad = Math.abs(column - ownColumn) <= half && Math.abs(row - ownRow) <= half;
      if (!inQuad) {
        outside += added.filter((value) => value !== 0).length;
        continue;
      }
      const weight = pixelShare(column + 0.5, SPRITE.xPx) * pixelShare(row + 0.5, SPRITE.yPx);
      const want = spriteToneCurve([
        SPRITE.exposedRgb[0] * weight,
        SPRITE.exposedRgb[1] * weight,
        SPRITE.exposedRgb[2] * weight,
      ]);
      for (const c of [0, 1, 2] as const) {
        seenSum[c] = (seenSum[c] ?? 0) + (added[c] ?? 0);
        wantSum[c] = (wantSum[c] ?? 0) + want[c];
      }
      if ((added[0] ?? 0) > peak.value) {
        peak = { value: added[0] ?? 0, column, row };
      }
    }
  }
  checks.check(
    "R02.T14 a star sprite peaks in its own pixel and lights nothing outside its 7 x 7 quad",
    peak.column === ownColumn && peak.row === ownRow && outside === 0,
    `peak at (${String(peak.column)}, ${String(peak.row)}), its pixel (${String(ownColumn)}, ${String(ownRow)}); ${String(outside)} channels lit outside`,
  );
  const worst = Math.max(
    ...[0, 1, 2].map((c) => Math.abs((seenSum[c] ?? 0) / (wantSum[c] ?? 1) - 1)),
  );
  checks.check(
    "R02.T14 a star sprite's summed texels are the tone curve of its PSF-weighted colour within 0.5%",
    worst < 5e-3,
    `summed ${show(seenSum)}, expected ${show(wantSum)}`,
  );
}
