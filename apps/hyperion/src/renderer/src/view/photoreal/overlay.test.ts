import { describe, expect, it } from "vitest";

import { add, normalise, vec3 } from "../../geometry/vec3";
import type { ColourTokens } from "../../spatial/paint";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import {
  aBody,
  aMarkedViewScene,
  aViewScene,
  FIXTURE_PLANET,
  FIXTURE_PLANET_CENTRE_M,
  FIXTURE_SYSTEM,
  NO_TURN,
  OFF_PLANET_CAMERA,
} from "../../test/viewFixtures";
import { rotationColumns } from "../bodies/smoothMesh";
import { lookAlong, rotationRows } from "../camera/quaternion";
import { DEFAULT_FOV_DEG, project, type Viewport } from "../camera/projection";
import { IDENTITY_ROTATION, rotation3FromRows } from "../coords/rotation";
import type { TextureHandle } from "../engine/types";
import type { ViewScene } from "../scene/model";
import { ATMOSPHERE_FIXTURES } from "../scenes/atmosphereScene";
import {
  buildWireframeDrawList,
  CASING_PX,
  type DrawCamera,
  type LineBatch,
  viewStrokesAt,
  type WireframeDrawList,
} from "../wireframe/drawList";
import { linearColour, type SurveyedDatumFrame, WireframeRenderer } from "../wireframe/submit";
import {
  depthObserved,
  hiddenMarksCulled,
  overlayDrawList,
  overlayDrawsAt,
  type OverlayHider,
  overlayHidersOf,
  overlaySubmission,
  type SurveyedPlan,
  surveyedDatumsOf,
} from "./overlay";

/** The guide's tokens, as `readTokens` reads them from the stylesheet. */
const TOKENS: ColourTokens = {
  text: "#c8d6e5",
  textMuted: "#8a9db3",
  accent: "#5cc8e6",
  target: "#e879f9",
  line: "#1c2a3a",
  surface0: "#05080d",
};

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };

const CAMERA = OFF_PLANET_CAMERA;

/**
 * A scene with every kind of mark the overlay draws: the planet's limb and graticule, a ring, the
 * moon's orbit, another craft's hull and its predicted path, the selection's bracket and the
 * other craft's target mark, and stars.
 */
function scene(): ViewScene {
  return aMarkedViewScene({
    rings: [
      {
        body: FIXTURE_PLANET,
        innerRadiusM: 1e7,
        outerRadiusM: 1.4e7,
        normal: normalise(vec3(0, 0.5, 0.866)),
      },
    ],
  });
}

/** The wireframe's list of the scene, its strokes at a display ratio, 1 by default. */
function wireframeList(ratio = 1): WireframeDrawList {
  return buildWireframeDrawList(scene(), CAMERA, VIEWPORT, TOKENS, {
    lowSetting: false,
    ev100: -1,
    selection: { kind: "body", body: FIXTURE_PLANET },
    destination: null,
    remPx: 16 * ratio,
    ...viewStrokesAt(ratio),
  });
}

/** Each mesh's shape, its fill aside: its craft, origin, triangles and sidedness. */
function shape(meshes: WireframeDrawList["occluderMeshes"]): unknown[] {
  return meshes.map((mesh) => [mesh.id, mesh.originF32, mesh.triangles, mesh.twoSided]);
}

/** The kinds of the batches of a list, from their stable names: `hull`, `ring`, `mark`, … */
function batchKinds(list: WireframeDrawList): ReadonlyArray<string> {
  return [...new Set(list.lines.map((line) => line.id.split(":")[0] ?? ""))].toSorted();
}

