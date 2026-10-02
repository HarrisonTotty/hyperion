import { describe, expect, it } from "vitest";

import type { ClientMessage } from "./generated/ClientMessage";
import type { RequestError } from "./generated/RequestError";
import type { SceneNotificationDto } from "./generated/SceneNotificationDto";
import type { SceneStateDto } from "./generated/SceneStateDto";
import type { ServerMessage } from "./generated/ServerMessage";
import { decodeServerMessage } from "./index";
import { RequestClient } from "./requests";
import type { NotificationOf, Subscription, SubscriptionEnd } from "./subscriptions";

const UNIVERSE = "0123456789abcdef";

const CLOCK = {
  time: { seconds: 0, nanos: 0 },
  time_rate: 1,
  state: "running",
} as const;

const STATE: SceneStateDto = {
  sequence: 0,
  clock: CLOCK,
  ship: {
    position: { frame: "galactic", position: { cell_ly: [0, 26_000, 0], offset_m: [0, 0, 0] } },
    velocity_m_s: [0, 0, 0],
    time: { seconds: 0, nanos: 0 },
  },
  system: null,
  craft: [],
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

/** The last request sent, which must exist. */
function lastRequestId(sent: readonly ClientMessage[]): number {
  const requests = sent.filter((message) => message.type === "request");
  const last = requests[requests.length - 1];
  if (last === undefined) {
    throw new Error("no request was sent");
  }
  return last.id;
}

/** Subscribes to the scene and answers with subscription `number`; returns the subscription. */
async function sceneSubscription(
  client: RequestClient,
  sent: ClientMessage[],
  number: number,
): Promise<Subscription<"scene">> {
  const pending = client.subscribe(UNIVERSE, {
    topic: "scene",
    detail: "full",
    cameras: [],
  });
  const id = lastRequestId(sent);
  expect(
    client.handleServerMessage({
      type: "response",
      id,
      body: { kind: "subscribe", subscription: number, state: { topic: "scene", ...STATE } },
    }),
  ).toBe(true);
  const outcome = await pending.outcome;
  if (!outcome.ok) {
    throw new Error(`the subscription failed: ${outcome.error.message}`);
  }
  return outcome.subscription;
}

/** A scene notification on `subscription`, numbered `sequence`. */
function notification(subscription: number, sequence: number): ServerMessage {
  const body: SceneNotificationDto = { sequence, clock: CLOCK, bodies: [] };
  return { type: "notification", subscription, body: { topic: "scene", ...body } };
}

describe("RequestClient.subscribe", () => {
  it("sends a subscribe request and opens with the topic's whole state", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 1);
    expect(sent[0]).toMatchObject({
      type: "request",
      body: { kind: "subscribe", universe: UNIVERSE, topic: { topic: "scene" } },
    });
    expect(subscription.id).toBe(1);
    expect(subscription.state).toEqual({ topic: "scene", ...STATE });
    expect(subscription.ended).toBe(false);
  });

  it("routes each notification to its own subscription and consumes it", async () => {
    const { client, sent } = recordingClient();
    const first = await sceneSubscription(client, sent, 1);
    const second = await sceneSubscription(client, sent, 2);
    const seen: Array<[number, number]> = [];
    first.onNotification((body) => seen.push([1, body.sequence]));
    second.onNotification((body) => seen.push([2, body.sequence]));
    expect(client.handleServerMessage(notification(2, 1))).toBe(true);
    expect(client.handleServerMessage(notification(1, 1))).toBe(true);
    expect(client.handleServerMessage(notification(1, 2))).toBe(true);
    expect(seen).toEqual([
      [2, 1],
      [1, 1],
      [1, 2],
    ]);
  });

  it("tells the server when the client unsubscribes", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 3);
    subscription.unsubscribe();
    expect(sent[sent.length - 1]).toMatchObject({
      type: "request",
      body: { kind: "unsubscribe", subscription: 3 },
    });
  });

  it("drops a notification after unsubscribe", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 3);
    const seen: Array<NotificationOf<"scene">> = [];
    subscription.onNotification((body) => seen.push(body));
    subscription.unsubscribe();
    expect(client.handleServerMessage(notification(3, 1))).toBe(true);
    expect(seen).toEqual([]);
  });

  it("ends the subscription as unsubscribed", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 3);
    const ends: SubscriptionEnd[] = [];
    subscription.onEnd((end) => ends.push(end));
    subscription.unsubscribe();
    expect(ends).toEqual([{ kind: "unsubscribed" }]);
    expect(subscription.ended).toBe(true);
  });

  it("sends one unsubscribe however often it is asked", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 3);
    subscription.unsubscribe();
    subscription.unsubscribe();
    expect(sent.filter((message) => message.type === "request").length).toBe(2);
  });

  it("ends every subscription when the link is lost", async () => {
    const { client, sent, setLinkUp } = recordingClient();
    const first = await sceneSubscription(client, sent, 1);
    const second = await sceneSubscription(client, sent, 2);
    const ends: Array<[number, SubscriptionEnd]> = [];
    first.onEnd((end) => ends.push([1, end]));
    second.onEnd((end) => ends.push([2, end]));
    setLinkUp(false);
    client.linkLost();
    expect(ends).toEqual([
      [1, { kind: "link_lost" }],
      [2, { kind: "link_lost" }],
    ]);
    expect(first.ended && second.ended).toBe(true);
  });

  it("tells a listener added after the end how it ended", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 1);
    client.linkLost();
    const late: SubscriptionEnd[] = [];
    subscription.onEnd((end) => late.push(end));
    expect(late).toEqual([{ kind: "link_lost" }]);
  });

  it("ends the subscription the server ends, with its reason, and no other", async () => {
    const { client, sent } = recordingClient();
    const first = await sceneSubscription(client, sent, 1);
    const second = await sceneSubscription(client, sent, 2);
    const ends: SubscriptionEnd[] = [];
    first.onEnd((end) => ends.push(end));
    const error: RequestError = {
      code: "internal",
      message: "the scene could not be advanced",
      field: null,
    };

    expect(client.handleServerMessage({ type: "subscription_ended", subscription: 1, error })).toBe(
      true,
    );
    expect(ends).toEqual([{ kind: "ended", error }]);
    expect(first.ended).toBe(true);
    expect(second.ended).toBe(false);
    // The server already ended it: the client sends no unsubscribe of its own.
    expect(sent.filter((message) => message.type === "request")).toHaveLength(2);
  });

  it("ends the subscription as internal when the server's reason cannot be read", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 1);
    const ends: SubscriptionEnd[] = [];
    subscription.onEnd((end) => ends.push(end));

    expect(
      client.handleServerMessage(
        decodeServerMessage('{"type":"subscription_ended","subscription":1}'),
      ),
    ).toBe(true);

    expect(subscription.ended).toBe(true);
    expect(ends).toEqual([
      {
        kind: "ended",
        error: {
          code: "internal",
          message: "the server ended the subscription without a reason the client can read",
          field: null,
        },
      },
    ]);
  });

  it("settles a subscribe answered with no state as a protocol violation, and unsubscribes", async () => {
    const { client, sent } = recordingClient();
    const pending = client.subscribe(UNIVERSE, { topic: "scene", detail: "full", cameras: [] });
    const id = lastRequestId(sent);

    expect(
      client.handleServerMessage(
        decodeServerMessage(
          JSON.stringify({
            type: "response",
            id,
            body: { kind: "subscribe", subscription: 4, state: null },
          }),
        ),
      ),
    ).toBe(true);

    const outcome = await pending.outcome;
    expect(outcome.ok ? null : outcome.error.code).toBe("protocol_violation");
    expect(sent[sent.length - 1]).toMatchObject({
      type: "request",
      body: { kind: "unsubscribe", subscription: 4 },
    });
  });

  it("consumes and ignores a subscription_ended for a subscription it does not have", () => {
    const { client } = recordingClient();
    const error: RequestError = { code: "internal", message: "gone", field: null };

    expect(client.handleServerMessage({ type: "subscription_ended", subscription: 9, error })).toBe(
      true,
    );
  });

  it("settles as link_lost at once when the link is down", async () => {
    const { client, setLinkUp } = recordingClient();
    setLinkUp(false);
    const outcome = await client.subscribe(UNIVERSE, {
      topic: "scene",
      detail: "full",
      cameras: [],
    }).outcome;
    expect(outcome).toMatchObject({ ok: false, error: { code: "link_lost" } });
  });

  it("ends a subscription the server opened as its subscribe was cancelled", async () => {
    const { client, sent } = recordingClient();
    const pending = client.subscribe(UNIVERSE, { topic: "scene", detail: "full", cameras: [] });
    const id = lastRequestId(sent);
    pending.cancel();
    expect((await pending.outcome).ok).toBe(false);
    // The answer was already on its way when the cancel went out.
    client.handleServerMessage({
      type: "response",
      id,
      body: { kind: "subscribe", subscription: 4, state: { topic: "scene", ...STATE } },
    });
    expect(sent[sent.length - 1]).toMatchObject({
      type: "request",
      body: { kind: "unsubscribe", subscription: 4 },
    });
  });

  it("consumes and ignores a notification for an unknown subscription", async () => {
    const { client, sent } = recordingClient();
    const subscription = await sceneSubscription(client, sent, 1);
    const seen: number[] = [];
    subscription.onNotification((body) => seen.push(body.sequence));
    expect(client.handleServerMessage(notification(9, 1))).toBe(true);
    expect(seen).toEqual([]);
  });

  it("settles a refused subscribe as a failure and opens nothing", async () => {
    const { client, sent } = recordingClient();
    const pending = client.subscribe(UNIVERSE, { topic: "scene", detail: "full", cameras: [] });
    client.handleServerMessage({
      type: "request_error",
      id: lastRequestId(sent),
      error: { code: "bad_request", message: "too many", field: "topic" },
    });
    const outcome = await pending.outcome;
    expect(outcome).toEqual({
      ok: false,
      error: { code: "bad_request", message: "too many", field: "topic" },
    });
  });
});
