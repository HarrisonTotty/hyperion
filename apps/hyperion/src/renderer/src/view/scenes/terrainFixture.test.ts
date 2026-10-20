import type { BulkChunk, BulkOutcome, CoverDto, PendingBulkRequest } from "@hyperion/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SceneSurfaceEvent } from "../../lib/scene/useScene";
import { toBodyDetailRequest } from "../../lib/system/bodiesWire";
import type { TerrainFixtureLink } from "../../displays/view/useTerrainSession";
import { terrainTestFixture, type TerrainTestFixture } from "./terrainFixture";
import { blockBytes, type TerrainGolden } from "./terrainGolden";
import { TERRAIN_TEST_UNIVERSE, terrainTestGolden } from "./terrainScene";

/** The delta's time in these tests, s of the script. */
const DELTA_AT_S = 10;

/** A fixture over the terrain golden, counting its reads of the golden. */
function countingFixture(chunkIntervalMs = 0): {
  readonly fixture: TerrainTestFixture;
  readonly reads: () => number;
} {
  let reads = 0;
  const golden = (): TerrainGolden => {
    reads += 1;
    return terrainTestGolden();
  };
  return {
    fixture: terrainTestFixture({
      universe: TERRAIN_TEST_UNIVERSE,
      golden,
      deltaAtS: DELTA_AT_S,
      chunkIntervalMs,
    }),
    reads: () => reads,
  };
}

/** Asks `link` for `TEST MOON`'s field from `have`, recording each chunk as it comes. */
function ask(
  link: TerrainFixtureLink,
  have: number | null,
): { readonly pending: PendingBulkRequest<"surface_field">; readonly chunks: BulkChunk[] } {
  const chunks: BulkChunk[] = [];
  const pending = link.requests.requestBulk(
    {
      kind: "surface_field",
      universe: TERRAIN_TEST_UNIVERSE,
      body: terrainTestGolden().body,
      have_revision: have,
    },
    (response) => (response.state === "ready" ? response.bulk : null),
    {
      onChunk: (chunk) => {
        chunks.push(chunk);
      },
    },
  );
  return { pending, chunks };
}

/** An answer's revision and coverage, or its failure's code and field. */
function answerOf(
  outcome: BulkOutcome<"surface_field">,
): readonly [number, CoverDto] | readonly [string, string | null] {
  if (!outcome.ok) {
    return [outcome.error.code, "field" in outcome.error ? outcome.error.field : null];
  }
  const { response } = outcome;
  if (response.state !== "ready") {
    throw new Error("the fixture answers ready fields alone");
  }
  return [response.revision, response.coverage];
}

/** Payload `n`'s blocks (from 1) as byte arrays. */
function payloadBytes(n: number): number[][] {
  return (terrainTestGolden().payloads[n - 1]?.blocksHex ?? []).map((hex) =>
    Array.from(blockBytes(hex)),
  );
}

