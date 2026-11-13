import { describe, expect, it } from "vitest";

import { longTaskFunctions, type PageDebugger, PageProfiler, readPageProfile } from "./pageProfile";

/** A V8 profile as `Profiler.stop` returns it: a reply's 3 ms in `writeBuffer`, then idle. */
const STOPPED = {
  profile: {
    nodes: [
      { id: 1, callFrame: { functionName: "(root)", url: "" }, children: [2, 3] },
      { id: 2, callFrame: { functionName: "(idle)", url: "" } },
      {
        id: 3,
        callFrame: { functionName: "onMessage", url: "file:///app/out/a-x.js", lineNumber: 8 },
        children: [4],
      },
      { id: 4, callFrame: { functionName: "writeBuffer", url: "" } },
    ],
    startTime: 10_000_000,
    endTime: 10_006_000,
    samples: [3, 4, 4, 2, 2, 2],
    timeDeltas: [0, 1_000, 1_000, 1_000, 1_000, 1_000],
  },
};

/** A debugger that answers the profile's commands, `stopped` for its stop, and logs them. */
function fakeDebugger(
  stopped: unknown,
  pageNowMs = 506,
): {
  readonly debugger: PageDebugger;
  readonly sent: string[];
} {
  const sent: string[] = [];
  let attached = false;
  return {
    debugger: {
      attach: () => {
        sent.push("attach");
        attached = true;
      },
      detach: () => {
        sent.push("detach");
        attached = false;
      },
      isAttached: () => attached,
      sendCommand: (method) => {
        sent.push(method);
        if (method === "Runtime.evaluate") {
          return Promise.resolve({ result: { type: "number", value: pageNowMs } });
        }
        return Promise.resolve(method === "Profiler.stop" ? stopped : {});
      },
    },
    sent,
  };
}

describe("the page's CPU profile over the run (lane perf-page-long-tasks)", () => {
  it("places the samples on the page's clock by its reading as the profile ends", () => {
    const profile = readPageProfile(STOPPED, 506);
    expect([profile?.sampleAtMs, profile?.endMs, profile?.nodes.get(3)]).toEqual([
      [500, 501, 502, 503, 504, 505],
      506,
      { parent: 1, name: "onMessage a-x.js:9", named: true },
    ]);
    expect(profile === null ? null : longTaskFunctions(profile, 500, 503)).toEqual({
      sampledMs: 3,
      entry: "onMessage a-x.js:9",
      self: [
        { name: "writeBuffer", ms: 2 },
        { name: "onMessage a-x.js:9", ms: 1 },
      ],
    });
  });

  it("reads no profile not shaped as V8's", () => {
    expect(
      [
        null,
        { profile: { ...STOPPED.profile, samples: [3] } },
        { profile: { ...STOPPED.profile, nodes: [{ id: "1" }] } },
        { profile: { ...STOPPED.profile, endTime: "late" } },
      ].map((stopped) => readPageProfile(stopped, 0)),
    ).toEqual([null, null, null, null]);
  });

  it("starts once at one sample a millisecond, stops and reads the page's clock first, and releases the debugger it attached", async () => {
    const { debugger: page, sent } = fakeDebugger(STOPPED);
    const profiler = new PageProfiler(page, () => undefined);
    await profiler.start();
    await profiler.start();
    const profile = await profiler.stop();
    expect(sent).toEqual([
      "attach",
      "Profiler.enable",
      "Profiler.setSamplingInterval",
      "Profiler.start",
      "Runtime.evaluate",
      "Profiler.stop",
      "Profiler.disable",
      "detach",
    ]);
    expect(profile?.sampleAtMs.at(0)).toBe(500);
    expect(await profiler.stop()).toBeNull();
  });

  it("leaves a debugger attached before it attached, its profiler disabled", async () => {
    const { debugger: page, sent } = fakeDebugger(STOPPED);
    page.attach();
    const profiler = new PageProfiler(page, () => undefined);
    await profiler.start();
    await profiler.stop();
    expect([sent.at(-1), page.isAttached()]).toEqual(["Profiler.disable", true]);
  });

  it("has none, and says why, where it does not start or does not read", async () => {
    const lines: string[] = [];
    const refusing: PageDebugger = {
      attach: () => {
        throw new Error("another debugger is attached");
      },
      detach: () => undefined,
      isAttached: () => false,
      sendCommand: () => Promise.resolve({}),
    };
    const unstarted = new PageProfiler(refusing, (line) => lines.push(line));
    await unstarted.start();
    const unread = new PageProfiler(fakeDebugger({ profile: {} }).debugger, (line) =>
      lines.push(line),
    );
    await unread.start();
    expect([
      await unstarted.stop(),
      await unread.stop(),
      await new PageProfiler(refusing, () => undefined).stop(),
    ]).toEqual([null, null, null]);
    expect(lines).toEqual([
      "views check: the page's CPU profile did not start: Error: another debugger is attached",
      "views check: the page's CPU profile could not be read",
    ]);
  });
});
