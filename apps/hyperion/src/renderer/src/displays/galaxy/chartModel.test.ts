import type { ObjectKindDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { AU_PER_LY } from "../../lib/format";
import { toChartResult } from "../../lib/galaxy/wire";
import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import {
  aCensus,
  anUnknownLayer,
  anUnknownLayerLine,
  aStellarBrief,
  aSystemsInRange,
  type BriefLayer,
  LAYERS,
  withCensusLine,
  withRowFields,
} from "../../test/galaxyFixtures";
import {
  censusHint,
  censusLine,
  chartDataFault,
  distanceDecimalsFor,
  distanceUnitFor,
  formatBandMsun,
  formatChartDistance,
  formatChartLengthLy,
  formatChartOffset,
  formatFeatureDistance,
  formatQueryRadius,
  filterSystems,
  inRangeCount,
  layerBands,
  nextStarFilter,
  passesStarFilter,
  queryRadiusForDriveRange,
  shownCount,
  STAR_FILTERS,
  starFilterLabel,
  SUBSTELLAR_FLOORS,
  toScene,
} from "./chartModel";

const CENTRE = [26_000, 0, 0] as const;
const FRAME = localFrameAt(vec3(CENTRE[0], CENTRE[1], CENTRE[2]));

function chartOf(
  systems: ReadonlyArray<{ readonly relLy: readonly [number, number, number] }>,
  radiusLy = 50,
) {
  return toChartResult(
    aSystemsInRange({
      centreLy: CENTRE,
      radiusLy,
      systems: systems.map(({ relLy }) => ({ relLy, layer: "a" as const })),
    }),
  );
}

function sceneOf(
  systems: ReadonlyArray<{ readonly relLy: readonly [number, number, number] }>,
  { radiusLy = 50, driveRangeLy = 50 } = {},
) {
  return toScene(chartOf(systems, radiusLy), {
    frame: FRAME,
    driveRangeLy,
    selectedId: null,
    destinationId: null,
  });
}

describe("toScene", () => {
  it("marks a system just inside the drive range as available", () => {
    const scene = sceneOf([{ relLy: [49.9, 0, 0] }]);

    expect(scene.points[0]?.status).toBe("available");
  });

  it("leaves a system behind the centre out of range although it projects inside the circle", () => {
    // Along the line of sight at TOP: it projects at the centre of the range circle.
    const scene = sceneOf([{ relLy: [0, 0, -50.1] }], { radiusLy: 80 });

    expect(scene.points[0]?.status).toBe("plain");
  });

  it("leaves a system high above the plane out of range although its foot is inside the ring", () => {
    const scene = sceneOf([{ relLy: [30, 0, 45] }], { radiusLy: 80 });

    expect(scene.points[0]?.status).toBe("plain");
  });

  it("draws no range sphere when the drive range is beyond the query radius", () => {
    const scene = sceneOf([{ relLy: [10, 0, 0] }], { radiusLy: 50, driveRangeLy: 80 });

    expect(scene.spheres.map((sphere) => sphere.role)).toEqual(["data_edge"]);
    expect(scene.points.every((point) => point.status === "available")).toBe(true);
  });

  it("labels the range sphere as an entered setting", () => {
    const scene = sceneOf([], { radiusLy: 80, driveRangeLy: 50 });

    expect(scene.spheres.map((sphere) => sphere.label)).toEqual([
      "RANGE 50 ly SET",
      "QUERY EDGE 80 ly",
    ]);
  });

  it("labels the query edge of a chart of the centre's finest step in AU, as its selector reads", () => {
    const scene = sceneOf([], { radiusLy: 0.001 });

    expect(scene.spheres.map((sphere) => sphere.label)).toEqual(["QUERY EDGE 63.2 AU"]);
    expect(scene.plane.spacing).toBe(0.0002);
    expect(scene.plane.rings.map((ring) => ring.label)).toEqual(["", "", "", "", ""]);
  });

  it("rings the plane at every grid spacing out to 50 ly, the drive range labelled", () => {
    const scene = sceneOf([], { radiusLy: 50, driveRangeLy: 50 });

    expect(scene.plane.spacing).toBe(20);
    expect(scene.plane.rings).toEqual([
      { radius: 20, label: "" },
      { radius: 40, label: "" },
      // The ring draws the value the range sphere does, so it says `SET` as well (ruling 10).
      { radius: 50, label: "PLANE 50 ly SET" },
    ]);
  });

  it("rings the plane in hundredths of a light-year on a 0.05 ly chart", () => {
    const scene = sceneOf([], { radiusLy: 0.05, driveRangeLy: 0.04 });

    expect(scene.plane.spacing).toBe(0.02);
    expect(scene.plane.rings).toEqual([
      { radius: 0.02, label: "" },
      { radius: 0.04, label: "PLANE 0.04 ly SET" },
    ]);
  });

  it("sizes a mark by its mass layer and labels it with its designation", () => {
    const result = toChartResult(
      aSystemsInRange({ centreLy: CENTRE, systems: [{ relLy: [1, 0, 0], layer: "d" }] }),
    );

    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });

    expect(scene.points[0]?.sizeClass).toBe(3);
    expect(scene.points[0]?.label).toBe("H7K 4C0RFZ D-1");
    expect(scene.points[0]?.shape).toBe("circle");
  });

  it("labels marks by initial mass, so the heaviest is labelled first", () => {
    const result = toChartResult(
      aSystemsInRange({
        centreLy: CENTRE,
        systems: [
          { relLy: [1, 0, 0], layer: "a" },
          { relLy: [2, 0, 0], layer: "e" },
        ],
      }),
    );

    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });
    const [nearest, furthest] = scene.points;

    expect(nearest?.labelPriority).toBeLessThan(furthest?.labelPriority ?? 0);
  });

  it("passes the selection and the destination through", () => {
    const result = chartOf([{ relLy: [1, 0, 0] }]);

    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: "0000000000000001",
      destinationId: null,
    });

    expect(scene.selectedId).toBe("0000000000000001");
    expect(scene.destinationId).toBeNull();
    expect(scene.frame).toBe(FRAME);
  });
});

