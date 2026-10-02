/**
 * The smoke page's wireframe checks (plan R02, R02.T14.c): R02's shaders, submitted through
 * `WireframeRenderer` as the `VIEW` display submits them, read back on the run's adapter.
 *
 * @remarks
 * Properties only, never a stored picture: the first frame of each kept scene is finite with depth
 * 0 where nothing was drawn; a cased stroke's neighbouring rows are `--surface-0`; the tone curve's
 * WGSL at 64 luminances equals its TypeScript twin within 10⁻⁵; a face at the near plane reads depth
 * near 1 in the upper half of the view (no half-Z conversion, no Y flip); and a hull face shows its
 * own edge and hides one 4 × 10⁻⁵ of the distance behind it, at 1 m and at 10⁸ m (Design note 5).
 */

import "../styles.css";

import { vec3 } from "../geometry/vec3";
import { DEFAULT_FOV_DEG } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { KernelPair } from "../view/engine/kernels";
import type { RenderEngine, RenderTarget } from "../view/engine/types";
import { controlEv100, DEFAULT_EXPOSURE } from "../view/photometry/exposure";
import { toneCurve } from "../view/photometry/toneCurve";
import toneCurveWgsl from "../view/shaders/toneCurve.wgsl?raw";
import {
  buildWireframeDrawList,
  type DrawCamera,
  HULL_OCCLUDER_BIAS,
  type LineBatch,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import { linearColour, WireframeRenderer } from "../view/wireframe/submit";
import { readTokens } from "../spatial/paint";
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
  renderer.dispose();
}
