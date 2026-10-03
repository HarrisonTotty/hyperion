import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { decodedSky, skyPayload } from "../../test/skyFixtures";
import { texelSolidAnglesSr } from "./cube";
import { bandTexels, BandLayer } from "./band";
import { cullSky } from "./cull";
import { fromHalfBits, HALF_MAX, toHalfBits } from "./half";

const FACE = 4;

/** A band of `FACE`² faces whose texels all hold 2.5 × 10⁻⁴ cd/m² of white. */
function band() {
  return decodedSky(skyPayload([], FACE, 6.6), 0).band;
}

describe("bandTexels", () => {
  it("lays out the band face after face with its luminance in its colour", () => {
    const texels = bandTexels(band(), FACE, new Float64Array(6 * FACE * FACE * 3));
    expect(texels.length).toBe(6 * FACE * FACE * 4);
    // The fixture's chromaticity is a third each: white of unit luminance.
    expect(texels[0]).toBeCloseTo(2.5e-4, 9);
    expect(texels[1]).toBeCloseTo(2.5e-4, 9);
    expect(texels[3]).toBe(1);
  });

  it("adds the culled stars' light, the band layer's flux equal to theirs to 10⁻⁶", () => {
    const stars = decodedSky(
      skyPayload(
        [
          { direction: [1, 0.2, -0.3], distanceLy: 100, vMag: 8 },
          { direction: [-0.1, 0.4, 1], distanceLy: 300, vMag: 9 },
        ],
        FACE,
        null,
      ),
      2,
    ).stars;
    const culled = cullSky(stars, { kind: "camera", limitV: 5 }, FACE);
    const empty = { ...band(), luminanceCdM2: new Float32Array(6 * FACE * FACE) };
    const texels = bandTexels(empty, FACE, culled.bandIlluminanceLx);
    const omegas = texelSolidAnglesSr(FACE);
    let flux = 0;
    let expected = 0;
    for (let texel = 0; texel < 6 * FACE * FACE; texel += 1) {
      flux += (texels[texel * 4 + 1] ?? 0) * (omegas[texel % (FACE * FACE)] ?? 0);
      expected += culled.bandIlluminanceLx[texel * 3 + 1] ?? 0;
    }
    expect(culled.culledCount).toBe(2);
    expect(Math.abs(flux / expected - 1)).toBeLessThan(1e-6);
  });

  it("refuses a band that does not cover six faces", () => {
    expect(() => bandTexels(band(), FACE + 1, new Float64Array(0))).toThrow(/six faces/);
  });
});

describe("BandLayer", () => {
  it("uploads the band as a six-face rgba16float cube and draws it with the frame's exposure", async () => {
    const engine = await countingRenderEngine();
    const layer = new BandLayer(engine);
    expect(layer.draw(1)).toBeNull();
    layer.update(band(), FACE, new Float64Array(6 * FACE * FACE * 3));
    expect(engine.textureSpecs.at(-1)).toMatchObject({
      dimension: "cube",
      format: "rgba16float",
      size: { width: FACE, height: FACE, depthOrArrayLayers: 6 },
    });
    expect(engine.counts.textureWrites).toBe(1);
    const draw = layer.draw(0.5);
    expect([...(draw?.uniforms["exposure"] ?? [])]).toEqual([0.5, 0, 0, 0]);
    expect(draw?.textures["band"]?.name).toBe("sky band");
  });
});

describe("half floats", () => {
  it("round to nearest, keep the largest half and its smallest subnormal", () => {
    expect(fromHalfBits(toHalfBits(1))).toBe(1);
    expect(fromHalfBits(toHalfBits(HALF_MAX))).toBe(HALF_MAX);
    expect(fromHalfBits(toHalfBits(2 ** -24))).toBe(2 ** -24);
    expect(toHalfBits(2 ** -26)).toBe(0);
    expect(fromHalfBits(toHalfBits(70_000))).toBe(Number.POSITIVE_INFINITY);
    expect(Math.abs(fromHalfBits(toHalfBits(2.5e-4)) / 2.5e-4 - 1)).toBeLessThan(2 ** -11);
    expect(fromHalfBits(toHalfBits(Number.NaN))).toBeNaN();
  });
});