describe("the photorealistic overlay's draw list (R07.T16.a)", () => {
  it("cases every mark over the image in --surface-0, the hull's edges included", () => {
    const overlay = overlayDrawList(wireframeList(), TOKENS);
    const casingPx = CASING_PX * overlay.strokeScale;
    const uncased = overlay.lines
      .filter((line) => !(line.casingWidthPx === casingPx && line.casingColour === TOKENS.surface0))
      .map((line) => line.id);
    expect({ kinds: batchKinds(overlay), uncased }).toEqual({
      kinds: ["body", "hull", "mark", "orbit", "predicted", "ring"],
      uncased: [],
    });
  });

  it("draws no star sprite, the image's, and keeps every occluder and mark anchor", () => {
    const list = wireframeList();
    const overlay = overlayDrawList(list, TOKENS);
    expect([
      list.sprites.length > 0,
      overlay.sprites,
      overlay.occluderSpheres === list.occluderSpheres,
      shape(overlay.occluderMeshes),
      overlay.anchors === list.anchors,
    ]).toEqual([true, [], true, shape(list.occluderMeshes), true]);
  });

  it("fills every hull's opaque faces in --surface-0, a silhouette, as the wireframe does not (R07.T16.e)", () => {
    const list = wireframeList();
    const overlay = overlayDrawList(list, TOKENS);
    expect({
      meshes: overlay.occluderMeshes.length > 0,
      overlay: [...new Set(overlay.occluderMeshes.map((mesh) => mesh.fill))],
      wireframe: [...new Set(list.occluderMeshes.map((mesh) => mesh.fill))],
    }).toEqual({ meshes: true, overlay: [TOKENS.surface0], wireframe: [null] });
  });

  it("cases every batch to 2 device px at ratios of 0.78125, 1 and 2, and 3 at 3 (R07.T16.d)", () => {
    expect(
      [0.78125, 1, 2, 3].map((ratio) =>
        Array.from(
          new Set(
            overlayDrawList(wireframeList(ratio), TOKENS).lines.map((line) => line.casingWidthPx),
          ),
        ),
      ),
    ).toEqual([[2], [2], [2], [3]]);
  });

  it("leaves the wireframe's own list as it was, its hull edges uncased", () => {
    const list = wireframeList();
    overlayDrawList(list, TOKENS);
    expect(list.lines.find((line) => line.id === "hull:other")?.casingWidthPx).toBe(0);
  });
});

describe("the photorealistic overlay with the view's GRID at OFF (the owner's request, 2026-11-05)", () => {
  it("keeps only the stand-ins, each hull and the ring's two edges, which the image does not draw yet", () => {
    const off = overlayDrawList(wireframeList(), TOKENS, false);
    expect(off.lines.map((line) => line.id)).toEqual([`ring:${FIXTURE_PLANET}`, "hull:other"]);
  });

  it("leaves out every limb and graticule, orbit, ring tick, predicted path and mark", () => {
    const list = wireframeList();
    const kept = new Set(overlayDrawList(list, TOKENS, false).lines.map((line) => line.id));
    expect(
      [
        ...new Set(
          list.lines
            .filter((line) => !kept.has(line.id))
            .map((line) => line.id.split(":")[0] ?? ""),
        ),
      ].toSorted((a, b) => a.localeCompare(b)),
    ).toEqual(["body", "mark", "orbit", "predicted", "ring"]);
  });

  it("keeps every occluder and each hull's silhouette, so that a stand-in behind a body stays hidden", () => {
    const list = wireframeList();
    const overlay = overlayDrawList(list, TOKENS, false);
    expect([
      overlay.occluderSpheres === list.occluderSpheres,
      shape(overlay.occluderMeshes),
      [...new Set(overlay.occluderMeshes.map((mesh) => mesh.fill))],
    ]).toEqual([true, shape(list.occluderMeshes), [TOKENS.surface0]]);
  });

  it("draws every batch at ON", () => {
    const list = wireframeList();
    expect(overlayDrawList(list, TOKENS, true).lines.map((line) => line.id)).toEqual(
      list.lines.map((line) => line.id),
    );
  });
});

describe("whether the overlay's pass is drawn (the owner's request, 2026-11-05)", () => {
  /** The scene's list with only the batches `GRID OFF` leaves out: no hull, ring or survey edge. */
  function bareList(): WireframeDrawList {
    const list = wireframeList();
    return { ...list, lines: list.lines.filter((line) => line.keptAtGridOff !== true) };
  }

  it("is drawn at ON, whatever its list holds", () => {
    expect([overlayDrawsAt(wireframeList(), true), overlayDrawsAt(bareList(), true)]).toEqual([
      true,
      true,
    ]);
  });

  it("is drawn at OFF only where a stand-in or survey edge stands in the list", () => {
    expect([overlayDrawsAt(wireframeList(), false), overlayDrawsAt(bareList(), false)]).toEqual([
      true,
      false,
    ]);
  });
});

