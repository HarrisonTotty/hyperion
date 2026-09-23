import { describe, expect, it } from "vitest";

import { mapGeometry, pixelToLy } from "../../lib/galaxy/mapGeometry";
import type { CentreLy } from "../../lib/galaxy/model";
import { localFrameAt } from "../../spatial/frame";
import { vec3 } from "../../spatial/vec3";
import {
  cursorInView,
  markOnPicture,
  pickCursor,
  pixelIndexAt,
  rasterDirection,
  rasterToScreen,
  screenSizePx,
  screenToRaster,
  stepCursor,
  withinExtent,
} from "./mapCursor";

/** A 4 × 2 edge-on map over ±65,536 ly by ±32,768 ly: 32,768 ly a pixel. */
const EDGE_ON = mapGeometry({
  view: "edge_on",
  width_px: 4,
  height_px: 2,
  centre_ly: [0, 0],
  ly_per_px: 32_768,
});

/** A 4 × 4 face-on map over ±65,536 ly. */
const FACE_ON = mapGeometry({
  view: "face_on",
  width_px: 4,
  height_px: 4,
  centre_ly: [0, 0],
  ly_per_px: 32_768,
});

/** The angle of a place on a picture about its centre, anticlockwise from the right, up positive. */
function angleOnPicture(fraction: { left: number; top: number } | null): number {
  return fraction === null ? Number.NaN : Math.atan2(-(fraction.top - 0.5), fraction.left - 0.5);
}

/** Where a point of a view's plane stands on its screen picture. */
function onScreen(view: "face_on" | "edge_on", horizontalLy: number, verticalLy: number) {
  const geometry = view === "face_on" ? FACE_ON : EDGE_ON;
  const place = markOnPicture(geometry, { horizontalLy, verticalLy });
  return place === null ? null : rasterToScreen(view, place.fraction);
}

describe("the views on screen", () => {
  it("shows +x down and +y right face-on, as TOP shows a chart centred on the +x axis", () => {
    // The spatial view's TOP at (26,000, 0, 0) ly puts coreward, -x, straight up and spinward, +y,
    // to the right (camera.test.ts, "on the galactic axes"); the map must agree.
    const frame = localFrameAt(vec3(26_000, 0, 0));
    expect([frame.coreward.x, frame.spinward.y]).toEqual([-1, 1]);

    expect(onScreen("face_on", 32_768, 0)).toEqual({ left: 0.5, top: 0.75 });
    expect(onScreen("face_on", 0, 32_768)).toEqual({ left: 0.75, top: 0.5 });
  });

  it("turns the galaxy counter-clockwise on screen face-on, seen from the north", () => {
    // A point on +x, a quarter-turn on counter-clockwise, lies on +y: on screen, from straight
    // below the centre round to the right of it, which is counter-clockwise with y pointing down.
    const start = onScreen("face_on", 32_768, 0);
    const later = onScreen("face_on", 0, 32_768);

    expect(angleOnPicture(later) - angleOnPicture(start)).toBeCloseTo(Math.PI / 2, 12);
  });

  it("shows x across and z up edge-on, as FRONT does on the +x axis", () => {
    expect(onScreen("edge_on", 32_768, 0)).toEqual({ left: 0.75, top: 0.5 });
    expect(onScreen("edge_on", 0, 16_384)).toEqual({ left: 0.5, top: 0.25 });
  });

  it("finds under a place on screen the raster place that is shown there", () => {
    for (const view of ["face_on", "edge_on"] as const) {
      const fraction = { left: 0.125, top: 0.625 };
      expect(rasterToScreen(view, screenToRaster(view, fraction))).toEqual(fraction);
    }
  });

  it("gives the turned face-on picture the raster's height as its width", () => {
    expect(screenSizePx("face_on", { widthPx: 4, heightPx: 2 })).toEqual({
      widthPx: 2,
      heightPx: 4,
    });
    expect(screenSizePx("edge_on", { widthPx: 4, heightPx: 2 })).toEqual({
      widthPx: 4,
      heightPx: 2,
    });
  });

  it("moves face-on +y on the right arrow and +x on the down arrow", () => {
    const right = stepCursor("face_on", [0, 0, 0], rasterDirection("face_on", "right"), 1, FACE_ON);
    const down = stepCursor("face_on", [0, 0, 0], rasterDirection("face_on", "down"), 1, FACE_ON);

    expect(right).toEqual([0, 32_768, 0]);
    expect(down).toEqual([32_768, 0, 0]);
    expect(rasterDirection("face_on", "left")).toBe("down");
    expect(rasterDirection("face_on", "up")).toBe("left");
    expect(rasterDirection("edge_on", "up")).toBe("up");
  });

  it("places each pixel's centre where the wire says it lies", () => {
    // Plan 04's DensityMap: column i and row j have their centre at
    // (centre + (i + 0.5 - width / 2) × ly per px, centre + (height / 2 - j - 0.5) × ly per px).
    expect(pixelToLy(EDGE_ON, 0, 0)).toEqual({ horizontalLy: -49_152, verticalLy: 16_384 });
    expect(pixelToLy(EDGE_ON, 3, 1)).toEqual({ horizontalLy: 49_152, verticalLy: -16_384 });
  });
});

