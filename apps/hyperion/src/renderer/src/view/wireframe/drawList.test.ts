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
import type { ViewPosition } from "../coords/position";
import { relativeToCamera } from "../coords/relative";
import { MIN_STROKE_DEVICE_PX } from "../../lib/strokes";
import { SYMBOL_STROKE_PX } from "../../spatial/symbols";
import { occluderRadius } from "../depth/depth";
import { sceneOrigins, type ViewScene } from "../scene/model";
import {
  buildWireframeDrawList,
  CASING_PX,
  type DrawCamera,
  type DrawOptions,
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

/** Strokes at a scale of 1: each width as the guide gives it, the outlines not moved. */
const UNSCALED: ViewStrokes = { strokeScale: 1, markStrokePx: SYMBOL_STROKE_PX, markShiftPx: 0 };

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

  it("draws the flight path marker for the own ship's velocity", () => {
    expect(build().lines.some((line) => line.id.startsWith("mark:flight_path"))).toBe(true);
  });

  it("brackets the selection in --accent and the destination in --target", () => {
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

describe("the selection's and the destination's reticles on one target (R07.T16.d)", () => {
  it.each(
    [0.78125, 1, 2].flatMap((ratio) => [0.8, 1, 1.5].map((scale) => [ratio, scale] as const)),
  )(
    "keep the destination's casing off the brackets' full-coverage core at %s and %s",
    (ratio, scale) => {
      const craft = { kind: "craft", craft: "other" } as const;
      const strokes = viewStrokesAt(ratio);
      const remPx = 16 * scale * ratio;
      const list = build({ selection: craft, destination: craft, remPx, ...strokes });
      const anchor = list.anchors.find((each) => each.target.kind === "craft");
      // A reticle's half-size: its corners' farthest reach from the mark along x.
      const half = (kind: string): number =>
        Math.max(
          ...list.lines
            .flatMap((line) =>
              line.id.startsWith(`mark:${kind}:`)
                ? [line.segments[0] ?? 0, line.segments[3] ?? 0]
                : [],
            )
            .map((x) => Math.abs(x - (anchor?.xPx ?? Number.NaN))),
        );
      const gap = half("destination") - half("selection");
      const casingPx = CASING_PX * strokes.strokeScale;
      // The destination's casing reaches w ÷ 2 + casing + 0.5 from its line; the brackets' core,
      // where they cover a texel wholly, is w ÷ 2 − 0.5 either side of theirs.
      const reach = strokes.markStrokePx / 2 + casingPx + 0.5;
      const core = strokes.markStrokePx / 2 - 0.5;
      // The segments are `f32`: within 1e-4 px.
      expect([gap - reach >= core - 1e-4, gap >= 0.25 * remPx - 1e-4]).toEqual([true, true]);
    },
  );
});
