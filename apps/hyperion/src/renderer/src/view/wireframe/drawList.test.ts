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
import { occluderRadius } from "../depth/depth";
import { sceneOrigins, type ViewScene } from "../scene/model";
import {
  buildWireframeDrawList,
  type DrawCamera,
  type DrawOptions,
  LOW_SETTING_MAX_SPRITES,
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

const OPTIONS: DrawOptions = {
  lowSetting: false,
  ev100: -1,
  selection: null,
  destination: null,
  remPx: 16,
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

  it("draws a hull over two-sided occluder faces pushed away by the bias", () => {
    const mesh = build().occluderMeshes.find((m) => m.id === "other");
    expect([mesh?.twoSided, mesh?.depthBiasAway]).toEqual([true, { constant: 128, slopeScale: 3 }]);
  });

  it("draws occluders, then lines, then sprites", () => {
    expect(Object.keys(build())).toEqual([
      "occluderSpheres",
      "occluderMeshes",
      "lines",
      "sprites",
      "anchors",
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
