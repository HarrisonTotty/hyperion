import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { waterWorldAnswer, waterWorldHeader } from "../../test/oceanFixtures";
import { oceanLiquidOf } from "./liquids";
import {
  airViscosityPaS,
  oceanFarOpticsOf,
  type OceanFrameOptics,
  oceanSeaOf,
  seaAirOf,
  STANDARD_GRAVITY_MPS2,
} from "./oceanBody";
import { oceanLiquidOptics, oceanSeaConstants } from "./oceanFar";
import { type HeldOceanMap, oceanMapTextureSpec, packOceanTexels } from "./oceanMap";
import { oceanMaskOf } from "./oceanMask";
import { EARTH_SEA } from "./seaState";

const EARTH = "0200080020000000.0300";

/** A held map of a water world, its texture a counting engine's. */
async function heldWaterWorld(): Promise<HeldOceanMap> {
  const engine = await countingRenderEngine();
  const answer = waterWorldAnswer(4000, 64, 152);
  const mask = oceanMaskOf(answer);
  return {
    answer,
    mask,
    texels: packOceanTexels(answer, mask, { from: 0, to: 0, weight: 0 }),
    texture: engine.createTexture(oceanMapTextureSpec(`ocean map ${EARTH}`, answer.level)),
  };
}

const FRAME: OceanFrameOptics = {
  gravityMps2: STANDARD_GRAVITY_MPS2,
  pixelAngleRad: 2e-4,
  whitecaps: true,
  sunDirection: vec3(1, 0, 0),
  glintCapCdM2: 4e4,
  sunLux: [1.1e5, 1.2e5, 1.25e5],
  skyLux: [1.5e4, 2e4, 3e4],
};

describe("the air over a sea (R11.T8.b)", () => {
  it("is the ISA's sea-level air at its pressure and temperature, seaState's Earth air", () => {
    const air = seaAirOf({ surface_pressure_pa: 101_325, reference_temperature_k: 288.15 });
    expect(air.densityKgM3).toBeCloseTo(EARTH_SEA.air.densityKgM3, 3);
    expect(air.kinematicViscosityM2s / EARTH_SEA.air.kinematicViscosityM2s).toBeCloseTo(1, 3);
  });

  it("takes Sutherland's viscosity for air, 1.7894e-5 Pa s at 288.15 K and 1.716e-5 at 0 °C", () => {
    expect(airViscosityPaS(288.15)).toBeCloseTo(1.7894e-5, 8);
    expect(airViscosityPaS(273.15)).toBeCloseTo(1.716e-5, 8);
  });

  it("is Titan-like at 146.7 kPa and 94 K: near its 5.3 kg/m³, ν about 1.2e-6 m²/s", () => {
    const air = seaAirOf({ surface_pressure_pa: 146_700, reference_temperature_k: 94 });
    expect(air.densityKgM3).toBeGreaterThan(5.2);
    expect(air.densityKgM3).toBeLessThan(5.6);
    expect(air.kinematicViscosityM2s).toBeCloseTo(1.2e-6, 7);
  });

  it("has no density on an airless body, so that its sea never moves", () => {
    const air = seaAirOf({ surface_pressure_pa: 0, reference_temperature_k: 250 });
    expect(air.densityKgM3).toBe(0);
    expect(air.kinematicViscosityM2s).toBeGreaterThan(0);
    expect(oceanSeaConstants(1.6, EARTH_SEA.liquid, air).equivalentPerMps).toBe(0);
  });

  it("refuses a temperature that is not positive and a pressure that is negative", () => {
    expect(() => seaAirOf({ surface_pressure_pa: 1, reference_temperature_k: 0 })).toThrow(
      RangeError,
    );
    expect(() => seaAirOf({ surface_pressure_pa: -1, reference_temperature_k: 280 })).toThrow(
      RangeError,
    );
  });
});

describe("a body's sea from its header (R11.T8.b)", () => {
  it("takes the main liquid's record, the air, the datum and the sea level", async () => {
    const map = await heldWaterWorld();
    const header = waterWorldHeader(EARTH);
    const sea = oceanSeaOf(header, map, [0.2, 0.18, 0.15]);
    expect(sea).toMatchObject({
      body: EARTH,
      map,
      substance: "H2O",
      equatorialRadiusM: 6_378_137,
      polarRadiusM: 6_356_752.314245,
      seaLevelM: -120,
      bedAlbedo: [0.2, 0.18, 0.15],
    });
    const record = header.palette[1]?.liquid;
    expect(record).not.toBeNull();
    expect(sea?.liquid).toEqual(
      record === null || record === undefined ? null : oceanLiquidOf(record),
    );
    expect(sea?.air).toEqual(seaAirOf(header));
  });

  it("is none for a header that names no main liquid, or whose entry carries no record", async () => {
    const map = await heldWaterWorld();
    const header = waterWorldHeader(EARTH);
    expect(oceanSeaOf({ ...header, main_liquid: null }, map, null)).toBeNull();
    expect(oceanSeaOf({ ...header, main_liquid: 0 }, map, null)).toBeNull();
    expect(oceanSeaOf({ ...header, main_liquid: 7 }, map, null)).toBeNull();
  });
});

describe("a sea's far optics in a frame (R11.T8.b)", () => {
  it("raises the datum by the sea level and takes the frame's light, cap and setting", async () => {
    const sea = oceanSeaOf(waterWorldHeader(EARTH), await heldWaterWorld(), [0.2, 0.18, 0.15]);
    if (sea === null) {
      throw new Error("the water world has a sea");
    }
    const optics = oceanFarOpticsOf(sea, FRAME);
    expect(optics).toMatchObject({
      equatorialRadiusM: 6_378_137 - 120,
      polarRadiusM: 6_356_752.314245 - 120,
      pixelAngleRad: 2e-4,
      whitecaps: true,
      glintCapCdM2: 4e4,
      sunLux: FRAME.sunLux,
      skyLux: FRAME.skyLux,
      bedAlbedo: [0.2, 0.18, 0.15],
      ...oceanLiquidOptics(sea.liquid),
    });
    expect(optics.sea).toEqual(oceanSeaConstants(STANDARD_GRAVITY_MPS2, sea.liquid, sea.air));
  });

  it("takes the bed as deep water, R∞, until a class map names a Seabed", async () => {
    const sea = oceanSeaOf(waterWorldHeader(EARTH), await heldWaterWorld(), null);
    if (sea === null) {
      throw new Error("the water world has a sea");
    }
    const optics = oceanFarOpticsOf(sea, FRAME);
    expect(optics.bedAlbedo).toEqual(optics.deepReflectance);
  });

  it("is the identity of Earth's sea state at Earth's gravity, liquid aside", async () => {
    const sea = oceanSeaOf(waterWorldHeader(EARTH), await heldWaterWorld(), null);
    if (sea === null) {
      throw new Error("the water world has a sea");
    }
    // The ISA's air and standard gravity: U_eq differs from U by the liquid's own tension and
    // density alone.
    const own = oceanFarOpticsOf(sea, FRAME).sea;
    const earth = oceanSeaConstants(STANDARD_GRAVITY_MPS2, sea.liquid, EARTH_SEA.air);
    expect(own.equivalentPerMps / earth.equivalentPerMps).toBeCloseTo(1, 3);
  });
});