describe("inRangeCount", () => {
  it("counts the systems within the drive range", () => {
    const result = chartOf([{ relLy: [10, 0, 0] }, { relLy: [60, 0, 0] }], 80);

    expect(inRangeCount(result.systems, 50)).toBe(1);
  });
});

describe("censusLine", () => {
  it("names the mass a complete census is complete above", () => {
    expect(censusLine({ kind: "complete", aboveMsun: 0.5, layer: "b" })).toEqual({
      kind: "complete",
      text: "COMPLETE ABOVE",
      aboveMsun: 0.5,
      above: { value: "0.50", unit: "msun" },
    });
  });

  it("names a substellar floor in its own unit", () => {
    const [rogue, brown] = SUBSTELLAR_FLOORS;
    expect(
      censusLine({ kind: "complete", aboveMsun: brown?.minMsun ?? 0, layer: "brown_dwarf" }),
    ).toMatchObject({ above: { value: "0.012", unit: "msun" } });
    expect(
      censusLine({ kind: "complete", aboveMsun: rogue?.minMsun ?? 0, layer: "rogue_planet" }),
    ).toMatchObject({ above: { value: "0.33", unit: "mearth" } });
  });

  it("says what to do when nothing fits", () => {
    expect(censusLine({ kind: "nothing_fits" })).toEqual({
      kind: "nothing_fits",
      text: "NOTHING FITS: reduce radius",
    });
  });
});

describe("censusHint", () => {
  it("says nothing when every layer is included", () => {
    expect(censusHint(aCensus().layers)).toBeNull();
  });

  it("says nothing when a layer is only below the mass floor", () => {
    expect(censusHint(aCensus({ minLayer: "c" }).layers)).toBeNull();
  });

  it("asks for a smaller radius before a higher floor when a layer is over the limit", () => {
    expect(censusHint(aCensus({ overLimit: ["a"] }).layers)).toBe(
      "DENSE REGION: reduce radius before raising MIN MASS",
    );
  });

  it("names the server's cell budget when a layer was dropped for it", () => {
    expect(censusHint(aCensus({ overCellBudget: ["a"] }).layers)).toBe(
      "LARGE VOLUME: layers dropped by the server cell budget; reduce radius",
    );
  });
});

