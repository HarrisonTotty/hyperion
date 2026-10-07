import { describe, expect, it } from "vitest";

import { NO_TURN, FIXTURE_SYSTEM } from "../test/viewFixtures";
import { vec3 } from "../geometry/vec3";
import { emptyDrawList, type LineBatch, viewStrokesAt } from "../view/wireframe/drawList";
import {
  contrastRatio,
  neighbourOf,
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

/** No other batch in the frame. */
const CLEAR = [] as const;

/** A vertical stroke down the image at `xPx`, `widthPx` wide, cased `casingPx` each side. */
function down(xPx: number, widthPx: number, casingPx: number): ScreenStroke {
  return {
    ...across(0, widthPx),
    name: "other",
    segments: [
      [
        { x: xPx, y: 0 },
        { x: xPx, y: 16 },
      ],
    ],
    casingWidthPx: casingPx,
  };
}

/** The contrast of a linear grey against black, through its 8-bit code. */
function greyOnBlack(grey: number): number {
  const code = srgb8(grey);
  return contrastRatio(wcagLuminance(code, code, code), BLACK);
}

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

  it("leaves out the points about a crossing that the crossing batch reaches", () => {
    // Drawn after the line, its casing covers it within 1 + 2 + 0.5 px of x = 8: columns 5 to 10.
    // A cross-section at x holds columns x − 1 and x, so the points at x = 5 to 11 are not read.
    const later = neighbourOf(down(8, 2, 2), true, 16, 16);
    const reading = readStroke(rows({ 8: 1 }), across(8.5, 1), [later], BLACK, null);
    expect([
      Array.from({ length: 16 }, (_, column) => later.region[column]).join(""),
      reading.samples,
    ]).toEqual(["0000011111100000", 13 - 7]);
  });

  it("leaves out a crossing batch drawn before it only where it lights the line", () => {
    // Drawn before, it lights the line within 1 + 0.5 px of x = 8: columns 7 and 8, so the points
    // at x = 7 to 9 are not read.
    const earlier = neighbourOf(down(8, 2, 2), false, 16, 16);
    const reading = readStroke(rows({ 8: 1 }), across(8.5, 1), [earlier], BLACK, null);
    expect(reading.samples).toBe(13 - 3);
  });

  it("leaves out a crossing that ends on the line, within its half-width", () => {
    const ending: ScreenStroke = {
      ...down(8, 2, 2),
      segments: [
        [
          { x: 8, y: 0 },
          { x: 8, y: 8.3 },
        ],
      ],
    };
    const reading = readStroke(
      rows({ 8: 1 }),
      across(8.5, 1),
      [neighbourOf(ending, true, 16, 16)],
      BLACK,
      null,
    );
    expect(reading.samples).toBe(13 - 7);
  });

  it("leaves out the texels a parallel neighbour lights, and reads the rest of the section", () => {
    // The line's 2 px lights rows 7 and 8 at half its colour; a neighbour 2.5 px below, drawn
    // before it, lights rows 9 to 11 fully. The line's cross-section holds rows 6 to 9, and row 9,
    // the neighbour's light, would read 21:1.
    const neighbour = neighbourOf(across(10.5, 2), false, 16, 16);
    const reading = readStroke(
      rows({ 7: 0.5, 8: 0.5, 9: 1, 10: 1, 11: 1 }),
      across(8, 2),
      [neighbour],
      BLACK,
      null,
    );
    expect([reading.samples, reading.worst]).toEqual([13, greyOnBlack(0.5)]);
  });

  it("counts a later neighbour's casing against the line, leaving its own faint edge", () => {
    // A neighbour 2.5 px below, drawn after with a 2 px casing, covers rows 7 to 13: of the line's
    // 2 px, only row 6, its faint edge, is left to read.
    const neighbour = neighbourOf({ ...across(10.5, 2), casingWidthPx: 2 }, true, 16, 16);
    const reading = readStroke(
      rows({ 6: 0.1, 7: 1, 8: 1 }),
      across(8, 2),
      [neighbour],
      BLACK,
      null,
    );
    expect([reading.samples, reading.worst]).toEqual([13, greyOnBlack(0.1)]);
  });

  it("does not read a cross-section the neighbour wholly covers", () => {
    // A neighbour 1 px below the 1 px line, not crossing it, covers rows 6 to 12 with its casing.
    const neighbour = neighbourOf({ ...across(9.5, 2), casingWidthPx: 2 }, true, 16, 16);
    const reading = readStroke(rows({ 8: 1 }), across(8.5, 1), [neighbour], BLACK, null);
    expect([reading.samples, reading.worst]).toEqual([0, Number.POSITIVE_INFINITY]);
  });

  it("keeps clear of a segment's ends when asked: a pixel at each end", () => {
    const reading = readStroke(rows({ 8: 1 }), across(8.5, 1), CLEAR, BLACK, null, 1);
    expect(reading.samples).toBe(13 - 2);
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
