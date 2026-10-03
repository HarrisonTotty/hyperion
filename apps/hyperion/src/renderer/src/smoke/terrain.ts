/**
 * The smoke harness's terrain checks (plan R05, R05.T11.a; T11.b adds the frames): the terrain's
 * GPU resources made on the real device at both settings, and the high setting's `FaceDifferences`
 * fallback on a second engine that asks for WebGPU's default limits (decisions-r06-r07.md item 7).
 *
 * @remarks
 * A buffer or texture beyond the device's limits, or a write outside one, raises an uncaptured
 * validation error, which the harness's main process counts against the run, so these checks pass
 * only if the layouts fit the devices they are made on.
 */

import { loadRenderEngine } from "../view/engine/loadEngine";
import { requestAdapterOutcome } from "../view/engine/platform";
import { GraphicsStatusStore, initialGraphicsStatus } from "../view/engine/status";
import type { RenderEngine } from "../view/engine/types";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { TerrainResources } from "../view/terrain/gpu/resources";
import { ContactRecords, InstanceRecords } from "../view/terrain/gpu/uniforms";
import { HEIGHTS_BYTES, OFFSETS_BYTES } from "../view/terrain/slotLayout";
import { type Checks, pause } from "./harness";

/** WGS 84 (NIMA TR8350.2): a, and c = a (1 − f) with 1 ÷ f = 298.257223563. */
const WGS84 = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563) };

/** WebGPU's default `maxStorageBufferBindingSize`, 128 MiB (W3C WebGPU §3.6.2). */
const DEFAULT_STORAGE_BINDING_BYTES = 134_217_728;

/** Makes `setting`'s resources on `engine`, writes its first and last slots and one frame. */
function exercise(engine: RenderEngine, setting: "high" | "low"): TerrainResources {
  const resources = new TerrainResources(engine, TERRAIN_SETTINGS[setting], WGS84);
  const { slots, atlas, vertexPath } = resources.layout;
  const n = atlas.samplesPerSide;
  for (const slot of [0, slots.slotCount - 1]) {
    resources.upload({
      slot,
      key: { face: 1, level: 10, i: 512, j: 300 },
      heights: new Float32Array(HEIGHTS_BYTES / 4).fill(100),
      offsets: vertexPath === "baked-offsets" ? new Float32Array(OFFSETS_BYTES / 4) : null,
      normals: new Float16Array(n * n * 2),
      originHeightM: 100,
      skirtDepthM: 2,
    });
  }
  const instances = new InstanceRecords(slots.slotCount);
  instances.push(slots.slotCount - 1, { x: 0, y: 0, z: -1000 }, 500, 1000);
  const contacts = new ContactRecords();
  contacts.push({ x: 0, y: 0, z: -10 }, 30, 17.7);
  resources.writeFrame(instances, contacts);
  return resources;
}

/** Describes a layout for a check's detail. */
function describe(resources: TerrainResources): string {
  const { slots, atlas, vertexPath, fallback } = resources.layout;
  return `${vertexPath} (fallback ${fallback}), ${slots.slotCount} slots, atlas ${atlas.widthTexels} × ${atlas.heightTexels} × ${atlas.layers}`;
}

/** The terrain's resources on `engine`, then the fallback on an engine with default limits. */
export async function checkTerrainResources(engine: RenderEngine, checks: Checks): Promise<void> {
  const binding = engine.capabilities.maxStorageBufferBindingSize;
  for (const setting of ["high", "low"] as const) {
    const resources = exercise(engine, setting);
    const offsets = resources.field("offsets");
    checks.check(
      `R05.T11.a the ${setting} setting's resources fit the device`,
      (offsets?.bytes ?? 0) <= binding &&
        (resources.field("heights")?.bytes ?? Number.POSITIVE_INFINITY) <= binding,
      `${describe(resources)}; binding ${binding} B`,
    );
    resources.dispose();
  }

  const outcome = await requestAdapterOutcome(navigator.gpu);
  if (outcome.kind !== "adapter") {
    throw new Error(`no adapter for the default-limits engine: ${outcome.kind}`);
  }
  const limited = await loadRenderEngine(
    outcome,
    new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    { overrides: { withholdSubgroups: false, withholdShaderF16: false, defaultLimits: true } },
  );
  try {
    checks.check(
      "R05.T11.a the default-limits engine has WebGPU's 128 MiB binding",
      limited.capabilities.maxStorageBufferBindingSize === DEFAULT_STORAGE_BINDING_BYTES,
      `${limited.capabilities.maxStorageBufferBindingSize} B`,
    );
    const resources = exercise(limited, "high");
    checks.check(
      "R05.T11.a under default limits the high setting falls back to FaceDifferences",
      resources.layout.vertexPath === "face-differences" &&
        resources.layout.fallback === "binding-limit" &&
        resources.field("offsets") === null &&
        (resources.field("heights")?.bytes ?? Number.POSITIVE_INFINITY) <=
          DEFAULT_STORAGE_BINDING_BYTES,
      describe(resources),
    );
    resources.dispose();
    // Uncaptured errors arrive as events: let them reach the page's log before the device goes.
    await pause(250);
  } finally {
    limited.dispose();
  }
}
