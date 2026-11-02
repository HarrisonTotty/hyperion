/**
 * The hand Jupiter's disc table over its envelope's deck (plan R08, R08.T16.b.2), by hand under
 * `HYPERION_SLOW_TESTS=1`: its upper medium on its floor's level surface, J and the deck's table
 * T15's split bakes at the floor, against the routed full passes' twins from 10⁹ m.
 *
 * @remarks
 * The deck's preparation and its three bands' bakes take about 80 s on a pod, and the table's and
 * the twins' as long again. The product's read path stays shut while the hand Jupiter lies outside
 * the envelope's route (`thick/routing.ts`'s `DECK_ROUTE_RANGE`): this is the table it reads once
 * a gate covering it passes.
 */
import { beforeAll, describe, expect, it } from "vitest";

import { detailOf, recordInput } from "../../../test/atmosphereRecords";
import {
  bakedDiscAir,
  type BakedDiscAir,
  type DiscAirCase,
  fullPassTwins,
  type FullPassTwins,
  type IntegratedRadiance,
  integratedRadianceAt,
} from "../../../test/discAirCases";
import { handJupiterDetail } from "../../../test/giantFixtures";
import { absorberCurveSlots, assembleMedium } from "../assemble";
import { DISC_LAYER_BYTES, discTableBytes, discTableHalves } from "../discTable";
import { thickBakeBands, thickBakeLights } from "./bake";
import { bakeDeck, prepareDeck } from "./deck";
import { deckDatumOf, upperMediumOf } from "./deckRead";
import { spectralColumnOf } from "./spectralColumn";

/** Whether the checks run: by hand, `HYPERION_SLOW_TESTS=1`. */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

const SLOW = slowTests();

/** The limit of each, ms: some ten times its run on a pod. */
const LIMIT_MS = 1_800_000;

/** The routed passes' converged march: sixteen times high's samples, on 60 radii and 16 azimuths. */
const CONVERGED_COUNTS = { marchScale: 16, radii: 60, azimuths: 16 } as const;

/** The hand Jupiter's upper medium over its deck, as a disc air case, its deck's bands baked. */
function jupiterOverItsDeck(): DiscAirCase {
  const assembled = assembleMedium(recordInput(detailOf(handJupiterDetail())), { envelopes: true });
  const { medium, drawn } = assembled;
  if (medium === undefined || drawn.envelope === undefined) {
    throw new Error("the hand Jupiter draws an envelope's medium");
  }
  const floorM = drawn.envelope.floorAltitudeM;
  const column = spectralColumnOf(medium, drawn.sources);
  const lights = thickBakeLights(column, drawn.absorbers, drawn.spectra);
  const prepared = prepareDeck(column, lights, { altitudeM: floorM, floor: "envelope" });
  const body = {
    figure: drawn.body.figure,
    gravity: drawn.body.gravity,
    bottomRadiusM: drawn.bottomRadiusM,
  };
  const deck = thickBakeBands(body).map((_, band) => bakeDeck(prepared, body, band));
  return {
    name: "the hand Jupiter over its deck",
    medium: upperMediumOf(medium, floorM),
    sources: drawn.sources,
    curves: absorberCurveSlots(drawn),
    body: {
      figure: deckDatumOf(drawn.body.figure, drawn.body.gravity, floorM),
      gravity: drawn.body.gravity,
    },
    deck,
  };
}

describe("the hand Jupiter's disc table over its deck (R08.T16.b.2, by hand)", () => {
  let jupiter: DiscAirCase | undefined;
  let baked: BakedDiscAir | undefined;
  let twins: FullPassTwins | undefined;
  beforeAll(() => {
    if (!SLOW) {
      return;
    }
    jupiter = jupiterOverItsDeck();
    baked = bakedDiscAir(jupiter);
    twins = fullPassTwins(jupiter, baked);
  }, LIMIT_MS);
  const built = (): {
    readonly jupiter: DiscAirCase;
    readonly baked: BakedDiscAir;
    readonly twins: FullPassTwins;
  } => {
    if (jupiter === undefined || baked === undefined || twins === undefined) {
      throw new Error("the hand Jupiter's table is baked before the tests");
    }
    return { jupiter, baked, twins };
  };

  it.skipIf(!SLOW)(
    "holds a layer a band and slot, T16.b.1's 53,248 B, its half floats the same from a second bake",
    () => {
      const { jupiter: item, baked: first } = built();
      const second = bakedDiscAir(item);
      expect([
        first.table.bands,
        first.table.slots,
        discTableBytes(first.table),
        discTableHalves(first.table),
      ]).toEqual([3, 1, 3 * DISC_LAYER_BYTES, discTableHalves(second.table)]);
    },
    LIMIT_MS,
  );

  /** The routed passes' march at sixteen times high's samples, on 60 radii and 16 azimuths. */
  const convergedAt = new Map<number, IntegratedRadiance>();
  const converged = (phaseDeg: number): IntegratedRadiance => {
    const { jupiter: item, baked: table, twins: passes } = built();
    let at = convergedAt.get(phaseDeg);
    if (at === undefined) {
      // Sixteen: within 0.2% of thirty-two times the samples in every channel at both phases.
      at = integratedRadianceAt(item, table, passes, phaseDeg, CONVERGED_COUNTS);
      convergedAt.set(phaseDeg, at);
    }
    return at;
  };

  for (const phaseDeg of [0, 90]) {
    it.skipIf(!SLOW)(
      `agrees with the routed march to 5% from 10⁹ m at a phase of ${phaseDeg}°, converged at sixteen times the high setting's samples; the product's own counts as measured`,
      () => {
        const { jupiter: item, baked: table, twins: passes } = built();
        const marched = converged(phaseDeg);
        const product = integratedRadianceAt(item, table, passes, phaseDeg);
        for (const c of [0, 1, 2] as const) {
          expect(Math.abs(marched.disc[c] / marched.routed[c] - 1)).toBeLessThan(0.05);
          // The step a routed giant would show at the switch, pinned as measured since the march
          // shares a cut side's steps by the light each piece returns (R08.T16.a.2): −1.8% to
          // +1.0%, red at 90° the most (−2.4% to +29.6% before; plan R08's records of T16.b.2).
          const step = product.disc[c] / product.routed[c] - 1;
          expect(step).toBeGreaterThan(-0.05);
          expect(step).toBeLessThan(0.02);
        }
      },
      LIMIT_MS,
    );

    it.skipIf(!SLOW)(
      `marches its routed passes at the high setting's 36 samples within 5% of sixteen times them from 10⁹ m at a phase of ${phaseDeg}°, in every channel (R08.T16.a.2)`,
      () => {
        const { jupiter: item, baked: table, twins: passes } = built();
        // On the converged march's own radii and azimuths. On R08.T6.i's shares by length its red
        // read 12% (0°) and 24% (90°) dark; it reads within 1.0% in every channel at both.
        const marched = integratedRadianceAt(item, table, passes, phaseDeg, {
          radii: CONVERGED_COUNTS.radii,
          azimuths: CONVERGED_COUNTS.azimuths,
        });
        const reference = converged(phaseDeg);
        for (const c of [0, 1, 2] as const) {
          expect(Math.abs(marched.routed[c] / reference.routed[c] - 1)).toBeLessThan(0.05);
        }
      },
      LIMIT_MS,
    );
  }
});
