/**
 * The system readout's readings of a member of the global list (plan 10, P10.T11): its `ORIGIN`
 * names its stream or core, and a stream member's stream rows (`StreamReadout`) stand once the
 * global list and the stream's track have answered, absent before.
 */
import type { SystemOriginWire, SystemRecord } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import { toChartFeatures } from "../../lib/galaxy/featureWire";
import { toGlobalList, toStreamTrack } from "../../lib/galaxy/globalWire";
import type { ChartFeature, StreamTrack } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { aFeaturesInRange, OPEN_CLUSTER } from "../../test/featureFixtures";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aSystemsInRange } from "../../test/galaxyFixtures";
import { aGlobalFeatures, aStreamEntry, aStreamTrack } from "../../test/globalListFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { distanceUnitFor } from "./chartModel";
import { SystemReadout } from "./SystemReadout";
import type { StreamReadoutQuery } from "./useStreamReadout";

const CENTRE = [26_000, 0, 0] as const;

/** One system 1 ly out, of origin `origin`, as the chart holds it. */
function aListMember(origin: SystemOriginWire) {
  const response = aSystemsInRange({
    centreLy: CENTRE,
    systems: [{ relLy: [1, 0, 0], layer: "a" }],
  });
  const [record] = response.systems;
  if (record === undefined) {
    throw new Error("the fixture built no system");
  }
  const member: SystemRecord = { ...record, origin };
  const [system] = toChartResult({ ...response, systems: [member] }).systems;
  if (system === undefined) {
    throw new Error("the adapter kept no system");
  }
  return system;
}

/** Stream 1's track, ready, 100,000 ly long. */
function aTrack(): StreamTrack {
  const answer = toStreamTrack(aStreamTrack(1, [[30_000, 0, 0]], [[31_000, 0, 0]]));
  if (answer.kind !== "ready") {
    throw new Error("the fixture's track is ready");
  }
  return answer.track;
}

/** The golden's open cluster, as the chart holds it. */
const CLUSTER: ChartFeature | undefined = toChartFeatures(
  aFeaturesInRange({ features: [OPEN_CLUSTER] }),
).features[0];

/** Renders the readout of a member of `origin`, reading `stream` of its stream, with the link up. */
function renderMember(origin: SystemOriginWire, stream: StreamReadoutQuery | null) {
  render(
    <ServerLinkHarness>
      <SystemReadout
        system={aListMember(origin)}
        frame={localFrameAt(vec3(...CENTRE))}
        driveRangeLy={50}
        timeYr={0}
        distanceUnit={distanceUnitFor(50)}
        stars={null}
        stale={false}
        starsStale={false}
        extinction={null}
        features={CLUSTER === undefined ? [] : [CLUSTER]}
        stream={stream}
      />
    </ServerLinkHarness>,
  );
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  return screen.getByRole("status", { name: "Selected system" });
}

/** The value beside a label in the readout, or `undefined` where it has no such reading. */
function valueOf(readout: HTMLElement, label: string): string | null | undefined {
  const term = within(readout).queryAllByText(label, { selector: "dt" })[0];
  return term === undefined ? undefined : (term.nextElementSibling?.textContent ?? null);
}

const STREAM_ROWS = [
  "STREAM MASS",
  "STREAM PROGENITOR",
  "STREAM LENGTH",
  "STREAM WIDTH",
  "STREAM DISPERSION",
];

describe("StreamReadout (plan 10, P10.T11)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("names a member's stream, and leaves its stream's rows out until they are answered", () => {
    const readout = renderMember(
      { type: "stream", stream: 1 },
      { list: null, track: null, state: { kind: "pending" } },
    );

    expect(valueOf(readout, "ORIGIN")).toBe("STREAM 1");
    for (const label of STREAM_ROWS) {
      expect(valueOf(readout, label)).toBeUndefined();
    }
  });

  it("reads the stream's mass, progenitor, length, width and dispersion once they arrive", () => {
    const list = toGlobalList(
      aGlobalFeatures({ entries: [aStreamEntry({ number: 1, mass_msun: 4_500_000 })] }),
    );
    const readout = renderMember(
      { type: "stream", stream: 1 },
      { list, track: aTrack(), state: null },
    );

    // The drawn M☉, whose sun is no text.
    expect(valueOf(readout, "STREAM MASS")).toBe("4,500,000 M");
    // The recent dwarf that left stream 1 (the fixture's progenitors).
    expect(valueOf(readout, "STREAM PROGENITOR")).toBe("RECENT DWARF");
    expect(valueOf(readout, "STREAM LENGTH")).toBe("100,000.0 ly");
    // The points' mean of 150 and 250 ly.
    expect(valueOf(readout, "STREAM WIDTH")).toBe("200.0 ly");
    expect(valueOf(readout, "STREAM DISPERSION")).toBe("0.8 km/s");
  });

  it("names an orphan's progenitor a dissolved globular", () => {
    const orphan = toGlobalList(aGlobalFeatures({ entries: [aStreamEntry({ number: 0 })] }));
    const readout = renderMember(
      { type: "stream", stream: 0 },
      { list: orphan, track: null, state: null },
    );

    expect(valueOf(readout, "STREAM PROGENITOR")).toBe("DISSOLVED GLOBULAR");
  });

  it("names a living progenitor by the chart's feature where the chart holds it", () => {
    const living = toGlobalList(
      aGlobalFeatures({ entries: [aStreamEntry({ number: 0, progenitor: OPEN_CLUSTER.id })] }),
    );
    const readout = renderMember(
      { type: "stream", stream: 0 },
      { list: living, track: null, state: null },
    );

    expect(valueOf(readout, "STREAM PROGENITOR")).toBe(`OC ${OPEN_CLUSTER.designation}`);
  });

  it("names a living progenitor the chart does not hold by its feature's ID", () => {
    const living = toGlobalList(
      aGlobalFeatures({ entries: [aStreamEntry({ number: 0, progenitor: "e842000280000000" })] }),
    );
    const readout = renderMember(
      { type: "stream", stream: 0 },
      { list: living, track: null, state: null },
    );

    expect(valueOf(readout, "STREAM PROGENITOR")).toBe("FEATURE E842000280000000");
  });

  it("names a dwarf core's member's core, as a member, with no stream rows", () => {
    const readout = renderMember({ type: "dwarf_core", core: 0 }, null);

    expect(valueOf(readout, "ORIGIN")).toBe("DWARF CORE 0");
    expect(valueOf(readout, "MEMBER CLASS")).toBe("—");
    for (const label of STREAM_ROWS) {
      expect(valueOf(readout, label)).toBeUndefined();
    }
  });
});
