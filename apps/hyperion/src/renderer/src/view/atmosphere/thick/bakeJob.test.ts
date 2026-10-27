/**
 * The thick bake as the optics worker answers it (plan R08, R08.T14.b): a `thick-bake` request
 * through `opticsJobs.ts`'s `OpticsJobs`, in process, on the generated Earth's toy line tables.
 */
import { describe, expect, it } from "vitest";

import { detailOf, recordInput } from "../../../test/atmosphereRecords";
import { toyLoader } from "../../../test/opticsFakes";
import { earthDetail } from "../../../test/planetaryFixture";
import { absorberCurvesBySun } from "../absorbers";
import type { AssembledMedium } from "../assemble";
import type { BodyAtmosphereInput } from "../atmosphereInput";
import { CHANNEL_WAVELENGTHS_NM, type MediumTerm } from "../medium";
import { OpticsJobs } from "../opticsJobs";
import type { OpticsRequest } from "../opticsMessages";
import { molecularMixture } from "../rayleigh";
import { bakeMultipleScattering, prepareBake, type ThickBakeBody } from "./bake";
import type { SpectralColumn } from "./spectralColumn";

/** The generated Earth under the Sun, its water's and CO₂'s lines on the toy tables. */
function earthInput(): BodyAtmosphereInput {
  return recordInput(detailOf(earthDetail()));
}

/** The Earth's assembly, as a worker answers it. */
async function assembled(jobs: OpticsJobs, input: BodyAtmosphereInput): Promise<AssembledMedium> {
  const { reply } = await jobs.answer({ kind: "assemble", id: 1, input });
  if (reply.kind !== "assembled") {
    throw new Error(`the Earth was answered ${reply.kind}`);
  }
  return reply.assembled;
}

/**
 * A column of the Earth's molecular term and its first lined absorber, by their sources: the
 * record's gases with n_s read from the term's green coefficient, and the absorber's amount.
 */
function columnOf(
  input: BodyAtmosphereInput,
  assembly: AssembledMedium,
): {
  readonly column: SpectralColumn;
  readonly body: ThickBakeBody;
  readonly absorber: MediumTerm;
} {
  if (assembly.medium === undefined || input.kind !== "record") {
    throw new Error("the Earth draws a medium");
  }
  const surface = input.detail.record.surface;
  if (surface.state !== "ok") {
    throw new Error("the Earth has a surface");
  }
  const { medium, drawn } = assembly;
  const rayleigh = medium.terms.find((t) => t.name === "rayleigh");
  const lined = drawn.absorbers.find(
    (spec) => spec.kind === "column" && spec.species.some((s) => s.lines !== undefined),
  );
  const absorber = medium.terms.find((t) => t.name === lined?.name);
  if (rayleigh === undefined || absorber?.absorber?.kind !== "column") {
    throw new Error("the Earth has a molecular term and a lined absorber");
  }
  const gases = surface.value.gases;
  return {
    column: {
      name: "the Earth's air and one absorber",
      topHeightM: medium.topHeightM,
      groundAlbedo: medium.groundAlbedo,
      terms: [
        {
          name: rayleigh.name,
          density: rayleigh.density,
          source: {
            kind: "molecular",
            fractions: gases,
            numberDensityPerM3:
              rayleigh.scattering[1] /
              molecularMixture(gases).crossSectionM2(CHANNEL_WAVELENGTHS_NM[1]),
          },
        },
        {
          name: absorber.name,
          density: absorber.density,
          source: {
            kind: "absorber",
            amount: { kind: "column", numberDensityPerM3: absorber.absorber.numberDensityPerM3 },
          },
        },
      ],
      ...(medium.pressureBar === undefined ? {} : { pressureBar: medium.pressureBar }),
    },
    body: { ...drawn.body, bottomRadiusM: drawn.bottomRadiusM },
    absorber,
  };
}

