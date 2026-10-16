import { describe, expect, it } from "vitest";

import { SETTINGS, type TerrainVertexPath } from "../quality/qualitySetting";
import {
  CLASS_LAWS_BYTES,
  CLASS_LAYERS_BYTES,
  CLASS_WEIGHTS_BYTES,
  DOUBLE_NORMALS_BYTES,
  HEIGHTS_BYTES,
  HORIZON_MAP_BYTES,
  MESH_NORMALS_BYTES,
  OFFSETS_BYTES,
  type SlotField,
  slotLayout,
  terrainSlotFields,
  terrainSlotLayout,
} from "./slotLayout";

const MIB = 1024 * 1024;

/** The low setting's horizon-map field under `shading`. */
function horizon(shading: "spike" | "lit"): SlotField | undefined {
  return terrainSlotFields(SETTINGS.low.terrain, shading).find((f) => f.name === "horizon-map");
}

function fieldNames(path: TerrainVertexPath): string[] {
  return terrainSlotFields({ ...SETTINGS.high.terrain, vertexPath: path }).map((f) => f.name);
}

describe("the slot layout", () => {
  it("sizes this plan's fields as Design notes 4 and 10 do", () => {
    expect(HEIGHTS_BYTES).toBe(33_800);
    expect(MESH_NORMALS_BYTES).toBe(67 * 67 * 4);
    expect(DOUBLE_NORMALS_BYTES).toBe(131 * 131 * 4);
    expect(OFFSETS_BYTES).toBe(101_400);
  });

  it("gives the low setting 51,756 B a slot, the gutter included, and 1,296 slots in 64 MiB", () => {
    const layout = terrainSlotLayout(SETTINGS.low.terrain);
    expect(layout.bytesPerSlot).toBe(51_756);
    expect(layout.slotCount).toBe(1_296);
  });

  it("sizes the horizon map on a lit layout alone, 68,896 B a slot, and none on R05's spike", () => {
    expect(horizon("spike")?.bytes).toBe(0);
    expect(HORIZON_MAP_BYTES).toBe((65 * 65 + 9 * 9) * 16);
    expect(horizon("lit")).toEqual({
      name: "horizon-map",
      storage: "storage-buffer",
      bytes: HORIZON_MAP_BYTES,
    });
  });

  it("adds R10.T10.b's class layers, its weights then its laws, to a lit layout alone, 34,056 B a slot", () => {
    expect(CLASS_WEIGHTS_BYTES).toBe(65 * 65 * 8);
    expect(CLASS_LAWS_BYTES).toBe(256);
    expect(CLASS_LAYERS_BYTES).toBe(34_056);
    const spike = terrainSlotLayout(SETTINGS.low.terrain);
    const lit = terrainSlotLayout(SETTINGS.low.terrain, "lit");
    expect(spike.fields.map((f) => f.name)).not.toContain("class-layers");
    expect(lit.fields.at(-1)).toEqual({
      name: "class-layers",
      storage: "storage-buffer",
      bytes: CLASS_LAYERS_BYTES,
    });
    expect(lit.bytesPerSlot - spike.bytesPerSlot).toBe(CLASS_LAYERS_BYTES + HORIZON_MAP_BYTES);
  });

  it("carries offsets on the baked-offsets path only", () => {
    expect(fieldNames("baked-offsets")).toContain("offsets");
    expect(fieldNames("face-differences")).not.toContain("offsets");
  });

  it("gives R10's 156 kB low layout about 430 slots in 64 MiB", () => {
    // R10 Design note 15: heights 33.8 kB, normals 16.9 kB, horizon map 67.6 kB, and the class
    // weights (33.8 kB) and survey mask (4.2 kB), which R10 adds as fields of their own, counted
    // here in the horizon map's.
    const layout = slotLayout(
      [
        { name: "heights", storage: "storage-buffer", bytes: 33_800 },
        { name: "normals", storage: "texture", bytes: 16_900 },
        { name: "horizon-map", storage: "storage-buffer", bytes: 67_600 + 33_800 + 4_200 },
      ],
      64 * MIB,
    );
    expect(layout.bytesPerSlot).toBe(156_300);
    expect(layout.slotCount).toBeGreaterThanOrEqual(425);
    expect(layout.slotCount).toBeLessThanOrEqual(435);
  });

  it("refuses a budget that does not hold the six roots", () => {
    expect(() =>
      slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], 500),
    ).toThrow(/fewer than 6/u);
  });
});
