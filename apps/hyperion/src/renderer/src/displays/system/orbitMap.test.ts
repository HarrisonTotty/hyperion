import type { SystemSummaryDto, UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { composePosition, orbitPolyline, positionAt } from "../../lib/orbit";
import { layoutHierarchy } from "../../lib/system/hierarchy";
import type { SystemModel } from "../../lib/system/model";
import { toSystemModel } from "../../lib/system/wire";
import { localFrameAt } from "../../spatial/frame";
import { gridSpacing } from "../../spatial/scale";
import { dot, norm, scale, sub, vec3 } from "../../spatial/vec3";
import {
  aSingleStarSummary,
  aSunlikeStar,
  aSystemSummary,
  aTripleSummary,
  aWhiteDwarf,
  BANDS,
  PIN_SYSTEM,
} from "../../test/systemFixtures";
import {
  fitRadiiAu,
  hostMarks,
  layerOfMass,
  LONE_STAR_FIT_AU,
  orbitPaths,
  orbitPlane,
  orbitScene,
} from "./orbitMap";
import { METRES_PER_AU } from "./orbitScale";

const STAR_0 = `${PIN_SYSTEM}.0000`;
const STAR_1 = `${PIN_SYSTEM}.0001`;
const TIME: UniverseTime = { seconds: 3_155_760_000, nanos: 0 };
const POSITION_LY = vec3(26_000, 0, 12);

function built(summary: SystemSummaryDto = aSystemSummary()) {
  const result = toSystemModel(summary, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  const model: SystemModel = result.model;
  const layout = layoutHierarchy(model.hierarchy, model.hosts);
  return { model, layout, plane: orbitPlane(layout, POSITION_LY) };
}

describe("hostMarks", () => {
  it("draws each host where its orbits put it at the display time, in AU", () => {
    const { model, layout } = built();
    const marks = hostMarks(model.hosts, layout, TIME, BANDS);

    expect(marks.map((mark) => mark.id)).toEqual([STAR_0, STAR_1]);
    for (const mark of marks) {
      const expected = scale(composePosition(layout.placements, mark.id, TIME), 1 / METRES_PER_AU);
      expect(norm(sub(mark.position, expected))).toBeLessThan(1e-12 * norm(expected));
    }
  });

  it("gives each host its registry symbol at its mass layer's size", () => {
    const { model, layout } = built();

    // The white dwarf was 2.5 M☉ at birth, the foot of layer D; its companion 1 M☉, in layer C.
    expect(
      hostMarks(model.hosts, layout, TIME, BANDS).map((mark) => [mark.shape, mark.sizeClass]),
    ).toEqual([
      ["diamond", 3],
      ["circle", 2],
    ]);
  });

  it("draws a giant of the lightest layer at size class 2, where its ring reads", () => {
    const { model, layout } = built(
      aSingleStarSummary(aSunlikeStar({ kind: "giant", initial_mass_msun: 0.3 })),
    );

    expect(hostMarks(model.hosts, layout, TIME, BANDS)[0]?.sizeClass).toBe(2);
  });

  it("does not draw a star that left no remnant, and draws no orbit for it", () => {
    const summary = aSystemSummary({
      stars: [
        aWhiteDwarf(),
        aSunlikeStar({ body_index: 1, kind: "no_remnant", remnant: { type: "no_remnant" } }),
      ],
    });
    const { model, layout } = built(summary);

    expect(hostMarks(model.hosts, layout, TIME, BANDS).map((mark) => mark.id)).toEqual([STAR_0]);
    expect(orbitPaths(model.hosts, layout, TIME, null).map((path) => path.id)).toEqual([STAR_0]);
  });

  it("moves a companion by the mean anomaly a step of the display time adds", () => {
    const { model, layout } = built();
    const pair = model.hierarchy[0];
    if (pair?.kind !== "pair") {
      throw new Error("the pinned binary's root is a pair");
    }
    const stepS = 10 * 86_400;
    const at = (time: UniverseTime) => {
      const [a, b] = hostMarks(model.hosts, layout, time, BANDS);
      return sub(b?.position ?? vec3(0, 0, 0), a?.position ?? vec3(0, 0, 0));
    };
    const advanced = {
      ...pair.orbit,
      meanAnomalyAtEpochRad:
        pair.orbit.meanAnomalyAtEpochRad + (2 * Math.PI * stepS) / pair.orbit.periodS,
    };
    const expected = scale(positionAt(advanced, TIME), 1 / METRES_PER_AU);
    const stepped = at({ seconds: TIME.seconds + stepS, nanos: 0 });

    expect(norm(sub(stepped, expected))).toBeLessThan(1e-9 * norm(expected));
    expect(norm(sub(stepped, at(TIME)))).toBeGreaterThan(0);
  });
});

describe("orbitPaths", () => {
  it("draws each star's path about the barycentre as its share of the relative orbit", () => {
    const { model, layout } = built();
    const pair = model.hierarchy[0];
    if (pair?.kind !== "pair") {
      throw new Error("the pinned binary's root is a pair");
    }
    const paths = orbitPaths(model.hosts, layout, TIME, null);
    const companion = paths.find((path) => path.id === STAR_1);
    const expected = orbitPolyline(pair.orbit, 180).map((pointM) =>
      scale(pointM, 2.5 / 3.5 / METRES_PER_AU),
    );

    expect(paths.map((path) => path.role)).toEqual(["reference", "reference"]);
    expect(companion?.points).toHaveLength(expected.length);
    companion?.points.forEach((point, index) => {
      const want = expected[index] ?? vec3(0, 0, 0);
      expect(norm(sub(point, want))).toBeLessThan(1e-12 * norm(want));
    });
  });

  it("puts each star on its own path", () => {
    const { model, layout } = built();
    const marks = hostMarks(model.hosts, layout, TIME, BANDS);
    const paths = orbitPaths(model.hosts, layout, TIME, null);

    for (const mark of marks) {
      const path = paths.find((candidate) => candidate.id === mark.id);
      const nearest = Math.min(
        ...(path?.points ?? []).map((point) => norm(sub(point, mark.position))),
      );
      // Within the chord of 2° of eccentric anomaly on a path of some 17 AU.
      expect(nearest).toBeLessThan(0.5);
    }
  });

  it("draws the selected star's path as the one selected path", () => {
    const { model, layout } = built();

    expect(orbitPaths(model.hosts, layout, TIME, STAR_1).map((path) => path.role)).toEqual([
      "reference",
      "selected",
    ]);
  });

  it("draws a triple's inner pair's barycentre too, never as the selection", () => {
    const { model, layout } = built(aTripleSummary());
    const paths = orbitPaths(model.hosts, layout, TIME, "pair:1");

    expect(paths.map((path) => path.id)).toEqual(["pair:1", `${PIN_SYSTEM}.0002`, STAR_0, STAR_1]);
    expect(paths.every((path) => path.role === "reference")).toBe(true);
  });
});

describe("orbitPlane", () => {
  it("draws a binary on its own orbit's plane, coreward laid onto it", () => {
    const { layout, plane } = built();
    const galactic = localFrameAt(POSITION_LY);

    expect(plane.name).toBe("SYSTEM PLANE");
    expect(plane.frame.north).toEqual(layout.primaryPairNormal);
    expect(dot(plane.frame.coreward, galactic.coreward)).toBeGreaterThan(0);
    expect(dot(plane.frame.coreward, plane.frame.north)).toBeCloseTo(0, 12);
  });

  it("draws a single star on the galactic plane, and says so", () => {
    const { plane } = built(aSingleStarSummary());

    expect(plane.name).toBe("GALACTIC PLANE");
    expect(plane.isSystemPlane).toBe(false);
    expect(plane.frame).toEqual(localFrameAt(POSITION_LY));
  });
});

describe("fitRadiiAu", () => {
  it("fits INNER to the primary's reach and ALL to the companion's", () => {
    const { model, layout } = built();
    const apoapsisAu = (3_515_625_000_000 * 1.5) / METRES_PER_AU;

    const radii = fitRadiiAu(layout, model.hosts);

    expect(radii.inner).toBeCloseTo(apoapsisAu / 3.5, 9);
    expect(radii.all).toBeCloseTo((apoapsisAu * 2.5) / 3.5, 9);
  });

  it("fits a single star at the barycentre to a grid of 1 AU", () => {
    const { model, layout } = built(aSingleStarSummary());

    expect(fitRadiiAu(layout, model.hosts)).toEqual({
      inner: LONE_STAR_FIT_AU,
      all: LONE_STAR_FIT_AU,
      belts: LONE_STAR_FIT_AU,
    });
  });
});

describe("layerOfMass", () => {
  it.each([
    [0.05, 0],
    [0.3, 0],
    [0.5, 1],
    [1, 2],
    [2.5, 3],
    [40, 4],
    [300, 4],
  ])("puts %f M☉ in layer %i", (massMsun, layer) => {
    expect(layerOfMass(massMsun, BANDS)).toBe(layer);
  });
});

describe("orbitScene", () => {
  it("covers the fitted radius with the grid, and keeps a selection only of a drawn body", () => {
    const { model, layout, plane } = built();
    const input = { hosts: model.hosts, layout, plane, time: TIME, bands: BANDS, fitRadiusAu: 17 };

    const scene = orbitScene({ bodies: null, ...input, selectedId: STAR_1 });

    expect(scene.plane.spacing).toBe(gridSpacing(17));
    expect(scene.plane.extent).toBe(17);
    expect(scene.spheres).toEqual([]);
    expect(scene.selectedId).toBe(STAR_1);
    expect(scene.paths?.filter((path) => path.role === "selected")).toHaveLength(1);
    expect(orbitScene({ bodies: null, ...input, selectedId: "elsewhere" }).selectedId).toBeNull();
  });

  it("places no mark off the centre for a single star", () => {
    const { model, layout, plane } = built(aSingleStarSummary());
    const scene = orbitScene({
      bodies: null,
      hosts: model.hosts,
      layout,
      plane,
      time: TIME,
      bands: BANDS,
      selectedId: null,
      fitRadiusAu: 1,
    });

    expect(scene.points.map((mark) => mark.position)).toEqual([vec3(0, 0, 0)]);
    expect(scene.paths).toEqual([]);
  });
});
