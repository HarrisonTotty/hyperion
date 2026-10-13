import { type GalacticPosition, galacticPositionFromLy } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { sub, type Vec3, vec3 } from "../../geometry/vec3";
import { RECONNECT_DELAY_MS } from "../../lib/connection";
import type { SystemPlace } from "../../lib/scene/model";
import { binaryFrame } from "../../test/binaryFrames";
import { autoAt, manualAt } from "../../test/exposureFixtures";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  FIXTURE_BAND_CD_M2,
  type FixtureStar,
  InThreadSkyWorker,
  nearestFirstCensus,
  skyPayload,
  skyResponse,
} from "../../test/skyFixtures";
import { AU_M, aViewScene, FIXTURE_SYSTEM } from "../../test/viewFixtures";
import { galacticDeltaM } from "../../view/coords/position";
import { DEFAULT_EXPOSURE, type ExposureControl } from "../../view/photometry/exposure";
import type { QualitySetting } from "../../view/quality/qualitySetting";
import type { ViewScene } from "../../view/scene/model";
import { primarySkyPlace, type SkyLineReading, useViewSky } from "./useViewSky";
import { type LabelField, startInstrumentRun, startServerRun, type ViewRun } from "./viewRun";

const UNIVERSE = "000000000000002a";

const SCENE = aViewScene();

/** A system's place as a chart gives it. */
type ChartedPlace = Extract<SystemPlace, { readonly kind: "charted" }>;

/** `scene`'s system where the chart puts it, so that its sky can be asked. */
function placeOf(scene: ViewScene): ChartedPlace {
  if (scene.barycentre === null) {
    throw new Error("the scene has no barycentre");
  }
  return {
    kind: "charted",
    system: scene.system,
    designation: "TEST SYSTEM",
    barycentre: scene.barycentre,
  };
}

/** The fixture scene's system where the chart puts it. */
function chartedPlace(): ChartedPlace {
  return placeOf(SCENE);
}

/** A camera view of the server's scene (R02's `camera` role, as an instrument's) at 60°. */
function cameraRun(): ViewRun {
  const run = startInstrumentRun(startServerRun(SCENE));
  return { ...run, camera: { ...run.camera, fovDeg: 60 } };
}

const RUN = cameraRun();

/** A camera view of `scene` at 60°, its camera at rest `offsetM` from the barycentre. */
function runAt(offsetM: Vec3, scene: ViewScene = SCENE): ViewRun {
  const run = startInstrumentRun(startServerRun(scene));
  return {
    ...run,
    camera: {
      ...run.camera,
      fovDeg: 60,
      move: null,
      pose: {
        ...run.camera.pose,
        frame: { kind: "system", system: scene.system },
        positionM: offsetM,
      },
    },
  };
}

/**
 * The largest axis's difference, m, between `position`'s offset from `from` and `offsetM`; infinite
 * where no position was asked.
 */
function offsetErrorM(
  from: GalacticPosition,
  position: GalacticPosition | undefined,
  offsetM: Vec3,
): number {
  if (position === undefined) {
    return Number.POSITIVE_INFINITY;
  }
  const error = sub(galacticDeltaM(from, position), offsetM);
  return Math.max(Math.abs(error.x), Math.abs(error.y), Math.abs(error.z));
}

/** `AUTO` stepping from EV100 −10 to 15 by tenths, as the readout shows it, then `MAN` entries. */
const EXPOSURES: ReadonlyArray<ExposureControl> = [
  ...Array.from({ length: 251 }, (_, i) => autoAt(-10 + i / 10)),
  manualAt(-1),
  manualAt(15),
  manualAt(-14),
  manualAt(42),
];

interface SkyProps {
  readonly exposure: ExposureControl;
}

