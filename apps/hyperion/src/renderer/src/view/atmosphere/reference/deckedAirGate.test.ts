/**
 * The decked thin air's gate (plan R08, fix-r08-decked-air-opacity): the served Earth-like world
 * with a water deck under partial cover (`test/servedDeckedWorld.ts`), its medium as the client
 * draws it, through its route, the bake as a haze route bakes it (`thick/routing.ts`'s
 * `bakedAsHaze`), and through the analytic path beside it, the scalar twin against its trace
 * (`hyperion-fit atmosphere-reference` of the case at its own counts, 22 thread-h) over the disc
 * and the limb from orbit: the record `deckedAirGate.json`. Over two grounds: the check lane's 0.1
 * (`served-decked-orbit`; `routed`, `analytic`) and 0.3 (`served-decked-orbit-bright`; `bright`),
 * above the cloud-free mean of its drawn class map, 0.267 / 0.259 / 0.244, which the atmosphere
 * cache hands its bake (research-decked-ground). Every verdict is recorded, never asserted;
 * `thick/routing.ts`'s `DECKED_AIR_GATE_TALLIES` holds the route's over 0.1 and opens it where
 * they pass.
 *
 * @remarks
 * Each ground's case file, `<case>.case.json`, is the case the client writes now, held byte for
 * byte (`HYPERION_BLESS=1` writes it; then trace its reference from the repository's root with
 * `target/release/hyperion-fit atmosphere-reference <case> --out <reference> --threads 16 --work
 * target/atmosphere-work/<case>`), so that a change to the medium the client draws for the record
 * names the trace as stale. By hand (`HYPERION_DECKED_AIR_GATE=1 pnpm --filter hyperion
 * exec vitest run src/renderer/src/view/atmosphere/reference/deckedAirGate.test.ts`, with
 * `HYPERION_BLESS=1` to write the record) the case is baked and its twin's values computed on both
 * paths, about a minute.
 */
import { describe, expect, it } from "vitest";

import { caseJson } from "../../../test/atmosphereCases";
import { blessMode, expectCommittedTable } from "../../../test/bless";
import {
  SERVED_DECKED_CASES,
  SERVED_DECKED_COUNTS,
  servedDeckedCase,
} from "../../../test/servedDeckedWorld";
import { bakeKeptCase } from "../thick/keptBake";
import {
  caseGateSection,
  caseTwin,
  type GateCase,
  recordedRgb,
  twinValueKey,
  twinValueOf,
  type TwinValueSpec,
} from "./caseGate";
import { type ReferenceCase, referenceCaseBakeLights } from "./cases";
import committed from "./deckedAirGate.json" with { type: "json" };
import type { ReferenceFile } from "./references";
import brightCase from "./served-decked-orbit-bright.case.json" with { type: "json" };
import brightReference from "./served-decked-orbit-bright.reference.json" with { type: "json" };
import darkCase from "./served-decked-orbit.case.json" with { type: "json" };
import darkReference from "./served-decked-orbit.reference.json" with { type: "json" };
import { twinBakeBody } from "./twin";

/** The gate's grounds: the check lane's dark one, and the bright one over the drawn map's mean. */
const GROUNDS = ["dark", "bright"] as const;

/** A gate's ground. */
type GateGround = (typeof GROUNDS)[number];

/** Each ground's committed case and trace. */
const COMMITTED: Readonly<
  Record<GateGround, { readonly caseFile: unknown; readonly reference: ReferenceFile }>
> = {
  dark: { caseFile: darkCase, reference: darkReference },
  bright: { caseFile: brightCase, reference: brightReference },
};

/** The record's file, beside this test. */
const RECORD_FILE = "deckedAirGate.json";

/** Whether the record is formed: by hand, `HYPERION_DECKED_AIR_GATE=1`. */
function byHand(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_DECKED_AIR_GATE"] === "1";
}

const BY_HAND = byHand();

/** A ground's case file, beside this test, which its trace read. */
function caseFileOf(ground: GateGround): string {
  return `./${SERVED_DECKED_CASES[ground]}.case.json`;
}

