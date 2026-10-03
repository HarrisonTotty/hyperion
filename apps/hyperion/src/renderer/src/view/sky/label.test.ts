import { describe, expect, it } from "vitest";

import { STAR_SOURCE, STARS_WITHOUT_POSITION } from "../../displays/view/viewRun";
import { skyLabelValue } from "./label";

describe("the sky line", () => {
  it("states the eye's limit with its kind", () => {
    expect(skyLabelValue(7.42, "eye", [])).toBe("V 7.4 EYE");
  });

  it("states a camera's limit with its kind", () => {
    expect(skyLabelValue(9.96, "camera", [])).toBe("V 10.0 CAM");
  });

  it("names each stand-in after a middle dot, the centre's members as clusters once", () => {
    expect(
      skyLabelValue(9.5, "camera", ["feature_members", "centre_members", "white_dwarfs"]),
    ).toBe("V 9.5 CAM · CLUSTERS NOT MODELLED · WD NOT MODELLED");
  });

  it("leaves R02's interim readings as they were drafted", () => {
    expect(STAR_SOURCE).toBe("RANGE QUERY · VOLUME-LIMITED · NO EXTINCTION");
    expect(STARS_WITHOUT_POSITION).toBe("NOT AVAILABLE: the system's position is not known");
  });
});
