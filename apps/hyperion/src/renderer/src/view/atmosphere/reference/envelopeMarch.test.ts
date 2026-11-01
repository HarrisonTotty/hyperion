/**
 * The ray march onto an envelope's floor against its own converged march (plan R08, R08.T16.a.2;
 * `thick/deckRead.ts`, "An envelope's floor"), by hand under `HYPERION_SLOW_TESTS=1`: a check of
 * the steps' placement that reads no reference. `giant-deck` split at its envelope floor, the gate's
 * envelope variant (`vectorGate.json`'s `deck.asAnEnvelope`), is baked on the twin's body as the
 * gate bakes it, and its twin's disc and limb views from orbit are marched at high's steps and at
 * sixteen times them: R05.T12.e's quadrature gate (`marchSteps.ts`'s `MARCH_TOLERANCE`, 2% against
 * a converged march) holds the march stepped from the camera, and the floor's placement, R05.T12.e's
 * toward t*, misses it at the disc's edge, where the gate's three values missed.
 *
 * @remarks
 * About 55 s in all, the bake 9 s of it (pod, 16 vCPU, 2026-11-01).
 */
import { describe, expect, it } from "vitest";

import type { Rgb } from "../../photometry/toneCurve";
import { TABLE_SIZES, type TableSizes } from "../hillaire";
import { MARCH_TOLERANCE } from "../marchSteps";
import type { DeckTables, DeckSplit } from "../thick/deck";
import { bakeKeptDeck } from "../thick/keptBake";
import { envelopeFloorM } from "../thick/regime";
import { caseTwin, twinValueOf } from "./caseGate";
import { referenceCaseBakeLights } from "./cases";
import { thickReferenceCases } from "./thickCases";
import { twinBakeBody } from "./twin";
import committed from "./vectorGate.json" with { type: "json" };

/** Whether the check runs: by hand, `HYPERION_SLOW_TESTS=1`. */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

const SLOW = slowTests();

/** The limit, ms: some ten times its run on a pod. */
const LIMIT_MS = 1_200_000;

/** The converged march's steps, as a multiple of high's: within 0.2% of 64 times them on the case. */
const CONVERGED = 16;

/** The gate's views above the envelope's split: the disc at five suns and the limb at five heights. */
const VIEWS = [
  "disc-p0",
  "disc-p30",
  "disc-p60",
  "disc-p75",
  "disc-p85",
  "limb-0.1H",
  "limb-0.3H",
  "limb-1H",
  "limb-3H",
  "limb-10H",
] as const;

/** High's sizes with `k` times its ray march's steps and its counts by optical depth. */
function marchedAt(k: number): TableSizes {
  const high = TABLE_SIZES.high;
  const rule = high.marchDepthSteps;
  return {
    ...high,
    rayMarchSamples: high.rayMarchSamples * k,
    marchDepthSteps:
      rule === null
        ? null
        : {
            ...rule,
            groundFloor: rule.groundFloor * k,
            groundFloorPerEdge: rule.groundFloorPerEdge * k,
          },
  };
}

/** `band` over a surface's floor: the march stepped toward t*, as before R08.T16.a.2. */
function asSurfaces(band: DeckTables): DeckTables {
  return { ...band, floor: "ground" };
}

/** The largest |ΔL| ÷ L of `views` against `converged` over every view and channel. */
function worstOf(views: ReadonlyArray<Rgb>, converged: ReadonlyArray<Rgb>): number {
  let worst = 0;
  for (const [i, view] of views.entries()) {
    for (let c = 0; c < 3; c += 1) {
      const reference = converged[i]?.[c] ?? Number.NaN;
      worst = Math.max(worst, Math.abs((view[c] ?? Number.NaN) - reference) / reference);
    }
  }
  return worst;
}

describe("the ray march onto an envelope's floor (R08.T16.a.2), by hand", () => {
  it.skipIf(!SLOW)(
    "steps giant-deck's envelope views from the camera within R05.T12.e's 2% of their converged march, where the floor's placement misses the disc's edge",
    { timeout: LIMIT_MS },
    () => {
      const referenceCase = thickReferenceCases()["giant-deck"];
      const split: DeckSplit = {
        altitudeM: envelopeFloorM(referenceCase.medium),
        floor: "envelope",
      };
      expect(split.altitudeM).toBeCloseTo(committed.deck.asAnEnvelope.split.altitudeM, 6);
      const kept = bakeKeptDeck(
        referenceCase,
        referenceCaseBakeLights(referenceCase),
        twinBakeBody(referenceCase.figure),
        split,
      );
      const radiance = (tables: ReadonlyArray<DeckTables>, sizes: TableSizes): Rgb[] => {
        const twin = caseTwin(referenceCase, {
          deck: { tables, polarisation: kept.correction },
          sizes,
        });
        return VIEWS.map((name) =>
          twinValueOf(twin, referenceCase, {
            quantity: "radiance",
            setting: "case",
            caseName: referenceCase.name,
            name,
          }),
        );
      };
      const converged = radiance(kept.tables, marchedAt(CONVERGED));
      const fromCamera = radiance(kept.tables, TABLE_SIZES.high);
      const towardFloor = radiance(kept.tables.map(asSurfaces), TABLE_SIZES.high);
      expect(worstOf(fromCamera, converged)).toBeLessThan(MARCH_TOLERANCE);
      // The disc at the edge, p75 and p85, missed R05.T12.e's gate by 7–9% in red.
      expect(worstOf(towardFloor.slice(3, 5), converged.slice(3, 5))).toBeGreaterThan(
        3 * MARCH_TOLERANCE,
      );
    },
  );
});
