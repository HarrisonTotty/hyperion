/**
 * The smoke page's check of R10.T9.b's engine work: a depth-only target of array layers, each
 * layer drawn by a pass of its own, read back through a comparison sampler, with the capability
 * path of `unclippedDepth` and a clamped depth bias.
 *
 * @remarks
 * A caster quad covers the left half of a 64² layer at a depth its pass's projection sets, and
 * discards the layer's lower half; a receiver draws a full-screen pass that compares a reference
 * depth against one layer through a linear `greater-equal` comparison sampler, 2 × 2 PCF, and
 * writes the result. Read back, the result is 1 where the reference is at least the stored depth
 * and 0 where a caster nearer the light occludes it, as the cascades' receivers read them. The
 * layers stand at different depths, so a pass that read another pass's `Frame` would show.
 * `just test-render`'s SwiftShader offers `depth-clip-control`, so the page's default variant
 * takes the unclipped path, `unclippedDepth` made; `no-subgroups` withholds the feature, so there
 * `unclippedDepth` must be refused, the path the cascades take with their near plane pulled back
 * (decision-r10-t9-split §5(g)).
 */

import type { AllocationEvent } from "../view/engine/memory";
import type {
  DepthLayerPass,
  DepthLayerTarget,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  WgslMaterialSpec,
} from "../view/engine/types";
import FRAME_WGSL from "../view/shaders/frame.wgsl?raw";
import {
  type Checks,
  frameOf,
  fullScreenMesh,
  halfTexels,
  IDENTITY,
  near,
  show,
  texel,
} from "./harness";

/** Texels on a side of each layer, and of the receiver's target. */
const SIZE_PX = 64;

/** The layers: four, as the cascades have. */
const LAYERS = 4;

/**
 * A caster: positions through the pass's `Frame`; no colour, and the lower half of the layer
 * discarded, so that a depth-only pipeline's kept fragment stage shows.
 */
const CASTER_WGSL = `${FRAME_WGSL}
struct Draw {
  offsetFromCameraM : vec3f,
}
@group(1) @binding(0) var<uniform> draw : Draw;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return frame.clipProjection * (frame.viewRotation * vec4f(position + draw.offsetFromCameraM, 1.0));
}

@fragment fn fragmentMain(@builtin(position) position : vec4f) {
  if (position.y > frame.viewport.y * 0.5) {
    discard;
  }
}
`;

/** A receiver: 1 where `reference` is at least layer `layer`'s stored depth, 0 where it is less. */
const RECEIVER_WGSL = `${FRAME_WGSL}
struct Draw {
  offsetFromCameraM : vec3f,
  layer : u32,
  reference : f32,
}
@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var layers : texture_depth_2d_array;
@group(2) @binding(1) var layersSampler : sampler_comparison;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return frame.clipProjection * (frame.viewRotation * vec4f(position + draw.offsetFromCameraM, 1.0));
}

@fragment fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let uv = position.xy * frame.viewport.zw;
  let lit = textureSampleCompareLevel(layers, layersSampler, uv, draw.layer, draw.reference);
  return vec4f(lit, lit, lit, 1.0);
}
`;

const CASTER: WgslMaterialSpec = {
  name: "R10.T9.b caster",
  displayName: "TEST CASTER",
  vertexWgsl: CASTER_WGSL,
  fragmentWgsl: CASTER_WGSL,
  uniforms: [],
  samplers: [],
  cullMode: "none",
  depthWrite: true,
  colourWrites: false,
  blend: "none",
};

const RECEIVER: WgslMaterialSpec = {
  name: "R10.T9.b receiver",
  displayName: "TEST RECEIVER",
  vertexWgsl: RECEIVER_WGSL,
  fragmentWgsl: RECEIVER_WGSL,
  uniforms: [
    { name: "layer", type: "u32" },
    { name: "reference", type: "f32" },
  ],
  samplers: [
    {
      name: "layersSampler",
      filter: "linear",
      address: "clamp-to-edge",
      binding: 1,
      compare: "greater-equal",
    },
  ],
  textures: [{ name: "layers", binding: 0, sampleType: "depth", viewDimension: "2d-array" }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The receiver's texels the check reads: under the caster, beside it, and where it discarded. */
const POINTS = {
  covered: [16, 16],
  open: [48, 16],
  discarded: [16, 48],
} as const;

/** A comparison and what it gives at each point. */
interface Case {
  readonly layer: number;
  readonly reference: number;
  readonly covered: 0 | 1;
}

/**
 * The comparisons, after layer 0 was drawn at 0.75 and then cleared by an empty pass, layer 1 drawn
 * at 0.25 with a clamped bias, layer 2 at 0.5, and layer 3 never drawn: away from the caster every
 * layer reads 0, its clear, so every reference is lit there.
 */
const CASES: ReadonlyArray<Case> = [
  { layer: 2, reference: 0.625, covered: 1 },
  { layer: 2, reference: 0.375, covered: 0 },
  { layer: 1, reference: 0.375, covered: 1 },
  { layer: 1, reference: 0.125, covered: 0 },
  { layer: 0, reference: 0.125, covered: 1 },
  { layer: 3, reference: 0.125, covered: 1 },
];

/** A reversed orthographic projection with no depth slope: every point at clip depth `depth`. */
function flatDepth(depth: number): Float32Array {
  return new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, depth, 1]);
}

/** A pass into `layer` of `draws` at clip depth `depth`. */
function layerPass(layer: number, depth: number, draws: ReadonlyArray<DrawItem>): DepthLayerPass {
  return {
    label: `R10.T9.b layer ${layer}`,
    layer,
    viewRotation: IDENTITY,
    projection: flatDepth(depth),
    draws,
  };
}

