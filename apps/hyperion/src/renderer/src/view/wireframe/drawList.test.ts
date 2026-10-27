import { describe, expect, it } from "vitest";

import { add, dot, norm, normalise, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { ColourTokens } from "../../spatial/paint";
import {
  aBody,
  aMarkedViewScene,
  aViewCraft,
  aViewScene,
  aViewStar,
  FIXTURE_MOON,
  FIXTURE_PLANET,
  FIXTURE_PLANET_CENTRE_M,
  FIXTURE_SHIP,
  FIXTURE_SYSTEM,
  NO_TURN,
  OFF_PLANET_CAMERA,
} from "../../test/viewFixtures";
import { quaternionFromAxisAngle } from "../camera/quaternion";
import {
  DEFAULT_FOV_DEG,
  type ProjectionCamera,
  project,
  type Viewport,
} from "../camera/projection";
import { overlayDrawList } from "../photoreal/overlay";
import { surveyEdgeMark, type SurveyEdgeView } from "../terrain/surveyEdge";
import { type CameraTarget, presetPose } from "../camera/state";
import type { ViewPosition } from "../coords/position";
import { relativeToCamera } from "../coords/relative";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { CoverageView, fieldCellIndex } from "../terrain/coverage";
import { MIN_STROKE_DEVICE_PX } from "../../lib/strokes";
import {
  DESTINATION_LABEL_PLACES,
  type DestinationLabelPlace,
  placeIsRight,
  placeIsUpper,
  SIZE_CLASS_REM,
  SYMBOL_STROKE_PX,
} from "../../spatial/symbols";
import { occluderRadius } from "../depth/depth";
import { narrow } from "../coords/narrow";
import { type HullOutline, hullOutline, TEST_HULL } from "../scene/hull";
import { type BodyMarkSymbol, cameraSceneOf, sceneOrigins, type ViewScene } from "../scene/model";
import {
  buildWireframeDrawList,
  CASING_PX,
  casingDevicePx,
  datumInsideAt,
  type DrawCamera,
  type DrawOptions,
  type LineBatch,
  type LineOfSightOptions,
  LOW_SETTING_MAX_SPRITES,
  occluderSlopePxAt,
  STROKE_PX,
  strokeReachPx,
  type ViewStrokes,
  viewStrokesAt,
  type WireframeDrawList,
} from "./drawList";
import { hullBoundingRadiusM } from "./hulls";
import { stretched } from "./spheroid";
import { type BodyFigure, PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { SATURN_OBLATE_SPHEROID } from "../atmosphere/reference/cases";
import { lineOfSightTripod, markBoxPx, type ScreenSegment, segmentDistancePx } from "./symbology";

/** Stand-in colours, one distinct string per token, as `readTokens` would read them. */
const TOKENS: ColourTokens = {
  text: "colour-text",
  textMuted: "colour-text-muted",
  accent: "colour-accent",
  target: "colour-target",
  line: "colour-line",
  surface0: "colour-surface-0",
};

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };

/** Strokes at a scale of 1: each width as the guide gives it, the outlines not moved, no least gap. */
const UNSCALED: ViewStrokes = {
  strokeScale: 1,
  markStrokePx: SYMBOL_STROKE_PX,
  markShiftPx: 0,
  minReticleGapPx: 0,
};

const OPTIONS: DrawOptions = {
  lowSetting: false,
  ev100: -1,
  selection: null,
  destination: null,
  remPx: 16,
  ...UNSCALED,
};

/** A free camera 3 × 10⁷ m from the planet along +z, looking at it. */
const CAMERA: DrawCamera = OFF_PLANET_CAMERA;

/** `TEST_HULL`'s plate's corners in `f32`, as an unturned hull's draw list has them. */
const PLATE_CORNERS = TEST_HULL.vertices.slice(9, 13).map((v) => narrow(v));

/**
 * How many of the items packed in `packed`, `size` corners of three `f32` each (a hull mesh's
 * triangles, a batch's segments), lie wholly on `TEST_HULL`'s plate.
 */
function onThePlate(packed: Float32Array, size: number): number {
  const corner = (i: number): Float32Array => packed.subarray(i * 3, i * 3 + 3);
  const onPlate = (c: Float32Array): boolean =>
    PLATE_CORNERS.some((p) => p.every((value, axis) => value === c[axis]));
  return Array.from({ length: packed.length / (3 * size) }, (_, item) => item).filter((item) =>
    Array.from({ length: size }, (_, j) => corner(item * size + j)).every(onPlate),
  ).length;
}

/** The labels of the body marks of a list, `null` for a body drawn larger than its symbol. */
function bodyLabels(list: WireframeDrawList): unknown[] {
  return list.anchors.filter((a) => a.target.kind === "body").map((a) => a.label?.kind ?? null);
}

/** A body's centre as a view position. */
function centre(body: string): ViewPosition {
  return { kind: "body", body, m: vec3(0, 0, 0) };
}

/** The fixture scene with the moon's orbit drawn, a craft on a predicted path, and a sky. */
function scene(): ViewScene {
  return aMarkedViewScene();
}

function build(options: Partial<DrawOptions> = {}, input: ViewScene = scene()): WireframeDrawList {
  return buildWireframeDrawList(input, CAMERA, VIEWPORT, TOKENS, { ...OPTIONS, ...options });
}

/** A cell of the planet's survey at level 5. */
interface SurveyCell {
  readonly face: 0 | 1 | 2 | 3 | 4 | 5;
  readonly i: number;
  readonly j: number;
}

/** The four cells about face +z's centre, which faces the camera, below it. */
const BELOW_FACE_CENTRE: ReadonlyArray<SurveyCell> = [
  { face: 2, i: 14, j: 14 },
  { face: 2, i: 15, j: 14 },
  { face: 2, i: 16, j: 14 },
  { face: 2, i: 17, j: 14 },
];

/** The planet's survey as the list's options give it: `cells` surveyed on a body of `figure`. */
function planetSurvey(
  input: ViewScene,
  figure: { equatorialRadiusM: number; polarRadiusM: number },
  cells: ReadonlyArray<SurveyCell>,
): SurveyEdgeView {
  const centreM = relativeToCamera(centre(FIXTURE_PLANET), CAMERA.pose, sceneOrigins(input));
  const merged = CoverageView.empty(5).merged(
    {
      ranges: cells.map((cell) => {
        const start = fieldCellIndex({ ...cell, level: 5 });
        return { start, end: start + 1, resolution: 12 };
      }),
    },
    1,
  );
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return {
    body: FIXTURE_PLANET,
    coverage: merged.coverage,
    figure,
    rotation: IDENTITY_ROTATION,
    centreM,
    heightAtM: () => null,
  };
}

/**
 * The fixture's list with the planet's survey edge marked: `cells` surveyed at level 5 (by default
 * the four about face +z's centre, which faces the camera) on a body of `figure`, with `options`.
 */
function withSurveyEdge(
  input: ViewScene = scene(),
  figure = { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6 },
  cells: ReadonlyArray<SurveyCell> = BELOW_FACE_CENTRE,
  options: Partial<DrawOptions> = {},
): WireframeDrawList {
  return build({ surveyEdges: [planetSurvey(input, figure, cells)], ...options }, input);
}

/** Whether the sight line from the camera to `p`, m from it, meets the sphere before reaching it. */
function behindSphere(p: Vec3, centreM: Vec3, radiusM: number): boolean {
  // |t p − c|² = r² along the sight line t p, t from 0 at the camera to 1 at the point.
  const a = dot(p, p);
  const b = -2 * dot(p, centreM);
  const c = dot(centreM, centreM) - radiusM * radiusM;
  const discriminant = b * b - 4 * a * c;
  return discriminant >= 0 && (-b - Math.sqrt(discriminant)) / (2 * a) < 1;
}

describe("buildWireframeDrawList", () => {
  it("is a function of the scene, camera, viewport and tokens alone", () => {
    expect(build()).toEqual(build());
  });

  it("draws orbits solid in --text-muted at 1 px", () => {
    const orbit = build().lines.find((line) => line.id === `orbit:${FIXTURE_MOON}`);
    expect([orbit?.token, orbit?.colour, orbit?.widthPx, orbit?.dash]).toEqual([
      "textMuted",
      "colour-text-muted",
      1,
      null,
    ]);
  });

  it("draws the selected orbit in --text at 2 px", () => {
    const orbit = build({ selection: { kind: "body", body: FIXTURE_MOON } }).lines.find(
      (line) => line.id === `orbit:${FIXTURE_MOON}`,
    );
    expect([orbit?.token, orbit?.widthPx]).toEqual(["text", 2]);
  });

  it("cases every mark in --surface-0, so that one over a star sprite reads", () => {
    const list = build({ selection: { kind: "body", body: FIXTURE_PLANET } });
    // Hull edges excepted: their own faces hide the stars behind them. The photorealistic
    // style's overlay cases them too (R07.T16.a).
    const uncased = list.lines.filter(
      (line) =>
        !line.id.startsWith("hull:") &&
        !(line.casingWidthPx > 0 && line.casingColour === TOKENS.surface0),
    );
    expect({ sprites: list.sprites.length > 0, uncased }).toEqual({ sprites: true, uncased: [] });
  });

  it("holds no coordinate larger than its own mark's camera-relative reach, narrowed", () => {
    const input = scene();
    const origins = sceneOrigins(input);
    const from = (p: ViewPosition): number => norm(relativeToCamera(p, CAMERA.pose, origins));
    const reach = new Map<string, number>([
      ...input.bodies.map((body): [string, number] => [
        body.id,
        from(centre(body.id)) + body.radiusM,
      ]),
      ...input.orbits.map((orbit): [string, number] => [
        orbit.body,
        from(
          orbit.parent === null
            ? { kind: "system", system: FIXTURE_SYSTEM, m: vec3(0, 0, 0) }
            : centre(orbit.parent),
        ) +
          orbit.orbit.semiMajorAxisM * (1 + orbit.orbit.eccentricity),
      ]),
      ...input.craft.map((craft): [string, number] => [
        craft.id,
        Math.max(
          craft.hull.lengthM,
          ...(craft.predictedPath ?? []).map((pose) => from(pose.position)),
        ),
      ]),
    ]);
    const list = build({ selection: { kind: "body", body: FIXTURE_PLANET } });
    const over = list.lines.flatMap((line) => {
      const owner = line.id.split(":")[1] ?? "";
      const bound = line.space === "screen" ? 2 * VIEWPORT.widthPx : (reach.get(owner) ?? 0);
      const largest = Math.max(...[...line.segments].map((v) => Math.abs(v)));
      return largest <= Math.fround(bound)
        ? []
        : [`${line.id}: ${String(largest)} > ${String(bound)}`];
    });
    const occluders = list.occluderSpheres.filter(
      (sphere) =>
        Math.max(...[...sphere.centreF32].map(Math.abs)) > Math.fround(reach.get(sphere.id) ?? 0),
    );
    expect({ over, occluders }).toEqual({ over: [], occluders: [] });
  });

  it("gives each occluder sphere its altitude above the occluder, differenced in f64", () => {
    const origins = sceneOrigins(scene());
    const gaps = build().occluderSpheres.map((sphere) => {
      const distanceM = norm(relativeToCamera(centre(sphere.id), CAMERA.pose, origins));
      const bodyRadiusM = scene().bodies.find((body) => body.id === sphere.id)?.radiusM ?? NaN;
      return sphere.altitudeM - (distanceM - occluderRadius(bodyRadiusM, distanceM));
    });
    expect(gaps.length > 0 && gaps.every((gap) => Math.abs(gap) < 1e-6)).toBe(true);
  });

  it("labels another craft's mark with its range", () => {
    const craft = build().anchors.filter((anchor) => anchor.target.kind === "craft");
    expect(craft.map((anchor) => anchor.label?.kind)).toEqual(["target"]);
  });

  it("anchors no mark that is in front of the camera but outside the view", () => {
    // Turned 40° about +y: the other craft, 3° off the old axis, falls outside the 60° field.
    const turned: DrawCamera = {
      ...CAMERA,
      pose: {
        ...CAMERA.pose,
        orientation: quaternionFromAxisAngle(vec3(0, 1, 0), (40 * Math.PI) / 180),
      },
    };
    const list = buildWireframeDrawList(scene(), turned, VIEWPORT, TOKENS, OPTIONS);
    expect(list.anchors.some((anchor) => anchor.target.kind === "craft")).toBe(false);
  });

  it("names a body drawn as its symbol, and no body drawn larger", () => {
    const base = scene();
    const tiny = {
      ...base,
      bodies: base.bodies.map((body) => Object.assign({}, body, { radiusM: 1 })),
    };
    expect([
      bodyLabels(build({}, tiny)).every((kind) => kind === "symbol"),
      bodyLabels(build()).includes(null),
    ]).toEqual([true, true]);
  });

  it("brackets another craft in --text", () => {
    const brackets = build().lines.filter((line) => line.id.startsWith("mark:target"));
    expect(brackets.map((line) => line.token)).toEqual(["text"]);
  });

  it("draws the graticule in --text-muted, its equator and prime meridian a step heavier", () => {
    const list = build();
    const major = list.lines.find((line) => line.id === `body:${FIXTURE_PLANET}:major`);
    const minor = list.lines.find((line) => line.id === `body:${FIXTURE_PLANET}:graticule`);
    expect([major?.token, major?.widthPx, minor?.token, minor?.widthPx]).toEqual([
      "textMuted",
      1.5,
      "textMuted",
      1,
    ]);
  });

  it("dashes a craft's predicted path, and nothing else", () => {
    const dashed = build().lines.filter((line) => line.dash !== null);
    expect(dashed.map((line) => line.id)).toEqual(["predicted:other"]);
  });

  it("draws hull edges uncased in the wireframe", () => {
    expect(build().lines.find((line) => line.id === "hull:other")?.casingWidthPx).toBe(0);
  });

  it("draws a hull over two-sided occluder faces with no hardware bias of their own", () => {
    const mesh = build().occluderMeshes.find((m) => m.id === "other");
    expect([mesh?.twoSided, mesh !== undefined && "depthBiasAway" in mesh]).toEqual([true, false]);
  });

  it("gives TEST_HULL's mesh its 14 opaque faces and none of its window's (R07.T16.e)", () => {
    const triangles =
      build().occluderMeshes.find((m) => m.id === "other")?.triangles ?? new Float32Array(0);
    expect([triangles.length / 9, onThePlate(triangles, 3)]).toEqual([14, 0]);
  });

  it("fills none of the wireframe's hull meshes, which hide by depth alone (R07.T16.e)", () => {
    expect([...new Set(build().occluderMeshes.map((m) => m.fill))]).toEqual([null]);
  });

  it("still draws the four edges of TEST_HULL's window (R07.T16.e)", () => {
    const segments =
      build().lines.find((line) => line.id === "hull:other")?.segments ?? new Float32Array(0);
    expect(onThePlate(segments, 2)).toBe(4);
  });

  it("marks a body's survey edge as a cased --text line 1 px wide in view space (R10.T10.c)", () => {
    const batch = withSurveyEdge().lines.find(
      (line) => line.id === `survey-edge:${FIXTURE_PLANET}`,
    );
    expect([
      batch?.space,
      batch?.token,
      batch?.colour,
      batch?.widthPx,
      batch?.dash,
      batch?.casingWidthPx,
      batch?.casingColour,
      (batch?.segments.length ?? 0) > 0,
    ]).toEqual(["view", "text", "colour-text", 1, null, CASING_PX, "colour-surface-0", true]);
  });

  it("gives a drawn survey edge its nearest range from the own ship (R10.T10.c)", () => {
    // From the own ship, some 3 × 10⁷ m from the planet's near side.
    const edges = withSurveyEdge().surveyEdges;
    expect([edges.length, (edges[0]?.nearestRangeM ?? 0) > 1e7]).toEqual([1, true]);
  });

  it("marks no survey edge where it is given none (R10.T10.c)", () => {
    expect(build().surveyEdges).toEqual([]);
  });

  it("draws every point of a mid-latitude survey edge on an oblate body in front of every occluder (R10.T10.c)", () => {
    // Earth's figure, 6,378.1 × 6,356.8 km: at 47° the spheroid lies some 11 km inside the
    // equatorial sphere, which an occluder of the equatorial radius would hide it behind.
    const figure = { equatorialRadiusM: 6.3781e6, polarRadiusM: 6.3568e6 };
    const input = scene();
    const planet = input.bodies.find((body) => body.id === FIXTURE_PLANET);
    if (planet === undefined) {
      throw new Error("the fixture scene has its planet");
    }
    const oblate: ViewScene = {
      ...input,
      bodies: [
        { ...planet, radiusM: figure.equatorialRadiusM },
        ...input.bodies.filter((body) => body !== planet),
      ],
    };
    const list = withSurveyEdge(oblate, figure, [{ face: 2, i: 1, j: 16 }]);
    const batch = list.lines.find((line) => line.id === `survey-edge:${FIXTURE_PLANET}`);
    const points = Array.from({ length: (batch?.segments.length ?? 0) / 3 }, (_, n) =>
      vec3(
        batch?.segments[n * 3] ?? 0,
        batch?.segments[n * 3 + 1] ?? 0,
        batch?.segments[n * 3 + 2] ?? 0,
      ),
    );
    const hidden = points.filter((p) =>
      list.occluderSpheres.some((sphere) =>
        behindSphere(
          p,
          vec3(sphere.centreF32[0] ?? 0, sphere.centreF32[1] ?? 0, sphere.centreF32[2] ?? 0),
          sphere.radiusM,
        ),
      ),
    );
    expect([points.length > 0, hidden.length]).toEqual([true, 0]);
  });

  it("draws occluders, then lines, then sprites, and carries its strokes and slope term", () => {
    expect(Object.keys(build())).toEqual([
      "occluderSpheres",
      "occluderMeshes",
      "lines",
      "sprites",
      "anchors",
      "strokeScale",
      "markStrokePx",
      "occluderSlopePx",
      "surveyEdges",
      "lineOfSight",
    ]);
  });

  it("draws the sky's sprites in place of the interim stars once given them (R06.T13.c)", () => {
    const withStars = {
      ...scene(),
      stars: [aViewStar({ direction: normalise(vec3(0.01, 0, -1)), absoluteV: 0 })],
    };
    const sky = {
      id: "7",
      direction: normalise(vec3(0, 0.02, -1)),
      illuminanceRgbLx: [2e-6, 3e-6, 4e-6] as const,
    };
    const sprites = build({ skyStars: [sky] }, withStars).sprites;
    expect(sprites.map((sprite) => sprite.id)).toEqual(["7"]);
    expect(sprites[0]?.illuminanceLx).toBeCloseTo(
      0.2126 * 2e-6 + 0.7152 * 3e-6 + 0.0722 * 4e-6,
      15,
    );
  });

  it("caps sprites at 2,000 by flux at the low setting", () => {
    const many = {
      ...scene(),
      stars: Array.from({ length: 2_500 }, (_, i) =>
        aViewStar({
          id: `03${i.toString(16).padStart(14, "0")}`,
          direction: normalise(vec3(((i % 50) - 25) / 100, (Math.floor(i / 50) - 25) / 150, -1)),
          absoluteV: 5 - (i % 97) * 0.05,
        }),
      ),
    };
    const all = build({}, many).sprites;
    const low = build({ lowSetting: true }, many).sprites;
    const kept = new Set(low.map((s) => s.id));
    const dimmestKept = Math.min(...low.map((s) => s.illuminanceLx));
    const brightestDropped = Math.max(
      ...all.filter((s) => !kept.has(s.id)).map((s) => s.illuminanceLx),
    );
    expect({
      all: all.length > LOW_SETTING_MAX_SPRITES,
      low: low.length,
      byFlux: dimmestKept >= brightestDropped,
    }).toEqual({ all: true, low: LOW_SETTING_MAX_SPRITES, byFlux: true });
  });

  it("keeps every one of the sky's sprites at the low setting, beyond the interim field's 2,000 (R07.T17)", () => {
    // R06's low selection holds 2,048, and a star it makes a sprite is in no bake.
    const skyStars = Array.from({ length: 2_048 }, (_, i) => ({
      id: String(i),
      direction: normalise(vec3(((i % 64) - 32) / 100, (Math.floor(i / 64) - 16) / 100, -1)),
      illuminanceRgbLx: [2e-6, 3e-6, 4e-6] as const,
    }));
    expect(build({ lowSetting: true, skyStars }).sprites).toHaveLength(2_048);
  });

  it("draws the flight path marker for the own ship's velocity", () => {
    expect(build().lines.some((line) => line.id.startsWith("mark:flight_path"))).toBe(true);
  });

  it("brackets the selection in --accent and marks the destination with --target chevrons", () => {
    const list = build({
      selection: { kind: "body", body: FIXTURE_PLANET },
      destination: { kind: "body", body: FIXTURE_PLANET },
    });
    const tokens = list.lines
      .filter(
        (line) => line.id.startsWith("mark:selection") || line.id.startsWith("mark:destination"),
      )
      .map((line) => line.token);
    expect(tokens).toEqual(["accent", "target"]);
  });
});

/** The fixture scene with a ring about the planet, facing the camera. */
function ringedScene(): ViewScene {
  return {
    ...scene(),
    rings: [
      { body: FIXTURE_PLANET, innerRadiusM: 1e7, outerRadiusM: 1.5e7, normal: vec3(0, 0, 1) },
    ],
  };
}

describe("buildWireframeDrawList's datum lines (R10.T11)", () => {
  it("names the planet as the body of its limb's and graticule's batches", () => {
    const planet = build()
      .lines.filter((line) => line.id.startsWith(`body:${FIXTURE_PLANET}:`))
      .map((line) => [line.id, line.body]);
    expect(planet).toEqual([
      [`body:${FIXTURE_PLANET}:major`, FIXTURE_PLANET],
      [`body:${FIXTURE_PLANET}:graticule`, FIXTURE_PLANET],
    ]);
  });

  it("names each body's own batches after the body they are drawn of", () => {
    const own = build().lines.filter((line) => line.id.startsWith("body:"));
    expect([own.length > 0, own.map((line) => line.body)]).toEqual([
      true,
      own.map((line) => line.id.split(":")[1]),
    ]);
  });

  it("names no body on an orbit, a ring, a hull, a predicted path, a mark or a survey edge", () => {
    const list = withSurveyEdge(ringedScene());
    const others = list.lines.filter((line) => !line.id.startsWith("body:"));
    expect([
      [...new Set(others.map((line) => line.id.split(":")[0] ?? ""))].toSorted((a, b) =>
        a.localeCompare(b),
      ),
      others.filter((line) => "body" in line).map((line) => line.id),
    ]).toEqual([["hull", "mark", "orbit", "predicted", "ring", "survey-edge"], []]);
  });
});

/** A batch's line widths, device px: its stroke, its casing each side and its dash. */
function widths(list: WireframeDrawList, kinds: (id: string) => boolean): unknown[] {
  return list.lines
    .filter((line) => kinds(line.id))
    .map((line) => [line.id, line.widthPx, line.casingWidthPx, line.dash]);
}

const isMark = (id: string): boolean => id.startsWith("mark:");

describe("buildWireframeDrawList's strokes (R07.T16.d; decision-thin-line-contrast, item 2)", () => {
  const selected = { selection: { kind: "body", body: FIXTURE_PLANET } } as const;

  it("draws every line's width, casing and dash twice as wide at a stroke scale of 2", () => {
    const doubled = (list: WireframeDrawList): unknown[] =>
      list.lines
        .filter((line) => !isMark(line.id))
        .map((line) => [
          line.id,
          2 * line.widthPx,
          2 * line.casingWidthPx,
          line.dash === null ? null : { onPx: 2 * line.dash.onPx, offPx: 2 * line.dash.offPx },
        ]);
    const ringed = aMarkedViewScene({
      rings: [
        { body: FIXTURE_PLANET, innerRadiusM: 1e7, outerRadiusM: 1.4e7, normal: vec3(0, 0, 1) },
      ],
    });
    const atTwo = build({ ...selected, strokeScale: 2 }, ringed);
    expect({
      kinds: [...new Set(atTwo.lines.map((line) => line.id.split(":")[0] ?? ""))].toSorted(),
      lines: widths(atTwo, (id) => !isMark(id)),
      slope: [build(selected, ringed).occluderSlopePx, atTwo.occluderSlopePx],
    }).toEqual({
      kinds: ["body", "hull", "mark", "orbit", "predicted", "ring"],
      lines: doubled(build(selected, ringed)),
      slope: [3, 5],
    });
  });

  it("draws every symbology outline at the marks' width, cased as a line", () => {
    // The moon shrunk below 3 px, so that it is drawn as its symbol.
    const base = aMarkedViewScene();
    const small = {
      ...base,
      bodies: base.bodies.map((body) =>
        body.id === FIXTURE_MOON ? Object.assign({}, body, { radiusM: 1 }) : body,
      ),
    };
    const out = [0.78125, 1, 2, 3].map((ratio) => {
      const list = build(
        { ...selected, destination: selected.selection, ...viewStrokesAt(ratio) },
        small,
      );
      const marks = list.lines.filter((line) => isMark(line.id));
      return [
        [...new Set(marks.map((line) => line.id.split(":")[1] ?? ""))].toSorted(),
        [...new Set(marks.map((line) => [line.widthPx, line.casingWidthPx].join(" ")))],
      ];
    });
    const kinds = ["body_symbol", "destination", "flight_path", "selection", "target"];
    expect(out).toEqual([
      [kinds, ["2 2"]],
      [kinds, ["2 2"]],
      [kinds, ["3 2"]],
      [kinds, ["4.5 3"]],
    ]);
  });

  it("draws no line, casing or outline under 2 device px at ratios 0.78125, 1, 2 and 3", () => {
    const narrowest = [0.78125, 1, 2, 3].map((ratio) => {
      const list = build({ ...selected, ...viewStrokesAt(ratio) });
      // A hull's edge is uncased in the wireframe: its casing is none, not a narrow one.
      const drawn = list.lines.flatMap((line) =>
        line.casingWidthPx > 0 ? [line.widthPx, line.casingWidthPx] : [line.widthPx],
      );
      return Math.min(...drawn) >= MIN_STROKE_DEVICE_PX;
    });
    expect(narrowest).toEqual([true, true, true, true]);
  });

  it("draws the guide's 1, 1.5 and 2 px lines at 2, 3 and 4 device px below a ratio of 2", () => {
    const list = build({ ...selected, ...viewStrokesAt(0.78125) });
    const width = (id: string): number | undefined =>
      list.lines.find((line) => line.id === id)?.widthPx;
    expect([
      width(`body:${FIXTURE_PLANET}:graticule`),
      width(`body:${FIXTURE_PLANET}:major`),
      width("hull:other"),
      width(`orbit:${FIXTURE_MOON}`),
      list.lines.find((line) => line.id === "predicted:other")?.dash,
      list.lines.find((line) => line.id === `orbit:${FIXTURE_MOON}`)?.casingWidthPx,
    ]).toEqual([2, 3, 3, 2, { onPx: 12, offPx: 8 }, 2 * CASING_PX]);
  });

  it("carries its strokes and the occluders' slope term at each ratio", () => {
    expect(
      [0.78125, 1, 2, 3].map((ratio) => {
        const list = build(viewStrokesAt(ratio));
        return [list.strokeScale, list.markStrokePx, list.occluderSlopePx];
      }),
    ).toEqual([
      [2, 2, 5],
      [2, 2, 5],
      [2, 3, 5],
      [3, 4.5, 7],
    ]);
  });

  it("takes the slope term as the heavy cased edge's half-width and its fringe, rounded up", () => {
    expect([1, 1.25, 1.5, 2, 3].map(occluderSlopePxAt)).toEqual([3, 4, 4, 5, 7]);
  });
});

/** The mark whose label the label tests place: the planet drawn as its symbol, or the other craft. */
type LabelledMark = "craft" | 0 | 1 | 2 | 3 | 4;

/** The fixture scene, its planet shrunk below 3 px, drawn as an open circle of `mark`'s class. */
function labelScene(mark: LabelledMark): ViewScene {
  const base = aMarkedViewScene();
  const symbol: BodyMarkSymbol = { shape: "circle", sizeClass: mark === "craft" ? 2 : mark };
  return {
    ...base,
    bodies: base.bodies.map((body) =>
      body.id === FIXTURE_PLANET ? Object.assign({}, body, { radiusM: 1, symbol }) : body,
    ),
  };
}

/** The target of a labelled mark. */
function labelTarget(mark: LabelledMark): CameraTarget {
  return mark === "craft"
    ? { kind: "craft", craft: "other" }
    : { kind: "body", body: FIXTURE_PLANET };
}

/** A stroke of a reticle about a mark, device px from the view's top left. */
interface ReticleStroke {
  /** The reticle it is part of: the selection's bracket or the destination's chevrons. */
  readonly kind: "selection" | "destination";
  readonly from: { readonly xPx: number; readonly yPx: number };
  readonly to: { readonly xPx: number; readonly yPx: number };
  /** Half its drawn width, px: its ink stands this far either side of the segment. */
  readonly halfWidthPx: number;
}

/** A mark's label's place, and the reticles about it, device px. */
interface LabelPlace {
  /** The anchor's position. */
  readonly xPx: number;
  readonly yPx: number;
  /** The list's offset of the label's plate to the anchor's right. */
  readonly offsetPx: number;
  /** The list's rise of a destination's label off its chevrons' line, or `null`. */
  readonly risePx: number | null;
  /** Each reticle's outer edge to the mark's right: its farthest reach along x, and half its outline. */
  readonly reticleEdgesPx: ReadonlyArray<number>;
  /** Every stroke of the reticles drawn about the mark. */
  readonly strokes: ReadonlyArray<ReticleStroke>;
}

/** The four states a mark is taken in: neither, selected, the destination alone, and both. */
const STATES = ["none", "selected", "destination", "both"] as const;

/** A mark's state. */
type MarkState = (typeof STATES)[number];

/** The strokes of the reticle batches of a list, `kind` from each batch's ID. */
function reticleStrokes(list: WireframeDrawList): ReticleStroke[] {
  return list.lines.flatMap((line): ReticleStroke[] => {
    const kind = line.id.startsWith("mark:selection:")
      ? "selection"
      : line.id.startsWith("mark:destination:")
        ? "destination"
        : null;
    if (kind === null) {
      return [];
    }
    // Six `f32` a segment: its two ends' x, y and 0.
    return Array.from({ length: line.segments.length / 6 }, (_, i) => ({
      kind,
      from: { xPx: line.segments[i * 6] ?? 0, yPx: line.segments[i * 6 + 1] ?? 0 },
      to: { xPx: line.segments[i * 6 + 3] ?? 0, yPx: line.segments[i * 6 + 4] ?? 0 },
      halfWidthPx: line.widthPx / 2,
    }));
  });
}

/** A mark's label's place and the reticles about it, device px, in each state, at a place. */
function labelPlaces(
  mark: LabelledMark,
  ratio: number,
  interfaceScale: number,
): Record<MarkState, LabelPlace> {
  const target = labelTarget(mark);
  const strokes = viewStrokesAt(ratio);
  const remPx = 16 * interfaceScale * ratio;
  const placed = (state: MarkState): LabelPlace => {
    const list = build(
      {
        selection: state === "selected" || state === "both" ? target : null,
        destination: state === "destination" || state === "both" ? target : null,
        remPx,
        ...strokes,
      },
      labelScene(mark),
    );
    const anchor = list.anchors.find(
      (each) => JSON.stringify(each.target) === JSON.stringify(target),
    );
    if (anchor === undefined) {
      throw new Error("the label tests' mark is not in view");
    }
    const reticles = reticleStrokes(list);
    const reticleEdgesPx = (["selection", "destination"] as const).flatMap((kind) => {
      const of = reticles.filter((stroke) => stroke.kind === kind);
      return of.length === 0
        ? []
        : [
            Math.max(
              ...of.flatMap((stroke) => [
                Math.abs(stroke.from.xPx - anchor.xPx),
                Math.abs(stroke.to.xPx - anchor.xPx),
              ]),
            ) + (of[0]?.halfWidthPx ?? 0),
          ];
    });
    return {
      xPx: anchor.xPx,
      yPx: anchor.yPx,
      offsetPx: anchor.labelOffsetPx,
      risePx: anchor.labelRisePx,
      reticleEdgesPx,
      strokes: reticles,
    };
  };
  return {
    none: placed("none"),
    selected: placed("selected"),
    destination: placed("destination"),
    both: placed("both"),
  };
}

/** The craft's label's place in a list, device px. */
function craftLabelPlace(list: WireframeDrawList): [number | undefined, number | null | undefined] {
  const anchor = list.anchors.find((each) => each.target.kind === "craft");
  return [anchor?.labelOffsetPx, anchor?.labelRisePx];
}

/** How many destination chevron batches a list draws. */
function destinationChevronBatches(list: WireframeDrawList): number {
  return list.lines.filter((line) => line.id.startsWith("mark:destination:")).length;
}

/**
 * The region a destination's label's plate may take at one of its places, device px: beyond its near
 * edge, right or left of the mark at its bracket place, and beyond its near horizontal edge, above or
 * below the whole chevron set; unbounded beyond, so that a stroke clear of it is clear of the plate
 * whatever its width and height.
 */
interface PlateRegion {
  /** The plate's near vertical edge: its left at a right-hand place, its right at a left-hand one. */
  readonly nearXPx: number;
  /** The plate's near horizontal edge: its bottom at an upper place, its top at a lower one. */
  readonly nearYPx: number;
  readonly right: boolean;
  readonly upper: boolean;
}

/** The plate's region at `place` about a destination's mark. */
function plateRegion(place: LabelPlace, at: DestinationLabelPlace): PlateRegion {
  const right = placeIsRight(at);
  const upper = placeIsUpper(at);
  const risePx = place.risePx ?? Number.NaN;
  return {
    nearXPx: right ? place.xPx + place.offsetPx : place.xPx - place.offsetPx,
    nearYPx: upper ? place.yPx - risePx : place.yPx + risePx,
    right,
    upper,
  };
}

/** The least distance from a reticle stroke's ink, its segment and half its width, to a region. */
function inkDistancePx(stroke: ReticleStroke, region: PlateRegion): number {
  const at = (t: number): number => {
    const xPx = stroke.from.xPx + (stroke.to.xPx - stroke.from.xPx) * t;
    const yPx = stroke.from.yPx + (stroke.to.yPx - stroke.from.yPx) * t;
    return Math.hypot(
      Math.max(0, region.right ? region.nearXPx - xPx : xPx - region.nearXPx),
      Math.max(0, region.upper ? yPx - region.nearYPx : region.nearYPx - yPx),
    );
  };
  // A distance to a convex region is convex along a segment, so a ternary search finds its least.
  let low = 0;
  let high = 1;
  for (let step = 0; step < 100; step += 1) {
    const a = low + (high - low) / 3;
    const b = high - (high - low) / 3;
    if (at(a) <= at(b)) {
      high = b;
    } else {
      low = a;
    }
  }
  return Math.min(at(0), at(1), at((low + high) / 2)) - stroke.halfWidthPx;
}

/** The rows the destination's chevrons' ink takes, device px: their ends and half their outline. */
function chevronInkRows(place: LabelPlace): { topPx: number; bottomPx: number } {
  const chevrons = place.strokes.filter((stroke) => stroke.kind === "destination");
  const ys = chevrons.flatMap((stroke) => [stroke.from.yPx, stroke.to.yPx]);
  const halfWidthPx = chevrons[0]?.halfWidthPx ?? Number.NaN;
  return { topPx: Math.min(...ys) - halfWidthPx, bottomPx: Math.max(...ys) + halfWidthPx };
}

/** The marks, ratios and interface scales the labels' places are tested at. */
const LABEL_PLACES = (["craft", 0, 1, 2, 3, 4] as const).flatMap((mark) =>
  [0.78125, 1, 2].flatMap((ratio) =>
    [0.8, 1, 1.5].map((interfaceScale) => [mark, ratio, interfaceScale] as const),
  ),
);

/** {@link LABEL_PLACES} at each of a destination's label's four places. */
const DESTINATION_PLACES = (["craft", 0, 1, 2, 3, 4] as const).flatMap((mark) =>
  [0.78125, 1, 2].flatMap((ratio) =>
    [0.8, 1, 1.5].flatMap((interfaceScale) =>
      DESTINATION_LABEL_PLACES.map((at) => [mark, ratio, interfaceScale, at] as const),
    ),
  ),
);

/**
 * The label's transitions by selection, each way: selecting or deselecting a mark, whether or not it
 * is the destination.
 */
const SELECTING: ReadonlyArray<readonly [MarkState, MarkState]> = [
  ["none", "selected"],
  ["selected", "none"],
  ["destination", "both"],
  ["both", "destination"],
];

/** The label's transitions by the destination's report, each way. */
const REPORTED: ReadonlyArray<readonly [MarkState, MarkState]> = [
  ["none", "destination"],
  ["destination", "none"],
  ["selected", "both"],
  ["both", "selected"],
];

describe("a mark's label beside its reticles (R07.T16.g; decision-r07-t16d-followups, items 1 and (d))", () => {
  it.each(LABEL_PLACES)(
    "stands %s's plate 0.125 rem less 0.75 px beyond the bracket's outer edge, drawn or not, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      const leastPx = (0.125 * 16 * interfaceScale - 0.75) * ratio;
      // The bracket stands in one place whether or not it is drawn: where the selection draws it.
      const [bracketPx = Number.NaN] = places.selected.reticleEdgesPx;
      // The segments are `f32`: within 1e-4 px.
      const clear = STATES.map((state) => places[state].offsetPx - bracketPx >= leastPx - 1e-4);
      const drawn = STATES.map((state) => places[state].reticleEdgesPx.length);
      expect([clear, drawn]).toEqual([
        [true, true, true, true],
        [0, 1, 1, 2],
      ]);
    },
  );

  it.each(LABEL_PLACES)(
    "starts %s's label's text 0.25 rem outside the bracket's outer edge, drawn or not, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      const remPx = 16 * interfaceScale * ratio;
      const [bracketPx = Number.NaN] = places.selected.reticleEdgesPx;
      // The plate's `0.25rem` padding starts its text.
      expect(
        STATES.map(
          (state) => places[state].offsetPx + 0.25 * remPx - bracketPx >= 0.25 * remPx - 1e-4,
        ),
      ).toEqual([true, true, true, true]);
    },
  );

  it.each(LABEL_PLACES)(
    "never moves %s's label when it is selected or deselected, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      expect(
        SELECTING.map(([from, to]) => [
          places[to].offsetPx - places[from].offsetPx,
          places[to].risePx === places[from].risePx,
        ]),
      ).toEqual([
        [0, true],
        [0, true],
        [0, true],
        [0, true],
      ]);
    },
  );

  it.each(LABEL_PLACES)(
    "raises %s's label on the destination's report and returns it to its line when it ends, never moving it outward, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      expect(
        REPORTED.map(([from, to]) => [
          places[to].offsetPx - places[from].offsetPx,
          places[to].risePx !== null,
        ]),
      ).toEqual([
        [0, true],
        [0, false],
        [0, true],
        [0, false],
      ]);
    },
  );

  it.each(
    (["craft", 0, 1, 2] as const).flatMap((mark) =>
      [
        [0.78125, 2.65],
        [1, 1.25],
        [2, 0],
      ].map(([ratio = 1, shiftCssPx = 0]) => [mark, ratio, shiftCssPx] as const),
    ),
  )(
    "stands %s's label at T16.d's place at a ratio of %s: 0.75 rem and %s px",
    (mark, ratio, shiftCssPx) => {
      const offsets = [0.8, 1, 1.5].map(
        (interfaceScale) =>
          labelPlaces(mark, ratio, interfaceScale).none.offsetPx / ratio -
          0.75 * 16 * interfaceScale,
      );
      expect(offsets.map((cssPx) => Math.abs(cssPx - shiftCssPx) < 1e-9)).toEqual([
        true,
        true,
        true,
      ]);
    },
  );

  it.each([
    ["craft", 0.78125, 0.8],
    [4, 0.78125, 0.8],
    ["craft", 2, 1.5],
    [0, 1, 1],
  ] as const)(
    "stands %s's destination chevrons in one place, selected or not, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      // Alone, its one reticle; on the selection, the second, drawn after the bracket.
      const [alone] = places.destination.reticleEdgesPx;
      const [bracket, onSelection] = places.both.reticleEdgesPx;
      expect([
        Math.abs((alone ?? 0) - (onSelection ?? Number.NaN)) < 1e-4,
        (alone ?? 0) > (bracket ?? Number.POSITIVE_INFINITY),
      ]).toEqual([true, true]);
    },
  );

  it("stands a giant's and a class-4 symbol's label 0.0625 and 0.125 rem further out", () => {
    const further = ([3, 4] as const).map(
      (mark) => (labelPlaces(mark, 1, 1).none.offsetPx - labelPlaces(2, 1, 1).none.offsetPx) / 16,
    );
    expect(further.map((rem) => Math.round(rem * 1e9) / 1e9)).toEqual([0.0625, 0.125]);
  });
});