describe("cursorInView", () => {
  it("takes x and y face-on and x and z edge-on", () => {
    expect(cursorInView("face_on", [1, 2, 3])).toEqual({ horizontalLy: 1, verticalLy: 2 });
    expect(cursorInView("edge_on", [1, 2, 3])).toEqual({ horizontalLy: 1, verticalLy: 3 });
  });
});

describe("withinExtent", () => {
  it.each([
    [{ horizontalLy: 65_536, verticalLy: -32_768 }, true],
    [{ horizontalLy: 0, verticalLy: 32_769 }, false],
    [{ horizontalLy: -65_537, verticalLy: 0 }, false],
  ])("is %j within the edge-on extent: %s", (point, within) => {
    expect(withinExtent(EDGE_ON, point)).toBe(within);
  });
});

describe("pickCursor", () => {
  it("sets x and y face-on, keeping z", () => {
    expect(pickCursor("face_on", [1, 2, 3], { horizontalLy: 10, verticalLy: 20 }, FACE_ON)).toEqual(
      [10, 20, 3],
    );
  });

  it("sets only z edge-on, although the pick has an x", () => {
    expect(pickCursor("edge_on", [1, 2, 3], { horizontalLy: 10, verticalLy: 20 }, EDGE_ON)).toEqual(
      [1, 2, 20],
    );
  });

  it("keeps the cursor within the centres of the map's edge pixels", () => {
    // Half a pixel, 16,384 ly, inside the edge-on map's top edge at 32,768 ly.
    expect(
      pickCursor("edge_on", [0, 0, 0], { horizontalLy: 0, verticalLy: 40_000 }, EDGE_ON),
    ).toEqual([0, 0, 16_384]);
    // The face-on map's right edge is the root cube's face at 65,536 ly, which is outside it.
    expect(
      pickCursor("face_on", [0, 0, 0], { horizontalLy: 65_536, verticalLy: -65_536 }, FACE_ON),
    ).toEqual([49_152, -49_152, 0]);
  });
});

describe("stepCursor", () => {
  it("moves up and down by a map pixel a step", () => {
    expect(stepCursor("face_on", [0, 0, 0], "up", 1, FACE_ON)).toEqual([0, 32_768, 0]);
    expect(stepCursor("face_on", [0, 0, 0], "down", 1, FACE_ON)).toEqual([0, -32_768, 0]);
  });

  it("reaches the centre of the edge pixel when it lies a whole pixel away", () => {
    expect(stepCursor("edge_on", [0, 0, -16_384], "up", 1, EDGE_ON)).toEqual([0, 0, 16_384]);
  });

  it("stays put when a whole pixel's step would reach the map's edge", () => {
    // From the plane the map's edge lies a whole pixel away, and a step stops strictly inside it
    // (the orchestrator's rulings 28 and 35).
    expect(stepCursor("edge_on", [0, 0, 0], "up", 1, EDGE_ON)).toEqual([0, 0, 0]);
  });

  it("stops a face-on step at the edge a whole number of pixels on, not at the edge pixel's centre", () => {
    // From the galactic centre both faces are two whole pixels away. The root cube holds its lower
    // face at -65,536 ly and not its upper one at 65,536 ly, so a step stops on the one and a pixel
    // short of the other (the orchestrator's ruling 35).
    expect(stepCursor("face_on", [0, 0, 0], "right", 10, FACE_ON)).toEqual([32_768, 0, 0]);
    expect(stepCursor("face_on", [0, 0, 0], "down", 10, FACE_ON)).toEqual([0, -65_536, 0]);
  });

  it("moves only the axis it steps along face-on", () => {
    // A typed x in the outer half of an edge pixel, where no pick puts it, stays through a y step.
    expect(stepCursor("face_on", [-60_000, 0, 7], "up", 1, FACE_ON)).toEqual([-60_000, 32_768, 7]);
  });

  it("brings a typed height beyond the map to the edge pixel's centre, as a pick would", () => {
    expect(stepCursor("edge_on", [0, 0, 100_000], "down", 1, EDGE_ON)).toEqual([0, 0, 16_384]);
    expect(stepCursor("edge_on", [0, 0, -50_000], "down", 1, EDGE_ON)).toEqual([0, 0, -16_384]);
  });

  it("does not move sideways edge-on", () => {
    expect(stepCursor("edge_on", [0, 0, 0], "left", 1, EDGE_ON)).toBeNull();
  });
});

