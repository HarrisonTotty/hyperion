import type { CameraReportDto } from "@hyperion/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FIXTURE_EARTH, FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import { viewId } from "../../view/camera/state";
import { CAMERA_REPORT_INTERVAL_MS, CameraReporter, MAX_SCENE_CAMERAS } from "./cameraReports";
import type { SceneKinematics } from "./model";

const FORWARD = viewId("forward");
const AFT = viewId("aft");

/** A camera in the fixture system's frame at `x` metres, at `seconds`. */
function inSystem(x: number, seconds = 100): SceneKinematics {
  return {
    position: { kind: "system", system: FIXTURE_SYSTEM, offsetM: { x, y: 0, z: 0 } },
    velocityMPerS: { x: 1, y: 2, z: 3 },
    time: { seconds, nanos: 0 },
  };
}

/** A camera in the fixture Earth's frame at `x` metres. */
function nearEarth(x: number): SceneKinematics {
  return {
    position: { kind: "body", body: FIXTURE_EARTH, offsetM: { x, y: 0, z: 0 } },
    velocityMPerS: { x: 0, y: 0, z: 0 },
    time: { seconds: 100, nanos: 0 },
  };
}

interface Sent {
  readonly atMs: number;
  readonly cameras: ReadonlyArray<CameraReportDto>;
}

/** A reporter attached to a recorder of what it sends and when. */
function recorded(): { reporter: CameraReporter; sent: Sent[] } {
  const sent: Sent[] = [];
  const reporter = new CameraReporter();
  reporter.attach((cameras) => {
    sent.push({ atMs: performance.now(), cameras });
  });
  return { reporter, sent };
}

describe("CameraReporter", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("never reports more than 4 times a second under 60 pose changes a second", () => {
    const { reporter, sent } = recorded();
    const startMs = performance.now();

    for (let change = 0; change < 180; change += 1) {
      reporter.report(FORWARD, inSystem(change));
      // Whole milliseconds, 16 or 17 apart, so that the fake clock adds no rounding of its own.
      vi.advanceTimersByTime(
        Math.round(((change + 1) * 1_000) / 60) - Math.round((change * 1_000) / 60),
      );
    }
    vi.advanceTimersByTime(CAMERA_REPORT_INTERVAL_MS);

    for (const { atMs } of sent) {
      const within = sent.filter((other) => other.atMs >= atMs && other.atMs < atMs + 1_000);
      expect(within.length).toBeLessThanOrEqual(4);
    }
    expect(sent.length).toBeGreaterThanOrEqual(12);
    expect(sent.at(-1)?.cameras[0]?.pose.position).toEqual({
      frame: "system",
      system: FIXTURE_SYSTEM,
      offset_m: [179, 0, 0],
    });
    expect(sent.at(-1)?.atMs).toBeLessThanOrEqual(startMs + 3_000 + CAMERA_REPORT_INTERVAL_MS);
  });

  it("reports a change of frame at once", () => {
    const { reporter, sent } = recorded();
    reporter.report(FORWARD, inSystem(1));
    reporter.report(FORWARD, inSystem(2));

    expect(sent).toHaveLength(1);

    reporter.report(FORWARD, nearEarth(7e6));

    expect(sent).toHaveLength(2);
    expect(sent[1]?.cameras).toEqual([
      {
        view: 0,
        pose: {
          position: { frame: "body", body: FIXTURE_EARTH, offset_m: [7e6, 0, 0] },
          velocity_m_s: [0, 0, 0],
          time: { seconds: 100, nanos: 0 },
        },
      },
    ]);
  });

  it("reports every view's camera in each report, each under its own slot", () => {
    const { reporter, sent } = recorded();
    reporter.report(FORWARD, inSystem(1));
    reporter.report(AFT, inSystem(2));

    expect(sent.at(-1)?.cameras.map(({ view }) => view)).toEqual([0, 1]);
    expect(sent.at(-1)?.cameras[1]?.pose.velocity_m_s).toEqual([1, 2, 3]);
  });

  it("leaves out a removed view at once and gives its slot to the next view", () => {
    const { reporter, sent } = recorded();
    reporter.report(FORWARD, inSystem(1));
    reporter.report(AFT, inSystem(2));
    reporter.remove(FORWARD);

    expect(sent.at(-1)?.cameras.map(({ view }) => view)).toEqual([1]);

    reporter.report(viewId("port"), inSystem(3));

    expect(sent.at(-1)?.cameras.map(({ view }) => view)).toEqual([0, 1]);
  });

  it("refuses a view beyond the eighth", () => {
    const { reporter } = recorded();
    for (let view = 0; view < MAX_SCENE_CAMERAS; view += 1) {
      reporter.report(viewId(`view ${view}`), inSystem(view));
    }

    expect(() => {
      reporter.report(viewId("ninth"), inSystem(9));
    }).toThrow(RangeError);
  });

  it("holds poses while detached and sends them at once when attached", () => {
    const reporter = new CameraReporter();
    reporter.report(FORWARD, inSystem(1));
    reporter.report(FORWARD, inSystem(5));
    const sent: Array<ReadonlyArray<CameraReportDto>> = [];

    reporter.attach((cameras) => {
      sent.push(cameras);
    });

    expect(sent).toHaveLength(1);
    expect(sent[0]?.[0]?.pose.position).toMatchObject({ offset_m: [5, 0, 0] });

    reporter.detach();
    reporter.report(FORWARD, inSystem(6));
    vi.advanceTimersByTime(CAMERA_REPORT_INTERVAL_MS * 4);

    expect(sent).toHaveLength(1);
  });
});