describe("a destination's label above its chevron set (R07.T16.h's follow-up; decision-r07-quality-and-destination, addenda A and B)", () => {
  it.each(DESTINATION_PLACES)(
    "stands %s's destination plate wholly beyond all four chevrons' ink by 0.25 rem, at %s and %s, %s",
    (mark, ratio, interfaceScale, at) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      const remPx = 16 * interfaceScale * ratio;
      // A plate at an upper place lies wholly above its near edge, at a lower one wholly below it.
      const gaps = (["destination", "both"] as const).map((state) => {
        const region = plateRegion(places[state], at);
        const ink = chevronInkRows(places[state]);
        return region.upper ? ink.topPx - region.nearYPx : region.nearYPx - ink.bottomPx;
      });
      expect(gaps.map((gapPx) => gapPx >= 0.25 * remPx - 1e-4)).toEqual([true, true]);
    },
  );

  it.each(DESTINATION_PLACES)(
    "stands %s's destination plate 0.25 rem clear of the selection's bracket, at %s and %s, %s",
    (mark, ratio, interfaceScale, at) => {
      // The bracket stands in one place whether or not it is drawn: where the selection draws it,
      // and the label stands in one place selected or not.
      const both = labelPlaces(mark, ratio, interfaceScale).both;
      const remPx = 16 * interfaceScale * ratio;
      const region = plateRegion(both, at);
      const bracket = both.strokes.filter((stroke) => stroke.kind === "selection");
      expect([
        bracket.length,
        bracket.every((stroke) => inkDistancePx(stroke, region) >= 0.25 * remPx - 1e-4),
      ]).toEqual([8, true]);
    },
  );

  it.each([
    [0, 13.25, 22.24],
    ["craft", 13.25, 25.19],
    [4, 15.25, 28.13],
  ] as const)(
    "stands %s's destination plate's near edge at %s px and its near horizontal edge %s px from the anchor, at a ratio of 1 and 100%",
    (mark, nearEdgePx, nearRisePx) => {
      const places = labelPlaces(mark, 1, 1);
      // A craft: the bracket 11 px out, the apices 15, the upper chevron's arm ends 7.33 ÷ √2 above
      // them, its ink 1 px more, and the plate 4 px above that.
      expect(
        [places.destination, places.both].map((place) => [
          Math.round(place.offsetPx * 100) / 100,
          Math.round((place.risePx ?? Number.NaN) * 100) / 100,
        ]),
      ).toEqual([
        [nearEdgePx, nearRisePx],
        [nearEdgePx, nearRisePx],
      ]);
    },
  );

  it("gives each mark its reach: its bracket place's outer edge, whether or not it is drawn", () => {
    const strokes = viewStrokesAt(1);
    const list = build({ ...strokes, remPx: 16 }, labelScene(0));
    const reach = (kind: CameraTarget["kind"]): number | undefined =>
      list.anchors.find((anchor) => anchor.target.kind === kind)?.markReachPx;
    // A class-0 symbol: 4 + 4 + 1 px and half its 2 px outline; a craft's contact, class 2, 6 px.
    expect([reach("body"), reach("craft")]).toEqual([10, 12]);
  });

  it("moves the label in the list that first draws the destination's chevrons", () => {
    const target = labelTarget("craft");
    const strokes = viewStrokesAt(1);
    const before = build({ ...strokes, selection: target }, labelScene("craft"));
    const reported = build(
      { ...strokes, selection: target, destination: target },
      labelScene("craft"),
    );
    const [offsetBefore, riseBefore] = craftLabelPlace(before);
    const [offsetReported, riseReported] = craftLabelPlace(reported);
    expect([
      destinationChevronBatches(before),
      destinationChevronBatches(reported),
      offsetReported === offsetBefore,
      riseBefore,
      typeof riseReported,
    ]).toEqual([0, 1, true, null, "number"]);
  });
});