/** A twin's radiance and drawn-surface share at each of the case's geometries, as recorded. */
function valuesOf(
  twin: ReturnType<typeof caseTwin>,
  referenceCase: ReferenceCase,
): Map<string, readonly [number, number, number]> {
  const specs: TwinValueSpec[] = referenceCase.geometries.flatMap((g) =>
    (["radiance", "surfaceShare"] as const).map((quantity): TwinValueSpec => ({
      quantity,
      setting: "case",
      caseName: referenceCase.name,
      name: g.name,
    })),
  );
  return new Map(
    specs.map((spec) => [twinValueKey(spec), recordedRgb(twinValueOf(twin, referenceCase, spec))]),
  );
}

/** The by-hand run's limit: a bake and both twins, about a minute on a pod, twenty times over. */
const BY_HAND_LIMIT_MS = 1_200_000;

/**
 * A ground's sections: the route's and the analytic path's gate against its trace, the case baked
 * as a haze route bakes it.
 */
async function sectionsOf(ground: GateGround): Promise<Record<string, unknown>> {
  const referenceCase = await servedDeckedCase(ground);
  const gateCase: GateCase = { referenceCase, reference: COMMITTED[ground].reference };
  const bands = bakeKeptCase(
    referenceCase,
    referenceCaseBakeLights(referenceCase),
    twinBakeBody(referenceCase.figure),
    { ownWavelengths: true, curvedStreams: true },
  );
  return {
    case: referenceCase.name,
    // A trace without --stokes holds its scalar radiance as its `radiance`, the `vector` reading.
    routed: caseGateSection(
      gateCase,
      valuesOf(caseTwin(referenceCase, { thick: bands }), referenceCase),
      "vector",
    ),
    analytic: caseGateSection(gateCase, valuesOf(caseTwin(referenceCase), referenceCase), "vector"),
  };
}

/** The record a by-hand run forms: the dark ground's sections, then the bright one's. */
async function deckedAirRecord(): Promise<unknown> {
  return {
    description:
      "fix-r08-decked-air-opacity's decked thin air gate: the served Earth-like world 42002cae00000004.0300 (generator version 28; test/servedDeckedWorld.ts) as the client draws it, through its route (its whole column baked as a haze route bakes it) and through the analytic path (Psi_ms, its lobes' surface light kept), the scalar twin against its trace over the disc and the limb from orbit, by image: over a ground of 0.1 (routed, analytic) and over one of 0.3, above the cloud-free mean of its drawn class map (bright). Verdicts recorded, never gated. Written by deckedAirGate.test.ts; do not edit by hand.",
    format: 1,
    ...(await sectionsOf("dark")),
    bright: await sectionsOf("bright"),
  };
}

describe("the decked thin air's gate (fix-r08-decked-air-opacity)", () => {
  it.each(GROUNDS)(
    "holds the %s ground's case file to the case the client draws for the served world now",
    async (ground) => {
      const referenceCase = await servedDeckedCase(ground);
      await expect(
        expectCommittedTable(
          caseJson(referenceCase, SERVED_DECKED_COUNTS),
          COMMITTED[ground].caseFile,
          caseFileOf(ground),
        ),
      ).resolves.toBeUndefined();
    },
    120_000,
  );

  it.each(GROUNDS)(
    "holds the %s ground's trace to its case: its name, seed and each geometry's samples",
    async (ground) => {
      const referenceCase = await servedDeckedCase(ground);
      const { reference } = COMMITTED[ground];
      expect([
        reference.case,
        reference.seed,
        reference.geometries.map((g) => [g.name, g.samples]),
      ]).toEqual([
        referenceCase.name,
        referenceCase.seed,
        referenceCase.geometries.map((g) => [g.name, SERVED_DECKED_COUNTS.geometries?.[g.name]]),
      ]);
    },
    120_000,
  );

  it.skipIf(!BY_HAND || blessMode() !== "compare")(
    "holds deckedAirGate.json by hand to the route's and the analytic path's tallies against the trace, by its values",
    { timeout: BY_HAND_LIMIT_MS },
    async () => {
      // Its file in Prettier's form, which the hook gives it, so its values alone are held.
      expect(JSON.parse(JSON.stringify(await deckedAirRecord()))).toEqual(committed);
    },
  );

  it.skipIf(!BY_HAND || blessMode() === "compare")(
    "writes deckedAirGate.json by hand under a bless",
    { timeout: BY_HAND_LIMIT_MS },
    async () => {
      await expect(
        expectCommittedTable(await deckedAirRecord(), committed, RECORD_FILE),
      ).resolves.toBeUndefined();
    },
  );
});
