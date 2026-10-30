/**
 * R07's disc as R08 widens it (plan R08, R08.T18.a and R08.T16.b; decision-composition §1.9): the
 * ground's thermal emission on the disc of a body above the visibility cut, and the air of a body
 * drawn by its reflectance table beyond the 10⁹ m boundary.
 *
 * @remarks
 * A disc record of a body whose ground the cut draws carries a {@link DiscEmission}
 * (`DiscRecord.emission`): its temperature field (`thermal.ts`' `SurfaceTemperatureField`), its
 * brightest star's direction, about which the field's substellar term turns, and which of its
 * classes are `Melt`. Each sample of its disc then adds what its surface emits there to the light it
 * reflects: the field's temperature at the sample's datum normal ν, from μ = ν · ŝ and the
 * latitude's sine ν · p (p the disc's pole), its laws by their shares at the sample split into the
 * ground's and the melt's, each through `thermal.ts`' shared function, the melt at no less than the
 * melt's least temperature, times the frame's exposure scale ({@link discEmissionAt}). The emitted
 * light is no star's, so no eclipse, horizon or ring term cuts it, and it lights the night side; the
 * meter's class of a pixel stays the starlight's (`bodyDisc.wgsl`'s `lit`).
 *
 * The kernels that draw it are the plain kernels' emissive forms ({@link withDiscEmission}): the
 * same sources, `shade` adding `disc_emission` to each sample's light as its last step, with
 * `thermal.wgsl` and `discEmission.wgsl` appended. A record below the cut carries none and is
 * drawn by the plain kernels, which stay as they were, byte for byte (R08.T11.f found that a
 * branch never entered costs the disc kernels 0.05–0.7 ms on the RTX 3080 by its presence alone).
 * The emissive forms read the record's emission block past the records ({@link packDiscEmission},
 * {@link DISC_EMISSION_ROWS} rows, row 46's y its first; 0 for none).
 *
 * **The air** (R08.T16.b; Design note 13). A disc record of a body with an atmosphere drawn beyond
 * the 10⁹ m boundary (`draw.ts`' `planLitBodies`, with R07's hysteresis) carries a {@link DiscAir}:
 * its body's reflectance table (`discTable.ts`, baked by `thick/discBake.ts`). Its kernels are the
 * plain kernels' air forms ({@link withDiscAir}): `shade` hands its sample's light to `disc_air`
 * (`discAir.wgsl`), which dims it by the march's mean transmittance along the view and adds each
 * light's air, so that the disc draws what the full passes draw over the mesh inside the boundary,
 * its class map, normal map and eclipses its own; their block past the records is one row
 * ({@link packDiscAir}, row 46's z its first). The table binds at group 2's binding 15
 * (`draw.ts`' `DISC_AIR_TEXTURES`). A record carries emission or air, never both: a body with air
 * has no ground field (`thermal.ts`' `groundTemperatureFieldOf`), and the plan gives a tabled record
 * none.
 */

import { dot, type Vec3 } from "../../geometry/vec3";
import type { TextureHandle } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import type { DiscReflectanceTable } from "./discTable";
import type { BodyGravity } from "./oblate";
import discAirWgsl from "./shaders/discAir.wgsl?raw";
import discEmissionWgsl from "./shaders/discEmission.wgsl?raw";
import {
  aboveThermalCut,
  fieldTemperatureK,
  packTemperatureField,
  type SurfaceTemperatureField,
  THERMAL_WGSL,
  thermalLuminance,
} from "./thermal";

/** The ground's thermal emission on one disc (the module's remarks). */
export interface DiscEmission {
  readonly field: SurfaceTemperatureField;
  /**
   * The brightest star's unit direction from the body's centre, about which the field's substellar
   * term turns, in its holder's axes: the galactic axes on a disc record, the body-fixed ones in a
   * terrain's lighting (`TerrainLighting.emission`); zero where no star lights the body, which
   * leaves it the terminator's temperature.
   */
  readonly star: Vec3;
  /**
   * Whether each class of the disc's class map (law k + 1 for class k) is `Melt`, which emits at no
   * less than the field's `meltK`; empty for a uniform surface, whose one law is the ground's.
   */
  readonly meltClasses: ReadonlyArray<boolean>;
}

