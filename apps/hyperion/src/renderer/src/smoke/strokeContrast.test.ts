import { describe, expect, it } from "vitest";

import { NO_TURN, FIXTURE_SYSTEM } from "../test/viewFixtures";
import { vec3 } from "../geometry/vec3";
import { emptyDrawList, type LineBatch, viewStrokesAt } from "../view/wireframe/drawList";
import {
  contrastRatio,
  reachCounts,
  type ReadImage,
  readStroke,
  type ScreenStroke,
  screenStrokes,
  srgb8,
  tokenLuminance,
  wcagLuminance,
} from "./strokeContrast";

/** A 16 × 16 image of black, but for the rows given, each at its linear grey. */
function rows(lit: Readonly<Record<number, number>>): ReadImage {
  const colour = new Float32Array(16 * 16 * 4);
  for (let row = 0; row < 16; row += 1) {
    for (let column = 0; column < 16; column += 1) {
      const grey = lit[row] ?? 0;
      colour.set([grey, grey, grey, 1], (row * 16 + column) * 4);
    }
  }
  return { colour, widthPx: 16, heightPx: 16 };
}

/** A horizontal stroke across the image at `yPx`, `widthPx` wide, solid or dashed. */
function across(yPx: number, widthPx: number, dash: LineBatch["dash"] = null): ScreenStroke {
  return {
    name: "line",
    segments: [
      [
        { x: 2, y: yPx },
        { x: 14, y: yPx },
      ],
    ],
    phases: [0],
    widthPx,
    casingWidthPx: 0,
    dash,
  };
}

const BLACK = wcagLuminance(0, 0, 0);

/** Nothing else reaches the stroke. */
const CLEAR = (): boolean => false;

describe("the stroke check's arithmetic", () => {
  it("stores a linear value as the canvas's 8-bit sRGB code", () => {
    expect([srgb8(0), srgb8(1), srgb8(0.5), srgb8(-1), srgb8(2)]).toEqual([0, 255, 188, 0, 255]);
  });

  it("scores white on black at 21:1, by WCAG's luminance", () => {
    expect([tokenLuminance("#ffffff"), contrastRatio(tokenLuminance("#ffffff"), BLACK)]).toEqual([
      1, 21,
    ]);
  });
});

describe("readStroke", () => {
  it("reads a stroke's brightest texel across it, at every pixel of its length", () => {
    const reading = readStroke(rows({ 8: 1 }), across(8.5, 1), CLEAR, BLACK, null);
    expect([reading.samples, reading.worst]).toEqual([13, 21]);
  });

  it("reads a stroke split across two rows at the brighter of them", () => {
    const reading = readStroke(rows({ 7: 0.25, 8: 0.5 }), across(8, 1), CLEAR, BLACK, null);
    expect(reading.worst).toBeCloseTo(contrastRatio(wcagLuminance(188, 188, 188), BLACK), 12);
  });

  it("finds a 45° stroke's centreline texel at every point, the grid's corners included", () => {
    // Only the diagonal is lit. At (3, 3), a corner, its texels' centres lie 0.71 px along.
    const colour = new Float32Array(16 * 16 * 4);
    for (let i = 0; i < 16; i += 1) {
      colour.set([1, 1, 1, 1], (i * 16 + i) * 4);
    }
    const diagonal: ScreenStroke = {
      ...across(0, 2),
      segments: [
        [
          { x: 2, y: 2 },
          { x: 14, y: 14 },
        ],
      ],
    };
    const reading = readStroke({ colour, widthPx: 16, heightPx: 16 }, diagonal, CLEAR, BLACK, null);
    expect([reading.samples, reading.worst]).toEqual([17, 21]);
  });

  it("reads a dash's on lengths alone, away from their ends", () => {
    // On 4 px, off 4 px from x = 2: the points 1, 2 and 3 px into each dash, none in the gaps.
    const reading = readStroke(rows({}), across(8.5, 1, { onPx: 4, offPx: 4 }), CLEAR, BLACK, null);
    expect(reading.samples).toBe(6);
  });

  it("leaves out the points another batch's stroke or casing can reach", () => {
    const other: ScreenStroke = {
      ...across(8.5, 2),
      name: "other",
      segments: [
        [
          { x: 8, y: 0 },
          { x: 8, y: 16 },
        ],
      ],
      casingWidthPx: 2,
    };
    // Its reach, 1 + 2 + 0.5 and a texel's half-diagonal: columns 4 to 11. A cross-section at x
    // holds columns x − 1 and x, so the points at x = 4 to 12 are not read.
    const line = across(8.5, 1);
    const { counts, masks } = reachCounts([line, other], 16, 16);
    const blocked = (column: number, row: number): boolean =>
      (counts[row * 16 + column] ?? 0) - (masks[0]?.[row * 16 + column] ?? 0) > 0;
    const reading = readStroke(rows({ 8: 1 }), line, blocked, BLACK, null);
    expect([
      // Its first row's texels, which the vertical stroke reaches.
      Array.from({ length: 16 }, (_, column) => masks[1]?.[column]).join(""),
      reading.samples,
    ]).toEqual(["0000111111110000", 13 - 9]);
  });

  it("reads only the points it is asked to keep", () => {
    const reading = readStroke(rows({ 8: 1 }), across(8.5, 1), CLEAR, BLACK, (p) => p.x < 6);
    expect(reading.samples).toBe(4);
  });
});

describe("screenStrokes", () => {
  it("runs a dash's phase on across a joint and starts it again at a break", () => {
    const batch: LineBatch = {
      id: "path",
      space: "screen",
      originF32: new Float32Array(3),
      segments: new Float32Array([0, 0, 0, 3, 4, 0, 3, 4, 0, 3, 10, 0, 20, 20, 0, 21, 20, 0]),
      token: "text",
      colour: "#ffffff",
      widthPx: 2,
      casingWidthPx: 2,
      casingColour: "#000000",
      dash: { onPx: 12, offPx: 8 },
    };
    const camera = {
      pose: {
        frame: { kind: "system", system: FIXTURE_SYSTEM },
        positionM: vec3(0, 0, 0),
        orientation: NO_TURN,
      },
      fovXRad: Math.PI / 3,
    } as const;
    const [stroke] = screenStrokes({ ...emptyDrawList(viewStrokesAt(1)), lines: [batch] }, camera, {
      widthPx: 64,
      heightPx: 64,
    });
    expect(stroke?.phases).toEqual([0, 5, 0]);
  });
});
