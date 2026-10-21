import type { BodyIdHex } from "@hyperion/protocol";
import { afterEach, describe, expect, it } from "vitest";

import { countingRenderEngine, type CountingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { readTokens } from "../../spatial/paint";
import { lookAlong } from "../camera/quaternion";
import { type RenderStyle, type ViewId, viewId } from "../camera/state";
import { IDENTITY_ROTATION } from "../coords/rotation";
import type { MemoryCategory } from "../engine/memory";
import type { QualitySetting } from "../quality/qualitySetting";
import { planetGeometry } from "../terrain/planet";
import type { PatchRequest } from "../terrain/select";
import type { TerrainViewReport } from "../terrain/session";
import { type TerrainFrameInput, TerrainPass, type TerrainPool } from "../terrain/terrainPass";
import type { BakedPatch } from "../terrain/workers/messages";
import { viewStrokesAt } from "../wireframe/drawList";
import { DescentProfile, landingSiteOf, trackStretches } from "./descentProfile";
import { contactRule, TEST_PLANET_FIGURE } from "./spikeScene";
import {
  type PreparedDescent,
  type SpikeFrameInput,
  SpikeRun,
  type SpikeTerrainSession,
  SPIKE_MAIN_VIEW_ID,
  SPIKE_VIEW_NAMES,
} from "./spikeRun";
import { SECOND_VIEW_ID, type SpikeSizesSession, SpikeSizes } from "./spikeSizes";
import { TEST_PLANET_WORLD } from "./spikeWorld";

const PROFILE = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n));

const PREPARED: PreparedDescent = {
  planet: planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off")),
  profile: PROFILE,
  contact: contactRule(PROFILE),
  siteHeightM: 0,
  stretches: trackStretches(PROFILE),
  stretchMaxHeightsM: trackStretches(PROFILE).map(() => 0),
  trackMaxHeightM: 0,
  omittedSigmaM: new Float64Array(25),
};

/** A pool that bakes nothing. */
class IdlePool implements TerrainPool {
  reprioritise(_demand: ReadonlyArray<PatchRequest>): void {}
  onBaked(_cb: (bake: BakedPatch) => void): () => void {
    return () => undefined;
  }
  terminate(): void {}
}

/** A frame 1,000 km above the test planet's north pole, looking down, `tiltRad` towards +x. */
function aboveThePole(nowMs: number, tiltRad: number): TerrainFrameInput {
  return {
    view: {
      rotation: IDENTITY_ROTATION,
      cameraM: { x: 0, y: 0, z: PREPARED.planet.figure.polarRadiusM + 1_000_000 },
      orientation: lookAlong(
        { x: Math.sin(tiltRad), y: 0, z: -Math.cos(tiltRad) },
        { x: Math.cos(tiltRad), y: 0, z: Math.sin(tiltRad) },
      ),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 640, heightPx: 360 },
    },
    grounded: [],
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    nowMs,
  };
}

/**
 * A terrain session stand-in that logs what the run asks of it, its passes real test-planet passes
 * stepped from above the pole, as the measured sizes read a session too.
 */
class SizedSession implements SpikeTerrainSession, SpikeSizesSession {
  readonly log: string[] = [];
  readonly reports: Array<readonly [ViewId, TerrainViewReport]> = [];
  readonly forgotten: ViewId[] = [];
  readonly wanted: ReadonlySet<BodyIdHex> = new Set();
  readonly classMaps = { classMapOf: () => null };
  readonly reliefs = { reliefM: () => null };
  readonly workerCount = 1;
  steps = 0;
  /** Whether the step steps the wireframe pass too, so that a test can tell the passes apart. */
  stepsWire = true;

  constructor(
    public lit: TerrainPass,
    public wire: TerrainPass,
  ) {}

  noteView(id: ViewId, report: TerrainViewReport): void {
    this.log.push(`note ${id}`);
    this.reports.push([id, report]);
  }

  forgetView(id: ViewId): void {
    this.forgotten.push(id);
  }

