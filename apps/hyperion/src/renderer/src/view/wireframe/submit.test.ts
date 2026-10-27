import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { centresJustBeyond, limbPastRightEdge } from "../../test/beyondView";
import { aCameraPose, NO_TURN } from "../../test/viewFixtures";
import { bindingStages } from "../../test/wgslStages";
import type {
  BufferHandle,
  DrawItem,
  FrameSubmission,
  MaterialHandle,
  MeshHandle,
  RenderView,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import {
  declaredStructLayout,
  OFFSET_MEMBER,
  type StructLayout,
  uniformLayout,
} from "../engine/webgpu/uniforms";
import {
  perspectiveReversedInfinite,
  project,
  toViewAxes,
  type Viewport,
  viewRotation4,
} from "../camera/projection";
import { quaternionFromAxisAngle } from "../camera/quaternion";
import { PSF_QUAD_PX, PSF_SIGMA_PX } from "../photometry/magnitude";
import {
  type DrawCamera,
  emptyDrawList,
  HULL_OCCLUDER_DEPTH_FRACTION,
  type LineBatch,
  viewStrokesAt,
  type WireframeDrawList,
} from "./drawList";
import {
  COVERAGE_MASK_BINDING,
  linearColour,
  MATERIAL_BUFFER,
  NO_WIREFRAME_TERRAIN,
  OCCLUDER_SPHERE_ROWS,
  OUTSIDE_VIEW_MARGIN_PX,
  type PackedDraw,
  packWireframe,
  sphereScreenRect,
  spheroidAxis,
  type SurveyedDatum,
  WIREFRAME_MATERIALS,
  type WireframeEngine,
  type WireframeMaterial,
  WireframeRenderer,
  type WireframeTerrain,
} from "./submit";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: DrawCamera = { pose: aCameraPose({ orientation: NO_TURN }), fovXRad: Math.PI / 3 };
const PROJECTION = { orientation: NO_TURN, fovXRad: Math.PI / 3 };
const MATERIALS: ReadonlyArray<WireframeMaterial> = [
  "lines",
  "occluderSphere",
  "occluderHull",
  "hullSilhouette",
  "starSprite",
];

const EMPTY: WireframeDrawList = emptyDrawList(viewStrokesAt(1));

function aBatch(overrides: Partial<LineBatch> = {}): LineBatch {
  return {
    id: "batch",
    space: "screen",
    originF32: new Float32Array(3),
    segments: new Float32Array([0, 0, 0, 10, 0, 0]),
    token: "text",
    colour: "#ffffff",
    widthPx: 1,
    casingWidthPx: 1,
    casingColour: "#000000",
    dash: null,
    ...overrides,
  };
}

/** The value of `const <name> = <number>;` in a material's shader, the sprite's by default. */
function wgslConstant(name: string, material: WireframeMaterial = "starSprite"): number {
  const found = new RegExp(`const ${name} = ([\\d.]+);`).exec(
    WIREFRAME_MATERIALS[material].vertexWgsl,
  );
  return Number(found?.[1]);
}

/** The fields of the last `struct Draw` in a composed source, by name and type. */
function drawStruct(source: string): string[] {
  const all = [...source.matchAll(/struct Draw \{([^}]*)\}/g)];
  const body = all.at(-1)?.[1] ?? "";
  return body
    .split("\n")
    .map((line) => line.replace(/\/\/.*$/, "").trim())
    .filter((line) => line.length > 0)
    .map((line) => line.replace(/,$/, ""));
}