/** `vec4f` rows of a record's emission block (`discEmission.wgsl`'s `EMISSION_ROWS`). */
export const DISC_EMISSION_ROWS = 3;

/**
 * A disc's emission, or `null` for a body below the visibility cut (`thermal.ts`' `aboveThermalCut`),
 * which is drawn by the plain kernels.
 *
 * @param albedos - The A per render channel of each of the disc's laws, law 0 first.
 * @param illuminancesLx - Each star's illuminance face-on at the body per channel, lx.
 * @param star - The brightest star's unit direction from the body's centre, galactic; zero for none.
 * @param meltClasses - Which of the class map's classes are `Melt`; empty for a uniform surface.
 */
export function discEmissionOf(
  field: SurfaceTemperatureField,
  albedos: ReadonlyArray<Rgb>,
  illuminancesLx: ReadonlyArray<Rgb>,
  star: Vec3,
  meltClasses: ReadonlyArray<boolean>,
): DiscEmission | null {
  return aboveThermalCut(field, albedos, illuminancesLx) ? { field, star, meltClasses } : null;
}

/**
 * The classes that are `Melt` as one integer, bit k for class k, as `discEmission.wgsl` reads it
 * from an `f32`, exact for the 16 classes a disc holds at most.
 */
function meltMask(meltClasses: ReadonlyArray<boolean>): number {
  return meltClasses.reduce((mask, melt, k) => (melt ? mask + 2 ** k : mask), 0);
}

/**
 * A disc's emission block as `discEmission.wgsl` reads it, {@link DISC_EMISSION_ROWS} rows:
 * 0 the star's direction, w the frame's exposure scale; 1 the field's profile
 * (`packTemperatureField`); 2 its floor, its melt's least temperature (0 for none) and the melt
 * classes' mask, then 0.
 */
export function packDiscEmission(emission: DiscEmission, exposureScale: number): Float32Array {
  const { field, star } = emission;
  return Float32Array.from([
    star.x,
    star.y,
    star.z,
    exposureScale,
    ...packTemperatureField(field),
    field.floorK,
    field.meltK ?? 0,
    meltMask(emission.meltClasses),
    0,
  ]);
}

/**
 * The light a disc's sample emits, pre-exposed, by `disc_emission`'s arithmetic in `f64` (the
 * module's remarks): its laws by their shares split into the ground's and the melt's, each through
 * `thermal.ts`' shared function at the field's temperature there, the melt's held to its least.
 *
 * @param normal - The sample's datum normal, unit, in the axes `star` and `pole` are given in.
 * @param star - The brightest star's unit direction from the body's centre; zero for none.
 * @param pole - The disc's pole, unit.
 * @param shares - Each of the disc's laws' share at the sample, law 0 first.
 * @param albedos - Each law's A per render channel at the sample, as the disc shades it.
 */
export function discEmissionAt(
  emission: DiscEmission,
  exposureScale: number,
  normal: Vec3,
  star: Vec3,
  pole: Vec3,
  shares: ReadonlyArray<number>,
  albedos: ReadonlyArray<Rgb>,
): Rgb {
  const t = fieldTemperatureK(emission.field, dot(normal, star), dot(normal, pole));
  const ground = [0, 0, 0];
  const melt = [0, 0, 0];
  let groundShare = 0;
  let meltShare = 0;
  shares.forEach((share, m) => {
    const a = albedos[m];
    if (!(share > 0) || a === undefined) {
      return;
    }
    const isMelt = m > 0 && emission.meltClasses[m - 1] === true;
    const sum = isMelt ? melt : ground;
    for (const c of [0, 1, 2] as const) {
      sum[c] = (sum[c] ?? 0) + share * a[c];
    }
    if (isMelt) {
      meltShare += share;
    } else {
      groundShare += share;
    }
  });
  const out = [0, 0, 0];
  const add = (sum: ReadonlyArray<number>, share: number, temperatureK: number): void => {
    const emitted = thermalLuminance([sum[0] ?? 0, sum[1] ?? 0, sum[2] ?? 0], share, temperatureK);
    for (const c of [0, 1, 2] as const) {
      out[c] = (out[c] ?? 0) + emitted[c];
    }
  };
  if (groundShare > 0) {
    add(ground, groundShare, t);
  }
  if (meltShare > 0) {
    add(melt, meltShare, Math.max(t, emission.field.meltK ?? 0));
  }
  return [
    (out[0] ?? 0) * exposureScale,
    (out[1] ?? 0) * exposureScale,
    (out[2] ?? 0) * exposureScale,
  ];
}