  step(): void {
    this.log.push("step");
    this.steps += 1;
    for (const pass of this.stepsWire ? [this.lit, this.wire] : [this.lit]) {
      pass.frame(aboveThePole(16 * this.steps, 0.1 * this.steps));
    }
  }

  failComputation(): void {}

  survey(): null {
    return null;
  }

  coverageMask(): null {
    return null;
  }

  drawTerrain(): null {
    return null;
  }

  drawWireTerrain(): null {
    return null;
  }

  pass(): TerrainPass {
    return this.lit;
  }

  wirePass(): TerrainPass {
    return this.wire;
  }

  memory(): {
    workerFieldBytes: number;
    keptFieldBytes: number;
    gpuBytes: Map<MemoryCategory, number>;
  } {
    return { workerFieldBytes: 0, keptFieldBytes: 0, gpuBytes: new Map() };
  }
}

function canvas(): HTMLCanvasElement {
  return document.createElement("canvas");
}

/** A test-planet pass on `engine`. */
function passOn(engine: CountingRenderEngine): TerrainPass {
  return new TerrainPass({
    engine,
    setting: "low",
    planet: PREPARED.planet,
    source: { kind: "test-planet", ridges: "off" },
    createPool: () => new IdlePool(),
  });
}

/** The run under `--sizes on` at `setting` in `style`, its views' draws logged with the session's calls. */
async function sizedRun(
  setting: QualitySetting,
  style: RenderStyle,
): Promise<{
  readonly run: SpikeRun;
  readonly engine: CountingRenderEngine;
  readonly session: SizedSession;
  readonly sizes: SpikeSizes;
}> {
  const engine = await countingRenderEngine();
  const session = new SizedSession(passOn(engine), passOn(engine));
  const make = engine.createView.bind(engine);
  engine.createView = (element, name) => {
    const view = make(element, name);
    const render = view.render.bind(view);
    view.render = (frame) => {
      session.log.push(`draw ${name}`);
      render(frame);
    };
    return view;
  };
  const sizes = new SpikeSizes({
    setting,
    style,
    mainViewId: SPIKE_MAIN_VIEW_ID,
    secondViewName: SPIKE_VIEW_NAMES.craft,
    nowMs: () => 0,
  });
  const run = new SpikeRun(
    engine,
    PREPARED,
    {
      setting,
      ridges: "off",
      createPool: () => new IdlePool(),
      body: {
        world: TEST_PLANET_WORLD,
        session,
        style,
        sizes: { measure: sizes, session },
      },
    },
    { main: canvas(), orbit: canvas(), craft: canvas() },
  );
  return { run, engine, session, sizes };
}

/** A frame's inputs with both stages laid out. */
function sizedInput(nowMs: number): SpikeFrameInput {
  return {
    nowMs,
    sizes: {
      main: { widthPx: 64, heightPx: 36 },
      orbit: { widthPx: 32, heightPx: 24 },
      craft: { widthPx: 32, heightPx: 18 },
    },
    tokens: readTokens(document.documentElement),
    remPx: 16,
    strokes: viewStrokesAt(1),
    selection: null,
    mainStage: { widthPx: 64, heightPx: 36, devicePixelRatio: 1, remPx: 16 },
    craftStage: { widthPx: 32, heightPx: 18, devicePixelRatio: 1, remPx: 16 },
  };
}

afterEach(() => {
  performance.clearMeasures();
});

