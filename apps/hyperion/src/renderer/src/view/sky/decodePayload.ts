/**
 * What the sky's decode worker does, as a pure function of the chunks it is handed (plan R06,
 * T12; R03 Design note 11).
 *
 * @remarks
 * The worker (`decode.worker.ts`) holds no logic of its own: it hands each request here and posts
 * the reply back with its typed arrays transferred, so the render thread never walks the 7 MB
 * payload. The chunks are joined, split by the response's `stars_bytes` and `band_bytes`, and
 * decoded by `@hyperion/protocol`'s decoders, the only place wire bytes are read.
 */

import {
  decodeSkyBand,
  decodeSkyStars,
  type SkyBand,
  type SkyStars,
  splitSkyPayload,
} from "@hyperion/protocol";

/** A message the render thread sends the decode worker. */
export interface SkyDecodeRequest {
  /** The request's own number, which the reply carries back. */
  readonly id: number;
  /** The payload's chunks in order, as R03's `requestBulk` gave them; transferred. */
  readonly chunks: ReadonlyArray<Uint8Array>;
  readonly starsBytes: number;
  readonly bandBytes: number;
}

/** The decode worker's answer: the stars and the band, or why the payload was refused. */
export type SkyDecodeReply =
  | { readonly id: number; readonly ok: true; readonly stars: SkyStars; readonly band: SkyBand }
  | { readonly id: number; readonly ok: false; readonly message: string };

/** The chunks joined into one payload, without a copy when there is one chunk. */
function joined(chunks: ReadonlyArray<Uint8Array>): Uint8Array {
  if (chunks.length === 1 && chunks[0] !== undefined) {
    return chunks[0];
  }
  const payload = new Uint8Array(chunks.reduce((total, chunk) => total + chunk.byteLength, 0));
  let offset = 0;
  for (const chunk of chunks) {
    payload.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return payload;
}

/** Decodes one payload. */
export function decodeSkyPayload(request: SkyDecodeRequest): SkyDecodeReply {
  const split = splitSkyPayload(joined(request.chunks), {
    stars_bytes: request.starsBytes,
    band_bytes: request.bandBytes,
  });
  if (!split.ok) {
    return { id: request.id, ok: false, message: split.message };
  }
  const stars = decodeSkyStars(split.value.stars);
  if (!stars.ok) {
    return { id: request.id, ok: false, message: stars.message };
  }
  const band = decodeSkyBand(split.value.band);
  if (!band.ok) {
    return { id: request.id, ok: false, message: band.message };
  }
  return { id: request.id, ok: true, stars: stars.value, band: band.value };
}

/** The buffers of a reply to transfer back, so that nothing is copied. */
export function transferablesOf(reply: SkyDecodeReply): ArrayBuffer[] {
  if (!reply.ok) {
    return [];
  }
  const { stars, band } = reply;
  return [
    stars.directions,
    stars.distanceLy,
    stars.vMag,
    stars.chroma,
    stars.eyeOffsetMag,
    stars.cameraBandMag,
    band.luminanceCdM2,
    band.chroma,
    band.eyeLimitMag,
    band.spRatio,
  ].flatMap((array) => (array.buffer instanceof ArrayBuffer ? [array.buffer] : []));
}
