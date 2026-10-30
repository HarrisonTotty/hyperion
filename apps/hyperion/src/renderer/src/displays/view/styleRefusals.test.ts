import { describe, expect, it } from "vitest";

import type { GraphicsLaunchMode } from "../../../../preload/api";
import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { requestAdapterOutcome } from "../../view/engine/platform";
import {
  type GraphicsStatus,
  initialGraphicsStatus,
  reduceGraphicsStatus,
} from "../../view/engine/status";
import { availabilityOf, PHOTOREAL_NOT_CREATED, styleRefusals } from "./styleRefusals";

async function answered(
  software: boolean,
  launchMode: GraphicsLaunchMode = "vulkan",
): Promise<GraphicsStatus> {
  const outcome = await requestAdapterOutcome(
    new FakeGpu([
      new FakeAdapter({ info: software ? SWIFTSHADER_INFO : INTEL_UHD_620_INFO, features: [] }),
    ]),
  );
  return reduceGraphicsStatus(initialGraphicsStatus(launchMode, false), {
    kind: "adapter-outcome",
    outcome,
  });
}

describe("styleRefusals", () => {
  it("offers both styles on a hardware adapter", async () => {
    expect(styleRefusals(await answered(false), "idle")).toEqual({
      wireframe: null,
      photorealistic: null,
    });
  });

  it("holds the photorealistic style back on a software adapter, saying so", async () => {
    expect(styleRefusals(await answered(true), "idle").photorealistic).toBe(
      "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
    );
  });

  it("holds it back on a software adapter in default mode too", async () => {
    expect(styleRefusals(await answered(true, "default"), "idle")).toEqual({
      wireframe: null,
      photorealistic: "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
    });
  });

  it("offers both styles on a software adapter in the check-only software mode (R01.T16)", async () => {
    expect(styleRefusals(await answered(true, "software"), "idle")).toEqual({
      wireframe: null,
      photorealistic: null,
    });
  });

  it("still refuses a software mode's photorealistic style whose pipelines failed", async () => {
    expect(styleRefusals(await answered(true, "software"), "failed").photorealistic).toBe(
      PHOTOREAL_NOT_CREATED,
    );
  });

  it("holds it back while the adapter is acquired, and where its pipelines failed", async () => {
    expect(styleRefusals(initialGraphicsStatus("vulkan", false), "idle").photorealistic).toBe(
      "GRAPHICS ACQUIRING ADAPTER",
    );
    expect(styleRefusals(await answered(false), "failed").photorealistic).toBe(
      PHOTOREAL_NOT_CREATED,
    );
  });

  it("holds both back in safe mode, where no view is drawn", () => {
    const refusals = styleRefusals(initialGraphicsStatus("safe", false), "idle");
    expect(availabilityOf(refusals)).toEqual({ wireframe: false, photorealistic: false });
    expect(refusals.wireframe).toMatch(/^GRAPHICS SAFE MODE/);
  });
});
