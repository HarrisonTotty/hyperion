import { describe, expect, it } from "vitest";

import {
  SECTION_NOT_APPLICABLE,
  SECTION_NOT_MODELLED,
  SECTION_NOT_RESOLVED,
  SECTION_OK_LIST,
} from "../../test/systemFixtures";
import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { populatedBodies, sliceBodies } from "../../test/planetaryFixture";
import { smallBodySections, systemNote } from "./systemNote";

/** The slice's tags: every small-body section not modelled, on the system and on one planet. */
const SLICE = {
  belts: SECTION_NOT_MODELLED.state,
  halo: SECTION_NOT_MODELLED.state,
  planets: [{ moons: SECTION_NOT_MODELLED.state, rings: SECTION_NOT_MODELLED.state }],
};

describe("systemNote", () => {
  it("names all four kinds the slice's tags leave unmodelled", () => {
    expect(systemNote({ kind: "tagged", sections: SLICE })).toBe(
      "MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
    );
  });

  it("says nothing when every kind is modelled", () => {
    const modelled = {
      belts: SECTION_OK_LIST.state,
      halo: SECTION_OK_LIST.state,
      planets: [{ moons: SECTION_OK_LIST.state, rings: SECTION_OK_LIST.state }],
    };

    expect(systemNote({ kind: "tagged", sections: modelled })).toBeNull();
  });

  it("drops each kind as the server starts to model it", () => {
    const sections = {
      ...SLICE,
      halo: SECTION_OK_LIST.state,
      planets: [{ moons: SECTION_NOT_MODELLED.state, rings: SECTION_OK_LIST.state }],
    };

    expect(systemNote({ kind: "tagged", sections })).toBe("MOONS AND BELTS: NOT YET MODELLED");
  });

  it("names moons when any planet's are not modelled", () => {
    const sections = {
      belts: SECTION_OK_LIST.state,
      halo: SECTION_OK_LIST.state,
      planets: [
        { moons: SECTION_OK_LIST.state, rings: SECTION_OK_LIST.state },
        { moons: SECTION_NOT_MODELLED.state, rings: SECTION_NOT_APPLICABLE.state },
      ],
    };

    expect(systemNote({ kind: "tagged", sections })).toBe("MOONS: NOT YET MODELLED");
  });

  it("does not read a section withheld or not applicable as not modelled", () => {
    const sections = {
      belts: SECTION_NOT_RESOLVED.state,
      halo: SECTION_NOT_APPLICABLE.state,
      planets: [{ moons: SECTION_NOT_RESOLVED.state, rings: SECTION_NOT_APPLICABLE.state }],
    };

    expect(systemNote({ kind: "tagged", sections })).toBeNull();
  });

  it("names the planets too while this protocol cannot ask for them", () => {
    expect(systemNote({ kind: "unserved" })).toBe(
      "PLANETS, MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
    );
  });
});

function noteOf(response: ReturnType<typeof sliceBodies>): string | null {
  const result = toSystemBodiesModel(response, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return systemNote({ kind: "tagged", sections: smallBodySections(result.bodies) });
}
describe("smallBodySections", () => {
  it("names all four kinds from the slice's tags", () => {
    expect(noteOf(sliceBodies())).toBe("MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED");
  });

  it("names only the moons and rings of the one planet whose tags leave them unmodelled", () => {
    // Its belts and halo are ok; its unformed planet's moons and rings are not modelled.
    expect(noteOf(populatedBodies())).toBe("MOONS AND RINGS: NOT YET MODELLED");
  });
});
