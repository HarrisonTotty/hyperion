import { afterEach, describe, expect, it, vi } from "vitest";

import { ATMOSPHERE_VIEW_PASS } from "../view/atmosphere/hillaire";
import type { PassTimes } from "../view/engine/types";
import {
  type FrameRecord,
  keptOf,
  perFrameSums,
  ResolveLedger,
  runTimed,
  type Timed,
  type TimedForm,
  type TimedFrame,
  timedFrameOf,
  type TimerEngine,
  viewPassesOf,
} from "./frameTimingLedger";

/** A timer whose submissions the test makes and whose resolves it reports, in any order. */
class StubTimer implements TimerEngine {
  passTimesFrame = 0;
  readonly #listeners = new Set<(times: PassTimes) => void>();

  onPassTimes(listener: (times: PassTimes) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Makes `count` submissions, each resolving its own timestamps; their numbers. */
  submit(count: number): number[] {
    return Array.from({ length: count }, () => {
      this.passTimesFrame += 1;
      return this.passTimesFrame;
    });
  }

  /** Reports resolve `frame`'s passes, each a label and its ns. */
  report(frame: number, passes: ReadonlyArray<readonly [string, number]>): void {
    const times: PassTimes = {
      frame,
      timer: "full",
      passes: passes.map(([label, ns]) => ({ label, ns, bracketed: false })),
    };
    for (const listener of this.#listeners) {
      listener(times);
    }
  }
}

/** A frame of `passes`, as `timedFrameOf` makes one. */
function frameOf(passes: ReadonlyArray<readonly [string, number]>): TimedFrame {
  return { passes: passes.map(([label, ns]) => ({ label, ns })), cpuMs: 0, timers: ["full"] };
}

/** A small form, so that a run is a few frames: one warm-up and two timed a sweep. */
const SMALL_FORM: TimedForm = { warmUp: 1, timed: 2, ahead: 3, waitMs: 2_000, lastWaitMs: 5_000 };

afterEach(() => {
  vi.useRealTimers();
});

describe("the ledger of the timer's resolves", () => {
  it("gives a frame the resolves after the count before its draw up to the count after it", () => {
    const timer = new StubTimer();
    const ledger = new ResolveLedger(timer);
    timer.submit(1);
    const before = ledger.count;
    timer.submit(2);
    const record = ledger.frame(before, 1.5);
    timer.submit(1);
    expect([record.first, record.last, record.resolves.length]).toEqual([2, 3, 2]);
  });

  it("puts a frame's passes in its resolves' order, whatever order they arrive in", () => {
    const timer = new StubTimer();
    const ledger = new ResolveLedger(timer);
    const before = ledger.count;
    const [first, second] = timer.submit(2);
    const record = ledger.frame(before, 1.5);
    timer.report(second ?? 0, [["discs", 30]]);
    timer.report(first ?? 0, [
      ["sky", 20],
      ["bodies", 21],
    ]);
    expect(timedFrameOf(record)).toEqual({
      passes: [
        { label: "sky", ns: 20 },
        { label: "bodies", ns: 21 },
        { label: "discs", ns: 30 },
      ],
      cpuMs: 1.5,
      timers: ["full", "full"],
    });
  });

  it("passes over a resolve no frame owns", () => {
    const timer = new StubTimer();
    const ledger = new ResolveLedger(timer);
    const [fence] = timer.submit(1);
    const before = ledger.count;
    const [own] = timer.submit(1);
    const record = ledger.frame(before, 0);
    timer.report(fence ?? 0, [["fence", 5]]);
    timer.report(99, [["elsewhere", 6]]);
    timer.report(own ?? 0, [["sky", 7]]);
    expect(timedFrameOf(record)?.passes).toEqual([{ label: "sky", ns: 7 }]);
  });

  it("drops and counts a frame with a resolve held back", () => {
    const timer = new StubTimer();
    const ledger = new ResolveLedger(timer);
    const records: FrameRecord[] = [];
    for (const passes of [1, 2]) {
      const before = ledger.count;
      timer.submit(passes);
      records.push(ledger.frame(before, 0));
    }
    const [whole, partial] = records;
    timer.report(whole?.first ?? 0, [["sky", 4_000]]);
    timer.report(partial?.first ?? 0, [["sky", 4_000]]);
    const combination: Timed<string> = { item: "a combination", records };
    const kept = keptOf(combination);
    expect([kept.frames.length, kept.missing]).toEqual([1, 1]);
    expect(perFrameSums(combination)).toBe("4 -");
  });

  it("drops a frame that made no resolve", () => {
    const ledger = new ResolveLedger(new StubTimer());
    expect(timedFrameOf(ledger.frame(ledger.count, 0))).toBeNull();
  });

  it("settles a frame's wait at its last resolve, and a held-back one's at the limit", async () => {
    vi.useFakeTimers();
    const timer = new StubTimer();
    const ledger = new ResolveLedger(timer);
    const before = ledger.count;
    const [only] = timer.submit(1);
    const arrived = ledger.frame(before, 0);
    const waiting = ledger.settled(arrived, 2_000);
    timer.report(only ?? 0, [["sky", 1]]);
    await expect(waiting).resolves.toBe(true);
    const next = ledger.count;
    timer.submit(1);
    const held = ledger.frame(next, 0);
    const timedOut = ledger.settled(held, 2_000);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(timedOut).resolves.toBe(false);
  });
});

describe("a frame's atmosphere view passes", () => {
  it("reads a lone view pass as the ray march", () => {
    expect(viewPassesOf(frameOf([[ATMOSPHERE_VIEW_PASS, 7]]))).toEqual({
      skyNs: 0,
      aerialNs: 0,
      marchNs: 7,
      moreNs: 0,
    });
  });

  it("reads three as the sky view, the aerial perspective and the march, in their order", () => {
    const frame = frameOf([
      [ATMOSPHERE_VIEW_PASS, 1],
      ["atmosphere composite", 9],
      [ATMOSPHERE_VIEW_PASS, 2],
      [ATMOSPHERE_VIEW_PASS, 3],
    ]);
    expect(viewPassesOf(frame)).toEqual({ skyNs: 1, aerialNs: 2, marchNs: 3, moreNs: 0 });
  });

  it("sums the view passes past the march", () => {
    const frame = frameOf([
      [ATMOSPHERE_VIEW_PASS, 1],
      [ATMOSPHERE_VIEW_PASS, 2],
      [ATMOSPHERE_VIEW_PASS, 3],
      [ATMOSPHERE_VIEW_PASS, 4],
      [ATMOSPHERE_VIEW_PASS, 5],
    ]);
    expect(viewPassesOf(frame).moreNs).toBe(9);
  });
});

/** What a run of `runTimed` drew, on a stub timer that reports each frame 5 ms after its draw. */
interface DrawLog {
  /** Each draw's combination, in order. */
  readonly items: string[];
  /** Each combination's `setUp` call, in order. */
  readonly setUps: string[];
  /** Each draw's resolve number. */
  readonly resolves: number[];
  /** The frames drawn but not yet reported when each draw began. */
  readonly inFlight: number[];
  /** Whether the frame three before had been reported when each draw began (from the fourth). */
  readonly aheadReported: boolean[];
  /** The fake clock at each draw, ms. */
  readonly at: number[];
}

/** Runs `runTimed` over `items` in {@link SMALL_FORM} on a stub timer under fake timers. */
async function drawLogOf(items: readonly string[]): Promise<{
  readonly log: DrawLog;
  readonly combinations: ReadonlyArray<Timed<string>>;
}> {
  vi.useFakeTimers();
  const timer = new StubTimer();
  const ledger = new ResolveLedger(timer);
  const reported = new Set<number>();
  const log: DrawLog = {
    items: [],
    setUps: [],
    resolves: [],
    inFlight: [],
    aheadReported: [],
    at: [],
  };
  const combinations = items.map((item): Timed<string> => ({ item, records: [] }));
  const run = runTimed(
    ledger,
    combinations,
    (item) => {
      log.setUps.push(item);
    },
    (item) => {
      const n = log.resolves.length;
      log.inFlight.push(log.resolves.filter((resolve) => !reported.has(resolve)).length);
      const ahead = log.resolves[n - SMALL_FORM.ahead];
      if (ahead !== undefined) {
        log.aheadReported.push(reported.has(ahead));
      }
      log.at.push(Date.now());
      log.items.push(item);
      const [resolve] = timer.submit(1);
      const number = resolve ?? 0;
      log.resolves.push(number);
      setTimeout(() => {
        reported.add(number);
        timer.report(number, [["sky", 1_000]]);
      }, 5);
      return 0;
    },
    SMALL_FORM,
  );
  await vi.runAllTimersAsync();
  await run;
  return { log, combinations };
}

describe("the timed form", () => {
  it("draws both sweeps, the second reversed, each combination's warm-up then timed frames", async () => {
    const { log } = await drawLogOf(["a", "b"]);
    expect(log.items).toEqual(["a", "a", "a", "b", "b", "b", "b", "b", "b", "a", "a", "a"]);
    expect(log.setUps).toEqual(["a", "b", "b", "a"]);
  });

  it("keeps each combination's timed frames, not its warm-up, in timed order", async () => {
    const { log, combinations } = await drawLogOf(["a", "b"]);
    const resolvesOf = (item: string): number[] =>
      combinations.find((combination) => combination.item === item)?.records.map((r) => r.first) ??
      [];
    // Draws 1, 2 and 10, 11 are a's timed frames; draws 0 and 9 its warm-up.
    expect(resolvesOf("a")).toEqual([1, 2, 10, 11].map((i) => log.resolves[i]));
    expect(resolvesOf("b")).toEqual([4, 5, 7, 8].map((i) => log.resolves[i]));
  });

  it("waits for frame n − 3's resolves before drawing frame n, two frames queued at most", async () => {
    const { log } = await drawLogOf(["a", "b"]);
    expect(log.aheadReported).toHaveLength(log.items.length - SMALL_FORM.ahead);
    expect(log.aheadReported.every((reported) => reported)).toBe(true);
    expect(Math.max(...log.inFlight)).toBeLessThanOrEqual(SMALL_FORM.ahead - 1);
    // The fourth frame waited for the first's report, 5 ms after it.
    expect(log.at[3]).toBeGreaterThanOrEqual((log.at[0] ?? 0) + 5);
  });
});