describe("layerBands", () => {
  it("gives the five bands from the census, lightest first", () => {
    const bands = layerBands(aCensus().layers);

    expect(bands?.map((band) => band.layer)).toEqual([...LAYERS]);
    expect(bands?.map((band) => band.index)).toEqual([0, 1, 2, 3, 4]);
    expect(bands?.[0]).toEqual({ layer: "a", index: 0, minMsun: 0.08, maxMsun: 0.5 });
    expect(bands?.[4]?.maxMsun).toBe(150);
  });

  it("gives nothing when the census does not list every layer", () => {
    expect(layerBands(aCensus().layers.slice(1))).toBeNull();
  });

  it("gives nothing when the census lists a layer twice", () => {
    const [first] = aCensus().layers;
    if (first === undefined) {
      throw new Error("the fixture built no census");
    }
    expect(layerBands([first, first, first, first, first])).toBeNull();
  });
});

describe("chartDataFault", () => {
  it("finds nothing wrong with an answer the server built properly", () => {
    expect(chartDataFault(chartOf([{ relLy: [1, 0, 0] }]))).toBeNull();
  });

  it("calls a census that does not list every layer incomplete", () => {
    const answer = aSystemsInRange({ centreLy: CENTRE });
    const result = toChartResult({
      ...answer,
      census: { ...answer.census, layers: answer.census.layers.slice(1) },
    });

    expect(chartDataFault(result)).toBe("census incomplete");
  });

  it("calls a radius that is not a length unusable", () => {
    const result = toChartResult(aSystemsInRange({ centreLy: CENTRE, radiusLy: 0 }));

    expect(chartDataFault(result)).toBe("query radius unusable");
  });
});

describe("formatChartLengthLy", () => {
  it.each([
    [50, "50"],
    [500, "500"],
    [0.05, "0.05"],
    [0.01, "0.01"],
    [37.5, "37.5"],
    [1_000, "1000"],
    [10_000, "10,000"],
  ])("writes %s ly as %s", (ly, text) => {
    expect(formatChartLengthLy(ly)).toBe(text);
  });
});

describe("formatBandMsun", () => {
  it.each([
    [0.08, "0.08"],
    [0.5, "0.5"],
    [2.5, "2.5"],
    [8, "8"],
    [150, "150"],
  ])("writes a band edge of %s M☉ as %s", (massMsun, text) => {
    expect(formatBandMsun(massMsun)).toBe(text);
  });
});

describe("distanceDecimalsFor", () => {
  it.each([
    [500, 2],
    [50, 2],
    [10, 2],
    [9.99, 3],
    [1, 3],
    [0.5, 4],
    [0.01, 4],
  ])("writes every distance on a %s ly chart with %s decimals", (queryRadiusLy, decimals) => {
    expect(distanceDecimalsFor(queryRadiusLy)).toBe(decimals);
  });
});

describe("distanceUnitFor", () => {
  it.each([
    [500, "ly", 2],
    [10, "ly", 2],
    [1, "ly", 3],
    [0.5, "ly", 4],
    [0.01, "ly", 4],
    // A computed 0.01 a bit off the literal stays in light-years, as the scale bar does.
    [0.1 * 0.1, "ly", 4],
    [0.01 * (1 - 1e-12), "ly", 4],
    [0.005, "AU", 1],
    [0.002, "AU", 1],
    [0.001, "AU", 1],
  ])(
    "writes every distance on a %s ly chart in %s with %s decimals",
    (radiusLy, symbol, decimals) => {
      const unit = distanceUnitFor(radiusLy);

      expect(unit.symbol).toBe(symbol);
      expect(unit.decimals).toBe(decimals);
      expect(unit.perLy).toBe(symbol === "AU" ? AU_PER_LY : 1);
    },
  );
});

