import { RequestClient } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import { freeRateMPerS, MAX_FREE_STEP_S } from "../../../view/camera/freeCamera";
import {
  closeRateStep,
  INSPECTION_CHECK_TIMING,
  inspectionCaptureLabel,
  inspectionGround,
  positionRangeM,
  recordInspectionCheck,
  runInspectionCheck,
} from "./inspectionCheckRun";

/** A body the launch names. */
const BODY = "61ffd967fe000023.0100";

/** The main process's rule for a capture's label (`viewsCheckRecord.ts`' `CAPTURE_LABEL`). */
const CAPTURE_LABEL = /^[a-z0-9-]{1,48}$/u;

/** A request client whose server answers every request with a list of no universe. */
function noUniverses(): RequestClient {
  const client: RequestClient = new RequestClient((message) => {
    if (message.type === "request") {
      queueMicrotask(() => {
        client.handleServerMessage({
          type: "response",
          id: message.id,
          body: { kind: "list_universes", universes: [], server_generator_version: 24 },
        });
      });
    }
    return true;
  });
  return client;
}

describe("the inspection run (the owner's request)", () => {
  it("names its capture for the body's system and index, as a capture may be named", () => {
    const label = inspectionCaptureLabel(BODY);
    expect(label).toBe("inspection-61ffd967fe000023-0100");
    expect(CAPTURE_LABEL.test(label)).toBe(true);
  });

  it("names its maximized and GRID OFF captures as a capture may be named (2026-11-01)", () => {
    const label = inspectionCaptureLabel(BODY);
    expect(
      [`${label}-maximized`, `${label}-grid-off`].map((each) => CAPTURE_LABEL.test(each)),
    ).toEqual([true, true]);
  });

  it("fails naming the universe where the server lists none of that ID", async () => {
    const openSystem = vi.fn<() => void>();
    await expect(
      runInspectionCheck({
        api: { capture: () => Promise.resolve() },
        views: { drawTimes: () => [] },
        requests: noUniverses(),
        served: { universe: "3504954ff4feabf4", body: BODY },
        openSystem,
        document,
        nowMs: () => 0,
        sleep: () => Promise.resolve(),
        signal: new AbortController().signal,
        timing: INSPECTION_CHECK_TIMING,
        profile: null,
      }),
    ).rejects.toThrow("views check: the server lists no universe 3504954ff4feabf4");
    expect(openSystem).not.toHaveBeenCalled();
  });

  it("ends passed once its capture is taken", async () => {
    const end = vi.fn<() => Promise<void>>(() => Promise.resolve());

    await recordInspectionCheck(
      { end },
      () =>
        Promise.resolve({
          label: "a",
          frame: null,
          exposure: null,
          statements: [],
          ground: [],
          closeGround: null,
        }),
      new AbortController().signal,
      null,
    );

    expect(end.mock.calls).toEqual([[{ status: "pass" }]]);
  });

  it("writes its frame profile's timing file before it ends passed (the owner's report)", async () => {
    const order: string[] = [];
    await recordInspectionCheck(
      {
        end: (outcome) => {
          order.push(`end ${outcome.status}`);
          return Promise.resolve();
        },
      },
      () =>
        Promise.resolve({
          label: "a",
          frame: null,
          exposure: null,
          statements: [],
          ground: [],
          closeGround: null,
        }),
      new AbortController().signal,
      {
        write: () => {
          order.push("write timing");
          return Promise.resolve();
        },
      },
    );

    expect(order).toEqual(["write timing", "end pass"]);
  });

  it("ends failed with the reason the run threw", async () => {
    const end = vi.fn<() => Promise<void>>(() => Promise.resolve());
    vi.spyOn(console, "error").mockImplementation(() => undefined);

    await recordInspectionCheck(
      { end },
      () => Promise.reject(new Error("views check: VIEW BODY was held back")),
      new AbortController().signal,
      null,
    );

    expect(end.mock.calls).toEqual([
      [{ status: "fail", reason: "views check: VIEW BODY was held back" }],
    ]);
  });
});

describe("the inspection run's reading of the ground and the camera's range", () => {
  it("reads the ground as surveyed by its ELEVATION, not by NOT SURVEYED", () => {
    expect(
      [
        ["SURVEY COVERAGE 100 % · FINEST 50 m", "GROUND AT LINE OF SIGHT", "ELEVATION 1204 ± 40 m"],
        ["SURVEY COVERAGE: PENDING", "GROUND AT LINE OF SIGHT", "NOT SURVEYED"],
        ["TERRAIN: NOT YET MODELLED"],
        [],
      ].map(inspectionGround),
    ).toEqual(["surveyed", "waiting", "not modelled", "waiting"]);
  });

  it("reads the POSITION line's range in its unit, its digits grouped", () => {
    expect(
      ["6.69 Mm 023° -12°", "1,000 km 001° +00°", "87.0 km 359° +89°", "—", null].map(
        positionRangeM,
      ),
    ).toEqual([6.69e6, 1e6, 87_000, null, null]);
  });

  it("closes in by holds whose one camera step flies at most half of what is left", () => {
    const steps = [3e3, 1e5, 1.55e7].map((remainingM) => {
      const flownM = freeRateMPerS(closeRateStep(remainingM)) * MAX_FREE_STEP_S;
      return flownM <= remainingM / 2 && flownM > remainingM / 2 / Math.sqrt(10);
    });
    expect(steps).toEqual([true, true, true]);
  });
});
