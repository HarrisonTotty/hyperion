import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { packClassMap } from "../../view/bodies/discSurface";
import { type LitBodyInput, planLitBodies } from "../../view/bodies/draw";
import { DISC_ANNULI_HIGH } from "../../view/lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH } from "../../view/lighting/planetshine";
import { lightingFramesOf } from "../../view/lighting/retarded";
import { DEFAULT_EXPOSURE, controlEv100, exposureScale } from "../../view/photometry/exposure";
import { PHASE_GIANT, PHASE_PLANETS, PHASE_STAR, phaseScene } from "../../view/scenes/phaseScene";
import { HostDiscLayer } from "../../view/sky/disc";
import { buildWireframeDrawList, viewStrokesAt } from "../../view/wireframe/drawList";
import { vec3 } from "../../geometry/vec3";
import { FIXTURE_EARTH, FIXTURE_JUPITER } from "../../test/planetaryFixture";
import {
  sceneModelOf,
  shipFrameOf,
  sliceSystemBodies,
  systemSceneState,
  viewBodyOf,
  viewSceneOf,
} from "../../test/sceneFixture";
import { PROVISIONAL_PHOTOMETRY } from "../../view/appearance/fromWire";
import type { PhotometricLaw } from "../../view/appearance/law";
import { lawFor } from "../../view/appearance/phase";
import type { ScreenCircle } from "../../view/bodies/regime";
import { IDENTITY_QUATERNION } from "../../view/camera/quaternion";
import type { TextureHandle } from "../../view/engine/types";
import type { ClassMapSource } from "../../view/terrain/classMap";
import { sceneHostDiscs } from "../../view/lighting/hostLights";
import type { ViewScene } from "../../view/scene/model";
import { litBodiesOf, litLabelsOf, photorealFrame, type PhotorealInputs } from "./photorealFrame";
import { runPose, startRun, startServerRun } from "./viewRun";

const VIEWPORT = { widthPx: 1920, heightPx: 1080 };

/** Stand-in colours, one distinct string per token, as `readTokens` would read them. */
const TOKENS = {
  text: "colour-text",
  textMuted: "colour-text-muted",
  accent: "colour-accent",
  target: "colour-target",
  line: "colour-line",
  surface0: "colour-surface-0",
} as const;

/** A lit body lit as a static scene's: by the frame's hosts and the bodies where drawn. */
function drawnOnly(body: LitBodyInput): LitBodyInput {
  return { ...body, lighting: undefined };
}

async function frameOfPhaseScene(
  sources: Pick<PhotorealInputs, "terrainFootprints" | "classMaps" | "reliefs" | "terrain"> = {},
): Promise<ReturnType<typeof photorealFrame>> {
  const engine = await countingRenderEngine();
  const run = startRun(phaseScene());
  const pose = runPose(run);
  const camera = { pose, fovXRad: (run.camera.fovDeg * Math.PI) / 180 };
  const list = buildWireframeDrawList(run.scene, camera, VIEWPORT, TOKENS, {
    lowSetting: false,
    ev100: controlEv100(DEFAULT_EXPOSURE),
    selection: null,
    destination: null,
    remPx: 16,
    ...viewStrokesAt(1),
  });
  return photorealFrame({
    run,
    pose,
    viewport: VIEWPORT,
    setting: "high",
    exposureScale: exposureScale(controlEv100(DEFAULT_EXPOSURE)),
    list,
    sky: null,
    band: null,
    discs: new HostDiscLayer(engine),
    cube: null,
    previousRegimes: new Map(),
    overlay: null,
    meter: "average",
    ...sources,
  });
}

