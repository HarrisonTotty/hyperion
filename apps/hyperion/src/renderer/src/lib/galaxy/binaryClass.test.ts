import type { BinaryClassDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { aSunlikeStar, aSystemSummary, aTripleSummary } from "../../test/systemFixtures";
import type { SystemModel } from "../system/model";
import { toSystemModel } from "../system/wire";
import { binaryClassLabel, binaryClassRows } from "./binaryClass";

describe("binaryClassLabel", () => {
  it("reads every class in the words ruling 130 gives it", () => {
    const words: ReadonlyArray<readonly [BinaryClassDto, string]> = [
      [{ type: "algol" }, "ALGOL"],
      [{ type: "contact" }, "CONTACT BINARY"],
      [{ type: "blue_straggler" }, "BLUE STRAGGLER"],
      [{ type: "hot_subdwarf" }, "HOT SUBDWARF"],
      [{ type: "r_coronae_borealis" }, "R CORONAE BOREALIS STAR"],
      [{ type: "symbiotic" }, "SYMBIOTIC STAR"],
      [{ type: "cataclysmic_variable", kind: "dwarf_nova" }, "DWARF NOVA"],
      [{ type: "cataclysmic_variable", kind: "nova_like" }, "NOVA-LIKE VARIABLE"],
      [{ type: "cataclysmic_variable", kind: "magnetic" }, "MAGNETIC CATACLYSMIC VARIABLE"],
      [{ type: "cataclysmic_variable", kind: "am_cvn" }, "AM CVN STAR"],
      [{ type: "low_mass_xray_binary", kind: "persistent" }, "LOW-MASS X-RAY BINARY"],
      [{ type: "low_mass_xray_binary", kind: "transient" }, "X-RAY NOVA"],
      [{ type: "low_mass_xray_binary", kind: "symbiotic" }, "SYMBIOTIC X-RAY BINARY"],
      [{ type: "high_mass_xray_binary", kind: "be_x" }, "Be X-RAY BINARY"],
      [{ type: "high_mass_xray_binary", kind: "supergiant" }, "SUPERGIANT X-RAY BINARY"],
      [{ type: "millisecond_pulsar" }, "MILLISECOND PULSAR"],
      [{ type: "double_neutron_star" }, "DOUBLE NEUTRON STAR"],
      [{ type: "double_white_dwarf" }, "DOUBLE WHITE DWARF"],
      [{ type: "type_ia_progenitor" }, "TYPE Ia PROGENITOR"],
    ];

    expect(words.map(([binaryClass]) => binaryClassLabel(binaryClass))).toEqual(
      words.map(([, label]) => label),
    );
    expect(new Set(words.map(([, label]) => label)).size).toBe(words.length);
    for (const [, label] of words) {
      const parts = label.split(" ");
      expect(new Set(parts).size, `${label} repeats a word`).toBe(parts.length);
    }
  });
});

function modelOf(summary: Parameters<typeof toSystemModel>[0]): SystemModel {
  const result = toSystemModel(summary, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(`the fixture should convert, but: ${result.fault}`);
  }
  return result.model;
}

function rowsOf(model: SystemModel): ReadonlyArray<readonly [string, string]> {
  const table = binaryClassRows(model);
  if (table.kind !== "rows") {
    throw new Error("the fixture's classes are computed");
  }
  return table.rows.map(({ stars, binaryClass }) => [
    stars,
    binaryClass.kind === "value" ? binaryClassLabel(binaryClass.value) : "—",
  ]);
}

describe("binaryClassRows", () => {
  it("gives a pair whose stars carry different classes a row for each star", () => {
    const summary = aSystemSummary({
      stars: [
        aSunlikeStar({ binary_class: { type: "blue_straggler" } }),
        aSunlikeStar({ body_index: 1, binary_class: { type: "algol" } }),
      ],
    });

    expect(rowsOf(modelOf(summary))).toEqual([
      ["A", "BLUE STRAGGLER"],
      ["B", "ALGOL"],
    ]);
  });

  it("joins only the innermost pair of a triple, and reads the third star under its letter", () => {
    const triple = aTripleSummary();
    const algol = { type: "algol" } as const;
    const stars = triple.stars.map((star) => ({ ...star, binary_class: algol }));

    expect(rowsOf(modelOf({ ...triple, stars }))).toEqual([
      ["A–B", "ALGOL"],
      ["C", "ALGOL"],
    ]);
  });
});
