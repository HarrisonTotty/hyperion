import { describe, expect, it } from "vitest";

import type { DescentSpikeReport } from "../preload/api";
import { smallReport } from "./fixtures/spikeReport";
import { readDescentSpikeReport, readSpikeCapture } from "./spikeReport";

describe("the spike report's check", () => {
  it("passes a valid report through unchanged", () => {
    expect(readDescentSpikeReport(smallReport())).toEqual(smallReport());
  });

  it("passes a report whose last trace window the renderer failed, with its reason", () => {
    const ended: DescentSpikeReport = {
      ...smallReport(),
      traceWindows: [
        { startedMs: 900, stopRequestedMs: 31_000, failure: null },
        { startedMs: 31_500, stopRequestedMs: 61_100, failure: "the trace's cycle at 30 s failed" },
      ],
    };
    expect(readDescentSpikeReport(ended)).toEqual(ended);
  });

  it("passes each frame's count of missing resolves through", () => {
    const report = smallReport();
    const incomplete: DescentSpikeReport = {
      ...report,
      frames: { ...report.frames, missingResolves: [0, 5, 1] },
    };
    expect(readDescentSpikeReport(incomplete)?.frames.missingResolves).toEqual([0, 5, 1]);
  });

  it("passes each frame's count of resolves still being read through", () => {
    const report = smallReport();
    const inFlight: DescentSpikeReport = {
      ...report,
      frames: { ...report.frames, missingResolves: [0, 5, 1], inFlightResolves: [0, 2, 1] },
    };
    expect(readDescentSpikeReport(inFlight)?.frames.inFlightResolves).toEqual([0, 2, 1]);
  });

  it.each<[string, (r: DescentSpikeReport) => unknown]>([
    ["a non-object", () => "report"],
    ["a series of another length", (r) => ({ ...r, frames: { ...r.frames, ourCodeMs: [4] } })],
    [
      "callback starts of another length",
      (r) => ({ ...r, frames: { ...r.frames, callbackStartsMs: [1000.1] } }),
    ],
    ["no callback starts", (r) => ({ ...r, frames: { ...r.frames, callbackStartsMs: undefined } })],
    [
      "missing resolves of another length",
      (r) => ({ ...r, frames: { ...r.frames, missingResolves: [0, 0] } }),
    ],
    ["no missing resolves", (r) => ({ ...r, frames: { ...r.frames, missingResolves: undefined } })],
    [
      "a missing-resolve count that is not whole",
      (r) => ({ ...r, frames: { ...r.frames, missingResolves: [0, 1.5, 0] } }),
    ],
    [
      "a negative missing-resolve count",
      (r) => ({ ...r, frames: { ...r.frames, missingResolves: [0, -1, 0] } }),
    ],
    [
      "no resolves in flight",
      (r) => ({ ...r, frames: { ...r.frames, inFlightResolves: undefined } }),
    ],
    [
      "resolves in flight of another length",
      (r) => ({ ...r, frames: { ...r.frames, inFlightResolves: [0, 0] } }),
    ],
    [
      "a count in flight that is not whole",
      (r) => ({ ...r, frames: { ...r.frames, inFlightResolves: [0, 0.5, 0] } }),
    ],
    [
      "more resolves in flight than missing",
      (r) => ({
        ...r,
        frames: { ...r.frames, missingResolves: [0, 1, 0], inFlightResolves: [0, 2, 0] },
      }),
    ],
    ["a non-finite figure", (r) => ({ ...r, uploadBytes: Number.NaN })],
    ["an unknown timer", (r) => ({ ...r, timer: "fast" })],
    [
      "a pass of an unknown row",
      (r) => ({
        ...r,
        frames: { ...r.frames, passes: [{ label: "t", row: "x", gpuMs: [1, 1, 1] }] },
      }),
    ],
    [
      "a segment that ends before it starts",
      (r) => ({ ...r, segments: [{ name: "a", startS: 2, endS: 1 }] }),
    ],
    ["a streaming figure missing", (r) => ({ ...r, streaming: [{ segment: "a" }] })],
    ["a canvas of no size", (r) => ({ ...r, canvas: { widthPx: -1, heightPx: 1 } })],
    ["no script start", (r) => ({ ...r, scriptStartMs: undefined })],
    [
      "a trace window stopped before it started",
      (r) => ({ ...r, traceWindows: [{ startedMs: 10, stopRequestedMs: 5, failure: null }] }),
    ],
    [
      "trace windows that overlap",
      (r) => ({
        ...r,
        traceWindows: [
          { startedMs: 0, stopRequestedMs: 100, failure: null },
          { startedMs: 99, stopRequestedMs: 200, failure: null },
        ],
      }),
    ],
    [
      "a failed trace window before the last",
      (r) => ({
        ...r,
        traceWindows: [
          { startedMs: 0, stopRequestedMs: 100, failure: "the cycle failed" },
          { startedMs: 200, stopRequestedMs: 300, failure: null },
        ],
      }),
    ],
    [
      "an empty failure",
      (r) => ({ ...r, traceWindows: [{ startedMs: 0, stopRequestedMs: 100, failure: "" }] }),
    ],
    [
      "a failure that is not text",
      (r) => ({ ...r, traceWindows: [{ startedMs: 0, stopRequestedMs: 100, failure: 3 }] }),
    ],
    [
      "a trace window with no failure field",
      (r) => ({ ...r, traceWindows: [{ startedMs: 0, stopRequestedMs: 100 }] }),
    ],
    ["no guard", (r) => ({ ...r, traceGuardS: undefined })],
    ["a negative guard", (r) => ({ ...r, traceGuardS: -1 })],
    ["a guard that is not finite", (r) => ({ ...r, traceGuardS: Number.NaN })],
  ])("refuses %s", (_, broken) => {
    expect(readDescentSpikeReport(broken(smallReport()))).toBeNull();
  });
});

describe("the capture's check", () => {
  it("passes JSON text and bytes", () => {
    const capture = { json: '{"version":1}', bin: new Uint8Array([1, 2]) };
    expect(readSpikeCapture(capture)).toEqual(capture);
  });

  it("refuses text that is not a JSON object, and bytes of another type", () => {
    expect(readSpikeCapture({ json: "{", bin: new Uint8Array() })).toBeNull();
    expect(readSpikeCapture({ json: "[1]", bin: new Uint8Array() })).toBeNull();
    expect(readSpikeCapture({ json: "{}", bin: [1, 2] })).toBeNull();
    expect(readSpikeCapture(null)).toBeNull();
  });
});
