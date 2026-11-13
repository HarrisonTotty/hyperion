/**
 * The sim's `planetary/envelope_profile` golden (P14.T24.d): the Solar System table's four giants', a
 * sub-Neptune's and a hand hot Jupiter's grey profiles and adiabats, and the sim's
 * `EnvelopeProfile::temperature_at` at stated pressures, which `column.ts`'s `envelope` profile is
 * held to bit for bit (R08.T16.a).
 *
 * @remarks
 * Read from the repository, as `verticalStructureGolden.ts` reads its golden, so that a re-blessed
 * golden leaves no copy behind here.
 */
import golden from "../../../../../../crates/hyperion-sim/tests/golden/planetary/envelope_profile.golden?raw";

import type { ProfileNode } from "../lib/system/model";
import type { EnvelopeProfile } from "../view/atmosphere/column";
import { goldenBodies, indexed } from "./verticalStructureGolden";

/** One body's envelope profile and the sim's temperatures at its levels. */
export interface GoldenEnvelope {
  readonly name: string;
  readonly profile: EnvelopeProfile;
  readonly photospherePa: number;
  /** `temperature_at` at each level's pressure, 0 Pa among them. */
  readonly levels: ReadonlyArray<ProfileNode>;
}

/** Jupiter's, Saturn's, Uranus's, Neptune's, the sub-Neptune's and the hot Jupiter's, in order. */
export const GOLDEN_ENVELOPES: ReadonlyArray<GoldenEnvelope> = goldenBodies(golden).map(
  ({ name, entries, scalar, nodes }) => ({
    name,
    profile: {
      kind: "envelope",
      grey: {
        thermalOpacityM2PerKg: scalar("thermal_opacity_m2_per_kg"),
        opticalDepthExponent: scalar("optical_depth_exponent"),
        opacityRatio: scalar("opacity_ratio"),
        redistributionFactor: scalar("redistribution"),
      },
      internalK: scalar("internal_temperature_k"),
      irradiationK: scalar("irradiation_temperature_k"),
      gravityMS2: scalar("gravity_m_s2"),
      nodes,
    },
    photospherePa: scalar("photosphere_pressure_pa"),
    levels: indexed(entries, "level"),
  }),
);

/** The golden's envelope of `name`. */
export function goldenEnvelope(
  name: "jupiter" | "saturn" | "uranus" | "neptune" | "sub_neptune" | "hot_jupiter",
): GoldenEnvelope {
  const envelope = GOLDEN_ENVELOPES.find((e) => e.name === name);
  if (envelope === undefined) {
    throw new Error(`the golden has no ${name}`);
  }
  return envelope;
}
