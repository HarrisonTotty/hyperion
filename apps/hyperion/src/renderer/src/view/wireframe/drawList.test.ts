import { describe, expect, it } from "vitest";

import { norm, normalise, vec3 } from "../../geometry/vec3";
import type { ColourTokens } from "../../spatial/paint";
import {
  aMarkedViewScene,
  aViewStar,
  FIXTURE_MOON,
  FIXTURE_PLANET,
  FIXTURE_SYSTEM,
  OFF_PLANET_CAMERA,
} from "../../test/viewFixtures";
import { quaternionFromAxisAngle } from "../camera/quaternion";
import type { Viewport } from "../camera/projection";
import type { CameraTarget } from "../camera/state";
import type { ViewPosition } from "../coords/position";
import { relativeToCamera } from "../coords/relative";
import { MIN_STROKE_DEVICE_PX } from "../../lib/strokes";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX } from "../../spatial/symbols";
import { occluderRadius } from "../depth/depth";
import { narrow } from "../coords/narrow";
import { TEST_HULL } from "../scene/hull";
import { type BodyMarkSymbol, sceneOrigins, type ViewScene } from "../scene/model";
import {
  buildWireframeDrawList,
  CASING_PX,
  type DrawCamera,
  type DrawOptions,
  type LineBatch,
  LOW_SETTING_MAX_SPRITES,
  occluderSlopePxAt,
  type ViewStrokes,
  viewStrokesAt,
  type WireframeDrawList,
} from "./drawList";

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

/** The four states a mark is taken in: neither, selected, the destination alone, and both. */
const STATES = ["none", "selected", "destination", "both"] as const;

/** A mark's state. */
type MarkState = (typeof STATES)[number];

/** A mark's label's place and the reticles about it, device px, in each state, at a place. */
function labelPlaces(
  mark: LabelledMark,
  ratio: number,
  interfaceScale: number,
): Record<MarkState, { offsetPx: number; reticleEdgesPx: number[] }> {
  const target = labelTarget(mark);
  const strokes = viewStrokesAt(ratio);
  const remPx = 16 * interfaceScale * ratio;
  const placed = (state: MarkState): { offsetPx: number; reticleEdgesPx: number[] } => {
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
    // Each reticle's outer edge to the mark's right: its corners' farthest reach along x, and half
    // its outline.
    const reticleEdgesPx = list.lines
      .filter(
        (line) => line.id.startsWith("mark:selection:") || line.id.startsWith("mark:destination:"),
      )
      .map(
        (line) =>
          Math.max(
            ...Array.from({ length: line.segments.length / 3 }, (_, i) =>
              Math.abs((line.segments[i * 3] ?? 0) - anchor.xPx),
            ),
          ) +
          line.widthPx / 2,
      );
    return { offsetPx: anchor.labelOffsetPx, reticleEdgesPx };
  };
  return {
    none: placed("none"),
    selected: placed("selected"),
    destination: placed("destination"),
    both: placed("both"),
  };
}

/** The craft's label's offset in a list, device px. */
function craftLabelOffset(list: WireframeDrawList): number | undefined {
  return list.anchors.find((anchor) => anchor.target.kind === "craft")?.labelOffsetPx;
}

/** How many destination reticles a list draws. */
function destinationReticles(list: WireframeDrawList): number {
  return list.lines.filter((line) => line.id.startsWith("mark:destination:")).length;
}

/** The marks, ratios and interface scales the labels' places are tested at. */
const LABEL_PLACES = (["craft", 0, 1, 2, 3, 4] as const).flatMap((mark) =>
  [0.78125, 1, 2].flatMap((ratio) =>
    [0.8, 1, 1.5].map((interfaceScale) => [mark, ratio, interfaceScale] as const),
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
    "stands %s's plate 0.125 rem less 0.75 px clear of every reticle about it, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      const leastPx = (0.125 * 16 * interfaceScale - 0.75) * ratio;
      // The segments are `f32`: within 1e-4 px.
      const clear = STATES.map((state) =>
        places[state].reticleEdgesPx.every(
          (edgePx) => places[state].offsetPx - edgePx >= leastPx - 1e-4,
        ),
      );
      const drawn = STATES.map((state) => places[state].reticleEdgesPx.length);
      expect([clear, drawn]).toEqual([
        [true, true, true, true],
        [0, 1, 1, 2],
      ]);
    },
  );

  it.each(LABEL_PLACES)(
    "never moves %s's label when it is selected or deselected, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      expect(SELECTING.map(([from, to]) => places[to].offsetPx - places[from].offsetPx)).toEqual([
        0, 0, 0, 0,
      ]);
    },
  );

  it.each(LABEL_PLACES)(
    "moves %s's label out on the destination's report and back when it ends, at %s and %s",
    (mark, ratio, interfaceScale) => {
      const places = labelPlaces(mark, ratio, interfaceScale);
      expect(
        REPORTED.map(([from, to]) => Math.sign(places[to].offsetPx - places[from].offsetPx)),
      ).toEqual([1, -1, 1, -1]);
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
    "stands %s's destination reticle in one place, selected or not, at %s and %s",
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

  it("stands a craft's label 22.44 px from its anchor while it is the destination, at a ratio of 1 and 100%", () => {
    const places = labelPlaces("craft", 1, 1);
    // The bracket 11 px out, the apices 15, the reach 15 + 7.33 ÷ √2; the plate 2.25 px beyond.
    expect(
      [places.destination.offsetPx, places.both.offsetPx].map(
        (offsetPx) => Math.round(offsetPx * 100) / 100,
      ),
    ).toEqual([22.44, 22.44]);
  });

  it("stands a giant's and a class-4 symbol's label 0.0625 and 0.125 rem further out", () => {
    const further = ([3, 4] as const).map(
      (mark) => (labelPlaces(mark, 1, 1).none.offsetPx - labelPlaces(2, 1, 1).none.offsetPx) / 16,
    );
    expect(further.map((rem) => Math.round(rem * 1e9) / 1e9)).toEqual([0.0625, 0.125]);
  });

  it("moves the label in the list that first draws the destination's reticle", () => {
    const target = labelTarget("craft");
    const strokes = viewStrokesAt(1);
    const before = build({ ...strokes, selection: target }, labelScene("craft"));
    const reported = build(
      { ...strokes, selection: target, destination: target },
      labelScene("craft"),
    );
    expect([
      destinationReticles(before),
      destinationReticles(reported),
      (craftLabelOffset(reported) ?? 0) > (craftLabelOffset(before) ?? Number.POSITIVE_INFINITY),
    ]).toEqual([0, 1, true]);
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
