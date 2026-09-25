import { describe, expect, it } from "vitest";

import { everyGalaxyParameter } from "../../test/galaxyFixtures";
import { GROUP_LABELS, groupLabel, PARAMETER_LABELS, parameterLabel } from "./parameterLabels";

const { groups } = everyGalaxyParameter();
const sentKeys = groups.flatMap((group) => group.parameters.map((parameter) => parameter.key));

/** Upper case is for labels of three words or fewer (the guide's Typography). */
function isShortUpperCase(label: string): boolean {
  return label === label.toUpperCase() && label.split(" ").length <= 3;
}

describe("the parameter glossary", () => {
  it.each(groups.map((group) => group.key))("names the group %s", (key) => {
    expect(Object.hasOwn(GROUP_LABELS, key)).toBe(true);
  });

  it.each(sentKeys)("names the parameter %s, which P04.T14.b sends", (key) => {
    expect(Object.hasOwn(PARAMETER_LABELS, key)).toBe(true);
  });

  // In order, not as sets: the fixture copies P04.T14.b's table, group by group and key by key,
  // so a key the glossary moves within its group, or a group moved, is caught (val07, round 9).
  it("names nothing that P04.T14.b does not send, in its table's order", () => {
    expect(Object.keys(PARAMETER_LABELS)).toEqual(sentKeys);
    expect(Object.keys(GROUP_LABELS)).toEqual(groups.map((group) => group.key));
  });

  it("says the populations' shares are shares of the systems", () => {
    expect(groupLabel("populations")).toBe("SHARE OF SYSTEMS");
    expect(parameterLabel("population.thick_disc.share")).toBe("THICK DISC");
  });

  it("sends each key once", () => {
    expect(new Set(sentKeys).size).toBe(sentKeys.length);
  });

  it.each([...Object.values(GROUP_LABELS), ...Object.values(PARAMETER_LABELS)])(
    "sets %s in upper case in three words or fewer",
    (label) => {
      expect(isShortUpperCase(label)).toBe(true);
    },
  );

  it("names the stellar halo as the population is named, and the dark halo apart from it", () => {
    expect(groupLabel("halo")).toBe(parameterLabel("population.halo.share"));
    expect(parameterLabel("dark_halo.mass")).toBe("DARK HALO MASS");
  });
});
