import { describe, expect, it } from "vitest";

import registry from "../../../../../../packages/protocol/fixtures/substances.json" with { type: "json" };
import { SUBSTANCE_NAMES, UNLISTED_MATERIAL, substanceName } from "./substances";

/** A name key, by `SubstanceKey`'s grammar. */
const NAME_KEY = /^[a-z][a-z0-9_]*$/u;
/** A formula key, by the same grammar: a name must never be written as one. */
const FORMULA_KEY = /^(?:[A-Z][a-z]?(?:[2-9]|[1-9][0-9]+)?)+[+-]?$/u;
/** Latin-1, the en dash and the minus sign: characters both bundled B612 faces hold. */
const BOTH_FACES = /^[\x20-\x7E\xA0-\xFF–−]+$/u;

describe("substanceName", () => {
  it("writes a formula key as its formula, digits on the line and case kept", () => {
    expect(["H2O", "Mg2SiO4", "NH4SH", "Co", "CO"].map((key) => substanceName(key))).toEqual([
      "H2O",
      "Mg2SiO4",
      "NH4SH",
      "Co",
      "CO",
    ]);
  });

  it("keeps a formula's case in an upper-case label", () => {
    expect(substanceName("Mg2SiO4", "upper")).toBe("Mg2SiO4");
  });

  it("puts an ion's sign on the line, a minus as the minus sign, and writes the electron e−", () => {
    expect(["Fe+", "H-", "e-"].map((key) => substanceName(key, "upper"))).toEqual([
      "Fe+",
      "H−",
      "e−",
    ]);
  });

  it("names a name key in words", () => {
    expect(substanceName("mars_dust")).toBe("Mars-type dust");
  });

  it("upper-cases a name's words in a label", () => {
    expect(substanceName("mars_dust", "upper")).toBe("MARS-TYPE DUST");
  });

  it("reads an unlisted material for a name key it does not name, never the key", () => {
    expect(["brine_xyz", "constructor", "h2o!", "H1", ""].map((key) => substanceName(key))).toEqual(
      Array.from({ length: 5 }, () => UNLISTED_MATERIAL),
    );
  });

  it("reads UNLISTED MATERIAL in an upper-case label", () => {
    expect(substanceName("brine_xyz", "upper")).toBe("UNLISTED MATERIAL");
  });

  it("names every row of the registry's fixture", () => {
    const unnamed = registry.substances
      .map(({ key }) => key)
      .filter(
        (key) => substanceName(key) === UNLISTED_MATERIAL || substanceName(key).includes("_"),
      );
    expect(unnamed).toEqual([]);
  });
});

describe("SUBSTANCE_NAMES", () => {
  const entries = Object.entries(SUBSTANCE_NAMES);

  it("holds name keys only, since a formula key is its own name", () => {
    expect(entries.filter(([key]) => !NAME_KEY.test(key))).toEqual([]);
  });

  it("gives no two keys one name", () => {
    expect(new Set(entries.map(([, name]) => name)).size).toBe(entries.length);
  });

  it("writes each name in three words or fewer, in characters both faces hold", () => {
    expect(
      entries.filter(([, name]) => name.split(" ").length > 3 || !BOTH_FACES.test(name)),
    ).toEqual([]);
  });

  // A name added here states its label form too: the formula rule keeps the case of any capital
  // and small letter, so a proper noun such as Io would keep its case in a label (signoff-5).
  it("gives each name its label's upper-case form", () => {
    expect(Object.fromEntries(entries.map(([key]) => [key, substanceName(key, "upper")]))).toEqual({
      mars_dust: "MARS-TYPE DUST",
      soot: "SOOT",
      tholin: "THOLIN",
      graphite: "GRAPHITE",
      chromophore: "JOVIAN CHROMOPHORE",
      ice_giant_haze: "ICE-GIANT HAZE",
      acid_uv_absorber: "ULTRAVIOLET-ABSORBING H2SO4",
      basalt: "BASALT",
      basalt_fe: "Fe-RICH BASALT",
      lava_glass: "VOLCANIC GLASS",
    });
  });

  it("writes no name as a key is written, with an underscore or as a formula", () => {
    expect(
      entries.filter(
        ([, name]) => name.includes("_") || FORMULA_KEY.test(name) || name === UNLISTED_MATERIAL,
      ),
    ).toEqual([]);
  });
});
