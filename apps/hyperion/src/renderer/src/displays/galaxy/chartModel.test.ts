import { describe, expect, it } from "vitest";

import { toChartResult } from "../../lib/galaxy/wire";
import { localFrameAt } from "../../spatial/frame";
import { vec3 } from "../../spatial/vec3";
import { aCensus, aSystemsInRange, LAYERS } from "../../test/galaxyFixtures";
import {
  censusHint,
  censusLine,
  chartDataFault,
  distanceDecimalsFor,
  formatBandMsun,
  formatChartLengthLy,
  inRangeCount,
  layerBands,
  queryRadiusForDriveRange,
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

    expect(inRangeCount(result, 50)).toBe(1);
  });
});

describe("censusLine", () => {
  it("names the mass a complete census is complete above", () => {
    expect(censusLine({ kind: "complete", aboveMsun: 0.5 })).toEqual({
      kind: "complete",
      text: "COMPLETE ABOVE",
      aboveMsun: 0.5,
    });
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

describe("queryRadiusForDriveRange", () => {
  it.each([
    [50, 50],
    [80, 100],
    [0.005, 0.01],
    [600, 500],
  ])("follows a drive range of %s ly with %s ly", (driveRangeLy, radiusLy) => {
    expect(queryRadiusForDriveRange(driveRangeLy)).toBe(radiusLy);
  });
});