describe("the symbology's canvas pass (R07.T16.a)", () => {
  it("is labelled for the pass timer and loads the tone-mapped image beneath it", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const pass = overlaySubmission(renderer, wireframeList(), TOKENS, CAMERA, VIEWPORT, new Map());
    expect([pass.label, pass.colourLoad, pass.encoding]).toEqual(["symbology", "load", undefined]);
    renderer.dispose();
  });

  it("strokes each batch's --surface-0 casing, two casings wider, before the batch itself", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const list = wireframeList();
    const pass = overlaySubmission(renderer, list, TOKENS, CAMERA, VIEWPORT, new Map());
    const lines = pass.draws.filter((draw) => draw.material.name === "wireframe:lines");
    const casing = [...linearColour(TOKENS.surface0)];
    // Each batch with a segment is drawn twice, its casing then its stroke, in the list's order.
    const drawn = list.lines.filter((line) => line.segments.length > 0);
    const pairs = drawn.map((_, i) => {
      const under = lines[2 * i];
      const over = lines[2 * i + 1];
      return [
        [...(under?.uniforms["colour"] ?? [])],
        (under?.uniforms["widthPx"]?.[0] ?? 0) - (over?.uniforms["widthPx"]?.[0] ?? 0),
        over?.uniforms["widthPx"]?.[0],
      ];
    });
    expect({ draws: lines.length, pairs }).toEqual({
      draws: 2 * drawn.length,
      pairs: drawn.map((line) => [casing, 2 * CASING_PX * list.strokeScale, line.widthPx]),
    });
    renderer.dispose();
  });

  it("draws the bodies' occluder spheres, then the silhouettes, then every line (R07.T16.e)", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const pass = overlaySubmission(renderer, wireframeList(), TOKENS, CAMERA, VIEWPORT, new Map());
    const kinds = pass.draws.map((draw) => draw.material.name);
    // The order of each kind's first and last draw.
    const span = (name: string): readonly [number, number] => [
      kinds.indexOf(name),
      kinds.lastIndexOf(name),
    ];
    const [sphereFirst, sphereLast] = span("wireframe:occluderSphere");
    const [silhouetteFirst, silhouetteLast] = span("wireframe:hullSilhouette");
    const [lineFirst] = span("wireframe:lines");
    expect([sphereFirst >= 0, sphereLast < silhouetteFirst, silhouetteLast < lineFirst]).toEqual([
      true,
      true,
      true,
    ]);
    renderer.dispose();
  });
});

/** A coverage mask's handle, as `CoverageMask.texture` gives it. */
const MASK: TextureHandle = { kind: "texture", name: "coverage mask" };

/** A rotation with no axis in common with the galactic ones. */
const TURNED = rotation3FromRows(rotationRows(lookAlong(normalise(vec3(1, 2, 3)), vec3(0, 0, 1))));