describe("the destination's least gap (R07.T16.g)", () => {
  it("is lib/strokes.ts's outline and casing at each ratio: 4, 4, 5 and 7.5 px", () => {
    expect([0.78125, 1, 2, 3].map((ratio) => viewStrokesAt(ratio).minReticleGapPx)).toEqual([
      4, 4, 5, 7.5,
    ]);
  });
});

/** A point on the target, device px. */
interface PointPx {
  readonly x: number;
  readonly y: number;
}

/** A screen batch's segments, device px. */
function screenSegments(line: LineBatch | undefined): Array<readonly [PointPx, PointPx]> {
  const packed = line?.segments ?? new Float32Array(0);
  return Array.from({ length: packed.length / 6 }, (_, i) => [
    { x: packed[i * 6] ?? 0, y: packed[i * 6 + 1] ?? 0 },
    { x: packed[i * 6 + 3] ?? 0, y: packed[i * 6 + 4] ?? 0 },
  ]);
}

/** The distance from `p` to the segment from `a` to `b`. */
function toSegment(p: PointPx, a: PointPx, b: PointPx): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const t = Math.min(1, Math.max(0, ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/** Which side of the line through `a` and `b` the point `p` lies on. */
function sideOf(a: PointPx, b: PointPx, p: PointPx): number {
  return Math.sign((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x));
}

/** The least distance between two segments: none where they cross, else from an end to the other. */
function betweenSegments(
  [a, b]: readonly [PointPx, PointPx],
  [c, d]: readonly [PointPx, PointPx],
): number {
  if (sideOf(a, b, c) * sideOf(a, b, d) < 0 && sideOf(c, d, a) * sideOf(c, d, b) < 0) {
    return 0;
  }
  return Math.min(toSegment(a, c, d), toSegment(b, c, d), toSegment(c, a, b), toSegment(d, a, b));
}

/** A list's batch of a symbology mark of `kind` (`selection`, `destination`, …), the first. */
function markBatch(list: WireframeDrawList, kind: string): LineBatch | undefined {
  return list.lines.find((line) => line.id.startsWith(`mark:${kind}:`));
}

/** The screen axes from a mark outward, in the chevrons' order: up, down, left, right. */
const CHEVRON_AXES = [
  [0, -1],
  [0, 1],
  [-1, 0],
  [1, 0],
] as const;

/** Whether two lengths agree to within the `f32` segments' precision here, a thousandth of a px. */
function nearPx(a: number, b: number): boolean {
  return Math.abs(a - b) < 1e-3;
}

describe("the destination's chevrons (R07.T16.h; decision-r07-quality-and-destination, Q2)", () => {
  /**
   * The chevrons and the bracket a list draws about a labelled mark at a ratio and an interface
   * scale, and the places they are meant to stand, device px.
   */
  function drawn(mark: LabelledMark, ratio: number, interfaceScale: number) {
    const target = labelTarget(mark);
    const strokes = viewStrokesAt(ratio);
    const remPx = 16 * interfaceScale * ratio;
    const listOf = (selection: CameraTarget | null): WireframeDrawList =>
      build({ selection, destination: target, remPx, ...strokes }, labelScene(mark));
    const both = listOf(target);
    const anchor = both.anchors.find(
      (each) => JSON.stringify(each.target) === JSON.stringify(target),
    );
    // A craft's reticles stand about its contact, size class 2.
    const radiusPx = (SIZE_CLASS_REM[mark === "craft" ? 2 : mark] * remPx) / 2;
    const bracketPx = radiusPx + 0.25 * remPx + 4 * strokes.markShiftPx;
    const gapPx = Math.max(0.25 * remPx, strokes.minReticleGapPx);
    return {
      strokes,
      at: { x: anchor?.xPx ?? Number.NaN, y: anchor?.yPx ?? Number.NaN },
      bracket: screenSegments(markBatch(both, "selection")),
      chevrons: screenSegments(markBatch(both, "destination")),
      alone: screenSegments(markBatch(listOf(null), "destination")),
      gapPx,
      apexPx: bracketPx + gapPx,
      armPx: (2 * bracketPx) / 3,
    };
  }

  /** The least distance between the chevrons' and the bracket's centrelines, device px. */
  function nearest(
    chevrons: ReturnType<typeof drawn>["chevrons"],
    bracket: typeof chevrons,
  ): number {
    return Math.min(
      ...chevrons.flatMap((arm) => bracket.map((corner) => betweenSegments(arm, corner))),
    );
  }

  it.each(LABEL_PLACES)(
    "stands %s's chevrons' apices on its axes at the destination's half-size, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const { at, chevrons, apexPx } = drawn(mark, ratio, interfaceScale);

      // Each chevron is two segments: an arm's outer end to the apex, and the apex to the other's.
      expect(
        CHEVRON_AXES.map(([ax, ay], i) => {
          const apex = chevrons[2 * i]?.[1];
          return (
            apex !== undefined &&
            nearPx(apex.x - at.x, ax * apexPx) &&
            nearPx(apex.y - at.y, ay * apexPx)
          );
        }),
      ).toEqual([true, true, true, true]);
    },
  );

  it.each(LABEL_PLACES)(
    "runs %s's chevrons' arms away from the mark at 45°, each the bracket's arm, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const { chevrons, armPx } = drawn(mark, ratio, interfaceScale);

      expect(
        chevrons.map(([a, b], i) => {
          const [ax, ay] = CHEVRON_AXES[Math.floor(i / 2)] ?? [0, 0];
          // From the apex outward: away from the mark along its axis as far as across it.
          const [apex, end] = i % 2 === 0 ? [b, a] : [a, b];
          const dx = end.x - apex.x;
          const dy = end.y - apex.y;
          return (
            nearPx(Math.hypot(dx, dy), armPx) && nearPx(dx * ax + dy * ay, armPx * Math.SQRT1_2)
          );
        }),
      ).toEqual(Array.from({ length: 8 }, () => true));
    },
  );

  it.each(LABEL_PLACES)(
    "keeps every point of %s's chevrons the least gap clear of its bracket, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const { chevrons, bracket, gapPx } = drawn(mark, ratio, interfaceScale);

      expect(nearest(chevrons, bracket)).toBeGreaterThanOrEqual(gapPx - 1e-3);
    },
  );

  it.each(LABEL_PLACES)(
    "keeps %s's chevrons' casing off its bracket's full-coverage core, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const { chevrons, bracket, strokes } = drawn(mark, ratio, interfaceScale);
      // The chevrons' casing reaches w ÷ 2 + casing + 0.5 from their line; the bracket's core,
      // where it covers a texel wholly, is w ÷ 2 − 0.5 either side of its own.
      const casingReach = strokes.markStrokePx / 2 + CASING_PX * strokes.strokeScale + 0.5;
      const core = strokes.markStrokePx / 2 - 0.5;

      expect(nearest(chevrons, bracket) - casingReach).toBeGreaterThanOrEqual(core - 1e-3);
    },
  );

  it.each(LABEL_PLACES)(
    "stands %s's lone destination where its pair does, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const { chevrons, alone } = drawn(mark, ratio, interfaceScale);

      expect(alone).toEqual(chevrons);
    },
  );
});

