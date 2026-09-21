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

  it("names nothing that P04.T14.b does not send", () => {
    expect(Object.keys(PARAMETER_LABELS).toSorted()).toEqual(sentKeys.toSorted());
    expect(Object.keys(GROUP_LABELS).toSorted()).toEqual(
      groups.map((group) => group.key).toSorted(),
    );
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
