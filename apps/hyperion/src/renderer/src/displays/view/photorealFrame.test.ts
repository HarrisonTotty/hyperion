import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
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
import { sceneHostDiscs } from "../../view/lighting/hostLights";
import type { ViewScene } from "../../view/scene/model";
import { litBodiesOf, litLabelsOf, photorealFrame } from "./photorealFrame";
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

async function frameOfPhaseScene(): Promise<ReturnType<typeof photorealFrame>> {
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
