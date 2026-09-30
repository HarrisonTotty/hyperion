import { describe, expect, it } from "vitest";

import type { ClientMessage } from "./generated/ClientMessage";
import type { ResponseBody } from "./generated/ResponseBody";
import type { UniverseInfo } from "./generated/UniverseInfo";
import {
  MAX_REQUEST_ID,
  nextFreeRequestId,
  RequestChannel,
  RequestClient,
  type RequestOf,
  setLastRequestId,
} from "./requests";

const UNIVERSE = "0123456789abcdef";

const TALOS: UniverseInfo = {
  id: UNIVERSE,
  name: "Talos",
  seed: "00000000000004d2",
  generator_version: 2,
  status: "compatible",
};

const LIST: ResponseBody = {
  kind: "list_universes",
  universes: [TALOS],
  server_generator_version: 2,
};

/** A client whose `send` records every message, with a link that can be taken down. */
function recordingClient(): {
  client: RequestClient;
  sent: ClientMessage[];
  setLinkUp: (up: boolean) => void;
} {
  const sent: ClientMessage[] = [];
  let linkUp = true;
  const client = new RequestClient((message) => {
    if (!linkUp) {
      return false;
    }
    sent.push(message);
    return true;
  });
  return {
    client,
    sent,
    setLinkUp: (up) => {
      linkUp = up;
    },
  };
}

/** The ID of the `index`-th request sent. */
function requestId(sent: readonly ClientMessage[], index: number): number {
  const message = sent.filter((each) => each.type === "request")[index];
  if (message === undefined) {
    throw new Error(`no request number ${index} was sent`);
  }
  return message.id;
}

/** Whether `promise` has settled, checked after the pending microtasks run. */
async function hasSettled(promise: Promise<unknown>): Promise<boolean> {
  const stillPending = Symbol("still pending");
  const first = await Promise.race([promise, Promise.resolve(stillPending)]);
  return first !== stillPending;
}

