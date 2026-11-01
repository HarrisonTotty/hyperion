import { describe, expect, it } from "vitest";

import { ATMOSPHERE_CHECK_TIMING } from "./atmosphereCheckRun";
import { HANDOVER_CHECK_TIMING, SERVED_HANDOVER_CHECK_TIMING } from "./handoverCheckRun";
import {
  atmosphereCheckTimingOf,
  handoverCheckTimingOf,
  inspectionCheckTimingOf,
  SOFTWARE_WAIT_FACTOR,
  terrainCheckTimingOf,
  viewsCheckTimingOf,
} from "./softwareTiming";
import { INSPECTION_CHECK_TIMING } from "./inspectionCheckRun";
import { TERRAIN_CHECK_TIMING } from "./terrainCheckRun";
import { VIEWS_CHECK_TIMING } from "./viewsCheckRun";

const OTHER_MODES = ["default", "vulkan", "safe"] as const;

describe("the software mode's timings (R01.T16)", () => {
  it("stretch by five, fixed (main's ruling, 2026-10-29)", () => {
    expect(SOFTWARE_WAIT_FACTOR).toBe(5);
  });

  it("stretch T20's control wait alone, its measured windows as they are", () => {
    for (const timing of [VIEWS_CHECK_TIMING.smoke, VIEWS_CHECK_TIMING.full]) {
      expect(viewsCheckTimingOf(timing, "software")).toEqual({
        ...timing,
        waitMs: 5 * timing.waitMs,
      });
    }
  });

  it("stretch the atmosphere run's wait and AUTO's settle limit, its holds as they are", () => {
    for (const timing of [ATMOSPHERE_CHECK_TIMING.smoke, ATMOSPHERE_CHECK_TIMING.full]) {
      expect(atmosphereCheckTimingOf(timing, "software")).toEqual({
        ...timing,
        waitMs: 5 * timing.waitMs,
        mediaWaitMs: 5 * timing.mediaWaitMs,
        settleMaxMs: 5 * timing.settleMaxMs,
      });
    }
  });

  it("stretch the terrain run's control and restore waits, its script times as they are", () => {
    for (const timing of [TERRAIN_CHECK_TIMING.smoke, TERRAIN_CHECK_TIMING.full]) {
      expect(terrainCheckTimingOf(timing, "software")).toEqual({
        ...timing,
        waitMs: 5 * timing.waitMs,
        restoreMs: 5 * timing.restoreMs,
      });
    }
  });

  it("stretch the hand-over check's waits and a look's overrun", () => {
    expect(handoverCheckTimingOf(HANDOVER_CHECK_TIMING, "software")).toEqual({
      ...HANDOVER_CHECK_TIMING,
      waitMs: 300_000,
      surveyWaitMs: 300_000,
      overrunMs: 300_000,
    });
  });

  it("stretch a served world's survey wait of 5 minutes to 25 (R10.T15.b)", () => {
    expect([
      SERVED_HANDOVER_CHECK_TIMING.surveyWaitMs,
      handoverCheckTimingOf(SERVED_HANDOVER_CHECK_TIMING, "software").surveyWaitMs,
    ]).toEqual([300_000, 1_500_000]);
  });

  it("stretch the inspection run's waits for a control, its drawing, AUTO and its ground (the owner's request)", () => {
    expect(inspectionCheckTimingOf(INSPECTION_CHECK_TIMING, "software")).toEqual({
      ...INSPECTION_CHECK_TIMING,
      waitMs: 5 * INSPECTION_CHECK_TIMING.waitMs,
      drawWaitMs: 5 * INSPECTION_CHECK_TIMING.drawWaitMs,
      settleMaxMs: 5 * INSPECTION_CHECK_TIMING.settleMaxMs,
      groundWaitMs: 5 * INSPECTION_CHECK_TIMING.groundWaitMs,
    });
  });

  it("leave every other mode's timings as they are", () => {
    for (const mode of OTHER_MODES) {
      expect([
        viewsCheckTimingOf(VIEWS_CHECK_TIMING.full, mode),
        atmosphereCheckTimingOf(ATMOSPHERE_CHECK_TIMING.smoke, mode),
        terrainCheckTimingOf(TERRAIN_CHECK_TIMING.full, mode),
        handoverCheckTimingOf(HANDOVER_CHECK_TIMING, mode),
        inspectionCheckTimingOf(INSPECTION_CHECK_TIMING, mode),
      ]).toEqual([
        VIEWS_CHECK_TIMING.full,
        ATMOSPHERE_CHECK_TIMING.smoke,
        TERRAIN_CHECK_TIMING.full,
        HANDOVER_CHECK_TIMING,
        INSPECTION_CHECK_TIMING,
      ]);
    }
  });
});
