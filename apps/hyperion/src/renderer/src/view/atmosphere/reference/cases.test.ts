/**
 * The reference tracer's cloud-free cases (plan R08, R08.T12.b.1): each `<name>.case.json` is the
 * case the client's optics write now, through `bless.ts`; each file here stays under the
 * repository's added-file hook; and the geometry sets are Design note 10's and the spheroid
 * ruling's.
 *
 * @remarks
 * A changed case is rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/cases.test.ts`, and its reference traced afresh from
 * the repository's root by `test/atmosphereCases.ts`'s `traceCommand`:
 * `cargo run --release -p hyperion-fit -- atmosphere-reference <case> --out <reference> --samples
 * 4096 --stokes --threads 4`, `<case>` and `<reference>` this directory's `<name>.case.json` and
 * `<name>.reference.json`.
 */

import { describe, expect, it } from "vitest";

import {
  ADDED_FILE_LIMIT_BYTES,
  caseJson,
  type SampleCounts,
  traceCommand,
} from "../../../test/atmosphereCases";
import {
  gravityOfSpheroid,
  SATURN_SPHEROID,
  saturnClassMedium,
  saturnScaleHeightM,
} from "../../../test/atmosphereFixtures";
import { expectCommittedTable } from "../../../test/bless";
import { geodeticOf } from "../hillaire";
import { localScaleHeightM } from "../medium";
import { gravityRatio } from "../oblate";
import earthCase from "./earth.case.json" with { type: "json" };
import earthOzoneSlabCase from "./earth-ozone-slab.case.json" with { type: "json" };
import marsCase from "./mars.case.json" with { type: "json" };
import saturnOblateCase from "./saturn-oblate.case.json" with { type: "json" };
import sampleCounts from "./sampleCounts.json" with { type: "json" };
import {
  EARTH_CASE,
  MARS_CASE,
  REFERENCE_CASES,
  type ReferenceCase,
  SATURN_OBLATE_CASE,
  SATURN_OBLATE_GRAVITY,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_SCALE_HEIGHT_M,
  SATURN_OBLATE_SPHEROID,
} from "./cases";
import {
  angleBetweenDeg,
  geodeticPoint,
  LEAST_SUN_DISTANCE_DEG,
  localDirectionVector,
  type ReferenceGeometry,
  SPHERE_LIMB_SCALE_HEIGHTS,
} from "./geometries";

/** Each case's own counts, from its pilot (`decision-r08-t12b-scope.md`, ruling 8). */
const COUNTS: Readonly<Record<string, SampleCounts>> = sampleCounts;

/** The committed case files, by case. */
const COMMITTED: Readonly<Record<string, unknown>> = {
  earth: earthCase,
  "earth-ozone-slab": earthOzoneSlabCase,
  mars: marsCase,
  "saturn-oblate": saturnOblateCase,
};

/** Every file in this directory, by path, as its text. */
const FILES = import.meta.glob<string>("./*", { query: "?raw", import: "default", eager: true });

const CASES = REFERENCE_CASES.map((c) => [c.name, c] as const);

describe("the reference cases", () => {
  it.each(CASES)("%s.case.json is the case the client's optics write now", async (name, c) => {
    try {
      await expect(
        expectCommittedTable(caseJson(c, COUNTS[name]), COMMITTED[name], `./${name}.case.json`),
      ).resolves.toBeUndefined();
    } catch (error: unknown) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`${reason}; then trace its reference: \`${traceCommand(name)}\``, {
        cause: error,
      });
    }
  });

  it.each(Object.keys(FILES))("keeps %s under the added-file hook", (file) => {
    expect(new TextEncoder().encode(FILES[file]).length).toBeLessThan(ADDED_FILE_LIMIT_BYTES);
  });

  it.each(CASES)("%s's counts each name one of its geometries or aggregates", (name, c) => {
    const counts = COUNTS[name];
    const geometries = new Set(c.geometries.map((g) => g.name));
    const aggregates = new Set(c.aggregates.map((a) => a.name));
    const strays = [
      ...Object.keys(counts?.geometries ?? {}).filter((key) => !geometries.has(key)),
      ...Object.keys(counts?.aggregates ?? {}).filter((key) => !aggregates.has(key)),
    ];
    expect(strays).toEqual([]);
  });
});

