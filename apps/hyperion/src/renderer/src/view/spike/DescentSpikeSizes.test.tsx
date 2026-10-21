import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { instrumentViewId } from "../../displays/view/viewNames";
import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import type { RenderStyle, ViewId } from "../camera/state";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../engine/status";
import { requestAdapterOutcome } from "../engine/platform";
import type { QualitySetting } from "../quality/qualitySetting";
import { planetGeometry } from "../terrain/planet";
import type { TerrainPool } from "../terrain/terrainPass";
import type { MeasureDescent } from "./bodyDescent";
import { DescentProfile, landingSiteOf, trackStretches } from "./descentProfile";
import { DescentSpike } from "./DescentSpike";
import {
  type PreparedDescent,
  SPIKE_MAIN_VIEW_ID,
  SPIKE_VIEW_NAMES,
  type SpikeWorkers,
} from "./spikeRun";
import { contactRule } from "./spikeScene";
import { SpikeSizes } from "./spikeSizes";
import { spikeWorldNamed } from "./spikeWorld";
import type { SurfaceQueryWorker } from "./surfaceQuery";

/** `TERRAIN TEST`'s world, and a descent over it at the bare datum. */
const MOON = spikeWorldNamed("terrain-test");
const MOON_PROFILE = new DescentProfile(MOON.figure, landingSiteOf(7n));
const MOON_STRETCHES = trackStretches(MOON_PROFILE);

const MOON_DESCENT: PreparedDescent = {
  planet: planetGeometry(MOON.figure, null),
  profile: MOON_PROFILE,
  contact: contactRule(MOON_PROFILE),
  siteHeightM: 0,
  stretches: MOON_STRETCHES,
  stretchMaxHeightsM: MOON_STRETCHES.map(() => 0),
  trackMaxHeightM: 0,
  omittedSigmaM: new Float64Array(25).fill(Infinity),
  siteReading: null,
};

/** A query worker that never answers: the terrain world measures through its session. */
const SILENT_QUERY: SurfaceQueryWorker = {
  postMessage: () => undefined,
  addEventListener: () => undefined,
  terminate: () => undefined,
};

/** A pool that bakes nothing: the terrain world's passes are the session's. */
const IDLE_POOL: TerrainPool = {
  reprioritise: () => undefined,
  onBaked: () => () => undefined,
  terminate: () => undefined,
};

/** A ready counting engine on a SwiftShader adapter. */
const READY_ENGINE: ViewEngineSource = {
  requestAdapter: () =>
    requestAdapterOutcome(new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })])),
  load: () => countingRenderEngine(),
};

const STORE = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));

/** Lays every element out at 640 × 360, so that the canvases have a size to draw at. */
function stubLayout(): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 640, height: 360 }),
  );
}

/** Plays `steps` × 300 ms of frames, letting each step's work settle. */
async function play(steps: number): Promise<void> {
  for (let step = 0; step < steps; step += 1) {
    // Each step waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
  }
}

/** The spike over `TERRAIN TEST`'s moon with the measured sizes, and the views its session was told of. */
function renderSized(
  setting: QualitySetting,
  style: RenderStyle,
): { readonly sizes: SpikeSizes; readonly noted: ViewId[] } {
  const sizes = new SpikeSizes({
    setting,
    style,
    mainViewId: SPIKE_MAIN_VIEW_ID,
    secondViewName: SPIKE_VIEW_NAMES.craft,
  });
  const noted: ViewId[] = [];
  const measure: MeasureDescent = (session) => {
    const note = session.noteView.bind(session);
    vi.spyOn(session, "noteView").mockImplementation((id, report) => {
      noted.push(id);
      note(id, report);
    });
    return Promise.resolve(MOON_DESCENT);
  };
  // The browser's factory as the launch gives it: the spike puts its workers on the bake clock.
  const spikeWorkers: SpikeWorkers = {
    query: () => SILENT_QUERY,
    pool: () => () => IDLE_POOL,
    height: () => new ScriptedHeightWorker(),
  };
  render(
    <GraphicsStatusContext value={STORE}>
      <DescentSpike
        seed={7n}
        setting={setting}
        ridges="off"
        workers={1}
        world="terrain-test"
        style={style}
        engineSource={READY_ENGINE}
        spikeWorkers={spikeWorkers}
        measure={measure}
        sizes={sizes}
      />
    </GraphicsStatusContext>,
  );
  return { sizes, noted };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("DescentSpike with the measured sizes (R10.T14.b)", () => {
  it("names its craft instrument in the style its second view drew in", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    renderSized("low", "photorealistic");
    await play(4);
    expect(screen.getByRole("img", { name: /^VIEW, WIREFRAME, CRAFT, / })).toBeDefined();
  });

  it("notes its craft instrument with the session as a second view, beside the main view", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const { noted } = renderSized("low", "photorealistic");
    await play(4);
    expect(noted).toEqual(expect.arrayContaining([SPIKE_MAIN_VIEW_ID, instrumentViewId(1)]));
  });

  it("names its second view photorealistic on high once it draws so", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    renderSized("high", "photorealistic");
    await play(8);
    expect(screen.getByRole("img", { name: /^VIEW, PHOTOREALISTIC, CRAFT, / })).toBeDefined();
  });

  it("publishes VIEW's terrain readout at 4 Hz, and states none of its answers", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const { sizes } = renderSized("low", "wireframe");
    await play(6);
    expect([
      sizes.report().readouts.publishes >= 4,
      screen.queryByText("GROUND AT LINE OF SIGHT"),
      screen.queryByText("DATUM", { selector: "dt" }),
    ]).toEqual([true, null, null]);
  });

  it("times the height workers' messages at the session's boundary", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    const { sizes } = renderSized("low", "wireframe");
    await play(4);
    expect(sizes.report().bakes.workersMade).toBeGreaterThan(0);
  });

  it("makes its session once across its renders, the workers' wrapped factory stable", async () => {
    fakeFramesAndTimeouts();
    stubLayout();
    // One worker: a session made anew at a render would start another.
    const { sizes } = renderSized("low", "wireframe");
    await play(6);
    expect(sizes.report().bakes.workersMade).toBe(1);
  });
});
