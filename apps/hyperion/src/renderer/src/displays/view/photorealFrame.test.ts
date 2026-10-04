import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { DEFAULT_EXPOSURE, controlEv100, exposureScale } from "../../view/photometry/exposure";
import { PHASE_GIANT, PHASE_PLANETS, PHASE_STAR, phaseScene } from "../../view/scenes/phaseScene";
import { HostDiscLayer } from "../../view/sky/disc";
import { buildWireframeDrawList } from "../../view/wireframe/drawList";
import { photorealFrame } from "./photorealFrame";
import { runPose, startRun } from "./viewRun";

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

  it("draws no sky without one, and takes the camera's role and field", async () => {
    const frame = await frameOfPhaseScene();
    expect(frame.sky).toEqual([]);
    expect(frame.role).toBe("eye");
    expect(frame.camera.fovXRad).toBeCloseTo(Math.PI / 3, 12);
  });
});
