/**
 * The decked thin air's gate (plan R08, fix-r08-decked-air-opacity): the served Earth-like world
 * with a water deck under partial cover (`test/servedDeckedWorld.ts`), its medium as the client
 * draws it over a ground of 0.1, about its drawn ground's, through its route, the bake as a haze
 * route bakes it (`thick/routing.ts`'s `bakedAsHaze`), and through the analytic path beside it, the
 * scalar twin against its trace (`served-decked-orbit.reference.json`, `hyperion-fit
 * atmosphere-reference` of the case at its own counts, 22 thread-h) over the disc and the limb from
 * orbit: the record `deckedAirGate.json` (`routed`, `analytic`). Every verdict is recorded, never
 * asserted; `thick/routing.ts`'s `DECKED_AIR_GATE_TALLIES` holds the route's and opens it where
 * they pass.
 *
 * @remarks
 * The case's file, `served-decked-orbit.case.json`, is the case the client writes now, held byte for
 * byte (`HYPERION_BLESS=1` writes it; then trace its reference from the repository's root with
 * `target/release/hyperion-fit atmosphere-reference <case> --out <reference> --threads 16 --work
 * target/atmosphere-work/served-decked-orbit`), so that a change to the medium the client draws for
 * the record names the trace as stale. By hand (`HYPERION_DECKED_AIR_GATE=1 pnpm --filter hyperion
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
import committedCase from "./served-decked-orbit.case.json" with { type: "json" };
import reference from "./served-decked-orbit.reference.json" with { type: "json" };
import { twinBakeBody } from "./twin";

/** The record's file, beside this test. */
const RECORD_FILE = "deckedAirGate.json";

/** Whether the record is formed: by hand, `HYPERION_DECKED_AIR_GATE=1`. */
function byHand(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_DECKED_AIR_GATE"] === "1";
}

const BY_HAND = byHand();

/** The case's file, beside this test, which its trace read. */
const CASE_FILE = `./${SERVED_DECKED_CASES.drawn}.case.json`;

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
 * The record a by-hand run forms: the route's and the analytic path's gate sections against the
 * trace, the case baked as a haze route bakes it.
 */
async function deckedAirRecord(): Promise<unknown> {
  const referenceCase = await servedDeckedCase("drawn");
  const gateCase: GateCase = { referenceCase, reference };
  const bands = bakeKeptCase(
    referenceCase,
    referenceCaseBakeLights(referenceCase),
    twinBakeBody(referenceCase.figure),
    { ownWavelengths: true, curvedStreams: true },
  );
  return {
    description:
      "fix-r08-decked-air-opacity's decked thin air gate: the served Earth-like world 42002cae00000004.0300 (generator version 28; test/servedDeckedWorld.ts) as the client draws it over a ground of 0.1, about its drawn ground's, through its route (its whole column baked as a haze route bakes it) and through the analytic path (Psi_ms, its lobes' surface light kept), the scalar twin against its trace over the disc and the limb from orbit, by image (routed, analytic). Verdicts recorded, never gated. Written by deckedAirGate.test.ts; do not edit by hand.",
    format: 1,
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

describe("the decked thin air's gate (fix-r08-decked-air-opacity)", () => {
  it("holds served-decked-orbit.case.json to the case the client draws for the served world now", async () => {
    const referenceCase = await servedDeckedCase("drawn");
    await expect(
      expectCommittedTable(caseJson(referenceCase, SERVED_DECKED_COUNTS), committedCase, CASE_FILE),
    ).resolves.toBeUndefined();
  }, 120_000);

  it("holds the trace to its case: its name, seed and each geometry's samples", async () => {
    const referenceCase = await servedDeckedCase("drawn");
    expect([
      reference.case,
      reference.seed,
      reference.geometries.map((g) => [g.name, g.samples]),
    ]).toEqual([
      referenceCase.name,
      referenceCase.seed,
      referenceCase.geometries.map((g) => [g.name, SERVED_DECKED_COUNTS.geometries?.[g.name]]),
    ]);
  }, 120_000);

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