/** A draw of `mesh` with `material` at the origin. */
function castOf(mesh: MeshHandle, material: MaterialHandle): DrawItem {
  return { mesh, material, offsetFromCameraM: new Float32Array(3), uniforms: {}, textures: {} };
}

/** What a material's creation gave: `made`, or the error's message. */
async function outcomeOf(made: Promise<MaterialHandle>): Promise<string> {
  try {
    await made;
    return "made";
  } catch (error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }
}

/** R10.T9.b: a depth-only target of array layers and a comparison sampler; see the module's remarks. */
export async function checkDepthLayers(engine: RenderEngine, checks: Checks): Promise<void> {
  const clipControl = engine.capabilities.depthClipControl;
  checks.check(
    "R10.T9.b the cascades' capability path",
    true,
    `depth-clip-control ${clipControl ? "yes: unclippedDepth" : "no: the near plane pulled back"}`,
  );

  let shadowBytes = 0;
  const stopTally = engine.onAllocation((event: AllocationEvent) => {
    if (event.kind !== "uploaded" && event.category === "shadows") {
      shadowBytes += event.kind === "created" ? event.bytes : -event.bytes;
    }
  });
  // Whatever a check or the device throws, the group lets its targets and its listener go.
  let heldTarget: DepthLayerTarget | null = null;
  let heldOut: RenderTarget | null = null;
  try {
    // The left half of the layer, x in [-1, 0] of clip space, its whole height.
    const quad = engine.createMesh({
      name: "R10.T9.b caster quad",
      positions: new Float32Array([-1, -1, 0, 0, -1, 0, 0, 1, 0, -1, -1, 0, 0, 1, 0, -1, 1, 0]),
      indices: null,
      topology: "triangle-list",
      attributes: {},
    });
    const caster = await engine.createMaterialAsync(CASTER, ["depth-only"], [quad]);
    const clampedMade = engine.createMaterialAsync(
      {
        ...CASTER,
        name: "R10.T9.b clamped caster",
        depthBiasAway: { constant: 1, slopeScale: 1, clamp: 0.001 },
      },
      ["depth-only"],
      [quad],
    );
    const clampedOutcome = await outcomeOf(clampedMade);
    checks.check(
      "R10.T9.b a caster with a clamped bias is made for depth-only",
      clampedOutcome === "made",
      clampedOutcome,
    );
    // Refused, the group ends here, its check above already failed.
    const clamped = await clampedMade;

    const unclipped = await outcomeOf(
      engine.createMaterialAsync(
        { ...CASTER, name: "R10.T9.b unclipped caster", unclippedDepth: true },
        ["depth-only"],
        [quad],
      ),
    );
    checks.check(
      `R10.T9.b unclippedDepth is ${clipControl ? "made with" : "refused without"} depth-clip-control`,
      clipControl ? unclipped === "made" : /depth-clip-control/u.test(unclipped),
      unclipped,
    );

    const target = engine.createDepthLayers({
      name: "R10.T9.b layers",
      sizePx: SIZE_PX,
      layers: LAYERS,
      category: "shadows",
    });
    heldTarget = target;
    const layerBytes = SIZE_PX * SIZE_PX * LAYERS * 4;
    checks.check(
      "R10.T9.b the layers are counted under shadows",
      shadowBytes === layerBytes,
      `${shadowBytes} B, expected ${layerBytes} B`,
    );
    target.render("smoke", [
      layerPass(0, 0.75, [castOf(quad, caster)]),
      layerPass(1, 0.25, [castOf(quad, clamped)]),
      layerPass(2, 0.5, [castOf(quad, caster)]),
    ]);
    // An empty pass clears its layer, and leaves the others as they were.
    target.render("smoke", [layerPass(0, 0.75, [])]);

    const screen = fullScreenMesh(engine, "R10.T9.b receiver screen");
    const receiver = await engine.createMaterialAsync(RECEIVER, ["rgba16float"], [screen]);
    const out = engine.createRenderTarget({
      name: "R10.T9.b receiver",
      size: { widthPx: SIZE_PX, heightPx: SIZE_PX },
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    heldOut = out;
    for (const { layer, reference, covered } of CASES) {
      out.render(
        frameOf(`R10.T9.b receiver ${layer}`, [
          {
            mesh: screen,
            material: receiver,
            offsetFromCameraM: new Float32Array(3),
            uniforms: {
              layer: new Float32Array([layer]),
              reference: new Float32Array([reference]),
            },
            textures: { layers: target.depth },
          },
        ]),
      );
      // The harness's checks run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = halfTexels(await engine.readTexture(out.colour));
      const at = (point: readonly [number, number]): readonly number[] =>
        texel(texels, SIZE_PX, point[0], point[1]);
      const seen = {
        covered: at(POINTS.covered),
        open: at(POINTS.open),
        discarded: at(POINTS.discarded),
      };
      checks.check(
        `R10.T9.b layer ${layer} at reference ${reference}: ${covered === 1 ? "lit" : "occluded"} under the caster, lit beside it and where it discarded`,
        near(seen.covered, [covered, covered, covered, 1], 1e-3) &&
          near(seen.open, [1, 1, 1, 1], 1e-3) &&
          near(seen.discarded, [1, 1, 1, 1], 1e-3),
        `under ${show(seen.covered)}; beside ${show(seen.open)}; discarded ${show(seen.discarded)}`,
      );
    }

    out.dispose();
    heldOut = null;
    target.dispose();
    heldTarget = null;
    checks.check(
      "R10.T9.b the layers' bytes are released at disposal",
      shadowBytes === 0,
      `${shadowBytes} B left under shadows`,
    );
  } finally {
    heldOut?.dispose();
    heldTarget?.dispose();
    stopTally();
  }
}
