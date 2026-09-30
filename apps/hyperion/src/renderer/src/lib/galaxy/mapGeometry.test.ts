import { describe, expect, it } from "vitest";

import { lyToPixel, mapGeometry, type MapRaster, pixelToLy } from "./mapGeometry";

const FACE_ON: MapRaster = {
  view: "face_on",
  width_px: 512,
  height_px: 512,
  centre_ly: [0, 0],
  ly_per_px: 256,
};

const EDGE_ON: MapRaster = { ...FACE_ON, view: "edge_on", height_px: 256 };

describe("mapGeometry", () => {
  it("spans ±65,536 ly on both axes for a face-on map of 512 pixels at 256 ly", () => {
    const geometry = mapGeometry(FACE_ON);

    expect(geometry.horizontal).toEqual({ minLy: -65_536, maxLy: 65_536 });
    expect(geometry.vertical).toEqual({ minLy: -65_536, maxLy: 65_536 });
    expect(geometry.verticalAxis).toBe("y");
  });

  it("spans ±32,768 ly vertically for the 512 × 256 edge-on map, along z", () => {
    const geometry = mapGeometry(EDGE_ON);

    expect(geometry.horizontal).toEqual({ minLy: -65_536, maxLy: 65_536 });
    expect(geometry.vertical).toEqual({ minLy: -32_768, maxLy: 32_768 });
    expect(geometry.verticalAxis).toBe("z");
  });

  it("follows an off-origin centre", () => {
    const geometry = mapGeometry({
      ...FACE_ON,
      centre_ly: [1_000, -2_000],
      width_px: 4,
      height_px: 2,
    });

    expect(geometry.horizontal).toEqual({ minLy: 1_000 - 512, maxLy: 1_000 + 512 });
    expect(geometry.vertical).toEqual({ minLy: -2_000 - 256, maxLy: -2_000 + 256 });
  });
});

describe("pixelToLy", () => {
  const geometry = mapGeometry(FACE_ON);

  it("gives row 0 the largest vertical value", () => {
    expect(pixelToLy(geometry, 0, 0).verticalLy).toBe(65_536 - 128);
    expect(pixelToLy(geometry, 0, 511).verticalLy).toBe(-65_536 + 128);
  });

  it("gives column 0 the smallest horizontal value", () => {
    expect(pixelToLy(geometry, 0, 0).horizontalLy).toBe(-65_536 + 128);
    expect(pixelToLy(geometry, 511, 0).horizontalLy).toBe(65_536 - 128);
  });

  it("puts the middle of the picture at the centre", () => {
    expect(pixelToLy(geometry, 255.5, 255.5)).toEqual({ horizontalLy: 0, verticalLy: 0 });
  });
});

describe("lyToPixel", () => {
  it("inverts pixelToLy on every pixel of a 5 × 3 map", () => {
    const geometry = mapGeometry({
      view: "edge_on",
      width_px: 5,
      height_px: 3,
      centre_ly: [120, -7.5],
      ly_per_px: 12.5,
    });

    for (let row = 0; row < 3; row += 1) {
      for (let column = 0; column < 5; column += 1) {
        const point = pixelToLy(geometry, column, row);
        const back = lyToPixel(geometry, point.horizontalLy, point.verticalLy);

        expect(back.column).toBeCloseTo(column, 12);
        expect(back.row).toBeCloseTo(row, 12);
      }
    }
  });

  it("clamps a point outside the map to the picture's edge", () => {
    const geometry = mapGeometry(FACE_ON);

    expect(lyToPixel(geometry, 1e6, -1e6)).toEqual({ column: 511.5, row: 511.5 });
    expect(lyToPixel(geometry, -1e6, 1e6)).toEqual({ column: -0.5, row: -0.5 });
    expect(lyToPixel(geometry, 65_536, 0)).toEqual({ column: 511.5, row: 255.5 });
  });
});
