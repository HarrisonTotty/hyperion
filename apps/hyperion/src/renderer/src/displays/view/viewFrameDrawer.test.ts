import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import { add, norm, normalise, scale, vec3 } from "../../geometry/vec3";
import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aViewCraft, NO_TURN } from "../../test/viewFixtures";
import { rotate } from "../../view/camera/quaternion";
import { EngineUnavailable } from "../../view/engine/resilientEngine";
import type { DrawItem, FrameSubmission, RenderView, TextureHandle } from "../../view/engine/types";
import { viewId } from "../../view/camera/state";
import { relativeToCamera } from "../../view/coords/relative";
import { IDENTITY_ROTATION, rotateToBodyFixed } from "../../view/coords/rotation";
import { DEFAULT_EXPOSURE } from "../../view/photometry/exposure";
import { PHOTOREAL_PASS_LABELS } from "../../view/photoreal/passes";
import { sceneOrigins, type ViewStar } from "../../view/scene/model";
import { ATMOSPHERE_STARS } from "../../view/scenes/atmosphereScene";
import { PHASE_GIANT, PHASE_PLANETS, phaseScene } from "../../view/scenes/phaseScene";
import { CoverageView, fieldCellIndex } from "../../view/terrain/coverage";
import { ANNUNCIATION_ONSET_MS } from "../../view/terrain/annunciation";
import { handoverTauPx, limbDistanceM, pixelAngleRad } from "../../view/terrain/handover";
import type {
  FrameTerrainPass,
  ViewTerrainSession,
  ViewTerrainSource,
} from "../../view/terrain/frameTerrain";
import { PoseIntervals } from "../../view/terrain/poseInterval";
import type { ResidentHeightsView } from "../../view/terrain/residentHeights";
import type { TerrainViewFrameInput } from "../../view/terrain/terrainPass";
import { heightAboveGroundM } from "../../view/terrain/terrainWire";
import {
  frameStyleOf,
  makeViewFrameDrawer,
  presentedViewport,
  skyFaceSizePx,
  terrainReportOf,
  ViewFrameDrawer,
  type ViewFrameDrawn,
  type ViewFrameInputs,
} from "./viewFrameDrawer";
import { runPose, startRun, type ViewRun } from "./viewRun";

/** A canvas view that records whether it was disposed. */
class RecordingView implements RenderView {
  readonly name: string;
  readonly frames: FrameSubmission[] = [];
  disposed = false;

  constructor(name: string) {
    this.name = name;
  }
  resize(): void {}
  render(frame: FrameSubmission): void {
    this.frames.push(frame);
  }
  readBack(): Promise<Uint8Array> {
    return Promise.resolve(new Uint8Array(0));
  }
  dispose(): void {
    this.disposed = true;
  }
}

/** Refuses the engine's materials, as `ResilientEngine` does while it has no device. */
function loseDevice(engine: Awaited<ReturnType<typeof countingRenderEngine>>): () => void {
  const spy = vi.spyOn(engine, "createMaterial").mockImplementation(() => {
    throw new EngineUnavailable("createMaterial");
  });
  return () => {
    spy.mockRestore();
  };
}

describe("a view's frame drawer, made during a device loss (R07.T19.c)", () => {
  it("is made after the restore, its handles once", async () => {
    const engine = await countingRenderEngine();
    // What one drawer makes, on a device.
    engine.resetCounts();
    makeViewFrameDrawer(engine, new RecordingView("measured"), "measured", () => undefined)();
    const once = { ...engine.counts };
    const regain = loseDevice(engine);
    const made: ViewFrameDrawer[] = [];
    const release = makeViewFrameDrawer(
      engine,
      new RecordingView("waiting"),
      "waiting",
      (drawer) => {
        made.push(drawer);
      },
    );
    regain();
    engine.resetCounts();
    engine.restore();
    await Promise.resolve();
    expect([made.length, engine.counts]).toEqual([1, once]);
    release();
  });

  it("is not made, and its view is disposed of, when released before the restore", async () => {
    const engine = await countingRenderEngine();
    const regain = loseDevice(engine);
    const view = new RecordingView("waiting");
    const made: ViewFrameDrawer[] = [];
    const release = makeViewFrameDrawer(engine, view, "waiting", (drawer) => {
      made.push(drawer);
    });
    regain();
    release();
    engine.restore();
    await Promise.resolve();
    expect([made.length, view.disposed]).toEqual([0, true]);
  });
});

/** Stand-in colours, one distinct colour per token, as `readTokens` would read them. */
const TOKENS = {
  text: "#e0e0e0",
  textMuted: "#909090",
  accent: "#40a0ff",
  target: "#ff40ff",
  line: "#404040",
  surface0: "#000000",
} as const;

/** The phase scene's run, its planets turning (their rotations the identity) where `rotated`. */
function phaseRun(rotated: boolean): ReturnType<typeof startRun> {
  const run = startRun(phaseScene());
  return rotated
    ? {
        ...run,
        scene: {
          ...run.scene,
          bodies: run.scene.bodies.map((body) =>
            body.kind === "star" ? body : { ...body, rotation: IDENTITY_ROTATION },
          ),
        },
      }
    : run;
}

