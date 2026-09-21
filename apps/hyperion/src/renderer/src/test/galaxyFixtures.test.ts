import { decodeDensityMap } from "@hyperion/protocol";
import { beforeEach, describe, expect, it } from "vitest";

import { FakeWebSocket } from "./FakeWebSocket";
import { aDensityMap, aSystemsInRange, aUniverse, aUniverseList } from "./galaxyFixtures";

/** Sends a request on `socket` as the client would. */
function clientRequests(socket: FakeWebSocket, id: number): void {
  socket.send(`{"type":"request","id":${id},"body":{"kind":"list_universes"}}`);
}

/** Records what the fake delivers to the client. */
function receivedMessages(socket: FakeWebSocket): unknown[] {
  const received: unknown[] = [];
  socket.addEventListener("message", (event) => {
    if (event instanceof MessageEvent && typeof event.data === "string") {
      received.push(JSON.parse(event.data));
    }
  });
  return received;
}

describe("FakeWebSocket", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
  });

  it("lists the requests of one kind with their IDs", () => {
    const socket = new FakeWebSocket("ws://ship/ws");
    clientRequests(socket, 1);
    socket.send(
      `{"type":"request","id":2,"body":{"kind":"open_universe","universe":"00000000000000a1"}}`,
    );
    socket.send(`{"type":"cancel","id":1}`);
    clientRequests(socket, 3);

    expect(socket.requestsOfKind("list_universes")).toEqual([
      { id: 1, body: { kind: "list_universes" } },
      { id: 3, body: { kind: "list_universes" } },
    ]);
    expect(socket.requestsOfKind("open_universe")).toEqual([
      { id: 2, body: { kind: "open_universe", universe: "00000000000000a1" } },
    ]);
    expect(socket.cancelledIds()).toEqual([1]);
  });

  it("answers the latest unanswered request of a kind, by its ID", () => {
    const socket = new FakeWebSocket("ws://ship/ws");
    const received = receivedMessages(socket);
    clientRequests(socket, 4);
    clientRequests(socket, 5);

    expect(socket.serverAnswers("list_universes", () => aUniverseList())).toBe(5);
    expect(socket.serverAnswers("list_universes", () => aUniverseList([]))).toBe(4);

    expect(received).toEqual([
      { type: "response", id: 5, body: aUniverseList() },
      { type: "response", id: 4, body: aUniverseList([]) },
    ]);
  });

  it("skips a request the server has already rejected", () => {
    const socket = new FakeWebSocket("ws://ship/ws");
    clientRequests(socket, 1);
    clientRequests(socket, 2);
    socket.serverRejects(2, { code: "queue_full", message: "busy", field: null });

    expect(socket.serverAnswers("list_universes", () => aUniverseList())).toBe(1);
  });

  it("passes the request's body to the builder", () => {
    const socket = new FakeWebSocket("ws://ship/ws");
    socket.send(
      `{"type":"request","id":7,"body":{"kind":"open_universe","universe":"00000000000000b2"}}`,
    );

    const received = receivedMessages(socket);
    socket.serverAnswers("open_universe", (body) => ({
      kind: "open_universe",
      ...aUniverse({ id: body.universe }),
    }));

    expect(received).toEqual([
      {
        type: "response",
        id: 7,
        body: { kind: "open_universe", ...aUniverse({ id: "00000000000000b2" }) },
      },
    ]);
  });

  it("names the kind when no request of it is waiting", () => {
    const socket = new FakeWebSocket("ws://ship/ws");
    clientRequests(socket, 1);
    socket.serverAnswers("list_universes", () => aUniverseList());

    expect(() => socket.serverAnswers("list_universes", () => aUniverseList())).toThrow(
      "no unanswered list_universes request has been sent",
    );
    expect(() =>
      socket.serverAnswers("galaxy_parameters", () => {
        throw new Error("never built");
      }),
    ).toThrow("no unanswered galaxy_parameters request has been sent");
  });
});

describe("galaxy fixtures", () => {
  it("build a density map that the protocol's decoder reads back", () => {
    const codes = Array.from({ length: 32 }, (_, index) => index * 8);
    const map = aDensityMap({ codes, floorLog10PerLy2: -4, ceilingLog10PerLy2: 1 });

    const decoded = decodeDensityMap(map);

    expect(decoded.widthPx).toBe(8);
    expect(decoded.heightPx).toBe(4);
    expect([...decoded.codes]).toEqual(codes);
    expect(decoded.maxCode).toBe(255);
    expect(decoded.log10PerLy2(0)).toBeNull();
    expect(decoded.log10PerLy2(1)).toBe(-4);
    expect(decoded.log10PerLy2(255)).toBe(1);
  });

  it("refuse codes that do not fill the map", () => {
    expect(() => aDensityMap({ codes: [1, 2, 3] })).toThrow("3 codes do not fill a 8 × 4 map");
  });

  it("build a census consistent with the systems returned", () => {
    const response = aSystemsInRange({
      minLayer: "b",
      systems: [
        { relLy: [1, 0, 0], layer: "b" },
        { relLy: [2, 0, 0], layer: "b" },
        { relLy: [3, 0, 0], layer: "e" },
      ],
    });

    expect(response.census.complete_above_msun).toBe(0.5);
    expect(
      response.census.layers.map((layer) => [layer.layer, layer.status, layer.returned]),
    ).toEqual([
      ["a", "below_mass_floor", 0],
      ["b", "included", 2],
      ["c", "included", 0],
      ["d", "included", 0],
      ["e", "included", 1],
    ]);
    expect(response.systems.map((system) => system.id)).toEqual([
      "0000000000000001",
      "0000000000000002",
      "0000000000000003",
    ]);
  });

  it("build a census in which nothing fits when every layer is over the limit", () => {
    const { census } = aSystemsInRange({ overLimit: ["a", "b", "c", "d", "e"] });

    expect(census.complete_above_msun).toBeNull();
    expect(census.layers.every((layer) => layer.status === "over_limit")).toBe(true);
  });
});
