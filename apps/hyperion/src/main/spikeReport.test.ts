import { describe, expect, it } from "vitest";

import type { DescentSpikeReport } from "../preload/api";
import { smallReport } from "./fixtures/spikeReport";
import { readDescentSpikeReport, readSpikeCapture } from "./spikeReport";

describe("the spike report's check", () => {
  it("passes a valid report through unchanged", () => {
    expect(readDescentSpikeReport(smallReport())).toEqual(smallReport());
  });

  it.each<[string, (r: DescentSpikeReport) => unknown]>([
    ["a non-object", () => "report"],
    ["a series of another length", (r) => ({ ...r, frames: { ...r.frames, ourCodeMs: [4] } })],
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