/** The phase scene's frame drawn photorealistically, at 320 × 180, with the view's `sources`. */
async function drawPhotoreal(
  sources: Pick<ViewFrameInputs, "classMaps" | "reliefs" | "terrain">,
  rotated = false,
): Promise<void> {
  const engine = await countingRenderEngine();
  const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
  drawer.draw({
    run: phaseRun(rotated),
    size: { widthPx: 320, heightPx: 180, devicePixelRatio: 1, remPx: 16 },
    tokens: TOKENS,
    exposure: DEFAULT_EXPOSURE,
    stars: [],
    sky: null,
    selection: null,
    style: "photorealistic",
    setting: "low",
    renderScale: 1,
    availability: { wireframe: true, photorealistic: true },
    meter: null,
    ...sources,
  });
  drawer.dispose();
}

describe("a view's photorealistic frame (R10.T10.d, T10.e)", () => {
  const LIT = [...PHASE_PLANETS, PHASE_GIANT];

  it("shades its lit bodies with the view's class maps", async () => {
    const asked: string[] = [];
    await drawPhotoreal({
      classMaps: {
        classMapOf: (body) => {
          asked.push(body);
          return null;
        },
      },
    });
    expect(asked).toEqual(LIT);
  });

  it("hands its lit bodies with a rotation over by the view's reliefs", async () => {
    const asked: string[] = [];
    await drawPhotoreal(
      {
        reliefs: {
          reliefM: (body) => {
            asked.push(body);
            return null;
          },
        },
      },
      true,
    );
    expect(asked).toEqual(LIT);
  });

  it("draws its lit bodies over the view's own terrain of the session (R10.T16.c)", async () => {
    const asked: string[] = [];
    await drawPhotoreal({
      terrain: {
        source: {
          wanted: new Set(),
          survey: () => null,
          drawTerrain: () => null,
          coverageMask: () => null,
          wirePass: () => null,
          drawWireTerrain: () => null,
          pass: (body) => {
            asked.push(body);
            return null;
          },
          failComputation: () => undefined,
        },
        view: viewId("primary"),
        nowMs: 0,
      },
    });
    expect(asked).toEqual(LIT);
  });
});

