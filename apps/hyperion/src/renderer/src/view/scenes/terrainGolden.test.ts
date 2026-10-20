import { describe, expect, it } from "vitest";

import terrainGoldenText from "../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test.golden?raw";
import { BLOCK_MAGIC } from "../terrain/coarseFieldStore";
import { blockBytes, parseTerrainGolden } from "./terrainGolden";
import { terrainTestGolden } from "./terrainScene";

/** The number of cells a cover's runs hold. */
function cells(ranges: ReadonlyArray<{ readonly start: number; readonly end: number }>): number {
  return ranges.reduce((sum, { start, end }) => sum + end - start, 0);
}

describe("the terrain golden (R10.T16.d)", () => {
  it("names the Moon-like world's body and seed, and its header is the body's", () => {
    const golden = terrainTestGolden();
    expect([golden.body, golden.seed, golden.header.body]).toEqual([
      "5e5e000000000000.0003",
      "617373656d626c79",
      "5e5e000000000000.0003",
    ]);
    expect(golden.header.level).toBe(6);
    expect(golden.header.polar_radius_m).toBeLessThan(golden.header.equatorial_radius_m);
    expect(golden.header.palette.map(({ substance, role }) => [substance, role])).toEqual([
      ["feldspathic", "primary_crust"],
      ["basalt_fe", "province"],
      ["H2O", "ice"],
    ]);
    expect(Object.hasOwn(golden.header, "maturity")).toBe(false);
  });

  it("gives each palette entry's FeO_T as R09.T2.f's palette holds it", () => {
    const { palette } = terrainTestGolden().header;
    expect(palette.map(({ feo_mass_fraction }) => feo_mass_fraction)).toEqual([0.05, 0.2, 0]);
  });

  it("leaves the weathering rate out while it is not modelled, as the wire does", () => {
    expect(Object.hasOwn(terrainTestGolden().header, "weathering_rate")).toBe(false);
  });

  it("reads a copy with CRLF line ends as the LF one", () => {
    const crlf = terrainGoldenText.replaceAll("\n", "\r\n");
    expect(parseTerrainGolden(crlf)).toEqual(parseTerrainGolden(terrainGoldenText));
  });

  it("holds the first payload, the delta, the whole at revision 2 and an ask at the revision held", () => {
    const { payloads, surveys } = terrainTestGolden();
    expect(payloads.map(({ revision, have }) => [revision, have])).toEqual([
      [1, null],
      [2, 1],
      [2, null],
      [2, 2],
    ]);
    expect(payloads.map(({ blocksHex }) => blocksHex.length >= 3)).toEqual([
      true,
      true,
      true,
      false,
    ]);
    expect([payloads[3]?.blocksHex.length, payloads[3]?.coverage.ranges]).toEqual([1, []]);
    // The second survey extends the first.
    expect(cells(surveys[1]?.ranges ?? [])).toBeGreaterThan(cells(surveys[0]?.ranges ?? []));
  });

  it("gives every block as R09's framed bytes, its length at offset 6", () => {
    for (const payload of terrainTestGolden().payloads) {
      for (const hex of payload.blocksHex) {
        const block = blockBytes(hex);
        const length = new DataView(block.buffer).getUint32(6, true);
        expect([[...block.subarray(0, 4)], length]).toEqual([[...BLOCK_MAGIC], block.length]);
      }
    }
  });

  it("gives what each block of the first payload and the delta changes, one a block", () => {
    const { changes, payloads } = terrainTestGolden();
    const posted = (payloads[0]?.blocksHex.length ?? 0) + (payloads[1]?.blocksHex.length ?? 0);
    expect(changes).toHaveLength(posted);
    expect(changes.some(({ surveyed }) => surveyed.length > 0)).toBe(true);
  });

  it("refuses a line it cannot read, naming it", () => {
    expect(() => parseTerrainGolden("# generator_version = 22\nbody 1\nseed 2\nshape 3\n")).toThrow(
      "line 4: shape is not a line of the terrain test's format",
    );
    expect(() =>
      parseTerrainGolden("seed 2\ndto palette 0 basalt primary_crust 1.5 1.5 1.5 0 - -\n"),
    ).toThrow("line 2: 1.5 is not a float's bits");
    expect(() => parseTerrainGolden("# generator_version = 22\nbody 1\nseed 2\n")).toThrow(
      "the header has no body",
    );
  });
});
