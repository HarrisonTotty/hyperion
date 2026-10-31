import {
  type BodyIdHex,
  decodeServerMessage,
  encodeClientMessage,
  RequestClient,
} from "@hyperion/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../../../test/FakeWebSocket";
import { FIXTURE_EARTH, FIXTURE_SYSTEM } from "../../../test/planetaryFixture";
import { moonLikeHeader } from "../../../test/coarseFieldFixtures";
import {
  answerServedField,
  answerServedScene,
  answerServedSky,
} from "../../../test/servedHandoverFixtures";
import { readServedHandoverWorld, type ServedHandoverRead } from "./servedHandover";

const UNIVERSE = "000000000000002a";

/** A request client over a fake socket the test plays the server on, through the wire's codec. */
function linked(): { readonly client: RequestClient; readonly socket: FakeWebSocket } {
  const socket = new FakeWebSocket("ws://127.0.0.1:7878/ws");
  socket.binaryType = "arraybuffer";
  const client = new RequestClient((message) => {
    socket.send(encodeClientMessage(message));
    return true;
  });
  socket.addEventListener("message", (event: Event) => {
    if (!(event instanceof MessageEvent)) {
      return;
    }
    const data: unknown = event.data;
    if (data instanceof ArrayBuffer) {
      client.handleBinaryFrame(data);
    } else if (typeof data === "string") {
      client.handleServerMessage(decodeServerMessage(data));
    }
  });
  return { client, socket };
}

/** The world a read gave, or a failure naming what it gave instead. */
function worldOf(read: ServedHandoverRead): Extract<ServedHandoverRead, { kind: "read" }>["world"] {
  if (read.kind !== "read") {
    throw new Error(`the read gave ${JSON.stringify(read)}`);
  }
  return read.world;
}

/** Lets the client's settled outcomes reach the reading's next step. */
async function settle(): Promise<void> {
  await vi.advanceTimersByTimeAsync(0);
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the hand-over check's served world, read from the server (R10.T15.b)", () => {
  it("reads the body from the scene, its discs from the sky and its survey from its field", async () => {
    const { client, socket } = linked();
    const reading = readServedHandoverWorld(
      client,
      UNIVERSE,
      FIXTURE_EARTH,
      new AbortController().signal,
    );
    answerServedScene(socket);
    await settle();
    expect(socket.requestsOfKind("unsubscribe")).toHaveLength(1);
    answerServedSky(socket);
    await settle();
    answerServedField(socket, FIXTURE_EARTH, 64);
    const world = worldOf(await reading);
    const header = moonLikeHeader(FIXTURE_EARTH);
    expect([world.body, world.reliefM, world.cellLevel, world.surveyedCells]).toEqual([
      FIXTURE_EARTH,
      header.realised_relief_m,
      header.level,
      64,
    ]);
    expect(world.figure).toEqual({
      equatorialRadiusM: header.equatorial_radius_m,
      polarRadiusM: header.polar_radius_m,
    });
    expect(socket.requestsOfKind("surface_field").map(({ body }) => body)).toEqual([
      { kind: "surface_field", universe: UNIVERSE, body: FIXTURE_EARTH, have_revision: null },
    ]);
  });

  it("gives the body's field not modelled, naming the section the server names", async () => {
    const { client, socket } = linked();
    const reading = readServedHandoverWorld(
      client,
      UNIVERSE,
      FIXTURE_EARTH,
      new AbortController().signal,
    );
    answerServedScene(socket);
    await settle();
    answerServedSky(socket);
    await settle();
    socket.serverAnswers("surface_field", () => ({
      kind: "surface_field",
      state: "not_modelled",
      section: "surface",
    }));
    expect(await reading).toEqual({ kind: "not served", section: "surface" });
  });

  it("fails naming the gap where the body is no served world of the scene", async () => {
    const { client, socket } = linked();
    const body: BodyIdHex = `${FIXTURE_SYSTEM}.0900`;
    const reading = readServedHandoverWorld(client, UNIVERSE, body, new AbortController().signal);
    answerServedScene(socket);
    expect(await reading).toEqual({
      kind: "failed",
      message: `${body} is no served world in the scene: not in the scene`,
    });
  });

  it("fails where the field holds no surveyed cell", async () => {
    const { client, socket } = linked();
    const reading = readServedHandoverWorld(
      client,
      UNIVERSE,
      FIXTURE_EARTH,
      new AbortController().signal,
    );
    answerServedScene(socket);
    await settle();
    answerServedSky(socket);
    await settle();
    // Every cell held is a margin's, code 0.
    answerServedField(socket, FIXTURE_EARTH, 64, 0);
    const read = await reading;
    expect(read.kind === "failed" ? read.message : read.kind).toContain("holds no surveyed cell");
  });

  it("cancels the request in flight once its signal aborts", async () => {
    const { client, socket } = linked();
    const abort = new AbortController();
    const reading = readServedHandoverWorld(client, UNIVERSE, FIXTURE_EARTH, abort.signal);
    abort.abort();
    expect(await reading).toMatchObject({ kind: "failed" });
    expect(socket.cancelledIds()).toEqual(socket.requestsOfKind("subscribe").map(({ id }) => id));
  });
});
