/**
 * R08.T5.e's geometric albedo on the plane-parallel Venus deck (plan R08): the committed
 * `venus-92bar-albedo.case.json` is the case `test/venusAlbedo.ts` writes now on the committed
 * `acid_uv_absorber`, through `bless.ts`; its geometries look along each node's μ with the sun
 * behind; and a fit's trial file stands in for mode 2's alone. The quadrature's own tests are
 * `test/venusAlbedo.test.ts`'s.
 *
 * @remarks
 * Rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/venusAlbedo.test.ts`, and its reference traced afresh
 * by `cargo run --release -p hyperion-fit -- atmosphere-reference <case> --out <reference> --samples
 * 6144 --stokes --threads 4`; `hyperion-fit`'s `venus_uv_absorber` tests hold that reference to
 * Venus's albedos. The deck's Mie at six wavelengths runs for about a minute and a half.
 */

import { describe, expect, it } from "vitest";

import { ADDED_FILE_LIMIT_BYTES, type CaseJson } from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import {
  ALBEDO_BANDS,
  ALBEDO_NODES,
  ALBEDO_WAVELENGTHS_NM,
  albedoCaseJson,
} from "../../../test/venusAlbedo";
import { type MaterialIndexFile, resolveMaterial } from "../materials/materials";
import committed from "./venus-92bar-albedo.case.json" with { type: "json" };
import { seedOf } from "./cases";
import { angleBetweenDeg } from "./geometries";
import { cos } from "../../../lib/pinnedMath";

/** The case's name, its file beside `venus-92bar`'s, and its seed its own. */
const NAME = "venus-92bar-albedo";

/** The texts of the committed albedo files, for their sizes. */
const FILES = import.meta.glob<string>("./venus-92bar-albedo.*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** The deck's Mie, ms. */
const SLOW_MS = 600_000;

/** A function's value, computed on the first call and held. */
function once<T>(make: () => T): () => T {
  let held: { readonly value: T } | undefined;
  return () => {
    held ??= { value: make() };
    return held.value;
  };
}

/** The case as the client's optics write it now, its Mie run when a test first asks. */
const written = once(() => albedoCaseJson({ name: NAME, seed: seedOf(NAME) }));

describe("the albedo case", { timeout: SLOW_MS }, () => {
  it("venus-92bar-albedo.case.json is the case the client's optics write now", async () => {
    await expect(
      expectCommittedTable(written(), committed, `./${NAME}.case.json`),
    ).resolves.toBeUndefined();
  });

  it("traces every band, the sun of irradiance π in each", () => {
    expect([written().wavelengthsNm, written().suns[0]?.irradiance]).toEqual([
      ALBEDO_BANDS.map((band) => ALBEDO_WAVELENGTHS_NM[band]),
      ALBEDO_BANDS.map(() => Math.PI),
    ]);
  });

  it("looks from above the top along each node's μ, the sun behind the observer", () => {
    const albedoCase = written();
    expect(
      albedoCase.geometries.map((g) => {
        const sun = g.sunDirections[0];
        return [
          g.observer.heightM > albedoCase.topHeightM,
          cos(((180 - g.view.zenithDeg) * Math.PI) / 180),
          sun === undefined ? Number.NaN : angleBetweenDeg(g.view, sun),
        ];
      }),
    ).toEqual(ALBEDO_NODES.map((n) => [true, expect.closeTo(n.mu, 12), expect.closeTo(180, 9)]));
  });

  it.each(Object.keys(FILES))("keeps %s under the added-file hook", (file) => {
    expect(new TextEncoder().encode(FILES[file]).length).toBeLessThan(ADDED_FILE_LIMIT_BYTES);
  });
});

/** Each term's absorption, by name. */
function absorptions(albedoCase: CaseJson): ReadonlyArray<readonly [string, readonly number[]]> {
  return albedoCase.terms.map((t) => [t.name, t.absorption] as const);
}

/** The committed `acid_uv_absorber` file, the fit's trials' template. */
function committedAbsorber(): MaterialIndexFile {
  const { file } = resolveMaterial("acid_uv_absorber");
  if (file === undefined) {
    throw new Error("acid_uv_absorber has its file");
  }
  return file;
}

describe("a fit's trial file", { timeout: SLOW_MS }, () => {
  const options = { name: NAME, seed: seedOf(NAME), bands: ["u365"] } as const;
  const plain = once(() => albedoCaseJson(options));

  it("of the committed absorber writes the committed absorber's case", () => {
    expect(albedoCaseJson({ ...options, acidFile: committedAbsorber() })).toEqual(plain());
  });

  it("of a greater k raises mode 2's absorption alone", () => {
    const absorber = committedAbsorber();
    const doubled = albedoCaseJson({
      ...options,
      acidFile: { ...absorber, k: absorber.k.map((k) => 2 * k) },
    });
    const before = absorptions(plain());
    expect(
      absorptions(doubled).map(([name, a], i) => [
        name,
        (a[0] ?? 0) > (before[i]?.[1][0] ?? Number.POSITIVE_INFINITY),
      ]),
    ).toEqual(before.map(([name]) => [name, name === "aerosol:acid_uv_absorber"]));
  });
});
