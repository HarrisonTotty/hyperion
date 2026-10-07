import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SystemPlace } from "../../lib/scene/model";
import { binaryFrame } from "../../test/binaryFrames";
import { autoAt, manualAt } from "../../test/exposureFixtures";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  type FixtureStar,
  InThreadSkyWorker,
  skyPayload,
  skyResponse,
} from "../../test/skyFixtures";
import { aViewScene, FIXTURE_SYSTEM } from "../../test/viewFixtures";
import { DEFAULT_EXPOSURE, type ExposureControl } from "../../view/photometry/exposure";
import type { QualitySetting } from "../../view/quality/qualitySetting";
import { useViewSky } from "./useViewSky";
import { startInstrumentRun, startServerRun, type ViewRun } from "./viewRun";

const UNIVERSE = "000000000000002a";

const SCENE = aViewScene();

/** The scene's system where the chart puts it, so that its sky can be asked. */
function chartedPlace(): SystemPlace {
  if (SCENE.barycentre === null) {
    throw new Error("the fixture scene has no barycentre");
  }
  return {
    kind: "charted",
    system: FIXTURE_SYSTEM,
    designation: "TEST SYSTEM",
    barycentre: SCENE.barycentre,
  };
}

/** A camera view of the server's scene (R02's `camera` role, as an instrument's) at 60°. */
function cameraRun(): ViewRun {
  const run = startInstrumentRun(startServerRun(SCENE));
  return { ...run, camera: { ...run.camera, fovDeg: 60 } };
}

const RUN = cameraRun();

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

/** One bright star 100 ly away, ahead of the camera. */
const ONE_STAR: ReadonlyArray<FixtureStar> = [{ direction: [0, 0, -1], distanceLy: 100, vMag: 1 }];

/**
 * Plays the server's answer to the latest sky request, with `stars` listed, and lets the decode
 * settle.
 */
async function answerSky(
  socket: FakeWebSocket,
  stars: ReadonlyArray<FixtureStar> = ONE_STAR,
): Promise<void> {
  const sent = socket.requestsOfKind("sky").at(-1);
  if (sent === undefined) {
    throw new Error("no sky was asked");
  }
  const payload = skyPayload(stars, 2, null);
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
  ])("asks at the camera's deepest limit, V 10.06 at 60°, under $level", ({ exposure }) => {
    const { socket } = renderViewSky(exposure);
    const limitV = socket.requestsOfKind("sky").at(-1)?.body.camera_limit_v ?? Number.NaN;
    expect(Math.round(limitV * 100) / 100).toBe(10.06);
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

  it("labels the camera's limit at the exposure shown: V 10.1 at MAN −1, V 2.6 at AUTO 15", async () => {
    const { result, rerender, socket } = renderViewSky(DEFAULT_EXPOSURE);
    await answerSky(socket);
    const atDefault = result.current.labelValue;
    rerender({ exposure: autoAt(15) });
    expect([atDefault, result.current.labelValue]).toEqual([
      "V 10.1 mag CAM · CLUSTERS: NOT YET MODELLED",
      "V 2.6 mag CAM · CLUSTERS: NOT YET MODELLED",
    ]);
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