describe("the spike's run with the measured sizes (R10.T14.b)", () => {
  it("notes the second view after the main view, steps once, and draws it after the main view in place of R02's craft instrument", async () => {
    const { run, session } = await sizedRun("low", "wireframe");
    run.frame(sizedInput(0));
    expect(session.log).toEqual([
      `note ${SPIKE_MAIN_VIEW_ID}`,
      `note ${SECOND_VIEW_ID}`,
      "step",
      "draw spike",
      "draw spike-craft",
      "draw spike-orbit",
    ]);
    const [id, report] = session.reports[1] ?? [];
    expect(id).toBe(viewId("instrument-1"));
    expect(report).toMatchObject({
      style: "wireframe",
      streamPriority: "secondary",
      readoutBody: null,
      presentedPx: { widthPx: 32, heightPx: 18 },
    });
    run.dispose();
  });

  it.each([
    ["high", "photorealistic", "photorealistic"],
    ["high", "wireframe", "wireframe"],
    ["low", "photorealistic", "wireframe"],
  ] as const)(
    "asks the second view on %s, the main view %s, in the %s style",
    async (setting, style, second) => {
      const { run, session } = await sizedRun(setting, style);
      run.frame(sizedInput(0));
      expect(session.reports.map(([view, report]) => [view, report.style])).toEqual([
        [SPIKE_MAIN_VIEW_ID, style],
        [SECOND_VIEW_ID, second],
      ]);
      run.dispose();
    },
  );

  it("names the craft's camera in the style its view drew in", async () => {
    const { run } = await sizedRun("low", "wireframe");
    expect(run.frame(sizedInput(0)).cameras.craft.style).toBe("wireframe");
    run.dispose();
  });

  it("counts both of the session's passes, and attaches again when the session's pass is another", async () => {
    const { run, engine, session, sizes } = await sizedRun("low", "photorealistic");
    for (let k = 0; k < 3; k += 1) {
      run.frame(sizedInput(1000 * k));
    }
    session.wire = passOn(engine);
    for (let k = 3; k < 6; k += 1) {
      run.frame(sizedInput(1000 * k));
    }
    const passes = sizes.report().passes;
    expect(passes.map((pass) => [pass.cache, pass.attachments, pass.samples > 0])).toEqual([
      ["lit", 1, true],
      ["wireframe", 2, true],
    ]);
    run.dispose();
  });

  it("marks the craft's first contact by the session's rule for the counters", async () => {
    const { run, sizes } = await sizedRun("low", "wireframe");
    run.frame(sizedInput(0));
    run.frame(sizedInput(16));
    expect(run.frame(sizedInput(1000 * PROFILE.durationS)).contact).toBe(true);
    run.frame(sizedInput(1000 * PROFILE.durationS + 16));
    expect(sizes.report().passes.map((pass) => pass.contact.sessionContact?.atS)).toEqual([
      PROFILE.durationS,
      PROFILE.durationS,
    ]);
    run.dispose();
  });

  it("marks touchdown for the counters at the first frame whose pose has reached the ground", async () => {
    const { run, sizes } = await sizedRun("low", "wireframe");
    run.frame(sizedInput(0));
    // In the hover, 2 m up and coming down; then at rest on the ground.
    const hoverS = (PROFILE.segmentSpans().at(-1)?.startS ?? 0) + 1;
    run.frame(sizedInput(1000 * hoverS));
    run.frame(sizedInput(1000 * (PROFILE.durationS - 10)));
    run.frame(sizedInput(1000 * PROFILE.durationS));
    expect(sizes.report().passes.map((pass) => pass.contact.touchdown?.atS)).toEqual([
      PROFILE.durationS - 10,
      PROFILE.durationS - 10,
    ]);
    run.dispose();
  });

  it("reads the pass of the main view's asked style while it draws another", async () => {
    const { run, session } = await sizedRun("low", "photorealistic");
    // Only the lit pass steps: a reading of the wireframe pass would hold no selection.
    session.stepsWire = false;
    const frame = run.frame(sizedInput(0));
    expect(frame.drawnStyle).toBe("wireframe");
    expect(frame.terrain.selected).toBe(session.lit.lastStep?.selection.patches.size);
    expect(frame.terrain.selected).toBeGreaterThan(0);
    run.dispose();
  });

  it("disposes the second view's drawer with the craft's view, and the session forgets both views", async () => {
    const { run, engine, session } = await sizedRun("low", "wireframe");
    run.frame(sizedInput(0));
    run.dispose();
    expect(engine.inner.views.map((view) => view.disposed)).toEqual([true, true, true]);
    expect(session.forgotten).toEqual([SPIKE_MAIN_VIEW_ID, SECOND_VIEW_ID]);
  });
});
