import { galacticPositionFromLy, type MapFeatureKindWire } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  aGalaxyFeatures,
  BRIGHT_SHELL,
  GALACTIC_CENTRE,
  GLOBULAR,
} from "../../test/featureFixtures";
import { toMapFeatureLayers } from "./featureWire";
import { mapGeometry } from "./mapGeometry";
import { MAP_FEATURE_MARK_REM, mapPointPx, placeMapFeatures } from "./mapFeatures";

/** The M1 maps' extents: 131,072 ly across face-on, half as tall edge-on (plan 04, design note 12). */
const FACE_ON = mapGeometry({
  view: "face_on",
  width_px: 256,
  height_px: 256,
  centre_ly: [0, 0],
  ly_per_px: 512,
});
const EDGE_ON = mapGeometry({
  view: "edge_on",
  width_px: 256,
  height_px: 128,
  centre_ly: [0, 0],
  ly_per_px: 512,
});

const SQUARE = { widthPx: 400, heightPx: 400, remPx: 16 };
const WIDE = { widthPx: 400, heightPx: 200, remPx: 16 };
const EVERY_KIND: ReadonlySet<MapFeatureKindWire> = new Set([
  "globular",
  "galactic_centre",
  "bright_shell",
]);

describe("mapPointPx", () => {
  it("turns the face-on picture a quarter-turn clockwise: +x down, +y to the right", () => {
    expect(mapPointPx("face_on", FACE_ON, SQUARE, { x: 0, y: 0, z: 0 })).toEqual({
      xPx: 200,
      yPx: 200,
    });
    expect(mapPointPx("face_on", FACE_ON, SQUARE, { x: 32_768, y: 0, z: 0 })).toEqual({
      xPx: 200,
      yPx: 300,
    });
    expect(mapPointPx("face_on", FACE_ON, SQUARE, { x: 0, y: 32_768, z: 0 })).toEqual({
      xPx: 300,
      yPx: 200,
    });
  });

  it("sets the edge-on picture x across and z up", () => {
    expect(mapPointPx("edge_on", EDGE_ON, WIDE, { x: 32_768, y: 9_000, z: 16_384 })).toEqual({
      xPx: 300,
      yPx: 50,
    });
  });

  it("places nothing beyond the view's extent", () => {
    expect(mapPointPx("edge_on", EDGE_ON, WIDE, { x: 0, y: 0, z: 40_000 })).toBeNull();
    expect(mapPointPx("face_on", FACE_ON, SQUARE, { x: 70_000, y: 0, z: 0 })).toBeNull();
  });
});

describe("placeMapFeatures (plan 09, P09.T41.b)", () => {
  const layers = toMapFeatureLayers(aGalaxyFeatures({ shells: [BRIGHT_SHELL] }));

  it("draws the shells first, then the globulars, then the centre on top", () => {
    const places = placeMapFeatures("face_on", FACE_ON, layers, EVERY_KIND, SQUARE);

    expect(places.map((place) => [place.key, place.kind, place.cross])).toEqual([
      [`shell:${BRIGHT_SHELL.system}`, "bright_shell", false],
      [GLOBULAR.id, "globular", true],
      [GALACTIC_CENTRE.id, "galactic_centre", true],
    ]);
  });

  it("draws a shell at its true radius, and no circle for a feature no wider than its mark", () => {
    const places = placeMapFeatures("face_on", FACE_ON, layers, EVERY_KIND, SQUARE);
    const pxPerLy = 400 / 131_072;

    expect(places[0]?.circlePx).toBeCloseTo(BRIGHT_SHELL.shell.radius_ly * pxPerLy, 12);
    expect(places[1]?.circlePx).toBeNull();
  });

  it("draws a feature's circle where its true radius is wider than its mark", () => {
    // 131,072 ly over 40 px: 3,277 ly to a pixel, so a 128 ly centre is no wider than its mark,
    // and over 40,000 px it is 39 px across.
    const close = { widthPx: 40_000, heightPx: 40_000, remPx: 16 };
    const [centre] = placeMapFeatures(
      "face_on",
      FACE_ON,
      layers,
      new Set(["galactic_centre"]),
      close,
    );

    expect(centre?.circlePx).toBeCloseTo((128 * 40_000) / 131_072, 9);
    expect(centre?.circlePx ?? 0).toBeGreaterThan((MAP_FEATURE_MARK_REM * 16) / 2);
  });

  it("draws only the kinds shown, and none of a kind not generated", () => {
    const golden = toMapFeatureLayers(aGalaxyFeatures());

    expect(
      placeMapFeatures("face_on", FACE_ON, golden, EVERY_KIND, SQUARE).map((place) => place.kind),
    ).toEqual(["globular", "galactic_centre"]);
    expect(
      placeMapFeatures("face_on", FACE_ON, golden, new Set(["globular"]), SQUARE).map(
        (place) => place.kind,
      ),
    ).toEqual(["globular"]);
  });

  it("labels each feature with its kind's abbreviation to the right of its mark", () => {
    const places = placeMapFeatures("face_on", FACE_ON, layers, EVERY_KIND, SQUARE);
    const centre = places.find((place) => place.kind === "galactic_centre");

    expect(places.map((place) => place.label?.text)).toEqual(["SNR", "GC", "CENTRE"]);
    // Half the 0.75 rem mark and 0.25 rem beyond it.
    expect(centre?.label?.leftPx).toBeCloseTo(200 + 6 + 4, 9);
  });

  it("labels the centre before a globular whose label would cross it, and drops the globular's", () => {
    const beside = {
      ...GLOBULAR,
      position: galacticPositionFromLy([0, 0, 0]),
    };
    const crowded = toMapFeatureLayers(aGalaxyFeatures({ globulars: [beside] }));

    const places = placeMapFeatures("face_on", FACE_ON, crowded, EVERY_KIND, SQUARE);

    expect(places.find((place) => place.kind === "galactic_centre")?.label?.text).toBe("CENTRE");
    // To the right is the centre's, and to the left is free.
    const globular = places.find((place) => place.kind === "globular");
    expect(globular?.label?.leftPx ?? 0).toBeLessThan(200);
  });

  it("puts a label to the left of a mark at the picture's right edge", () => {
    const atEdge = {
      ...GLOBULAR,
      position: galacticPositionFromLy([0, 65_000, 0]),
    };
    const edge = toMapFeatureLayers(aGalaxyFeatures({ globulars: [atEdge], centre: [] }));

    const [globular] = placeMapFeatures("face_on", FACE_ON, edge, EVERY_KIND, SQUARE);

    expect(globular?.label?.leftPx ?? 400).toBeLessThan(globular?.xPx ?? 0);
  });
});