/** The view's sky for the camera run, over a welcomed link, at an exposure and a setting. */
function renderViewSky(exposure: ExposureControl, setting: QualitySetting = "high") {
  const hook = renderHook(
    (props: SkyProps) =>
      useViewSky({
        universe: UNIVERSE,
        place: chartedPlace(),
        run: RUN,
        exposure: props.exposure,
        widthPx: 1_920,
        setting,
      }),
    { initialProps: { exposure }, wrapper: ServerLinkHarness },
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { ...hook, socket };
}

/**
 * The camera's limit in its field (R06.T13.j), sized for `V -20.2 mag S`, live or stale: V 9.6, its
 * limit at 60° and the default exposure over the fixture's band, unless given.
 */
function limitField(stale: boolean, text = "V 9.6 mag"): LabelField {
  return { text, widthCh: 13, stale };
}

/** The stars-arriving note's edge in its field (R06.T11.f), sized for `128,000 ly S`. */
function edgeField(text: string, stale: boolean): LabelField {
  return { text, widthCh: 12, stale };
}

/** One bright star 100 ly away, ahead of the camera. */
const ONE_STAR: ReadonlyArray<FixtureStar> = [{ direction: [0, 0, -1], distanceLy: 100, vMag: 1 }];

/**
 * Plays the server's answer to the latest sky request, with `stars` listed, and lets the decode
 * settle; its band texels lit at `luminanceOf` their index where that is given.
 */
async function answerSky(
  socket: FakeWebSocket,
  stars: ReadonlyArray<FixtureStar> = ONE_STAR,
  luminanceOf?: (texel: number) => number,
): Promise<void> {
  const sent = socket.requestsOfKind("sky").at(-1);
  if (sent === undefined) {
    throw new Error("no sky was asked");
  }
  const payload = skyPayload(stars, 2, null, luminanceOf);
  await act(async () => {
    socket.serverSendsBinary(binaryFrame(sent.id, 0, 1, [...payload]));
    socket.serverResponds(sent.id, {
      kind: "sky",
      ...skyResponse(sent.body, payload, stars.length, 2),
    });
    await vi.advanceTimersByTimeAsync(0);
  });
}

/**
 * 3,000 stars of V 1 a thousand light years away, spread over the sky: more than the low
 * setting's 2,048 sprites and fewer than the high setting's 4,096, none near enough to be a sprite
 * whatever the budget.
 */
const FAR_STARS: ReadonlyArray<FixtureStar> = Array.from({ length: 3_000 }, (_, i) => {
  // A Fibonacci lattice on the sphere: distinct directions, none repeated.
  const z = 1 - (2 * (i + 0.5)) / 3_000;
  const r = Math.sqrt(1 - z * z);
  const phi = i * Math.PI * (3 - Math.sqrt(5));
  return { direction: [r * Math.cos(phi), r * Math.sin(phi), z], distanceLy: 1_000, vMag: 1 };
});

describe("a camera view's sky", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.stubGlobal("Worker", InThreadSkyWorker);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it.each([
    { level: "MAN −1", exposure: manualAt(-1) },
    { level: "MAN 15", exposure: manualAt(15) },
    { level: "AUTO −10", exposure: autoAt(-10) },
    { level: "AUTO 0", exposure: autoAt(0) },
    { level: "AUTO 15", exposure: autoAt(15) },
  ])("asks at the camera's black-sky limit, V 10.15 at 60°, under $level", ({ exposure }) => {
    const { socket } = renderViewSky(exposure);
    const limitV = socket.requestsOfKind("sky").at(-1)?.body.camera_limit_v ?? Number.NaN;
    expect(Math.round(limitV * 100) / 100).toBe(10.15);
  });

  it("is not asked again as AUTO steps and MAN is entered once it is held", async () => {
    // Held from MAN 15, where the exposure's own limit is V 2.56, so a request that followed the
    // exposure would ask again for a deeper limit at the first step.
    const { rerender, socket } = renderViewSky(manualAt(15));
    await answerSky(socket);
    for (const exposure of EXPOSURES) {
      rerender({ exposure });
    }
    expect(socket.requestsOfKind("sky")).toHaveLength(1);
  });

  it("keeps its drawn sky across AUTO's steps and MAN's entries, culled and baked once", async () => {
    const { result, rerender, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await answerSky(socket);
    const drawn = result.current.drawn;
    const kept = EXPOSURES.map((exposure) => {
      rerender({ exposure });
      return result.current.drawn === drawn;
    });
    expect([drawn === null, kept.filter((same) => !same).length]).toEqual([false, 0]);
  });

  it("labels the camera's limit at the exposure shown over the band (μ 21.6): V 9.6 at MAN −1, V 2.6 at AUTO 15", async () => {
    const { result, rerender, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await answerSky(socket);
    const atDefault = result.current.label("alone")?.value;
    rerender({ exposure: autoAt(15) });
    expect([atDefault, result.current.label("alone")?.value]).toEqual([
      "V 9.6 mag CAM · CLUSTERS: NOT YET MODELLED",
      "V 2.6 mag CAM · CLUSTERS: NOT YET MODELLED",
    ]);
  });

  it("labels the darkest texel's limit over the whole sky, the same wherever the camera points", async () => {
    // The fixture's band at μ 21.6 but its last texel, on the −Z face, at μ 25.25, whose limit at
    // 60° and the default exposure, V 10.12, is the label's whether it lies ahead or behind.
    const darkest = 6 * 2 * 2 - 1;
    const pointed = (orientation: ViewRun["camera"]["pose"]["orientation"]): ViewRun => ({
      ...RUN,
      camera: { ...RUN.camera, move: null, pose: { ...RUN.camera.pose, orientation } },
    });
    const runs = [
      pointed({ w: 1, x: 0, y: 0, z: 0 }),
      pointed({ w: 0, x: 0, y: 1, z: 0 }),
      pointed({ w: Math.SQRT1_2, x: Math.SQRT1_2, y: 0, z: 0 }),
    ];
    const { result, rerender } = renderHook(
      ({ run }: { readonly run: ViewRun }) =>
        useViewSky({
          universe: UNIVERSE,
          place: chartedPlace(),
          run,
          exposure: DEFAULT_EXPOSURE,
          widthPx: 1_920,
          setting: "high",
        }),
      { initialProps: { run: runs[0] ?? RUN }, wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    await answerSky(socket, ONE_STAR, (texel) =>
      texel === darkest ? 10 ** ((12.58 - 25.25) / 2.5) : FIXTURE_BAND_CD_M2,
    );
    const labels = runs.map((run) => {
      rerender({ run });
      return result.current.label("alone")?.value;
    });
    expect([labels, socket.requestsOfKind("sky").length]).toEqual([
      runs.map(() => "V 10.1 mag CAM · CLUSTERS: NOT YET MODELLED"),
      1,
    ]);
  });
});

describe("a view's sky arriving nearest first (R06.T11.f)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.stubGlobal("Worker", InThreadSkyWorker);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /**
   * Plays a reply to the latest sky request whose census has reached `edgesLy`, held at the
   * synthetic ceiling `ceilingV` where one is given (R13.T2.b).
   */
  async function replyAt(
    socket: FakeWebSocket,
    edgesLy: readonly [number | null, number | null, number | null],
    final: boolean,
    ceilingV?: number,
  ): Promise<void> {
    const sent = socket.requestsOfKind("sky").at(-1);
    if (sent === undefined) {
      throw new Error("no sky was asked");
    }
    const payload = skyPayload(ONE_STAR, 2, null);
    const body = {
      kind: "sky" as const,
      ...skyResponse(sent.body, payload, ONE_STAR.length, 2),
      census: nearestFirstCensus(edgesLy),
      final,
      ...(ceilingV === undefined ? {} : { synthetic_ceiling_v: ceilingV, real_limit_ly: 2_000 }),
    };
    await act(async () => {
      socket.serverSendsBinary(binaryFrame(sent.id, 0, 1, [...payload]));
      if (final) {
        socket.serverResponds(sent.id, body);
      } else {
        socket.serverAnswersInPart(sent.id, body);
      }
      await vi.advanceTimersByTimeAsync(0);
    });
  }

  it("reads PENDING from the request until its first reply, on every block", () => {
    const { result } = renderViewSky(DEFAULT_EXPOSURE);
    expect([
      result.current.awaiting,
      result.current.label("alone"),
      result.current.label("instrument"),
    ]).toEqual([true, { value: "PENDING" }, { value: "PENDING" }]);
  });

  it("sets the edge in its field while the reply is not final, and clears it on the final", async () => {
    const { result, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await replyAt(socket, [500, 500, 500], false);
    const streaming = result.current.label("alone");
    await replyAt(socket, [null, null, null], true);
    expect([result.current.awaiting, streaming, result.current.label("alone")]).toEqual([
      false,
      {
        value: "V 9.6 mag CAM · BEYOND 500 ly: STREAMING · CLUSTERS: NOT YET MODELLED",
        fields: [limitField(false), edgeField("500 ly", false)],
      },
      { value: "V 9.6 mag CAM · CLUSTERS: NOT YET MODELLED", fields: [limitField(false)] },
    ]);
  });

  it("gives the edge's field up where the line's one note is what the sky leaves out", async () => {
    const { result, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await replyAt(socket, [2_000, 4_000, 4_000], false);
    expect([result.current.label("beside"), result.current.label("beside-unshown")]).toEqual([
      {
        value: "V 9.6 mag CAM · BEYOND 2000 ly: STREAMING",
        fields: [limitField(false), edgeField("2000 ly", false)],
      },
      { value: "V 9.6 mag CAM · CLUSTERS: NOT YET MODELLED", fields: [limitField(false)] },
    ]);
  });

  it("marks the limit and the edge stale while the link is down, and holds the notes", async () => {
    const { result, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await replyAt(socket, [2_000, 4_000, 4_000], false);
    act(() => {
      socket.close();
    });
    expect(result.current.label("alone")).toEqual({
      value: "V 9.6 mag CAM · BEYOND 2000 ly: STREAMING · CLUSTERS: NOT YET MODELLED",
      fields: [limitField(true), edgeField("2000 ly", true)],
    });
  });

  it("holds the interim note at a ceiling, unmarked, the limit's and the edge's fields stale while the link is down", async () => {
    const { result, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await replyAt(socket, [1_000, 1_000, 1_000], false, 5.0);
    const live = result.current.label("alone");
    act(() => {
      socket.close();
    });
    const value =
      "V 9.6 mag CAM · BEYOND 1000 ly: STREAMING · DISTANT STARS: DETAIL LIMITED · CLUSTERS: NOT YET MODELLED";
    expect([live, result.current.label("alone"), result.current.label("instrument")]).toEqual([
      { value, fields: [limitField(false), edgeField("1000 ly", false)] },
      { value, fields: [limitField(true), edgeField("1000 ly", true)] },
      { value: "V 9.6 mag CAM · CLUSTERS: NOT YET MODELLED", fields: [limitField(true)] },
    ]);
  });

  it("gives the primary's one note to STREAMING beside an instrument, then to the interim note", async () => {
    const { result, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await replyAt(socket, [1_000, 1_000, 1_000], false, 5.0);
    const streaming = result.current.label("beside");
    await replyAt(socket, [null, null, null], true, 5.0);
    expect([streaming, result.current.label("beside")]).toEqual([
      {
        value: "V 9.6 mag CAM · BEYOND 1000 ly: STREAMING",
        fields: [limitField(false), edgeField("1000 ly", false)],
      },
      { value: "V 9.6 mag CAM · DISTANT STARS: DETAIL LIMITED", fields: [limitField(false)] },
    ]);
  });
});

/** Closes the link and lets what it cut off settle. */
async function loseLink(socket: FakeWebSocket): Promise<void> {
  await act(async () => {
    socket.close();
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** A `STARS` reading's text and its fields' texts and widths: all but their staleness. */
function readingShape(reading: SkyLineReading | null) {
  return [reading?.value, reading?.fields?.map((field) => [field.text, field.widthCh])];
}

describe("a view's star limit while the link is down (R06.T13.j)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.stubGlobal("Worker", InThreadSkyWorker);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** An eye view of the server's scene at 60°, the `PRIMARY` view's role. */
  function eyeRun(): ViewRun {
    const run = startServerRun(SCENE);
    return { ...run, camera: { ...run.camera, fovDeg: 60 } };
  }

  /** The view's sky for `run`, over a welcomed link. */
  function renderRun(run: ViewRun) {
    const hook = renderHook(
      () =>
        useViewSky({
          universe: UNIVERSE,
          place: chartedPlace(),
          run,
          exposure: DEFAULT_EXPOSURE,
          widthPx: 1_920,
          setting: "high",
        }),
      { wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    return { ...hook, socket };
  }

  /**
   * Plays a reply to the latest sky request, its band's eye limits at V 7.4, whose census has
   * reached `edgesLy` (final where every edge is `null`).
   */
  async function replyAt(
    socket: FakeWebSocket,
    edgesLy: readonly [number | null, number | null, number | null],
  ): Promise<void> {
    const sent = socket.requestsOfKind("sky").at(-1);
    if (sent === undefined) {
      throw new Error("no sky was asked");
    }
    const final = edgesLy.every((edge) => edge === null);
    const payload = skyPayload(ONE_STAR, 2, 7.4);
    const body = {
      kind: "sky" as const,
      ...skyResponse(sent.body, payload, ONE_STAR.length, 2),
      census: nearestFirstCensus(edgesLy),
      final,
    };
    await act(async () => {
      socket.serverSendsBinary(binaryFrame(sent.id, 0, 1, [...payload]));
      if (final) {
        socket.serverResponds(sent.id, body);
      } else {
        socket.serverAnswersInPart(sent.id, body);
      }
      await vi.advanceTimersByTimeAsync(0);
    });
  }

  it.each([
    { kind: "an eye", run: eyeRun, limit: "V 7.4 mag", line: "V 7.4 mag EYE" },
    { kind: "a camera", run: cameraRun, limit: "V 9.6 mag", line: "V 9.6 mag CAM" },
  ])(
    "reads $kind's limit muted with its S on the primary's line and an instrument's, the sky still drawn",
    async ({ run, limit, line }) => {
      const { result, socket } = renderRun(run());
      await replyAt(socket, [null, null, null]);
      await loseLink(socket);
      const value = `${line} · CLUSTERS: NOT YET MODELLED`;
      expect([
        result.current.drawn === null,
        result.current.label("alone"),
        result.current.label("instrument"),
      ]).toEqual([
        false,
        { value, fields: [limitField(true, limit)] },
        { value, fields: [limitField(true, limit)] },
      ]);
    },
  );

  it("takes no mark on PENDING before the first reply", () => {
    const { result } = renderRun(cameraRun());
    expect([result.current.label("alone"), result.current.label("instrument")]).toStrictEqual([
      { value: "PENDING" },
      { value: "PENDING" },
    ]);
  });

  it("takes no mark on PENDING after a jump while the link is down, though the sky left is stale", async () => {
    const arrived = aViewScene({
      system: "0200080020000001",
      barycentre: galacticPositionFromLy([8_004, 26_000, 20]),
    });
    const { rerender, result } = renderHook(
      ({ place }: { readonly place: SystemPlace }) =>
        useViewSky({
          universe: UNIVERSE,
          place,
          run: cameraRun(),
          exposure: DEFAULT_EXPOSURE,
          widthPx: 1_920,
          setting: "high",
        }),
      { initialProps: { place: chartedPlace() }, wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    await replyAt(socket, [null, null, null]);
    // The place has moved to the system arrived at; the published run is still in the one left.
    rerender({ place: placeOf(arrived) });
    await loseLink(socket);
    expect([result.current.label("alone"), result.current.label("instrument")]).toStrictEqual([
      { value: "PENDING" },
      { value: "PENDING" },
    ]);
  });

  it("clears the limit's mark with the edge's when the link returns", async () => {
    const { result, socket } = renderRun(eyeRun());
    await replyAt(socket, [2_000, 4_000, 4_000]);
    await loseLink(socket);
    const down = result.current.label("alone");
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    act(() => {
      FakeWebSocket.latest().serverWelcomes();
    });
    const value = "V 7.4 mag EYE · BEYOND 2000 ly: STREAMING · CLUSTERS: NOT YET MODELLED";
    expect([down, result.current.label("alone")]).toEqual([
      { value, fields: [limitField(true, "V 7.4 mag"), edgeField("2000 ly", true)] },
      { value, fields: [limitField(false, "V 7.4 mag"), edgeField("2000 ly", false)] },
    ]);
  });

  it("keeps the line's reading and its fields' widths as it goes stale, each wide enough for its mark", async () => {
    const { result, socket } = renderRun(cameraRun());
    await replyAt(socket, [128_000, null, null]);
    const live = result.current.label("alone");
    await loseLink(socket);
    const stale = result.current.label("alone");
    expect([
      readingShape(stale),
      stale?.fields?.every((field) => `${field.text} S`.length <= field.widthCh),
    ]).toEqual([readingShape(live), true]);
  });
});

describe("a view's sky asked again within its arrival (R06.T12.b)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.stubGlobal("Worker", InThreadSkyWorker);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  interface ArrivalProps {
    readonly system: string;
    readonly setting: QualitySetting;
  }

  /** The view's sky for the camera run in `system` at `setting`, over a welcomed link. */
  function renderArriving(initial: ArrivalProps) {
    const hook = renderHook(
      ({ system, setting }: ArrivalProps) =>
        useViewSky({
          universe: UNIVERSE,
          place: { ...chartedPlace(), system },
          // A jump's published run follows its place into the system arrived at.
          run: system === FIXTURE_SYSTEM ? RUN : runAt(vec3(0, 1e8, 0), aViewScene({ system })),
          exposure: DEFAULT_EXPOSURE,
          widthPx: 1_920,
          setting,
        }),
      { initialProps: initial, wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    return { ...hook, socket };
  }

  /** Plays a reply to the latest sky request whose census has reached `edgesLy`. */
  async function replyAt(
    socket: FakeWebSocket,
    edgesLy: readonly [number | null, number | null, number | null],
  ): Promise<void> {
    const sent = socket.requestsOfKind("sky").at(-1);
    if (sent === undefined) {
      throw new Error("no sky was asked");
    }
    const final = edgesLy.every((edge) => edge === null);
    const payload = skyPayload(ONE_STAR, 2, null);
    const body = {
      kind: "sky" as const,
      ...skyResponse(sent.body, payload, ONE_STAR.length, 2),
      census: nearestFirstCensus(edgesLy),
      final,
    };
    await act(async () => {
      socket.serverSendsBinary(binaryFrame(sent.id, 0, 1, [...payload]));
      if (final) {
        socket.serverResponds(sent.id, body);
      } else {
        socket.serverAnswersInPart(sent.id, body);
      }
      await vi.advanceTimersByTimeAsync(0);
    });
  }

  const FINAL_LINE = {
    value: "V 9.6 mag CAM · CLUSTERS: NOT YET MODELLED",
    fields: [limitField(false)],
  };

  it("keeps the held sky's line, neither PENDING nor STREAMING, through a deeper re-ask", async () => {
    const { rerender, result, socket } = renderArriving({ system: FIXTURE_SYSTEM, setting: "low" });
    await replyAt(socket, [null, null, null]);
    // A larger N_max asks for more stars than the held sky's request did.
    rerender({ system: FIXTURE_SYSTEM, setting: "high" });
    expect(socket.requestsOfKind("sky")).toHaveLength(2);
    const asked = [result.current.awaiting, result.current.pending, result.current.label("alone")];
    await replyAt(socket, [125, 125, 125]);
    const partial = [
      result.current.awaiting,
      result.current.pending,
      result.current.label("alone"),
    ];
    expect([asked, partial]).toEqual([
      [false, false, FINAL_LINE],
      [false, false, FINAL_LINE],
    ]);
  });

  it("keeps a partial sky's edge, never moving it back, through the re-ask after a lost link", async () => {
    const { result, socket } = renderArriving({ system: FIXTURE_SYSTEM, setting: "high" });
    await replyAt(socket, [500, 500, 500]);
    // The request the link cut off settles before the link returns.
    await act(async () => {
      socket.close();
      await vi.advanceTimersByTimeAsync(0);
    });
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const next = FakeWebSocket.latest();
    act(() => {
      next.serverWelcomes();
    });
    expect(next.requestsOfKind("sky")).toHaveLength(1);
    await replyAt(next, [125, 125, 125]);
    const behind = result.current.label("alone");
    await replyAt(next, [1_000, 1_000, 1_000]);
    expect([behind, result.current.label("alone")]).toEqual([
      {
        value: "V 9.6 mag CAM · BEYOND 500 ly: STREAMING · CLUSTERS: NOT YET MODELLED",
        fields: [limitField(false), edgeField("500 ly", false)],
      },
      {
        value: "V 9.6 mag CAM · BEYOND 1000 ly: STREAMING · CLUSTERS: NOT YET MODELLED",
        fields: [limitField(false), edgeField("1000 ly", false)],
      },
    ]);
  });

  it("reads PENDING on every block on a new arrival until its first reply", async () => {
    const { rerender, result, socket } = renderArriving({
      system: FIXTURE_SYSTEM,
      setting: "high",
    });
    await replyAt(socket, [null, null, null]);
    rerender({ system: "0200080020000001", setting: "high" });
    const before = [
      result.current.awaiting,
      result.current.label("alone"),
      result.current.label("instrument"),
    ];
    await replyAt(socket, [125, 125, 125]);
    expect([before, result.current.awaiting]).toEqual([
      [true, { value: "PENDING" }, { value: "PENDING" }],
      false,
    ]);
  });
});

describe("a view's sky asked from its camera's place (R06.T12; a correction to T13.c)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.stubGlobal("Worker", InThreadSkyWorker);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** `GalacticPosition`'s resolution anywhere in the galaxy, m: the protocol's precision. */
  const GALACTIC_RESOLUTION_M = 2;

  /**
   * One star 10 ly away: the nearest baked star at 1080p and 60°, beyond the 8.7 ly of the parallax
   * sprites, which a camera 100 au from the sky's observer shifts by 0.29 px, past the request
   * rule's tenth of a pixel and short of the hold's 1 px.
   */
  const NEAR_BAKED: ReadonlyArray<FixtureStar> = [
    { direction: [0, 0, -1], distanceLy: 10, vMag: 1 },
  ];

  interface PlacedProps {
    readonly run: ViewRun;
    readonly place: SystemPlace;
  }

  /** The view's sky for a run and a place, over a welcomed link. */
  function renderPlaced(initial: PlacedProps) {
    const hook = renderHook(
      ({ run, place }: PlacedProps) =>
        useViewSky({
          universe: UNIVERSE,
          place,
          run,
          exposure: DEFAULT_EXPOSURE,
          widthPx: 1_920,
          setting: "high",
        }),
      { initialProps: initial, wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    return { ...hook, socket };
  }

  /**
   * Plays a reply of {@link NEAR_BAKED} to the latest sky request whose census has reached
   * `edgesLy`, final where every edge is `null`.
   */
  async function replyAt(
    socket: FakeWebSocket,
    edgesLy: readonly [number | null, number | null, number | null],
  ): Promise<void> {
    const sent = socket.requestsOfKind("sky").at(-1);
    if (sent === undefined) {
      throw new Error("no sky was asked");
    }
    const final = edgesLy.every((edge) => edge === null);
    const payload = skyPayload(NEAR_BAKED, 2, null);
    const body = {
      kind: "sky" as const,
      ...skyResponse(sent.body, payload, NEAR_BAKED.length, 2),
      census: nearestFirstCensus(edgesLy),
      final,
    };
    await act(async () => {
      socket.serverSendsBinary(binaryFrame(sent.id, 0, 1, [...payload]));
      if (final) {
        socket.serverResponds(sent.id, body);
      } else {
        socket.serverAnswersInPart(sent.id, body);
      }
      await vi.advanceTimersByTimeAsync(0);
    });
  }

  const BARYCENTRE = placeOf(SCENE).barycentre;

  /** Where the camera starts: 100 au from the barycentre. */
  const START_M = vec3(100 * AU_M, 0, 0);

  it("asks a camera 100 au out once, holds its final, and asks nothing more while it stays", async () => {
    const { rerender, result, socket } = renderPlaced({
      run: runAt(START_M),
      place: chartedPlace(),
    });
    await replyAt(socket, [null, null, null]);
    // Each published run is a new one, at the same place.
    for (let i = 0; i < 8; i += 1) {
      rerender({ run: runAt(START_M), place: chartedPlace() });
    }
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect([
      socket.requestsOfKind("sky").length,
      result.current.drawn?.model.response.final,
    ]).toEqual([1, true]);
  });

  it("asks again from the camera's new place past the rule's tenth of a pixel, holding its final", async () => {
    const { rerender, result, socket } = renderPlaced({
      run: runAt(START_M),
      place: chartedPlace(),
    });
    await replyAt(socket, [null, null, null]);
    // 200 au from the held sky's observer: 0.58 px, short of the hold's 1 px.
    const to = vec3(300 * AU_M, 0, 0);
    rerender({ run: runAt(to), place: chartedPlace() });
    const asked = socket.requestsOfKind("sky");
    await replyAt(socket, [125, 125, 125]);
    expect([
      asked.length,
      offsetErrorM(BARYCENTRE, asked.at(-1)?.body.observer, to) <= GALACTIC_RESOLUTION_M,
      result.current.drawn?.model.response.final,
    ]).toEqual([2, true, true]);
  });

  it("past the hold's 1 px cap, holds the first reply of the re-ask from the camera's new place", async () => {
    const { rerender, result, socket } = renderPlaced({
      run: runAt(START_M),
      place: chartedPlace(),
    });
    await replyAt(socket, [null, null, null]);
    // 400 au from the held sky's observer: 1.16 px.
    const to = vec3(500 * AU_M, 0, 0);
    rerender({ run: runAt(to), place: chartedPlace() });
    await replyAt(socket, [125, 125, 125]);
    const held = result.current.drawn?.model;
    expect([
      held?.response.final,
      offsetErrorM(BARYCENTRE, held?.request.observer, to) <= GALACTIC_RESOLUTION_M,
    ]).toEqual([false, true]);
  });

  it("asks from the barycentre plus the camera's offset, to the galactic frame's 2 m", () => {
    // A barycentre just short of a cell's edge along x and just past one along y, so that the
    // offset carries into the next cell along x and borrows from the one below along y.
    const scene = aViewScene({
      barycentre: galacticPositionFromLy([8_000.999_999_9, 26_000.000_000_1, 20.5]),
    });
    const offsetM = vec3(100 * AU_M, -37.25 * AU_M, 12.5 * AU_M);
    const place = placeOf(scene);
    const { socket } = renderPlaced({ run: runAt(offsetM, scene), place });
    const observer = socket.requestsOfKind("sky").at(-1)?.body.observer;
    expect(offsetErrorM(place.barycentre, observer, offsetM)).toBeLessThanOrEqual(
      GALACTIC_RESOLUTION_M,
    );
  });

  /** The system a jump arrives at, 4 ly from the fixture's. */
  const ARRIVED = aViewScene({
    system: "0200080020000001",
    barycentre: galacticPositionFromLy([8_004, 26_000, 20]),
  });

  /** The view's sky held in the fixture's system, then the place moved to {@link ARRIVED}'s. */
  async function renderJumped() {
    const placed = renderPlaced({ run: runAt(START_M), place: chartedPlace() });
    await replyAt(placed.socket, [null, null, null]);
    // The place has moved to the system arrived at; the published run is still in the one left.
    placed.rerender({ run: runAt(START_M), place: placeOf(ARRIVED) });
    return placed;
  }

  it("asks nothing and reads PENDING while a jump's published run is still in the system left", async () => {
    const { result, socket } = await renderJumped();
    expect([
      socket.requestsOfKind("sky").length,
      result.current.awaiting,
      result.current.pending,
      result.current.label("alone"),
    ]).toEqual([1, true, true, { value: "PENDING" }]);
  });

  it("asks nothing for a place whose position is unknown, though the run's scene still carries one", () => {
    const { result, socket } = renderPlaced({
      run: runAt(START_M),
      place: { kind: "unknown", system: FIXTURE_SYSTEM, designation: "TEST SYSTEM" },
    });
    expect([socket.requestsOfKind("sky").length, result.current.awaiting]).toEqual([0, false]);
  });

  it("asks the system a jump arrives at from its camera once the published run reaches it", async () => {
    const { rerender, socket } = await renderJumped();
    const offsetM = vec3(0, 50 * AU_M, 0);
    rerender({ run: runAt(offsetM, ARRIVED), place: placeOf(ARRIVED) });
    const asked = socket.requestsOfKind("sky").at(-1)?.body;
    expect([
      asked?.exclude_system,
      offsetErrorM(placeOf(ARRIVED).barycentre, asked?.observer, offsetM) <= GALACTIC_RESOLUTION_M,
    ]).toEqual([ARRIVED.system, true]);
  });
});

describe("where the primary's sky line stands (decision-r06-t11f-stars-line)", () => {
  const SHOWN = {};

  it("stands alone with no instrument open", () => {
    expect(
      primarySkyPlace([
        { open: false, shown: null, skyLabel: null },
        { open: false, shown: null, skyLabel: null },
      ]),
    ).toBe("alone");
  });

  it("stands beside an open instrument whose line shows the sky's reading", () => {
    expect(
      primarySkyPlace([
        { open: true, shown: SHOWN, skyLabel: { value: "V 10.0 mag CAM" } },
        { open: true, shown: null, skyLabel: null },
      ]),
    ).toBe("beside");
  });

  it("keeps what the sky leaves out while no open instrument's line shows the sky", () => {
    expect(
      primarySkyPlace([
        { open: true, shown: SHOWN, skyLabel: null },
        { open: true, shown: null, skyLabel: { value: "V 10.0 mag CAM" } },
        { open: false, shown: SHOWN, skyLabel: { value: "V 10.0 mag CAM" } },
      ]),
    ).toBe("beside-unshown");
  });
});

describe("a view's sky at its quality setting (R07.T17)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.stubGlobal("Worker", InThreadSkyWorker);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it.each([
    { setting: "high", nMax: 300_000 },
    { setting: "low", nMax: 100_000 },
  ] as const)("is asked at the $setting setting's N_max, $nMax stars", ({ setting, nMax }) => {
    const { socket } = renderViewSky(DEFAULT_EXPOSURE, setting);
    expect(socket.requestsOfKind("sky").at(-1)?.body.n_max).toBe(nMax);
  });

  it.each([
    { setting: "high", sprites: 3_000 },
    { setting: "low", sprites: 2_048 },
  ] as const)(
    "draws at most the $setting setting's sprite budget: $sprites of 3,000 bright stars",
    async ({ setting, sprites }) => {
      const { result, socket } = renderViewSky(DEFAULT_EXPOSURE, setting);
      await answerSky(socket, FAR_STARS);
      expect(result.current.drawn?.selection.sprites.length).toBe(sprites);
    },
  );
});
