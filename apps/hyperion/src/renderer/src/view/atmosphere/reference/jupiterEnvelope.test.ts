/**
 * R08.T16.a.2's absorbing envelope case (plan R08): the committed `jupiter-envelope.case.json` is
 * the case `test/jupiterEnvelopeCase.ts` writes now from the hand Jupiter's medium, through
 * `bless.ts`; it stands under the disc and the limb from orbit, above its split at the hand
 * Jupiter's envelope floor, whose medium above absorbs in every channel.
 *
 * @remarks
 * Rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/jupiterEnvelope.test.ts`, and its reference traced
 * afresh from the repository's root by `test/atmosphereCases.ts`'s `traceCommand`.
 */

import { describe, expect, it } from "vitest";

import {
  ADDED_FILE_LIMIT_BYTES,
  caseJson,
  type SampleCounts,
  traceCommand,
} from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import { JUPITER_ENVELOPE_CASE, jupiterEnvelopeCase } from "../../../test/jupiterEnvelopeCase";
import { upperMediumOf } from "../thick/deckRead";
import { regimeChannels } from "../thick/regime";
import committed from "./jupiter-envelope.case.json" with { type: "json" };
import sampleCounts from "./sampleCounts.json" with { type: "json" };
import { angleBetweenDeg, LEAST_SUN_DISTANCE_DEG } from "./geometries";

/** Each case's own counts, from its pilot (`decision-r08-t12b-scope.md`, ruling 8). */
const COUNTS: Readonly<Record<string, SampleCounts>> = sampleCounts;

/** The texts of the committed case's files, for their sizes. */
const FILES = import.meta.glob<string>("./jupiter-envelope.*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** The hand Jupiter's assembly and its modes' Mie, ms. */
const BUILD_MS = 300_000;

describe("the absorbing envelope's case", { timeout: BUILD_MS }, () => {
  it("jupiter-envelope.case.json is the case the client's optics write now", async () => {
    const { referenceCase } = jupiterEnvelopeCase();
    try {
      await expect(
        expectCommittedTable(
          caseJson(referenceCase, COUNTS[JUPITER_ENVELOPE_CASE]),
          committed,
          `./${JUPITER_ENVELOPE_CASE}.case.json`,
        ),
      ).resolves.toBeUndefined();
    } catch (error: unknown) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(
        `${reason}; then trace its reference: \`${traceCommand(JUPITER_ENVELOPE_CASE)}\``,
        {
          cause: error,
        },
      );
    }
  });

  it("views the disc and the limb from orbit alone, above its split and its top, none within 3° of its sun", () => {
    const { referenceCase, split } = jupiterEnvelopeCase();
    const { geometries, medium } = referenceCase;
    const nearest = Math.min(...geometries.map((g) => angleBetweenDeg(g.view, g.sun)));
    expect([
      geometries.map((g) => g.image),
      geometries.every((g) => g.observer.heightM > Math.max(split.altitudeM, medium.topHeightM)),
      referenceCase.aggregates,
    ]).toEqual([
      ["disc-p0", "disc-p30", "disc-p60", "disc-p75", "disc-p85", ...Array<string>(4).fill("limb")],
      true,
      [],
    ]);
    expect(nearest).toBeGreaterThan(LEAST_SUN_DISTANCE_DEG - 1e-9);
  });

  // At 27's chromophore (its disc column, τ(550) 0.117 from 0.307 at 26) the medium above absorbs
  // in green and blue alone, red's τ (1 − ω̄) 0.025 (0.065 at 26); the reference was traced at 26's.
  it("splits at the hand Jupiter's envelope floor, its medium above absorbing in green and blue at 27", () => {
    const { referenceCase, split } = jupiterEnvelopeCase();
    const above = regimeChannels(upperMediumOf(referenceCase.medium, split.altitudeM));
    expect([
      split.floor,
      referenceCase.medium.groundAlbedo,
      above.map((c) => c.opticalDepth * (1 - c.singleScatteringAlbedo) > 0.05),
    ]).toEqual(["envelope", [0, 0, 0], [false, true, true]]);
  });

  it("names one of its geometries by each of its counts", () => {
    const { referenceCase } = jupiterEnvelopeCase();
    const names = new Set(referenceCase.geometries.map((g) => g.name));
    const counts = COUNTS[JUPITER_ENVELOPE_CASE];
    const strays = [
      ...Object.keys(counts?.geometries ?? {}),
      ...Object.keys(counts?.aggregates ?? {}),
    ].filter((key) => !names.has(key));
    expect(strays).toEqual([]);
  });

  it.each(Object.keys(FILES))("keeps %s under the added-file hook", (file) => {
    expect(new TextEncoder().encode(FILES[file]).length).toBeLessThan(ADDED_FILE_LIMIT_BYTES);
  });
});
