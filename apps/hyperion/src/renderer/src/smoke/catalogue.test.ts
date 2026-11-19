import { describe, expect, it } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../test/fakeGpu";
import { FakeRenderEngine } from "../test/fakeRenderEngine";
import type { CatalogueEntry } from "../view/engine/catalogue";
import { GraphicsStatusStore, initialGraphicsStatus } from "../view/engine/status";
import type { MaterialHandle, WgslMaterialSpec } from "../view/engine/types";
import { BROKEN_ENTRY, checkCatalogue } from "./catalogue";
import { Checks, flatSpec } from "./harness";

/** An engine whose compiler refuses the material named `refused`, as SwiftShader's refuses bad WGSL. */
class RefusingEngine extends FakeRenderEngine {
  readonly #refused: string;

  constructor(device: GPUDevice, refused: string) {
    super(device);
    this.#refused = refused;
  }

  override createMaterialAsync(spec: WgslMaterialSpec): Promise<MaterialHandle> {
    return spec.name === this.#refused
      ? Promise.reject(new Error(`material ${spec.name} failed to compile`))
      : super.createMaterialAsync(spec);
  }
}

/** The checks `entries` give on an engine refusing the material `refused`, by name and pass. */
async function catalogueChecks(
  entries: ReadonlyArray<CatalogueEntry>,
  refused = "",
): Promise<ReadonlyArray<readonly [string, boolean]>> {
  const device = await new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
  const checks = new Checks();
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  await checkCatalogue(new RefusingEngine(device, refused), status, entries, checks);
  return checks.list.map((check) => [check.name, check.pass] as const);
}

describe("the catalogue's settings (R01.T9.c; plan R12, R12.T7.a)", () => {
  it("renders an entry once at default where it declares no setting", async () => {
    const entry: CatalogueEntry = { kind: "material", spec: flatSpec("any setting") };
    expect(await catalogueChecks([entry])).toEqual([
      ["T9.b material any setting (default)", true],
      ["T9.b no shader was refused", true],
    ]);
  });

  it("renders an entry declaring both ladder settings at each of them", async () => {
    const entry: CatalogueEntry = {
      kind: "material",
      spec: flatSpec("two settings"),
      settings: ["high", "low"],
    };
    expect(await catalogueChecks([entry])).toEqual([
      ["T9.b material two settings (high)", true],
      ["T9.b material two settings (low)", true],
      ["T9.b no shader was refused", true],
    ]);
  });

  it("fails the broken fixture at both settings it declares, naming each", async () => {
    expect(BROKEN_ENTRY.settings).toEqual(["high", "low"]);
    expect(await catalogueChecks([BROKEN_ENTRY], BROKEN_ENTRY.spec.name)).toEqual([
      ["T9.b material broken fixture (high)", false],
      ["T9.b material broken fixture (low)", false],
      ["T9.b no shader was refused", true],
    ]);
  });
});