describe("formatChartDistance and formatChartOffset", () => {
  it.each([
    [0.0042, 0.5, "0.0042", "+0.0042"],
    [-0.0042, 0.01, "0.0042", "-0.0042"],
    [12.3456, 20, "12.35", "+12.35"],
    // 0.001 ly is 63.241 AU (the IAU's 2012 au and the Julian light-year, both exact).
    [0.001, 0.001, "63.2", "+63.2"],
    [-0.0005, 0.001, "31.6", "-31.6"],
    [0, 0.005, "0.0", "+0.0"],
    // A feature's centre may lie far beyond a fine chart's edge: grouped from five digits.
    [2, 0.001, "126,482.2", "+126,482.2"],
  ])(
    "writes %s ly on a %s ly chart as %s, and as an offset %s",
    (ly, radiusLy, distance, offset) => {
      const unit = distanceUnitFor(radiusLy);

      expect(formatChartDistance(Math.abs(ly), unit)).toBe(distance);
      expect(formatChartOffset(ly, unit)).toBe(offset);
    },
  );
});

describe("formatFeatureDistance", () => {
  it.each([
    // Within the column's seven characters, at the chart's precision.
    [99.9999, 0.5, "99.9999"],
    [0.0042, 0.001, "265.6"],
    [0.15, 0.001, "9486.2"],
    [499.99, 50, "499.99"],
    // Past it, to three figures in the chart's unit.
    [300, 0.5, "300"],
    [12_345.678, 0.01, "12,300"],
    [1.5, 0.001, "94,900"],
    [2, 0.001, "126,000"],
    [10_000, 0.001, "6.32E8"],
    [12_345.678, 50, "12,300"],
  ])("writes a feature %s ly off on a %s ly chart as %s", (ly, radiusLy, text) => {
    const written = formatFeatureDistance(ly, distanceUnitFor(radiusLy));

    expect(written).toBe(text);
    expect(written.length).toBeLessThanOrEqual(7);
  });
});

describe("formatQueryRadius", () => {
  it.each([
    [500, "500 ly"],
    [80, "80 ly"],
    [37.5, "37.5 ly"],
    [0.05, "0.05 ly"],
    [0.01, "0.01 ly"],
    [0.005, "316 AU"],
    [0.002, "126 AU"],
    [0.001, "63.2 AU"],
  ])("writes a radius of %s ly as %s", (ly, text) => {
    expect(formatQueryRadius(ly)).toBe(text);
  });
});

describe("queryRadiusForDriveRange", () => {
  it.each([
    [50, 50],
    [80, 100],
    [0.01, 0.01],
    [0.0005, 0.001],
    [600, 500],
  ])("follows a drive range of %s ly with %s ly", (driveRangeLy, radiusLy) => {
    expect(queryRadiusForDriveRange(driveRangeLy)).toBe(radiusLy);
  });
});

/** A chart of one system per kind given, 1 ly apart along x, each in the layer given. */
function chartOfKinds(
  kinds: ReadonlyArray<ObjectKindDto | null>,
  layer: BriefLayer = "c",
): ReturnType<typeof toChartResult> {
  return toChartResult(
    aSystemsInRange({
      centreLy: CENTRE,
      systems: kinds.map((kind, index) => ({
        relLy: [index + 1, 0, 0] as const,
        layer,
        stellar: kind === null ? null : aStellarBrief(layer, kind),
      })),
    }),
  );
}

describe("toScene's star symbols", () => {
  it("draws each system with its primary's symbol", () => {
    const result = chartOfKinds(["dwarf", "giant", "white_dwarf", "neutron_star", "black_hole"]);

    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });

    expect(scene.points.map((point) => point.shape)).toEqual([
      "circle",
      "ringed-circle",
      "diamond",
      "triangle",
      "square",
    ]);
  });

  it("raises a light giant to the ringed circle's smallest size class", () => {
    const result = chartOfKinds(["giant", "dwarf"], "a");

    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });

    expect(scene.points.map((point) => point.sizeClass)).toEqual([2, 0]);
  });

  it("lists but does not draw a star that left no remnant or a system not yet formed", () => {
    const result = chartOfKinds(["no_remnant", null, "dwarf"]);

    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });

    expect(result.systems).toHaveLength(3);
    expect(scene.points.map((point) => point.id)).toEqual([result.systems[2]?.id]);
  });

  it("draws only the systems that pass the STARS filter", () => {
    const result = chartOfKinds(["dwarf", "white_dwarf", "giant"]);

    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
      starFilter: "remnants",
    });

    expect(scene.points.map((point) => point.shape)).toEqual(["diamond"]);
  });
});

