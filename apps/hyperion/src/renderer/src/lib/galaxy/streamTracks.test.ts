import { describe, expect, it } from "vitest";

import { mapGeometry } from "./mapGeometry";
import type { GlobalDwarfCore, StreamTrack, TrackPoint } from "./model";
import { placeStreamOverlay, projectTrack, streamFigures } from "./streamTracks";

/** A face-on map 200,000 ly across, centred on the galactic centre. */
const FACE_ON = mapGeometry({
  view: "face_on",
  width_px: 8,
  height_px: 8,
  centre_ly: [0, 0],
  ly_per_px: 25_000,
});

/** An edge-on map 200,000 ly across and 100,000 ly high. */
const EDGE_ON = mapGeometry({
  view: "edge_on",
  width_px: 8,
  height_px: 4,
  centre_ly: [0, 0],
  ly_per_px: 25_000,
});

/** A square picture of 400 CSS px, at a 16 px rem. */
const PICTURE = { widthPx: 400, heightPx: 400, remPx: 16 };

function point(
  x: number,
  y: number,
  z: number,
  widthLy = 200,
  dispersionKmS: number | null = 0.75,
): TrackPoint {
  return {
    positionLy: { x, y, z },
    normalWidthLy: widthLy,
    binormalWidthLy: widthLy,
    speedKmS: 210,
    dispersionKmS,
  };
}

function track(
  trailing: ReadonlyArray<TrackPoint>,
  leading: ReadonlyArray<TrackPoint>,
  number = 0,
): StreamTrack {
  return { number, lengthLy: 100_000, trailing, leading };
}

describe("projectTrack (plan 10, P10.T11)", () => {
  it("draws a track inside the view as one line through its points, trailing arm first", () => {
    const runs = projectTrack(
      "face_on",
      FACE_ON,
      PICTURE,
      track([point(-50_000, 0, 0), point(0, 0, 0)], [point(0, 0, 0), point(50_000, 0, 0)]),
    );

    // Face-on, +x runs down the picture and +y to the right.
    expect(runs).toEqual([
      [
        { xPx: 200, yPx: 100 },
        { xPx: 200, yPx: 200 },
        { xPx: 200, yPx: 200 },
        { xPx: 200, yPx: 300 },
      ],
    ]);
  });

  it("cuts a track where it leaves the view, at the picture's edge", () => {
    const runs = projectTrack(
      "face_on",
      FACE_ON,
      PICTURE,
      track([point(0, 0, 0)], [point(150_000, 0, 0)]),
    );

    expect(runs).toEqual([
      [
        { xPx: 200, yPx: 200 },
        { xPx: 200, yPx: 400 },
      ],
    ]);
  });

  it("draws a track that leaves the view and comes back as its two parts inside", () => {
    const runs = projectTrack(
      "face_on",
      FACE_ON,
      PICTURE,
      track(
        [point(0, 0, 0), point(150_000, 0, 0)],
        [point(150_000, 50_000, 0), point(0, 50_000, 0)],
      ),
    );

    expect(runs).toEqual([
      [
        { xPx: 200, yPx: 200 },
        { xPx: 200, yPx: 400 },
      ],
      [
        { xPx: 300, yPx: 400 },
        { xPx: 300, yPx: 200 },
      ],
    ]);
  });

  it("draws nothing of a track wholly beyond the view", () => {
    expect(
      projectTrack(
        "face_on",
        FACE_ON,
        PICTURE,
        track([point(120_000, 0, 0)], [point(150_000, 10_000, 0)]),
      ),
    ).toEqual([]);
  });

  it("projects edge-on along y, with z up the picture", () => {
    const picture = { widthPx: 400, heightPx: 200, remPx: 16 };
    const runs = projectTrack(
      "edge_on",
      EDGE_ON,
      picture,
      track([point(-50_000, 90_000, 25_000)], [point(50_000, -90_000, -25_000)]),
    );

    expect(runs).toEqual([
      [
        { xPx: 100, yPx: 50 },
        { xPx: 300, yPx: 150 },
      ],
    ]);
  });
});

/** A dwarf core 50,000 ly out along -y, 400 ly in radius, with what `overrides` sets. */
function core(overrides: Partial<GlobalDwarfCore> = {}): GlobalDwarfCore {
  return {
    number: 0,
    massMsun: 2e6,
    positionLy: { x: 0, y: -50_000, z: 0 },
    extentLy: 400,
    ...overrides,
  };
}