describe("RequestClient", () => {
  it("sends a request at once and resolves with the typed response", async () => {
    const { client, sent } = recordingClient();

    const pending = client.request({ kind: "list_universes" });

    expect(sent).toEqual([{ type: "request", id: 1, body: { kind: "list_universes" } }]);
    expect(client.handleServerMessage({ type: "response", id: 1, body: LIST })).toBe(true);
    const outcome = await pending.outcome;
    if (!outcome.ok) {
      throw new Error(`expected a response, got ${outcome.error.code}`);
    }
    expect(outcome.response.universes).toEqual([TALOS]);
  });

  it("passes a server error through with its code", async () => {
    const { client } = recordingClient();

    const pending = client.request({ kind: "open_universe", universe: UNIVERSE });
    client.handleServerMessage({
      type: "request_error",
      id: 1,
      error: { code: "unknown_universe", message: "no such universe", field: null },
    });

    expect(await pending.outcome).toEqual({
      ok: false,
      error: { code: "unknown_universe", message: "no such universe", field: null },
    });
  });

  it("reports a response of another kind as a protocol violation", async () => {
    const { client } = recordingClient();

    const pending = client.request({ kind: "open_universe", universe: UNIVERSE });
    client.handleServerMessage({ type: "response", id: 1, body: LIST });

    const outcome = await pending.outcome;
    expect(outcome.ok).toBe(false);
    expect(outcome.ok ? null : outcome.error.code).toBe("protocol_violation");
  });

  it("ignores an answer to an unknown ID", async () => {
    const { client } = recordingClient();

    const pending = client.request({ kind: "list_universes" });

    expect(client.handleServerMessage({ type: "response", id: 99, body: LIST })).toBe(true);
    expect(await hasSettled(pending.outcome)).toBe(false);
  });

  it("leaves messages that are not request traffic to the caller", () => {
    const { client } = recordingClient();

    expect(
      client.handleServerMessage({
        type: "welcome",
        server_version: "0.1.0",
        protocol_version: 2,
        generator_version: 2,
      }),
    ).toBe(false);
    expect(client.handleServerMessage({ type: "pong", nonce: 3 })).toBe(false);
    expect(client.handleServerMessage({ type: "error", message: "malformed" })).toBe(false);
  });

  it("sends one cancel and settles as aborted when cancelled", async () => {
    const { client, sent } = recordingClient();

    const pending = client.request({ kind: "list_universes" });
    pending.cancel();
    pending.cancel();

    expect(sent.filter((message) => message.type === "cancel")).toEqual([
      { type: "cancel", id: 1 },
    ]);
    expect(await pending.outcome).toEqual({
      ok: false,
      error: { code: "aborted", message: "the request was cancelled" },
    });
  });

  it("drops the server's late answer to a cancelled request", async () => {
    const { client } = recordingClient();

    const pending = client.request({ kind: "list_universes" });
    pending.cancel();

    expect(
      client.handleServerMessage({
        type: "request_error",
        id: 1,
        error: { code: "cancelled", message: "cancelled", field: null },
      }),
    ).toBe(true);
    const outcome = await pending.outcome;
    expect(outcome.ok ? null : outcome.error.code).toBe("aborted");
  });

  it("does not cancel a request that has already settled", async () => {
    const { client, sent } = recordingClient();

    const pending = client.request({ kind: "list_universes" });
    client.handleServerMessage({ type: "response", id: 1, body: LIST });
    pending.cancel();

    expect(sent.some((message) => message.type === "cancel")).toBe(false);
    expect((await pending.outcome).ok).toBe(true);
  });

  it("settles every request in flight as link_lost when the link drops", async () => {
    const { client } = recordingClient();

    const first = client.request({ kind: "list_universes" });
    const second = client.request({ kind: "galaxy_parameters", universe: UNIVERSE });
    client.linkLost();

    const outcomes = await Promise.all([first.outcome, second.outcome]);
    expect(outcomes.map((outcome) => (outcome.ok ? null : outcome.error.code))).toEqual([
      "link_lost",
      "link_lost",
    ]);
  });

  it("settles as link_lost at once when the link refuses the request", async () => {
    const { client, sent, setLinkUp } = recordingClient();
    setLinkUp(false);

    const pending = client.request({ kind: "list_universes" });

    expect(sent).toEqual([]);
    const outcome = await pending.outcome;
    expect(outcome.ok ? null : outcome.error.code).toBe("link_lost");
  });

  it("does not reuse an ID while its request is pending", () => {
    const { client, sent } = recordingClient();

    client.request({ kind: "list_universes" });
    for (let index = 1; index <= 10; index += 1) {
      client.request({ kind: "list_universes" });
      client.handleServerMessage({ type: "response", id: requestId(sent, index), body: LIST });
    }

    const ids = sent.flatMap((message) => (message.type === "request" ? [message.id] : []));
    expect(ids).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
  });

  it("wraps its IDs from 2^32 - 1 to 1", () => {
    const { client, sent } = recordingClient();
    client[setLastRequestId](MAX_REQUEST_ID - 1);

    client.request({ kind: "list_universes" });
    client.request({ kind: "list_universes" });

    expect([requestId(sent, 0), requestId(sent, 1)]).toEqual([MAX_REQUEST_ID, 1]);
  });

  it("skips a cancelled request's ID until the server answers it, however often it is cancelled", async () => {
    const { client, sent } = recordingClient();
    client[setLastRequestId](MAX_REQUEST_ID - 1);
    const cancelled = client.request({ kind: "list_universes" });
    cancelled.cancel();
    cancelled.cancel();

    client[setLastRequestId](MAX_REQUEST_ID - 1);
    const next = client.request({ kind: "list_universes" });
    client.handleServerMessage({
      type: "request_error",
      id: MAX_REQUEST_ID,
      error: { code: "cancelled", message: "cancelled", field: null },
    });

    expect([requestId(sent, 0), requestId(sent, 1)]).toEqual([MAX_REQUEST_ID, 1]);
    expect(await hasSettled(next.outcome)).toBe(false);
  });

  it("reuses a cancelled request's ID once the server has answered it", () => {
    const { client, sent } = recordingClient();
    client[setLastRequestId](MAX_REQUEST_ID - 1);
    client.request({ kind: "list_universes" }).cancel();
    client.handleServerMessage({
      type: "request_error",
      id: MAX_REQUEST_ID,
      error: { code: "cancelled", message: "cancelled", field: null },
    });

    client[setLastRequestId](MAX_REQUEST_ID - 1);
    client.request({ kind: "list_universes" });

    expect(requestId(sent, 1)).toBe(MAX_REQUEST_ID);
  });

  it("forgets cancelled IDs when the link drops, since the server forgets them too", () => {
    const { client, sent } = recordingClient();
    client[setLastRequestId](MAX_REQUEST_ID - 1);
    client.request({ kind: "list_universes" }).cancel();
    client.linkLost();

    client[setLastRequestId](MAX_REQUEST_ID - 1);
    client.request({ kind: "list_universes" });

    expect(requestId(sent, 1)).toBe(MAX_REQUEST_ID);
  });

  it("types a request body by its kind", () => {
    const map: RequestOf<"density_map"> = {
      kind: "density_map",
      universe: UNIVERSE,
      view: "face_on",
      population: "all",
      resolution: 128,
      bits: 8,
    };
    const query: RequestOf<"density_map"> = {
      // @ts-expect-error A range query's body is not a density map request.
      kind: "systems_in_range",
      universe: UNIVERSE,
      centre: { cell_ly: [26_000, 0, 0], offset_m: [0, 0, 0] },
      radius_ly: 50,
      time: { seconds: 0, nanos: 0 },
      min_layer: "a",
      limit: 5_000,
    };

    expect([map.kind, query.kind]).toEqual(["density_map", "systems_in_range"]);
  });
});