/** The readout's point, the tripod not given way before (R10.T12). */
const AT_POINT = { hasPoint: true, rangeM: null, yielded: false } as const;

/** The view's centre, where the projection puts the line of sight. */
const VIEW_CENTRE = { xPx: VIEWPORT.widthPx / 2, yPx: VIEWPORT.heightPx / 2 };

/** The tripod's ticks in {@link OPTIONS}, from 3 to 7 px about the view's centre: 1.5 px, cased 1 px. */
const TRIPOD_TICKS = lineOfSightTripod(VIEW_CENTRE, 16, {
  markStrokePx: SYMBOL_STROKE_PX,
  markShiftPx: 0,
}).segments;

/** {@link CAMERA} as the list projects through it. */
const PROJECTION: ProjectionCamera = {
  orientation: CAMERA.pose.orientation,
  fovXRad: CAMERA.fovXRad,
};

/** The planet's survey edge across the view's centre: the strip of cells just below it. */
const THROUGH_CENTRE: ReadonlyArray<SurveyCell> = [14, 15, 16, 17].map((i) => ({
  face: 2,
  i,
  j: 15,
}));

/** The planet's figure, a sphere. */
const SPHERE = { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6 };

/** A list's tripod's batch, if it is drawn. */
function tripodOf(list: WireframeDrawList): LineBatch | undefined {
  return list.lines.find((line) => line.id === "mark:line_of_sight");
}

