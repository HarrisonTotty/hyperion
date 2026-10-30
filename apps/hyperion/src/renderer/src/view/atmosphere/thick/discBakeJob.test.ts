/**
 * The disc's table as the optics worker answers it (plan R08, R08.T16.b): a `disc-bake` request
 * through `opticsJobs.ts`'s `OpticsJobs`, in process, for a kept Saturn-class body's first band.
 */
import { beforeAll, describe, expect, it } from "vitest";

import { toyLoader } from "../../../test/opticsFakes";
import { R06_SOLAR_BAKE_SPECTRUM } from "../earth";
import { tableRadiusM } from "../hillaire";
import { OpticsJobs } from "../opticsJobs";
import { keptBakeLightsOf, type OpticsRequest } from "../opticsMessages";
import {
  SATURN_OBLATE_GRAVITY,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_REFERENCE_SOURCES,
  SATURN_OBLATE_SPHEROID,
} from "../reference/cases";
import type { DiscReflectanceTable } from "../discTable";
import { kernelMedium } from "../tables";
import { thickBakeBands, type ThickBakeBody } from "./bake";
import { type DiscBakeAir, discReflectanceFor } from "./discBake";
import { bakeKeptCase } from "./keptBake";
import { spectralColumnOf } from "./spectralColumn";

const BODY: ThickBakeBody = {
  figure: SATURN_OBLATE_SPHEROID,
  gravity: SATURN_OBLATE_GRAVITY,
  bottomRadiusM: tableRadiusM(SATURN_OBLATE_SPHEROID),
};

const LIGHTS = keptBakeLightsOf(SATURN_OBLATE_REFERENCE, [R06_SOLAR_BAKE_SPECTRUM]);

const AIR: DiscBakeAir = {
  medium: SATURN_OBLATE_REFERENCE,
  kernelMedium: kernelMedium(SATURN_OBLATE_REFERENCE),
  curves: [],
};

/** A kept body's disc request of `bands`. */
function discRequest(bands: ReadonlyArray<number>, air: DiscBakeAir = AIR): OpticsRequest {
  return {
    kind: "disc-bake",
    id: 9,
    input: { kind: "kept", atmosphere: { kind: "unset" } },
    slots: [0],
    bands,
    column: spectralColumnOf(SATURN_OBLATE_REFERENCE, SATURN_OBLATE_REFERENCE_SOURCES),
    body: BODY,
    kept: LIGHTS,
    air,
  };
}

// A band's solve and its table take some 2 s on a pod's 16 vCPU (2026-10-30); the tests bake it
// three times: ten times that.
const JOB_TIMEOUT_MS = 60_000;

/** The worker's answers to a band's request, J baked and given, and the direct bake's table. */
interface Answered {
  readonly baked: Awaited<ReturnType<OpticsJobs["answer"]>>;
  readonly routed: Awaited<ReturnType<OpticsJobs["answer"]>>;
  readonly direct: DiscReflectanceTable;
}

/** Answers the band's request twice, J baked and the routed bake's given, beside the direct bake. */
async function answered(): Promise<Answered> {
  const jobs = new OpticsJobs(toyLoader());
  const baked = await jobs.answer(discRequest([0]));
  const solved = bakeKeptCase(
    { medium: SATURN_OBLATE_REFERENCE, sources: SATURN_OBLATE_REFERENCE_SOURCES },
    LIGHTS,
    BODY,
    { bands: [0] },
  );
  const band = thickBakeBands(BODY)[0];
  if (band === undefined) {
    throw new Error("the body has a band");
  }
  const routed = await jobs.answer(discRequest([0], { ...AIR, bake: solved }));
  return { baked, routed, direct: discReflectanceFor(AIR, [band], () => solved) };
}

/** An answer's table, or a test failure. */
function tableOf(answer: Awaited<ReturnType<OpticsJobs["answer"]>>): DiscReflectanceTable {
  if (answer.reply.kind !== "disc-baked") {
    throw new Error(`the disc's bake was answered ${answer.reply.kind}`);
  }
  return answer.reply.table;
}

describe("the optics worker's disc table", () => {
  let held: Answered | undefined;
  beforeAll(async () => {
    held = await answered();
  }, JOB_TIMEOUT_MS);
  const answers = (): Answered => {
    if (held === undefined) {
      throw new Error("the requests are answered before the tests");
    }
    return held;
  };

  it("answers a disc's table, its buffer transferred", () => {
    const { baked } = answers();
    const table = tableOf(baked);
    expect(baked.transfer.some((buffer) => buffer === table.values.buffer)).toBe(true);
  });

  it("answers the table the direct bake makes", () => {
    const { baked, direct } = answers();
    const table = tableOf(baked);
    expect([table.bands, table.slots, table.sunFloor, table.values]).toEqual([
      1,
      1,
      direct.sunFloor,
      direct.values,
    ]);
  });

  it("reads the routed bake's J where it is given, in place of solving: the same table", () => {
    const { routed, direct } = answers();
    expect(tableOf(routed).values).toEqual(direct.values);
  });

  it("answers a bake that fails with its own failure, never failed", async () => {
    const jobs = new OpticsJobs(toyLoader());
    const noBand = await jobs.answer(discRequest([7]));
    expect(noBand.reply).toEqual({
      kind: "disc-bake-failed",
      id: 9,
      message: expect.stringMatching(/band/u),
    });
  });
});
