import { describe, expect, it, vi } from "vitest";

import type { StillLabel, StillSaveAnswer } from "../../../../preload/api";
import {
  FAKE_FRAME_NS,
  FAKE_PASSES,
  FakeStillDevice,
  FakeStillRenderer,
  FakeStillSource,
} from "../../test/fakeStillJob";
import { TEST_STILL_REQUEST } from "../../test/stillFixtures";
import { batchCapNs } from "./batchController";
import type { StillControlState, StillStep } from "./StillControl";
import {
  assembleTiles,
  runStillJob,
  STILL_PROGRESS_INTERVAL_MS,
  STILL_STREAMING_TIMEOUT_S,
  type StillJobOptions,
  StillSession,
} from "./stillJob";
import { STILL_TERRAIN_STREAMING, type StillRequest } from "./stillLabel";
import { planTiles } from "./stillPlan";

/** A still of four tiles at a 64 px tile limit: 2 × 2, its last column padded. */
const REQUEST: StillRequest = { ...TEST_STILL_REQUEST, size: { widthPx: 100, heightPx: 70 } };

/** A save that records each PNG and label it is handed. */
function recordingSave() {
  const saved: Array<{ readonly png: Uint8Array; readonly label: StillLabel }> = [];
  const save = (png: Uint8Array, label: StillLabel): Promise<StillSaveAnswer> => {
    saved.push({ png, label });
    return Promise.resolve({
      kind: "saved",
      still: { id: `20261113T000000000Z-0000000${String(saved.length)}`, label },
    });
  };
  return { saved, save };
}

/** An encoder that hands the pixels back as the PNG's bytes, so that a test reads them. */
function rawEncoder(rgba: Uint8ClampedArray<ArrayBuffer>): Promise<Uint8Array> {
  return Promise.resolve(new Uint8Array(rgba));
}

/** A job over the fakes, any option a test's. */
function job(device: FakeStillDevice, overrides: Partial<StillJobOptions> = {}) {
  const renderer = new FakeStillRenderer(device, REQUEST);
  const steps: StillStep[] = [];
  const { saved, save } = recordingSave();
  const options: StillJobOptions = {
    device,
    renderer,
    request: REQUEST,
    setting: "low",
    frameIntervalNs: FAKE_FRAME_NS,
    encode: rawEncoder,
    save,
    onStep: (step) => {
      steps.push(step);
    },
    ...overrides,
  };
  return { renderer, steps, saved, run: () => runStillJob(options) };
}

/** The sum of `units`. */
function sum(units: ReadonlyArray<number>): number {
  return units.reduce((a, b) => a + b, 0);
}