/** A list's survey edge's batch on the planet, if it is drawn. */
function edgeOf(list: WireframeDrawList): LineBatch | undefined {
  return list.lines.find((line) => line.id === `survey-edge:${FIXTURE_PLANET}`);
}

/** A screen batch's segments as {@link ScreenSegment}s, px. */
function screenSegmentsPx(batch: LineBatch | undefined): ScreenSegment[] {
  const s = batch?.segments ?? new Float32Array(0);
  return Array.from({ length: s.length / 6 }, (_, i): ScreenSegment => [
    { xPx: s[i * 6] ?? Number.NaN, yPx: s[i * 6 + 1] ?? Number.NaN },
    { xPx: s[i * 6 + 3] ?? Number.NaN, yPx: s[i * 6 + 4] ?? Number.NaN },
  ]);
}

/** Segments' ends' coordinates as the list packs them, in `f32`. */
function f32Ends(segments: ReadonlyArray<ScreenSegment>): number[] {
  return segments.flatMap(([a, b]) => [a.xPx, a.yPx, b.xPx, b.yPx].map((v) => Math.fround(v)));
}

/**
 * The fixture scene with the own ship moving away below the view's centre, its flight path
 * marker's fin's tip `clearRem` below the end of the tripod's lower tick: the tick ends 7 px down,
 * and the fin's tip stands 6 + 5 px above the marker's centre.
 */
