import { describe, expect, it } from "vitest";

import {
  aCoreEntry,
  aGlobalFeatures,
  aStreamEntry,
  aStreamTrack,
  atLy,
} from "../../test/globalListFixtures";
import { UNIVERSE_ID } from "../../test/galaxyFixtures";
import {
  toGlobalFeaturesRequest,
  toGlobalList,
  toStreamTrack,
  toStreamTrackRequest,
} from "./globalWire";

describe("toGlobalList (plan 10, P10.T11)", () => {
  it("reads the streams and cores by number, leaving out the centre, and the progenitors", () => {
    const list = toGlobalList(
      aGlobalFeatures({
        entries: [
          aStreamEntry({ number: 0, track: "ready", progenitor: "e842000280000000" }),
          aStreamEntry({ number: 1, mass_msun: 9_000 }),
          aCoreEntry(),
        ],
      }),
    );

    expect(list.streams).toEqual([
      { number: 0, massMsun: 12_500, progenitor: "e842000280000000", trackReady: true },
      { number: 1, massMsun: 9_000, progenitor: null, trackReady: false },
    ]);
    expect(list.cores).toEqual([
      {
        number: 0,
        massMsun: 2e6,
        positionLy: { x: -30_000, y: 12_000, z: 8_000 },
        extentLy: 3_375,
      },
    ]);
    expect(list.progenitors.map((progenitor) => [progenitor.kind, progenitor.stream])).toEqual([
      ["dominant_merger", null],
      ["recent", 1],
    ]);
  });

  it("reads a core from a server that does not send its place or extent as having none", () => {
    const { position, extent_ly, ...earlier } = aCoreEntry();
    expect([position, extent_ly]).not.toContain(undefined);

    const list = toGlobalList(aGlobalFeatures({ entries: [earlier] }));

    expect(list.cores[0]).toMatchObject({ positionLy: null, extentLy: null });
  });
});

describe("toStreamTrack (plan 10, P10.T11)", () => {
  it("reads a ready track's arms and length, each point in light-years from the centre", () => {
    const answer = toStreamTrack(aStreamTrack(4, [[100, 200, 300]], [[400, 500, 600]]));

    expect(answer.kind === "ready" ? answer.track : null).toEqual({
      number: 4,
      lengthLy: 100_000,
      trailing: [
        {
          positionLy: { x: 100, y: 200, z: 300 },
          normalWidthLy: 150,
          binormalWidthLy: 250,
          speedKmS: 210,
          dispersionKmS: 0.75,
        },
      ],
      leading: [expect.objectContaining({ positionLy: { x: 400, y: 500, z: 600 } })],
    });
  });

  it("reads a pending track as pending, and an earlier server's as having no length or dispersion", () => {
    expect(
      toStreamTrack({
        universe: UNIVERSE_ID,
        stream: 4,
        track: { status: "pending" },
      }),
    ).toEqual({ kind: "pending" });
    const earlier = toStreamTrack({
      universe: UNIVERSE_ID,
      stream: 4,
      track: {
        status: "ready",
        trailing: [
          { position: atLy(1, 2, 3), normal_width_ly: 1, binormal_width_ly: 2, speed_km_s: 3 },
        ],
        leading: [],
      },
    });

    const track = earlier.kind === "ready" ? earlier.track : null;
    expect(track?.lengthLy).toBeNull();
    expect(track?.trailing[0]?.dispersionKmS).toBeNull();
  });

  it("asks for the list and a track of the universe named", () => {
    expect(toGlobalFeaturesRequest(UNIVERSE_ID)).toEqual({
      kind: "global_features",
      universe: UNIVERSE_ID,
    });
    expect(toStreamTrackRequest(UNIVERSE_ID, 7)).toEqual({
      kind: "stream_track",
      universe: UNIVERSE_ID,
      stream: 7,
    });
  });
});