describe("a view's hand-over at its frame's internal scale (R10.T10.g)", () => {
  /** The presented size: 1080 rows, which the low setting renders at 720. */
  const PRESENTED = { widthPx: 1920, heightPx: 1080 };

  /**
   * Each lit body of the turning phase scene its relief at `reliefTau` × the setting's τ at its limb,
   * in the presented centre pixels, where its disc spans 12 presented px or more, and none for a
   * smaller one, which a frame's internal scale could make a point.
   */
  function reliefsOf(
    run: ViewRun,
    setting: "high" | "low",
    reliefTau: number,
  ): NonNullable<ViewFrameInputs["reliefs"]> {
    const pose = runPose(run);
    const origins = sceneOrigins(run.scene);
    const radPerPx = pixelAngleRad(
      { orientation: pose.orientation, fovXRad: (run.camera.fovDeg * Math.PI) / 180 },
      PRESENTED,
    );
    return {
      reliefM: (id) => {
        const body = run.scene.bodies.find((each) => each.id === id);
        if (body === undefined) {
          return null;
        }
        const distanceM = norm(
          relativeToCamera({ kind: "body", body: id, m: vec3(0, 0, 0) }, pose, origins),
        );
        if ((2 * Math.asin(body.radiusM / distanceM)) / radPerPx < 12) {
          return null;
        }
        const limbM = limbDistanceM(distanceM, body.radiusM);
        return reliefTau * handoverTauPx("photorealistic", setting) * limbM * radPerPx;
      },
    };
  }

  /** The bodies a drawer's photorealistic frame of the phase scene hands over. */
  async function handedOver(
    setting: "high" | "low",
    renderScale: number,
    reliefTau: number,
  ): Promise<string[]> {
    const run = phaseRun(true);
    const inputs: ViewFrameInputs = {
      run,
      size: { ...PRESENTED, devicePixelRatio: 1, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: [],
      sky: null,
      selection: null,
      style: "photorealistic",
      setting,
      renderScale,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
      reliefs: reliefsOf(run, setting, reliefTau),
    };
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    // The first frame draws the wireframe while its pipelines compile, and plans no regimes.
    drawer.draw(inputs);
    await vi.waitFor(() => {
      expect(drawer.photorealStatus).toBe("ready");
    });
    drawer.draw(inputs);
    const handed = [...drawer.regimes].flatMap(([id, regime]) =>
      regime === "terrain" ? [id] : [],
    );
    drawer.dispose();
    return handed;
  }

  it.each(["high", "low"] as const)(
    "hands the same bodies over at an internal scale of 0.5 as at 1 on %s, a relief of 1.5 τ",
    async (setting) => {
      const whole = await handedOver(setting, 1, 1.5);
      expect(whole.length).toBeGreaterThan(0);
      expect(await handedOver(setting, 0.5, 1.5)).toEqual(whole);
    },
  );

  it("hands none over at either scale on either setting at a relief of 0.5 τ", async () => {
    const handed = await Promise.all(
      (["high", "low"] as const).flatMap((setting) =>
        [1, 0.5].map((renderScale) => handedOver(setting, renderScale, 0.5)),
      ),
    );
    expect(handed.flat()).toEqual([]);
  });
});

describe("what a view drew, for the terrain session (R10.T16.a)", () => {
  const LIT = [...PHASE_PLANETS, PHASE_GIANT];

  /** The phase scene's frame inputs at 320 × 180 CSS px and a ratio of `ratio`, in `style`. */
  function inputsOf(style: "photorealistic" | "wireframe", ratio = 1): ViewFrameInputs {
    return {
      run: startRun(phaseScene()),
      size: { widthPx: 320, heightPx: 180, devicePixelRatio: ratio, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: [],
      sky: null,
      selection: null,
      style,
      setting: "low",
      renderScale: 1,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
    };
  }

  /** A drawer that has drawn the phase scene's photorealistic frame, its pipelines made. */
  async function plannedDrawer(): Promise<ViewFrameDrawer> {
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    // The first frame draws the wireframe while its pipelines compile, and plans no regimes.
    drawer.draw(inputsOf("photorealistic"));
    await vi.waitFor(() => {
      expect(drawer.photorealStatus).toBe("ready");
    });
    drawer.draw(inputsOf("photorealistic"));
    return drawer;
  }

  it("plans no regimes before a photorealistic frame", async () => {
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const regimes = drawer.regimes.size;
    drawer.dispose();
    expect(regimes).toBe(0);
  });

  it("keeps its last photorealistic plan's regimes", async () => {
    const drawer = await plannedDrawer();
    const regimes = [...drawer.regimes.keys()];
    drawer.dispose();
    expect(regimes).toEqual(LIT);
  });

  it("keeps them through a frame drawn in the wireframe", async () => {
    const drawer = await plannedDrawer();
    const planned = drawer.regimes;
    drawer.draw(inputsOf("wireframe"));
    const kept = drawer.regimes;
    drawer.dispose();
    expect([planned.size > 0, kept === planned]).toEqual([true, true]);
  });

  it("reports its scene, pose, field of view, presented size, styles, setting and priority", () => {
    const inputs = inputsOf("wireframe", 1.5);
    const regimes = new Map();
    expect(terrainReportOf(inputs, regimes, "secondary", "photorealistic")).toEqual({
      scene: inputs.run.scene,
      pose: runPose(inputs.run),
      fovXRad: (inputs.run.camera.fovDeg * Math.PI) / 180,
      presentedPx: { widthPx: 480, heightPx: 270 },
      style: "wireframe",
      drawnStyle: "photorealistic",
      setting: "low",
      streamPriority: "secondary",
      regimes,
    });
  });

  it("reports nothing while its stage is not laid out", () => {
    expect(
      terrainReportOf({ ...inputsOf("wireframe"), size: null }, new Map(), "primary", "wireframe"),
    ).toBe(null);
  });

  it("presents at its stage's size times its ratio, rounded, at least a pixel", () => {
    expect([
      presentedViewport({ widthPx: 320.4, heightPx: 180, devicePixelRatio: 1.25, remPx: 16 }),
      presentedViewport({ widthPx: 0.2, heightPx: 0.1, devicePixelRatio: 1, remPx: 16 }),
    ]).toEqual([
      { widthPx: 401, heightPx: 225 },
      { widthPx: 1, heightPx: 1 },
    ]);
  });
});

describe("the sky cube's face size (R07.T17)", () => {
  it.each([
    { setting: "high", blendable: true, faceSizePx: 3_072 },
    { setting: "low", blendable: true, faceSizePx: 1_024 },
    { setting: "high", blendable: false, faceSizePx: 1_024 },
    { setting: "low", blendable: false, faceSizePx: 1_024 },
  ] as const)(
    "is $faceSizePx² on the $setting setting where float32-blendable is $blendable",
    ({ setting, blendable, faceSizePx }) => {
      expect(skyFaceSizePx(setting, blendable)).toBe(faceSizePx);
    },
  );
});

describe("the style a view's frame is drawn in, foreseen (the batch review's S8)", () => {
  it.each([
    ["photorealistic", true, "ready", "photorealistic"],
    ["photorealistic", true, "making", "wireframe"],
    ["photorealistic", true, "failed", "wireframe"],
    ["photorealistic", false, "ready", "wireframe"],
    ["wireframe", true, "ready", "wireframe"],
  ] as const)(
    "is %s asked, available %s, pipelines %s: %s",
    (style, photorealistic, status, drawn) => {
      expect(
        frameStyleOf({ style, availability: { wireframe: true, photorealistic } }, status),
      ).toBe(drawn);
    },
  );
});

/**
 * A session stand-in whose survey throws `error` while `throws` holds, the faults its drawer's
 * guard raised counted.
 */
function failingSource(
  error: Error,
  throws: () => boolean = () => true,
): ViewTerrainSession & { readonly faulted: () => number } {
  let faulted = 0;
  return {
    wanted: new Set(PHASE_PLANETS),
    survey: () => {
      if (throws()) {
        throw error;
      }
      return null;
    },
    drawTerrain: () => null,
    coverageMask: () => null,
    pass: () => null,
    wirePass: () => null,
    drawWireTerrain: () => null,
    failComputation: () => {
      faulted += 1;
    },
    faulted: () => faulted,
  };
}

describe("a view's survey edges that cannot be marked (the batch review's S5; R10.T16.e)", () => {
  /** The phase scene's wireframe frame over `source` at `nowMs`. */
  function inputsAt(source: ViewTerrainSession, nowMs: number): ViewFrameInputs {
    return {
      run: phaseRun(true),
      size: { widthPx: 320, heightPx: 180, devicePixelRatio: 1, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: [],
      sky: null,
      selection: null,
      style: "wireframe",
      setting: "low",
      renderScale: 1,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
      terrain: { source, view: viewId("view"), nowMs },
    };
  }

  /** `drawer`'s frames over `source` at each of `timesMs`: whether each drew. */
  function drawAt(
    drawer: ViewFrameDrawer,
    source: ViewTerrainSession,
    timesMs: ReadonlyArray<number>,
  ): boolean[] {
    return timesMs.map((nowMs) => drawer.draw(inputsAt(source, nowMs)) !== null);
  }

  it("draws on without them, saying so once", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const drawn = drawAt(drawer, failingSource(new Error("a bug of the source's")), [0, 16]);
    drawer.dispose();
    expect([drawn, error.mock.calls.length]).toEqual([[true, true], 1]);
  });

  it("faults the session's terrain once they have failed for 250 ms, and not before", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const source = failingSource(new Error("a bug of the source's"));
    drawAt(drawer, source, [1_000, 1_100, 1_000 + ANNUNCIATION_ONSET_MS - 1]);
    const before = source.faulted();
    drawAt(drawer, source, [1_000 + ANNUNCIATION_ONSET_MS, 1_300]);
    drawer.dispose();
    expect([before, source.faulted()]).toEqual([0, 1]);
  });

  it("raises nothing for a single failed frame", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    let nowMs = 0;
    const source = failingSource(new Error("a bug of the source's"), () => nowMs === 0);
    for (nowMs = 0; nowMs <= 1_000; nowMs += 100) {
      drawer.draw(inputsAt(source, nowMs));
    }
    drawer.dispose();
    expect(source.faulted()).toBe(0);
  });

  it("lets a loss pass in silence, however long it lasts", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const source = failingSource(new EngineUnavailable("createBuffer"));
    drawAt(drawer, source, [0, 300, 600]);
    drawer.dispose();
    expect([source.faulted(), error.mock.calls.length]).toEqual([0, 0]);
  });

  it("counts a new session's failures afresh", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const first = failingSource(new Error("a bug of the source's"));
    const second = failingSource(new Error("a bug of the source's"));
    drawAt(drawer, first, [0, 200]);
    drawAt(drawer, second, [240, 300]);
    drawer.dispose();
    expect([first.faulted(), second.faulted()]).toEqual([0, 0]);
  });
});