describe("the symbology's canvas pass over a surveyed body (R10.T16.f)", () => {
  const datum: SurveyedDatumFrame = { mask: MASK, axes: rotationColumns(TURNED), stretch: 1.0034 };

  it("draws the body's own lines, casing and stroke, with linesSurveyed and its occluder with occluderSphereSurveyed, each binding its mask", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const list = wireframeList();
    const pass = overlaySubmission(
      renderer,
      list,
      TOKENS,
      CAMERA,
      VIEWPORT,
      new Map([[FIXTURE_PLANET, datum]]),
    );
    const own = list.lines.filter(
      (line) => line.body === FIXTURE_PLANET && line.segments.length > 0,
    ).length;
    const surveyed = pass.draws.filter((draw) => draw.material.name.endsWith("Surveyed"));
    const named = (name: string): number =>
      surveyed.filter((draw) => draw.material.name === name).length;
    expect({
      own: own > 0,
      lines: named("wireframe:linesSurveyed"),
      spheres: named("wireframe:occluderSphereSurveyed"),
      masks: [...new Set(surveyed.map((draw) => draw.textures["coverageMask"]))],
      stretch: [...new Set(surveyed.map((draw) => draw.uniforms["coverageStretch"]?.[0]))],
    }).toEqual({
      own: true,
      lines: 2 * own,
      spheres: 1,
      masks: [MASK],
      stretch: [Math.fround(1.0034)],
    });
    renderer.dispose();
  });

  it("draws every datum whole with no body surveyed, as R07 built the pass", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const pass = overlaySubmission(renderer, wireframeList(), TOKENS, CAMERA, VIEWPORT, new Map());
    expect(
      pass.draws.filter(
        (draw) => draw.material.name.endsWith("Surveyed") || Object.keys(draw.textures).length > 0,
      ),
    ).toEqual([]);
    renderer.dispose();
  });

  it("keeps the casing over the image of the surveyed body's lines, and every other line R02's", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const list = wireframeList();
    const surveyed = new Map([[FIXTURE_PLANET, datum]]);
    const lines = overlaySubmission(
      renderer,
      list,
      TOKENS,
      CAMERA,
      VIEWPORT,
      surveyed,
    ).draws.filter((draw) => draw.material.name.startsWith("wireframe:lines"));
    const casing = [...linearColour(TOKENS.surface0)];
    const drawn = list.lines.filter((line) => line.segments.length > 0);
    expect(
      drawn.map((_, i) => [
        lines[2 * i]?.material.name,
        [...(lines[2 * i]?.uniforms["colour"] ?? [])],
        lines[2 * i + 1]?.material.name,
      ]),
    ).toEqual(
      drawn.map((line) => {
        const material =
          line.body === FIXTURE_PLANET ? "wireframe:linesSurveyed" : "wireframe:lines";
        return [material, casing, material];
      }),
    );
    renderer.dispose();
  });
});

/** `TEST EARTH`'s figure's radius, m, a sphere of 6,371,008.8 m (`ATMOSPHERE_FIXTURES`). */
function testEarthRadiusM(): number {
  const earth = ATMOSPHERE_FIXTURES.find((fixture) => fixture.designation === "TEST EARTH");
  if (earth === undefined) {
    throw new Error("ATMOSPHERE_FIXTURES has no TEST EARTH");
  }
  return earth.figure.equatorialRadiusM;
}

/** `TEST EARTH`'s radius, m. */
const TEST_EARTH_RADIUS_M = testEarthRadiusM();

/** The range at which the datum covered the air's limb glow, m from the body's centre: 51 Mm. */
const GLOW_RANGE_M = 51e6;

/** A camera 51 Mm from the planet, looking at its centre at the default 60°. */
const AT_GLOW_RANGE: DrawCamera = {
  pose: {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM: add(FIXTURE_PLANET_CENTRE_M, vec3(0, 0, GLOW_RANGE_M)),
    orientation: NO_TURN,
  },
  fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180,
};

/** The planet as `TEST EARTH`, alone: no craft, orbit or star. */
function testEarthScene(): ViewScene {
  return aViewScene({
    bodies: [aBody({ designation: "TEST EARTH", radiusM: TEST_EARTH_RADIUS_M })],
    craft: [],
    stars: [],
    ownShip: null,
  });
}

/** The overlay's list of `TEST EARTH` at 51 Mm, at a display ratio, its datum inside or centred. */
function testEarthOverlay(ratio: number, datumInside: boolean): WireframeDrawList {
  const list = buildWireframeDrawList(testEarthScene(), AT_GLOW_RANGE, VIEWPORT, TOKENS, {
    lowSetting: false,
    ev100: -1,
    selection: null,
    destination: null,
    remPx: 16 * ratio,
    ...viewStrokesAt(ratio),
    datumInside,
  });
  return overlayDrawList(list, TOKENS);
}

/**
 * How far inside `TEST EARTH`'s outline a point of a segment is seen, device px: on the view's
 * axis the outline is the circle of radius f tan α about the view's centre, α = asin(r ÷ d).
 */