describe("the STARS filter", () => {
  const EVERY_KIND: ReadonlyArray<ObjectKindDto> = [
    "protostar",
    "pre_main_sequence",
    "dwarf",
    "subgiant",
    "giant",
    "supergiant",
    "wolf_rayet",
    "hot_subdwarf",
    "stripped_helium_star",
    "white_dwarf",
    "neutron_star",
    "black_hole",
    "no_remnant",
    "substellar",
  ];

  it("counts the living stars and brown dwarfs as LIVING", () => {
    const result = chartOfKinds(EVERY_KIND);

    const living = result.systems.filter((system) => passesStarFilter(system, "living"));

    expect(living.map((system) => system.star?.kind)).toEqual([
      "protostar",
      "pre_main_sequence",
      "dwarf",
      "subgiant",
      "giant",
      "supergiant",
      "wolf_rayet",
      "hot_subdwarf",
      "stripped_helium_star",
      "substellar",
    ]);
  });

  it("counts white dwarfs, neutron stars and black holes as REMNANTS", () => {
    const result = chartOfKinds(EVERY_KIND);

    const remnants = filterSystems(result.systems, "remnants");

    expect(remnants.map((system) => system.star?.kind)).toEqual([
      "white_dwarf",
      "neutron_star",
      "black_hole",
    ]);
  });

  it("shows a star that left no remnant under ALL alone", () => {
    const result = chartOfKinds(["no_remnant"]);

    expect(
      STAR_FILTERS.filter((filter) => filterSystems(result.systems, filter).length > 0),
    ).toEqual(["all"]);
  });

  it("shows a system not yet formed under ALL alone", () => {
    const result = chartOfKinds([null]);

    expect(
      STAR_FILTERS.filter((filter) => filterSystems(result.systems, filter).length > 0),
    ).toEqual(["all"]);
  });

  it("keeps every system, in order, under ALL", () => {
    const result = chartOfKinds(["white_dwarf", "dwarf", null]);

    expect(filterSystems(result.systems, "all")).toBe(result.systems);
  });

  it("steps from ALL to LIVING to REMNANTS and back to ALL", () => {
    expect(nextStarFilter("all")).toBe("living");
    expect(nextStarFilter("living")).toBe("remnants");
    expect(nextStarFilter("remnants")).toBe("all");
    expect(STAR_FILTERS.map(starFilterLabel)).toEqual(["ALL", "LIVING", "REMNANTS"]);
  });
});

describe("shownCount", () => {
  it("gives the figure apart from the words saying what each filter hides, digits grouped from five", () => {
    expect(shownCount(412, 1630, "living")).toEqual({
      figure: "412 OF 1630",
      filterWords: "SHOWN: LIVING",
    });
    expect(shownCount(9, 12_480, "remnants")).toEqual({
      figure: "9 OF 12,480",
      filterWords: "SHOWN: REMNANTS",
    });
  });

  it("gives the total alone under ALL, with no words after it", () => {
    expect(shownCount(1630, 1630, "all")).toEqual({ figure: "1630", filterWords: null });
  });
});

