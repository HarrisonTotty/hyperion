/**
 * Surface sections for the thermal emission's tests (plan R08, R08.T18.a): the protocol fixture's
 * locked lava world and Earth as the client decodes them, and the forms of them a test needs.
 */

import { toBodyDetail } from "../lib/system/bodiesWire";
import type { BodySurface } from "../lib/system/model";
import { earthDetail, FIXTURE_SYSTEM, lavaWorldDetail } from "./planetaryFixture";

/** The designation the fixture's records are decoded under; the surface does not read it. */
const DESIGNATION = "H7K 4C0RFZ D-7";

/** The surface section of a fixture's `body_detail` answer as the client decodes it. */
function surfaceOf(answer: ReturnType<typeof earthDetail>): BodySurface {
  const result = toBodyDetail(answer, FIXTURE_SYSTEM, DESIGNATION);
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  const { surface } = result.detail.record;
  if (surface.state !== "ok") {
    throw new Error(`the fixture's surface is ${surface.state}`);
  }
  return surface.value;
}

/**
 * The fixture's locked lava world (P14.T54.a): `airless`, locked 1:1, T_s 1,238.7 K, its day 1,561.58
 * K above its night, its dayside pool a quarter of its surface over a `basalt_reduced` lid.
 */
export function lavaWorldSurface(): BodySurface {
  return surfaceOf(lavaWorldDetail());
}

/** The fixture's Earth: `temperate`, 287.9 K, under its N₂–O₂ air. */
export function earthSurface(): BodySurface {
  return surfaceOf(earthDetail());
}

/**
 * The lava world as a magma ocean with no volatile air (`magma_ocean`, molten everywhere): its
 * night, 21 K by its record's contrast, held at the solidus.
 */
export function magmaOceanSurface(): BodySurface {
  const lava = lavaWorldSurface();
  if (lava.crust.kind !== "value") {
    throw new Error("the lava world's crust is modelled");
  }
  return {
    ...lava,
    state: "magma_ocean",
    crust: { kind: "value", value: { ...lava.crust.value, meltAreaFraction: 1 } },
  };
}

/**
 * An airless world 300 K in the mean, rotating, with the lava world's crust and none of its melt:
 * Earth-like contrasts, 40 K equator to pole and 10 K day to night.
 */
export function warmAirlessSurface(): BodySurface {
  const lava = lavaWorldSurface();
  if (lava.crust.kind !== "value") {
    throw new Error("the lava world's crust is modelled");
  }
  return {
    ...lava,
    meanSurfaceTemperatureK: 300,
    equatorPoleContrastK: 40,
    dayNightContrastK: 10,
    climateRegime: { ...lava.climateRegime, forcing: "seasonal" },
    crust: { kind: "value", value: { ...lava.crust.value, meltAreaFraction: 0 } },
  };
}
