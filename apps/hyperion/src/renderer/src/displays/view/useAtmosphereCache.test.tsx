import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { ScriptedOpticsWorker } from "../../test/scriptedOpticsWorker";
import type { OpticsWorkerLike } from "../../view/atmosphere/opticsPool";
import type { GiantRoute } from "../../view/atmosphere/thick/routing";
import { useAtmosphereCache } from "./useAtmosphereCache";

/** Starts a scripted optics worker, stable in identity, as the app's factory is. */
function createWorker(): OpticsWorkerLike {
  return new ScriptedOpticsWorker();
}

describe("useAtmosphereCache (R08.T16.a.2's giant route)", () => {
  it("makes its cache on the setting's giant route, and a cache of its own for another route", async () => {
    const engine = await countingRenderEngine();
    const { result, rerender } = renderHook(
      (giantRoute: GiantRoute) =>
        useAtmosphereCache({
          engineState: { kind: "ready", engine },
          requests: null,
          createWorker,
          giantRoute,
        }),
      { initialProps: "envelope" },
    );
    const envelope = result.current;
    rerender("closed");
    expect([envelope?.giantRoute, result.current?.giantRoute, result.current === envelope]).toEqual(
      ["envelope", "closed", false],
    );
  });
});