describe("the wireframe's shaders", () => {
  it.each(MATERIALS)("%s's Draw is the offset, then the spec's uniforms in order", (name) => {
    const spec = WIREFRAME_MATERIALS[name];
    expect(drawStruct(spec.vertexWgsl)).toEqual([
      "offsetFromCameraM: vec3f",
      ...spec.uniforms.map((u) => `${u.name}: ${u.type}`),
    ]);
  });

  it.each(MATERIALS)("%s declares the frame, the draw and its buffer at R01's groups", (name) => {
    // Spacing is free around a colon: the decision's own frame.wgsl writes `frame : Frame`.
    const source = WIREFRAME_MATERIALS[name].vertexWgsl.replace(/\s*:\s*/g, ": ");
    expect(
      [
        "@group(0) @binding(0) var<uniform> frame: Frame;",
        "@group(1) @binding(0) var<uniform> draw: Draw;",
        `@group(2) @binding(0) var<storage, read> ${MATERIAL_BUFFER[name]}: array<vec4f>;`,
      ].filter((line) => !source.includes(line)),
    ).toEqual([]);
  });

  it.each(MATERIALS)("%s has the entry points vertexMain and fragmentMain", (name) => {
    const source = WIREFRAME_MATERIALS[name].vertexWgsl;
    expect([/fn vertexMain\(/.test(source), /fn fragmentMain\(/.test(source)]).toEqual([
      true,
      true,
    ]);
  });

  it.each(MATERIALS)("%s names its storage buffer at binding 0", (name) => {
    expect(WIREFRAME_MATERIALS[name].storageBuffers).toEqual([
      { name: MATERIAL_BUFFER[name], binding: 0 },
    ]);
  });

  it("carries the point-spread constants of the photometry in the sprite shader", () => {
    expect([wgslConstant("PSF_SIGMA_PX"), wgslConstant("PSF_QUAD_PX")]).toEqual([
      PSF_SIGMA_PX,
      PSF_QUAD_PX,
    ]);
  });

  it("push a hull face away in its fragment by the draw list's constant, as its own depth", () => {
    const source = WIREFRAME_MATERIALS.occluderHull.fragmentWgsl;
    expect([
      wgslConstant("DEPTH_FRACTION", "occluderHull"),
      /@builtin\(frag_depth\)/.test(source),
      /dpdxFine\(depth\), dpdyFine\(depth\)/.test(source),
    ]).toEqual([HULL_OCCLUDER_DEPTH_FRACTION, true, true]);
  });

  it("read a sphere's rows by the stride packWireframe writes them at, named once in each language (R02.T19)", () => {
    const source = WIREFRAME_MATERIALS.occluderSphere.vertexWgsl;
    const stride = /const SPHERE_ROWS: u32 = (\d+)u;/.exec(source);
    expect([
      Number(stride?.[1]),
      OCCLUDER_SPHERE_ROWS,
      // Every read of `spheres` goes through the stride.
      [...source.matchAll(/spheres\[([^\]]*)\]/g)].map((read) => read[1]),
    ]).toEqual([
      3,
      3,
      ["sphere * SPHERE_ROWS", "v.sphere * SPHERE_ROWS + 1u", "v.sphere * SPHERE_ROWS + 2u"],
    ]);
  });

  it("take both occluders' slope term from the draw's uniform, not a constant", () => {
    expect([
      WIREFRAME_MATERIALS.occluderSphere.uniforms.map((u) => u.name),
      WIREFRAME_MATERIALS.occluderHull.uniforms.map((u) => u.name),
      /SLOPE_SCALE/.test(WIREFRAME_MATERIALS.occluderSphere.fragmentWgsl),
    ]).toEqual([["occluderSlopePx"], ["firstTriangle", "occluderSlopePx", "fill"], false]);
  });

  it("draw a hull's silhouette from its occluder's source, its colour the draw's fill (R07.T16.e)", () => {
    const silhouette = WIREFRAME_MATERIALS.hullSilhouette;
    const fragment = silhouette.fragmentWgsl.slice(
      silhouette.fragmentWgsl.indexOf("fn fragmentMain"),
    );
    expect([
      silhouette.vertexWgsl === WIREFRAME_MATERIALS.occluderHull.vertexWgsl,
      silhouette.uniforms,
      /out\.colour = draw\.fill;/.test(fragment),
      // No colour literal: the fill is the token's, through the uniform.
      /vec4f\(\s*[\d.]/.test(fragment),
    ]).toEqual([true, WIREFRAME_MATERIALS.occluderHull.uniforms, true, false]);
  });

  it("are composed of ASCII alone", () => {
    expect(
      MATERIALS.filter((name) => !/^[\t\n -~]*$/.test(WIREFRAME_MATERIALS[name].vertexWgsl)),
    ).toEqual([]);
  });
});

/** A material's pipeline state, in a fixed order. */
function state(spec: WgslMaterialSpec): unknown[] {
  return [
    spec.depthWrite,
    spec.colourWrites,
    spec.blend,
    spec.cullMode,
    spec.depthBiasAway ?? null,
  ];
}

describe("the wireframe's materials", () => {
  it("write depth only from the occluders, and set no hardware depth bias", () => {
    expect(
      Object.fromEntries(MATERIALS.map((name) => [name, state(WIREFRAME_MATERIALS[name])])),
    ).toEqual({
      lines: [false, true, "premultiplied", "none", null],
      occluderSphere: [true, false, "none", "none", null],
      occluderHull: [true, false, "none", "none", null],
      hullSilhouette: [true, true, "none", "none", null],
      starSprite: [false, true, "additive", "none", null],
    });
  });
});

describe("linearColour", () => {
  it("decodes the token's sRGB value to linear light", () => {
    expect([
      [...linearColour("#ffffff")],
      [...linearColour("#000")],
      Math.round((linearColour("rgb(188, 188, 188)")[0] ?? 0) * 1000) / 1000,
    ]).toEqual([[1, 1, 1, 1], [0, 0, 0, 1], 0.503]);
  });

  it("refuses a form it cannot read", () => {
    expect(() => linearColour("red")).toThrow(/is not #rgb/);
  });
});

describe("sphereScreenRect", () => {
  it("holds every point of a sphere's projected limb", () => {
    const camera = {
      orientation: quaternionFromAxisAngle(vec3(0, 1, 0), 0.1),
      fovXRad: Math.PI / 3,
    };
    const centre = vec3(2e6, -1e6, -2e7);
    const radius = 6.4e6;
    const rect = sphereScreenRect(centre, radius, camera, VIEWPORT);
    // The limb: the points of tangency, at angle α = asin(r ÷ D) from the centre's direction.
    const d = norm(centre);
    const axis = normalise(centre);
    const u = normalise(cross(axis, vec3(0, 1, 0)));
    const w = cross(axis, u);
    const sinA = radius / d;
    const cosA = Math.sqrt(1 - sinA * sinA);
    const outside = Array.from({ length: 720 }, (_, i) => (2 * Math.PI * i) / 720).filter((t) => {
      const direction = add(
        scale(axis, cosA),
        scale(add(scale(u, Math.cos(t)), scale(w, Math.sin(t))), sinA),
      );
      const p = project(scale(direction, d * cosA), camera, VIEWPORT);
      const x = Math.min(Math.max(p.xPx, 0), VIEWPORT.widthPx);
      const y = Math.min(Math.max(p.yPx, 0), VIEWPORT.heightPx);
      return (
        rect === null || x < rect.leftPx || x > rect.rightPx || y < rect.topPx || y > rect.bottomPx
      );
    });
    expect(outside).toEqual([]);
  });

  it("is the whole view where the limb reaches behind the near plane", () => {
    // 400 km above an Earth-sized sphere, looking along the horizon.
    const rect = sphereScreenRect(vec3(0, -6.771e6, 0), 6.371e6, PROJECTION, VIEWPORT);
    expect(rect).toEqual({ leftPx: 0, topPx: 0, rightPx: 1920, bottomPx: 1080 });
  });

  it("is absent for a sphere off the view", () => {
    expect(sphereScreenRect(vec3(1e8, 0, -1e7), 1e6, PROJECTION, VIEWPORT)).toBeNull();
  });
});

/** A packed draw's `fill` uniform, or `null` where it has none. */
function fillOf(draw: PackedDraw): number[] | null {
  const value = draw.uniforms["fill"];
  return value === undefined ? null : [...value];
}

describe("packWireframe", () => {
  const list: WireframeDrawList = {
    occluderSpheres: [
      { id: "near", centreF32: new Float32Array([0, 0, -1e7]), radiusM: 1e6, altitudeM: 9e6 },
      { id: "off", centreF32: new Float32Array([1e8, 0, -1e7]), radiusM: 1e6, altitudeM: 9.9e7 },
    ],
    occluderMeshes: [1, 2].map((n) => ({
      id: `hull${String(n)}`,
      originF32: new Float32Array([0, 0, -n]),
      triangles: new Float32Array(9 * n).fill(n),
      twoSided: true as const,
      // The first depth only, as the wireframe draws a hull; the second filled, as the overlay does.
      fill: n === 2 ? "#05080d" : null,
    })),
    lines: [
      aBatch({ id: "cased" }),
      aBatch({ id: "uncased", casingWidthPx: 0, widthPx: 1.5 }),
      aBatch({
        id: "dashed",
        segments: new Float32Array([0, 0, 0, 10, 0, 0, 10, 0, 0, 10, 5, 0, 50, 50, 0, 60, 50, 0]),
        dash: { onPx: 6, offPx: 4 },
      }),
    ],
    sprites: [
      {
        id: "star",
        directionF32: new Float32Array([0, 0, -1]),
        xPx: 960.25,
        yPx: 540.75,
        exposedRgb: [1, 2, 3],
        illuminanceLx: 1e-6,
      },
    ],
    anchors: [],
    strokeScale: 2,
    markStrokePx: 2,
    occluderSlopePx: 5,
    surveyEdges: [],
    lineOfSight: null,
  };
  const packed = packWireframe(list, CAMERA, VIEWPORT);
  const summary = packed.draws.map((d) => [
    d.material,
    d.instanceCount,
    d.uniforms["widthPx"]?.[0] ?? null,
    d.uniforms["firstSegment"]?.[0] ?? d.uniforms["firstTriangle"]?.[0] ?? null,
  ]);

  it("draws the occluders, then the sprites, then each batch's casing before its stroke", () => {
    expect(summary).toEqual([
      ["occluderSphere", 1, null, null],
      ["occluderHull", 1, null, 0],
      ["hullSilhouette", 2, null, 1],
      ["starSprite", 1, null, null],
      ["lines", 1, 3, 0],
      ["lines", 1, 1, 0],
      ["lines", 1, 1.5, 1],
      ["lines", 3, 3, 2],
      ["lines", 3, 1, 2],
    ]);
  });

  it("draws a casing in its own colour, the stroke in the token's", () => {
    const lines = packed.draws.filter((d) => d.material === "lines");
    expect([
      [...(lines[0]?.uniforms["colour"] ?? [])],
      [...(lines[1]?.uniforms["colour"] ?? [])],
    ]).toEqual([
      [0, 0, 0, 1],
      [1, 1, 1, 1],
    ]);
  });

  it("carries a dash's phase on along a polyline and restarts it at a break", () => {
    const phases = [2, 3, 4].map((segment) => packed.segments[segment * 8 + 3]);
    expect(phases).toEqual([0, 10, 0]);
  });

  it("gives both occluders the list's slope term", () => {
    const occluders = packed.draws.filter(
      (d) => d.material !== "lines" && d.material !== "starSprite",
    );
    expect(occluders.map((d) => [d.material, d.uniforms["occluderSlopePx"]?.[0]])).toEqual([
      ["occluderSphere", 5],
      ["occluderHull", 5],
      ["hullSilhouette", 5],
    ]);
  });

  it("fills a silhouette in its mesh's colour, linear, and gives depth-only faces none (R07.T16.e)", () => {
    const hulls = packed.draws.filter(
      (d) => d.material === "occluderHull" || d.material === "hullSilhouette",
    );
    expect(hulls.map((d) => [d.material, fillOf(d)])).toEqual([
      ["occluderHull", null],
      ["hullSilhouette", [...linearColour("#05080d")]],
    ]);
  });

  it("packs a sphere's centre, radius and altitude", () => {
    expect([...packed.spheres].slice(4, 12)).toEqual([0, 0, -1e7, 1e6, 9e6, 0, 0, 0]);
  });

  it("leaves out a sphere off the view, packing the other's rows at the stride the shader reads", () => {
    expect(packed.spheres.length).toBe(4 * OCCLUDER_SPHERE_ROWS);
  });

  it("packs a spheroid's axis, its pole times √(a ÷ c − 1), after its altitude (R02.T19)", () => {
    const pole = normalise(vec3(0, 3, 4));
    const spheroidal = packWireframe(
      {
        ...list,
        occluderSpheres: [
          {
            id: "giant",
            centreF32: new Float32Array([0, 0, -1e9]),
            radiusM: 6e7,
            altitudeM: 9.4e8,
            spheroid: { pole, stretch: 1.25 },
          },
        ],
      },
      CAMERA,
      VIEWPORT,
    );
    const row = [...spheroidal.spheres.subarray(8, 12)];
    const axis = spheroidAxis({ pole, stretch: 1.25 });
    expect([row, [axis.x, axis.y, axis.z]]).toEqual([
      [9.4e8, ...[0, 0.3, 0.4].map((x) => Math.fround(x))],
      [0, 0.30000000000000004, 0.4],
    ]);
  });

  it("packs a sprite's position and colour", () => {
    expect([...packed.sprites]).toEqual([960.25, 540.75, 0, 0, 1, 2, 3, 0]);
  });

  it("packs nothing for an empty list", () => {
    expect(packWireframe(EMPTY, CAMERA, VIEWPORT).draws).toEqual([]);
  });
});

/**
 * A body's occluder sphere, as the draw list gives it (taken from the list's own type here, which
 * leaves the `./drawList` import as it was).
 */
type OccluderSphere = WireframeDrawList["occluderSpheres"][number];

/**
 * How many pixels' depth `occluderSphere.wgsl` writes, in `f64`: those whose centre's ray meets
 * the sphere ahead of a camera outside it.
 */
function writtenPixelCount(sphere: OccluderSphere, camera: DrawCamera, viewport: Viewport): number {
  const c = sphere.centreF32;
  const centre = toViewAxes(vec3(c[0] ?? 0, c[1] ?? 0, c[2] ?? 0), camera.pose.orientation);
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const aspect = viewport.widthPx / viewport.heightPx;
  let written = 0;
  for (let y = 0; y < viewport.heightPx; y += 1) {
    for (let x = 0; x < viewport.widthPx; x += 1) {
      const ndcX = ((x + 0.5) / viewport.widthPx) * 2 - 1;
      const ndcY = 1 - ((y + 0.5) / viewport.heightPx) * 2;
      const ray = normalise(vec3(ndcX / s, ndcY / (s * aspect), -1));
      const along = dot(centre, ray);
      const perp = norm(sub(centre, scale(ray, along)));
      const h2 = sphere.radiusM ** 2 - perp ** 2;
      written += along > 0 && h2 >= 0 && sphere.altitudeM > 0 ? 1 : 0;
    }
  }
  return written;
}

describe("an occluder sphere wholly off the view (R07.T19.e)", () => {
  // Small, so that the twin tests every pixel quickly; the margin is in pixels. The tall view's
  // height spans 144° at a 120° width.
  const SMALL: Viewport = { widthPx: 72, heightPx: 40 };
  const TALL: Viewport = { widthPx: 40, heightPx: 72 };
  const RADIUS_M = 6.371e6;

  /** A sphere at `centreM` from the camera, as the draw list gives it. */
  function sphereAt(centreM: Vec3): OccluderSphere {
    return {
      id: "body",
      centreF32: new Float32Array([centreM.x, centreM.y, centreM.z]),
      radiusM: RADIUS_M,
      altitudeM: norm(centreM) - RADIUS_M,
    };
  }

  const cases = [SMALL, TALL].flatMap((viewport) =>
    [10, 60, 120].flatMap((fovDeg) =>
      [3, 300].map((distanceRadii) => ({
        view: `${viewport.widthPx} × ${viewport.heightPx}`,
        viewport,
        fovDeg,
        distanceRadii,
      })),
    ),
  );

  /** Spheres just beyond each widened side plane, `distanceRadii` radii away. */
  function justBeyond(camera: DrawCamera, viewport: Viewport, distanceRadii: number) {
    // A thousandth of a radius beyond, past the centre's rounding to f32.
    return centresJustBeyond(
      camera.fovXRad,
      viewport,
      OUTSIDE_VIEW_MARGIN_PX,
      distanceRadii * RADIUS_M,
      RADIUS_M * (1 + 1e-3),
    ).map(sphereAt);
  }

  it.each(cases)(
    "packs none beside, level with a corner, across or behind: $distanceRadii radii, $fovDeg° across $view",
    ({ viewport, fovDeg, distanceRadii }) => {
      const camera: DrawCamera = { pose: CAMERA.pose, fovXRad: (fovDeg * Math.PI) / 180 };
      const spheres = justBeyond(camera, viewport, distanceRadii);
      const packed = packWireframe({ ...EMPTY, occluderSpheres: spheres }, camera, viewport);
      expect(packed.spheres.length).toBe(0);
    },
  );

  it.each(cases)(
    "writes no pixel there by its twin: $distanceRadii radii, $fovDeg° across $view",
    ({ viewport, fovDeg, distanceRadii }) => {
      const camera: DrawCamera = { pose: CAMERA.pose, fovXRad: (fovDeg * Math.PI) / 180 };
      const written = justBeyond(camera, viewport, distanceRadii).reduce(
        (sum, sphere) => sum + writtenPixelCount(sphere, camera, viewport),
        0,
      );
      expect(written).toBe(0);
    },
  );

  it("keeps a sphere the view's edge cuts", () => {
    const onEdge = sphereAt(
      scale(normalise(vec3(Math.tan(CAMERA.fovXRad / 2), 0, -1)), 30 * RADIUS_M),
    );
    const packed = packWireframe({ ...EMPTY, occluderSpheres: [onEdge] }, CAMERA, SMALL);
    expect(packed.spheres.length).toBe(12);
  });

  it("keeps a sphere whose limb is a pixel past the edge", () => {
    const pastEdge = sphereAt(limbPastRightEdge(CAMERA.fovXRad, SMALL, 30 * RADIUS_M, RADIUS_M, 1));
    const packed = packWireframe({ ...EMPTY, occluderSpheres: [pastEdge] }, CAMERA, SMALL);
    expect(packed.spheres.length).toBe(12);
  });
});

/** An engine that records what the renderer makes and writes. */
class RecordingEngine implements WireframeEngine {
  readonly materials: string[] = [];
  readonly buffers: BufferHandle[] = [];
  readonly writes: { buffer: string; bytes: number }[] = [];
  #restored: (() => void) | null = null;

  createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    this.materials.push(spec.name);
    return { kind: "material", name: spec.name };
  }
  createMesh(spec: { readonly name: string }): MeshHandle {
    return { kind: "mesh", name: spec.name };
  }
  createBuffer(spec: { readonly name: string; readonly bytes: number }): BufferHandle {
    const handle: BufferHandle = { kind: "buffer", name: spec.name, bytes: spec.bytes };
    this.buffers.push(handle);
    return handle;
  }
  writeBuffer(buffer: BufferHandle, _offsetBytes: number, data: ArrayBufferView): void {
    this.writes.push({ buffer: buffer.name, bytes: data.byteLength });
  }
  onRestored(listener: () => void): () => void {
    this.#restored = listener;
    return () => {
      this.#restored = null;
    };
  }
  restore(): void {
    this.#restored?.();
  }
}

describe("WireframeRenderer", () => {
  it("submits the camera's rotation and reversed-Z projection under a stable label", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const frame = renderer.frame(EMPTY, CAMERA, VIEWPORT);
    expect([frame.label, [...frame.viewRotation], [...frame.projection]]).toEqual([
      "view:wireframe",
      [...viewRotation4(NO_TURN)],
      [...perspectiveReversedInfinite(Math.PI / 3, 1920 / 1080, 0.1)],
    ]);
  });

  it("binds each draw's storage buffer by the material's name", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const frame = renderer.frame({ ...EMPTY, lines: [aBatch()] }, CAMERA, VIEWPORT);
    expect(frame.draws.map((d) => [d.material.name, Object.keys(d.storageBuffers ?? {})])).toEqual([
      ["wireframe:lines", ["segments"]],
      ["wireframe:lines", ["segments"]],
    ]);
  });

  it("encodes the background after the occluders and before the sprites (R06.T13.g)", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const list: WireframeDrawList = {
      ...EMPTY,
      occluderSpheres: [
        { id: "near", centreF32: new Float32Array([0, 0, -1e7]), radiusM: 1e6, altitudeM: 9e6 },
      ],
      sprites: [
        {
          id: "star",
          directionF32: new Float32Array([0, 0, -1]),
          xPx: 960,
          yPx: 540,
          exposedRgb: [1, 1, 1],
          illuminanceLx: 1e-6,
        },
      ],
    };
    const cube = {
      mesh: { kind: "mesh", name: "sky cube triangle" },
      material: { kind: "material", name: "sky:cubeDisplay" },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    } as const;
    const frame = renderer.frame(list, CAMERA, VIEWPORT, [cube]);
    expect(frame.draws.map((d) => d.material.name)).toEqual([
      "wireframe:occluderSphere",
      "sky:cubeDisplay",
      "wireframe:starSprite",
    ]);
  });

  it("grows a buffer by doubling when a frame outgrows it", () => {
    const engine = new RecordingEngine();
    const renderer = new WireframeRenderer(engine);
    // 300 segments: 300 × 32 bytes = 9,600, past 4,096 and 8,192.
    const segments = new Float32Array(300 * 6);
    renderer.frame({ ...EMPTY, lines: [aBatch({ segments })] }, CAMERA, VIEWPORT);
    expect(
      engine.buffers.filter((b) => b.name === "wireframe:segments").map((b) => b.bytes),
    ).toEqual([4_096, 16_384]);
  });

  it("draws the spheres, then the silhouettes, before the background and every line (R07.T16.e)", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const list: WireframeDrawList = {
      ...EMPTY,
      occluderSpheres: [
        { id: "near", centreF32: new Float32Array([0, 0, -1e7]), radiusM: 1e6, altitudeM: 9e6 },
      ],
      occluderMeshes: [
        {
          id: "hull",
          originF32: new Float32Array([0, 0, -1]),
          triangles: new Float32Array(9).fill(1),
          twoSided: true,
          fill: "#05080d",
        },
      ],
      lines: [aBatch()],
    };
    const cube = {
      mesh: { kind: "mesh", name: "sky cube triangle" },
      material: { kind: "material", name: "sky:cubeDisplay" },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    } as const;
    const frame = renderer.frame(list, CAMERA, VIEWPORT, [cube]);
    expect(frame.draws.map((d) => d.material.name)).toEqual([
      "wireframe:occluderSphere",
      "wireframe:hullSilhouette",
      "sky:cubeDisplay",
      "wireframe:lines",
      "wireframe:lines",
    ]);
  });

  it("makes its materials again when the engine restores its device", () => {
    const engine = new RecordingEngine();
    const renderer = new WireframeRenderer(engine);
    engine.restore();
    renderer.dispose();
    expect(engine.materials.length).toBe(14);
  });

  it("stops following the engine's restores once disposed", () => {
    const engine = new RecordingEngine();
    new WireframeRenderer(engine).dispose();
    engine.restore();
    expect(engine.materials.length).toBe(7);
  });
});

