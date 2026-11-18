import { describe, expect, it } from "vitest";

import {
  ARRIVAL_SETTLE_LIMIT_S,
  ARRIVAL_SETTLE_S,
  type ArrivalFrame,
  ArrivalHolds,
} from "./arrivalHolds";
import { ARRIVAL_HELD_S } from "./scripts";

/**
 * A frame at `spanS` with the field resident or not, streaming or not, holding `patches`, its
 * latest patch made resident at `lastResidentS`.
 */
function at(
  spanS: number,
  fieldResident: boolean,
  streaming = false,
  patches = 10,
  lastResidentS: number | null = null,
): ArrivalFrame {
  return { spanS, fieldResident, streaming, patches, lastResidentS };
}

describe("arrival-memory's holds", () => {
  it("hold the first pose until the field is resident, then stand at the deepest", () => {
    const holds = new ArrivalHolds();
    expect([holds.frame(at(0, false)), holds.deep]).toEqual(["arrival", false]);
    expect([holds.frame(at(30, true)), holds.deep]).toEqual(["deepest", true]);
  });

  it("count the deepest pose resident only once it draws patches without streaming for the window", () => {
    const holds = new ArrivalHolds();
    holds.frame(at(10, true));
    // The field's own frame drew the first pose, so it settles nothing.
    expect(holds.frame(at(10, true))).toBe("deepest");
    holds.frame(at(11, true));
    expect(holds.frame(at(11 + ARRIVAL_SETTLE_S / 2, true, true))).toBe("deepest");
    holds.frame(at(12, true, false, 0));
    holds.frame(at(13, true));
    expect(holds.frame(at(13 + ARRIVAL_SETTLE_S, true))).toBe("held");
  });

  it("wait for the deepest pose's patches to stop being made resident, not streaming alone", () => {
    const holds = new ArrivalHolds();
    holds.frame(at(10, true));
    holds.frame(at(11, true, false, 10, 11));
    expect(holds.frame(at(11 + ARRIVAL_SETTLE_S, true, false, 10, 12.5))).toBe("deepest");
    expect(holds.frame(at(12.5 + ARRIVAL_SETTLE_S, true, false, 10, 12.5))).toBe("held");
  });

  it("hold a deepest pose that has not settled by the limit, so that no run waits for ever", () => {
    const holds = new ArrivalHolds();
    holds.frame(at(10, true));
    expect(holds.frame(at(10 + ARRIVAL_SETTLE_LIMIT_S - 1, true, true))).toBe("deepest");
    expect(holds.frame(at(10 + ARRIVAL_SETTLE_LIMIT_S, true, true))).toBe("held");
  });

  it("fly once the deepest pose has been held its minute, with three stretches", () => {
    const holds = new ArrivalHolds();
    holds.frame(at(10, true));
    holds.frame(at(11, true));
    holds.frame(at(11 + ARRIVAL_SETTLE_S, true));
    const heldFromS = 11 + ARRIVAL_SETTLE_S;
    expect(holds.frame(at(heldFromS + ARRIVAL_HELD_S - 0.5, true))).toBe("held");
    expect(holds.frame(at(heldFromS + ARRIVAL_HELD_S, true))).toBe("flown");
    expect(holds.span()).toEqual({
      spanS: heldFromS + ARRIVAL_HELD_S,
      segments: [
        { name: "arrival", startS: 0, endS: 10 },
        { name: "deepest", startS: 10, endS: heldFromS },
        { name: "held", startS: heldFromS, endS: heldFromS + ARRIVAL_HELD_S },
      ],
    });
  });

  it("have no span before they are flown", () => {
    expect(() => new ArrivalHolds().span()).toThrow("not flown");
  });
});