/**
 * The line of `bodyDisc.wgsl`'s `shade` after which its emissive form adds the sample's emission:
 * the last of its planetshine loop, its closing brace and the function's return.
 */
const SHADE_END =
  "    out.radiance = out.radiance + source.xyz * misc.z * reflectance * transmitted;\n  }\n  return out;\n}";

/** {@link SHADE_END} with the emission added before the return. */
const EMISSIVE_SHADE_END =
  "    out.radiance = out.radiance + source.xyz * misc.z * reflectance * transmitted;\n  }\n  out.radiance = out.radiance + disc_emission(body, normal, surface, laws);\n  return out;\n}";

/**
 * A disc kernel's emissive form (the module's remarks): `source`, a composition of `bodyDisc.wgsl`,
 * with `shade` adding `disc_emission` to each sample's light as its last step, and the shared
 * function (`thermal.ts`' {@link THERMAL_WGSL}) and the disc's term (`discEmission.wgsl`)
 * appended. It declares no binding of its own: the block is read from the records' buffer.
 *
 * @throws Error unless `shade`'s end occurs exactly once in `source`.
 */
export function withDiscEmission(source: string): string {
  const parts = source.split(SHADE_END);
  if (parts.length !== 2) {
    throw new Error(
      `a disc kernel's emissive form needs bodyDisc.wgsl's shade once, found ${parts.length - 1}`,
    );
  }
  return `${parts.join(EMISSIVE_SHADE_END)}\n${THERMAL_WGSL}\n${discEmissionWgsl}`;
}

/** A disc's air beyond the boundary (the module's remarks): its body's table, as its draws bind it. */
export interface DiscAir {
  readonly table: DiscReflectanceTable;
  /** The table's texture (`AtmosphereTables.discReflectance`), at binding 15. */
  readonly texture: TextureHandle;
  /** ln s_b of the body's tables' bands, ascending, which the twin reads the bands by. */
  readonly lnBands: ReadonlyArray<number>;
  /**
   * The body's gravity, whose s at a point's latitude the bands are read at, as the body's table
   * (`oblate.wgsl`'s `gravityRatioAt`) gives it to the kernels; `null` for a body of one band at s
   * = 1 with no spheroid.
   */
  readonly gravity: BodyGravity | null;
}

/** `vec4f` rows of a record's air block (`discAir.wgsl`'s). */
export const DISC_AIR_ROWS = 1;

/** A disc's air block as `discAir.wgsl` reads it: its table's light slots, its sun's floor, 0, 0. */
export function packDiscAir(air: DiscAir): Float32Array {
  return Float32Array.of(air.table.slots, air.table.sunFloor, 0, 0);
}

/** {@link SHADE_END} with the sample's light handed to `disc_air` before the return. */
const AIR_SHADE_END =
  "    out.radiance = out.radiance + source.xyz * misc.z * reflectance * transmitted;\n  }\n  out.radiance = disc_air(body, q, ray, out.radiance);\n  return out;\n}";

/**
 * A disc kernel's air form (the module's remarks): `source`, a composition of `bodyDisc.wgsl`, with
 * `shade` handing each sample's light to `disc_air` as its last step, and the air's read
 * (`discAir.wgsl`) appended, which declares the table at group 2's binding 15.
 *
 * @throws Error unless `shade`'s end occurs exactly once in `source`.
 */
export function withDiscAir(source: string): string {
  const parts = source.split(SHADE_END);
  if (parts.length !== 2) {
    throw new Error(
      `a disc kernel's air form needs bodyDisc.wgsl's shade once, found ${parts.length - 1}`,
    );
  }
  return `${parts.join(AIR_SHADE_END)}\n${discAirWgsl}`;
}
