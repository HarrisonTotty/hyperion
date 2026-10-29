import { type BodyIdHex, PROTOCOL_VERSION } from "@hyperion/protocol";
import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import type { SceneSurfaceEvent } from "../../lib/scene/useScene";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { FIXTURE_EARTH, FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { answerServedScene, answerServedSky } from "../../test/servedHandoverFixtures";
import { statusLine } from "../../test/statusLine";
import { requestAdapterOutcome } from "../engine/platform";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../engine/status";
import { TERRAIN_HOST_DISCS } from "../scenes/terrainScene";
import type { TerrainPool } from "../terrain/terrainPass";
import { FieldNotServed, type MeasureDescent } from "./bodyDescent";
import { surfaceNotServed } from "./DescentSpike";
import {
  READING_SERVED,
  SERVED_WORLD_NOT_READ,
  ServedDescentSpike,
  SurfaceRelay,
} from "./ServedDescentSpike";
import type { SpikeWorkers } from "./spikeRun";
import type { SpikeWorld } from "./spikeWorld";

const UNIVERSE = "000000000000002a";
const STORE = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));

/** A pool that bakes nothing, for the instruments' unused test-planet passes. */
const IDLE_POOL: TerrainPool = {
  reprioritise: () => undefined,
  onBaked: () => () => undefined,
  terminate: () => undefined,
};

const WORKERS: SpikeWorkers = {
  query: () => {
    throw new Error("a served world asks no surface query");
  },
  pool: () => () => IDLE_POOL,
  height: () => new ScriptedHeightWorker(),
};

/** A ready engine on a fake SwiftShader adapter. */
const READY_ENGINE: ViewEngineSource = {
  requestAdapter: () =>
    requestAdapterOutcome(new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })])),
  load: () => countingRenderEngine(),
};

