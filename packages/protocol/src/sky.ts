/**
 * The `sky` request's bulk payload (rendering plan R06, R06.T10; Design note 17): the listed
 * stars, then the band's texels, decoded into typed arrays a worker can transfer.
 *
 * @remarks
 * The layouts are `crates/hyperion-protocol/src/sky.rs`'s, every value little-endian, and the
 * server's encoder (`crates/hyperion-server/src/bulk/sky.rs`) pins them byte for byte. A payload
 * is decoded only once R03's `BulkAssembler` has all of it, and is split by the response's
 * `stars_bytes` and `band_bytes`.
 */

import type { SkyResponse } from "./generated/SkyResponse";

/** One star's bytes in the payload: the protocol crate's `SKY_STAR_BYTES`, restated. */
export const SKY_STAR_BYTES = 24;

/** One band texel's bytes in the payload: the protocol crate's `SKY_TEXEL_BYTES`, restated. */
export const SKY_TEXEL_BYTES = 12;

/** The deepest cut a request may ask, V 11.0: the protocol crate's `MAX_CUT_V`, restated. */
export const MAX_CUT_V = 11;

/** The most stars a request may list, 3 × 10⁵: the protocol crate's `MAX_SKY_STARS`, restated. */
export const MAX_SKY_STARS = 300_000;

/** The `i16` a texel's eye limit takes where the eye was not asked. */
const NO_EYE_LIMIT = -32_768;

/** The listed stars, one entry a star in each array (two or three where a field has parts). */
export interface SkyStars {
  /** The stars decoded. */
  readonly count: number;
  /** Unit directions from the observer, galactic axes, three a star. */
  readonly directions: Float32Array;
  /** Distances, ly. */
  readonly distanceLy: Float32Array;
  /** Apparent V after extinction, mag, to 0.001. */
  readonly vMag: Float32Array;
  /**
   * Chromaticity after reddening, r ÷ (r + g + b) and g ÷ (r + g + b), two a star; b's is one less
   * the two.
   */
  readonly chroma: Float32Array;
  /** The eye's colour offset, mag, to 0.01. */
  readonly eyeOffsetMag: Float32Array;
  /** The view camera's band term, mag, to 0.01. */
  readonly cameraBandMag: Float32Array;
}

/** The band's texels, face after face (+X, −X, +Y, −Y, +Z, −Z), rows from the top. */
export interface SkyBand {
  /** The texels decoded: six faces of the response's `face_texels` squared. */
  readonly count: number;
  /** Luminance, cd/m². */
  readonly luminanceCdM2: Float32Array;
  /** Chromaticity, as a star's, two a texel. */
  readonly chroma: Float32Array;
  /** The eye's limit at the request's field factor, mag, to 0.001; NaN where the eye was not asked. */
  readonly eyeLimitMag: Float32Array;
  /** The scotopic-to-photopic ratio ρ, to 10⁻⁴. */
  readonly spRatio: Float32Array;
}

/** A decode's result, or why the bytes were refused. */
export type SkyDecoded<T> =
  { readonly ok: true; readonly value: T } | { readonly ok: false; readonly message: string };

/** The stars' and the band's bytes of a complete payload, as the response splits it. */
export interface SkyPayloadParts {
  /** The stars' bytes, for {@link decodeSkyStars}. */
  readonly stars: Uint8Array;
  /** The band's bytes, for {@link decodeSkyBand}. */
  readonly band: Uint8Array;
}

/**
 * Splits a complete payload into its stars and its band by the response's byte counts.
 *
 * @returns The two views, or a refusal when the counts do not sum to the payload's length.
 */
export function splitSkyPayload(
  payload: Uint8Array,
  response: Pick<SkyResponse, "stars_bytes" | "band_bytes">,
): SkyDecoded<SkyPayloadParts> {
  const counts = [response.stars_bytes, response.band_bytes];
  if (
    !counts.every((bytes) => Number.isSafeInteger(bytes) && bytes >= 0) ||
    response.stars_bytes + response.band_bytes !== payload.byteLength
  ) {
    return {
      ok: false,
      message: `a sky payload of ${payload.byteLength} bytes is not ${response.stars_bytes} star bytes and ${response.band_bytes} band bytes`,
    };
  }
  return {
    ok: true,
    value: {
      stars: payload.subarray(0, response.stars_bytes),
      band: payload.subarray(response.stars_bytes),
    },
  };
}

/** A refusal for bytes that are not a whole number of records. */
function notWhole(
  what: string,
  bytes: Uint8Array,
  recordBytes: number,
): { readonly ok: false; readonly message: string } {
  return {
    ok: false,
    message: `${bytes.byteLength} bytes of sky ${what} are not a whole number of ${recordBytes}-byte records`,
  };
}

/** Decodes the payload's stars. */
export function decodeSkyStars(bytes: Uint8Array): SkyDecoded<SkyStars> {
  if (bytes.byteLength % SKY_STAR_BYTES !== 0) {
    return notWhole("stars", bytes, SKY_STAR_BYTES);
  }
  const count = bytes.byteLength / SKY_STAR_BYTES;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const stars = {
    count,
    directions: new Float32Array(count * 3),
    distanceLy: new Float32Array(count),
    vMag: new Float32Array(count),
    chroma: new Float32Array(count * 2),
    eyeOffsetMag: new Float32Array(count),
    cameraBandMag: new Float32Array(count),
  };
  for (let star = 0; star < count; star += 1) {
    const at = star * SKY_STAR_BYTES;
    stars.directions[star * 3] = view.getFloat32(at, true);
    stars.directions[star * 3 + 1] = view.getFloat32(at + 4, true);
    stars.directions[star * 3 + 2] = view.getFloat32(at + 8, true);
    stars.distanceLy[star] = view.getFloat32(at + 12, true);
    stars.vMag[star] = view.getInt16(at + 16, true) / 1_000;
    stars.chroma[star * 2] = view.getUint16(at + 18, true) / 65_535;
    stars.chroma[star * 2 + 1] = view.getUint16(at + 20, true) / 65_535;
    stars.eyeOffsetMag[star] = view.getInt8(at + 22) / 100;
    stars.cameraBandMag[star] = view.getInt8(at + 23) / 100;
  }
  return { ok: true, value: stars };
}

/** Decodes the payload's band. */
export function decodeSkyBand(bytes: Uint8Array): SkyDecoded<SkyBand> {
  if (bytes.byteLength % SKY_TEXEL_BYTES !== 0) {
    return notWhole("band", bytes, SKY_TEXEL_BYTES);
  }
  const count = bytes.byteLength / SKY_TEXEL_BYTES;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const band = {
    count,
    luminanceCdM2: new Float32Array(count),
    chroma: new Float32Array(count * 2),
    eyeLimitMag: new Float32Array(count),
    spRatio: new Float32Array(count),
  };
  for (let texel = 0; texel < count; texel += 1) {
    const at = texel * SKY_TEXEL_BYTES;
    band.luminanceCdM2[texel] = view.getFloat32(at, true);
    band.chroma[texel * 2] = view.getUint16(at + 4, true) / 65_535;
    band.chroma[texel * 2 + 1] = view.getUint16(at + 6, true) / 65_535;
    const limit = view.getInt16(at + 8, true);
    band.eyeLimitMag[texel] = limit === NO_EYE_LIMIT ? Number.NaN : limit / 1_000;
    band.spRatio[texel] = view.getUint16(at + 10, true) / 10_000;
  }
  return { ok: true, value: band };
}
