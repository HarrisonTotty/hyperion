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

  it("stops at the centre of the edge pixel", () => {
    expect(stepCursor("edge_on", [0, 0, 0], "up", 1, EDGE_ON)).toEqual([0, 0, 16_384]);
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
    ["a point on the edge between the rows, in column 0", -49_152, 0, 4],
  ])("puts %s in the pixel it lies in", (_, horizontalLy, verticalLy, index) => {
    expect(pixelIndexAt(EDGE_ON, { horizontalLy, verticalLy })).toBe(index);
  });

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
});
