/**
 * R08.T5.e's geometric albedo on the plane-parallel Venus deck (plan R08): the committed
 * `venus-92bar-albedo.case.json` is the case `test/venusAlbedo.ts` writes now on the committed
 * `acid_uv_absorber`, through `bless.ts`; its quadrature gives a Lambert plane 2A ÷ 3; and its
 * geometries look along each node's μ with the sun behind.
 *
 * @remarks
 * Rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/venusAlbedo.test.ts`, and its reference traced afresh
 * by `cargo run --release -p hyperion-fit -- atmosphere-reference <case> --out <reference> --samples
 * 2048 --stokes --threads 4`; `hyperion-fit`'s `venus_uv_absorber` tests hold that reference to
 * Venus's albedos. The deck's Mie at six wavelengths runs for about a minute and a half.
 */

import { describe, expect, it } from "vitest";

import { ADDED_FILE_LIMIT_BYTES } from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import {
  ALBEDO_BANDS,
  ALBEDO_NODES,
  ALBEDO_WAVELENGTHS_NM,
  albedoCaseJson,
  geometricAlbedo,
  readAlbedoReference,
} from "../../../test/venusAlbedo";
import committed from "./venus-92bar-albedo.case.json" with { type: "json" };
import { seedOf } from "./cases";
import { angleBetweenDeg } from "./geometries";

/** The case's name and seed: `venus-92bar`'s own, its file beside the case's. */
const NAME = "venus-92bar-albedo";

/** The texts of the committed albedo files, for their sizes. */
const FILES = import.meta.glob<string>("./venus-92bar-albedo.*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

describe("the albedo's quadrature", () => {
  it("has eight nodes in μ, ascending, inside (0, 1)", () => {
    const mus = ALBEDO_NODES.map((n) => n.mu);
    expect([mus.length, mus.toSorted((a, b) => a - b), mus[0] ?? 0, mus.at(-1) ?? 1]).toEqual([
      8,
      mus,
      expect.closeTo(0.0199, 4),
      expect.closeTo(0.9801, 4),
    ]);
  });

  it("gives a Lambert plane two thirds of its albedo", () => {
    const albedo = 0.37;
    const reference = readAlbedoReference({
      wavelengthsNm: [365],
      geometries: ALBEDO_NODES.map((node, k) => ({
        name: `albedo-n${k}`,
        // R = π I ÷ F = A μ for a Lambert plane, the sun of irradiance π.
        radiance: [albedo * node.mu],
        standardError: [0],
      })),
    });
    expect(geometricAlbedo(reference, 365)).toEqual({
      albedo: expect.closeTo((2 * albedo) / 3, 14),
      standardError: 0,
    });
  });

  it("refuses a reference without one of its nodes", () => {
    const reference = readAlbedoReference({ wavelengthsNm: [365], geometries: [] });
    expect(() => geometricAlbedo(reference, 365)).toThrow(/node 0/u);
  });
});

describe("the albedo case", { timeout: 300_000 }, () => {
  const written = albedoCaseJson({ name: NAME, seed: seedOf("venus-92bar") });

  it("venus-92bar-albedo.case.json is the case the client's optics write now", async () => {
    await expect(
      expectCommittedTable(written, committed, `./${NAME}.case.json`),
    ).resolves.toBeUndefined();
  });

  it("traces every band, the sun of irradiance π in each", () => {
    expect([written.wavelengthsNm, written.suns[0]?.irradiance]).toEqual([
      ALBEDO_BANDS.map((band) => ALBEDO_WAVELENGTHS_NM[band]),
      ALBEDO_BANDS.map(() => Math.PI),
    ]);
  });

  it("looks from above the top along each node's μ, the sun behind the observer", () => {
    expect(
      written.geometries.map((g) => {
        const sun = g.sunDirections[0];
        return [
          g.observer.heightM > written.topHeightM,
          Math.cos(((180 - g.view.zenithDeg) * Math.PI) / 180),
          sun === undefined ? Number.NaN : angleBetweenDeg(g.view, sun),
        ];
      }),
    ).toEqual(ALBEDO_NODES.map((n) => [true, expect.closeTo(n.mu, 12), expect.closeTo(180, 9)]));
  });

  it.each(Object.keys(FILES))("keeps %s under the added-file hook", (file) => {
    expect(new TextEncoder().encode(FILES[file]).length).toBeLessThan(ADDED_FILE_LIMIT_BYTES);
  });
});
