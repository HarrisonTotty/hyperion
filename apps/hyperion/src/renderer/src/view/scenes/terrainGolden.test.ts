import { describe, expect, it } from "vitest";

import terrainGoldenText from "../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test.golden?raw";
import weatheredGoldenText from "../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test_weathered.golden?raw";
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

  it("gives TERRAIN TEST's field, whose header carries no maturity, no maturity's lines", () => {
    expect(Object.hasOwn(parseTerrainGolden(terrainGoldenText), "maturity")).toBe(false);
  });

  it("reads TEST MOON weathered's maturity: its crust's albedo, three patches' digests and the class map's (R10.T1.c)", () => {
    const golden = parseTerrainGolden(weatheredGoldenText);
    expect([golden.body, golden.header.maturity, golden.header.weathering_rate]).toEqual([
      "5e5e000000000000.0003",
      1,
      1,
    ]);
    const { maturity } = golden;
    // The line's comment, which the reader does not read, gives the same values in decimal.
    expect(maturity?.crustAlbedoBvr).toEqual([
      0.099776827272101, 0.12322632988308144, 0.13121348331486257,
    ]);
    expect(maturity?.patches.map(({ key }) => key)).toEqual([
      { face: 0, level: 8, i: 255, j: 216 },
      { face: 0, level: 13, i: 8_161, j: 6_926 },
      { face: 0, level: 17, i: 130_579, j: 110_831 },
    ]);
    expect([maturity?.patches[0]?.codesDigest, maturity?.classMap.nodesDigest]).toEqual([
      0x726e2b8a01c7c42en,
      0x7fd95ce487df8338n,
    ]);
    // The weathered field's payloads are TERRAIN TEST's own surveys over another world.
    expect(golden.payloads.map(({ revision, have }) => [revision, have])).toEqual(
      terrainTestGolden().payloads.map(({ revision, have }) => [revision, have]),
    );
  });

  it("refuses a maturity line out of its place or form, and a header's maturity without its lines", () => {
    const head = "# generator_version = 22\nbody 1\nseed 2\n";
    const digest = "0x0123456789abcdef";
    expect(() => parseTerrainGolden(`${head}maturity_patch 0 8 1 2 ${digest} ${digest}\n`)).toThrow(
      "line 4: a maturity_patch is <face> <level> <i> <j> <codes> <nodes>",
    );
    expect(() =>
      parseTerrainGolden(
        `${head}crust_albedo 0x3fb999999999999a 0x3fb999999999999a 0x3fb999999999999a\n` +
          `maturity_patch 6 8 1 2 ${digest} ${digest}\n`,
      ),
    ).toThrow("line 5: 6 is not a face");
    expect(() =>
      parseTerrainGolden(
        `${head}crust_albedo 0x3fb999999999999a 0x3fb999999999999a 0x3fb999999999999a\n` +
          `class_map_maturity ${digest} 0x12\n`,
      ),
    ).toThrow("line 5: 0x12 is not a digest's 64 bits");
    const unlined = weatheredGoldenText.slice(0, weatheredGoldenText.indexOf("crust_albedo"));
    expect(() => parseTerrainGolden(unlined)).toThrow(
      "the maturity's lines follow a header with a maturity, and only one",
    );
  });

  it("names the file it refuses: TERRAIN TEST's by default, or the one it is given", () => {
    const text = "# generator_version = 22\nbody 1\nseed 2\nshape 3\n";
    expect(() => parseTerrainGolden(text)).toThrow("terrain_test.golden: line 4: shape");
    expect(() => parseTerrainGolden(text, "terrain_test_weathered.golden")).toThrow(
      "terrain_test_weathered.golden: line 4: shape",
    );
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