describe("a still's job (R11.T10.b)", () => {
  it.each(["absent", "quantized", "full"] as const)(
    "submits one batch at a time, one an animation frame, each once the last is done (timer %s)",
    async (timer) => {
      const device = new FakeStillDevice(timer);
      const { run, renderer } = job(device);
      const outcome = await run();
      expect([outcome.kind, device.violations, sum(device.batches)]).toEqual([
        "saved",
        0,
        renderer.units.length,
      ]);
    },
  );

  it("makes every pipeline, asynchronously, before its first batch", async () => {
    const device = new FakeStillDevice("absent");
    const { run, renderer } = job(device);
    await run();
    expect([renderer.unprepared, renderer.asyncPipelines]).toEqual([0, FAKE_PASSES.length]);
  });

  it("measures five empty submits without timestamps, and none with them", async () => {
    const absent = new FakeStillDevice("absent");
    const timed = new FakeStillDevice("quantized");
    await job(absent).run();
    await job(timed).run();
    expect([absent.emptySubmits, timed.emptySubmits]).toEqual([5, 0]);
  });

  it("keeps each batch under the cap at the cost measured, growing from one unit", async () => {
    // 2 ms a unit beside 3 ms of overhead: the low setting's 6 ms cap holds three units at most.
    const device = new FakeStillDevice("absent", 2_000_000);
    await job(device).run();
    const capUnits = Math.floor(batchCapNs("low", FAKE_FRAME_NS) / 2_000_000);
    expect([device.batches[0], Math.max(...device.batches) <= capUnits]).toEqual([1, true]);
  });

  it("fills a batch to the cap with timestamps at the pass times measured", async () => {
    const device = new FakeStillDevice("full", 1_000_000);
    await job(device).run();
    // 1 ms a pass under the low cap less two quanta, 5.87 ms: five passes once all are measured.
    expect(Math.max(...device.batches)).toBe(5);
  });

  it("takes at most the batch cap it is given, after a suspension", async () => {
    const device = new FakeStillDevice("full", 10_000);
    await job(device, { maxUnits: 2 }).run();
    expect(Math.max(...device.batches)).toBe(2);
  });

  it("assembles the tiles into the still, cropping the padded column", async () => {
    const device = new FakeStillDevice("absent");
    const { run, saved, renderer } = job(device);
    await run();
    const pixels = saved[0]?.png ?? new Uint8Array(0);
    // Each pixel holds its tile's index plus one: tiles of 64 × 35 px and 36 × 35 px, row by row.
    const at = (x: number, y: number): number | undefined => pixels[(y * 100 + x) * 4];
    expect([
      pixels.byteLength,
      at(0, 0),
      at(99, 0),
      at(0, 69),
      at(99, 69),
      renderer.tiling.columns,
    ]).toEqual([100 * 70 * 4, 1, 2, 3, 4, 2]);
  });

  it("refuses tiles whose texels are not their textures' rows", () => {
    const tiling = planTiles({ widthPx: 100, heightPx: 70 }, 64, "low");
    expect(() => assembleTiles(tiling, [new Uint8Array(4)])).toThrow(RangeError);
  });

  it("steps through preparing, streaming, rendering and saving, its passes counted", async () => {
    const device = new FakeStillDevice("absent");
    const { run, steps, renderer } = job(device);
    await run();
    const total = renderer.units.length;
    expect([
      steps.map(({ kind }) => kind).filter((kind, i, all) => all[i - 1] !== kind),
      steps.at(-2),
      steps.at(-1),
    ]).toEqual([
      ["preparing", "streaming", "rendering", "saving"],
      { kind: "rendering", donePasses: total, totalPasses: total },
      { kind: "saving", donePasses: total, totalPasses: total },
    ]);
  });

  it("labels a still whose frustum had not streamed in time TERRAIN: STREAMING", async () => {
    const device = new FakeStillDevice("absent");
    const streaming = job(device, { streamingTimeoutS: 0.1 });
    streaming.renderer.terrainStreamed = false;
    const outcome = await streaming.run();
    expect([
      outcome.kind === "saved" && outcome.streamed,
      streaming.saved[0]?.label.annunciations.at(-1),
    ]).toEqual([false, STILL_TERRAIN_STREAMING]);
  });

  it("waits for the terrain no longer than the streaming timeout, 20 s of animation frames", async () => {
    const device = new FakeStillDevice("absent");
    const streaming = job(device);
    streaming.renderer.terrainStreamed = false;
    await streaming.run();
    const waitedNs = device.nowNs();
    expect([
      waitedNs >= STILL_STREAMING_TIMEOUT_S * 1e9,
      waitedNs < (STILL_STREAMING_TIMEOUT_S + 1) * 1e9,
    ]).toEqual([true, true]);
  });

  it("labels a still whose terrain had streamed with no streaming annunciation", async () => {
    const device = new FakeStillDevice("absent");
    const { run, saved } = job(device);
    await run();
    expect(saved[0]?.label.annunciations).toEqual(["ATMOSPHERE: APPROXIMATE"]);
  });

  it("abandons the job at a loss in flight, saving nothing and releasing the renderer", async () => {
    const device = new FakeStillDevice("absent", 10_000);
    device.loseAtBatch = 2;
    const { run, saved, renderer } = job(device);
    const outcome = await run();
    expect([
      outcome,
      saved.length,
      device.batches.length,
      renderer.disposed,
      device.lossListeners,
    ]).toEqual([{ kind: "lost", batchUnits: device.batches[2] }, 0, 3, true, 0]);
  });

  it("abandons the job at a loss while it awaits a frame, settling at once", async () => {
    const device = new FakeStillDevice("absent", 10_000);
    device.holdFrameAt = 4;
    const { run, saved, renderer } = job(device);
    const running = run();
    await vi.waitFor(() => {
      expect(device.holding).toBe(true);
    });
    device.lose();
    expect([await running, saved.length, renderer.disposed, device.lossListeners]).toEqual([
      expect.objectContaining({ kind: "lost" }),
      0,
      true,
      0,
    ]);
  });

  it("saves a still whose device was lost only after its tiles were read back", async () => {
    const device = new FakeStillDevice("absent");
    const { save } = recordingSave();
    const { run } = job(device, {
      save: (png, label) => {
        device.lose();
        return save(png, label);
      },
    });
    expect((await run()).kind).toBe("saved");
  });

  it("takes a failure that a loss reported in the next frame as the loss", async () => {
    const device = new FakeStillDevice("absent");
    const renderer = new FakeStillRenderer(device, REQUEST);
    renderer.readTile = () => {
      // A lost device fails its read-back before its loss is reported.
      queueMicrotask(() => {
        device.lose();
      });
      return Promise.reject(new Error("mapAsync: the device is lost"));
    };
    expect((await job(device, { renderer }).run()).kind).toBe("lost");
  });

  it("makes every pixel of the still opaque, whatever a tile's alpha carried", () => {
    const tiling = planTiles({ widthPx: 64, heightPx: 2 }, 64, "low");
    const pixels = assembleTiles(tiling, [new Uint8Array(64 * 2 * 4).fill(7)]);
    expect([
      pixels[2],
      pixels[3],
      pixels.every((byte, i) => (i % 4 === 3 ? byte === 255 : byte === 7)),
    ]).toEqual([7, 255, true]);
  });

  it("says why a refused still was not saved, and what to do where there is anything", async () => {
    const answer = (save: StillSaveAnswer) =>
      job(new FakeStillDevice("absent"), { save: () => Promise.resolve(save) }).run();
    expect([
      await answer({ kind: "refused", reason: "malformed-label" }),
      await answer({ kind: "refused", reason: "busy" }),
      await answer({ kind: "failed", reason: "ENOSPC" }),
      await answer({ kind: "failed", reason: "EIO" }),
    ]).toEqual([
      { kind: "not-saved", reason: "the console refused its record" },
      { kind: "not-saved", reason: "another still is being saved, TAKE STILL to retry" },
      { kind: "not-saved", reason: "the disk is full, free space and TAKE STILL to retry" },
      { kind: "not-saved", reason: "the still could not be written, TAKE STILL to retry" },
    ]);
  });
});