describe("nextFreeRequestId", () => {
  it("counts up from 1", () => {
    expect(nextFreeRequestId(0, () => false)).toBe(1);
    expect(nextFreeRequestId(41, () => false)).toBe(42);
  });

  it("wraps after 2^32 - 1", () => {
    expect(MAX_REQUEST_ID).toBe(2 ** 32 - 1);
    expect(nextFreeRequestId(MAX_REQUEST_ID, () => false)).toBe(1);
  });

  it("skips IDs still in use, across the wrap", () => {
    const inUse = new Set([MAX_REQUEST_ID, 1, 2]);

    expect(nextFreeRequestId(MAX_REQUEST_ID - 1, (id) => inUse.has(id))).toBe(3);
  });
});

describe("RequestChannel", () => {
  it("cancels its previous request, and only the newer one resolves", async () => {
    const { client, sent } = recordingClient();
    const channel = new RequestChannel(client);

    const older = channel.request({ kind: "open_universe", universe: UNIVERSE });
    const newer = channel.request({ kind: "list_universes" });

    expect(sent).toEqual([
      { type: "request", id: 1, body: { kind: "open_universe", universe: UNIVERSE } },
      { type: "cancel", id: 1 },
      { type: "request", id: 2, body: { kind: "list_universes" } },
    ]);
    client.handleServerMessage({
      type: "response",
      id: 1,
      body: { kind: "open_universe", ...TALOS },
    });
    client.handleServerMessage({ type: "response", id: 2, body: LIST });
    expect(await older.outcome).toEqual({
      ok: false,
      error: { code: "superseded", message: "a newer request replaced this one" },
    });
    expect((await newer.outcome).ok).toBe(true);
  });

  it("sends no cancel when the previous request has already settled", () => {
    const { client, sent } = recordingClient();
    const channel = new RequestChannel(client);

    channel.request({ kind: "list_universes" });
    client.handleServerMessage({ type: "response", id: 1, body: LIST });
    channel.request({ kind: "list_universes" });

    expect(sent.map((message) => message.type)).toEqual(["request", "request"]);
  });

  it("still skips the ID of a request cancelled directly and then superseded", () => {
    const { client, sent } = recordingClient();
    const channel = new RequestChannel(client);
    client[setLastRequestId](MAX_REQUEST_ID - 1);
    channel.request({ kind: "list_universes" }).cancel();
    channel.request({ kind: "list_universes" });

    client[setLastRequestId](MAX_REQUEST_ID - 1);
    client.request({ kind: "list_universes" });

    expect(sent.filter((message) => message.type === "cancel")).toEqual([
      { type: "cancel", id: MAX_REQUEST_ID },
    ]);
    expect([requestId(sent, 1), requestId(sent, 2)]).toEqual([1, 2]);
  });

  it("settles as aborted when its request is cancelled directly", async () => {
    const { client } = recordingClient();
    const channel = new RequestChannel(client);

    const pending = channel.request({ kind: "list_universes" });
    pending.cancel();

    const outcome = await pending.outcome;
    expect(outcome.ok ? null : outcome.error.code).toBe("aborted");
  });
});