describe("markOnPicture", () => {
  it("places the centre half-way and an edge at the picture's edge", () => {
    expect(markOnPicture(FACE_ON, { horizontalLy: 0, verticalLy: 0 })).toEqual({
      fraction: { left: 0.5, top: 0.5 },
      peg: null,
    });
    expect(markOnPicture(FACE_ON, { horizontalLy: 65_536, verticalLy: 65_536 })).toEqual({
      fraction: { left: 1, top: 0 },
      peg: null,
    });
  });

  it("pegs a point above or below the map to its edge", () => {
    expect(markOnPicture(EDGE_ON, { horizontalLy: 0, verticalLy: 50_000 })).toEqual({
      fraction: { left: 0.5, top: 0 },
      peg: "above",
    });
    expect(markOnPicture(EDGE_ON, { horizontalLy: 0, verticalLy: -50_000 })).toEqual({
      fraction: { left: 0.5, top: 1 },
      peg: "below",
    });
  });

  it("places nothing beyond the map's sides", () => {
    expect(markOnPicture(EDGE_ON, { horizontalLy: 70_000, verticalLy: 0 })).toBeNull();
  });
});

describe("pixelIndexAt", () => {
  it.each([
    ["the top left corner", -65_536, 32_768, 0],
    ["the bottom right corner", 65_536, -32_768, 7],
    ["a point on the edge between columns 1 and 2, in row 0", 0, 16_384, 2],
    ["a point on the edge between the rows, in column 0", -49_152, 0, 0],
  ])("puts %s in the pixel it lies in", (_, horizontalLy, verticalLy, index) => {
    expect(pixelIndexAt(EDGE_ON, { horizontalLy, verticalLy })).toBe(index);
  });

  it.each([
    ["columns 0 and 1 in column 1", -32_768, 16_384, 5],
    ["columns 2 and 3 in column 3", 32_768, 16_384, 7],
    ["rows 0 and 1 in row 0", -16_384, 32_768, 1],
    ["rows 2 and 3 in row 2", -16_384, -32_768, 9],
    ["the middle columns in column 2", 0, 16_384, 6],
    ["the middle rows in row 1", -16_384, 0, 5],
    ["the map's bottom edge in the bottom row", -16_384, -65_536, 13],
  ])(
    "puts a point on the edge between %s, the pixel on its +x or +y side",
    (_, horizontalLy, verticalLy, index) => {
      // The 4 × 4 face-on map: row 0 is the top, the most +y, and column 0 the most -x. Each pixel
      // holds its lower edges, as the root cube holds its lower faces.
      expect(pixelIndexAt(FACE_ON, { horizontalLy, verticalLy })).toBe(index);
    },
  );

  it("gives nothing off the map", () => {
    expect(pixelIndexAt(EDGE_ON, { horizontalLy: 0, verticalLy: -40_000 })).toBeNull();
  });
});

