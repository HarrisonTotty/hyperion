import { PERF_PINS, PERF_SCENE_HARNESS, PERF_SCENE_IDS, VIEW_ROLES } from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import { INSTRUMENT_RATE_HZ, WIREFRAME_PRIMARY_RATE_HZ } from "../budget/viewBudget";
import { RENDER_STYLES } from "../photoreal/style";
import { type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import { DESCENT_SPAN_S, INSTRUMENT_CANVAS_PX, PERF_SCENES, STATION_CANVAS_PX } from "./scenes";

const SETTINGS_NAMES: ReadonlyArray<QualitySetting> = ["high", "low"];

/** Every scene with its ID, in the catalogue's order. */
const ALL = PERF_SCENE_IDS.map((id) => [id, PERF_SCENES[id]] as const);

describe("the perf scene catalogue", () => {
  it("has exactly the catalogue's scenes, each under its own ID", () => {
    expect(Object.keys(PERF_SCENES).toSorted()).toEqual([...PERF_SCENE_IDS].toSorted());
    for (const [id, scene] of ALL) {
      expect(scene.id).toBe(id);
      expect(scene.version).toBeGreaterThanOrEqual(1);
      expect(scene.harness).toBe(PERF_SCENE_HARNESS[id]);
      expect(scene.warmUp.scriptTimeS).toBe(0);
    }
  });

  it("judges every scene on a criterion at each setting, an informative one with its reason", () => {
    const criteria = ALL.flatMap(([, scene]) => SETTINGS_NAMES.map((s) => scene.criterion[s]));
    expect(criteria).toHaveLength(2 * PERF_SCENE_IDS.length);
    const frameColumns = criteria.flatMap((c) => (c.kind === "frame" ? [c.column] : []));
    expect(new Set(frameColumns)).toEqual(new Set(["1080p60", "720p30", "station-1080p60"]));
    const memory = criteria.flatMap((c) => (c.kind === "memory" ? [c] : []));
    expect(memory.every((c) => c.findingBytes <= c.ceilingBytes)).toBe(true);
    const reasons = criteria.flatMap((c) => (c.kind === "informative" ? [c.reason] : []));
    expect(reasons).toHaveLength(2);
    expect(reasons.every((reason) => reason.length > 0)).toBe(true);
    expect(PERF_SCENES["nuclear-disc-sky"].criterion.high.kind).toBe("informative");
    expect(PERF_SCENES["arrival-memory"].criterion).toEqual({
      high: { kind: "memory", ceilingBytes: 3e9, findingBytes: 2e9 },
      low: { kind: "memory", ceilingBytes: 1e9, findingBytes: 1e9 },
    });
  });

  it("paces the station at every vsync and the UHD 620's 720p30 scenes at every second", () => {
    expect(PERF_SCENES["station-wireframe-descent"].pacing).toEqual({
      high: "every-vsync",
      low: "every-vsync",
    });
    const others = ALL.filter(([id]) => id !== "station-wireframe-descent");
    expect(others.map(([, scene]) => scene.pacing)).toEqual(
      others.map(() => ({ high: "every-vsync", low: "every-second-vsync" })),
    );
    const at720p30 = ALL.filter(([, scene]) => {
      const low = scene.criterion.low;
      return low.kind === "frame" && low.column === "720p30";
    });
    expect(at720p30.map(([id]) => id)).toEqual(["cockpit-descent", "ringed-giant-approach"]);
    expect(at720p30.map(([, scene]) => scene.pacing.low)).toEqual([
      "every-second-vsync",
      "every-second-vsync",
    ]);
  });

  it("draws every view in a style that exists, in a role of the record's, at R07's rates", () => {
    for (const [, scene] of ALL) {
      for (const setting of SETTINGS_NAMES) {
        const views = scene.views[setting];
        expect(views.length).toBeGreaterThan(0);
        for (const view of views) {
          expect(RENDER_STYLES).toContain(view.style);
          expect(VIEW_ROLES).toContain(view.role);
          expect(view.lineAntialiasing).toBe(view.style === "wireframe" ? "analytic" : "none");
          const rate =
            view.role === "instrument"
              ? INSTRUMENT_RATE_HZ
              : view.style === "photorealistic"
                ? SETTINGS[setting].budget.photorealisticRateHz
                : WIREFRAME_PRIMARY_RATE_HZ;
          expect(view.rateHz).toBe(rate);
        }
      }
    }
  });

  it("draws the cockpit's photorealistic view at the window's size beside two instruments", () => {
    for (const setting of SETTINGS_NAMES) {
      const views = PERF_SCENES["cockpit-descent"].views[setting];
      expect(views.map(({ role, style, size }) => [role, style, size])).toEqual([
        ["primary", "photorealistic", { kind: "window" }],
        ["instrument", "wireframe", { kind: "canvas", ...INSTRUMENT_CANVAS_PX }],
        ["instrument", "wireframe", { kind: "canvas", ...INSTRUMENT_CANVAS_PX }],
      ]);
    }
    expect(INSTRUMENT_CANVAS_PX).toEqual({ widthPx: 240, heightPx: 180 });
    expect(PERF_SCENES["station-wireframe-descent"].views.low).toEqual([
      {
        role: "station",
        style: "wireframe",
        size: { kind: "canvas", ...STATION_CANVAS_PX },
        rateHz: 60,
        lineAntialiasing: "analytic",
      },
    ]);
  });

  it("spans R05's whole descent, the approach's 300 s, the sweep's 120 s and the arrival's hold", () => {
    expect(DESCENT_SPAN_S).toBe(1230);
    expect(PERF_SCENES["cockpit-descent"].span).toEqual({ kind: "script", seconds: 1230 });
    expect(PERF_SCENES["station-wireframe-descent"].span).toEqual({
      kind: "script",
      seconds: 1230,
    });
    expect(PERF_SCENES["ringed-giant-approach"].span).toEqual({ kind: "script", seconds: 300 });
    expect(PERF_SCENES["nuclear-disc-sky"].span).toEqual({ kind: "script", seconds: 120 });
    expect(PERF_SCENES["arrival-memory"].span).toEqual({ kind: "arrival", heldS: 60 });
  });

  it("restarts the server before every repetition and posts the field where a world has one", () => {
    for (const [id, scene] of ALL) {
      expect(scene.resets.server).toBe(true);
      expect(scene.resets.patchCache).toBe(true);
      expect(scene.resets.field).toBe(scene.body === PERF_PINS.world);
      expect(scene.world).toBe(scene.body ?? scene.system);
      expect(id.length).toBeGreaterThan(0);
    }
  });

  it("pins its universe, systems and bodies in the wire's forms", () => {
    expect(PERF_PINS.seed).toMatch(/^[0-9a-f]{16}$/);
    for (const system of [PERF_PINS.system, PERF_PINS.nuclearSystem]) {
      expect(system).toMatch(/^[0-9a-f]{16}$/);
    }
    for (const body of [PERF_PINS.world, PERF_PINS.giant]) {
      expect(body).toMatch(/^[0-9a-f]{16}\.[0-9a-f]{4}$/);
      expect(body.startsWith(PERF_PINS.system)).toBe(true);
    }
  });
});
