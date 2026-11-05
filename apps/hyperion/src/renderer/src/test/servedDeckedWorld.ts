/**
 * A served Earth-like world with a water deck under partial cover (plan R08,
 * fix-r08-decked-air-opacity): `42002cae00000004.0300` of the universe of seed 0x4d2 at generator
 * version 28, its `body_detail` answer (`servedDeckedWorld.json`, as the server sent it), decoded as
 * the client decodes a frame. Its deck, H₂O of τ(550) 1.72 under a cloud fraction of 0.67, drew the
 * owner's "too opaque from space" (check-atmosphere-opacity). The record's universe ID is a fresh
 * universe's; its system is the fixture's own.
 */
import {
  decodeServerMessage,
  type RequestKind,
  type ResponseBody,
  type ResponseFor,
  type SystemIdHex,
} from "@hyperion/protocol";

import type { BodyDetail } from "../lib/system/model";
import { heldAppearanceOf } from "../view/appearance/fromWire";
import { loadKDistributions } from "../view/atmosphere/absorbers/kDistribution";
import { absorberCurveSlots, assembleMedium, lineSpeciesOf } from "../view/atmosphere/assemble";
import { phaseTableMoment } from "../view/atmosphere/forwardPeak";
import { type ReferenceCase, slotZero, sphereCase } from "../view/atmosphere/reference/cases";
import { RECORD_SIGNIFICANT_FIGURES } from "../view/atmosphere/reference/caseGate";
import type { SampleCounts } from "./atmosphereCases";
import { detailOf, recordInput, SUN_LIGHT } from "./atmosphereRecords";
import { inlinePart } from "./kDistributionFixtures";
import type { DensityProfile, MediumTerm } from "../view/atmosphere/medium";
import type { Rgb } from "../view/photometry/toneCurve";
import frame from "./servedDeckedWorld.json" with { type: "json" };

/** The served world's system. */
export const SERVED_DECKED_SYSTEM: SystemIdHex = "42002cae00000004";

/** The served world's body ID. */
export const SERVED_DECKED_BODY = `${SERVED_DECKED_SYSTEM}.0300`;

function isResponseOf<K extends RequestKind>(body: ResponseBody, kind: K): body is ResponseFor<K> {
  return body.kind === kind;
}

/**
 * The served world's record, decoded as a frame is.
 *
 * @throws Error where the fixture is not a `body_detail` response.
 */
export function servedDeckedDetail(): BodyDetail {
  const message = decodeServerMessage(JSON.stringify(frame));
  if (message.type !== "response" || !isResponseOf(message.body, "body_detail")) {
    throw new Error("servedDeckedWorld.json is not a body_detail response");
  }
  return detailOf(message.body, SERVED_DECKED_SYSTEM);
}

/**
 * The served world's ground as the atmosphere cache takes it: R07's p q per channel
 * (`AtmosphereCache.ts`'s `groundAlbedoOf`), 0.277 / 0.284 / 0.347.
 */
export function servedDeckedGroundAlbedo(): Rgb {
  const { geometricAlbedo: p, phaseIntegral: q } = heldAppearanceOf(
    servedDeckedDetail().record,
  ).photometry;
  return [p[0] * q[0], p[1] * q[1], p[2] * q[2]];
}

/**
 * The ground a gate case of the served world takes: `drawn`, 0.1 grey, about its drawn ground's
 * (its seabed and land read I/F 0.06–0.08 without air from 2,000 km, check-atmosphere-opacity), the
 * check lane's; or `client`, R07's p q, the atmosphere cache's ({@link servedDeckedGroundAlbedo}).
 */
export type ServedDeckedGround = "drawn" | "client";

/** Each case's name: the gate's, `drawn`, as its trace names it; `client`'s, untraced. */
export const SERVED_DECKED_CASES: Readonly<Record<ServedDeckedGround, string>> = {
  drawn: "served-decked-orbit",
  client: "served-decked-app-orbit",
};

/**
 * The gate case's orbit images and each one's samples, from a 4,096-sample pilot's σ to reach
 * T ÷ 8 with a third to spare, disc-p0's capped at a million: its trace's counts.
 */
export const SERVED_DECKED_COUNTS: SampleCounts = {
  geometries: {
    "disc-p0": 1_000_448,
    "disc-p30": 193_536,
    "disc-p60": 118_784,
    "disc-p75": 189_440,
    "disc-p85": 317_440,
    "limb-0.1H": 174_080,
    "limb-0.3H": 58_368,
    "limb-1H": 135_168,
  },
};

