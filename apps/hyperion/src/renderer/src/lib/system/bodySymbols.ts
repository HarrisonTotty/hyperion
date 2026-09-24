/**
 * The ship-wide symbol of every kind of body, and the size it is drawn at (plan 14, P14.T42.a; the
 * owner's draft of the guide's symbol set, which the client is built to).
 *
 * @remarks
 * Shape encodes the type and one shape means one thing on every spatial display: a planet, bound or
 * free-floating, is plan 13's inverted triangle, whatever its class, with its size telling a giant
 * from a smaller planet; a moon is the pentagon; an unresolved contact, a body whose kind is
 * withheld, the hexagon. Rings, belts, discs and the cometary halo are populations, not points, and
 * take no symbol. The list and the readout name every kind in words, so shape is never the only
 * signal.
 */
import type { SizeClass, SymbolShape } from "../../spatial/marks";
import type { SystemBody } from "./model";

/**
 * Plan 05's size classes for planets: a giant 3, a smaller planet 1, a dwarf planet 0, each drawn
 * no smaller than {@link BODY_MIN_SIZE_CLASS}.
 */
export const PLANET_SIZE_CLASSES = { giant: 3, planet: 1, dwarfPlanet: 0 } as const;

/**
 * The smallest size class a body's symbol is drawn at: 1 (the orchestrator's rulings 35.5 and
 * 36.4), which raises a dwarf planet to a smaller planet's size.
 */
export const BODY_MIN_SIZE_CLASS = 1;

/**
 * The size class a moon's pentagon is drawn at: the smallest from {@link BODY_MIN_SIZE_CLASS} at
 * which its flat side lies at least half the 1.5 px outline inside a circle of its size, 0.95 px
 * at 100%, so that it reads apart from a star's circle (the orchestrator's ruling 35.5).
 */
export const MOON_SIZE_CLASS = 1;

/**
 * The size class an unresolved contact's hexagon is drawn at, by the same test as a moon's: at
 * class 1 its flat sides lie only 0.67 px inside the circle, and at 2 they lie 0.80 px inside.
 */
export const CONTACT_SIZE_CLASS = 2;

/** A body's symbol and its size, or `null` for a population, which is not a point. */
export interface BodySymbol {
  readonly shape: SymbolShape;
  readonly sizeClass: SizeClass;
}

function atLeastFloor(sizeClass: SizeClass): SizeClass {
  return sizeClass < BODY_MIN_SIZE_CLASS ? BODY_MIN_SIZE_CLASS : sizeClass;
}

/**
 * The symbol a body is drawn with, or `null` for a population (a ring, a belt, a disc, the halo).
 *
 * @remarks
 * A planet is a giant when its bulk section names a gas or ice giant. Below the `bulk` detail level
 * its class is withheld, and it is drawn at a smaller planet's size, which the readout's
 * `NOT RESOLVED` explains.
 */
export function bodySymbol(body: SystemBody): BodySymbol | null {
  let symbol: BodySymbol | null;
  switch (body.kind.kind) {
    case "planet": {
      const planetClass = body.bulk.state === "ok" ? body.bulk.value.planetClass : null;
      const giant = planetClass === "gas_giant" || planetClass === "ice_giant";
      symbol = {
        shape: "triangle-down",
        sizeClass: atLeastFloor(giant ? PLANET_SIZE_CLASSES.giant : PLANET_SIZE_CLASSES.planet),
      };
      break;
    }
    case "dwarf_planet":
      symbol = { shape: "triangle-down", sizeClass: atLeastFloor(PLANET_SIZE_CLASSES.dwarfPlanet) };
      break;
    case "moon":
      symbol = { shape: "pentagon", sizeClass: MOON_SIZE_CLASS };
      break;
    case "unresolved":
      symbol = { shape: "hexagon", sizeClass: CONTACT_SIZE_CLASS };
      break;
    case "ring":
    case "belt":
    case "cometary_halo":
    case "protoplanetary_disc":
    case "debris_disc":
      symbol = null;
      break;
  }
  return symbol;
}
