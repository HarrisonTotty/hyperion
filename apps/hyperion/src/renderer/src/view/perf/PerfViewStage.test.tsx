import { PROTOCOL_VERSION } from "@hyperion/protocol";
import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aUniverse, aUniverseList } from "../../test/galaxyFixtures";
import { REQUEST_TIMEOUT_MS } from "../../lib/useServerRequest";
import { requestAdapterOutcome } from "../engine/platform";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../engine/status";
import { PerfViewStage, SCENE_RETRIES } from "./PerfViewStage";
import type { PerfFailure } from "./runPerfScene";

const UNIVERSE = "000000000000002a";
const STORE = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));

/** A ready engine on a fake SwiftShader adapter. */
const READY_ENGINE: ViewEngineSource = {
  requestAdapter: () =>
    requestAdapterOutcome(new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })])),
  load: () => countingRenderEngine(),
};

/** Plays the server's part, then lets what it settles run on. */
async function play(serve: () => void = () => undefined): Promise<void> {
  await act(async () => {
    serve();
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The stage over a fake server, its socket not yet opened, and the failures it told. */
function staged(): { readonly socket: FakeWebSocket; readonly failures: PerfFailure[] } {
  const failures: PerfFailure[] = [];
  render(
    <GraphicsStatusContext value={STORE}>
      <PerfViewStage
        serverUrl="ws://127.0.0.1:7879/ws"
        universe={UNIVERSE}
        setting="low"
        perf={{ cameraAt: () => null, onDrawn: () => undefined }}
        watchTerrain={() => () => undefined}
        onFailed={(failure) => failures.push(failure)}
        engineSource={READY_ENGINE}
        performanceThreads={null}
        platform="linux"
      />
    </GraphicsStatusContext>,
  );
  return { socket: FakeWebSocket.latest(), failures };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  FakeWebSocket.instances = [];
  vi.useRealTimers();
});

describe("a perf run's mount of VIEW", () => {
  it("stands under VIEW's title and the MEASUREMENT banner", () => {
    staged();
    expect([
      screen.getByRole("heading", { name: "VIEW" }).tagName,
      screen.getByRole("status", { name: "Mode" }).textContent,
    ]).toEqual(["H1", "MEASUREMENT"]);
  });

  it("takes the arrival's universe from the server's list, opening none, and asks its scene", async () => {
    const { socket, failures } = staged();
    await play(() => {
      socket.serverWelcomes(32);
    });
    await play(() => {
      socket.serverAnswers("list_universes", () =>
        aUniverseList([aUniverse(), aUniverse({ id: UNIVERSE, name: "HYPERION PERF" })]),
      );
    });
    await play();
    expect([
      socket.requestsOfKind("open_universe"),
      socket
        .requestsOfKind("subscribe")
        .some(({ body }) => JSON.stringify(body).includes(UNIVERSE)),
      failures,
    ]).toEqual([[], true, []]);
  });

  it("asks again for a scene left unanswered, and ends the run after its last ask", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { socket, failures } = staged();
    await play(() => {
      socket.serverWelcomes(32);
    });
    await play(() => {
      socket.serverAnswers("list_universes", () =>
        aUniverseList([aUniverse({ id: UNIVERSE, name: "HYPERION PERF" })]),
      );
    });
    const subscriptions = (): number =>
      socket
        .requestsOfKind("subscribe")
        .filter(({ body }) => JSON.stringify(body).includes(UNIVERSE)).length;
    const asked = [subscriptions()];
    for (let ask = 0; ask <= SCENE_RETRIES; ask += 1) {
      // oxlint-disable-next-line no-await-in-loop
      await play(() => {
        vi.advanceTimersByTime(REQUEST_TIMEOUT_MS + 1);
      });
      asked.push(subscriptions());
    }
    expect(asked).toEqual([1, 2, 3, 4, 4]);
    expect(failures.map(({ shown }) => shown)).toEqual([
      "the server did not answer the scene, relaunch to retry",
    ]);
  });

  it("ends the run once on a list that lacks the arrival's universe", async () => {
    const { socket, failures } = staged();
    await play(() => {
      socket.serverWelcomes(32);
    });
    await play(() => {
      socket.serverAnswers("list_universes", () => aUniverseList([aUniverse()]));
    });
    await play();
    expect(failures).toEqual([
      {
        reason: `the server lists no universe ${UNIVERSE}, the arrival's`,
        shown: "the server lists no arrival's universe, relaunch to retry",
      },
    ]);
  });

  it("ends the run on a list the server refuses, with the server's reason", async () => {
    const { socket, failures } = staged();
    await play(() => {
      socket.serverWelcomes(32);
    });
    await play(() => {
      const [list] = socket.requestsOfKind("list_universes");
      socket.serverRejects(list?.id ?? -1, {
        code: "storage_failed",
        message: "the registry could not be read",
        field: null,
      });
    });
    await play();
    expect(failures.map(({ shown }) => shown)).toEqual([
      "the server refused its universe list (the registry could not be read), relaunch to retry",
    ]);
  });

  it("ends the run on a server of another protocol version, with the remedy", async () => {
    const { socket, failures } = staged();
    await play(() => {
      socket.serverOpens();
      socket.serverSends({
        type: "welcome",
        server_version: "9.9.9",
        protocol_version: PROTOCOL_VERSION + 1,
        generator_version: 32,
      });
    });
    expect(failures).toEqual([
      {
        reason: "the scene's server speaks another protocol version",
        shown: "server of another protocol version, relaunch with a matching server",
      },
    ]);
  });
});
