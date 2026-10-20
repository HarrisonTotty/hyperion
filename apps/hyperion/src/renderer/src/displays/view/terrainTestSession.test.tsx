import { RequestClient } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { type FieldMessage, ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import type { RenderEngine } from "../../view/engine/types";
import { blockBytes } from "../../view/scenes/terrainGolden";
import {
  TERRAIN_SCENE_NAME,
  TERRAIN_TEST_SCRIPT,
  terrainScene,
  terrainSceneFixture,
  terrainTestGolden,
} from "../../view/scenes/terrainScene";
import type { TerrainTestFixture } from "../../view/scenes/terrainFixture";
import type { TerrainSession, TerrainViewReport } from "../../view/terrain/session";
import type { WorkerLike } from "../../view/terrain/workers/pool";
import { terrainSessionSource, useTerrainSession } from "./useTerrainSession";
import { PRIMARY_VIEW_ID } from "./viewNames";
import type { SceneOption } from "./viewRun";

const scene = terrainScene();

/** The primary's report of `TERRAIN TEST` at script time `tS`, from its seat, photorealistic. */
function reportAt(tS: number): TerrainViewReport {
  return {
    scene: scene.sceneAt(tS),
    pose: scene.cameraAt(tS),
    fovXRad: Math.PI / 3,
    presentedPx: { widthPx: 1920, heightPx: 1080 },
    style: "photorealistic",
    drawnStyle: "photorealistic",
    setting: "high",
    streamPriority: "primary",
    regimes: new Map(),
  };
}

/** `VIEW`'s terrain session for a `TERRAIN TEST` option of its own, over two scripted workers. */
function renderTerrainSession(engine: RenderEngine) {
  const fixture: TerrainTestFixture = terrainSceneFixture();
  const option: SceneOption = { name: TERRAIN_SCENE_NAME, make: terrainScene, terrain: fixture };
  const workers: ScriptedHeightWorker[] = [];
  const createWorker = (): WorkerLike => {
    const worker = new ScriptedHeightWorker();
    workers.push(worker);
    return worker;
  };
  const hook = renderHook(
    () =>
      useTerrainSession({
        engineState: { kind: "ready", engine },
        source: terrainSessionSource({ kind: "kept", option }),
        createWorker,
        workers: 2,
      }),
    { wrapper: ServerLinkHarness },
  );
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  const session = hook.result.current;
  if (session === null) {
    throw new Error("TERRAIN TEST has a terrain session");
  }
  return { ...hook, fixture, workers, session };
}

/** Plays the script to `tS`: the fixture told, the primary's report noted, the session stepped. */
async function playTo(fixture: TerrainTestFixture, session: TerrainSession, tS: number) {
  fixture.notePlayed(tS);
  session.noteView(PRIMARY_VIEW_ID, reportAt(tS));
  session.step();
  // The fixture's chunks and answers, each a task after the last; the link's own timers (its
  // heartbeat) run on, so the clock is moved on a little rather than to every timer's end.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(100);
  });
}

/** The bytes of each block a worker was posted, with the message's kind, oldest first. */
function posted(messages: ReadonlyArray<FieldMessage>): Array<readonly [string, number[]]> {
  return messages.flatMap((message) =>
    message.kind === "field-release"
      ? []
      : [[message.kind, Array.from(new Uint8Array(message.block))]],
  );
}

/** Payload `n`'s blocks (from 1) as a session posts them: block 0 a start or a chunk. */
function blocksOf(n: number, started: boolean): Array<readonly [string, number[]]> {
  return (terrainTestGolden().payloads[n - 1]?.blocksHex ?? []).map(
    (hex, index) =>
      [
        index === 0 && !started ? "field-init" : "field-chunk",
        Array.from(blockBytes(hex)),
      ] as const,
  );
}

let engine: RenderEngine;

beforeEach(async () => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
  engine = await countingRenderEngine();
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("TERRAIN TEST's terrain session (R10.T16.d)", () => {
  it("asks its fields of the fixture, sending no frame over the link", async () => {
    const { fixture, session, workers, unmount } = renderTerrainSession(engine);
    await playTo(fixture, session, 0);
    const socket = FakeWebSocket.latest();
    expect(
      socket.sent.filter(
        (message) =>
          typeof message === "object" &&
          message !== null &&
          "type" in message &&
          (message.type === "request" || message.type === "cancel"),
      ),
    ).toEqual([]);
    expect(posted(workers[0]?.fieldMessages ?? [])).toEqual(blocksOf(1, false));
    unmount();
  });

  it("takes TEST MOON's detail seed from the scene, not from body_detail", async () => {
    const { fixture, session, unmount } = renderTerrainSession(engine);
    await playTo(fixture, session, 0);
    const golden = terrainTestGolden();
    expect([...session.wanted]).toEqual([golden.body]);
    expect(session.detailSeed(golden.body)).toBe(golden.seed);
    unmount();
  });

  it("brings the first payload, then the delta at the revision's rise, to the workers", async () => {
    const { fixture, session, workers, unmount } = renderTerrainSession(engine);
    const asked = vi.spyOn(RequestClient.prototype, "requestBulk");
    await playTo(fixture, session, 0);
    workers.forEach((worker) => {
      worker.answerFields();
    });
    const moon = terrainTestGolden().body;
    expect(session.survey(moon)?.coverage?.revision).toBe(1);
    await playTo(fixture, session, TERRAIN_TEST_SCRIPT.deltaAtS);
    expect(posted(workers[0]?.fieldMessages ?? [])).toEqual([
      ...blocksOf(1, false),
      ...blocksOf(2, true),
    ]);
    expect(session.survey(moon)?.coverage?.revision).toBe(2);
    expect(
      asked.mock.calls.map(([body]) => ("have_revision" in body ? body.have_revision : 0)),
    ).toEqual([null, 1]);
    unmount();
  });

  it("asks for no field again and lowers no revision over a loop of the script", async () => {
    const { fixture, session, workers, unmount } = renderTerrainSession(engine);
    const asked = vi.spyOn(RequestClient.prototype, "requestBulk");
    for (let tS = 0; tS <= TERRAIN_TEST_SCRIPT.durationS; tS += 20) {
      // Each play waits on the last.
      // oxlint-disable-next-line no-await-in-loop
      await playTo(fixture, session, tS);
    }
    const blocks = workers[0]?.fieldMessages.length;
    // The script runs again from its start, past the delta's time, and is scrubbed back.
    for (const tS of [0, 40, 140, 280, 10]) {
      // oxlint-disable-next-line no-await-in-loop
      await playTo(fixture, session, tS);
    }
    const moon = terrainTestGolden().body;
    expect([
      asked.mock.calls.length,
      workers[0]?.fieldMessages.length,
      session.survey(moon)?.coverage?.revision,
    ]).toEqual([2, blocks, 2]);
    unmount();
  });
});