/** A bake request of the Earth's column. */
function bakeRequest(
  input: BodyAtmosphereInput,
  column: SpectralColumn,
  body: ThickBakeBody,
  bands: ReadonlyArray<number> = [0],
): OpticsRequest {
  return { kind: "thick-bake", id: 5, input, slots: [0], bands, column, body };
}

describe("the optics worker's thick bake", () => {
  it(
    "answers a bake with each band's table, the bake's own, its buffers transferred",
    { timeout: 120_000 },
    async () => {
      const jobs = new OpticsJobs(toyLoader());
      const input = earthInput();
      const assembly = await assembled(jobs, input);
      const { column, body, absorber } = columnOf(input, assembly);
      const answer = await jobs.answer(bakeRequest(input, column, body));
      const { reply } = answer;
      if (reply.kind !== "thick-baked") {
        throw new Error(`the bake was answered ${reply.kind}`);
      }
      expect(reply.tables.length).toBe(1);
      const values = reply.tables[0]?.slots[0]?.values;
      expect(answer.transfer.some((buffer) => buffer === values?.buffer)).toBe(true);
      // The same bake, made directly from the same specs and curves.
      const spec = assembly.drawn?.absorbers.find((each) => each.name === absorber.name);
      const sun = input.kind === "record" ? input.lights[0]?.spectrum : undefined;
      if (spec === undefined || sun === undefined) {
        throw new Error("the Earth has the absorber's spec and a sun");
      }
      const bins = absorberCurvesBySun(spec, [sun])[0]?.bins;
      if (bins === undefined) {
        throw new Error("the absorber has its bins' curves");
      }
      const direct = bakeMultipleScattering(
        prepareBake(column, [{ spectrum: sun, absorbers: [bins] }]),
        body,
        0,
      );
      expect(values).toEqual(direct.slots[0]?.values);
      const copy = structuredClone(reply, { transfer: answer.transfer });
      expect([copy.kind, answer.transfer.every((buffer) => buffer.byteLength === 0)]).toEqual([
        "thick-baked",
        true,
      ]);
    },
  );

  it("answers a bake that fails with its own failure, never failed", async () => {
    const jobs = new OpticsJobs(toyLoader());
    const input = earthInput();
    const { column, body } = columnOf(input, await assembled(jobs, input));
    const noBand = await jobs.answer(bakeRequest(input, column, body, [3]));
    expect(noBand.reply).toEqual({
      kind: "thick-bake-failed",
      id: 5,
      bands: [3],
      message: expect.stringMatching(/band/u),
    });
    const [molecular, absorber] = column.terms;
    if (molecular === undefined || absorber === undefined) {
      throw new Error("the column has its two terms");
    }
    const renamed = {
      name: "absorber:not-on-the-body",
      density: absorber.density,
      source: absorber.source,
    };
    const unknown: SpectralColumn = { ...column, terms: [molecular, renamed] };
    const noSpec = await jobs.answer(bakeRequest(input, unknown, body));
    expect(noSpec.reply).toEqual({
      kind: "thick-bake-failed",
      id: 5,
      bands: [0],
      message: expect.stringMatching(/absorber:not-on-the-body/u),
    });
  });

  it(
    "leaves the other kinds' answers as they were: a worker that baked assembles and shares as a fresh one",
    { timeout: 120_000 },
    async () => {
      const input = earthInput();
      const share: OpticsRequest = {
        kind: "curve-sums",
        id: 2,
        input,
        slots: [0],
        share: { index: 0, count: 2 },
      };
      const fresh = new OpticsJobs(toyLoader());
      const expected = [
        (await fresh.answer({ kind: "assemble", id: 1, input })).reply,
        (await fresh.answer(share)).reply,
      ];
      const baker = new OpticsJobs(toyLoader());
      const { column, body } = columnOf(input, await assembled(new OpticsJobs(toyLoader()), input));
      expect((await baker.answer(bakeRequest(input, column, body))).reply.kind).toBe("thick-baked");
      expect([
        (await baker.answer({ kind: "assemble", id: 1, input })).reply,
        (await baker.answer(share)).reply,
      ]).toEqual(expected);
    },
  );
});