/** A level-5 coverage of one cell of face 0, so that a body has an edge to mark. */
function oneCell(): CoverageView {
  const start = fieldCellIndex({ face: 0, level: 5, i: 16, j: 16 });
  const merged = CoverageView.empty(5).merged(
    { ranges: [{ start, end: start + 1, resolution: 8 }] },
    1,
  );
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

describe("what a view's frame gives its label block and list (the batch review's S16)", () => {
  // The planet at 90°, which the camera looks at.
  const SURVEYED = PHASE_PLANETS[1];

  /**
   * A terrain session stand-in: the 90° planet's field wanted and surveyed in part, and each
   * draw of its terrain annunciating `TERRAIN: STREAMING`.
   */
  function streamingSource(): ViewTerrainSession {
    return {
      wanted: new Set([SURVEYED]),
      survey: (body) =>
        body === SURVEYED ? { coverage: oneCell(), header: moonLikeHeader(SURVEYED) } : null,
      drawTerrain: () => ({
        draw: null,
        conditions: { streaming: true, detailLimited: false },
        annunciation: "TERRAIN: STREAMING",
        fault: null,
        missing: true,
        standingIn: false,
        maskReady: false,
        cascades: null,
      }),
      coverageMask: () => null,
      wirePass: () => null,
      drawWireTerrain: () => null,
      pass: () => null,
      failComputation: () => undefined,
    };
  }

  /** The phase scene's frame inputs over {@link streamingSource}, its planets turning. */
  function inputsIn(style: "photorealistic" | "wireframe"): ViewFrameInputs {
    return {
      run: phaseRun(true),
      size: { widthPx: 320, heightPx: 180, devicePixelRatio: 1, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: [],
      sky: null,
      selection: null,
      style,
      setting: "low",
      renderScale: 1,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
      // Relief enough that every planet with a rotation is handed over.
      reliefs: { reliefM: () => 1e9 },
      terrain: { source: streamingSource(), view: viewId("view"), nowMs: 0 },
    };
  }

  /** A frame drawn in `style` by a drawer whose photorealistic pipelines are made. */
  async function drawnIn(style: "photorealistic" | "wireframe"): Promise<ViewFrameDrawn | null> {
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    // The first photorealistic frame draws the wireframe while its pipelines compile.
    drawer.draw(inputsIn("photorealistic"));
    await vi.waitFor(() => {
      expect(drawer.photorealStatus).toBe("ready");
    });
    const drawn = drawer.draw(inputsIn(style));
    drawer.dispose();
    return drawn;
  }

  it("gives the terrain's annunciation of a photorealistic frame", async () => {
    expect((await drawnIn("photorealistic"))?.terrain).toBe("TERRAIN: STREAMING");
  });

  it("gives no terrain annunciation of a wireframe frame whose source has no wireframe pass", async () => {
    expect((await drawnIn("wireframe"))?.terrain).toBeNull();
  });

  it.each(["photorealistic", "wireframe"] as const)(
    "gives the session's survey edges in the %s style",
    async (style) => {
      expect((await drawnIn(style))?.surveyEdges.map((edge) => edge.body)).toEqual([SURVEYED]);
    },
  );
});

/** The camera's height above `body`'s datum in `run`, as its header's figure gives it, m. */
function datumHeightM(run: ViewRun, body: BodyIdHex): number {
  const centreM = relativeToCamera(
    { kind: "body", body, m: vec3(0, 0, 0) },
    runPose(run),
    sceneOrigins(run.scene),
  );
  const header = moonLikeHeader(body);
  return heightAboveGroundM(
    rotateToBodyFixed(IDENTITY_ROTATION, scale(centreM, -1)),
    { equatorialRadiusM: header.equatorial_radius_m, polarRadiusM: header.polar_radius_m },
    () => null,
  );
}

describe("a view's wireframe frame of its terrain (R10.T11)", () => {
  // The planet at 90°, which the camera looks at.
  const HANDED = PHASE_PLANETS[1];
  const VIEW = viewId("view");
  const MASK: TextureHandle = { kind: "texture", name: "coverage mask" };
  /** The wireframe pass's draw, as the session's would name it. */
  const WIRE_DRAW: DrawItem = {
    mesh: { kind: "mesh", name: "terrain patches" },
    material: { kind: "material", name: "terrain-wire-face-differences" },
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: {},
  };

  /** No resident ground: the datum everywhere. */
  const DATUM: ResidentHeightsView = {
    rimHeightAtM: () => null,
    groundHeightAtM: () => null,
    ceilingM: 0,
  };

  /**
   * A terrain session stand-in whose wireframe pass of the 90° planet took the view, over `heights`,
   * each of its draws annunciating `TERRAIN: STREAMING`, the frame inputs it was given in `asked`;
   * no field is wanted, so that no survey edge reads it.
   */
  function wireSource(
    heights: ResidentHeightsView = DATUM,
    overrides: Partial<ViewTerrainSource> = {},
  ): { readonly source: ViewTerrainSession; readonly asked: TerrainViewFrameInput[] } {
    const asked: TerrainViewFrameInput[] = [];
    return {
      asked,
      source: {
        wanted: new Set(),
        survey: (body) =>
          body === HANDED ? { coverage: null, header: moonLikeHeader(HANDED) } : null,
        drawTerrain: () => null,
        coverageMask: (body) => (body === HANDED ? MASK : null),
        pass: () => null,
        wirePass: (body) =>
          body === HANDED
            ? {
                maskReadyFor: () => true,
                symbologyDepthMaterial: null,
                heights,
                viewIds: new Set([VIEW]),
                faulted: false,
              }
            : null,
        drawWireTerrain: (_view, body, input) => {
          if (body !== HANDED) {
            return null;
          }
          asked.push(input);
          return {
            draw: WIRE_DRAW,
            conditions: { streaming: true, detailLimited: false },
            annunciation: "TERRAIN: STREAMING",
            fault: null,
            missing: false,
            standingIn: false,
            maskReady: true,
            cascades: null,
          };
        },
        // Never faulted here: the drawer's guards are R10.T16.e's own tests'.
        failComputation: () => undefined,
        ...overrides,
      },
    };
  }

  /** The phase scene's frame inputs over `source` in `style`, its planets turning. */
  function inputsOver(
    source: ViewTerrainSession,
    style: "photorealistic" | "wireframe" = "wireframe",
  ): ViewFrameInputs {
    return {
      run: phaseRun(true),
      size: { widthPx: 320, heightPx: 180, devicePixelRatio: 1, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: [],
      sky: null,
      selection: null,
      style,
      setting: "low",
      renderScale: 1,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
      terrain: { source, view: VIEW, nowMs: 0 },
    };
  }

  /** A drawer's wireframe frame over `source`, with what its view was given. */
  async function wireframeOf(source: ViewTerrainSession): Promise<{
    readonly drawn: ViewFrameDrawn | null;
    readonly frame: FrameSubmission | undefined;
  }> {
    const engine = await countingRenderEngine();
    const view = new RecordingView("view");
    const drawer = new ViewFrameDrawer(engine, view, "view");
    const drawn = drawer.draw(inputsOver(source));
    drawer.dispose();
    return { drawn, frame: view.frames.at(-1) };
  }

  it("draws the wireframe pass's terrain after the occluders and before the lines", async () => {
    const { frame } = await wireframeOf(wireSource().source);
    const names = frame?.draws.map((draw) => draw.material.name) ?? [];
    const at = names.indexOf(WIRE_DRAW.material.name);
    const occluders = names.flatMap((name, n) =>
      name.startsWith("wireframe:occluder") ? [n] : [],
    );
    const lines = names.flatMap((name, n) => (name.startsWith("wireframe:lines") ? [n] : []));
    expect([
      occluders.length > 0 && lines.length > 0,
      occluders.every((n) => n < at),
      lines.every((n) => n > at),
    ]).toEqual([true, true, true]);
  });

  it("binds the body's coverage mask to its datum's draws, which give way to its terrain", async () => {
    const { frame } = await wireframeOf(wireSource().source);
    expect(frame?.draws.some((draw) => draw.textures["coverageMask"] === MASK)).toBe(true);
  });

  it("gives the datum way over the image too, the symbology binding the mask its plan handed the body over with (R10.T16.f)", async () => {
    // The lit pass of the 90° planet, which gives its mask, beside the wireframe's.
    const pass: FrameTerrainPass = {
      maskReadyFor: () => true,
      symbologyDepthMaterial: null,
      heights: DATUM,
      viewIds: new Set([VIEW]),
      faulted: false,
    };
    const { source } = wireSource(DATUM, { pass: (body) => (body === HANDED ? pass : null) });
    // Relief enough that every planet with a rotation is handed over.
    const inputs = { ...inputsOver(source, "photorealistic"), reliefs: { reliefM: () => 1e9 } };
    const engine = await countingRenderEngine();
    const view = new RecordingView("view");
    const drawer = new ViewFrameDrawer(engine, view, "view");
    // The first photorealistic frame draws the wireframe while its pipelines compile.
    drawer.draw(inputs);
    await vi.waitFor(() => {
      expect(drawer.photorealStatus).toBe("ready");
    });
    const drawn = drawer.draw(inputs);
    drawer.dispose();
    const symbology = view.frames.findLast(
      (frame) => frame.label === PHOTOREAL_PASS_LABELS.symbology,
    );
    const surveyed = (symbology?.draws ?? []).filter((draw) =>
      draw.material.name.endsWith("Surveyed"),
    );
    expect([
      drawn?.drawnStyle,
      [...new Set(surveyed.map((draw) => draw.material.name))].toSorted(),
      surveyed.every((draw) => draw.textures["coverageMask"] === MASK),
    ]).toEqual([
      "photorealistic",
      ["wireframe:linesSurveyed", "wireframe:occluderSphereSurveyed"],
      true,
    ]);
  });

  it("gives the contour interval its terrain was drawn at, and the terrain's annunciation", async () => {
    const { source, asked } = wireSource();
    const { drawn } = await wireframeOf(source);
    const intervalM = asked[0]?.wire?.intervalM;
    expect([
      intervalM !== undefined && intervalM > 0,
      drawn?.contourIntervalM,
      drawn?.terrain,
    ]).toEqual([true, intervalM, "TERRAIN: STREAMING"]);
  });

  it("gives no contour interval of a photorealistic frame, which draws no wireframe terrain", async () => {
    const { source, asked } = wireSource();
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    // The first photorealistic frame draws the wireframe while its pipelines compile.
    drawer.draw(inputsOver(source, "photorealistic"));
    await vi.waitFor(() => {
      expect(drawer.photorealStatus).toBe("ready");
    });
    const before = asked.length;
    const drawn = drawer.draw(inputsOver(source, "photorealistic"));
    drawer.dispose();
    expect([drawn?.drawnStyle, drawn?.contourIntervalM, asked.length - before]).toEqual([
      "photorealistic",
      null,
      0,
    ]);
  });

  it("holds the contour interval from frame to frame while the height lies between its thresholds", async () => {
    // The ground under the camera raised so that it stands 4 km over it, then 7 km: a twentieth of
    // 7 km is 350 m, past the midpoint 316 m to 500 m but short of its 1.2 × 316 m.
    const datumM = datumHeightM(phaseRun(true), HANDED);
    const ground = { aboveM: 4_000 };
    const heights: ResidentHeightsView = {
      ...DATUM,
      groundHeightAtM: () => datumM - ground.aboveM,
    };
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const fresh = new ViewFrameDrawer(engine, new RecordingView("fresh"), "fresh");
    const { source } = wireSource(heights);
    const at4km = drawer.draw(inputsOver(source))?.contourIntervalM;
    ground.aboveM = 7_000;
    const at7km = drawer.draw(inputsOver(source))?.contourIntervalM;
    const freshAt7km = fresh.draw(inputsOver(source))?.contourIntervalM;
    drawer.dispose();
    fresh.dispose();
    expect([at4km, at7km, freshAt7km]).toEqual([200, 200, 500]);
  });

  describe("through the stage's interval holder (R10.T12.b)", () => {
    const OTHER = viewId("other");

    /**
     * The primary's drawer and another's at one camera, over a wireframe pass that took both, the
     * ground under them first 4 km below and then 7 km: the primary holds 200 m where a view from
     * no interval states 500 m.
     */
    async function twoViews(): Promise<{
      readonly primary: ViewFrameDrawer;
      readonly other: ViewFrameDrawer;
      readonly inputs: (
        view: typeof OTHER,
        style: "photorealistic" | "wireframe",
        nowMs: number,
      ) => ViewFrameInputs;
      readonly toSevenKm: () => void;
    }> {
      const datumM = datumHeightM(phaseRun(true), HANDED);
      const ground = { aboveM: 4_000 };
      const heights: ResidentHeightsView = {
        ...DATUM,
        groundHeightAtM: () => datumM - ground.aboveM,
      };
      const { source } = wireSource(heights, {
        wirePass: (body) =>
          body === HANDED
            ? {
                maskReadyFor: () => true,
                symbologyDepthMaterial: null,
                heights,
                viewIds: new Set([VIEW, OTHER]),
                faulted: false,
              }
            : null,
      });
      const intervals = new PoseIntervals();
      const engine = await countingRenderEngine();
      return {
        primary: new ViewFrameDrawer(engine, new RecordingView("view"), "view"),
        other: new ViewFrameDrawer(engine, new RecordingView("other"), "other"),
        inputs: (view, style, nowMs) => ({
          ...inputsOver(source, style),
          terrain: { source, view, nowMs, intervals },
        }),
        toSevenKm: () => {
          ground.aboveM = 7_000;
        },
      };
    }

    it("states the interval another view holds at its camera's point", async () => {
      const { primary, other, inputs, toSevenKm } = await twoViews();
      primary.draw(inputs(VIEW, "wireframe", 0));
      toSevenKm();
      primary.draw(inputs(VIEW, "wireframe", 16));
      const otherM = other.draw(inputs(OTHER, "wireframe", 16))?.contourIntervalM;
      primary.dispose();
      other.dispose();
      expect(otherM).toBe(200);
    });

    it("forgets the view in the holder on a photorealistic frame, so that a view at its point keeps its own", async () => {
      const { primary, other, inputs, toSevenKm } = await twoViews();
      primary.draw(inputs(VIEW, "wireframe", 0));
      toSevenKm();
      primary.draw(inputs(VIEW, "wireframe", 16));
      // The first photorealistic frame draws the wireframe while its pipelines compile.
      primary.draw(inputs(VIEW, "photorealistic", 32));
      await vi.waitFor(() => {
        expect(primary.photorealStatus).toBe("ready");
      });
      const drawn = primary.draw(inputs(VIEW, "photorealistic", 48));
      const otherM = other.draw(inputs(OTHER, "wireframe", 64))?.contourIntervalM;
      primary.dispose();
      other.dispose();
      expect([drawn?.drawnStyle, otherM]).toEqual(["photorealistic", 500]);
    });
  });

  it.each([
    ["reports a bug of its source's once", new Error("a bug of the source's"), 1],
    ["passes a loss in silence", new EngineUnavailable("createMaterial"), 0],
  ] as const)(
    "draws on without the wireframe terrain when it cannot be drawn, and %s",
    async (_case, failure, reports) => {
      const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
      const { source } = wireSource(DATUM, {
        wirePass: () => {
          throw failure;
        },
      });
      const engine = await countingRenderEngine();
      const view = new RecordingView("view");
      const drawer = new ViewFrameDrawer(engine, view, "view");
      const drawn = [drawer.draw(inputsOver(source)), drawer.draw(inputsOver(source))];
      drawer.dispose();
      expect([
        drawn.map((each) => each?.contourIntervalM),
        view.frames.length,
        error.mock.calls.length,
      ]).toEqual([[null, null], 2, reports]);
    },
  );
});

describe("a view's line-of-sight mark across its frames (R10.T12)", () => {
  /**
   * The phase scene's run with another craft 10⁸ m out, below the view's centre by `belowPx` at
   * 320 × 180: the camera looks along its −z, its screen's down being its −y.
   */
  function runWithCraftBelow(belowPx: number): ViewRun {
    const run = phaseRun(false);
    const pose = runPose(run);
    if (pose.frame.kind !== "system") {
      throw new Error("the phase scene's camera is not in its system's frame");
    }
    const s = 1 / Math.tan((run.camera.fovDeg * Math.PI) / 360);
    const direction = rotate(pose.orientation, normalise(vec3(0, -belowPx / (s * 160), -1)));
    const craft = aViewCraft({
      id: "other",
      designation: "OTHER",
      pose: {
        position: {
          kind: "system",
          system: pose.frame.system,
          m: add(pose.positionM, scale(direction, 1e8)),
        },
        attitude: NO_TURN,
      },
    });
    return { ...run, scene: { ...run.scene, craft: [...run.scene.craft, craft] } };
  }

  /**
   * The craft below the centre with its upper tick `clearRem` below the end of the tripod's lower
   * tick, at a ratio of 1 and 16 px a rem: the tripod's ticks from 3.25 to 7.25 px out (0.1875 rem
   * and δ's 0.25 px; decision-r10-t12-commit3 §3), the craft's upper tick ending 6.25 + 4 px above
   * its anchor.
   */
  function craftClear(clearRem: number): ViewRun {
    return runWithCraftBelow(0.1875 * 16 + 0.25 + 4 + 10.25 + clearRem * 16);
  }

  /** A wireframe frame's inputs over `run`, at 320 × 180, its readout's point as given. */
  function inputsOver(run: ViewRun, lineOfSightPoint?: boolean): ViewFrameInputs {
    return {
      run,
      size: { widthPx: 320, heightPx: 180, devicePixelRatio: 1, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: [],
      sky: null,
      selection: null,
      style: "wireframe",
      setting: "low",
      renderScale: 1,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
      ...(lineOfSightPoint === undefined
        ? {}
        : { lineOfSightPoint: { hasPoint: lineOfSightPoint, rangeM: null } }),
    };
  }

  /** Each frame's line-of-sight mark, drawn in turn by one drawer. */
  async function framesOf(
    ...inputs: ReadonlyArray<ViewFrameInputs>
  ): Promise<Array<ViewFrameDrawn["lineOfSight"] | undefined>> {
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const drawn = inputs.map((each) => drawer.draw(each)?.lineOfSight);
    drawer.dispose();
    return drawn;
  }

  it("marks no line of sight for a view that states no readout", async () => {
    expect(await framesOf(inputsOver(phaseRun(false)))).toEqual([null]);
  });

  it("draws the tripod at the view's centre where the readout has a point", async () => {
    const [mark] = await framesOf(inputsOver(phaseRun(false), true));
    const box = mark?.boxPx ?? null;
    expect({
      yielded: mark?.yielded,
      holdsCentre:
        box !== null &&
        box.leftPx < 160 &&
        box.leftPx + box.widthPx > 160 &&
        box.topPx < 90 &&
        box.topPx + box.heightPx > 90,
    }).toEqual({ yielded: false, holdsCentre: true });
  });

  it("keeps its give-way from frame to frame, so that a craft between 0.5 and 0.75 rem holds it out", async () => {
    const [near, between, beyond] = await framesOf(
      inputsOver(craftClear(0.25), true),
      inputsOver(craftClear(0.6), true),
      inputsOver(craftClear(0.9), true),
    );
    // A drawer whose first frame has the craft between draws the tripod.
    const [fresh] = await framesOf(inputsOver(craftClear(0.6), true));
    expect({
      near: near?.boxPx ?? null,
      between: between?.boxPx ?? null,
      beyond: beyond?.boxPx !== null && beyond?.yielded === false,
      fresh: fresh?.boxPx !== null && fresh?.yielded === false,
    }).toEqual({ near: null, between: null, beyond: true, fresh: true });
  });
});

describe("a kept scene's own stars, offline (R08.T10.b)", () => {
  /** One star of an interim field, dead ahead of the phase scene's camera. */
  const INTERIM_STAR: ViewStar = {
    id: "0000000000000001",
    direction: vec3(0, 0, -1),
    distanceM: 3e17,
    absoluteV: 0,
    tEffK: 5_800,
  };

  /**
   * The star sprites a wireframe frame of the phase scene draws, the scene carrying `own` stars and
   * the view given the interim field `interim`.
   */
  async function spritesDrawn(
    own: ReadonlyArray<ViewStar>,
    interim: ReadonlyArray<ViewStar>,
  ): Promise<number> {
    return (await framesStars(own, interim)).sprites;
  }

  /** The star sprites a frame drew, as {@link spritesDrawn}, and whether it says they were the scene's. */
  async function framesStars(
    own: ReadonlyArray<ViewStar>,
    interim: ReadonlyArray<ViewStar>,
  ): Promise<{ readonly sprites: number; readonly sceneStars: boolean | undefined }> {
    const engine = await countingRenderEngine();
    const view = new RecordingView("view");
    const drawer = new ViewFrameDrawer(engine, view, "view");
    const run = phaseRun(false);
    const drawn = drawer.draw({
      run: { ...run, scene: { ...run.scene, stars: own } },
      size: { widthPx: 320, heightPx: 180, devicePixelRatio: 1, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: interim,
      sky: null,
      selection: null,
      style: "wireframe",
      setting: "high",
      renderScale: 1,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
    });
    drawer.dispose();
    const draws = view.frames.at(-1)?.draws ?? [];
    return {
      sprites:
        draws.find((draw) => draw.material.name === "wireframe:starSprite")?.instanceCount ?? 0,
      sceneStars: drawn?.sceneStars,
    };
  }

  it("says which stars it drew: the scene's own field where it stands in, else none of its own", async () => {
    expect([
      (await framesStars(ATMOSPHERE_STARS, [])).sceneStars,
      (await framesStars(ATMOSPHERE_STARS, [INTERIM_STAR])).sceneStars,
      (await framesStars([], [])).sceneStars,
    ]).toEqual([true, false, false]);
  });

  it("draws a kept scene's own stars where the interim field is empty", async () => {
    expect([await spritesDrawn([], []), (await spritesDrawn(ATMOSPHERE_STARS, [])) > 0]).toEqual([
      0,
      true,
    ]);
  });

  it("draws the interim field, not the scene's own stars, where it has stars", async () => {
    expect(await spritesDrawn(ATMOSPHERE_STARS, [INTERIM_STAR])).toBe(1);
  });
});
