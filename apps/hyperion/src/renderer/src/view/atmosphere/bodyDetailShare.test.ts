/**
 * One `body_detail` fetch for the atmosphere cache and the terrain session (plan R08, R08.T10.b):
 * joined requests, the replies kept for the terrain, and cancelling.
 */
import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { toBodyDetailRequest } from "../../lib/system/bodiesWire";
import { BodyDetailSeam } from "../../test/bodyDetailSeam";
import { earthDetail, FIXTURE_SYSTEM, FIXTURE_UNIVERSE } from "../../test/planetaryFixture";
import {
  BODY_DETAIL_SHARE_REPLIES,
  type BodyDetailClient,
  BodyDetailShare,
} from "./bodyDetailShare";

const UNIVERSE = FIXTURE_UNIVERSE;
const BODY: BodyIdHex = `${FIXTURE_SYSTEM}.0300`;
const TIME = { seconds: 0, nanos: 0 };

/** A body of the fixture's system by index. */
function bodyOf(index: number): BodyIdHex {
  return `${FIXTURE_SYSTEM}.${index.toString(16).padStart(4, "0")}`;
}

/** The share over a seam that records what reaches the server. */
function harness(): { readonly seam: BodyDetailSeam; readonly share: BodyDetailShare } {
  const seam = new BodyDetailSeam();
  return { seam, share: new BodyDetailShare(seam.client) };
}

/** Lets every settled outcome's continuations run: a task's turn, after every microtask. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => {
    setTimeout(resolve, 0);
  });
}

/** A bulk answer's manifest: none, for a request the client is only handed. */
function noManifest(): null {
  return null;
}

describe("the body_detail share (R08.T10.b)", () => {
  it("sends one request for the cache's and the terrain's asks of a body together", async () => {
    const { seam, share } = harness();
    const cache = share.atmosphere.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    const terrain = share.terrain.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    expect(seam.askedFor(BODY)).toBe(1);
    seam.answer(seam.latestFor(BODY).id, earthDetail());
    const [one, two] = await Promise.all([cache.outcome, terrain.outcome]);
    expect([one.ok, two.ok]).toEqual([true, true]);
  });

  it("answers the terrain's ask from the cache's reply, with no request", async () => {
    const { seam, share } = harness();
    const cache = share.atmosphere.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    seam.answer(seam.latestFor(BODY).id, earthDetail());
    await cache.outcome;
    await settle();
    const terrain = share.terrain.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    expect((await terrain.outcome).ok).toBe(true);
    expect(seam.askedFor(BODY)).toBe(1);
  });

  it("asks the server again at each of the cache's triggers once the last is answered", async () => {
    const { seam, share } = harness();
    for (let n = 1; n <= 2; n += 1) {
      const asked = share.atmosphere.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
      seam.answer(seam.latestFor(BODY).id, earthDetail());
      // Each trigger follows the last one's answer, in turn.
      // oxlint-disable-next-line no-await-in-loop
      await asked.outcome;
      // oxlint-disable-next-line no-await-in-loop
      await settle();
      expect(seam.askedFor(BODY)).toBe(n);
    }
  });

  it("lets a body's reply go when the cache asks it afresh, so the terrain joins the new ask", async () => {
    const { seam, share } = harness();
    const first = share.atmosphere.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    seam.answer(seam.latestFor(BODY).id, earthDetail());
    await first.outcome;
    await settle();
    share.atmosphere.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    share.terrain.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    expect(seam.askedFor(BODY)).toBe(2);
  });

  it("keeps a joined request for the asker left when the other cancels, and cancels it with the last", async () => {
    const { seam, share } = harness();
    const cache = share.atmosphere.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    const terrain = share.terrain.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    cache.cancel();
    expect(seam.cancelled).toEqual([]);
    terrain.cancel();
    expect(seam.cancelled).toEqual([seam.latestFor(BODY).id]);
  });

  it("keeps no refused reply for the terrain", async () => {
    const { seam, share } = harness();
    const cache = share.atmosphere.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    seam.refuse(seam.latestFor(BODY).id, "unknown_body", "no such body");
    await cache.outcome;
    await settle();
    share.terrain.request(toBodyDetailRequest(UNIVERSE, BODY, TIME));
    expect(seam.askedFor(BODY)).toBe(2);
    expect(share.repliesKept).toBe(0);
  });

  it("passes a bulk request to the client whole, its options included", () => {
    const calls: unknown[][] = [];
    const client: BodyDetailClient = {
      request: () => {
        throw new Error("no request is expected");
      },
      requestBulk: (...args: unknown[]) => {
        calls.push(args);
        return { outcome: new Promise(() => undefined), cancel: () => undefined };
      },
    };
    const share = new BodyDetailShare(client);
    const manifestOf = noManifest;
    const options = { onChunk: (): void => undefined };
    share.terrain.requestBulk(toBodyDetailRequest(UNIVERSE, BODY, TIME), manifestOf, options);
    expect(calls).toEqual([[toBodyDetailRequest(UNIVERSE, BODY, TIME), manifestOf, options]]);
  });

  it("keeps the latest replies of at most its cap of bodies", async () => {
    const { seam, share } = harness();
    for (let n = 0; n <= BODY_DETAIL_SHARE_REPLIES; n += 1) {
      const asked = share.atmosphere.request(
        toBodyDetailRequest(UNIVERSE, bodyOf(0x300 + n), TIME),
      );
      seam.answer(seam.latestFor(bodyOf(0x300 + n)).id, earthDetail());
      // Each body is answered in turn, so that the least recently answered is the first.
      // oxlint-disable-next-line no-await-in-loop
      await asked.outcome;
    }
    await settle();
    expect(share.repliesKept).toBe(BODY_DETAIL_SHARE_REPLIES);
  });
});
