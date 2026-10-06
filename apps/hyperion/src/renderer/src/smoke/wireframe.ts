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
 * of its PSF-weighted colour (both added in RM1 validation, m2 and m3); and a hull edge cased as
 * the photorealistic overlay draws it stays whole over its own receding face (R07.T16.a).
 */

import "../styles.css";

import { dot, normalise, scale, sub, vec3 } from "../geometry/vec3";
import { DEFAULT_FOV_DEG, NEAR_PLANE_M } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { KernelPair } from "../view/engine/kernels";
import type { RenderEngine, RenderTarget } from "../view/engine/types";
import { controlEv100, DEFAULT_EXPOSURE } from "../view/photometry/exposure";
import { erf, PSF_QUAD_PX, PSF_SIGMA_PX } from "../view/photometry/magnitude";
import { type Rgb, spriteToneCurve, toneCurve } from "../view/photometry/toneCurve";
import toneCurveWgsl from "../view/shaders/toneCurve.wgsl?raw";
import { overlayDrawList } from "../view/photoreal/overlay";
import {
  buildWireframeDrawList,
  type DrawCamera,
  HULL_OCCLUDER_BIAS,
  type LineBatch,
  type OccluderMesh,
  type StarSprite,
  STROKE_PX,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import { linearColour, WireframeRenderer } from "../view/wireframe/submit";
import { type ColourTokens, readTokens } from "../spatial/paint";
import { runPose, SCENE_OPTIONS, startRun } from "../displays/view/viewRun";
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

/** An empty draw list. */
const NOTHING: WireframeDrawList = {
  occluderSpheres: [],
  occluderMeshes: [],
  lines: [],
  sprites: [],
  anchors: [],
};

/** Renders `list` from the axis camera into a fresh square target and reads colour and depth. */
async function drawSquare(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  name: string,
  list: WireframeDrawList,
): Promise<{ readonly colour: Float32Array; readonly depth: Float32Array }> {
  const target = squareTarget(engine, name);
  target.render(renderer.frame(list, AXIS_CAMERA, { widthPx: SIDE_PX, heightPx: SIDE_PX }));
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
        depthBiasAway: HULL_OCCLUDER_BIAS,
        twoSided: true,
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
          depthBiasAway: HULL_OCCLUDER_BIAS,
          twoSided: true,
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

  await checkSphereSlope(engine, renderer, checks);
  await checkCasedHullEdge(engine, renderer, tokens, checks);
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
 * R07.T16.a: a hull edge cased by the photorealistic overlay (`overlayDrawList`), on the far edge
 * of its own face as the face recedes towards it, draws every texel it draws with no face, its
 * casing's outer texel included: the hull faces' slope bias, 3 px, covers the cased edge's
 * 2.25 px of coverage where the depth's slope runs along the screen's axes, as here (Design note
 * 5's w ÷ 2 + 1, the UX decisions, item 12). The face's depth changes only down the view, so one
 * column reads for all.
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
    depthBiasAway: HULL_OCCLUDER_BIAS,
    twoSided: true,
  };
  const onFace = await drawSquare(
    engine,
    renderer,
    "R07 cased hull edge",
    overlayDrawList({ ...NOTHING, occluderMeshes: [recedingFace], lines: [edge] }),
  );
  const bare = await drawSquare(
    engine,
    renderer,
    "R07 bare cased hull edge",
    overlayDrawList({ ...NOTHING, lines: [edge] }),
  );
  // The column through the edge's middle, from 4 px above the edge to 4 px below its casing.
  const rows = Array.from({ length: 10 }, (_, i) => 20 + i);
  // A texel that could not be read differs, so that a short readback fails the check.
  const differs = rows.filter((row) => {
    const seen = texel(onFace.colour, SIDE_PX, 32, row);
    const want = texel(bare.colour, SIDE_PX, 32, row);
    return [0, 1, 2].some((c) => !(Math.abs((seen[c] ?? Number.NaN) - (want[c] ?? 0)) <= 2e-3));
  });
  // The control: the casing's outer texel, 2.15 px below the edge, is drawn with no face.
  const outer = texel(bare.colour, SIDE_PX, 32, 26);
  checks.check(
    "R07.T16.a a cased hull edge on its own receding face draws every texel it draws with no face",
    differs.length === 0 && outer[1] > 0.02,
    `rows that differ ${differs.length === 0 ? "none" : differs.join(", ")}; the casing's outer texel with the face ${show(texel(onFace.colour, SIDE_PX, 32, 26))}, with none ${show(outer)}`,
  );
}

/** The sphere occluder's slope check: its centre straight ahead, m, and its radius, m. */
const SLOPE_SPHERE = { distanceM: 10, radiusM: 6 } as const;

/** A texel off both axes, where the depth's screen gradient runs diagonally (column, row). */
const DIAGONAL_TEXEL = [44, 20] as const;

/**
 * The depth `occluderSphere.wgsl` should write at a texel of the axis camera's square view: the
 * exact ray-sphere depth less `SLOPE_SCALE` (3) px of the depth's screen slope, its magnitude or,
 * for the old form, its larger component.
 */
function sphereDepth(column: number, row: number, slope: "length" | "max"): number {
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
  return depth - 3 * (slope === "length" ? Math.hypot(slopeX, slopeY) : Math.max(slopeX, slopeY));
}

/**
 * RM1 validation m2: a body's occluder sphere pushes its depth away by the magnitude of its screen
 * slope, so that a graticule stroke's whole cased width stays in front where the slope runs
 * diagonally (Design note 5).
 */
async function checkSphereSlope(
  engine: RenderEngine,
  renderer: WireframeRenderer,
  checks: Checks,
): Promise<void> {
  const { distanceM, radiusM } = SLOPE_SPHERE;
  const drawn = await drawSquare(engine, renderer, "R02 sphere slope", {
    ...NOTHING,
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
  const want = sphereDepth(column, row, "length");
  const old = sphereDepth(column, row, "max");
  checks.check(
    "R02.T14 a sphere occluder's depth is pushed by its slope's magnitude where the slope is diagonal",
    Math.abs(seen - want) < 0.1 * Math.abs(old - want),
    `depth ${seen.toPrecision(7)}, by the magnitude ${want.toPrecision(7)}, by the larger component ${old.toPrecision(7)}`,
  );
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
