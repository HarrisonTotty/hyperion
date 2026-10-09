import type { SkyGapDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { STAR_SOURCE, STARS_WITHOUT_POSITION } from "../../displays/view/viewRun";
import { nearestFirstCensus, skyPayload, skyRequest, skyResponse } from "../../test/skyFixtures";
import {
  SKY_EDGE_FIELD_CH,
  SKY_PENDING,
  skyEdgeReading,
  skyLabelValue,
  type SkyLineStanding,
  type SkyLinePlace,
  streamingEdgeLy,
} from "./label";

/** The gaps every reply states until R06.T16.a, and those of a sky near the centre. */
const GAPS: ReadonlyArray<SkyGapDto> = ["white_dwarfs", "feature_members", "centre_members"];

/** A line's notes at a place, with the edge reached or none. */
function at(place: SkyLinePlace, edgeLy: number | null = null): SkyLineStanding {
  return { place, streamingEdgeLy: edgeLy };
}

/** A reply whose census has reached `edgesLy` for C, D and E, final where `null`. */
function replyAt(edgesLy: readonly [number | null, number | null, number | null], final = false) {
  const request = skyRequest(0);
  const payload = skyPayload([], 2, 7.4);
  return {
    ...skyResponse(request, payload, 0, 2),
    census: nearestFirstCensus(edgesLy),
    final,
  };
}

describe("the sky line", () => {
  it("states the eye's limit with its kind", () => {
    expect(skyLabelValue(7.42, "eye", [], at("alone"))).toBe("V 7.4 mag EYE");
  });

  it("states a camera's limit with its kind", () => {
    expect(skyLabelValue(9.96, "camera", [], at("alone"))).toBe("V 10.0 mag CAM");
  });

  it("composes the gaps into one note, in order, the centre's members as clusters once", () => {
    expect(skyLabelValue(9.5, "camera", GAPS, at("alone"))).toBe(
      "V 9.5 mag CAM · CLUSTERS AND WHITE DWARFS: NOT YET MODELLED",
    );
  });

  it("names a single gap alone in the note", () => {
    expect(skyLabelValue(9.5, "camera", ["white_dwarfs"], at("alone"))).toBe(
      "V 9.5 mag CAM · WHITE DWARFS: NOT YET MODELLED",
    );
  });

  it("words the reading before the sky's first reply PENDING, as LIGHTING: PENDING is", () => {
    expect(SKY_PENDING).toBe("PENDING");
  });

  it("leaves R02's interim readings as they were drafted", () => {
    expect(STAR_SOURCE).toBe("RANGE QUERY · VOLUME-LIMITED · NO EXTINCTION");
    expect(STARS_WITHOUT_POSITION).toBe("NOT AVAILABLE: the system's position is not known");
  });
});

describe("the stars-arriving note (R06.T11.f)", () => {
  it.each([
    [500, "BEYOND 500 ly"],
    [1_000, "BEYOND 1000 ly"],
    [2_000, "BEYOND 2000 ly"],
    [4_000, "BEYOND 4000 ly"],
    [8_000, "BEYOND 8000 ly"],
    [16_000, "BEYOND 16,000 ly"],
    [32_000, "BEYOND 32,000 ly"],
    [64_000, "BEYOND 64,000 ly"],
    [128_000, "BEYOND 128,000 ly"],
  ])("states the fixed edge %d ly with the guide's grouping, %s", (edgeLy, edge) => {
    expect(skyLabelValue(7.4, "eye", [], at("alone", edgeLy))).toBe(
      `V 7.4 mag EYE · ${edge}: STREAMING`,
    );
  });

  it("sizes the edge's field for its longest reading with its stale mark, 128,000 ly S", () => {
    expect([skyEdgeReading(2_000), SKY_EDGE_FIELD_CH]).toEqual(["2000 ly", 12]);
  });

  it("goes before what the sky leaves out, with no instrument open", () => {
    expect(skyLabelValue(7.4, "eye", GAPS, at("alone", 2_000))).toBe(
      "V 7.4 mag EYE · BEYOND 2000 ly: STREAMING · CLUSTERS AND WHITE DWARFS: NOT YET MODELLED",
    );
  });

  it("stands alone beside an open instrument whose line shows what the sky leaves out", () => {
    expect([
      skyLabelValue(7.4, "eye", GAPS, at("beside", 16_000)),
      skyLabelValue(7.4, "eye", [], at("beside", 16_000)),
    ]).toEqual([
      "V 7.4 mag EYE · BEYOND 16,000 ly: STREAMING",
      "V 7.4 mag EYE · BEYOND 16,000 ly: STREAMING",
    ]);
  });

  it("yields to what the sky leaves out beside open instruments whose lines do not show it", () => {
    expect([
      skyLabelValue(7.4, "eye", GAPS, at("beside-unshown", 2_000)),
      skyLabelValue(7.4, "eye", [], at("beside-unshown", 2_000)),
    ]).toEqual([
      "V 7.4 mag EYE · CLUSTERS AND WHITE DWARFS: NOT YET MODELLED",
      "V 7.4 mag EYE · BEYOND 2000 ly: STREAMING",
    ]);
  });

  it("never stands on an instrument's line, which keeps its limit and what the sky leaves out", () => {
    expect([
      skyLabelValue(10.0, "camera", GAPS, at("instrument", 2_000)),
      skyLabelValue(10.0, "camera", [], at("instrument", 2_000)),
    ]).toEqual(["V 10.0 mag CAM · CLUSTERS AND WHITE DWARFS: NOT YET MODELLED", "V 10.0 mag CAM"]);
  });

  it("clears on the final reply, wherever the line stands", () => {
    const places: ReadonlyArray<SkyLinePlace> = ["alone", "beside", "beside-unshown", "instrument"];
    expect(places.map((place) => skyLabelValue(7.4, "eye", GAPS, at(place)))).toEqual(
      places.map(() => "V 7.4 mag EYE · CLUSTERS AND WHITE DWARFS: NOT YET MODELLED"),
    );
  });
});

describe("the edge a reply's stars have arrived to", () => {
  it("is the least edge over the layers not yet final, never a final layer's cap", () => {
    expect([
      streamingEdgeLy(replyAt([500, 500, 500])),
      streamingEdgeLy(replyAt([2_000, 4_000, 4_000])),
      streamingEdgeLy(replyAt([null, null, 16_000])),
    ]).toEqual([500, 2_000, 16_000]);
  });

  it("is none for a final reply, or one whose every layer is final", () => {
    expect([
      streamingEdgeLy(replyAt([null, null, null], true)),
      streamingEdgeLy(replyAt([null, null, null])),
    ]).toEqual([null, null]);
  });
});
