/**
 * R07.T8.a's six air bodies' fixtures (plan R07, R07.T8.a; plan R08, R08.T16.b), by hand:
 * `HYPERION_AIR_SWITCH=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/bodies/airSwitchCases.test.ts`, with `HYPERION_BLESS=1` to write them
 * (`test/airSwitchCases.ts`). Each case's disc table is baked anew and held to its committed
 * fixture byte for byte, as `bless.ts` holds a table; the 3 px switch's tests read the fixtures.
 */
import { describe, expect, it } from "vitest";

import {
  AIR_SWITCH_CASES,
  AIR_SWITCH_FIXTURES,
  airSwitchFixtureOf,
  bakedAirSwitchFixture,
} from "../../test/airSwitchCases";
import { expectCommittedTable } from "../../test/bless";

/** Whether the bakes are made: by hand, `HYPERION_AIR_SWITCH=1`. */
const RUN = ((): boolean => {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_AIR_SWITCH"] === "1";
})();

/** A case's bake at most, ms: `TEST VENUS`'s Mie and whole-column bake some 2 minutes. */
const BAKE_MS = 900_000;

describe.skipIf(!RUN)("the air switch's fixtures (by hand)", () => {
  for (const name of AIR_SWITCH_CASES) {
    it(`bakes ${name}'s disc table and holds its fixture to it`, { timeout: BAKE_MS }, async () => {
      const { json, file } = AIR_SWITCH_FIXTURES[name];
      const fixture = await bakedAirSwitchFixture(name, airSwitchFixtureOf(json));
      await expect(expectCommittedTable(fixture, json, file)).resolves.toBeUndefined();
    });
  }
});
