import type { SkyGapDto, SkyLayerCensusDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { STAR_SOURCE, STARS_WITHOUT_POSITION } from "../../displays/view/viewRun";
import { autoAt, manualAt } from "../../test/exposureFixtures";
import {
  decodedSky,
  nearestFirstCensus,
  skyPayload,
  skyRequest,
  skyResponse,
} from "../../test/skyFixtures";
import { FOV_STEPS_DEG } from "../camera/projection";
import {
  detailLimited,
  SKY_DETAIL_LIMITED_NOTE,
  SKY_EDGE_FIELD_CH,
  SKY_LIMIT_FIELD_CH,
  SKY_PENDING,
  skyEdgeReading,
  skyLabelValue,
  skyLimitReading,
  type SkyLineStanding,
  type SkyLinePlace,
  streamingEdgeLy,
} from "./label";
import type { SkyModel } from "./model";
import { viewSkyLabelV, viewSkyRequest } from "./viewSky";

/** The gaps every reply states until R06.T16.a, and those of a sky near the centre. */
const GAPS: ReadonlyArray<SkyGapDto> = ["white_dwarfs", "feature_members", "centre_members"];

/** A line's notes at a place, with the edge reached or none, and the sky held at a ceiling or not. */
function at(place: SkyLinePlace, edgeLy: number | null = null, atCeiling = false): SkyLineStanding {
  return { place, streamingEdgeLy: edgeLy, detailLimited: atCeiling };
}

/** A `STARS` reading of `parts`, each after a middle dot. */
function line(...parts: ReadonlyArray<string>): string {
  return parts.join(" · ");
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

/** A camera's held sky asked at `fovDeg`, its band uniform at `backgroundCdM2`. */
function cameraSkyOver(backgroundCdM2: number, fovDeg: number): SkyModel {
  const payload = skyPayload([], 2, null, () => backgroundCdM2);
  const request = viewSkyRequest({
    universe: "000000000000002a",
    observer: skyRequest(0).observer,
    system: "0200080020000000",
    time: { seconds: 0, nanos: 0 },
    role: "camera",
    fovDeg,
    nMax: 300_000,
  });
  return {
    request,
    response: skyResponse(request, payload, 0, 2),
    ...decodedSky(payload, 0),
    stale: false,
  };
}

/** A sky of surface brightness μ, mag arcsec⁻², as a luminance: B = 10^((12.58 − μ) ÷ 2.5). */
function skyOf(mu: number): number {
  return 10 ** ((12.58 - mu) / 2.5);
}

describe("the limit's field (R06.T13.j)", () => {
  it("holds the limit before its kind, to one decimal in mag", () => {
    expect([skyLimitReading(9.46), skyLimitReading(11), skyLimitReading(-20.2)]).toEqual([
      "V 9.5 mag",
      "V 11.0 mag",
      "V -20.2 mag",
    ]);
  });

  it("composes the line from the limit as its field holds it", () => {
    expect(skyLabelValue(10.12, "camera", [], at("alone"))).toBe(`${skyLimitReading(10.12)} CAM`);
  });

  it("pins the field at 13ch, its widest text V -20.2 mag S: a camera at 120° and EV100 42, the darkest exposure MAN takes", () => {
    const widest = `${skyLimitReading(viewSkyLabelV(cameraSkyOver(0, 120), "camera", manualAt(42), 120))} S`;
    expect([SKY_LIMIT_FIELD_CH, widest, widest.length]).toEqual([13, "V -20.2 mag S", 13]);
  });

  it("is wider than the deepest cut's reading with its mark, V 11.0 mag S", () => {
    expect(`${skyLimitReading(11)} S`.length).toBeLessThan(SKY_LIMIT_FIELD_CH);
  });

  it("holds every limit a camera states with its mark: EV100 -14 to 60, every field, a black sky to daylight", () => {
    // Past EV100 42 only AUTO goes, its ND pegged; the darkest texel of a band is far fainter than
    // a daylit sky's 10,000 cd/m².
    const backgrounds = [0, skyOf(30), skyOf(24), skyOf(21.6), skyOf(15), 1, 100, 10_000];
    let widest = "";
    for (const fovDeg of FOV_STEPS_DEG) {
      for (const backgroundCdM2 of backgrounds) {
        const sky = cameraSkyOver(backgroundCdM2, fovDeg);
        for (let ev100 = -14; ev100 <= 60; ev100 += 0.5) {
          const reading = `${skyLimitReading(viewSkyLabelV(sky, "camera", autoAt(ev100), fovDeg))} S`;
          widest = reading.length > widest.length ? reading : widest;
        }
      }
    }
    expect(widest.length).toBeLessThanOrEqual(SKY_LIMIT_FIELD_CH);
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

describe("the interim note (R13.T2.b; decision-r13-t2b-note)", () => {
  const L = "V 7.4 mag EYE";
  const S = "BEYOND 1000 ly: STREAMING";
  const I = "DISTANT STARS: DETAIL LIMITED";
  const N = "CLUSTERS AND WHITE DWARFS: NOT YET MODELLED";

  it("reads DISTANT STARS: DETAIL LIMITED, with no figure", () => {
    expect(SKY_DETAIL_LIMITED_NOTE).toBe(I);
  });

  it.each<[string, SkyLinePlace, number | null, string]>([
    ["not final, no instrument open", "alone", 1_000, line(L, S, I, N)],
    ["final, no instrument open", "alone", null, line(L, I, N)],
    ["not final, beside an instrument", "beside", 1_000, line(L, S)],
    ["final, beside an instrument", "beside", null, line(L, I)],
    ["not final, an instrument's", "instrument", 1_000, line(L, N)],
    ["final, an instrument's", "instrument", null, line(L, N)],
    ["not final, beside instruments not yet showing the sky", "beside-unshown", 1_000, line(L, N)],
    ["final, beside instruments not yet showing the sky", "beside-unshown", null, line(L, N)],
  ])("composes by the ruled table: %s", (_, place, edgeLy, expected) => {
    expect(skyLabelValue(7.4, "eye", GAPS, at(place, edgeLy, true))).toBe(expected);
  });

  it("stands as the one annunciation where nothing is left out, but never on an instrument's line", () => {
    expect([
      skyLabelValue(7.4, "eye", [], at("alone", null, true)),
      skyLabelValue(7.4, "eye", [], at("beside", null, true)),
      skyLabelValue(7.4, "eye", [], at("beside-unshown", null, true)),
      skyLabelValue(7.4, "eye", [], at("beside-unshown", 1_000, true)),
      skyLabelValue(10.0, "camera", [], at("instrument", null, true)),
    ]).toEqual([line(L, I), line(L, I), line(L, I), line(L, S), "V 10.0 mag CAM"]);
  });

  it("is absent from a sky held at no ceiling, wherever the line stands", () => {
    const places: ReadonlyArray<SkyLinePlace> = ["alone", "beside", "beside-unshown", "instrument"];
    expect(places.map((place) => skyLabelValue(7.4, "eye", GAPS, at(place, null, false)))).toEqual(
      places.map(() => line(L, N)),
    );
  });

  it("holds while a reply states a ceiling, at RM3's V 5.0 or at its own cut", () => {
    const reply = replyAt([null, null, null], true);
    expect([
      detailLimited({ ...reply, synthetic_ceiling_v: 5.0, real_limit_ly: 2_000 }),
      detailLimited({ ...reply, synthetic_ceiling_v: reply.cut_v, real_limit_ly: 2_000 }),
      detailLimited({ ...reply, synthetic_ceiling_v: 5.0 }),
    ]).toEqual([true, true, true]);
  });

  it("does not hold where the reply states no ceiling, the limit and counts beyond absent too", () => {
    const reply = replyAt([null, null, null], true);
    expect(reply.synthetic_ceiling_v).toBeUndefined();
    expect([reply.real_limit_ly, reply.census.map((layer) => layer.bright_beyond)]).toEqual([
      undefined,
      reply.census.map(() => undefined),
    ]);
    const counted: SkyLayerCensusDto[] = [];
    for (const layer of reply.census) {
      counted.push({ ...layer, bright_beyond: 172 });
    }
    expect([
      detailLimited(reply),
      detailLimited({ ...reply, real_limit_ly: 2_000 }),
      detailLimited({ ...reply, census: counted }),
    ]).toEqual([false, false, false]);
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
