import { describe, expect, it, vi } from "vitest";

import type { TerrainCheckRecord } from "../../../../../preload/api";
import { norm } from "../../../geometry/vec3";
import {
  TERRAIN_TEST_SCRIPT,
  terrainShipBodyFixedM,
  terrainTestGolden,
} from "../../../view/scenes/terrainScene";
import { TERRAIN_COMPUTATION_FAILED } from "../../../view/terrain/annunciation";
import { handoverTauPx, limbDistanceM } from "../../../view/terrain/handover";
import {
  recordTerrainCheck,
  TERRAIN_CHECK_PLAN,
  type TerrainCheckEnding,
  terrainHandover,
} from "./terrainCheckRun";

const RECORD: TerrainCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  phases: [],
  faults: [],
  terrainFaults: [],
  restoresAtMs: [],
  loss: {
    destroyedAtMs: null,
    destroyedAtScriptS: null,
    notDestroyed: "a test",
    blocksToCome: 0,
    bakesInFlight: 0,
    after: { fieldAsks: 0, fieldInits: 0, fieldChunks: 0, blocksSent: 0 },
    workers: 0,
    terrainAgainAtMs: null,
    masks: {
      before: null,
      afterRestore: null,
      end: null,
      expected: 0,
      beforeKept: false,
      endAsExpected: false,
    },
  },
  teardown: { disposed: true, workersMade: 0, workersLive: 0, sessionGpuBytes: 0, liveBytes: [] },
  handover: null,
};

/** The ending's calls, in order. */
function ending(): TerrainCheckEnding & { readonly calls: unknown[][] } {
  const calls: unknown[][] = [];
  return {
    calls,
    writeTerrainResults: (record) => {
      calls.push(["write", record]);
      return Promise.resolve({ json: "a.json", markdown: "a.md" });
    },
    end: (outcome) => {
      calls.push(["end", outcome]);
      return Promise.resolve();
    },
  };
}

describe("the terrain run's plan (R10.T16.d)", () => {
  it("runs each setting's phases in the ruled order and views", () => {
    expect(
      Object.entries(TERRAIN_CHECK_PLAN).map(([setting, plans]) => [
        setting,
        plans.map((plan) => [plan.name, plan.primary, ...plan.instruments]),
      ]),
    ).toEqual([
      [
        "high",
        [
          ["terrain-alone", "photorealistic", null, null],
          ["terrain-photoreal-wireframe", "photorealistic", "photorealistic", "wireframe"],
          ["terrain-loss", "photorealistic", null, null],
        ],
      ],
      [
        "low",
        [
          ["terrain-alone", "photorealistic", null, null],
          ["terrain-wireframe", "photorealistic", "wireframe", null],
          ["terrain-loss", "photorealistic", null, null],
        ],
      ],
    ]);
  });

  it.each(["high", "low"] as const)(
    "finds where the descent crosses TEST MOON's hand-over range on %s",
    (setting) => {
      const handover = terrainHandover(1126, setting);
      if (handover === null) {
        throw new Error("the descent crosses the range");
      }
      const { header } = terrainTestGolden();
      const radPerPx = (2 * Math.tan(Math.PI / 6)) / 1126;
      expect(handover.rangeM).toBeCloseTo(
        header.realised_relief_m / (handoverTauPx("photorealistic", setting) * radPerPx),
        3,
      );
      const limb = limbDistanceM(
        norm(terrainShipBodyFixedM(handover.scriptS)),
        header.equatorial_radius_m,
      );
      expect(limb / handover.rangeM).toBeCloseTo(1, 6);
      expect(handover.scriptS).toBeGreaterThan(0);
      expect(handover.scriptS).toBeLessThan(TERRAIN_TEST_SCRIPT.descentEndS);
    },
  );

  it("finds none for a canvas of no width", () => {
    expect(terrainHandover(0, "high")).toBeNull();
  });
});

describe("the terrain run's ending", () => {
  it("writes the record and ends the run, which the main process judges", async () => {
    const api = ending();
    await recordTerrainCheck(api, () => Promise.resolve(RECORD), new AbortController().signal);
    expect(api.calls).toEqual([
      ["write", RECORD],
      ["end", { status: "pass" }],
    ]);
  });

  it("writes a record holding the terrain's fault as it stands, for the main process to fail (R10.T16.e)", async () => {
    const api = ending();
    const faulted: TerrainCheckRecord = {
      ...RECORD,
      terrainFaults: [{ atMs: 40_000, text: TERRAIN_COMPUTATION_FAILED, views: ["view"] }],
    };
    await recordTerrainCheck(api, () => Promise.resolve(faulted), new AbortController().signal);
    expect(api.calls[0]).toEqual(["write", faulted]);
  });

  it("fails the run with the reason the script threw", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = ending();
    await recordTerrainCheck(
      api,
      () => Promise.reject(new Error("views check: TERRAIN TEST was not chosen")),
      new AbortController().signal,
    );
    expect(api.calls).toEqual([
      ["end", { status: "fail", reason: "views check: TERRAIN TEST was not chosen" }],
    ]);
  });

  it("ends nothing once stopped", async () => {
    const api = ending();
    const stop = new AbortController();
    stop.abort();
    await recordTerrainCheck(api, () => Promise.resolve(RECORD), stop.signal);
    expect(api.calls).toEqual([]);
  });
});
