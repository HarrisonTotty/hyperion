import { describe, expect, it } from "vitest";

import { keptBody } from "../../scenes/kept";
import { answerRequest, HeldFields, type HeightModule } from "./heightBake";

/** A module the field messages never call: they hold blocks in JavaScript until R10.T6.b. */
const UNCALLED: HeightModule = {
  bakePatch: () => {
    throw new Error("a field message baked");
  },
  testPlanetVersion: () => 2,
};

const MOON = keptBody(1);
const PLANET = keptBody(2);

describe("a height worker's fields", () => {
  it("holds each body's blocks from its field-init and answers with what it holds", () => {
    const fields = new HeldFields();
    const block = new ArrayBuffer(8);
    expect(
      answerRequest(UNCALLED, { kind: "field-init", id: 1, body: MOON, block }, fields),
    ).toEqual({ reply: { kind: "field-held", id: 1, heldBytes: 8 }, transfer: [] });
    answerRequest(
      UNCALLED,
      { kind: "field-chunk", id: 2, body: MOON, block: new ArrayBuffer(4) },
      fields,
    );
    const answer = answerRequest(
      UNCALLED,
      { kind: "field-init", id: 3, body: PLANET, block: new ArrayBuffer(2) },
      fields,
    );
    expect(answer.reply).toEqual({ kind: "field-held", id: 3, heldBytes: 14 });
    expect(fields.bodies).toEqual([MOON, PLANET]);
  });

  it("refuses a block of a body with no field-init, holding nothing more", () => {
    const fields = new HeldFields();
    const answer = answerRequest(
      UNCALLED,
      { kind: "field-chunk", id: 4, body: MOON, block: new ArrayBuffer(8) },
      fields,
    );
    expect(answer.reply).toMatchObject({
      kind: "field-refused",
      id: 4,
      body: MOON,
      heldBytes: 0,
    });
    expect(fields.bodies).toEqual([]);
  });

  it("drops a body's field on its release and at a second field-init", () => {
    const fields = new HeldFields();
    fields.init(MOON, new ArrayBuffer(8));
    fields.chunk(MOON, new ArrayBuffer(8));
    fields.init(MOON, new ArrayBuffer(3));
    expect(fields.heldBytes).toBe(3);
    const answer = answerRequest(UNCALLED, { kind: "field-release", id: 5, body: MOON }, fields);
    expect(answer.reply).toEqual({ kind: "field-held", id: 5, heldBytes: 0 });
    expect(fields.bodies).toEqual([]);
  });
});
