import type { ObjectKindDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import type { SymbolShape } from "../../spatial/marks";
import type { LayerIndex } from "./model";
import { RINGED_CIRCLE_MIN_SIZE_CLASS, starSizeClass, starSymbol } from "./starSymbols";

/** Every kind the wire has, with the shape the owner's draft of the symbol set gives it. */
const SHAPES: ReadonlyArray<readonly [ObjectKindDto, SymbolShape | null]> = [
  ["protostar", "circle"],
  ["pre_main_sequence", "circle"],
  ["dwarf", "circle"],
  ["subgiant", "circle"],
  ["hot_subdwarf", "circle"],
  ["substellar", "circle"],
  ["giant", "ringed-circle"],
  ["supergiant", "ringed-circle"],
  ["wolf_rayet", "ringed-circle"],
  ["white_dwarf", "diamond"],
  ["neutron_star", "triangle"],
  ["black_hole", "square"],
  ["no_remnant", null],
];

const LAYERS: ReadonlyArray<LayerIndex> = [0, 1, 2, 3, 4];

describe("starSymbol", () => {
  it.each(SHAPES)("draws a %s as a %s", (kind, shape) => {
    expect(starSymbol(kind)).toBe(shape);
  });

  it("gives every closed shape but the planets', moons' and contacts' to a star", () => {
    const used = new Set(SHAPES.map(([, shape]) => shape));

    expect(used).toEqual(
      new Set(["circle", "ringed-circle", "diamond", "triangle", "square", null]),
    );
  });
});

describe("starSizeClass", () => {
  it.each(LAYERS)("draws a circle of layer %i at that size class", (layer) => {
    expect(starSizeClass("circle", layer)).toBe(layer);
  });

  it("never draws a ringed circle below size class 2", () => {
    const classes = LAYERS.map((layer) => starSizeClass("ringed-circle", layer));

    expect(RINGED_CIRCLE_MIN_SIZE_CLASS).toBe(2);
    expect(classes).toEqual([2, 2, 2, 3, 4]);
  });

  it("keeps the layer of every other star's shape", () => {
    for (const shape of ["diamond", "triangle", "square"] as const) {
      expect(LAYERS.map((layer) => starSizeClass(shape, layer))).toEqual([0, 1, 2, 3, 4]);
    }
  });
});
