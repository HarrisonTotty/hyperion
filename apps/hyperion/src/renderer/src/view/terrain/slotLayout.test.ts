import { describe, expect, it } from "vitest";

import { SETTINGS, type TerrainVertexPath } from "../quality/qualitySetting";
import { MATURITY_NODES } from "./maturityNodes";
import {
  CLASS_LAWS_BYTES,
  CLASS_LAYERS_BYTES,
  CLASS_MATURITY_BYTES,
  CLASS_NODES_BYTES,
  CLASS_WEIGHTS_BYTES,
  DOUBLE_NORMALS_BYTES,
  HEIGHTS_BYTES,
  HORIZON_MAP_BYTES,
  MESH_NORMALS_BYTES,
  OFFSETS_BYTES,
  type SlotField,
  SURVEY_CELL_CODE_BYTE,
  SURVEY_MASK_BYTES,
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

  it("adds R10.T10.b's class layers to a lit layout alone, its weights, laws, maturity codes and nodes, 54,028 B a slot", () => {
    expect(CLASS_WEIGHTS_BYTES).toBe(65 * 65 * 8);
    expect(CLASS_LAWS_BYTES).toBe(256);
    // R10.T1.c's client half: two unorm16 codes a vertex, one word, and 8 slots × 2 levels × K
    // nodes × 3 channels as f32, K = 16.
    expect(CLASS_MATURITY_BYTES).toBe(65 * 65 * 2 * 2);
    expect(CLASS_NODES_BYTES).toBe(8 * 2 * MATURITY_NODES * 3 * 4);
    expect([CLASS_MATURITY_BYTES, CLASS_NODES_BYTES]).toEqual([16_900, 3_072]);
    expect(CLASS_LAYERS_BYTES).toBe(
      CLASS_WEIGHTS_BYTES + CLASS_LAWS_BYTES + CLASS_MATURITY_BYTES + CLASS_NODES_BYTES,
    );
    expect(CLASS_LAYERS_BYTES).toBe(54_028);
    // Each part starts on a word, as a buffer write's offset and size must.
    expect(
      [CLASS_WEIGHTS_BYTES, CLASS_LAWS_BYTES, CLASS_MATURITY_BYTES, CLASS_NODES_BYTES].map(
        (bytes) => bytes % 4,
      ),
    ).toEqual([0, 0, 0, 0]);
    const spike = terrainSlotLayout(SETTINGS.low.terrain);
    const lit = terrainSlotLayout(SETTINGS.low.terrain, "lit");
    expect(spike.fields.map((f) => f.name)).not.toContain("class-layers");
    expect(lit.fields.at(-2)).toEqual({
      name: "class-layers",
      storage: "storage-buffer",
      bytes: CLASS_LAYERS_BYTES,
    });
    expect(lit.bytesPerSlot - spike.bytesPerSlot).toBe(
      CLASS_LAYERS_BYTES + HORIZON_MAP_BYTES + SURVEY_MASK_BYTES,
    );
  });

  it("adds R10.T10.c's survey mask to a lit layout alone, 65 × 65 bytes four to a word, 4,228 B a slot", () => {
    expect(SURVEY_MASK_BYTES).toBe(4_228);
    expect(SURVEY_MASK_BYTES % 4).toBe(0);
    // The cell code's byte is the first past the vertices', in the last word's spare bytes.
    expect(SURVEY_CELL_CODE_BYTE).toBe(65 * 65);
    expect(SURVEY_CELL_CODE_BYTE).toBeLessThan(SURVEY_MASK_BYTES);
    const lit = terrainSlotLayout(SETTINGS.low.terrain, "lit");
    expect(lit.fields.at(-1)).toEqual({
      name: "survey-mask",
      storage: "storage-buffer",
      bytes: SURVEY_MASK_BYTES,
    });
    expect(terrainSlotFields(SETTINGS.high.terrain).map((f) => f.name)).not.toContain(
      "survey-mask",
    );
    // The low lit slot is 178,908 B with R10.T1.c's maturity layer (158,936 B before): 375 slots in
    // 64 MiB (422 before), against Design note 15's rule of 430.
    expect(lit.bytesPerSlot).toBe(178_908);
    expect(lit.slotCount).toBe(375);
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

describe("the wireframe's slot layout (R10.T11)", () => {
  it("holds a face-differences wire slot's heights and survey mask alone, its normals and horizon map at no bytes", () => {
    expect(terrainSlotFields(SETTINGS.low.terrain, "wire")).toEqual([
      { name: "heights", storage: "storage-buffer", bytes: HEIGHTS_BYTES },
      { name: "normals", storage: "texture", bytes: 0 },
      { name: "horizon-map", storage: "storage-buffer", bytes: 0 },
      { name: "survey-mask", storage: "storage-buffer", bytes: SURVEY_MASK_BYTES },
    ]);
  });

  it("adds the offsets to a baked-offsets wire slot, and no normals at twice the mesh's resolution", () => {
    const { terrain } = SETTINGS.high;
    expect([terrain.normals, terrainSlotFields(terrain, "wire")]).toEqual([
      "double",
      [
        { name: "heights", storage: "storage-buffer", bytes: HEIGHTS_BYTES },
        { name: "offsets", storage: "storage-buffer", bytes: OFFSETS_BYTES },
        { name: "normals", storage: "texture", bytes: 0 },
        { name: "horizon-map", storage: "storage-buffer", bytes: 0 },
        { name: "survey-mask", storage: "storage-buffer", bytes: SURVEY_MASK_BYTES },
      ],
    ]);
  });

  it("sizes a wire slot at 38,028 B on the face-differences path and 139,428 B on the baked-offsets path", () => {
    expect([
      terrainSlotLayout(SETTINGS.low.terrain, "wire").bytesPerSlot,
      terrainSlotLayout(SETTINGS.high.terrain, "wire").bytesPerSlot,
    ]).toEqual([33_800 + 4_228, 33_800 + 101_400 + 4_228]);
  });

  it("carries no class layers in a wire slot, which draws no class", () => {
    const names = (["low", "high"] as const).flatMap((setting) =>
      terrainSlotFields(SETTINGS[setting].terrain, "wire").map((f) => f.name),
    );
    expect(names).not.toContain("class-layers");
  });
});