describe("a client's stills for the session (R11.T10.b)", () => {
  /** A session over the fakes, its clock the test's to move. */
  function session(timer: "absent" | "full" = "absent") {
    const device = new FakeStillDevice(timer, 10_000);
    const source = new FakeStillSource(device, REQUEST);
    const { saved, save } = recordingSave();
    let nowMs = 0;
    const reportError = vi.fn<(error: unknown) => void>();
    const stills = new StillSession({
      source,
      encode: rawEncoder,
      save,
      nowMs: () => nowMs,
      reportError,
    });
    const states: StillControlState[] = [];
    stills.subscribe(() => {
      states.push(stills.getSnapshot());
    });
    return {
      device,
      source,
      saved,
      stills,
      states,
      tick: (ms: number) => {
        nowMs += ms;
      },
    };
  }

  it("makes a still on request, and says it was saved", async () => {
    const { stills, saved } = session();
    const outcome = await stills.request();
    expect([outcome?.kind, saved.length, stills.getSnapshot()]).toEqual([
      "saved",
      1,
      {
        standing: "available",
        activity: { kind: "idle", last: { kind: "saved" } },
        heldBack: null,
      },
    ]);
  });

  it("holds TAKE STILL back with the view's cause while it gives no still, starts nothing, and offers it again", async () => {
    const { stills, source } = session();
    source.cause = "the view draws no photorealistic image";
    stills.refresh();
    const held = stills.getSnapshot().heldBack;
    const refused = await stills.request();
    source.cause = null;
    stills.refresh();
    expect([held, refused, source.snapshots, stills.getSnapshot().heldBack]).toEqual([
      "NOT AVAILABLE: the view draws no photorealistic image",
      null,
      0,
      null,
    ]);
  });

  it("holds TAKE STILL back from the start where the view gives no still as the session is made", () => {
    const device = new FakeStillDevice("absent");
    const source = new FakeStillSource(device, REQUEST);
    source.cause = "the view is not drawn";
    const stills = new StillSession({
      source,
      encode: rawEncoder,
      save: recordingSave().save,
      nowMs: () => 0,
      reportError: () => undefined,
    });
    expect(stills.getSnapshot().heldBack).toBe("NOT AVAILABLE: the view is not drawn");
  });

  it("holds TAKE STILL back where the view gives no still at the request, starting no job", async () => {
    const { stills, source } = session();
    source.cause = "stills draw no terrain yet";
    const outcome = await stills.request();
    expect([outcome, source.renderers.length, stills.getSnapshot()]).toEqual([
      null,
      0,
      {
        standing: "available",
        activity: { kind: "idle", last: null },
        heldBack: "NOT AVAILABLE: stills draw no terrain yet",
      },
    ]);
  });

  it("runs a still on while the view comes to give none, and keeps TAKE STILL held back after it", async () => {
    const { stills, saved, source } = session();
    const running = stills.request();
    source.cause = "the view draws no photorealistic image";
    stills.refresh();
    const outcome = await running;
    expect([outcome?.kind, saved.length, stills.getSnapshot().heldBack]).toEqual([
      "saved",
      1,
      "NOT AVAILABLE: the view draws no photorealistic image",
    ]);
  });

  it("makes one still at a time: a request while one runs starts nothing", async () => {
    const { stills, source } = session();
    const first = stills.request();
    const second = await stills.request();
    await first;
    expect([second, source.snapshots, source.renderers.length]).toEqual([null, 1, 1]);
  });

  it("uses the cells surveyed at the request, not those surveyed while the job runs", async () => {
    const { stills, source, saved } = session();
    const running = stills.request();
    source.current = {
      ...REQUEST,
      survey: [
        {
          body: "0200080020000000.0100",
          name: "TEST EARTH",
          level: 7,
          surveyedFraction: 0.9,
          bestResolutionM: 10,
        },
      ],
    };
    await running;
    expect(saved[0]?.label.survey).toEqual(REQUEST.survey);
  });

  it("suspends stills at a loss in flight, says so once, and caps the next at half its batch", async () => {
    const { stills, device, states } = session("full");
    device.loseAtBatch = 3;
    const lost = await stills.request();
    const inFlight = lost?.kind === "lost" ? lost.batchUnits : 0;
    const abandoned = states.filter(
      ({ activity }) => activity.kind === "idle" && activity.last?.kind === "abandoned",
    ).length;
    const before = device.batches.length;
    const again = await stills.request();
    const after = device.batches.slice(before);
    expect([
      stills.getSnapshot().standing,
      abandoned,
      again?.kind,
      after[0],
      Math.max(...after) <= Math.max(1, Math.floor(inFlight / 2)),
    ]).toEqual(["suspended", 1, "saved", 1, true]);
  });

  it("disables stills at a second loss in flight, and starts nothing after", async () => {
    const { stills, device, source } = session();
    device.loseAtBatch = 1;
    await stills.request();
    // The second still's second batch: the device counts batches across stills.
    device.loseAtBatch = device.batches.length + 1;
    await stills.request();
    const later = await stills.request();
    expect([stills.getSnapshot().standing, later, source.snapshots]).toEqual(["disabled", null, 2]);
  });

  it("is not suspended by a loss while no still runs", async () => {
    const { stills, device } = session();
    device.lose();
    await stills.request();
    expect(stills.getSnapshot().standing).toBe("available");
  });

  it("publishes a still's progress at most at 4 Hz, each new step at once", async () => {
    const { stills, states, tick } = session();
    const running = stills.request();
    tick(STILL_PROGRESS_INTERVAL_MS - 1);
    await running;
    const steps = states.map(({ activity }) =>
      activity.kind === "running" ? activity.step.kind : "ended",
    );
    // Every rendering step but the first and the last fell within one 250 ms.
    expect(steps).toEqual(["preparing", "streaming", "rendering", "rendering", "saving", "ended"]);
  });

  it("reports a job that failed for a reason not its own, and says the still was not saved", async () => {
    const device = new FakeStillDevice("absent");
    const reportError = vi.fn<(error: unknown) => void>();
    const stills = new StillSession({
      source: new FakeStillSource(device, REQUEST),
      encode: () => Promise.reject(new Error("no encoder")),
      save: recordingSave().save,
      nowMs: () => 0,
      reportError,
    });
    await stills.request();
    expect([reportError.mock.calls.length, stills.getSnapshot().activity]).toEqual([
      1,
      {
        kind: "idle",
        last: { kind: "not-saved", reason: "the still could not be made, TAKE STILL to retry" },
      },
    ]);
  });
});
