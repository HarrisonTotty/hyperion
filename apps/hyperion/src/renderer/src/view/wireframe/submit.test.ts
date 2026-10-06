import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { centresJustBeyond, limbPastRightEdge } from "../../test/beyondView";
import { aCameraPose, NO_TURN } from "../../test/viewFixtures";
import type { BufferHandle, MaterialHandle, MeshHandle, WgslMaterialSpec } from "../engine/types";
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
  linearColour,
  MATERIAL_BUFFER,
  OUTSIDE_VIEW_MARGIN_PX,
  packWireframe,
  sphereScreenRect,
  WIREFRAME_MATERIALS,
  type WireframeEngine,
  type WireframeMaterial,
  WireframeRenderer,
} from "./submit";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: DrawCamera = { pose: aCameraPose({ orientation: NO_TURN }), fovXRad: Math.PI / 3 };
const PROJECTION = { orientation: NO_TURN, fovXRad: Math.PI / 3 };
const MATERIALS: ReadonlyArray<WireframeMaterial> = [
  "lines",
  "occluderSphere",
  "occluderHull",
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

  it("take both occluders' slope term from the draw's uniform, not a constant", () => {
    expect([
      WIREFRAME_MATERIALS.occluderSphere.uniforms.map((u) => u.name),
      WIREFRAME_MATERIALS.occluderHull.uniforms.map((u) => u.name),
      /SLOPE_SCALE/.test(WIREFRAME_MATERIALS.occluderSphere.fragmentWgsl),
    ]).toEqual([["occluderSlopePx"], ["firstTriangle", "occluderSlopePx"], false]);
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
      ["occluderHull", 2, null, 1],
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
      (d) => d.material === "occluderSphere" || d.material === "occluderHull",
    );
    expect(occluders.map((d) => [d.material, d.uniforms["occluderSlopePx"]?.[0]])).toEqual([
      ["occluderSphere", 5],
      ["occluderHull", 5],
      ["occluderHull", 5],
    ]);
  });

  it("packs a sphere's centre, radius and altitude", () => {
    expect([...packed.spheres].slice(4, 12)).toEqual([0, 0, -1e7, 1e6, 9e6, 0, 0, 0]);
  });

  it("leaves out a sphere off the view", () => {
    expect(packed.spheres.length).toBe(12);
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

  it("makes its materials again when the engine restores its device", () => {
    const engine = new RecordingEngine();
    const renderer = new WireframeRenderer(engine);
    engine.restore();
    renderer.dispose();
    expect(engine.materials.length).toBe(8);
  });

  it("stops following the engine's restores once disposed", () => {
    const engine = new RecordingEngine();
    new WireframeRenderer(engine).dispose();
    engine.restore();
    expect(engine.materials.length).toBe(4);
  });
});