describe("the substellar layers (plan 13, P13.T8.c)", () => {
  const substellar = (): ReturnType<typeof toChartResult> =>
    toChartResult(
      aSystemsInRange({
        centreLy: CENTRE,
        radiusLy: 10,
        minLayer: "rogue_planet",
        systems: [
          { relLy: [1, 0, 0], layer: "a" },
          { relLy: [2, 0, 0], layer: "brown_dwarf" },
          { relLy: [3, 0, 0], layer: "rogue_planet" },
        ],
      }),
    );

  it("draws a brown dwarf as a circle and a free-floating planet as an inverted triangle, both at layer A's size", () => {
    const scene = toScene(substellar(), {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });
    expect(scene.points.map((point) => [point.shape, point.sizeClass])).toEqual([
      ["circle", 0],
      ["circle", 0],
      ["triangle-down", 0],
    ]);
  });

  it("does not draw a free-floating planet not yet formed", () => {
    const result = substellar();
    const [star, brown, planet] = result.systems;
    if (star === undefined || brown === undefined || planet === undefined) {
      throw new Error("the fixture built three objects");
    }
    const unborn = { ...result, systems: [star, brown, { ...planet, ageMyr: -0.001 }] };
    const scene = toScene(unborn, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });
    expect(scene.points.map((point) => point.shape)).toEqual(["circle", "circle"]);
  });

  it("shows a brown dwarf under LIVING and a free-floating planet only under ALL", () => {
    const [star, brown, planet] = substellar().systems;
    expect([star, brown, planet].map((system) => system?.kind)).toEqual([
      "stellar",
      "brown_dwarf",
      "rogue_planet",
    ]);
    for (const filter of STAR_FILTERS) {
      expect(planet !== undefined && passesStarFilter(planet, filter)).toBe(filter === "all");
      expect(brown !== undefined && passesStarFilter(brown, filter)).toBe(filter !== "remnants");
    }
  });

  it("keeps the five stellar bands as the size scale whatever substellar lines the census has", () => {
    const bands = layerBands(substellar().layers);
    expect(bands?.map((band) => band.layer)).toEqual(LAYERS);
    expect(chartDataFault(substellar())).toBeNull();
  });

  it("refuses a census listing the planets without the brown dwarfs, or a layer twice", () => {
    const layers = aCensus({ minLayer: "rogue_planet" }).layers;
    const noBrown = layers.filter((line) => line.layer !== "brown_dwarf");
    const twice = [...layers, ...layers.filter((line) => line.layer === "rogue_planet")];
    expect(layerBands(noBrown)).toBeNull();
    expect(layerBands(twice)).toBeNull();
  });

  it("offers the two substellar floors lightest first at a third of an Earth mass and 13 Jupiter masses", () => {
    expect(SUBSTELLAR_FLOORS.map((floor) => floor.layer)).toEqual(["rogue_planet", "brown_dwarf"]);
    const [rogue, brown] = SUBSTELLAR_FLOORS;
    // 1/3 M⊕ is 1.001E-6 M☉ and 13 M_Jup 0.01241 M☉.
    expect(rogue?.minMsun).toBeCloseTo(1.001_2e-6, 9);
    expect(brown?.minMsun).toBeCloseTo(0.012_41, 5);
  });
});

describe("a layer this build does not know (plan 04's reserved list)", () => {
  /** Two layer A systems, the nearer moved to a layer this build does not know, as a newer server's. */
  function newerChart() {
    const response = withCensusLine(
      aSystemsInRange({
        centreLy: CENTRE,
        systems: [
          { relLy: [1, 0, 0], layer: "a" },
          { relLy: [2, 0, 0], layer: "a" },
        ],
      }),
      anUnknownLayerLine(),
    );
    return toChartResult(withRowFields(response, { layer: anUnknownLayer() }, 1));
  }

  it("charts an answer whose census adds a line of it to the five stellar layers", () => {
    const result = newerChart();

    expect(chartDataFault(result)).toBeNull();
    expect(layerBands(result.layers)?.map((band) => band.layer)).toEqual(LAYERS);
  });

  it("draws none of its objects, whose size class is not known", () => {
    const result = newerChart();
    const scene = toScene(result, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });

    expect(scene.points.map((point) => point.label)).toEqual(["H7K 4C0RFZ A-2"]);
  });

  it("reads the census's edge as Missing where it is the lightest layer included", () => {
    const line = censusLine(newerChart().census);

    expect(line).toMatchObject({ kind: "complete", aboveMsun: 1e-9, above: null });
  });
});