function insetPx(x: number, y: number, z: number): number {
  const p = project(
    vec3(x, y, z),
    { orientation: AT_GLOW_RANGE.pose.orientation, fovXRad: AT_GLOW_RANGE.fovXRad },
    VIEWPORT,
  );
  const focalPx = VIEWPORT.widthPx / (2 * Math.tan(AT_GLOW_RANGE.fovXRad / 2));
  const outlinePx =
    (focalPx * TEST_EARTH_RADIUS_M) / Math.sqrt(GLOW_RANGE_M ** 2 - TEST_EARTH_RADIUS_M ** 2);
  return outlinePx - Math.hypot(p.xPx - VIEWPORT.widthPx / 2, p.yPx - VIEWPORT.heightPx / 2);
}

/**
 * The least distance inside the outline of a batch's segments, at their ends and at their
 * quarter points, less the reach of its cased stroke as the overlay draws it: its width ÷ 2, its
 * casing and the half pixel the coverage's fringe adds; negative where it passes the outline.
 */
function leastMarginPx(batch: LineBatch): number {
  const s = batch.segments;
  const reachPx = batch.widthPx / 2 + batch.casingWidthPx + 0.5;
  let least = Number.POSITIVE_INFINITY;
  for (let i = 0; i + 5 < s.length; i += 6) {
    for (const t of [0, 0.25, 0.5, 0.75, 1]) {
      const at = (axis: number): number =>
        (s[i + axis] ?? Number.NaN) * (1 - t) + (s[i + 3 + axis] ?? Number.NaN) * t;
      least = Math.min(least, insetPx(at(0), at(1), at(2)));
    }
  }
  return least - reachPx;
}

/** The planet's own batches, the limb's and graticule's, and the equator's and prime meridian's. */
function ownBatches(list: WireframeDrawList): ReadonlyArray<LineBatch> {
  return list.lines.filter((line) => line.body === FIXTURE_PLANET);
}

describe("the overlay's datum over TEST EARTH at 51 Mm (R10.T16.g)", () => {
  it.each([
    [1, 2],
    [3, 3],
  ])(
    "covers no pixel outside the figure's silhouette at a display ratio of %s, a line scale of %s: no segment widened by its cased reach passes the outline",
    (ratio, strokeScale) => {
      const overlay = testEarthOverlay(ratio, true);
      const own = ownBatches(overlay);
      expect({
        strokeScale: overlay.strokeScale,
        batches: own.map((line) => line.id.split(":").at(-1)),
        inside: own.map((line) => leastMarginPx(line) >= 0),
      }).toEqual({ strokeScale, batches: ["major", "graticule"], inside: [true, true] });
    },
  );

  it.each([1, 3])(
    "insets the limb by the casing the overlay draws, at a display ratio of %s: its least margin within a twentieth of a pixel",
    (ratio) => {
      const limbs = testEarthOverlay(ratio, true).lines.find(
        (line) => line.id === `body:${FIXTURE_PLANET}:graticule`,
      );
      const margin = limbs === undefined ? Number.NaN : leastMarginPx(limbs);
      expect([margin >= 0, margin <= 0.05]).toEqual([true, true]);
    },
  );

  it("lays the centred datum, as the overlay drew it before, over the image beyond the figure", () => {
    const own = ownBatches(testEarthOverlay(1, false));
    expect(own.map((line) => leastMarginPx(line) < -3)).toEqual([true, true]);
  });
});

/** Three bodies' IDs. */
const A = "0200080020000000.0301";
const B = "0200080020000000.0302";
const C = "0200080020000000.0303";

/** A second mask, the third body's. */
const OTHER_MASK: TextureHandle = { kind: "texture", name: "another coverage mask" };