/**
 * The significant figures the cases' files write their terms to, as the thick cases' 9 do
 * (`reference/thickCases.ts`'s `THICK_CASE_SIGNIFICANT_FIGURES`): 7, the gate records' own
 * (`caseGate.ts`'s `RECORD_SIGNIFICANT_FIGURES`), so that the gate's case, whose absorbers' 1,026
 * levels it holds, stays under the added-file hook beside its trace (506 kB at 9).
 */
export const SERVED_DECKED_FIGURES = RECORD_SIGNIFICANT_FIGURES;

/**
 * The least spacing of a term's levels in the cases, m: 10 cm, ten of the centimetres the cases' 7
 * figures keep of heights under 100 km, as `jupiterEnvelopeCase.ts` holds its own apart. The
 * assembly gives SO₂'s profile two levels 4 × 10⁻¹² m apart at a step, which any rounding merges.
 */
export const SERVED_DECKED_LEAST_LEVEL_SPACING_M = 0.1;

/** `density` with each level at least {@link SERVED_DECKED_LEAST_LEVEL_SPACING_M} above the last. */
function heldApart(density: DensityProfile, topHeightM: number): DensityProfile {
  if (density.kind !== "tabulated") {
    return density;
  }
  const altitudesM = Float64Array.from(density.altitudesM);
  for (let i = 1; i < altitudesM.length; i += 1) {
    const least = (altitudesM[i - 1] ?? Number.NaN) + SERVED_DECKED_LEAST_LEVEL_SPACING_M;
    altitudesM[i] = Math.max(altitudesM[i] ?? Number.NaN, least);
  }
  if (
    !((altitudesM.at(-1) ?? Number.NaN) <= Math.max(topHeightM, density.altitudesM.at(-1) ?? 0))
  ) {
    throw new RangeError("a profile's levels held apart pass its medium's top");
  }
  return { ...density, altitudesM };
}

/** `term` with its density's levels held apart ({@link heldApart}). */
function termHeldApart(term: MediumTerm, topHeightM: number): MediumTerm {
  return { ...term, density: heldApart(term.density, topHeightM) };
}

/**
 * The served world's medium as the client draws it, under the reference Sun, over `ground`, as a
 * sphere case over the orbit images its gate judges ({@link SERVED_DECKED_COUNTS}), each tabulated
 * term's mean cosine its asymmetry, each term's levels held apart ({@link heldApart}). Both grounds'
 * cases take the seed 16, so that their traces' difference is the ground's.
 *
 * @throws Error where the record assembles no medium.
 */
export async function servedDeckedCase(ground: ServedDeckedGround): Promise<ReferenceCase> {
  const albedo: Rgb = ground === "client" ? servedDeckedGroundAlbedo() : [0.1, 0.1, 0.1];
  const input = recordInput(servedDeckedDetail(), [SUN_LIGHT], albedo);
  const tables = await loadKDistributions([...lineSpeciesOf(input)], inlinePart);
  const { medium, drawn } = assembleMedium(input, { kDistributions: tables });
  if (medium === undefined) {
    throw new Error(`${SERVED_DECKED_BODY} assembles no medium`);
  }
  const asymmetry: Record<string, Rgb> = {};
  for (const term of medium.terms) {
    if (term.phase.kind === "tabulated") {
      const { u, values } = term.phase.table;
      const mean = values.map(
        (v) => phaseTableMoment(u, v, (mu) => mu) / phaseTableMoment(u, v, () => 1),
      );
      asymmetry[term.name] = [mean[0] ?? Number.NaN, mean[1] ?? Number.NaN, mean[2] ?? Number.NaN];
    }
  }
  const whole = sphereCase(
    SERVED_DECKED_CASES[ground],
    { ...medium, terms: medium.terms.map((term) => termHeldApart(term, medium.topHeightM)) },
    drawn.sources,
    drawn.bottomRadiusM,
    slotZero(absorberCurveSlots(drawn)),
    asymmetry,
  );
  const traced = Object.keys(SERVED_DECKED_COUNTS.geometries ?? {});
  return {
    ...whole,
    geometries: whole.geometries.filter((g) => traced.includes(g.name)),
    aggregates: [],
    significantFigures: SERVED_DECKED_FIGURES,
  };
}
