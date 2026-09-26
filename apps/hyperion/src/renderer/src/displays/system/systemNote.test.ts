import { describe, expect, it } from "vitest";

import {
  SECTION_NOT_APPLICABLE,
  SECTION_NOT_MODELLED,
  SECTION_NOT_RESOLVED,
  SECTION_OK_LIST,
} from "../../test/systemFixtures";
import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { populatedBodies, populatedBodiesWith, sliceBodies } from "../../test/planetaryFixture";
import { smallBodySections, systemNotes } from "./systemNote";

/** The slice's tags: every small-body section not modelled, on the system and on one planet. */
const SLICE = {
  belts: SECTION_NOT_MODELLED.state,
  halo: SECTION_NOT_MODELLED.state,
  planets: [{ moons: SECTION_NOT_MODELLED.state, rings: SECTION_NOT_MODELLED.state }],
};

describe("systemNotes", () => {
  it("names all four kinds the slice's tags leave unmodelled", () => {
    expect(systemNotes({ kind: "tagged", sections: SLICE })).toEqual([
      "MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
    ]);
  });

  it("says nothing when every kind is modelled", () => {
    const modelled = {
      belts: SECTION_OK_LIST.state,
      halo: SECTION_OK_LIST.state,
      planets: [{ moons: SECTION_OK_LIST.state, rings: SECTION_OK_LIST.state }],
    };

    expect(systemNotes({ kind: "tagged", sections: modelled })).toEqual([]);
  });

  it("drops each kind as the server starts to model it", () => {
    const sections = {
      ...SLICE,
      halo: SECTION_OK_LIST.state,
      planets: [{ moons: SECTION_NOT_MODELLED.state, rings: SECTION_OK_LIST.state }],
    };

    expect(systemNotes({ kind: "tagged", sections })).toEqual([
      "MOONS AND BELTS: NOT YET MODELLED",
    ]);
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

    expect(systemNotes({ kind: "tagged", sections })).toEqual(["MOONS: NOT YET MODELLED"]);
  });

  it("names a section withheld as not resolved, and one not applicable not at all", () => {
    const sections = {
      belts: SECTION_NOT_RESOLVED.state,
      halo: SECTION_NOT_APPLICABLE.state,
      planets: [{ moons: SECTION_NOT_RESOLVED.state, rings: SECTION_NOT_APPLICABLE.state }],
    };

    expect(systemNotes({ kind: "tagged", sections })).toEqual(["MOONS AND BELTS: NOT RESOLVED"]);
  });

  it("names the belts and halo a contact withholds on a line of their own", () => {
    // Ruling 113.2: at `contact` the system's belts and halo are withheld, and no planet's kind is
    // resolved, so no planet's tags are read.
    const sections = {
      belts: SECTION_NOT_RESOLVED.state,
      halo: SECTION_NOT_RESOLVED.state,
      planets: [],
    };

    expect(systemNotes({ kind: "tagged", sections })).toEqual([
      "BELTS AND COMETARY HALO: NOT RESOLVED",
    ]);
  });

  it("puts the not-modelled line first and the withheld line after it", () => {
    const sections = {
      belts: SECTION_NOT_RESOLVED.state,
      halo: SECTION_NOT_RESOLVED.state,
      planets: [{ moons: SECTION_NOT_MODELLED.state, rings: SECTION_NOT_MODELLED.state }],
    };

    expect(systemNotes({ kind: "tagged", sections })).toEqual([
      "MOONS AND RINGS: NOT YET MODELLED",
      "BELTS AND COMETARY HALO: NOT RESOLVED",
    ]);
  });

  it("names the planets too while this protocol cannot ask for them", () => {
    expect(systemNotes({ kind: "unserved" })).toEqual([
      "PLANETS, MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
    ]);
  });
});

function notesOf(response: ReturnType<typeof sliceBodies>): ReadonlyArray<string> {
  const result = toSystemBodiesModel(response, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return systemNotes({ kind: "tagged", sections: smallBodySections(result.bodies) });
}
describe("smallBodySections", () => {
  it("names all four kinds from the slice's tags", () => {
    expect(notesOf(sliceBodies())).toEqual([
      "MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
    ]);
  });

  it("names only the moons and rings of the one planet whose tags leave them unmodelled", () => {
    // Its belts and halo are ok; its unformed planet's moons and rings are not modelled.
    expect(notesOf(populatedBodies())).toEqual(["MOONS AND RINGS: NOT YET MODELLED"]);
  });
});

describe("smallBodySections of a system with every small body modelled", () => {
  it("says nothing once moons, rings, belts and the halo are all tagged ok", () => {
    // The populated answer with its unformed planet's moons and rings modelled as none, as a
    // phase-D server tags them: every small-body section is then ok.
    const modelled = populatedBodiesWith((body) =>
      body.moons.state === "not_modelled"
        ? { ...body, moons: { state: "ok", value: [] }, rings: { state: "ok", value: [] } }
        : body,
    );

    expect(notesOf(modelled)).toEqual([]);
  });
});

describe("smallBodySections of a belt's members", () => {
  it("does not name rings for a member whose rings are not modelled when every planet's are", () => {
    // Every planet's moons and rings modelled; the belt's dwarf planet keeps its rings not modelled,
    // as the wire tags a dwarf planet's.
    const modelled = populatedBodiesWith((body) => {
      if (body.kind.type === "dwarf_planet") {
        return { ...body, rings: { state: "not_modelled" } };
      }
      return body.moons.state === "not_modelled"
        ? { ...body, moons: { state: "ok", value: [] }, rings: { state: "ok", value: [] } }
        : body;
    });

    expect(notesOf(modelled)).toEqual([]);
  });

  it("still names moons for a member whose moons are not modelled", () => {
    const unmodelled = populatedBodiesWith((body) => {
      if (body.kind.type === "dwarf_planet") {
        return { ...body, moons: { state: "not_modelled" }, rings: { state: "not_modelled" } };
      }
      return body.moons.state === "not_modelled"
        ? { ...body, moons: { state: "ok", value: [] }, rings: { state: "ok", value: [] } }
        : body;
    });

    expect(notesOf(unmodelled)).toEqual(["MOONS: NOT YET MODELLED"]);
  });
});