describe("the datums that give way over the image, from the frame's plan (R10.T16.f)", () => {
  const figure = { equatorialRadiusM: 2.44e6, polarRadiusM: 2.43e6 };

  it("gives a body handed over with a mask its role's mask, its rotation's columns and its figure's a ÷ c", () => {
    const plan: SurveyedPlan = {
      meshes: [
        {
          index: 1,
          role: {
            kind: "handed-over",
            coverage: { mask: MASK, rotation: TURNED, whole: false },
            detailShare: 1,
          },
        },
      ],
      discs: [{ body: B }, { body: A }],
    };
    expect(surveyedDatumsOf(plan, [{ id: A, figure }])).toEqual(
      new Map([[A, { mask: MASK, axes: rotationColumns(TURNED), stretch: 2.44e6 / 2.43e6 }]]),
    );
  });

  it("gives none for a body promoted to a mesh, one handed over without a mask, or one not among the bodies", () => {
    const plan: SurveyedPlan = {
      meshes: [
        { index: 0, role: { kind: "promoted" } },
        { index: 1, role: { kind: "handed-over", coverage: null, detailShare: 1 } },
        {
          index: 2,
          role: {
            kind: "handed-over",
            coverage: { mask: OTHER_MASK, rotation: IDENTITY_ROTATION, whole: false },
            detailShare: 1,
          },
        },
        {
          index: 3,
          role: {
            kind: "handed-over",
            coverage: { mask: MASK, rotation: TURNED, whole: false },
            detailShare: 1,
          },
        },
      ],
      discs: [{ body: A }, { body: B }, { body: C }],
    };
    expect([
      ...surveyedDatumsOf(plan, [
        { id: A, figure },
        { id: B, figure },
      ]).keys(),
    ]).toEqual([]);
  });

  it("gives each of several bodies handed over its own mask and rotation", () => {
    const plan: SurveyedPlan = {
      meshes: [
        {
          index: 0,
          role: {
            kind: "handed-over",
            coverage: { mask: MASK, rotation: TURNED, whole: false },
            detailShare: 1,
          },
        },
        {
          index: 1,
          role: {
            kind: "handed-over",
            coverage: { mask: OTHER_MASK, rotation: IDENTITY_ROTATION, whole: false },
            detailShare: 1,
          },
        },
      ],
      discs: [{ body: A }, { body: C }],
    };
    const datums = surveyedDatumsOf(plan, [
      { id: C, figure: { equatorialRadiusM: 1e6, polarRadiusM: 1e6 } },
      { id: A, figure },
    ]);
    expect([...datums].map(([id, each]) => [id, each.mask, [...each.axes], each.stretch])).toEqual([
      [A, MASK, [...rotationColumns(TURNED)], 2.44e6 / 2.43e6],
      [C, OTHER_MASK, [...rotationColumns(IDENTITY_ROTATION)], 1],
    ]);
  });
});

/** The hidden marks' camera: at the origin, looking down −z, 60° across. */
const HIGH_CAMERA: DrawCamera = {
  pose: {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM: vec3(0, 0, 0),
    orientation: NO_TURN,
  },
  fovXRad: Math.PI / 3,
};

/** The planet below it: its centre 30 Mm down the view's axis, its ground outside 6.39 Mm. */
const BELOW_M = 3e7;
const INNER_RADIUS_M = 6.39e6;
const HIDER: OverlayHider = { body: "planet", innerRadiusM: INNER_RADIUS_M };

/** The cone's half-angle about the planet from the camera, rad. */
const HALF_ANGLE_RAD = Math.asin(INNER_RADIUS_M / BELOW_M);

/** A solid view-space batch of `segments`, six numbers each, 2 px wide in a 2 px casing. */
function batchOf(
  id: string,
  segments: ReadonlyArray<number>,
  extra: Partial<LineBatch> = {},
): LineBatch {
  return {
    id,
    space: "view",
    originF32: new Float32Array(3),
    segments: Float32Array.from(segments),
    token: "text",
    colour: TOKENS.text,
    widthPx: 2,
    casingWidthPx: 2,
    casingColour: TOKENS.surface0,
    dash: null,
    ...extra,
  };
}

/** An overlay's list of `lines` over the planet, whose occluder gives its centre. */
function hiddenList(
  lines: ReadonlyArray<LineBatch>,
  occluderMeshes: WireframeDrawList["occluderMeshes"] = [],
): WireframeDrawList {
  return {
    occluderSpheres: [
      {
        id: "planet",
        centreF32: Float32Array.of(0, 0, -BELOW_M),
        radiusM: 6.4e6,
        altitudeM: BELOW_M - 6.4e6,
      },
    ],
    occluderMeshes,
    lines,
    sprites: [],
    anchors: [],
    strokeScale: 1,
    markStrokePx: 2,
    occluderSlopePx: 1,
    surveyEdges: [],
    lineOfSight: null,
  };
}

/** A segment 1e11 m away from `fromRad` to `toRad` off the view's axis, across x. */
function far(fromRad: number, toRad: number): number[] {
  const d = 1e11;
  return [d * Math.tan(fromRad), 0, -d, d * Math.tan(toRad), 0, -d];
}