/**
 * The lowest point along a written view ray, by a golden-section search: its geodetic latitude and
 * height. The height is flat there, so the search finds it to a few centimetres along the ray and
 * its latitude to about 10⁻⁶°.
 */
function lowestPoint(
  referenceCase: ReferenceCase,
  geometry: ReferenceGeometry,
): { readonly latitudeDeg: number; readonly heightM: number; readonly tM: number } {
  const figure =
    referenceCase.figure.kind === "sphere"
      ? {
          equatorialRadiusM: referenceCase.figure.radiusM,
          polarRadiusM: referenceCase.figure.radiusM,
        }
      : referenceCase.figure.gravity.spheroid;
  const origin = geodeticPoint(figure, geometry.observer.latitudeDeg, geometry.observer.heightM);
  const direction = localDirectionVector(geometry.observer.latitudeDeg, geometry.view);
  const heightAt = (t: number): number =>
    geodeticOf(
      {
        x: origin.x + t * direction.x,
        y: origin.y + t * direction.y,
        z: origin.z + t * direction.z,
      },
      figure,
    ).heightM;
  // Golden-section search for the least height over the ray's first 4 × 10⁷ m.
  let low = 0;
  let high = 4e7;
  const ratio = (Math.sqrt(5) - 1) / 2;
  for (let i = 0; i < 200; i += 1) {
    const a = high - ratio * (high - low);
    const b = low + ratio * (high - low);
    if (heightAt(a) < heightAt(b)) {
      high = b;
    } else {
      low = a;
    }
  }
  const tM = 0.5 * (low + high);
  const point = {
    x: origin.x + tM * direction.x,
    y: origin.y + tM * direction.y,
    z: origin.z + tM * direction.z,
  };
  const geodetic = geodeticOf(point, figure);
  return { latitudeDeg: (geodetic.latitudeRad * 180) / Math.PI, heightM: geodetic.heightM, tM };
}