/** Plays the server's part, then lets the outcomes it settles, and their effects, run on. */
async function play(serve: () => void = () => undefined): Promise<void> {
  await act(async () => {
    serve();
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** What a render of the served spike asked and was told. */
interface Rendered {
  readonly socket: FakeWebSocket;
  readonly measured: SpikeWorld[];
  readonly failed: string[];
}

/** The served spike over `body`, its measurement `measure`, its socket opened, not yet welcomed. */
function renderServed(
  body: BodyIdHex,
  measure: MeasureDescent = () => new Promise(() => undefined),
): Rendered {
  const measured: SpikeWorld[] = [];
  const failed: string[] = [];
  render(
    <GraphicsStatusContext value={STORE}>
      <ServedDescentSpike
        serverUrl="ws://127.0.0.1:7879/ws"
        universe={UNIVERSE}
        body={body}
        seed={7n}
        setting="low"
        ridges="off"
        workers={1}
        engineSource={READY_ENGINE}
        spikeWorkers={WORKERS}
        measure={(session, world, seed, signal) => {
          measured.push(world);
          return measure(session, world, seed, signal);
        }}
        listeners={{ onFailed: (status) => failed.push(status) }}
      />
    </GraphicsStatusContext>,
  );
  return { socket: FakeWebSocket.latest(), measured, failed };
}

/** The served spike over `body`, welcomed and its scene opened on Earth's frame, its sky answered. */
async function readServed(body: BodyIdHex, measure?: MeasureDescent): Promise<Rendered> {
  const rendered = renderServed(body, measure);
  const { socket } = rendered;
  await play(() => {
    socket.serverWelcomes(23);
  });
  await play(() => {
    answerServedScene(socket);
  });
  if (socket.requestsOfKind("sky").length > 0) {
    await play(() => {
      answerServedSky(socket);
    });
  }
  await play();
  return rendered;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("WebSocket", FakeWebSocket);
  vi.spyOn(console, "error").mockImplementation(() => undefined);
});

afterEach(() => {
  FakeWebSocket.instances = [];
  vi.useRealTimers();
});

describe("the descent spike over a served world (R10.T15.b)", () => {
  it("reads the body from the server's scene, lights it by the server's sky, and measures over it", async () => {
    const { socket, measured, failed } = await readServed(FIXTURE_EARTH);
    expect(socket.requestsOfKind("subscribe").map(({ body }) => body)).toEqual([
      expect.objectContaining({ kind: "subscribe", universe: UNIVERSE }),
    ]);
    expect(socket.requestsOfKind("sky").map(({ body }) => body.exclude_system)).toEqual([
      FIXTURE_SYSTEM,
    ]);
    expect(measured.map(({ name, body, system }) => [name, body, system])).toEqual([
      ["served", FIXTURE_EARTH, FIXTURE_SYSTEM],
    ]);
    expect(measured[0]?.hostDiscs).toEqual(TERRAIN_HOST_DISCS);
    expect(failed).toEqual([]);
    expect(
      screen.getByText(statusLine("MEASURING TERRAIN: landing site and ground track")),
    ).toBeDefined();
  });

  it("stands its frame, under the measurement banner, and shows the link's state while it links", () => {
    renderServed(FIXTURE_EARTH);
    expect(screen.getByRole("heading", { level: 1, name: "DESCENT SPIKE" })).toBeDefined();
    expect(screen.getByRole("status", { name: "Mode" }).textContent).toBe("MEASUREMENT");
    expect(screen.getByRole("status", { name: "Server link" }).textContent).toBe(
      "ESTABLISHING LINK",
    );
    expect(screen.getByText(statusLine("ESTABLISHING LINK"))).toBeDefined();
  });

  it("says no carrier while the server does not answer, ending nothing", async () => {
    const { socket, failed } = renderServed(FIXTURE_EARTH);
    await play(() => {
      socket.close();
    });
    expect(screen.getByText(statusLine("NO CARRIER"))).toBeDefined();
    expect(failed).toEqual([]);
  });

  it("says it is reading the served world once the link is up", async () => {
    const { socket } = renderServed(FIXTURE_EARTH);
    await play(() => {
      socket.serverWelcomes(23);
    });
    expect(screen.getByText(statusLine(READING_SERVED))).toBeDefined();
  });

  it("names a body that is no world of the scene, and another body as the remedy", async () => {
    const body = `${FIXTURE_SYSTEM}.0900`;
    const { measured, failed } = await readServed(body);
    const said = `TERRAIN NOT MEASURED: ${body} is no served world (not in the scene), relaunch with another body`;
    expect(screen.getByText(statusLine(said))).toBeDefined();
    expect([measured, failed]).toEqual([[], [said]]);
  });

  it("states the scene's refusal with its reason, telling the run once", async () => {
    const { socket, failed } = renderServed(FIXTURE_EARTH);
    await play(() => {
      socket.serverWelcomes(23);
    });
    await play(() => {
      const [subscribe] = socket.requestsOfKind("subscribe");
      if (subscribe === undefined) {
        throw new Error("the spike asked for no scene");
      }
      socket.serverRejects(subscribe.id, {
        code: "unknown_universe",
        message: "no universe 000000000000002a",
        field: "universe",
      });
    });
    const said = "SCENE REJECTED: no universe 000000000000002a, relaunch to retry";
    expect(screen.getByText(statusLine(said))).toBeDefined();
    await play();
    expect(failed).toEqual([said]);
  });

  it("states a scene that was never answered", async () => {
    const { socket, failed } = renderServed(FIXTURE_EARTH);
    await play(() => {
      socket.serverWelcomes(23);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000);
    });
    expect(failed).toEqual(["SCENE TIMED OUT: relaunch to retry"]);
  });

  it("states an incompatible server as the link's state, and ends the run", async () => {
    const { socket, failed } = renderServed(FIXTURE_EARTH);
    await play(() => {
      socket.serverOpens();
      socket.serverSends({
        type: "welcome",
        server_version: "9.9.9",
        protocol_version: PROTOCOL_VERSION + 1,
        generator_version: 23,
      });
    });
    expect(screen.getByText(statusLine("LINK INCOMPATIBLE"))).toBeDefined();
    expect(failed).toEqual(["LINK INCOMPATIBLE"]);
  });

  it("says the served world was not read where the sky gives no discs", async () => {
    const { socket, failed } = renderServed(FIXTURE_EARTH);
    await play(() => {
      socket.serverWelcomes(23);
    });
    await play(() => {
      answerServedScene(socket);
    });
    await play(() => {
      const [sky] = socket.requestsOfKind("sky");
      if (sky === undefined) {
        throw new Error("the spike asked for no sky");
      }
      socket.serverRejects(sky.id, { code: "queue_full", message: "busy", field: null });
    });
    expect(screen.getByText(statusLine(SERVED_WORLD_NOT_READ))).toBeDefined();
    expect(failed).toEqual([SERVED_WORLD_NOT_READ]);
  });

  it("says plainly that the body's surface is not yet modelled, naming it, and ends failed", async () => {
    const { measured, failed } = await readServed(FIXTURE_EARTH, () =>
      Promise.reject(new FieldNotServed(FIXTURE_EARTH, "surface")),
    );
    await play();
    const designation = measured[0]?.bodyDesignation ?? "";
    const said = surfaceNotServed(designation);
    expect(said).toBe(
      `TERRAIN NOT MEASURED: the surface of ${designation} is not yet modelled at this generator version`,
    );
    const line = screen.getByText(statusLine(said));
    // A data state, not a fault: plain text, not caution.
    expect(line.querySelector(".request-status__text--fault")).toBeNull();
    expect(line.classList.contains("request-status__text--fault")).toBe(false);
    expect(failed).toEqual([said]);
  });
});

describe("the scene's surface events, relayed", () => {
  it("reach each listener until it unsubscribes", () => {
    const relay = new SurfaceRelay();
    const heard: SceneSurfaceEvent[] = [];
    const stop = relay.onSurface((event) => heard.push(event));
    relay.emit({ kind: "resync" });
    stop();
    relay.emit({ kind: "resync" });
    expect(heard).toEqual([{ kind: "resync" }]);
  });
});