describe("the chart centre's height", () => {
  // Plan 04's 128-pixel maps: 1,024 ly a pixel, the edge-on map 64 rows over ±32,768 ly, so that no
  // row is centred on the plane; the one nearest it is centred half a pixel off, at ±512 ly.
  const FACE_ON_128 = mapGeometry({
    view: "face_on",
    width_px: 128,
    height_px: 128,
    centre_ly: [0, 0],
    ly_per_px: 1_024,
  });
  const EDGE_ON_128 = mapGeometry({
    view: "edge_on",
    width_px: 128,
    height_px: 64,
    centre_ly: [0, 0],
    ly_per_px: 1_024,
  });
  const DIRECTIONS = ["left", "right", "up", "down"] as const;

  it.each([
    ["a pixel's centre", 512, -1_536],
    ["a pixel's corner", 0, 1_024],
    ["a point inside a pixel", 26_000.3, -7_777.7],
    ["a point beyond the map's edge", 70_000, -70_000],
  ])("keeps z exactly 0 through a face-on pick at %s", (_, horizontalLy, verticalLy) => {
    const [, , zLy] = pickCursor("face_on", [0, 0, 0], { horizontalLy, verticalLy }, FACE_ON_128);

    // `toBe` compares with `Object.is`, so −0 fails it too.
    expect(zLy).toBe(0);
  });

  it("keeps z exactly 0 through face-on steps of one and ten pixels, into the edges and back", () => {
    let cursorLy: CentreLy = [0, 0, 0];
    // Every height a step left, other than +0 exactly: `Object.is` tells −0 from 0.
    const offThePlane: number[] = [];
    for (const pixels of [1, 10]) {
      for (const direction of DIRECTIONS) {
        // Far enough to stop at the edge pixel, then back the other way.
        for (let step = 0; step < 80; step += 1) {
          cursorLy = stepCursor("face_on", cursorLy, direction, pixels, FACE_ON_128) ?? cursorLy;
          if (!Object.is(cursorLy[2], 0)) {
            offThePlane.push(cursorLy[2]);
          }
        }
      }
    }

    expect(offThePlane).toEqual([]);
  });

  it.each([
    ["on the plane", 0],
    ["off a row's centre", -300],
  ])("takes an edge-on pick's height where it is, %s, not a row's centre", (_, verticalLy) => {
    const [, , zLy] = pickCursor(
      "edge_on",
      [26_000, 0, 5],
      { horizontalLy: 0, verticalLy },
      EDGE_ON_128,
    );

    expect(zLy).toBe(verticalLy);
  });

  /** The cursor after `steps` one-pixel edge-on steps in `direction`. */
  function steppedEdgeOn(cursorLy: CentreLy, direction: "up" | "down", steps: number): CentreLy {
    let stepped = cursorLy;
    for (let step = 0; step < steps; step += 1) {
      stepped = stepCursor("edge_on", stepped, direction, 1, EDGE_ON_128) ?? stepped;
    }
    return stepped;
  }

  it("steps z by whole pixels from the plane edge-on", () => {
    expect(steppedEdgeOn([26_000, 0, 0], "down", 7)[2]).toBe(-7_168);
  });

  it("steps z back onto the plane edge-on", () => {
    expect(steppedEdgeOn(steppedEdgeOn([26_000, 0, 0], "down", 7), "up", 7)[2]).toBe(0);
  });

  it.each([
    ["up", 31_744],
    ["down", -32_768],
  ] as const)(
    "stops a step %s at the edge on the cursor's own lattice, not on the edge pixel's centre",
    (direction, zLy) => {
      // The edge pixels' centres are ±32,256 ly, half a pixel off every whole pixel from the plane.
      // A step keeps within the map's half-open extent, which holds its bottom edge at -32,768 ly
      // and not its top edge at +32,768 ly (the orchestrator's ruling 35).
      expect(steppedEdgeOn([26_000, 0, 0], direction, 40)[2]).toBe(zLy);
    },
  );

  /**
   * One-pixel edge-on steps in `direction` until the cursor stops at the edge, and how many of them
   * moved it.
   */
  function steppedToEdge(cursorLy: CentreLy, direction: "up" | "down", pixels: number) {
    let stepped = cursorLy;
    let movedPx = 0;
    for (let step = 0; step < 100; step += 1) {
      const next = stepCursor("edge_on", stepped, direction, pixels, EDGE_ON_128) ?? stepped;
      movedPx += Math.round(Math.abs(next[2] - stepped[2]) / EDGE_ON_128.lyPerPx);
      stepped = next;
    }
    return { stepped, movedPx };
  }

  it.each([
    ["the plane", "up", 0],
    ["the plane", "down", 0],
    ["a typed height", "up", 300],
    ["a height off the whole pixels", "down", -700.25],
  ] as const)(
    "comes back exactly to %s after a clamp %s and the same number of steps back",
    (_, direction, startLy) => {
      const back = direction === "up" ? "down" : "up";
      const { stepped, movedPx } = steppedToEdge([26_000, 0, startLy], direction, 1);

      expect(steppedEdgeOn(stepped, back, movedPx)[2]).toBe(startLy);
    },
  );

  it("stops a clamped ten-pixel step a whole number of pixels on, which one-pixel steps undo", () => {
    const { stepped, movedPx } = steppedToEdge([26_000, 0, 0], "up", 10);

    expect(stepped[2]).toBe(31_744);
    expect(steppedEdgeOn(stepped, "down", movedPx)[2]).toBe(0);
  });

  /** The row of the edge-on map the cursor's height lies in, 0 at the top. */
  function edgeOnRow(cursorLy: CentreLy): number | null {
    const index = pixelIndexAt(EDGE_ON_128, cursorInView("edge_on", cursorLy));
    return index === null ? null : Math.floor(index / EDGE_ON_128.widthPx);
  }

  /** The rows read on the way from the bottom edge to the top one, a one-pixel step at a time. */
  function rowsReadBottomToTop(startLy: CentreLy): Array<number | null> {
    let stepped = steppedEdgeOn(startLy, "down", 70);
    const rows = [edgeOnRow(stepped)];
    for (let step = 0; step < 70; step += 1) {
      stepped = steppedEdgeOn(stepped, "up", 1);
      if (edgeOnRow(stepped) !== rows.at(-1)) {
        rows.push(edgeOnRow(stepped));
      }
    }
    return rows;
  }

  /** Every row of the edge-on map, from the bottom one up. */
  const EVERY_ROW = Array.from({ length: 64 }, (_, index) => 63 - index);

  it.each([
    ["up", 0],
    ["down", 63],
  ] as const)(
    "reaches the edge row by arrow keys from the plane, stepping %s",
    (direction, row) => {
      // The orchestrator's ruling 35: +31,744 ly is the top row's lower edge, and reads the top row.
      expect(edgeOnRow(steppedEdgeOn([26_000, 0, 0], direction, 40))).toBe(row);
    },
  );

  it.each([
    ["the plane", 0],
    ["a typed height", 300],
    ["a height off the whole pixels", -700.25],
    ["a height half a pixel off them", 512],
  ])("reads every row, one after another, by arrow keys from %s", (_, startLy) => {
    expect(rowsReadBottomToTop([26_000, 0, startLy])).toEqual(EVERY_ROW);
  });

  /** The cursor after `steps` face-on steps of `pixels` in `direction` on the raster. */
  function steppedFaceOn(
    cursorLy: CentreLy,
    direction: (typeof DIRECTIONS)[number],
    steps: number,
    pixels = 1,
  ): CentreLy {
    let stepped = cursorLy;
    for (let step = 0; step < steps; step += 1) {
      stepped = stepCursor("face_on", stepped, direction, pixels, FACE_ON_128) ?? stepped;
    }
    return stepped;
  }

  /** The face-on pixel the cursor lies in, as its column and row. */
  function faceOnPixel(cursorLy: CentreLy): { column: number; row: number } | null {
    const index = pixelIndexAt(FACE_ON_128, cursorInView("face_on", cursorLy));
    return index === null ? null : { column: index % 128, row: Math.floor(index / 128) };
  }

  it.each([
    ["the galactic centre", [0, 0, 0]],
    ["a typed point off the whole pixels", [26_000.5, -7_777.25, 0]],
  ] as const)(
    "reads every column and every row face-on by arrow keys from %s",
    (_, startLy: CentreLy) => {
      const read = (backward: "left" | "down", forward: "right" | "up", axis: "column" | "row") => {
        let stepped = steppedFaceOn(startLy, backward, 140);
        const seen = new Set<number>();
        for (let step = 0; step <= 140; step += 1) {
          const pixel = faceOnPixel(stepped);
          if (pixel !== null) {
            seen.add(pixel[axis]);
          }
          stepped = steppedFaceOn(stepped, forward, 1);
        }
        return seen.size;
      };

      expect([read("left", "right", "column"), read("down", "up", "row")]).toEqual([128, 128]);
    },
  );

  it.each(DIRECTIONS)(
    "comes back exactly to where it was after a face-on clamp %s and the same number of steps back",
    (direction) => {
      const back = { left: "right", right: "left", up: "down", down: "up" } as const;
      const startLy: CentreLy = [26_000.5, -7_777.25, 0];
      let stepped = startLy;
      let movedPx = 0;
      for (let step = 0; step < 20; step += 1) {
        const next = stepCursor("face_on", stepped, direction, 10, FACE_ON_128) ?? stepped;
        movedPx += Math.round(
          (Math.abs(next[0] - stepped[0]) + Math.abs(next[1] - stepped[1])) / FACE_ON_128.lyPerPx,
        );
        stepped = next;
      }

      expect(steppedFaceOn(stepped, back[direction], movedPx)).toEqual(startLy);
    },
  );
});
