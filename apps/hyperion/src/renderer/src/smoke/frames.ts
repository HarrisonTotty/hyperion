/**
 * The smoke page's frame checks: a clear and a triangle into a target and a view (T9.a), three
 * canvases on one device (T9.d), and depth, culling and bias (T9.f).
 */

import type { RenderEngine, RenderView, ViewSize } from "../view/engine/types";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  near,
  NEAR_M,
  show,
  texel,
  triangleMesh,
} from "./harness";

/** A canvas in the page, for a view. */
export function addCanvas(): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  document.body.append(canvas);
  return canvas;
}

/** The RGBA bytes of pixel (x, y) of a view's read-back `widthPx` wide. */
function pixel(bytes: Uint8Array | Float32Array, widthPx: number, x: number, y: number): number[] {
  return Array.from(texel(bytes, widthPx, x, y));
}

/** T9.a: a clear and a triangle, into an `rgba16float` target and into a view. */
export async function checkClearAndTriangle(engine: RenderEngine, checks: Checks): Promise<void> {
  const material = engine.createMaterial(flatSpec("basic"));
  // A counter-clockwise triangle in view space's upper right, 1 m ahead; its centroid is at
  // (0.367, 0.367), pixel (43, 20) of 64 × 64.
  const triangle = triangleMesh(engine, "basic triangle", [0.1, 0.1, 0.9, 0.1, 0.1, 0.9]);
  const target = engine.createRenderTarget({
    name: "basic",
    size: { widthPx: 64, heightPx: 64 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const tint = [0.25, 0.5, 2, 1] as const;
  target.render(frameOf("basic target", [drawOf(triangle, material, tint)]));
  const texels = halfTexels(await engine.readTexture(target.colour));
  checks.check(
    "T9.a target texels finite",
    texels.every(Number.isFinite),
    `${texels.length / 4} texels`,
  );
  const clear = texel(texels, 64, 8, 56);
  checks.check(
    "T9.a target clear colour where nothing is drawn",
    near(clear, [0, 0, 0, 1], 0),
    show(clear),
  );
  const centroid = texel(texels, 64, 43, 20);
  checks.check(
    "T9.a target triangle colour at its centroid",
    near(centroid, tint, 1e-3),
    show(centroid),
  );
  target.dispose();

  const view = engine.createView(addCanvas(), "basic view");
  view.resize({ widthPx: 64, heightPx: 64 });
  view.render(frameOf("basic view", [drawOf(triangle, material, [1, 0, 0, 1])]));
  const bytes = await view.readBack();
  checks.check(
    "T9.a view triangle colour at its centroid",
    near(pixel(bytes, 64, 43, 20), [255, 0, 0, 255], 0),
    show(pixel(bytes, 64, 43, 20)),
  );
  checks.check(
    "T9.a view clear colour where nothing is drawn",
    near(pixel(bytes, 64, 8, 56), [0, 0, 0, 255], 0),
    show(pixel(bytes, 64, 8, 56)),
  );
  view.dispose();
}

/** One of T9.d's views, with its size. */
interface NamedView {
  readonly view: RenderView;
  size: ViewSize;
}

/** T9.d: a 1280 × 720 cockpit and two 320 × 240 instruments on one device, resized independently. */
export async function checkThreeCanvases(engine: RenderEngine, checks: Checks): Promise<void> {
  const material = engine.createMaterial(flatSpec("marker"));
  const copy = engine.createPostProcess({
    name: "copy",
    uniforms: [],
    fragmentWgsl: `
@group(2) @binding(0) var colour : texture_2d<f32>;
@group(2) @binding(1) var colourSampler : sampler;
@fragment fn fragmentMain(@location(0) uv : vec2f) -> @location(0) vec4f {
  return textureSample(colour, colourSampler, uv);
}`,
  });
  const views: NamedView[] = [
    { view: engine.createView(addCanvas(), "cockpit"), size: { widthPx: 1280, heightPx: 720 } },
    { view: engine.createView(addCanvas(), "instrument 1"), size: { widthPx: 320, heightPx: 240 } },
    { view: engine.createView(addCanvas(), "instrument 2"), size: { widthPx: 320, heightPx: 240 } },
  ];
  const meshes = new Map<number, ReturnType<RenderEngine["createMesh"]>>();
  /** A marker in the top-left quadrant of a view of `aspect`, and a background quad. */
  const markerFor = (aspect: number): ReturnType<RenderEngine["createMesh"]> => {
    const key = Math.round(aspect * 1000);
    let mesh = meshes.get(key);
    if (mesh === undefined) {
      mesh = triangleMesh(engine, `marker ${key}`, [
        -0.8 * aspect,
        0.2,
        -0.2 * aspect,
        0.2,
        -0.8 * aspect,
        0.8,
      ]);
      meshes.set(key, mesh);
    }
    return mesh;
  };
  const renderAll = (): Array<Promise<Uint8Array | Float32Array>> =>
    views.map(({ view, size }) => {
      view.resize(size);
      const aspect = size.widthPx / size.heightPx;
      view.render(
        frameOf(view.name, [drawOf(markerFor(aspect), material, [0, 1, 0, 1])], aspect, [
          { postProcess: copy, uniforms: {} },
        ]),
      );
      // A canvas texture expires once the task yields: read it back now.
      return view.readBack();
    });
  const judge = async (round: string): Promise<void> => {
    const reads = await Promise.all(renderAll());
    reads.forEach((bytes, index) => {
      const entry = views[index];
      if (entry === undefined) {
        return;
      }
      const { widthPx: w, heightPx: h } = entry.size;
      const marker = pixel(bytes, w, Math.round(0.2 * w), Math.round(0.3 * h));
      const below = pixel(bytes, w, Math.round(0.2 * w), Math.round(0.7 * h));
      const right = pixel(bytes, w, Math.round(0.8 * w), Math.round(0.3 * h));
      checks.check(
        `T9.d ${round} ${entry.view.name} is ${w} × ${h}`,
        bytes.length === w * h * 4,
        `${bytes.length / 4} pixels`,
      );
      checks.check(
        `T9.d ${round} ${entry.view.name} marker top left, right way up`,
        near(marker, [0, 255, 0, 255], 0) &&
          near(below, [0, 0, 0, 255], 0) &&
          near(right, [0, 0, 0, 255], 0),
        `marker ${show(marker)}, below ${show(below)}, right ${show(right)}`,
      );
    });
  };
  await judge("first");
  const resized = views[1];
  if (resized !== undefined) {
    resized.size = { widthPx: 400, heightPx: 300 };
  }
  await judge("after instrument 1's resize");
  for (const { view } of views) {
    view.dispose();
  }
}

/** T9.f: a point's depth, culling by mode, and a bias that loses to a coplanar line. */
export async function checkDepthCullBias(engine: RenderEngine, checks: Checks): Promise<void> {
  const size = { widthPx: 32, heightPx: 32 };
  const target = engine.createRenderTarget({
    name: "depth",
    size,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const flat = engine.createMaterial(flatSpec("depth flat"));
  // A point 2 m ahead at pixel (16, 16)'s centre: reversed-Z depth is NEAR_M / 2.
  const point = engine.createMesh({
    name: "depth point",
    positions: new Float32Array([0.0625, -0.0625, -2]),
    indices: null,
    topology: "point-list",
    attributes: {},
  });
  target.render(frameOf("depth point", [drawOf(point, flat, [1, 1, 1, 1])]));
  const depth = new Float32Array(await engine.readTexture(target.depth ?? target.colour));
  const at = depth[16 * 32 + 16] ?? Number.NaN;
  checks.check(
    "T9.f a point writes depth near ÷ distance, no half-Z",
    Math.abs(at - NEAR_M / 2) < 1e-6 && (depth[0] ?? Number.NaN) === 0,
    `depth ${at}, expected ${NEAR_M / 2}; clear ${depth[0]}`,
  );

  // A clockwise triangle, drawn with each cull mode.
  const clockwise = triangleMesh(engine, "clockwise", [-0.9, -0.1, -0.1, -0.1, -0.9, -0.9]);
  for (const cullMode of ["back", "none"] as const) {
    const material = engine.createMaterial(flatSpec(`cull ${cullMode}`, { cullMode }));
    target.render(frameOf(`cull ${cullMode}`, [drawOf(clockwise, material, [1, 0, 0, 1])]));
    // The triangle's centroid, (-0.63, -0.37), is pixel (5, 21).
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const seen = texel(halfTexels(await engine.readTexture(target.colour)), 32, 5, 21);
    const drawn = seen[0] === 1;
    checks.check(
      `T9.f a back face is ${cullMode === "back" ? "culled" : "drawn"} under cullMode ${cullMode}`,
      drawn === (cullMode === "none"),
      show(seen),
    );
  }

  // A line along row 16, then a coplanar surface: biased away it loses to the line, unbiased wins.
  const line = engine.createMesh({
    name: "bias line",
    positions: new Float32Array([-1, -0.03125, -1, 1, -0.03125, -1]),
    indices: null,
    topology: "line-list",
    attributes: {},
  });
  const surface = fullScreenMesh(engine, "bias surface");
  for (const biased of [true, false]) {
    const surfaceMaterial = engine.createMaterial(
      flatSpec(`surface ${biased ? "biased" : "unbiased"}`, {
        cullMode: "back",
        ...(biased ? { depthBiasAway: { constant: 64, slopeScale: 1 } } : {}),
      }),
    );
    target.render(
      frameOf(`bias ${String(biased)}`, [
        drawOf(line, flat, [0, 1, 0, 1]),
        drawOf(surface, surfaceMaterial, [0, 0, 1, 1]),
      ]),
    );
    // The harness's checks run in order: each reads the GPU back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const seen = texel(halfTexels(await engine.readTexture(target.colour)), 32, 8, 16);
    checks.check(
      `T9.f a coplanar line ${biased ? "beats a surface biased away" : "loses to an unbiased surface"}`,
      biased ? near(seen, [0, 1, 0, 1], 0) : near(seen, [0, 0, 1, 1], 0),
      show(seen),
    );
  }
  target.dispose();
}
