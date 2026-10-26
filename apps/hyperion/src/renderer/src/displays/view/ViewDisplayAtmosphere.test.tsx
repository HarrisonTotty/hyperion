/**
 * The atmosphere on a live `VIEW` (plan R08, R08.T10.b): the cache's `body_detail` joined with the
 * terrain session's, and a refused record's line on the primary's plate.
 */
import type { ResponseBody } from "@hyperion/protocol";
import { act, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { FIXTURE_EARTH } from "../../test/planetaryFixture";
import { SCENE_TIDAL_RADIUS_M, sceneClock, sliceSceneSystem } from "../../test/sceneFixture";
import {
  nominalStore,
  openUniverse,
  primaryLabelBlock,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";

afterEach(() => {
  vi.useRealTimers();
});

/** WGS 84's equatorial radius, m: the slice's Earth's figure. */
const EARTH_RADIUS_M = 6_378_137;

/** `VIEW` drawing the server's scene photorealistically, the ship 2,000 km over the Earth. */
async function overEarthPhotoreal(): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
  });
  await settle();
  view.advance(100);
  await openUniverse(view);
  const answer: ResponseBody = {
    kind: "subscribe",
    subscription: 5,
    state: {
      topic: "scene",
      sequence: 0,
      clock: sceneClock(3_000),
      ship: {
        position: { frame: "body", body: FIXTURE_EARTH, offset_m: [0, 0, EARTH_RADIUS_M + 2e6] },
        velocity_m_s: [0, 0, 0],
        time: { seconds: 3_000, nanos: 0 },
      },
      system: sliceSceneSystem(),
      tidal_radius_m: SCENE_TIDAL_RADIUS_M,
      craft: [],
    },
  };
  await act(async () => {
    view.socket.serverAnswers("subscribe", () => answer);
    await vi.advanceTimersByTimeAsync(0);
  });
  await settle();
  view.advance(300);
  await view.user.keyboard("4");
  view.advance(100);
  await settle();
  view.advance(300);
  await settle();
  return view;
}

describe("the atmosphere on a live VIEW (R08.T10.b)", () => {
  it("asks the Earth's record once for its atmosphere and its terrain together", async () => {
    const view = await overEarthPhotoreal();
    const asked = view.socket
      .requestsOfKind("body_detail")
      .filter((request) => request.body.body === FIXTURE_EARTH);
    expect(asked).toHaveLength(1);
  });

  it("states ATMOSPHERE REJECTED with the server's reason where it refuses the record", async () => {
    const view = await overEarthPhotoreal();
    const asked = view.socket
      .requestsOfKind("body_detail")
      .filter((request) => request.body.body === FIXTURE_EARTH);
    await act(async () => {
      for (const request of asked) {
        view.socket.serverRejects(request.id, {
          code: "unknown_body",
          message: "no such body",
          field: null,
        });
      }
      await vi.advanceTimersByTimeAsync(0);
    });
    view.advance(600);
    await settle();
    const rejected = within(primaryLabelBlock())
      .queryAllByRole("status")
      .map((output) => output.textContent)
      .filter((text) => text.startsWith("ATMOSPHERE REJECTED"));
    expect(rejected).toEqual([
      expect.stringMatching(/^ATMOSPHERE REJECTED: for .+, no such body, reopen VIEW to retry$/u),
    ]);
  });
});