function withMarkerBelow(clearRem: number): ViewScene {
  const yPx = 7 + 11 + clearRem * 16;
  // Below the centre by y px along (0, −y ÷ (s W ÷ 2), −1), s = 1 ÷ tan 30°.
  const t = yPx / (Math.sqrt(3) * (VIEWPORT.widthPx / 2));
  const base = scene();
  return {
    ...base,
    craft: base.craft.map((craft) =>
      craft.id === FIXTURE_SHIP
        ? Object.assign({}, craft, { velocityMPerS: vec3(0, -t, -1) })
        : craft,
    ),
  };
}

/** The fixture scene with the other craft on the line of sight, 10⁷ m off the planet's centre. */
function withCraftOnTheLine(): ViewScene {
  const base = scene();
  return {
    ...base,
    craft: base.craft.map((craft) =>
      craft.id === "other"
        ? Object.assign({}, craft, {
            pose: {
              position: {
                kind: "system",
                system: FIXTURE_SYSTEM,
                m: add(FIXTURE_PLANET_CENTRE_M, vec3(0, 0, 1e7)),
              },
              attitude: craft.pose.attitude,
            },
            predictedPath: null,
          })
        : craft,
    ),
  };
}

/** The fixture scene with the moon shrunk to a metre on the line of sight, drawn as its symbol. */
function withMoonOnTheLine(): ViewScene {
  const base = scene();
  return {
    ...base,
    bodies: base.bodies.map((body) =>
      body.id === FIXTURE_MOON
        ? aBody({
            id: FIXTURE_MOON,
            parent: FIXTURE_PLANET,
            kind: "moon",
            designation: "TEST MOON",
            radiusM: 1,
            centreM: add(FIXTURE_PLANET_CENTRE_M, vec3(0, 0, 1e7)),
          })
        : body,
    ),
  };
}

/** The survey edge's ticks across the view's centre in a list, px: its batch's past its line's. */
function edgeTicksPx(list: WireframeDrawList): ScreenSegment[] {
  const lineSegments =
    surveyEdgeMark(planetSurvey(scene(), SPHERE, THROUGH_CENTRE), PROJECTION, VIEWPORT, {
      remPx: 16,
      keepClear: [],
      rangeFromM: null,
    }).line.length / 6;
  const s = edgeOf(list)?.segments ?? new Float32Array(0);
  const at = (i: number): { xPx: number; yPx: number } => {
    const p = project(
      vec3(s[i] ?? Number.NaN, s[i + 1] ?? Number.NaN, s[i + 2] ?? Number.NaN),
      PROJECTION,
      VIEWPORT,
    );
    return { xPx: p.xPx, yPx: p.yPx };
  };
  return Array.from({ length: s.length / 6 - lineSegments }, (_, k): ScreenSegment => {
    const i = (lineSegments + k) * 6;
    return [at(i), at(i + 3)];
  });
}

/** A point `t` of the way along a segment. */
function alongPx(s: ScreenSegment, t: number): { xPx: number; yPx: number } {
  return {
    xPx: s[0].xPx + t * (s[1].xPx - s[0].xPx),
    yPx: s[0].yPx + t * (s[1].yPx - s[0].yPx),
  };
}

/** The least distance between two segments, px, by sampling each at 129 points: the test's own. */
function sampledDistancePx(a: ScreenSegment, b: ScreenSegment): number {
  let least = Number.POSITIVE_INFINITY;
  for (let i = 0; i <= 128; i += 1) {
    const p = alongPx(a, i / 128);
    for (let j = 0; j <= 128; j += 1) {
      const q = alongPx(b, j / 128);
      least = Math.min(least, Math.hypot(p.xPx - q.xPx, p.yPx - q.yPx));
    }
  }
  return least;
}

/** How near a segment comes to the tripod's ticks, px, by `measure`. */
function nearestTripodPx(tick: ScreenSegment, measure = segmentDistancePx): number {
  return Math.min(...TRIPOD_TICKS.map((each) => measure(tick, each)));
}

describe("the line-of-sight mark, the tripod (R10.T12)", () => {
  it("is drawn by no list given no line of sight, which is as it was", () => {
    const list = build();
    expect({
      none: build({ lineOfSight: null }),
      lineOfSight: list.lineOfSight,
      tripod: tripodOf(list),
    }).toEqual({ none: list, lineOfSight: null, tripod: undefined });
  });

  it("is not drawn where the readout has no point, its give-way kept all the same", () => {
    const list = build({ lineOfSight: { hasPoint: false, rangeM: null, yielded: false } });
    expect({ lines: list.lines, lineOfSight: list.lineOfSight }).toEqual({
      lines: build().lines,
      lineOfSight: { yielded: false, boxPx: null },
    });
  });

  it("is drawn first of the marks where the readout has a point, and changes nothing else", () => {
    const before = build().lines;
    const lines = build({ lineOfSight: AT_POINT }).lines;
    const at = lines.findIndex((line) => line.id === "mark:line_of_sight");
    expect({ at, others: lines.filter((_, i) => i !== at) }).toEqual({
      at: before.findIndex((line) => line.id.startsWith("mark:")),
      others: before,
    });
  });

  it("is drawn over the survey edge, after every view-space batch and before every other mark", () => {
    const ids = withSurveyEdge(scene(), undefined, BELOW_FACE_CENTRE, {
      lineOfSight: AT_POINT,
    }).lines.map((line) => `${line.space} ${line.id}`);
    const at = ids.indexOf("screen mark:line_of_sight");
    expect({
      edgeBefore: ids.indexOf(`view survey-edge:${FIXTURE_PLANET}`) < at,
      viewAfter: ids.slice(at).filter((id) => id.startsWith("view ")),
      marksBefore: ids.slice(0, at).filter((id) => id.startsWith("screen ")),
    }).toEqual({ edgeBefore: true, viewAfter: [], marksBefore: [] });
  });

  it("strokes three ticks about the view's centre in --text at the marks' width, cased in --surface-0", () => {
    const batch = tripodOf(build({ lineOfSight: AT_POINT }));
    expect([
      batch?.space,
      batch?.token,
      batch?.colour,
      batch?.widthPx,
      batch?.casingWidthPx,
      batch?.casingColour,
      batch?.dash,
      f32Ends(screenSegmentsPx(batch)),
    ]).toEqual([
      "screen",
      "text",
      "colour-text",
      SYMBOL_STROKE_PX,
      CASING_PX,
      "colour-surface-0",
      null,
      f32Ends(TRIPOD_TICKS),
    ]);
  });

  it("is cased over the photorealistic image as in the wireframe, at each ratio", () => {
    const casings = [1, 2].map((ratio) => {
      const list = build({ lineOfSight: AT_POINT, ...viewStrokesAt(ratio) });
      return [tripodOf(list), tripodOf(overlayDrawList(list, TOKENS))].map((batch) => [
        batch?.widthPx,
        batch?.casingWidthPx,
        batch?.casingColour,
      ]);
    });
    expect(casings).toEqual([
      [
        [2, 2, "colour-surface-0"],
        [2, 2, "colour-surface-0"],
      ],
      [
        [3, 2, "colour-surface-0"],
        [3, 2, "colour-surface-0"],
      ],
    ]);
  });

  it("gives the box of every pixel it lights, for the labels to stand clear of", () => {
    const list = build({ lineOfSight: AT_POINT });
    const box = list.lineOfSight?.boxPx ?? null;
    expect({
      yielded: list.lineOfSight?.yielded,
      box,
      holdsCentre:
        box !== null &&
        box.leftPx < VIEW_CENTRE.xPx &&
        box.leftPx + box.widthPx > VIEW_CENTRE.xPx &&
        box.topPx < VIEW_CENTRE.yPx &&
        box.topPx + box.heightPx > VIEW_CENTRE.yPx,
    }).toEqual({
      yielded: false,
      box: markBoxPx({ segments: TRIPOD_TICKS }, SYMBOL_STROKE_PX / 2 + CASING_PX + 0.5),
      holdsCentre: true,
    });
  });

  it("gives way to the flight path marker within 0.5 rem, and says so for the next frame", () => {
    const list = build({ lineOfSight: AT_POINT }, withMarkerBelow(0.4));
    expect([tripodOf(list), list.lineOfSight]).toEqual([undefined, { yielded: true, boxPx: null }]);
  });

  it("stays out while the flight path marker is within 0.75 rem, and returns beyond it", () => {
    const drawn = (clearRem: number, yielded: boolean): boolean =>
      tripodOf(
        build(
          { lineOfSight: { hasPoint: true, rangeM: null, yielded } },
          withMarkerBelow(clearRem),
        ),
      ) !== undefined;
    expect([drawn(0.6, false), drawn(0.6, true), drawn(0.8, true)]).toEqual([true, false, true]);
  });

  it("keeps its give-way through a frame with no point", () => {
    expect(
      build(
        { lineOfSight: { hasPoint: false, rangeM: null, yielded: false } },
        withMarkerBelow(0.4),
      ).lineOfSight,
    ).toEqual({ yielded: true, boxPx: null });
  });

  it.each([
    ["a craft's ticks", "target", withCraftOnTheLine],
    ["a body's symbol", "body_symbol", withMoonOnTheLine],
  ])("gives way to %s on the line of sight", (_, kind, input) => {
    const list = build({ lineOfSight: AT_POINT }, input());
    expect({
      drawn: list.lines.some((line) => line.id.startsWith(`mark:${kind}:`)),
      tripod: tripodOf(list),
      yielded: list.lineOfSight?.yielded,
    }).toEqual({ drawn: true, tripod: undefined, yielded: true });
  });

  it("stands beneath the selection's bracket and the destination's chevrons about the body it marks, which do not make it give way", () => {
    // The camera looks at the planet's centre, drawn as a sphere: its reticles stand round the
    // tripod, within 0.5 rem of its ticks.
    const planet = { kind: "body", body: FIXTURE_PLANET } as const;
    const list = build({ lineOfSight: AT_POINT, selection: planet, destination: planet });
    const ids = list.lines.map((line) => line.id);
    const at = ids.indexOf("mark:line_of_sight");
    const reticles = ["mark:selection:", "mark:destination:"].map((prefix) =>
      screenSegmentsPx(list.lines.find((line) => line.id.startsWith(prefix))),
    );
    expect({
      yielded: list.lineOfSight?.yielded,
      within: reticles.map((segments) => segments.some((segment) => nearestTripodPx(segment) < 8)),
      beneath: [
        at >= 0,
        at < ids.findIndex((id) => id.startsWith("mark:selection:")),
        at < ids.findIndex((id) => id.startsWith("mark:destination:")),
      ],
    }).toEqual({ yielded: false, within: [true, true], beneath: [true, true, true] });
  });

  it("leaves out the survey edge's ticks within 0.5 rem of its ticks, and keeps every other", () => {
    const all = edgeTicksPx(withSurveyEdge(scene(), SPHERE, THROUGH_CENTRE));
    const kept = edgeTicksPx(
      withSurveyEdge(scene(), SPHERE, THROUGH_CENTRE, { lineOfSight: AT_POINT }),
    );
    expect({
      someWithin: all.some((tick) => nearestTripodPx(tick) < 8),
      keptWithin: kept.filter((tick) => nearestTripodPx(tick, sampledDistancePx) < 8 - 0.05),
      kept: kept.length,
    }).toEqual({
      someWithin: true,
      keptWithin: [],
      kept: all.filter((tick) => nearestTripodPx(tick) >= 8).length,
    });
  });

  it("leaves the survey edge's ticks as they are where it gives way", () => {
    const input = withMarkerBelow(0.2);
    expect(
      edgeOf(withSurveyEdge(input, SPHERE, THROUGH_CENTRE, { lineOfSight: AT_POINT })),
    ).toEqual(edgeOf(withSurveyEdge(input, SPHERE, THROUGH_CENTRE)));
  });
});

