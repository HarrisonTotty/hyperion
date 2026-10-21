import { describe, expect, it } from "vitest";

import { normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { project } from "../../../view/camera/projection";
import { IDENTITY_QUATERNION } from "../../../view/camera/quaternion";
import { METER_CLASS } from "../../../view/post/meter";
import { toHalfBits } from "../../../view/sky/half";
import { texelOfDirection } from "./handoverGeometry";
import {
  bodyRect,
  compareReadings,
  type FrameCamera,
  type FrameReading,
  OFF_BODY,
  type PixelRect,
  readFrame,
  UNSURVEYED,
} from "./handoverReading";

/** A sphere of 1 km. */
const SPHERE = { equatorialRadiusM: 1000, polarRadiusM: 1000 };

const FOV_X_RAD = Math.PI / 3;

/** A camera 5 km out along +z looking down −z, its axes the frame's (R02's identity pose). */
const CAMERA: FrameCamera = {
  eyeM: vec3(0, 0, 5000),
  right: vec3(1, 0, 0),
  up: vec3(0, 1, 0),
  back: vec3(0, 0, 1),
  fovXRad: FOV_X_RAD,
};

const WIDTH = 40;
const HEIGHT = 30;
const WHOLE: PixelRect = { x: 0, y: 0, width: WIDTH, height: HEIGHT };

/** One texel of a synthetic HDR: its linear red, green and blue and its meter class. */
interface Texel {
  readonly rgb: readonly [number, number, number];
  readonly meter: number;
}

/** A synthetic `rgba16float` region, each pixel's texel by `at`. */
function hdr(rect: PixelRect, at: (x: number, y: number) => Texel): Uint16Array {
  const halves = new Uint16Array(rect.width * rect.height * 4);
  for (let row = 0; row < rect.height; row += 1) {
    for (let column = 0; column < rect.width; column += 1) {
      const { rgb, meter } = at(rect.x + column, rect.y + row);
      const p = (row * rect.width + column) * 4;
      halves.set(
        [toHalfBits(rgb[0]), toHalfBits(rgb[1]), toHalfBits(rgb[2]), toHalfBits(meter)],
        p,
      );
    }
  }
  return halves;
}

/** The pixel a body-fixed point projects into from {@link CAMERA}, by R02's own projection. */
function pixelOf(point: Vec3): readonly [number, number] {
  const projected = project(
    sub(point, CAMERA.eyeM),
    { fovXRad: FOV_X_RAD, orientation: IDENTITY_QUATERNION },
    { widthPx: WIDTH, heightPx: HEIGHT },
  );
  return [Math.floor(projected.xPx), Math.floor(projected.yPx)];
}

const OTHER: Texel = { rgb: [4, 4, 4], meter: METER_CLASS.other };

const LIT: Texel = { rgb: [0.5, 0.25, 0.125], meter: METER_CLASS.litBody };

describe("a frame's reading", () => {
  it("assigns a body pixel the cell its centre's ray meets, the inverse of R02's projection", () => {
    const point = scale(normalise(vec3(0.3, 0.3, 1)), 1000);
    const [px, py] = pixelOf(point);
    const level = 1;
    const cell = texelOfDirection(point, level);
    const reading = readFrame({
      halfBits: hdr(WHOLE, (x, y) =>
        x === px && y === py ? { rgb: [0.5, 0.5, 0.5], meter: METER_CLASS.litBody } : OTHER,
      ),
      widthPx: WIDTH,
      heightPx: HEIGHT,
      rect: WHOLE,
      camera: CAMERA,
      figure: SPHERE,
      level,
      surveyed: new Set([cell]),
    });
    expect([reading.cells[py * WIDTH + px], reading.bodyPixels, reading.surveyPixels]).toEqual([
      cell,
      1,
      1,
    ]);
  });

  it("takes the lit and unlit body's pixels alone, each by Rec. 709's luminance", () => {
    const meters = [
      METER_CLASS.hostDisc,
      METER_CLASS.other,
      METER_CLASS.litBody,
      METER_CLASS.unlitBody,
    ];
    const rect = { x: 0, y: 0, width: 4, height: 1 };
    const reading = readFrame({
      halfBits: hdr(rect, (x) => ({ rgb: [1, 0.5, 0.25], meter: meters[x] ?? 0 })),
      widthPx: 4,
      heightPx: 1,
      rect,
      camera: CAMERA,
      figure: SPHERE,
      level: 1,
      surveyed: new Set(),
    });
    const luminance = 0.2126 * 1 + 0.7152 * 0.5 + 0.0722 * 0.25;
    expect([
      Array.from(reading.cells.subarray(0, 2)),
      reading.bodyPixels,
      reading.luminance[2],
      reading.luminance[3],
    ]).toEqual([
      [OFF_BODY, OFF_BODY],
      2,
      expect.closeTo(luminance, 6),
      expect.closeTo(luminance, 6),
    ]);
  });

  it("counts a body pixel whose ray misses the datum, the camera model's check", () => {
    const reading = readFrame({
      halfBits: hdr(WHOLE, (x, y) =>
        x === 0 && y === 0 ? { rgb: [1, 1, 1], meter: METER_CLASS.unlitBody } : OTHER,
      ),
      widthPx: WIDTH,
      heightPx: HEIGHT,
      rect: WHOLE,
      camera: CAMERA,
      figure: SPHERE,
      level: 1,
      surveyed: new Set([
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
      ]),
    });
    expect([reading.bodyMisses, reading.cells[0]]).toEqual([1, UNSURVEYED]);
  });

  it("reads a region's pixels as the whole target's at the same places", () => {
    const rect = { x: 16, y: 11, width: 8, height: 8 };
    const surveyed = new Set(Array.from({ length: 24 }, (_, i) => i));
    const read = (region: PixelRect): FrameReading =>
      readFrame({
        halfBits: hdr(region, () => LIT),
        widthPx: WIDTH,
        heightPx: HEIGHT,
        rect: region,
        camera: CAMERA,
        figure: SPHERE,
        level: 1,
        surveyed,
      });
    const whole = read(WHOLE);
    const part = read(rect);
    const inPart = Array.from({ length: 64 }, (_, p) => part.cells[p]);
    const inWhole = Array.from(
      { length: 64 },
      (_, p) => whole.cells[(rect.y + Math.floor(p / 8)) * WIDTH + rect.x + (p % 8)],
    );
    expect(inPart).toEqual(inWhole);
  });

  it("refuses texels that do not fill their region", () => {
    expect(() =>
      readFrame({
        halfBits: new Uint16Array(8),
        widthPx: WIDTH,
        heightPx: HEIGHT,
        rect: WHOLE,
        camera: CAMERA,
        figure: SPHERE,
        level: 1,
        surveyed: new Set(),
      }),
    ).toThrow(/does not fill/);
  });
});

describe("the region about the body", () => {
  it("holds the disc and its margin, centred on the view's axis", () => {
    const rect = bodyRect(WIDTH, HEIGHT, FOV_X_RAD, 5000, 1000, 1);
    // tan(asin 0.2) of a focal length of 20 ÷ tan 30° px is 7.07 px, and 1 px of margin.
    expect(rect).toEqual({ x: 11, y: 6, width: 18, height: 18 });
  });

  it("is the whole target where the disc fills it", () => {
    expect(bodyRect(WIDTH, HEIGHT, FOV_X_RAD, 900, 1000, 4)).toEqual(WHOLE);
  });
});

/** A reading of a strip 1 px high, its pixels' cells and luminances given. */
function strip(
  cells: ReadonlyArray<number>,
  luminance: ReadonlyArray<number>,
  x = 0,
): FrameReading {
  return {
    widthPx: 8,
    heightPx: 1,
    rect: { x, y: 0, width: cells.length, height: 1 },
    luminance: Float32Array.from(luminance),
    cells: Int32Array.from(cells),
    bodyPixels: cells.filter((cell) => cell !== OFF_BODY).length,
    bodyMisses: 0,
    surveyPixels: cells.filter((cell) => cell >= 0).length,
  };
}

describe("the comparison of two frames", () => {
  // Pixels 0 and 1 on cell 7 in both; pixel 2 on cell 9, then on 8; pixel 3 off the body, then
  // on cell 9; pixel 4 unsurveyed in both; pixel 5 on cell 11 in both, dark after.
  const before = strip([7, 7, 9, OFF_BODY, UNSURVEYED, 11], [1, 1, 2, 0, 3, 0.5]);
  const after = strip([7, 7, 8, 9, UNSURVEYED, 11], [2, 2, 3, 5, 3, 0]);

  it("takes each cell's mean over the pixels both frames assign to it", () => {
    expect(compareReadings(before, after).texels).toEqual([
      { texel: 7, sharedPixels: 2, meanBefore: 1, meanAfter: 2 },
      { texel: 11, sharedPixels: 1, meanBefore: 0.5, meanAfter: 0 },
    ]);
  });

  it("takes the body's and the survey's means over their own and their shared pixels", () => {
    const { body, survey } = compareReadings(before, after);
    expect([body, survey]).toEqual([
      {
        own: { pixelsBefore: 5, pixelsAfter: 6, meanBefore: 7.5 / 5, meanAfter: 15 / 6 },
        shared: { pixels: 5, meanBefore: 7.5 / 5, meanAfter: 10 / 5 },
      },
      {
        own: { pixelsBefore: 4, pixelsAfter: 5, meanBefore: 4.5 / 4, meanAfter: 12 / 5 },
        shared: { pixels: 3, meanBefore: 2.5 / 3, meanAfter: 4 / 3 },
      },
    ]);
  });

  it("shares pixels by their place in the target, whatever the regions", () => {
    const left = strip([7, 7, 7], [1, 1, 1], 0);
    const right = strip([7, 7, 7], [2, 2, 2], 1);
    expect(compareReadings(left, right).texels).toEqual([
      { texel: 7, sharedPixels: 2, meanBefore: 1, meanAfter: 2 },
    ]);
  });

  it("refuses frames of targets of different sizes", () => {
    expect(() => compareReadings(before, { ...after, widthPx: 9 })).toThrow(/scene target/);
  });
});
