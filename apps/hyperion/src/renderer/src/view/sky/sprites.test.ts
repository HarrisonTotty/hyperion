import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { decodedSky, skyPayload } from "../../test/skyFixtures";
import { pixelSolidAngle, project } from "../camera/projection";
import { illuminanceLx, pixelLuminance, psfPixelWeights } from "../photometry/magnitude";
import { selectSkySprites } from "./select";
import { starIlluminanceRgbLx } from "./photometry";
import { skySpriteStars } from "./sprites";

const AU_M = 149_597_870_700;

/** A 1080p, 60° camera looking along −z, as R02's projection takes it. */
const CAMERA = { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 };
const VIEWPORT = { widthPx: 1_920, heightPx: 1_080 };

/** Stars along −z at the given distances, brightest first. */
function skyOf(distancesLy: ReadonlyArray<number>) {
  const stars = distancesLy.map((distanceLy, i) => ({
    direction: [0, 0, -1] as const,
    distanceLy,
    vMag: 1 + i,
  }));
  return decodedSky(skyPayload(stars, 2, null), stars.length).stars;
}

describe("selectSkySprites", () => {
  it("makes the budget's brightest sprites and bakes the rest", () => {
    const stars = skyOf([100, 200, 300, 400]);
    const selection = selectSkySprites(stars, Uint32Array.from([0, 1, 2, 3]), 2, {
      fovDeg: 60,
      widthPx: 1_920,
    });
    expect([...selection.sprites]).toEqual([0, 1]);
    expect([...selection.baked]).toEqual([2, 3]);
  });

  it("makes a near star a sprite whatever the budget, for its parallax", () => {
    const stars = skyOf([100, 200, 0.5]);
    const selection = selectSkySprites(stars, Uint32Array.from([0, 1, 2]), 1, {
      fovDeg: 60,
      widthPx: 1_920,
    });
    expect([...selection.sprites]).toEqual([0, 2]);
    expect([...selection.baked]).toEqual([1]);
  });
});

describe("skySpriteStars", () => {
  it("moves a star 0.1 ly away by about nine pixels across 30 au at 1080p and 60°", () => {
    const stars = skyOf([0.1]);
    const before = skySpriteStars(stars, Uint32Array.from([0]), vec3(0, 0, 0))[0];
    const after = skySpriteStars(stars, Uint32Array.from([0]), vec3(30 * AU_M, 0, 0))[0];
    if (before === undefined || after === undefined) {
      throw new Error("no sprite was made");
    }
    const x0 = project(before.direction, CAMERA, VIEWPORT).xPx;
    const x1 = project(after.direction, CAMERA, VIEWPORT).xPx;
    expect(Math.abs(x1 - x0)).toBeGreaterThan(7.5);
    expect(Math.abs(x1 - x0)).toBeLessThan(9.5);
  });

  it("gives a sprite the light a baked star of the same V and colour takes", () => {
    const stars = skyOf([100]);
    const sprite = skySpriteStars(stars, Uint32Array.from([0]), vec3(0, 0, 0))[0];
    expect(sprite?.illuminanceRgbLx).toEqual(
      starIlluminanceRgbLx(stars.vMag[0] ?? 0, stars.chroma[0] ?? 0, stars.chroma[1] ?? 0),
    );
  });

  it("keeps a moving star's summed energy within 1% as it crosses a pixel", () => {
    const e = illuminanceLx(2);
    const omega = pixelSolidAngle(vec3(0, 0, -1), CAMERA, VIEWPORT);
    for (let step = 0; step <= 10; step += 1) {
      const at = { xPx: -0.5 + step / 10, yPx: 0.25 - step / 40 };
      const energy = psfPixelWeights(at).reduce(
        (total, weight) => total + pixelLuminance(e, weight, omega) * omega,
        0,
      );
      expect(Math.abs(energy / e - 1)).toBeLessThan(0.01);
    }
  });
});