/**
 * The fixture scene with the own ship alone, unturned and not moving, its hull `hull` at `offsetM`
 * from {@link CAMERA} along the system's axes: the own ship's, so that no craft's ticks stand at it.
 */
function withOwnHullAt(hull: HullOutline, offsetM: Vec3): ViewScene {
  return aViewScene({
    craft: [
      aViewCraft({
        hull,
        pose: {
          position: {
            kind: "system",
            system: FIXTURE_SYSTEM,
            m: add(CAMERA.pose.positionM, offsetM),
          },
          attitude: NO_TURN,
        },
      }),
    ],
  });
}

/** How far ahead of {@link CAMERA} the test outlines stand, m. */
const OUTLINE_AHEAD_M = 100;

/** A point `px` from the view's centre, as m across at {@link OUTLINE_AHEAD_M}: px ÷ (s W ÷ 2) × d. */
function acrossM(px: number): number {
  return (px / (Math.sqrt(3) * (VIEWPORT.widthPx / 2))) * OUTLINE_AHEAD_M;
}

/**
 * A bare outline, one edge across the view below its centre at {@link OUTLINE_AHEAD_M}, `clearRem`
 * from the end of the tripod's lower tick (7 px down), with no face.
 */
function barBelow(clearRem: number): HullOutline {
  const dropM = acrossM(7 + clearRem * 16);
  return hullOutline({
    name: "BAR",
    vertices: [vec3(-5, -dropM, 0), vec3(5, -dropM, 0)],
    edges: [[0, 1]],
    faces: [],
    windows: [],
    eyePointM: vec3(0, 0, 0),
    lengthM: 10,
  });
}

/**
 * A square plate square to the line of sight at {@link OUTLINE_AHEAD_M}, about the view's centre,
 * its edges `clearRem` beyond the tripod's ticks' reach (7 px), opaque or glazed.
 */
function plateAbout(clearRem: number, glazed: boolean): HullOutline {
  const halfM = acrossM(7 + clearRem * 16);
  return hullOutline({
    name: "PLATE",
    vertices: [
      vec3(-halfM, -halfM, 0),
      vec3(halfM, -halfM, 0),
      vec3(halfM, halfM, 0),
      vec3(-halfM, halfM, 0),
    ],
    edges: [
      [0, 1],
      [1, 2],
      [2, 3],
      [3, 0],
    ],
    faces: [
      [0, 1, 2],
      [0, 2, 3],
    ],
    windows: glazed ? [0, 1] : [],
    eyePointM: vec3(0, 0, 0),
    lengthM: 2 * halfM,
  });
}

/** Whether a list of `input` from {@link CAMERA} draws the tripod, given `lineOfSight`. */
function tripodDrawn(input: ViewScene, lineOfSight: LineOfSightOptions): boolean {
  return tripodOf(build({ lineOfSight }, input)) !== undefined;
}

/** A point at `rangeM` from the camera, the tripod not given way before. */
function pointAt(rangeM: number | null): LineOfSightOptions {
  return { hasPoint: true, rangeM, yielded: false };
}

/**
 * Whether the tripod is drawn over the own ship in `preset` on `TEST_HULL`, at a ratio of 1, in a
 * view of `viewport` at an interface rem of `remPx`, its point 100 km beyond.
 */
function tripodDrawnFrom(preset: "seat" | "chase", viewport: Viewport, remPx: number): boolean {
  const input = aViewScene();
  const pose = presetPose(preset, "forward", null, cameraSceneOf(input), input.defaultPose);
  if (pose === null) {
    throw new Error("the fixture scene has no own ship");
  }
  const camera: DrawCamera = { pose, fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180 };
  const list = buildWireframeDrawList(input, camera, viewport, TOKENS, {
    ...OPTIONS,
    ...viewStrokesAt(1),
    remPx,
    lineOfSight: pointAt(1e5),
  });
  return tripodOf(list) !== undefined;
}

/** The windows the tripod's look was ruled at (decision-r10-t12-commit3 §1). */
const RULED_WINDOWS: ReadonlyArray<Viewport> = [
  { widthPx: 1920, heightPx: 1080 },
  { widthPx: 1280, heightPx: 720 },
];

/** The interface's rem at a ratio of 1 from 80% to 150%, device px. */
const RULED_REMS_PX = [12.8, 16, 20, 24] as const;

describe("the tripod over a craft's hull (R10.T12; decision-r10-t12-commit3 §1)", () => {
  /** {@link OUTLINE_AHEAD_M} ahead of {@link CAMERA}, along its line of sight. */
  const AHEAD = vec3(0, 0, -OUTLINE_AHEAD_M);

  it("leaves out for a hull's edge within 0.5 rem of its ticks, keeps its state up to 0.75 rem, and returns beyond", () => {
    const drawn = (clearRem: number, yielded: boolean): boolean =>
      tripodDrawn(withOwnHullAt(barBelow(clearRem), AHEAD), { ...pointAt(null), yielded });
    expect({
      within: drawn(0.4, false),
      between: [drawn(0.6, false), drawn(0.6, true)],
      beyond: drawn(0.8, true),
    }).toEqual({ within: false, between: [true, false], beyond: true });
  });

  it("counts an edge that runs from behind the camera, clipped at the near plane, not dropped", () => {
    /** One edge from 10 m behind the camera to {@link OUTLINE_AHEAD_M} ahead, `clearRem` below. */
    const fromBehind = (clearRem: number): HullOutline => {
      const dropM = acrossM(7 + clearRem * 16);
      return hullOutline({
        name: "BOOM",
        vertices: [vec3(0, -dropM, OUTLINE_AHEAD_M + 10), vec3(0, -dropM, 0)],
        edges: [[0, 1]],
        faces: [],
        windows: [],
        eyePointM: vec3(0, 0, 0),
        lengthM: OUTLINE_AHEAD_M + 10,
      });
    };
    expect({
      near: tripodDrawn(withOwnHullAt(fromBehind(0.4), AHEAD), pointAt(null)),
      clear: tripodDrawn(withOwnHullAt(fromBehind(0.8), AHEAD), pointAt(null)),
    }).toEqual({ near: false, clear: true });
  });

  it("says it gave way to a hull for the next frame", () => {
    const list = build({ lineOfSight: pointAt(null) }, withOwnHullAt(barBelow(0.4), AHEAD));
    expect(list.lineOfSight).toEqual({ yielded: true, boxPx: null });
  });

  it("leaves out for an opaque face over the point with every edge 2 rem away, and not for a window", () => {
    const drawn = (glazed: boolean): boolean =>
      tripodDrawn(withOwnHullAt(plateAbout(2, glazed), AHEAD), pointAt(null));
    expect({ opaque: drawn(false), glazed: drawn(true) }).toEqual({ opaque: false, glazed: true });
  });

  it("counts no hull wholly beyond the point: its distance less its bounding radius not nearer than the point", () => {
    // TEST_HULL 1 km ahead on the line of sight, its faces over the point.
    const input = withOwnHullAt(TEST_HULL, vec3(0, 0, -1_000));
    const nearSideM = 1_000 - hullBoundingRadiusM(TEST_HULL);
    expect({
      pointBeforeIt: tripodDrawn(input, pointAt(nearSideM - 1)),
      pointPastItsNearSide: tripodDrawn(input, pointAt(nearSideM + 1)),
    }).toEqual({ pointBeforeIt: true, pointPastItsNearSide: false });
  });

  it("counts every drawn hull where the point's range is not known", () => {
    const input = withOwnHullAt(TEST_HULL, vec3(0, 0, -1_000));
    expect({
      unknown: tripodDrawn(input, pointAt(null)),
      before: tripodDrawn(input, pointAt(10)),
    }).toEqual({
      unknown: false,
      before: true,
    });
  });

  it("counts no hull behind a body's limb", () => {
    // TEST_HULL on the line of sight 10⁷ m beyond the planet's centre, and 10⁷ m before it.
    expect({
      behind: tripodDrawn(withOwnHullAt(TEST_HULL, vec3(0, 0, -4e7)), pointAt(null)),
      before: tripodDrawn(withOwnHullAt(TEST_HULL, vec3(0, 0, -2e7)), pointAt(null)),
    }).toEqual({ behind: true, before: false });
  });

  it.each(RULED_WINDOWS)(
    "leaves it out in CHASE over TEST_HULL at $widthPx × $heightPx, a ratio of 1, from 80% to 150%",
    (viewport) => {
      expect(RULED_REMS_PX.map((remPx) => tripodDrawnFrom("chase", viewport, remPx))).toEqual(
        RULED_REMS_PX.map(() => false),
      );
    },
  );

  it.each(RULED_WINDOWS)(
    "draws it in SEAT looking forward over TEST_HULL at $widthPx × $heightPx, a ratio of 1, from 80% to 150%",
    (viewport) => {
      expect(RULED_REMS_PX.map((remPx) => tripodDrawnFrom("seat", viewport, remPx))).toEqual(
        RULED_REMS_PX.map(() => true),
      );
    },
  );
});

/** The moon brought near, 1.6 × 10⁷ m from the camera beside the planet, about 350 px across. */
const NEAR_MOON = add(FIXTURE_PLANET_CENTRE_M, vec3(-6e6, 3e6, 1.5e7));

/** The moon 2 × 10⁸ m off in the view, 29 px across, its apparent radius under 24 px. */
const FAR_MOON = add(CAMERA.pose.positionM, vec3(-0.3 * 1.9e8, 0.1 * 1.9e8, -1.9e8));

