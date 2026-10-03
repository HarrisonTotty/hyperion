import { describe, expect, it } from "vitest";

import {
  decodeSkyBand,
  decodeSkyStars,
  SKY_STAR_BYTES,
  SKY_TEXEL_BYTES,
  splitSkyPayload,
} from "./sky";

/** `crates/hyperion-server/src/bulk/sky.rs`'s `PINNED_STAR`, the bytes its encoder writes. */
const PINNED_STAR = Uint8Array.from([
  0x9a, 0x99, 0x19, 0x3f, 0xcd, 0xcc, 0x4c, 0xbf, 0x00, 0x00, 0x00, 0x00, 0x9a, 0x99, 0x09, 0x41,
  0x4c, 0xfa, 0x00, 0x40, 0x66, 0x66, 0x0c, 0xf8,
]);

/** The same file's `PINNED_TEXELS`: one texel with the eye's limit, one without. */
const PINNED_TEXELS = Uint8Array.from([
  0x6f, 0x12, 0x83, 0x39, 0xeb, 0x51, 0x7b, 0x54, 0xc8, 0x19, 0x48, 0x58, 0xac, 0xc5, 0x27, 0x37,
  0x00, 0x00, 0xff, 0xff, 0x00, 0x80, 0x00, 0x00,
]);

describe("decodeSkyStars", () => {
  it("decodes the server's pinned star to its values", () => {
    const decoded = decodeSkyStars(PINNED_STAR);
    if (!decoded.ok) {
      throw new Error(decoded.message);
    }
    const stars = decoded.value;
    expect(stars.count).toBe(1);
    expect([...stars.directions]).toEqual([Math.fround(0.6), Math.fround(-0.8), 0]);
    expect(stars.distanceLy[0]).toBe(Math.fround(8.6));
    expect(stars.vMag[0]).toBe(Math.fround(-1.46));
    expect([...stars.chroma]).toEqual([Math.fround(16_384 / 65_535), Math.fround(26_214 / 65_535)]);
    expect(stars.eyeOffsetMag[0]).toBe(Math.fround(0.12));
    expect(stars.cameraBandMag[0]).toBe(-0.25);
  });

  it("decodes a camera band term in thirty-seconds of a magnitude, to the full i8 range", () => {
    const bytes = new Uint8Array(PINNED_STAR);
    bytes[23] = 0x9d; // −99
    const low = decodeSkyStars(bytes);
    bytes[23] = 0x80; // −128, where the server saturates −4.5
    const floor = decodeSkyStars(bytes);
    if (!low.ok || !floor.ok) {
      throw new Error("the stars did not decode");
    }
    expect(low.value.cameraBandMag[0]).toBe(-3.093_75);
    expect(floor.value.cameraBandMag[0]).toBe(-4);
  });

  it("refuses a truncated payload, naming its length", () => {
    const decoded = decodeSkyStars(PINNED_STAR.subarray(0, 23));
    expect(decoded).toEqual({
      ok: false,
      message: `23 bytes of sky stars are not a whole number of ${SKY_STAR_BYTES}-byte records`,
    });
  });
});

describe("decodeSkyBand", () => {
  it("decodes the server's pinned texels, NaN where the eye was not asked", () => {
    const decoded = decodeSkyBand(PINNED_TEXELS);
    if (!decoded.ok) {
      throw new Error(decoded.message);
    }
    const band = decoded.value;
    expect(band.count).toBe(2);
    expect([...band.luminanceCdM2]).toEqual([Math.fround(2.5e-4), Math.fround(1e-5)]);
    expect([...band.chroma]).toEqual([
      Math.fround(20_971 / 65_535),
      Math.fround(21_627 / 65_535),
      0,
      1,
    ]);
    expect(band.eyeLimitMag[0]).toBe(Math.fround(6.6));
    expect(band.eyeLimitMag[1]).toBeNaN();
    expect([...band.spRatio]).toEqual([Math.fround(2.26), 0]);
  });

  it("refuses a truncated payload, naming its length", () => {
    const decoded = decodeSkyBand(PINNED_TEXELS.subarray(0, 13));
    expect(decoded).toEqual({
      ok: false,
      message: `13 bytes of sky band are not a whole number of ${SKY_TEXEL_BYTES}-byte records`,
    });
  });
});

describe("splitSkyPayload", () => {
  it("splits the stars from the band by the response's byte counts", () => {
    const payload = new Uint8Array([...PINNED_STAR, ...PINNED_TEXELS]);
    const split = splitSkyPayload(payload, { stars_bytes: 24, band_bytes: 24 });
    if (!split.ok) {
      throw new Error(split.message);
    }
    expect([...split.value.stars]).toEqual([...PINNED_STAR]);
    expect([...split.value.band]).toEqual([...PINNED_TEXELS]);
  });

  it("refuses counts that do not sum to the payload", () => {
    expect(splitSkyPayload(new Uint8Array(48), { stars_bytes: 24, band_bytes: 12 })).toEqual({
      ok: false,
      message: "a sky payload of 48 bytes is not 24 star bytes and 12 band bytes",
    });
    expect(splitSkyPayload(new Uint8Array(48), { stars_bytes: -12, band_bytes: 60 }).ok).toBe(
      false,
    );
  });
});