describe("a photorealistic frame of the phase scene", () => {
  it("lights the planets by the scene's own Sun, placed at its centre", async () => {
    const frame = await frameOfPhaseScene();
    expect(frame.lights).toHaveLength(1);
    expect(frame.lights[0]?.disc.star).toBe(0);
    expect(frame.bodies.map((body) => body.id)).toEqual([...PHASE_PLANETS, PHASE_GIANT]);
    expect(frame.bodies.some((body) => body.id === PHASE_STAR)).toBe(false);
  });

  it("lights each planet by its own lighting frame, retarded to the light that reaches it", async () => {
    const frame = await frameOfPhaseScene();
    const run = startRun(phaseScene());
    const entries = lightingFramesOf(run.scene, runPose(run), run.scene.hostDiscs ?? []);
    expect(frame.bodies.map((body) => body.lighting)).toEqual(entries.map((entry) => entry.frame));
  });

  it("lights the kept scene bit for bit as its drawing places it (T10.a)", async () => {
    const frame = await frameOfPhaseScene();
    const options = {
      camera: frame.camera,
      viewport: frame.viewport,
      exposureScale: frame.exposureScale,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: frame.setting,
    };
    expect(frame.bodies.every((body) => body.lighting !== undefined)).toBe(true);
    expect(planLitBodies(frame.bodies, frame.lights, options, new Map())).toEqual(
      planLitBodies(frame.bodies.map(drawnOnly), frame.lights, options, new Map()),
    );
  });

  it("takes the terrain's footprints as the geometry that writes depth (R10.T10.b)", async () => {
    expect((await frameOfPhaseScene()).depthWriters).toEqual([]);
    const footprint: ScreenCircle = { xPx: 960, yPx: 540, radiusPx: 300 };
    expect((await frameOfPhaseScene({ terrainFootprints: [footprint] })).depthWriters).toEqual([
      footprint,
    ]);
  });

  it("shades each lit body with the view's class maps (R10.T10.d)", async () => {
    const asked: string[] = [];
    const classMaps: ClassMapSource = {
      classMapOf: (body) => {
        asked.push(body);
        return null;
      },
    };
    await frameOfPhaseScene({ classMaps });
    expect(asked).toEqual([...PHASE_PLANETS, PHASE_GIANT]);
  });

  it("gives each lit body its relief from the view's reliefs (R10.T10.e)", async () => {
    const frame = await frameOfPhaseScene({ reliefs: { reliefM: () => 4_000 } });
    expect(frame.bodies.map((body) => body.handoverReliefM)).toEqual(
      [...PHASE_PLANETS, PHASE_GIANT].map(() => 4_000),
    );
  });

  it("asks the view's terrain for each lit body's survey coverage (R10.T10.e)", async () => {
    const asked: string[] = [];
    await frameOfPhaseScene({
      terrain: {
        surveyCoverage: (body) => {
          asked.push(body);
          return null;
        },
        draws: () => [],
      },
    });
    expect(asked).toEqual([...PHASE_PLANETS, PHASE_GIANT]);
  });

  it("carries the view's terrain, which draws the bodies handed over (R10.T10.e)", async () => {
    const terrain = { surveyCoverage: () => null, draws: () => [] };
    expect((await frameOfPhaseScene({ terrain })).terrain).toBe(terrain);
  });

  it("draws no sky without one, and takes the camera's role and field", async () => {
    const frame = await frameOfPhaseScene();
    expect(frame.sky).toEqual([]);
    expect(frame.role).toBe("eye");
    expect(frame.camera.fovXRad).toBeCloseTo(Math.PI / 3, 12);
  });
});

/** The slice's server scene, the ship 1 au out, as the view draws it, with its camera's pose. */
function sliceScene() {
  const model = sceneModelOf(
    systemSceneState(
      sliceSystemBodies(),
      { seconds: 3_000, nanos: 0 },
      vec3(1.5e11, 2e9, 0),
      vec3(0, 29_780, 0),
    ),
  );
  const scene = viewSceneOf(model, shipFrameOf(model));
  return { scene, pose: runPose(startServerRun(scene)) };
}

