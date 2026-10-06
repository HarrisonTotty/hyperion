import type { DescentSpikeReport } from "../../preload/api";

/** A small valid report: three frames, one pass, one segment, one trace window. */
export function smallReport(): DescentSpikeReport {
  return {
    scriptStartMs: 1000,
    traceWindows: [{ startedMs: 900, stopRequestedMs: 61_100, failure: null }],
    traceGuardS: 1,
    warmupS: 10,
    segments: [{ name: "orbit coast", startS: 0, endS: 60 }],
    levels: [{ level: 0, epsilonM: 9000, k: 4.1 }],
    timer: "full",
    untimedPasses: 0,
    frames: {
      scriptTimesS: [0, 0.016, 0.033],
      rafIntervalsMs: [0, 16.7, 16.7],
      ourCodeMs: [4, 4, 4],
      callbackStartsMs: [1000.1, 1016.1, 1033.1],
      passes: [{ label: "terrain", row: "terrain", gpuMs: [1, null, 1.2] }],
    },
    streaming: [
      {
        segment: "orbit coast",
        requestedPerS: 4,
        bakedPerS: 4,
        residentPerS: 4,
        predictedHardPerS: 5,
        predictedCalibratedPerS: 2,
        patchesHard: 90,
        patchesCalibrated: 38,
        streamingS: 0,
      },
    ],
    uploadBytes: 1024,
    latePipelines: [{ label: "x", kind: "render", async: false, scriptTimeS: 12 }],
    adapterPeakBytes: 2048,
    canvas: { widthPx: 1280, heightPx: 720 },
  };
}
