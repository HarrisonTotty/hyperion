import { beforeAll, describe, expect, it } from "vitest";

import {
  bandLimitM as moduleBandLimitM,
  finestSpacingM,
  initSync,
  levelTable,
  Ridges,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { FINEST_SPACING_M } from "./cube";
import { bandLimitM } from "./planet";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

describe("the planet's mirror against the surface module", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  it("mirrors the module's band limit and finest spacing", () => {
    expect(bandLimitM()).toBe(moduleBandLimitM());
    expect(FINEST_SPACING_M).toBe(finestSpacingM());
  });

  it("gets from the module the level table the golden pins", () => {
    expect([...levelTable(Ridges.Off)]).toEqual([...goldenLevelTable("off")]);
    expect([...levelTable(Ridges.On)]).toEqual([...goldenLevelTable("on")]);
  });
});