describe("placeStreamOverlay (plan 10, P10.T11)", () => {
  const along = track([point(-50_000, 0, 0)], [point(0, 0, 0), point(50_000, 0, 0)]);

  it("labels a track STREAM at its leading arm's tip and a core DWARF CORE beside its mark", () => {
    const places = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      {
        tracks: [{ track: along, massMsun: 1e4 }],
        cores: [core()],
        streamsShown: true,
        coresShown: true,
      },
      [],
    );

    const [placed] = places.tracks;
    expect(placed?.label?.text).toBe("STREAM");
    // Beside the tip, at 300 px down: to its right, the guide's gap from it.
    expect(placed?.label?.leftPx).toBe(204);
    const [mark] = places.cores;
    expect(mark).toMatchObject({ key: "core:0", xPx: 100, yPx: 200, circlePx: null });
    expect(mark?.label?.text).toBe("DWARF CORE");
  });

  it("draws a core wider than its mark at its true radius too", () => {
    const places = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      { tracks: [], cores: [core({ extentLy: 10_000 })], streamsShown: true, coresShown: true },
      [],
    );

    expect(places.cores[0]?.circlePx).toBe(20);
  });

  it("draws no core beyond the view or without a place, and nothing of a kind not shown", () => {
    const hidden = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      {
        tracks: [{ track: along, massMsun: 1e4 }],
        cores: [
          core({ positionLy: { x: 0, y: 150_000, z: 0 } }),
          core({ number: 1, positionLy: null }),
        ],
        streamsShown: false,
        coresShown: true,
      },
      [],
    );

    expect(hidden).toEqual({ tracks: [], cores: [] });
  });

  it("labels a track at its trailing tip where its leading tip lies beyond the view", () => {
    const places = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      {
        tracks: [{ track: track([point(0, 0, 0)], [point(150_000, 0, 0)]), massMsun: 1e4 }],
        cores: [],
        streamsShown: true,
        coresShown: false,
      },
      [],
    );

    expect(places.tracks[0]?.label).toMatchObject({ text: "STREAM", leftPx: 204 });
  });

  it("labels no track whose tips both lie beyond the view, where the picture's edge cuts it", () => {
    const places = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      {
        tracks: [{ track: track([point(-150_000, 0, 0)], [point(150_000, 0, 0)]), massMsun: 1e4 }],
        cores: [],
        streamsShown: true,
        coresShown: false,
      },
      [],
    );

    expect(places.tracks[0]?.runs).toHaveLength(1);
    expect(places.tracks[0]?.label).toBeNull();
  });

  it("keeps a track's label off a core's mark, to the left where the right is taken", () => {
    const places = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      {
        tracks: [{ track: along, massMsun: 1e4 }],
        // The core's mark stands where the tip's label would start, at (230, 300) px.
        cores: [core({ positionLy: { x: 50_000, y: 15_000, z: 0 } })],
        streamsShown: true,
        coresShown: true,
      },
      [],
    );

    expect(places.tracks[0]?.label?.leftPx).toBeLessThan(200);
  });

  it("places a core's label outside its circle at true radius", () => {
    const places = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      { tracks: [], cores: [core({ extentLy: 10_000 })], streamsShown: true, coresShown: true },
      [],
    );

    // The circle's 20 px radius and the guide's gap beyond the mark's centre at 100 px.
    expect(places.cores[0]?.label?.leftPx).toBe(124);
  });

  it("drops a label that would cross one placed before it", () => {
    const taken = [{ leftPx: 0, topPx: 0, widthPx: 400, heightPx: 400 }];
    const places = placeStreamOverlay(
      "face_on",
      FACE_ON,
      PICTURE,
      {
        tracks: [{ track: along, massMsun: 1e4 }],
        cores: [],
        streamsShown: true,
        coresShown: false,
      },
      taken,
    );

    expect(places.tracks[0]?.label).toBeNull();
    expect(places.tracks[0]?.runs).toHaveLength(1);
  });
});

describe("streamFigures (plan 10, P10.T11)", () => {
  it("states the server's length and the medians of the points' widths and dispersions", () => {
    const figures = streamFigures(
      track(
        [point(0, 0, 0, 100, 0.5), point(1, 0, 0, 300, 1.5)],
        [point(2, 0, 0, 200, 1), point(3, 0, 0, 400, null)],
      ),
    );

    expect(figures).toEqual({ lengthLy: 100_000, widthLy: 250, dispersionKmS: 1 });
  });

  it("states no dispersion where the server sends none", () => {
    const figures = streamFigures(track([point(0, 0, 0, 100, null)], []));

    expect(figures.dispersionKmS).toBeNull();
    expect(figures.widthLy).toBe(100);
  });
});
