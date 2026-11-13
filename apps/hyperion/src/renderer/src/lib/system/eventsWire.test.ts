import { SECONDS_PER_JULIAN_YEAR } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { FIXTURE_SYSTEM, FIXTURE_UNIVERSE, populatedEvents } from "../../test/planetaryFixture";
import { toEventsRequest, toSystemEvents } from "./eventsWire";

const YEAR = SECONDS_PER_JULIAN_YEAR;

describe("toEventsRequest", () => {
  it("asks for the century around a time", () => {
    expect(toEventsRequest(FIXTURE_UNIVERSE, FIXTURE_SYSTEM, { seconds: 7, nanos: 9 })).toEqual({
      kind: "body_events",
      universe: FIXTURE_UNIVERSE,
      system: FIXTURE_SYSTEM,
      from: { seconds: 7 - 50 * YEAR, nanos: 9 },
      to: { seconds: 7 + 50 * YEAR, nanos: 9 },
    });
  });

  it("holds each end inside the clock window", () => {
    const late = toEventsRequest(FIXTURE_UNIVERSE, FIXTURE_SYSTEM, {
      seconds: 980 * YEAR,
      nanos: 0,
    });
    expect([late.from, late.to]).toEqual([
      { seconds: 930 * YEAR, nanos: 0 },
      { seconds: 1_000 * YEAR, nanos: 0 },
    ]);
    const early = toEventsRequest(FIXTURE_UNIVERSE, FIXTURE_SYSTEM, {
      seconds: -1_000 * YEAR,
      nanos: 0,
    });
    expect([early.from, early.to]).toEqual([
      { seconds: -1_000 * YEAR, nanos: 0 },
      { seconds: -950 * YEAR, nanos: 0 },
    ]);
  });
});

describe("toSystemEvents", () => {
  it("lists the fixture's events in its order, each at its kind's time and on its body", () => {
    const result = toSystemEvents(populatedEvents(), FIXTURE_SYSTEM);
    if (result.kind !== "ok") {
      throw new Error(`the fixture's events are usable: ${result.fault}`);
    }
    expect(
      result.events.events.map((event) => [event.kind, event.body, event.time.seconds]),
    ).toEqual([
      ["comet", `${FIXTURE_SYSTEM}.ef00`, -1_577_489_772],
      ["eruption", `${FIXTURE_SYSTEM}.0300`, -1_545_016_715],
      ["impact", `${FIXTURE_SYSTEM}.0500`, -61_215_250],
      ["dust_storm", `${FIXTURE_SYSTEM}.0400`, 25_116_480],
      ["storm", `${FIXTURE_SYSTEM}.0500`, 198_238_971],
    ]);
    expect([result.events.listedFrom, result.events.listedTo]).toEqual([
      result.events.from,
      result.events.to,
    ]);
  });

  it("names an unusable time and an event outside the listed span faults", () => {
    const answer = populatedEvents();
    expect(
      toSystemEvents({ ...answer, listed_to: { seconds: 0.5, nanos: 0 } }, FIXTURE_SYSTEM),
    ).toEqual({
      kind: "fault",
      fault: "event time unusable",
    });
    expect(
      toSystemEvents({ ...answer, listed_to: { seconds: 0, nanos: 0 } }, FIXTURE_SYSTEM),
    ).toEqual({ kind: "fault", fault: "event outside the listed span" });
  });

  it("names an answer for another system, a span outside its window and events out of order faults", () => {
    const answer = populatedEvents();
    expect(toSystemEvents(answer, "0200080020000001")).toEqual({
      kind: "fault",
      fault: `answer for system ${FIXTURE_SYSTEM}`,
    });
    expect(
      toSystemEvents(
        { ...answer, listed_to: { seconds: answer.to.seconds + 1, nanos: 0 } },
        FIXTURE_SYSTEM,
      ),
    ).toEqual({ kind: "fault", fault: "listed span outside the window" });
    expect(
      toSystemEvents({ ...answer, events: answer.events.toReversed() }, FIXTURE_SYSTEM),
    ).toEqual({ kind: "fault", fault: "events out of order" });
  });
});
