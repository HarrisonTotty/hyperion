import { describe, expect, it } from "vitest";

import { normalise, vec3 } from "../../geometry/vec3";
import type { ColourTokens } from "../../spatial/paint";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aMarkedViewScene, FIXTURE_PLANET, OFF_PLANET_CAMERA } from "../../test/viewFixtures";
import type { Viewport } from "../camera/projection";
import type { ViewScene } from "../scene/model";
import {
  buildWireframeDrawList,
  CASING_PX,
  viewStrokesAt,
  type WireframeDrawList,
} from "../wireframe/drawList";
import { linearColour, WireframeRenderer } from "../wireframe/submit";
import { overlayDrawList, overlaySubmission } from "./overlay";

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

describe("the symbology's canvas pass (R07.T16.a)", () => {
  it("is labelled for the pass timer and loads the tone-mapped image beneath it", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const pass = overlaySubmission(renderer, wireframeList(), TOKENS, CAMERA, VIEWPORT);
    expect([pass.label, pass.colourLoad, pass.encoding]).toEqual(["symbology", "load", undefined]);
    renderer.dispose();
  });

  it("strokes each batch's --surface-0 casing, two casings wider, before the batch itself", async () => {
    const renderer = new WireframeRenderer(await countingRenderEngine());
    const list = wireframeList();
    const pass = overlaySubmission(renderer, list, TOKENS, CAMERA, VIEWPORT);
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
    const pass = overlaySubmission(renderer, wireframeList(), TOKENS, CAMERA, VIEWPORT);
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