describe("the geometry sets", () => {
  it("give Design note 10's 107 ground geometries on a sphere", () => {
    expect(EARTH_CASE.geometries.filter((g) => g.name.startsWith("ground-"))).toHaveLength(107);
  });

  it("group a sphere's case into Design note 10's images", () => {
    const images = new Set(EARTH_CASE.geometries.map((g) => g.image));
    expect([...images].toSorted()).toEqual(
      [
        ...["s0", "s30", "s60", "s80", "s90", "s95"].map((s) => `ground-${s}`),
        ...[0, 30, 60, 75, 85].map((p) => `disc-p${p}`),
        "limb",
        "aerial",
      ].toSorted(),
    );
  });

  it("give a sphere's case the aggregates of its ground images' suns below 90°", () => {
    expect(EARTH_CASE.aggregates.map((a) => a.sun.zenithDeg)).toEqual([0, 30, 60, 80]);
  });

  it("add R08.T13's Mars points 3°–12° above a sun 5° and 30° up", () => {
    const aureole = MARS_CASE.geometries.filter((g) => g.name.startsWith("aureole-"));
    expect(aureole.map((g) => [g.image, angleBetweenDeg(g.view, g.sun)])).toEqual(
      [85, 60].flatMap((s) => [3, 5, 8, 12].map((d) => [`ground-s${s}`, expect.closeTo(d, 9)])),
    );
  });

  it.each(CASES)("%s names each geometry once", (_, referenceCase) => {
    const names = referenceCase.geometries.map((g) => g.name);
    expect(new Set(names).size).toBe(names.length);
  });

  it.each(CASES)("%s has no view within 3° of its sun", (_, referenceCase) => {
    const nearest = Math.min(
      ...referenceCase.geometries.map((g) => angleBetweenDeg(g.view, g.sun)),
    );
    expect(nearest).toBeGreaterThan(LEAST_SUN_DISTANCE_DEG - 1e-9);
  });

  it("put a sphere's limbs at n scale heights of its medium, tangent over the equator", () => {
    const height = localScaleHeightM(EARTH_CASE.medium, 0);
    const limbs = EARTH_CASE.geometries.filter((g) => g.image === "limb");
    expect(limbs.map((g) => lowestPoint(EARTH_CASE, g))).toEqual(
      SPHERE_LIMB_SCALE_HEIGHTS.map((n) => ({
        latitudeDeg: expect.closeTo(0, 4),
        heightM: expect.closeTo(n * height, 3),
        tM: expect.any(Number),
      })),
    );
  });

  it("put saturn-oblate's limbs at n H ÷ s(φ) over the equator, 45° and the pole", () => {
    const limbs = SATURN_OBLATE_CASE.geometries.filter((g) => g.name.startsWith("limb-"));
    const expected = [0, 45, 90].flatMap((latitudeDeg) =>
      [0.3, 1, 3].map((n) => ({
        latitudeDeg: expect.closeTo(latitudeDeg, 4),
        heightM: expect.closeTo(
          (n * SATURN_OBLATE_SCALE_HEIGHT_M) /
            gravityRatio(SATURN_OBLATE_GRAVITY, (latitudeDeg * Math.PI) / 180),
          2,
        ),
        tM: expect.any(Number),
      })),
    );
    expect(limbs.map((g) => lowestPoint(SATURN_OBLATE_CASE, g))).toEqual(expected);
  });

  it("put saturn-oblate's limb sun on each tangent point's horizon", () => {
    const limbs = SATURN_OBLATE_CASE.geometries.filter((g) => g.name.startsWith("limb-"));
    const zeniths = limbs.map((g) => {
      const { tM } = lowestPoint(SATURN_OBLATE_CASE, g);
      const figure = SATURN_OBLATE_SPHEROID;
      const origin = geodeticPoint(figure, g.observer.latitudeDeg, g.observer.heightM);
      const view = localDirectionVector(g.observer.latitudeDeg, g.view);
      const sun = localDirectionVector(g.observer.latitudeDeg, g.sun);
      const normal = geodeticOf(
        { x: origin.x + tM * view.x, y: origin.y + tM * view.y, z: origin.z + tM * view.z },
        figure,
      ).normal;
      const cos = sun.x * normal.x + sun.y * normal.y + sun.z * normal.z;
      return (Math.acos(cos) * 180) / Math.PI;
    });
    expect(zeniths).toEqual(limbs.map(() => expect.closeTo(90, 4)));
  });

  it("give saturn-oblate the spheroid ruling's three ground sites", () => {
    const ground = SATURN_OBLATE_CASE.geometries.filter((g) => !g.name.startsWith("limb-"));
    expect(
      ["equator", "lat60", "pole"].map(
        (site) => ground.filter((g) => g.name.startsWith(`${site}-`)).length,
      ),
    ).toEqual([51, 51, 47]);
  });
});

describe("saturn-oblate's medium", () => {
  it("is the twin tests' Saturn-class fixture, term for term", () => {
    const fixture = saturnClassMedium();
    expect([
      SATURN_OBLATE_REFERENCE.terms,
      SATURN_OBLATE_REFERENCE.topHeightM,
      SATURN_OBLATE_REFERENCE.groundAlbedo,
      SATURN_OBLATE_SCALE_HEIGHT_M,
    ]).toEqual([fixture.terms, fixture.topHeightM, fixture.groundAlbedo, saturnScaleHeightM()]);
  });

  it("lies on the fixture's figure and gravity", () => {
    expect([SATURN_OBLATE_SPHEROID, SATURN_OBLATE_GRAVITY]).toEqual([
      SATURN_SPHEROID,
      gravityOfSpheroid(SATURN_SPHEROID),
    ]);
  });
});