/** Runs every timer the fixture set, and the promises they settle. */
async function drain(): Promise<void> {
  await vi.runAllTimersAsync();
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("TERRAIN TEST's fixture (R10.T16.d)", () => {
  it("answers the first ask with the first payload, a block a frame, each on a later task", async () => {
    const { fixture } = countingFixture();
    const link = fixture.open();
    const { pending, chunks } = ask(link, null);
    expect(chunks).toHaveLength(0);
    await drain();
    const golden = terrainTestGolden();
    expect(answerOf(await pending.outcome)).toEqual([1, golden.payloads[0]?.coverage]);
    expect(chunks.map((chunk) => Array.from(chunk.payload))).toEqual(payloadBytes(1));
    const outcome = await pending.outcome;
    expect(
      outcome.ok && outcome.response.state === "ready" ? outcome.response.header : null,
    ).toEqual(golden.header);
    link.close();
  });

  it("reads the golden's field only at a link's first answer", async () => {
    const { fixture, reads } = countingFixture();
    const link = fixture.open();
    expect(reads()).toBe(0);
    ask(link, null);
    await drain();
    expect(reads()).toBeGreaterThan(0);
    link.close();
  });

  it("settles a cancelled ask as aborted and sends nothing more for it", async () => {
    const { fixture } = countingFixture(5);
    const link = fixture.open();
    const { pending, chunks } = ask(link, null);
    await vi.advanceTimersByTimeAsync(5);
    expect(chunks).toHaveLength(1);
    pending.cancel();
    expect(answerOf(await pending.outcome)).toEqual(["aborted", null]);
    await drain();
    expect([chunks.length, vi.getTimerCount()]).toEqual([1, 0]);
    link.close();
  });

  it("refuses a have_revision above its own as the server does, naming it", async () => {
    const { fixture } = countingFixture();
    const link = fixture.open();
    const { pending, chunks } = ask(link, 2);
    await drain();
    expect([answerOf(await pending.outcome), chunks.length]).toEqual([
      ["bad_request", "have_revision"],
      0,
    ]);
    link.close();
  });

  it("answers an ask at the revision held with block 0 alone and no cell", async () => {
    const { fixture } = countingFixture();
    const link = fixture.open();
    const { pending, chunks } = ask(link, 1);
    await drain();
    expect(answerOf(await pending.outcome)).toEqual([1, { ranges: [] }]);
    expect(chunks.map((chunk) => Array.from(chunk.payload))).toEqual(payloadBytes(4));
    link.close();
  });

  it("raises its revision once the script passes the delta's time, announces it once, and never lowers it", async () => {
    const { fixture } = countingFixture();
    const link = fixture.open();
    const events: SceneSurfaceEvent[] = [];
    link.onSurface((event) => {
      events.push(event);
    });
    fixture.notePlayed(DELTA_AT_S - 1);
    await drain();
    expect(events).toEqual([]);
    fixture.notePlayed(DELTA_AT_S);
    expect(events).toEqual([]);
    await drain();
    const body = terrainTestGolden().body;
    expect(events).toEqual([{ kind: "revisions", revisions: [{ body, revision: 2 }] }]);
    // A loop, a scrub back and a pause leave it risen, and tell nothing more.
    for (const scriptS of [0, 3, DELTA_AT_S, 299, 0]) {
      fixture.notePlayed(scriptS);
    }
    await drain();
    expect(events).toHaveLength(1);
    const delta = ask(link, 1);
    const whole = ask(link, null);
    const held = ask(link, 2);
    await drain();
    const golden = terrainTestGolden();
    expect([
      answerOf(await delta.pending.outcome),
      answerOf(await whole.pending.outcome),
      answerOf(await held.pending.outcome),
    ]).toEqual([
      [2, golden.payloads[1]?.coverage],
      [2, golden.payloads[2]?.coverage],
      [2, { ranges: [] }],
    ]);
    expect(
      [delta.chunks, whole.chunks].map((c) => c.map((chunk) => Array.from(chunk.payload))),
    ).toEqual([payloadBytes(2), payloadBytes(3)]);
    link.close();
  });

  it("starts each link at revision 1", async () => {
    const { fixture } = countingFixture();
    const first = fixture.open();
    fixture.notePlayed(DELTA_AT_S);
    await drain();
    const second = fixture.open();
    const { pending } = ask(second, null);
    await drain();
    expect(answerOf(await pending.outcome)).toEqual([1, terrainTestGolden().payloads[0]?.coverage]);
    first.close();
    second.close();
  });

  it("answers body_detail unsupported, since the scene's body carries its seed", async () => {
    const { fixture } = countingFixture();
    const link = fixture.open();
    const pending = link.requests.request(
      toBodyDetailRequest(TERRAIN_TEST_UNIVERSE, terrainTestGolden().body, {
        seconds: 0,
        nanos: 0,
      }),
    );
    await drain();
    const outcome = await pending.outcome;
    expect(outcome.ok ? null : outcome.error.code).toBe("unsupported");
    link.close();
  });

  it("counts its links' asks and the blocks they sent and have still to send (R10.T16.d, part 2)", async () => {
    const { fixture } = countingFixture(5);
    const link = fixture.open();
    const blocks = terrainTestGolden().payloads[0]?.blocksHex.length ?? 0;
    const { pending } = ask(link, null);
    const detail = link.requests.request(
      toBodyDetailRequest(TERRAIN_TEST_UNIVERSE, terrainTestGolden().body, {
        seconds: 0,
        nanos: 0,
      }),
    );
    expect(fixture.traffic()).toEqual({
      asks: 2,
      fieldAsks: 1,
      blocksSent: 0,
      blocksToCome: blocks,
    });
    await vi.advanceTimersByTimeAsync(5);
    expect(fixture.traffic()).toMatchObject({ blocksSent: 1, blocksToCome: blocks - 1 });
    await drain();
    await Promise.all([pending.outcome, detail.outcome]);
    expect(fixture.traffic()).toEqual({
      asks: 2,
      fieldAsks: 1,
      blocksSent: blocks,
      blocksToCome: 0,
    });
    link.close();
  });

  it("counts a cancelled answer's blocks, and a closed link's, as no longer to come", async () => {
    const { fixture } = countingFixture(5);
    const first = fixture.open();
    const cancelled = ask(first, null);
    await vi.advanceTimersByTimeAsync(5);
    cancelled.pending.cancel();
    expect(fixture.traffic()).toMatchObject({ blocksSent: 1, blocksToCome: 0 });
    const second = fixture.open();
    ask(second, null);
    await vi.advanceTimersByTimeAsync(5);
    second.close();
    expect(fixture.traffic()).toMatchObject({ fieldAsks: 2, blocksSent: 2, blocksToCome: 0 });
    first.close();
  });

  it("ends its requests in flight, its timers and its announcements when closed", async () => {
    const { fixture } = countingFixture(5);
    const link = fixture.open();
    const events: SceneSurfaceEvent[] = [];
    link.onSurface((event) => {
      events.push(event);
    });
    const { pending } = ask(link, null);
    await vi.advanceTimersByTimeAsync(5);
    link.close();
    fixture.notePlayed(DELTA_AT_S);
    expect(answerOf(await pending.outcome)).toEqual(["link_lost", null]);
    await drain();
    expect([events, vi.getTimerCount()]).toEqual([[], 0]);
  });
});
