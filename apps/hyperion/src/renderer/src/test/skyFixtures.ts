/**
 * Skies for the sky's tests: a response and its payload, laid out as the server's encoder lays
 * them (`crates/hyperion-server/src/bulk/sky.rs`).
 */

import {
  decodeSkyBand,
  decodeSkyStars,
  type SkyBand,
  type SkyRequest,
  type SkyResponse,
  type SkyStars,
  SKY_STAR_BYTES,
  SKY_TEXEL_BYTES,
} from "@hyperion/protocol";

import { decodeSkyPayload, type SkyDecodeRequest } from "../view/sky/decodePayload";

/** One star to lay out: its direction, distance in ly and V. */
export interface FixtureStar {
  readonly direction: readonly [number, number, number];
  readonly distanceLy: number;
  readonly vMag: number;
}

/** A payload of `stars` and a band of `faceTexels`² faces whose eye limits are `eyeLimitMag`. */
export function skyPayload(
  stars: ReadonlyArray<FixtureStar>,
  faceTexels: number,
  eyeLimitMag: number | null,
): Uint8Array {
  const texels = 6 * faceTexels * faceTexels;
  const bytes = new Uint8Array(stars.length * SKY_STAR_BYTES + texels * SKY_TEXEL_BYTES);
  const view = new DataView(bytes.buffer);
  stars.forEach((star, index) => {
    const at = index * SKY_STAR_BYTES;
    star.direction.forEach((component, axis) => {
      view.setFloat32(at + axis * 4, component, true);
    });
    view.setFloat32(at + 12, star.distanceLy, true);
    view.setInt16(at + 16, Math.round(star.vMag * 1_000), true);
    view.setUint16(at + 18, 21_845, true);
    view.setUint16(at + 20, 21_845, true);
  });
  for (let texel = 0; texel < texels; texel += 1) {
    const at = stars.length * SKY_STAR_BYTES + texel * SKY_TEXEL_BYTES;
    view.setFloat32(at, 2.5e-4, true);
    view.setUint16(at + 4, 21_845, true);
    view.setUint16(at + 6, 21_845, true);
    view.setInt16(at + 8, eyeLimitMag === null ? -32_768 : Math.round(eyeLimitMag * 1_000), true);
    view.setUint16(at + 10, 22_600, true);
  }
  return bytes;
}

/** A sky request at a time, in years after the epoch, for `system`. */
export function skyRequest(years: number, system = "0200080020000000"): SkyRequest {
  return {
    universe: "000000000000002a",
    observer: { cell_ly: [-26_000, 0, 20], offset_m: [0, 0, 0] },
    time: { seconds: Math.round(years * 31_557_600), nanos: 0 },
    eye: { field_factor: 1.4, age_years: 25, pigmentation: 0.5 },
    camera_limit_v: 9.5,
    n_max: 1_000,
    cone: null,
    exclude_system: system,
  };
}

/** The response to `request` whose payload is `payload` with `stars` stars, valid for a year. */
export function skyResponse(
  request: SkyRequest,
  payload: Uint8Array,
  stars: number,
  faceTexels: number,
  chunks = 1,
): SkyResponse {
  const starsBytes = stars * SKY_STAR_BYTES;
  return {
    universe: request.universe,
    time: request.time,
    observer: request.observer,
    valid_until: { seconds: request.time.seconds + 31_557_600, nanos: 0 },
    cut_v: 9.5,
    census: [],
    listed: stars,
    overflow: 0,
    band: { face_texels: faceTexels },
    hosts: [],
    not_modelled: ["feature_members"],
    bulk: { chunks, bytes: payload.byteLength },
    stars_bytes: starsBytes,
    band_bytes: payload.byteLength - starsBytes,
  };
}

/** The decoded stars and band of a payload, through the protocol's decoders. */
export function decodedSky(
  payload: Uint8Array,
  stars: number,
): { readonly stars: SkyStars; readonly band: SkyBand } {
  const starsBytes = stars * SKY_STAR_BYTES;
  const decodedStars = decodeSkyStars(payload.subarray(0, starsBytes));
  const decodedBand = decodeSkyBand(payload.subarray(starsBytes));
  if (!decodedStars.ok || !decodedBand.ok) {
    throw new Error("the fixture payload did not decode");
  }
  return { stars: decodedStars.value, band: decodedBand.value };
}

/**
 * A stand-in for the sky's decode worker, constructed as `Worker` is: it decodes each payload on
 * this thread a microtask later.
 */
export class InThreadSkyWorker {
  readonly #events = new EventTarget();

  addEventListener(type: string, listener: (event: Event) => void): void {
    this.#events.addEventListener(type, { handleEvent: listener });
  }

  postMessage(message: SkyDecodeRequest): void {
    queueMicrotask(() => {
      this.#events.dispatchEvent(new MessageEvent("message", { data: decodeSkyPayload(message) }));
    });
  }

  terminate(): void {}
}