/** The fixture's planet and moon, the moon's centre at `moonM`, with no craft, orbit or star. */
function planetAndMoon(moonM: Vec3): ViewScene {
  return aViewScene({
    bodies: [
      aBody(),
      aBody({
        id: FIXTURE_MOON,
        parent: FIXTURE_PLANET,
        kind: "moon",
        designation: "TEST MOON",
        radiusM: 1.737e6,
        centreM: moonM,
      }),
    ],
    craft: [],
    stars: [],
    ownShip: null,
  });
}

/** The list of `bodies` at a display ratio of 1, its datums inside or not. */
function datumListOf(bodies: ViewScene, datumInside: boolean | undefined): WireframeDrawList {
  const options: DrawOptions = { ...OPTIONS, ...viewStrokesAt(1) };
  return buildWireframeDrawList(
    bodies,
    CAMERA,
    VIEWPORT,
    TOKENS,
    datumInside === undefined ? options : { ...options, datumInside },
  );
}

/** A list's batches by name. */
function batchesById(list: WireframeDrawList): ReadonlyMap<string, LineBatch> {
  return new Map(list.lines.map((line) => [line.id, line]));
}

/** The names of a list's batches of the moon's datum. */
function moonBatchIds(list: WireframeDrawList): string[] {
  return list.lines.filter((line) => line.body === FIXTURE_MOON).map((line) => line.id);
}

/** The names of both bodies' datum batches, each body's heavy lines' and thin lines'. */
const DATUM_BATCHES = [FIXTURE_PLANET, FIXTURE_MOON].flatMap((body) =>
  ["major", "graticule"].map((kind) => `body:${body}:${kind}`),
);

describe("the bodies' datums inside their figures, for the image's overlay (R10.T16.g)", () => {
  it("moves both batches of every body's datum, which keep their names and their body", () => {
    const bodies = planetAndMoon(NEAR_MOON);
    const centred = batchesById(datumListOf(bodies, undefined));
    const inside = batchesById(datumListOf(bodies, true));
    // Each batch's tag, and whether its segments moved.
    const moved = (id: string): [string | undefined, boolean] => {
      const was = centred.get(id)?.segments ?? new Float32Array(0);
      const now = inside.get(id);
      const segments = now?.segments ?? new Float32Array(0);
      return [
        now?.body,
        segments.length > 0 &&
          (segments.length !== was.length || segments.some((value, i) => value !== was[i])),
      ];
    };
    expect(DATUM_BATCHES.map(moved)).toEqual(DATUM_BATCHES.map((id) => [id.split(":")[1], true]));
  });

  it("leaves every other mark, the occluder spheres and the anchors as they were", () => {
    const bodies = planetAndMoon(NEAR_MOON);
    const centred = datumListOf(bodies, undefined);
    const inside = datumListOf(bodies, true);
    const others = (list: WireframeDrawList): unknown[] => [
      list.lines.filter((line) => !DATUM_BATCHES.includes(line.id)),
      list.occluderSpheres,
      list.anchors,
    ];
    expect(others(inside)).toEqual(others(centred));
  });

  it("is the list as built when the option is off", () => {
    const bodies = planetAndMoon(NEAR_MOON);
    expect(datumListOf(bodies, false)).toEqual(datumListOf(bodies, undefined));
  });

  it("draws no datum over the image for a body too small to stroke inside, which the wireframe draws", () => {
    const bodies = planetAndMoon(FAR_MOON);
    expect([
      moonBatchIds(datumListOf(bodies, undefined)),
      moonBatchIds(datumListOf(bodies, true)),
    ]).toEqual([[`body:${FIXTURE_MOON}:major`, `body:${FIXTURE_MOON}:graticule`], []]);
  });

  it("keeps the occluder sphere of a body too small to stroke inside", () => {
    const inside = datumListOf(planetAndMoon(FAR_MOON), true);
    expect(inside.occluderSpheres.some((sphere) => sphere.id === FIXTURE_MOON)).toBe(true);
  });

  it("insets each line by its reach at the overlay's casing: 3.5 and 4 px at a line scale of 2, 5 and 5.75 at 3", () => {
    expect(
      [2, 3].map((strokeScale) => [
        datumInsideAt(strokeScale).reachPx,
        casingDevicePx(strokeScale),
        strokeReachPx(STROKE_PX.thin * strokeScale, casingDevicePx(strokeScale)),
      ]),
    ).toEqual([
      [{ thin: 3.5, heavy: 4 }, 2 * CASING_PX, 3.5],
      [{ thin: 5, heavy: 5.75 }, 3 * CASING_PX, 5],
    ]);
  });
});

/** `TEST SATURN`'s spheroid at 1 bar (`SATURN_OBLATE_SPHEROID`), its pole up the view. */
const GIANT = {
  equatorialRadiusM: SATURN_OBLATE_SPHEROID.equatorialRadiusM,
  polarRadiusM: SATURN_OBLATE_SPHEROID.polarRadiusM,
  pole: vec3(0, 1, 0),
} as const;

/** A camera 482 Mm from the planet along +z, looking at it, equator-on, at the default 60°. */
const GIANT_CAMERA: DrawCamera = {
  pose: {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM: add(FIXTURE_PLANET_CENTRE_M, vec3(0, 0, 4.82e8)),
    orientation: NO_TURN,
  },
  fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180,
};

/**
 * The planet as `TEST SATURN`'s figure, with no rotation and its orbit normal off its pole, or with
 * no appearance (a sphere of its equatorial radius), and a moon behind it seen just past its pole:
 * the moon's line of sight passes the planet's centre (a + c) ÷ 2 off it, outside the spheroid
 * and inside the sphere.
 */
function giantScene(figured: boolean, figure: BodyFigure = GIANT): ViewScene {
  const height = (GIANT.equatorialRadiusM + GIANT.polarRadiusM) / 2;
  const behindM = 1e9;
  const range = 4.82e8;
  return aViewScene({
    bodies: [
      aBody({
        radiusM: GIANT.equatorialRadiusM,
        orbitNormal: normalise(vec3(1, 0.4, 0.7)),
        appearance: figured
          ? {
              photometry: PROVISIONAL_PHOTOMETRY,
              figure,
              labels: [],
              bondRatioFinding: null,
            }
          : null,
      }),
      aBody({
        id: FIXTURE_MOON,
        parent: FIXTURE_PLANET,
        kind: "moon",
        designation: "TEST MOON",
        radiusM: 1e6,
        centreM: add(
          FIXTURE_PLANET_CENTRE_M,
          vec3(0, (height * (range + behindM)) / range, -behindM),
        ),
      }),
    ],
    craft: [],
    stars: [],
    ownShip: null,
  });
}

function giantList(figured: boolean, options: Partial<DrawOptions> = {}): WireframeDrawList {
  return buildWireframeDrawList(giantScene(figured), GIANT_CAMERA, VIEWPORT, TOKENS, {
    ...OPTIONS,
    ...viewStrokesAt(1),
    ...options,
  });
}

/** Whether a list marks the moon. */
function moonMarked(list: WireframeDrawList): boolean {
  return list.anchors.some(
    (anchor) => anchor.target.kind === "body" && anchor.target.body === FIXTURE_MOON,
  );
}

/** The planet's centre from the giant's camera, m. */
const GIANT_CENTRE_M = vec3(0, 0, -4.82e8);

/** How far off `TEST SATURN`'s spheroid a point from the camera lies, as a fraction of a. */
function offGiant(p: Vec3): number {
  const q = sub(p, GIANT_CENTRE_M);
  const z = dot(q, GIANT.pole);
  const rho2 = dot(q, q) - z * z;
  return Math.abs(
    Math.sqrt(rho2 / GIANT.equatorialRadiusM ** 2 + (z / GIANT.polarRadiusM) ** 2) - 1,
  );
}

/** The points of a list's planet datum batches, from the camera, m. */
function planetDatumPoints(list: WireframeDrawList): Vec3[] {
  return list.lines
    .filter((line) => line.body === FIXTURE_PLANET)
    .flatMap((line) =>
      Array.from({ length: line.segments.length / 3 }, (_, i) =>
        vec3(
          line.segments[3 * i] ?? 0,
          line.segments[3 * i + 1] ?? 0,
          line.segments[3 * i + 2] ?? 0,
        ),
      ),
    );
}

describe("a flattened body's datum on its spheroid (R02.T19)", () => {
  it.each([false, true])(
    "draws its limb and graticule on its figure, to the f32 the list packs, with the datum inside the figure %s",
    (datumInside) => {
      const off = planetDatumPoints(giantList(true, { datumInside })).map(offGiant);
      const sphere = planetDatumPoints(giantList(false, { datumInside })).map(offGiant);
      // A packed coordinate at 482 Mm is good to about 30 m, 5 × 10⁻⁷ of a; the sphere stands up
      // to (a − c) ÷ c, 11%, off the figure at its poles.
      expect([off.length > 200, Math.max(...off) < 2e-6, Math.max(...sphere) > 0.05]).toEqual([
        true,
        true,
        true,
      ]);
    },
  );

  it("occludes with its figure, both radii inset by the sphere's margin, its altitude in its stretched space", () => {
    const occluder = giantList(true).occluderSpheres.find((sphere) => sphere.id === FIXTURE_PLANET);
    const distanceM = 4.82e8;
    const a = occluderRadius(GIANT.equatorialRadiusM, distanceM);
    const c = occluderRadius(GIANT.polarRadiusM, distanceM);
    const spheroid = { pole: GIANT.pole, stretch: a / c };
    expect([occluder?.radiusM, occluder?.spheroid, occluder?.altitudeM]).toEqual([
      a,
      spheroid,
      norm(stretched(GIANT_CENTRE_M, spheroid)) - a,
    ]);
  });

  it("keeps a sphere's occluder as it was: no spheroid, its altitude d − r′", () => {
    const occluder = giantList(false).occluderSpheres.find(
      (sphere) => sphere.id === FIXTURE_PLANET,
    );
    const r = occluderRadius(GIANT.equatorialRadiusM, 4.82e8);
    expect(occluder).toEqual({
      id: FIXTURE_PLANET,
      centreF32: narrow(GIANT_CENTRE_M),
      radiusM: r,
      altitudeM: 4.82e8 - r,
    });
  });

  it("marks a moon behind it seen past its pole, outside the figure, which its sphere would hide", () => {
    expect([moonMarked(giantList(true)), moonMarked(giantList(false))]).toEqual([true, false]);
  });

  it("draws a body whose figure is a sphere as one with no figure", () => {
    const round = giantScene(true, { ...GIANT, polarRadiusM: GIANT.equatorialRadiusM });
    const options = { ...OPTIONS, ...viewStrokesAt(1) };
    expect(buildWireframeDrawList(round, GIANT_CAMERA, VIEWPORT, TOKENS, options)).toEqual(
      giantList(false),
    );
  });
});
