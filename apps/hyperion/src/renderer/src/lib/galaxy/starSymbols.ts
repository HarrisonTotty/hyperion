/**
 * The ship-wide symbol of every kind of star, and the size it is drawn at (plan 06, P06.T35.b and
 * design note 17; the owner's draft of the guide's symbol set, which the client is built to).
 *
 * @remarks
 * Shape encodes the type and one shape means one thing on every spatial display: a circle is a
 * protostar, a pre-main-sequence star, a dwarf, a subgiant, a hot subdwarf or a brown dwarf; the
 * ringed circle a giant, a supergiant or a Wolf-Rayet star; the diamond a white dwarf; the triangle
 * a neutron star; the square a black hole. A star that left no remnant has nothing to draw, and is
 * listed and not drawn. The list and the readout beside every view name the kind in words, so shape
 * is never the only signal.
 */
import type { ObjectKindDto } from "@hyperion/protocol";

import type { SizeClass, SymbolShape } from "../../spatial/marks";
import type { LayerIndex } from "./model";

/**
 * The smallest size class a ringed circle is drawn at: 2, where its ring, the gap inside it and its
 * disc are each at least as wide as the outline (the orchestrator's ruling 35.4).
 *
 * @remarks
 * At size class 0 the ring's and the disc's 1.5 px outlines take 6 of its 8 px, and open could not
 * be told from filled. Giants come from the heavier mass layers in a galaxy of the Milky Way's age,
 * so the floor rarely moves one.
 */
export const RINGED_CIRCLE_MIN_SIZE_CLASS = 2;

/**
 * The symbol a star of this kind is drawn with, or `null` for a star that left no remnant, which is
 * not drawn.
 *
 * @remarks
 * Exhaustive over the wire's kinds, with no default, so that a kind the protocol adds is a type
 * error here until it has a shape.
 */
export function starSymbol(kind: ObjectKindDto): SymbolShape | null {
  let shape: SymbolShape | null;
  switch (kind) {
    case "protostar":
    case "pre_main_sequence":
    case "dwarf":
    case "subgiant":
    case "hot_subdwarf":
    case "substellar":
      shape = "circle";
      break;
    case "giant":
    case "supergiant":
    case "wolf_rayet":
      shape = "ringed-circle";
      break;
    case "white_dwarf":
      shape = "diamond";
      break;
    case "neutron_star":
      shape = "triangle";
      break;
    case "black_hole":
      shape = "square";
      break;
    case "no_remnant":
      shape = null;
      break;
  }
  return shape;
}

/**
 * The size class a star's symbol is drawn at: its mass layer, as on the chart (the orchestrator's
 * ruling 36), raised to {@link RINGED_CIRCLE_MIN_SIZE_CLASS} for a ringed circle.
 *
 * @param layer - The mass layer the star's initial mass falls in, 0 (A) to 4 (E).
 */
export function starSizeClass(shape: SymbolShape, layer: LayerIndex): SizeClass {
  return shape === "ringed-circle" && layer < RINGED_CIRCLE_MIN_SIZE_CLASS
    ? RINGED_CIRCLE_MIN_SIZE_CLASS
    : layer;
}
