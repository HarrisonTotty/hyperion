import { describe, expect, it } from "vitest";

import type { FrameSubmission } from "../types";
import { sceneLoadOp } from "./drawing";
import { pipelineOutputs } from "./engine";
import { canvasPassFormat } from "./view";

const FRAME: FrameSubmission = {
  label: "frame",
  viewRotation: new Float32Array(16),
  projection: new Float32Array(16),
  draws: [],
  postProcesses: [],
};

describe("a frame's encoding on a canvas (R07.T15)", () => {
  it("writes through the sRGB twin by default, and the canvas's own format in-pass", () => {
    expect(canvasPassFormat("bgra8unorm", undefined)).toBe("bgra8unorm-srgb");
    expect(canvasPassFormat("bgra8unorm", "srgb-view")).toBe("bgra8unorm-srgb");
    expect(canvasPassFormat("rgba8unorm", "in-pass")).toBe("rgba8unorm");
  });

  it("makes a material's pipelines ahead for the format each canvas target writes", () => {
    expect(pipelineOutputs("canvas", "bgra8unorm")).toEqual([["bgra8unorm-srgb", true]]);
    expect(pipelineOutputs("canvas-in-pass", "bgra8unorm")).toEqual([["bgra8unorm", true]]);
    expect(pipelineOutputs("rgba16float", "bgra8unorm")).toEqual([
      ["rgba16float", true],
      ["rgba16float", false],
    ]);
  });
});

describe("a frame's load operation (R07.T15, for T16)", () => {
  it("clears by default, and loads when asked without post-processes", () => {
    expect(sceneLoadOp(FRAME, false)).toBe("clear");
    expect(sceneLoadOp({ ...FRAME, colourLoad: "clear" }, false)).toBe("clear");
    expect(sceneLoadOp({ ...FRAME, colourLoad: "load" }, false)).toBe("load");
  });

  it("clears where a post-process chain follows, whatever is asked", () => {
    expect(sceneLoadOp({ ...FRAME, colourLoad: "load" }, true)).toBe("clear");
  });
});
