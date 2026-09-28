import type { BinaryClassDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { binaryClassLabel } from "./binaryClass";

describe("binaryClassLabel", () => {
  it("spells out every class, one name each", () => {
    const classes: ReadonlyArray<BinaryClassDto> = [
      { type: "algol" },
      { type: "contact" },
      { type: "blue_straggler" },
      { type: "hot_subdwarf" },
      { type: "r_coronae_borealis" },
      { type: "symbiotic" },
      { type: "cataclysmic_variable", kind: "dwarf_nova" },
      { type: "cataclysmic_variable", kind: "nova_like" },
      { type: "cataclysmic_variable", kind: "magnetic" },
      { type: "cataclysmic_variable", kind: "am_cvn" },
      { type: "low_mass_xray_binary", kind: "persistent" },
      { type: "low_mass_xray_binary", kind: "transient" },
      { type: "high_mass_xray_binary", kind: "be_x" },
      { type: "high_mass_xray_binary", kind: "supergiant" },
      { type: "millisecond_pulsar" },
      { type: "double_neutron_star" },
      { type: "double_white_dwarf" },
      { type: "type_ia_progenitor" },
    ];
    const labels = classes.map(binaryClassLabel);

    expect(new Set(labels).size).toBe(classes.length);
    for (const label of labels) {
      expect(label).toBe(label.toUpperCase());
    }
    expect(labels).toContain("DWARF NOVA");
    expect(labels).toContain("LOW-MASS X-RAY BINARY · TRANSIENT");
    expect(labels).toContain("BE X-RAY BINARY");
  });
});