describe("a server scene's lit bodies (R07.T2.b)", () => {
  it("shade a body with its sections' figure and photometry, and pass its rotation", () => {
    const { scene, pose } = sliceScene();
    const earth = viewBodyOf(scene, FIXTURE_EARTH);
    const lit = litBodiesOf(scene, pose, sceneHostDiscs(scene, null)).find(
      (body) => body.id === FIXTURE_EARTH,
    );
    expect([lit?.figure, lit?.photometry, lit?.rotation]).toEqual([
      earth.appearance?.figure,
      earth.appearance?.photometry,
      earth.rotation,
    ]);
    expect(lit?.photometry.provenance).toBe("modelled");
  });

  it("shade a body without a photometric section with the provisional photometry", () => {
    const { scene, pose } = sliceScene();
    const lit = litBodiesOf(scene, pose, sceneHostDiscs(scene, null)).find(
      (body) => body.id === FIXTURE_JUPITER,
    );
    expect(lit?.photometry).toBe(PROVISIONAL_PHOTOMETRY);
    expect(lit).toBeDefined();
    expect(lit).not.toHaveProperty("rotation");
  });

  it("shade each body's disc with its class map, oriented where its rotation is known and elsewhere's alone where it is null (R10.T10.d)", () => {
    const { scene, pose } = sliceScene();
    const weights: TextureHandle = { kind: "texture", name: "slice class map" };
    const texels = packClassMap(1, 1, () => [1]);
    const dark = lawFor([0.05, 0.05, 0.05], [0.6, 0.6, 0.6], "moon");
    const asked: Array<readonly [string, PhotometricLaw]> = [];
    const classMaps: ClassMapSource = {
      classMapOf: (body, elsewhere) => {
        asked.push([body, elsewhere]);
        return { kind: "class-map", weights, texels, laws: [dark], elsewhere };
      },
    };
    const lit = litBodiesOf(scene, pose, sceneHostDiscs(scene, null), classMaps);
    const earth = lit.find((body) => body.id === FIXTURE_EARTH);
    const jupiter = lit.find((body) => body.id === FIXTURE_JUPITER);
    if (earth === undefined || jupiter === undefined) {
      throw new Error("the slice lights its Earth and its Jupiter");
    }
    // Each asked with its own photometry's law as `elsewhere`.
    expect(asked).toContainEqual([FIXTURE_EARTH, earth.photometry.law]);
    expect(asked).toContainEqual([FIXTURE_JUPITER, PROVISIONAL_PHOTOMETRY.law]);
    expect(earth.surface).toMatchObject({ kind: "class-map", laws: [dark] });
    expect(earth.rotation).toEqual(viewBodyOf(scene, FIXTURE_EARTH).rotation);
    expect(jupiter.surface?.kind).toBe("class-map");
    expect(jupiter).not.toHaveProperty("rotation");
    // Drawn as discs straight ahead: Earth's map oriented by its rotation, Jupiter's elsewhere alone.
    const options = {
      camera: { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
      viewport: VIEWPORT,
      exposureScale: 1e-3,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high" as const,
    };
    const ahead = (body: LitBodyInput): LitBodyInput => ({
      ...drawnOnly(body),
      centreM: vec3(0, 0, -40 * body.figure.equatorialRadiusM),
    });
    const surfaces = [earth, jupiter].map((body) => {
      const plan = planLitBodies([ahead(body)], [], options, new Map([[body.id, "disc"]]));
      return plan.discs[0]?.surface;
    });
    expect(surfaces[0]).toMatchObject({ kind: "class-map", rotation: earth.rotation });
    expect(surfaces[1]).toEqual({ kind: "uniform", law: PROVISIONAL_PHOTOMETRY.law });
  });

  it("shade every disc uniform without class maps", () => {
    const { scene, pose } = sliceScene();
    const lit = litBodiesOf(scene, pose, sceneHostDiscs(scene, null));
    expect(lit.filter((body) => body.surface !== undefined)).toEqual([]);
  });

  it("give each body its relief and its survey's coverage mask where the view's terrain holds them, and none hands over without (R10.T10.e)", () => {
    const { scene, pose } = sliceScene();
    const mask: TextureHandle = { kind: "texture", name: "slice coverage mask" };
    const lit = litBodiesOf(scene, pose, sceneHostDiscs(scene, null), undefined, {
      reliefs: { reliefM: (body) => (body === FIXTURE_EARTH ? 18_000 : null) },
      terrain: { surveyCoverage: (body) => (body === FIXTURE_EARTH ? mask : null) },
    });
    const earth = lit.find((body) => body.id === FIXTURE_EARTH);
    const jupiter = lit.find((body) => body.id === FIXTURE_JUPITER);
    // The mask comes with the axes it is read in, Earth's rotation.
    expect([earth?.handoverReliefM, earth?.surveyCoverage]).toEqual([
      18_000,
      { mask, rotation: viewBodyOf(scene, FIXTURE_EARTH).rotation },
    ]);
    expect(jupiter).not.toHaveProperty("handoverReliefM");
    expect(jupiter).not.toHaveProperty("surveyCoverage");
    const bare = litBodiesOf(scene, pose, sceneHostDiscs(scene, null));
    expect(
      bare.filter(
        (body) => body.handoverReliefM !== undefined || body.surveyCoverage !== undefined,
      ),
    ).toEqual([]);
  });

  it("give no coverage mask to a body without a rotation to read it in (R10.T10.e)", () => {
    const { scene, pose } = sliceScene();
    const mask: TextureHandle = { kind: "texture", name: "slice coverage mask" };
    const lit = litBodiesOf(scene, pose, sceneHostDiscs(scene, null), undefined, {
      terrain: { surveyCoverage: () => mask },
    });
    expect(lit.find((body) => body.id === FIXTURE_JUPITER)).not.toHaveProperty("surveyCoverage");
  });

  it("state the provisional photometry's label while one lit body takes it", () => {
    expect(litLabelsOf(sliceScene().scene)).toEqual(["BODY PHOTOMETRY: NOT YET MODELLED"]);
  });

  it("state no label when every lit body carries its photometric section", () => {
    const { scene } = sliceScene();
    const modelled: ViewScene = {
      ...scene,
      bodies: scene.bodies.filter((body) => body.id !== FIXTURE_JUPITER),
    };
    expect(litLabelsOf(modelled)).toEqual([]);
  });

  it("state no label when every lit body carries an ok section, one of them flagged provisional", () => {
    // decision-r07-provisional-photometry: a flagged section is drawn as it states, unlabelled.
    const bodies = sliceSystemBodies();
    const earth = bodies.bodies.find((body) => body.id === FIXTURE_EARTH);
    const jupiter = bodies.bodies.find((body) => body.id === FIXTURE_JUPITER);
    if (earth?.photometry?.state !== "ok" || jupiter === undefined) {
      throw new Error("the fixture's Earth has a photometric section, beside its Jupiter");
    }
    // Jupiter with a section of its template, flagged as a hot giant's would be.
    const flaggedJupiter = {
      ...jupiter,
      photometry: {
        state: "ok" as const,
        value: { ...earth.photometry.value, phase_template: "jupiter" as const, provisional: true },
      },
    };
    const withFlagged = {
      ...bodies,
      bodies: bodies.bodies.map((body) => (body.id === FIXTURE_JUPITER ? flaggedJupiter : body)),
    };
    const model = sceneModelOf(
      systemSceneState(
        withFlagged,
        { seconds: 3_000, nanos: 0 },
        vec3(1.5e11, 2e9, 0),
        vec3(0, 29_780, 0),
      ),
    );
    const scene = viewSceneOf(model, shipFrameOf(model));
    expect(viewBodyOf(scene, FIXTURE_JUPITER).appearance?.photometry.provenance).toBe(
      "modelled-provisional",
    );
    expect(litLabelsOf(scene)).toEqual([]);
  });

  it("state the label for a kept scene's bodies, which carry no appearance", () => {
    expect(litLabelsOf(phaseScene().sceneAt(0))).toEqual(["BODY PHOTOMETRY: NOT YET MODELLED"]);
  });
});