/** The two materials of a body's datum over its surveyed terrain (R10.T11). */
const SURVEYED_MATERIALS: ReadonlyArray<WireframeMaterial> = [
  "linesSurveyed",
  "occluderSphereSurveyed",
];

/** A layout's members as `name : type @ offset`. */
function membersOf(layout: StructLayout | null): string[] | undefined {
  return layout?.members.map((m) => `${m.name} : ${m.type} @ ${String(m.offsetBytes)}`);
}

describe("the wireframe's shaders over a surveyed body (R10.T11)", () => {
  it.each(SURVEYED_MATERIALS)(
    "%s's Draw is laid out as the engine packs the offset and the spec's uniforms",
    (name) => {
      const spec = WIREFRAME_MATERIALS[name];
      expect(membersOf(declaredStructLayout(spec.vertexWgsl, "Draw"))).toEqual(
        membersOf(uniformLayout([OFFSET_MEMBER, ...spec.uniforms])),
      );
    },
  );

  it.each(SURVEYED_MATERIALS)(
    "%s declares the frame, the draw, its buffer and the coverage mask at R01's groups",
    (name) => {
      const source = WIREFRAME_MATERIALS[name].vertexWgsl.replace(/\s*:\s*/g, ": ");
      expect(
        [
          "@group(0) @binding(0) var<uniform> frame: Frame;",
          "@group(1) @binding(0) var<uniform> draw: Draw;",
          `@group(2) @binding(0) var<storage, read> ${MATERIAL_BUFFER[name]}: array<vec4f>;`,
          `@group(2) @binding(${String(COVERAGE_MASK_BINDING)}) var coverage_mask: texture_2d_array<f32>;`,
        ].filter((line) => !source.includes(line)),
      ).toEqual([]);
    },
  );

  it.each(SURVEYED_MATERIALS)(
    "%s lists its buffer at binding 0 and the coverage mask, a 2D array, at binding 1",
    (name) => {
      const spec = WIREFRAME_MATERIALS[name];
      expect([COVERAGE_MASK_BINDING, spec.storageBuffers, spec.textures, spec.samplers]).toEqual([
        1,
        [{ name: MATERIAL_BUFFER[name], binding: 0 }],
        [{ name: "coverageMask", binding: 1, viewDimension: "2d-array" }],
        [],
      ]);
    },
  );

  it.each(SURVEYED_MATERIALS)(
    "%s reads the coverage mask in its fragment stage alone, and no binding its spec leaves out",
    (name) => {
      const read = bindingStages(WIREFRAME_MATERIALS[name].vertexWgsl);
      expect([
        [...read.keys()].toSorted((a, b) => a - b),
        [...(read.get(COVERAGE_MASK_BINDING) ?? [])],
      ]).toEqual([[0, 1], ["fragment"]]);
    },
  );

  it.each(SURVEYED_MATERIALS)("%s has the entry points vertexMain and fragmentMain", (name) => {
    const source = WIREFRAME_MATERIALS[name].vertexWgsl;
    expect([/fn vertexMain\(/.test(source), /fn fragmentMain\(/.test(source)]).toEqual([
      true,
      true,
    ]);
  });

  it("draw as their twins do: the datum's lines as R02's lines, its occluder as R02's occluder", () => {
    expect([
      state(WIREFRAME_MATERIALS.linesSurveyed),
      state(WIREFRAME_MATERIALS.occluderSphereSurveyed),
    ]).toEqual([state(WIREFRAME_MATERIALS.lines), state(WIREFRAME_MATERIALS.occluderSphere)]);
  });

  it("take R02's line uniforms first, as the pack spreads them, then the coverage's", () => {
    expect(WIREFRAME_MATERIALS.linesSurveyed.uniforms.map((u) => u.name)).toEqual([
      ...WIREFRAME_MATERIALS.lines.uniforms.map((u) => u.name),
      "coverageAxes",
      "coverageStretch",
      "coverageCentreM",
    ]);
  });

  it("name the two on the console DATUM LINES and DATUM OCCLUDER", () => {
    expect(SURVEYED_MATERIALS.map((name) => WIREFRAME_MATERIALS[name].displayName)).toEqual([
      "DATUM LINES",
      "DATUM OCCLUDER",
    ]);
  });
});

/** A body's occluder sphere 10⁷ m ahead, `x` and `y` m off the view's axis. */
function sphereOf(id: string, x: number, y = 0): OccluderSphere {
  return {
    id,
    centreF32: new Float32Array([x, y, -1e7]),
    radiusM: 5e5,
    altitudeM: Math.hypot(x, y, 1e7) - 5e5,
  };
}

/** A body's own line batch, its limb or graticule, in view space before it. */
function bodyBatch(body: string, kind: "major" | "graticule"): LineBatch {
  return aBatch({
    id: `body:${body}:${kind}`,
    space: "view",
    segments: new Float32Array([0, 0, -1e7, 1e5, 0, -1e7]),
    widthPx: kind === "major" ? 3 : 2,
    body,
  });
}

/** A body's datum with distinct axes and stretch. */
function datumOf(seed: number): SurveyedDatum {
  return {
    axes: Float32Array.from({ length: 16 }, (_, k) => seed + k / 16),
    stretch: 1 + seed / 1_000,
  };
}

/** A draw's uniforms as plain arrays. */
function uniformsOf(draw: PackedDraw | undefined): Record<string, number[]> {
  return Object.fromEntries(
    Object.entries(draw?.uniforms ?? {}).map(([name, value]) => [name, [...value]]),
  );
}

/** A pack's line draws, R02's and the surveyed ones, in their order. */
function lineDraws(draws: ReadonlyArray<PackedDraw>): PackedDraw[] {
  return draws.filter((d) => d.material === "lines" || d.material === "linesSurveyed");
}

/** The packed sphere rows' centres, in their order in the buffer. */
function sphereCentres(spheres: Float32Array): number[][] {
  const centres: number[][] = [];
  for (let row = 0; row < spheres.length / 12; row += 1) {
    centres.push([...spheres.subarray(row * 12 + 4, row * 12 + 7)]);
  }
  return centres;
}

describe("packWireframe over a surveyed body's terrain (R10.T11)", () => {
  // Three bodies in view, b's datum giving way to its terrain; a's and b's own lines and an orbit.
  const list: WireframeDrawList = {
    ...EMPTY,
    occluderSpheres: [sphereOf("a", -2e6), sphereOf("b", 0), sphereOf("c", 2e6)],
    lines: [
      bodyBatch("a", "major"),
      bodyBatch("b", "major"),
      bodyBatch("b", "graticule"),
      aBatch({
        id: "orbit:c",
        space: "view",
        segments: new Float32Array([0, 0, -1e7, 1, 0, -1e7]),
      }),
    ],
    occluderSlopePx: 5,
  };
  const B = datumOf(2);
  const surveyed = new Map([["b", B]]);
  const packed = packWireframe(list, CAMERA, VIEWPORT, surveyed);
  const plain = packWireframe(list, CAMERA, VIEWPORT);

  it("packs the surveyed body's sphere after the others in the spheres' buffer", () => {
    expect(sphereCentres(packed.spheres)).toEqual([
      [-2e6, 0, -1e7],
      [2e6, 0, -1e7],
      [0, 0, -1e7],
    ]);
  });

  it("draws the other spheres as one occluderSphere draw of their count, the surveyed one as its own", () => {
    const spheres = packed.draws.filter(
      (d) => d.material === "occluderSphere" || d.material === "occluderSphereSurveyed",
    );
    expect(
      spheres.map((d) => [
        d.material,
        d.instanceCount,
        d.uniforms["firstSphere"]?.[0] ?? null,
        d.surveyedBody ?? null,
      ]),
    ).toEqual([
      ["occluderSphere", 2, null, null],
      ["occluderSphereSurveyed", 1, 2, "b"],
    ]);
  });

  it("gives each of several surveyed bodies' spheres its own index among the frame's spheres", () => {
    const four: WireframeDrawList = {
      ...list,
      occluderSpheres: [
        sphereOf("a", -2e6),
        sphereOf("b", 0),
        sphereOf("c", 2e6),
        sphereOf("d", 0, 1.5e6),
      ],
    };
    const both = packWireframe(
      four,
      CAMERA,
      VIEWPORT,
      new Map([
        ["b", B],
        ["d", datumOf(4)],
      ]),
    );
    const centres = sphereCentres(both.spheres);
    expect(
      both.draws
        .filter((d) => d.material === "occluderSphereSurveyed")
        .map((d) => {
          const first = d.uniforms["firstSphere"]?.[0] ?? -1;
          return [d.surveyedBody, first, centres[first]];
        }),
    ).toEqual([
      ["b", 2, [0, 0, -1e7]],
      ["d", 3, [0, 1.5e6, -1e7]],
    ]);
  });

  it("gives the surveyed sphere the list's slope term and its datum's axes and stretch", () => {
    const sphere = packed.draws.find((d) => d.material === "occluderSphereSurveyed");
    expect(uniformsOf(sphere)).toEqual({
      occluderSlopePx: [5],
      firstSphere: [2],
      coverageAxes: [...B.axes],
      coverageStretch: [Math.fround(B.stretch), 0, 0, 0],
    });
  });

  it("draws no surveyed sphere for a surveyed body off the view, and packs none for it", () => {
    const off = packWireframe(
      { ...list, occluderSpheres: [sphereOf("a", -2e6), sphereOf("b", 1e8)] },
      CAMERA,
      VIEWPORT,
      surveyed,
    );
    expect([
      off.draws.filter((d) => d.material.startsWith("occluder")).map((d) => d.material),
      off.spheres.length,
    ]).toEqual([["occluderSphere"], 12]);
  });

  it("draws the surveyed body's own lines, casing and stroke, with linesSurveyed", () => {
    expect(
      lineDraws(packed.draws).map((d) => [d.material, d.surveyedBody ?? null, d.instanceCount]),
    ).toEqual([
      ["lines", null, 1],
      ["lines", null, 1],
      ["linesSurveyed", "b", 1],
      ["linesSurveyed", "b", 1],
      ["linesSurveyed", "b", 1],
      ["linesSurveyed", "b", 1],
      ["lines", null, 1],
      ["lines", null, 1],
    ]);
  });

  it("gives a surveyed line R02's line uniforms with its datum's axes and stretch and its body's centre", () => {
    const surveyedLines = lineDraws(packed.draws);
    const r02 = lineDraws(plain.draws);
    expect([2, 3, 4, 5].map((n) => uniformsOf(surveyedLines[n]))).toEqual(
      [2, 3, 4, 5].map((n) =>
        Object.assign(uniformsOf(r02[n]), {
          coverageAxes: [...B.axes],
          coverageStretch: [Math.fround(B.stretch), 0, 0, 0],
          coverageCentreM: [0, 0, -1e7, 0],
        }),
      ),
    );
  });

  it("draws every other line as R02 did", () => {
    const surveyedLines = lineDraws(packed.draws);
    const r02 = lineDraws(plain.draws);
    expect([0, 1, 6, 7].map((n) => surveyedLines[n])).toEqual([0, 1, 6, 7].map((n) => r02[n]));
  });

  it("draws a surveyed body's lines as R02's where the list holds no sphere of it", () => {
    const sphereless = packWireframe(
      { ...list, occluderSpheres: [sphereOf("a", -2e6)], lines: [bodyBatch("b", "major")] },
      CAMERA,
      VIEWPORT,
      surveyed,
    );
    expect(lineDraws(sphereless.draws).map((d) => [d.material, "surveyedBody" in d])).toEqual([
      ["lines", false],
      ["lines", false],
    ]);
  });

  it("writes the same segments, corners and sprites whether or not a body is surveyed", () => {
    expect([packed.segments, packed.corners, packed.sprites]).toEqual([
      plain.segments,
      plain.corners,
      plain.sprites,
    ]);
  });

  it("packs R02's draws with no body surveyed: one sphere draw for every sphere, every line R02's", () => {
    expect(
      packWireframe(list, CAMERA, VIEWPORT, new Map()).draws.map((d) => [
        d.material,
        d.instanceCount,
        "surveyedBody" in d,
      ]),
    ).toEqual([
      ["occluderSphere", 3, false],
      ...Array.from({ length: 8 }, () => ["lines", 1, false]),
    ]);
  });

  it("packs with an empty survey exactly as with none given", () => {
    expect(packWireframe(list, CAMERA, VIEWPORT, new Map())).toEqual(plain);
  });
});

/** An engine that records each material's spec too. */
class SpecRecordingEngine extends RecordingEngine {
  readonly specs: WgslMaterialSpec[] = [];

  override createMaterial(spec: WgslMaterialSpec): MaterialHandle {
    this.specs.push(spec);
    return super.createMaterial(spec);
  }
}

describe("WireframeRenderer over the bodies' terrain (R10.T11)", () => {
  const MASK: TextureHandle = { kind: "texture", name: "coverage mask b" };
  const TERRAIN_DRAW: DrawItem = {
    mesh: { kind: "mesh", name: "terrain lit patch" },
    material: { kind: "material", name: "terrain-wire-face-differences" },
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: {},
  };
  const CUBE: DrawItem = {
    mesh: { kind: "mesh", name: "sky cube triangle" },
    material: { kind: "material", name: "sky:cubeDisplay" },
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: {},
  };
  const list: WireframeDrawList = {
    ...EMPTY,
    occluderSpheres: [sphereOf("a", -2e6), sphereOf("b", 0)],
    occluderMeshes: [
      {
        id: "hull",
        originF32: new Float32Array([0, 0, -1]),
        triangles: new Float32Array(9).fill(1),
        twoSided: true,
        fill: "#05080d",
      },
    ],
    lines: [bodyBatch("b", "major"), aBatch({ id: "mark:craft" })],
    sprites: [
      {
        id: "star",
        directionF32: new Float32Array([0, 0, -1]),
        xPx: 960,
        yPx: 540,
        exposedRgb: [1, 1, 1],
        illuminanceLx: 1e-6,
      },
    ],
  };
  const terrain: WireframeTerrain = {
    draws: [TERRAIN_DRAW],
    surveyed: new Map([["b", { ...datumOf(2), mask: MASK }]]),
  };

  it("encodes the occluders, the surveyed sphere among them, then the terrain, the background and the rest", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const frame = renderer.frame(list, CAMERA, VIEWPORT, [CUBE], terrain);
    expect(frame.draws.map((d) => d.material.name)).toEqual([
      "wireframe:occluderSphere",
      "wireframe:occluderSphereSurveyed",
      "wireframe:hullSilhouette",
      "terrain-wire-face-differences",
      "sky:cubeDisplay",
      "wireframe:starSprite",
      "wireframe:linesSurveyed",
      "wireframe:linesSurveyed",
      "wireframe:lines",
      "wireframe:lines",
    ]);
  });

  it("binds the body's coverage mask to its surveyed draws alone", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const frame = renderer.frame(list, CAMERA, VIEWPORT, [], terrain);
    expect(
      frame.draws
        .filter((d) => d !== TERRAIN_DRAW)
        .map((d) => [d.material.name, Object.values(d.textures).map((texture) => texture.name)]),
    ).toEqual([
      ["wireframe:occluderSphere", []],
      ["wireframe:occluderSphereSurveyed", ["coverage mask b"]],
      ["wireframe:hullSilhouette", []],
      ["wireframe:starSprite", []],
      ["wireframe:linesSurveyed", ["coverage mask b"]],
      ["wireframe:linesSurveyed", ["coverage mask b"]],
      ["wireframe:lines", []],
      ["wireframe:lines", []],
    ]);
  });

  it("binds the mask by the materials' name for it", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const frame = renderer.frame(list, CAMERA, VIEWPORT, [], terrain);
    const surveyedDraws = frame.draws.filter((d) => d.material.name.endsWith("Surveyed"));
    expect(surveyedDraws.map((d) => Object.keys(d.textures))).toEqual([
      ["coverageMask"],
      ["coverageMask"],
      ["coverageMask"],
    ]);
  });

  it("binds each surveyed draw's storage buffer by its material's", () => {
    const renderer = new WireframeRenderer(new RecordingEngine());
    const frame = renderer.frame(list, CAMERA, VIEWPORT, [], terrain);
    expect(
      frame.draws
        .filter((d) => d.material.name.endsWith("Surveyed"))
        .map((d) => [d.material.name, Object.keys(d.storageBuffers ?? {})]),
    ).toEqual([
      ["wireframe:occluderSphereSurveyed", ["spheres"]],
      ["wireframe:linesSurveyed", ["segments"]],
      ["wireframe:linesSurveyed", ["segments"]],
    ]);
  });

  it("adds the terrain's draws alone, after the occluders, where no body's datum gives way", () => {
    const r02 = new WireframeRenderer(new RecordingEngine()).frame(list, CAMERA, VIEWPORT, [CUBE]);
    const withTerrain = new WireframeRenderer(new RecordingEngine()).frame(
      list,
      CAMERA,
      VIEWPORT,
      [CUBE],
      { draws: [TERRAIN_DRAW], surveyed: new Map() },
    );
    const names = r02.draws.map((d) => d.material.name);
    expect(withTerrain.draws.map((d) => d.material.name)).toEqual([
      ...names.slice(0, 2),
      TERRAIN_DRAW.material.name,
      ...names.slice(2),
    ]);
  });

  it("gives R02's frame with no terrain", () => {
    const given = new WireframeRenderer(new RecordingEngine()).frame(
      list,
      CAMERA,
      VIEWPORT,
      [CUBE],
      NO_WIREFRAME_TERRAIN,
    );
    const r02 = new WireframeRenderer(new RecordingEngine()).frame(list, CAMERA, VIEWPORT, [CUBE]);
    expect(given).toEqual(r02);
  });

  it("renders into the view the frame with the bodies' terrain", () => {
    const frames: FrameSubmission[] = [];
    const view: RenderView = {
      name: "view",
      resize: () => undefined,
      render: (frame) => {
        frames.push(frame);
      },
      readBack: () => Promise.resolve(new Uint8Array()),
      dispose: () => undefined,
    };
    new WireframeRenderer(new RecordingEngine()).render(view, list, CAMERA, VIEWPORT, [], terrain);
    const expected = new WireframeRenderer(new RecordingEngine()).frame(
      list,
      CAMERA,
      VIEWPORT,
      [],
      terrain,
    );
    expect(frames).toEqual([expected]);
  });

  it("makes its seven materials, the datum's two among them", () => {
    const engine = new SpecRecordingEngine();
    new WireframeRenderer(engine).dispose();
    expect(engine.specs.map((spec) => [spec.name, spec.displayName])).toEqual([
      ["wireframe:lines", "WIREFRAME LINES"],
      ["wireframe:occluderSphere", "BODY OCCLUDER"],
      ["wireframe:occluderHull", "HULL OCCLUDER"],
      ["wireframe:hullSilhouette", "HULL SILHOUETTE"],
      ["wireframe:starSprite", "STAR SPRITES"],
      ["wireframe:linesSurveyed", "DATUM LINES"],
      ["wireframe:occluderSphereSurveyed", "DATUM OCCLUDER"],
    ]);
  });
});
