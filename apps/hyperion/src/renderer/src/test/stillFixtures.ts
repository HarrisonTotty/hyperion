import type { StillLabel } from "../../../preload/api";
import { type StillRequest, stillLabel } from "../view/stills/stillLabel";

/**
 * A still's request as a view holds it (plan R11, R11.T10.c): every field of Design note 17's
 * label, from a camera in a body's frame over one surveyed body, at the high setting with
 * decoration drawn and one annunciation holding.
 */
export const TEST_STILL_REQUEST: StillRequest = {
  time: { seconds: 412_345_678, nanos: 250_000_000 },
  camera: {
    frame: { kind: "body", body: "0200080020000000.0100" },
    positionM: [6_771_000, -12_500.25, 340.5],
    orientation: { w: 0.5, x: 0.5, y: -0.5, z: 0.5 },
    fovXDeg: 60,
  },
  exposure: { kind: "auto", ev100: 12.4 },
  readings: [
    { label: "FRAME", value: "BODY TEST EARTH" },
    { label: "TIME", value: "UT +13 yr 024/06:27:58" },
    { label: "CAMERA", value: "ORBIT" },
    { label: "FOV", value: "60°" },
    { label: "EXPOSURE", value: "EV100 12.4 AUTO" },
  ],
  setting: "high",
  decoration: true,
  survey: [
    {
      body: "0200080020000000.0100",
      name: "TEST EARTH",
      level: 7,
      surveyedFraction: 0.34,
      bestResolutionM: 1200,
    },
  ],
  annunciations: ["ATMOSPHERE: APPROXIMATE"],
  size: { widthPx: 16, heightPx: 8 },
};

/** The label of {@link TEST_STILL_REQUEST}'s still, its frustum streamed. */
export function testStillLabel(request: Partial<StillRequest> = {}, streamed = true): StillLabel {
  return stillLabel({ ...TEST_STILL_REQUEST, ...request }, streamed);
}
