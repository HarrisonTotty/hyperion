import { describe, expect, it } from "vitest";

import { decodedSky, skyPayload } from "../../test/skyFixtures";
import { decodeSkyPayload, transferablesOf } from "./decodePayload";

const STARS = [
  { direction: [1, 0, 0], distanceLy: 8.6, vMag: -1.46 },
  { direction: [0, 0.6, 0.8], distanceLy: 25, vMag: 0.03 },
] as const;

describe("the decode worker's work", () => {
  it("decodes chunks to what the main thread's decoders give", () => {
    const payload = skyPayload(STARS, 2, 6.6);
    const reply = decodeSkyPayload({
      id: 7,
      chunks: [payload.slice(0, 30), payload.slice(30)],
      starsBytes: 48,
      bandBytes: payload.byteLength - 48,
    });
    if (!reply.ok) {
      throw new Error(reply.message);
    }
    const direct = decodedSky(payload, 2);
    expect(reply.id).toBe(7);
    expect(reply.stars).toEqual(direct.stars);
    expect(reply.band).toEqual(direct.band);
  });

  it("transfers every typed array of a decoded sky", () => {
    const payload = skyPayload(STARS, 2, 6.6);
    const reply = decodeSkyPayload({
      id: 1,
      chunks: [payload],
      starsBytes: 48,
      bandBytes: payload.byteLength - 48,
    });
    expect(transferablesOf(reply)).toHaveLength(10);
  });

  it("refuses split counts that do not sum to the payload, naming them", () => {
    const reply = decodeSkyPayload({
      id: 2,
      chunks: [new Uint8Array(30)],
      starsBytes: 24,
      bandBytes: 12,
    });
    expect(reply).toEqual({
      id: 2,
      ok: false,
      message: "a sky payload of 30 bytes is not 24 star bytes and 12 band bytes",
    });
    expect(transferablesOf(reply)).toEqual([]);
  });
});
