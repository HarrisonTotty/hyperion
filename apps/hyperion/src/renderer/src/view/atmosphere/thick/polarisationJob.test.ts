/**
 * The polarisation correction as the optics worker answers it (plan R08, R08.T14.e): a
 * `polarisation-bake` request through `opticsJobs.ts`'s `OpticsJobs`, in process, on the generated
 * Earth's toy line tables, its column as `bakeJob.test.ts` forms it.
 */
import { describe, expect, it } from "vitest";

import { toyLoader } from "../../../test/opticsFakes";
import { absorberCurvesBySun } from "../absorbers";
import { OpticsJobs } from "../opticsJobs";
import type { OpticsRequest } from "../opticsMessages";
import type { ThickBakeBody } from "./bake";
import { bakePolarisationCorrection, preparePolarisation } from "./polarisation";
import { earthFixture, type EarthFixture } from "./polarisationFixtures";
import type { SpectralColumn } from "./spectralColumn";

/** A correction request of a column. */
function correctionRequest(
  fixture: EarthFixture,
  column: SpectralColumn = fixture.column,
  body: ThickBakeBody = fixture.body,
  bands: ReadonlyArray<number> = [0],
): OpticsRequest {
  return {
    kind: "polarisation-bake",
    id: 7,
    input: fixture.input,
    slots: [0],
    bands,
    column,
    body,
  };
}

// A band of the Earth's correction takes about 1 s (load 5, 2026-10-27), and the test bakes it
// twice: ten times the loaded figure, rounded up.
const JOB_TIMEOUT_MS = 60_000;

describe("the optics worker's polarisation correction", () => {
  it(
    "answers a correction with each band's table, the direct bake's, its buffers transferred",
    { timeout: JOB_TIMEOUT_MS },
    async () => {
      const fixture = await earthFixture();
      const jobs = new OpticsJobs(toyLoader());
      const answer = await jobs.answer(correctionRequest(fixture));
      const { reply } = answer;
      if (reply.kind !== "polarisation-baked") {
        throw new Error(`the correction was answered ${reply.kind}`);
      }
      expect(reply.tables.length).toBe(1);
      const values = reply.tables[0]?.slots[0]?.values;
      expect(answer.transfer.some((buffer) => buffer === values?.buffer)).toBe(true);
      // The same correction, made directly from the same spec and curves as the worker forms them.
      const [spec] = fixture.specs;
      if (spec === undefined) {
        throw new Error("the Earth has its absorber's spec");
      }
      const bins = absorberCurvesBySun(spec, [fixture.sun])[0]?.bins;
      if (bins === undefined) {
        throw new Error("the absorber has its bins' curves");
      }
      const direct = bakePolarisationCorrection(
        preparePolarisation(fixture.column, [{ spectrum: fixture.sun, absorbers: [bins] }]),
        fixture.body,
        0,
      );
      expect(values).toEqual(direct.slots[0]?.values);
      const copy = structuredClone(reply, { transfer: answer.transfer });
      expect([copy.kind, answer.transfer.every((buffer) => buffer.byteLength === 0)]).toEqual([
        "polarisation-baked",
        true,
      ]);
    },
  );

  it("answers a correction that fails with its own failure, never failed", async () => {
    const fixture = await earthFixture();
    const jobs = new OpticsJobs(toyLoader());
    const noBand = await jobs.answer(correctionRequest(fixture, fixture.column, fixture.body, [2]));
    expect(noBand.reply).toEqual({
      kind: "polarisation-bake-failed",
      id: 7,
      bands: [2],
      message: expect.stringMatching(/band/u),
    });
    const [molecular, absorber] = fixture.column.terms;
    if (molecular === undefined || absorber === undefined) {
      throw new Error("the column has its two terms");
    }
    const unknown: SpectralColumn = {
      ...fixture.column,
      terms: [molecular, { ...absorber, name: "absorber:not-on-the-body" }],
    };
    const noSpec = await jobs.answer(correctionRequest(fixture, unknown));
    expect(noSpec.reply).toEqual({
      kind: "polarisation-bake-failed",
      id: 7,
      bands: [0],
      message: expect.stringMatching(/absorber:not-on-the-body/u),
    });
  });
});
