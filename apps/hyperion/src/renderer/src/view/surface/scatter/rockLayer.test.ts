import { describe, expect, it } from "vitest";

import { vec3 } from "../../../geometry/vec3";
import {
  ROCK_WORDS,
  rockAt,
  rockLayerBytes,
  rockLayerOf,
  rockOffsetsF32,
  rocksOf,
} from "./rockLayer";

/** Two rocks' words: rock r's word k is 100 r + k, its base centre far from the origin. */
function twoRocks(): Float64Array {
  return Float64Array.from({ length: 2 * ROCK_WORDS }, (_, n) => {
    const [r, k] = [Math.floor(n / ROCK_WORDS), n % ROCK_WORDS];
    return k < 3 ? 6_371_000.25 + 100 * r + k : 100 * r + k;
  });
}

describe("a patch's rock layer (R11.T4.a)", () => {
  it("unpacks each rock's place, frame, shape, ground and axis in the crate's word order", () => {
    const layer = rockLayerOf(twoRocks());
    expect(layer.count).toBe(2);
    const rock = rockAt(layer, 1);
    expect(rock.baseCentreM).toEqual(vec3(6_371_100.25, 6_371_101.25, 6_371_102.25));
    expect(rock.axes).toEqual([vec3(103, 104, 105), vec3(106, 107, 108), vec3(109, 110, 111)]);
    expect(rock.semiAxesM).toEqual([112, 113, 114]);
    expect([rock.exponent, rock.groundHeightM]).toEqual([115, 116]);
    expect(rock.dir).toEqual(vec3(117, 118, 119));
    expect(rocksOf(layer)).toEqual([rockAt(layer, 0), rock]);
    expect(rockLayerBytes(layer)).toBe(2 * ROCK_WORDS * 8);
  });

  it("places each rock about the patch's origin, the difference taken in f64 and narrowed once", () => {
    const layer = rockLayerOf(twoRocks());
    const origin = vec3(6_371_000, 6_371_000, 6_371_000);
    expect(Array.from(rockOffsetsF32(layer, origin))).toEqual(
      [0.25, 1.25, 2.25, 100.25, 101.25, 102.25].map(Math.fround),
    );
  });

  it("refuses words that are not a whole number of rocks, and a rock it does not hold", () => {
    expect(() => rockLayerOf(new Float64Array(ROCK_WORDS + 1))).toThrow(RangeError);
    const layer = rockLayerOf(twoRocks());
    expect(() => rockAt(layer, 2)).toThrow(RangeError);
    expect(() => rockAt(layer, 0.5)).toThrow(RangeError);
  });
});