/** Each batch's id and its segments' count. */
function counts(list: WireframeDrawList): ReadonlyArray<readonly [string, number]> {
  return list.lines.map((line) => [line.id, line.segments.length / 6]);
}

describe("the overlay's marks a body's ground hides (perf-ground-gpu; GeometrySettings.hiddenMarks)", () => {
  it("leaves out a segment wholly behind the planet's ground, and keeps one before it and one beyond its limb", () => {
    const list = hiddenList([
      batchOf("orbit:far", far(-0.01, 0.01)),
      batchOf("orbit:near", [-10, 0, -1000, 10, 0, -1000]),
      batchOf("orbit:limb", far(0, 0.35)),
    ]);
    expect(counts(hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [HIDER]))).toEqual([
      ["orbit:near", 1],
      ["orbit:limb", 1],
    ]);
  });

  it("keeps a segment within its stroke's reach of the limb, and leaves out one past it", () => {
    // The stroke reaches 1 + 2 + 2 px past its segment, 5 px of about 0.6 mrad.
    const list = hiddenList([
      batchOf("within", far(0, HALF_ANGLE_RAD - 1e-3)),
      batchOf("past", far(0, HALF_ANGLE_RAD - 1e-2)),
    ]);
    expect(counts(hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [HIDER]))).toEqual([
      ["within", 1],
    ]);
  });

  it("tests a segment's part within the view alone, so that one running past the view's edge behind the ground is left out, its off-view end beyond the limb or not", () => {
    // From 1.5 Mm up the planet's cone, 79° about the view's axis, is wider than the view's 30°.
    const nearBelowM = 6.5e6;
    const nearList: WireframeDrawList = {
      ...hiddenList([
        batchOf("orbit:past-the-edge", far(-0.1, 1.2)),
        batchOf("orbit:past-the-limb", far(-0.1, 1.5)),
        batchOf("orbit:in-view", far(-0.1, 0.1)),
      ]),
      occluderSpheres: [
        {
          id: "planet",
          centreF32: Float32Array.of(0, 0, -nearBelowM),
          radiusM: 6.4e6,
          altitudeM: nearBelowM - 6.4e6,
        },
      ],
    };
    expect([
      Math.asin(INNER_RADIUS_M / nearBelowM) > 1.2 && Math.asin(INNER_RADIUS_M / nearBelowM) < 1.5,
      counts(hiddenMarksCulled(nearList, HIGH_CAMERA, VIEWPORT, [HIDER])),
    ]).toEqual([true, []]);
  });

  it("keeps a batch's other segments where only some lie hidden, in their order", () => {
    const list = hiddenList([
      batchOf("orbit:mixed", [...far(0, 0.35), ...far(-0.01, 0.01), ...far(0.3, 0.35)]),
    ]);
    const [kept] = hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [HIDER]).lines;
    expect([...(kept?.segments ?? [])]).toEqual([
      ...Float32Array.from([...far(0, 0.35), ...far(0.3, 0.35)]),
    ]);
  });

  it("leaves out the hider's datum lines, which its whole mask discards, and keeps another body's", () => {
    const list = hiddenList([
      batchOf("body:planet:major", [-10, 0, -1000, 10, 0, -1000], { body: "planet" }),
      batchOf("body:moon:major", [-10, 0, -1000, 10, 0, -1000], { body: "moon" }),
    ]);
    expect(counts(hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [HIDER]))).toEqual([
      ["body:moon:major", 1],
    ]);
  });

  it("keeps the hider's datum lines where the list holds no occluder sphere of it, which the packer masks them by", () => {
    const list = {
      ...hiddenList([
        batchOf("body:planet:major", [-10, 0, -1000, 10, 0, -1000], { body: "planet" }),
      ]),
      occluderSpheres: [],
    };
    expect(counts(hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [HIDER]))).toEqual([
      ["body:planet:major", 1],
    ]);
  });

  it("keeps a dashed path, a mark on a body's ground and a screen mark whole, hidden or not", () => {
    const hidden = far(-0.01, 0.01);
    const list = hiddenList([
      batchOf("predicted:craft", hidden, { dash: { onPx: 6, offPx: 4 } }),
      batchOf("survey-edge:planet", hidden, { groundCentreF32: Float32Array.of(0, 0, -BELOW_M) }),
      batchOf("mark:bracket:0", [0, 0, 0, 10, 10, 0], { space: "screen" }),
    ]);
    expect(hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [HIDER])).toEqual(list);
  });

  it("keeps every segment with no hider, or one whose occluder the list does not hold", () => {
    const list = hiddenList([batchOf("orbit:far", far(-0.01, 0.01))]);
    expect([
      hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, []),
      hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [{ body: "moon", innerRadiusM: 1e6 }]),
    ]).toEqual([list, list]);
  });

  it("says no draw tests depth once the hidden marks are out, and that one does where a mark is in view or a hull is filled", () => {
    const behind = batchOf("orbit:behind-camera", [-10, 0, 1000, 10, 0, 1000]);
    const silhouette = {
      id: "craft",
      originF32: Float32Array.of(0, 0, -100),
      triangles: new Float32Array(9),
      twoSided: true,
      fill: TOKENS.surface0,
    } as const;
    const culled = (list: WireframeDrawList): boolean =>
      depthObserved(hiddenMarksCulled(list, HIGH_CAMERA, VIEWPORT, [HIDER]), HIGH_CAMERA, VIEWPORT);
    expect([
      culled(hiddenList([batchOf("orbit:far", far(-0.01, 0.01)), behind])),
      culled(hiddenList([batchOf("mark:bracket:0", [0, 0, 0, 10, 10, 0], { space: "screen" })])),
      culled(hiddenList([batchOf("orbit:near", [-10, 0, -1000, 10, 0, -1000])])),
      culled(hiddenList([], [silhouette])),
      depthObserved(hiddenList([batchOf("orbit:far", far(-0.01, 0.01))]), HIGH_CAMERA, VIEWPORT),
    ]).toEqual([false, false, true, true, true]);
  });

  it("gives the symbology's pass as it was, and that it tests depth, with no hider, or one whose datum does not give way in it; with one, the hidden marks left out and that none does", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const list = hiddenList([batchOf("orbit:far", far(-0.01, 0.01))]);
    const surveyed = new Map([
      ["planet", { mask: MASK, axes: rotationColumns(TURNED), stretch: 1 }],
    ]);
    const passOf = (
      hiders: ReadonlyArray<OverlayHider>,
      over: ReadonlyMap<string, SurveyedDatumFrame>,
    ): readonly [number, boolean] => {
      const pass = overlaySubmission(
        renderer,
        list,
        TOKENS,
        HIGH_CAMERA,
        VIEWPORT,
        over,
        true,
        hiders,
      );
      return [
        pass.draws.filter((draw) => draw.material.name === "wireframe:lines").length,
        pass.depthTested,
      ];
    };
    expect([passOf([], surveyed), passOf([HIDER], surveyed), passOf([HIDER], new Map())]).toEqual([
      [2, true],
      [0, false],
      [2, true],
    ]);
    renderer.dispose();
  });
});

/** The planet's terrain's inner radius, and a moon's, m. */
function innerRadiusOf(body: string): number | null {
  return body === FIXTURE_PLANET ? 6.3e6 : 1e6;
}

describe("the bodies behind whose ground the overlay's marks may lie hidden, from the frame's plan", () => {
  const plan = (whole: boolean): SurveyedPlan => ({
    meshes: [
      {
        index: 0,
        role: {
          kind: "handed-over",
          coverage: { mask: MASK, rotation: IDENTITY_ROTATION, whole },
          detailShare: 1,
        },
      },
      { index: 1, role: { kind: "promoted" } },
    ],
    discs: [{ body: FIXTURE_PLANET }, { body: "e101" }],
  });

  it("gives a body handed over with a whole mask, with its terrain's inner radius, and no other", () => {
    expect([
      overlayHidersOf(plan(true), innerRadiusOf),
      overlayHidersOf(plan(false), innerRadiusOf),
      overlayHidersOf(plan(true), () => null),
    ]).toEqual([[{ body: FIXTURE_PLANET, innerRadiusM: 6.3e6 }], [], []]);
  });
});
