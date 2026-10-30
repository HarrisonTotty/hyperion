import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import type { SystemEvent } from "../../lib/system/eventsWire";
import { eventKindLabel, eventRow, formatTimeSpan, nextEventIndex } from "./eventRows";

const at = (seconds: number): UniverseTime => ({ seconds, nanos: 0 });

function anEvent(overrides: Partial<SystemEvent> = {}): SystemEvent {
  return {
    id: "0200080020000000.0500:0402fffffffff300",
    kind: "storm",
    body: "0200080020000000.0500",
    time: at(198_238_971),
    ...overrides,
  };
}

describe("eventKindLabel", () => {
  it.each([
    ["impact", "IMPACT"],
    ["eruption", "ERUPTION"],
    ["storm", "GREAT STORM"],
    ["dust_storm", "DUST STORM"],
    ["comet", "COMET"],
  ] as const)("names a %s %s", (kind, label) => {
    expect(eventKindLabel(kind)).toBe(label);
  });
});

describe("eventRow", () => {
  const designations = new Map([["0200080020000000.0500", "H7K /1280"]]);

  it("reads an event after the display time with its UT time and a T- countdown", () => {
    expect(eventRow(anEvent(), designations, at(0))).toEqual({
      id: "0200080020000000.0500:0402fffffffff300",
      time: "UT +6 yr 102/22:22:51",
      countdown: "T-6 yr 102/22:22:51",
      kind: "GREAT STORM",
      body: "H7K /1280",
    });
  });

  it("reads one before the display time with a T+ countdown", () => {
    const row = eventRow(anEvent({ time: at(-86_400) }), designations, at(3_600));
    expect([row.time, row.countdown]).toEqual(["UT -0 yr 001/00:00:00", "T+0 yr 001/01:00:00"]);
  });

  it("has no designation for a body the bodies on show do not hold", () => {
    expect(eventRow(anEvent({ body: "0200080020000000.0400" }), designations, at(0)).body).toBe(
      null,
    );
  });
});

describe("nextEventIndex", () => {
  const events = [anEvent({ time: at(-10) }), anEvent({ time: at(0) }), anEvent({ time: at(10) })];

  it("finds the first event at or after the display time", () => {
    expect(nextEventIndex(events, at(-5))).toBe(1);
    expect(nextEventIndex(events, at(0))).toBe(1);
  });

  it("falls on the last where every event has passed", () => {
    expect(nextEventIndex(events, at(11))).toBe(2);
  });

  it("falls on none for no events", () => {
    expect(nextEventIndex([], at(0))).toBe(-1);
  });
});

describe("formatTimeSpan", () => {
  it("writes a span of universe time as one reading in the long form, the label once", () => {
    expect(
      formatTimeSpan(at(-50 * SECONDS_PER_JULIAN_YEAR), at(50 * SECONDS_PER_JULIAN_YEAR)),
    ).toBe("UT -50 yr 000/00:00:00 – +50 yr 000/00:00:00");
  });
});
