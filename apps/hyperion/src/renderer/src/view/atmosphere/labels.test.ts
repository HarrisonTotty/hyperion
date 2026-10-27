import { describe, expect, it } from "vitest";

import { substanceName } from "../../lib/substances";
import {
  ATMOSPHERE_ANSWER_UNREADABLE,
  ATMOSPHERE_LABELS,
  ATMOSPHERE_STATEMENTS,
  atmosphereApproximateLines,
  atmosphereDataInvalid,
  type AtmosphereLabel,
  atmosphereNotAvailable,
  atmosphereProblemLines,
  atmosphereNotes,
  atmosphereRejected,
  atmosphereStatements,
} from "./labels";
import { THICK_REASON } from "./thick/regime";

describe("the atmosphere labels", () => {
  it("spell the eight statements as the guide's rows do", () => {
    expect(ATMOSPHERE_STATEMENTS).toEqual({
      atmosphereNotResolved: "ATMOSPHERE: NOT RESOLVED",
      atmosphereNotYetModelled: "ATMOSPHERE: NOT YET MODELLED",
      aerosolsNotYetModelled: "AEROSOLS AND ABSORBERS: NOT YET MODELLED",
      exosphereNotYetModelled: "EXOSPHERE: NOT YET MODELLED",
      silicateVapourNotYetModelled: "SILICATE VAPOUR: NOT YET MODELLED",
      atmospherePending: "ATMOSPHERE: PENDING",
      atmosphereComputing: "ATMOSPHERE: COMPUTING",
      atmosphereApproximate: "ATMOSPHERE: APPROXIMATE",
    });
  });

  it("spell the fault as the guide's ATMOSPHERE NOT AVAILABLE row does, naming the body", () => {
    expect(atmosphereNotAvailable(["TEST PLANET"])).toBe(
      "ATMOSPHERE NOT AVAILABLE: computation failed for TEST PLANET, reopen VIEW to retry",
    );
  });

  it("name the bodies before the server's reason for a refused record", () => {
    expect(atmosphereRejected(["TEST MOON"], "unknown body")).toBe(
      "ATMOSPHERE REJECTED: for TEST MOON, unknown body, reopen VIEW to retry",
    );
  });

  it("leave the remedy alone when the server gives no reason", () => {
    expect(atmosphereRejected(["TEST MOON"], "  ")).toBe(
      "ATMOSPHERE REJECTED: for TEST MOON, reopen VIEW to retry",
    );
  });

  it("name the body after the check its unreadable record failed", () => {
    expect(atmosphereDataInvalid(["TEST MOON"], ATMOSPHERE_ANSWER_UNREADABLE)).toBe(
      "ATMOSPHERE DATA INVALID: answer unreadable for TEST MOON, reopen VIEW to retry",
    );
  });

  it("join three designations as the guide joins names in one line", () => {
    expect(atmosphereNotAvailable(["TEST PLANET", "TEST MOON", "TEST GIANT"])).toBe(
      "ATMOSPHERE NOT AVAILABLE: computation failed for TEST PLANET, TEST MOON and TEST GIANT, " +
        "reopen VIEW to retry",
    );
  });

  it("refuse a fault that names no body", () => {
    expect(() => atmosphereNotAvailable([])).toThrow(RangeError);
  });

  it("list every label once", () => {
    expect(new Set(ATMOSPHERE_LABELS).size).toBe(ATMOSPHERE_LABELS.length);
    expect(ATMOSPHERE_LABELS.toSorted()).toEqual(Object.keys(ATMOSPHERE_STATEMENTS).toSorted());
  });
});

describe("atmosphereNotes (R08.T10.b)", () => {
  it("sets the REJECTED lines after NOT RESOLVED and before the annunciations, each with its standing", () => {
    const refused = {
      statement: "ATMOSPHERE REJECTED: for A, refused, reopen VIEW to retry",
      refusal: true,
    };
    const failed = {
      statement: "ATMOSPHERE REJECTED: for B, failed, reopen VIEW to retry",
      refusal: false,
    };
    expect(
      atmosphereNotes(
        [
          "atmosphereApproximate",
          "atmosphereNotResolved",
          "atmosphereNotYetModelled",
          "atmospherePending",
        ],
        [refused, failed],
        [],
      ),
    ).toEqual([
      { statement: "ATMOSPHERE: NOT YET MODELLED", standing: "note" },
      { statement: "ATMOSPHERE: NOT RESOLVED", standing: "note" },
      { statement: refused.statement, standing: "refused" },
      { statement: failed.statement, standing: "fault" },
      { statement: "ATMOSPHERE: PENDING", standing: "note" },
      { statement: "ATMOSPHERE: APPROXIMATE", standing: "note" },
    ]);
  });

  it("gives the labels' statements alone where nothing is refused", () => {
    expect(
      atmosphereNotes(["aerosolsNotYetModelled"], [], []).map((note) => note.statement),
    ).toEqual(atmosphereStatements(["aerosolsNotYetModelled"]));
  });
});

describe("atmosphereNotes with APPROXIMATE's reasons (R08.T10.c)", () => {
  const reasons = [
    "H2O: its absorption estimated, not measured, for KX 4417 /3",
    "A thick atmosphere drawn by the thin-atmosphere method, for KX 4417 /2 and KX 4417 /5",
  ];

  it("sets the reasons straight after ATMOSPHERE: APPROXIMATE, each a reason and the note a note, the REJECTED lines in their place (R08.T10.d)", () => {
    const failed = {
      statement: "ATMOSPHERE REJECTED: for KX 4417 /4, failed, reopen VIEW to retry",
      refusal: false,
    };
    expect(
      atmosphereNotes(
        ["atmosphereApproximate", "aerosolsNotYetModelled", "atmosphereNotResolved"],
        [failed],
        reasons,
      ),
    ).toEqual([
      { statement: "AEROSOLS AND ABSORBERS: NOT YET MODELLED", standing: "note" },
      { statement: "ATMOSPHERE: NOT RESOLVED", standing: "note" },
      { statement: failed.statement, standing: "fault" },
      { statement: "ATMOSPHERE: APPROXIMATE", standing: "note" },
      { statement: reasons[0], standing: "reason" },
      { statement: reasons[1], standing: "reason" },
    ]);
  });

  it("sets none without ATMOSPHERE: APPROXIMATE", () => {
    expect(
      atmosphereNotes(["aerosolsNotYetModelled", "atmospherePending"], [], reasons).map(
        (note) => note.statement,
      ),
    ).toEqual(["AEROSOLS AND ABSORBERS: NOT YET MODELLED", "ATMOSPHERE: PENDING"]);
  });
});

describe("atmosphereApproximateLines (R08.T10.c)", () => {
  it("words the ruling's lines: a reason, then the bodies it holds for, one, two or three", () => {
    expect(
      atmosphereApproximateLines([
        { designation: "KX 4417 /3", reasons: ["H2O: its absorption estimated, not measured"] },
        { designation: "KX 4417 /5", reasons: [THICK_REASON] },
        { designation: "KX 4417 /2", reasons: [THICK_REASON] },
        {
          designation: "TEST SATURN",
          reasons: [
            `${substanceName("e-")} scattering: left out, the console can draw no more layers`,
          ],
        },
      ]),
    ).toEqual([
      "A thick atmosphere drawn by the thin-atmosphere method, for KX 4417 /2 and KX 4417 /5",
      "H2O: its absorption estimated, not measured, for KX 4417 /3",
      "e− scattering: left out, the console can draw no more layers, for TEST SATURN",
    ]);
    expect(
      atmosphereApproximateLines(
        ["KX 4417 /5", "KX 4417 /2", "KX 4417 /3"].map((designation) => ({
          designation,
          reasons: [THICK_REASON],
        })),
      ),
    ).toEqual([
      "A thick atmosphere drawn by the thin-atmosphere method, for KX 4417 /2, KX 4417 /3 and KX 4417 /5",
    ]);
  });

  it("names the bodies in designation order, numbers by their value", () => {
    expect(
      atmosphereApproximateLines(
        ["Kepler-9 /10", "Kepler-9 /2"].map((designation) => ({
          designation,
          reasons: [THICK_REASON],
        })),
      ),
    ).toEqual([
      "A thick atmosphere drawn by the thin-atmosphere method, for Kepler-9 /2 and Kepler-9 /10",
    ]);
  });

  it("gives a reason two bodies share one line, and names a body listed twice once", () => {
    const water = "H2O: its absorption estimated, not measured";
    expect(
      atmosphereApproximateLines([
        { designation: "B", reasons: [water] },
        { designation: "A", reasons: [water] },
        { designation: "B", reasons: [water] },
      ]),
    ).toEqual(["H2O: its absorption estimated, not measured, for A and B"]);
  });

  it("orders the lines by their first bodies, a body's own reasons in its order", () => {
    expect(
      atmosphereApproximateLines([
        { designation: "B", reasons: ["CH4: second", "CO: first"] },
        { designation: "A", reasons: ["O3: its own", "CO: first", "CH4: second"] },
        { designation: "C", reasons: ["N2O: last"] },
      ]),
    ).toEqual([
      "O3: its own, for A",
      "CO: first, for A and B",
      "CH4: second, for A and B",
      "N2O: last, for C",
    ]);
  });

  it("capitalises a reason that begins with a word, and keeps a formula's or a symbol's case", () => {
    const lines = atmosphereApproximateLines([
      {
        designation: "A",
        reasons: [
          "liquid H2SO4 particles: their sizes summed short of convergence at 450 nm",
          "no SO2 ice on file: liquid SO2 at 198 K stands in, a named analogue",
          "H2O–air pairs: their absorption estimated, not measured",
          `${substanceName("e-")} scattering: left out, the console can draw no more layers`,
          `${substanceName("ice_giant_haze")}: its phase function a named analogue's`,
          `${substanceName("mars_dust")}: its phase function a named analogue's`,
        ],
      },
    ]);
    expect(lines.map((line) => line.split(" ")[0])).toEqual([
      "Liquid",
      "No",
      "H2O–air",
      "e−",
      "Ice-giant",
      "Mars-type",
    ]);
  });

  it("compares reasons after the capital, so that one worded either way gives one line", () => {
    expect(
      atmosphereApproximateLines([
        { designation: "A", reasons: [THICK_REASON] },
        { designation: "B", reasons: [`A${THICK_REASON.slice(1)}`] },
      ]),
    ).toEqual(["A thick atmosphere drawn by the thin-atmosphere method, for A and B"]);
  });

  it("gives no line for a body with no reasons, and none for no bodies", () => {
    expect([
      atmosphereApproximateLines([{ designation: "A", reasons: [] }]),
      atmosphereApproximateLines([]),
    ]).toEqual([[], []]);
  });
});

describe("atmosphereProblemLines", () => {
  it("gives two bodies failing alike one line naming both, in designation order", () => {
    const lines = atmosphereProblemLines([
      { designation: "Kepler-9 /10", problem: { kind: "failed" } },
      { designation: "Kepler-9 /2", problem: { kind: "failed" } },
    ]);
    expect(lines.fault).toEqual([
      "ATMOSPHERE NOT AVAILABLE: computation failed for Kepler-9 /2 and Kepler-9 /10, " +
        "reopen VIEW to retry",
    ]);
  });

  it("gives each distinct reason a line of its own, coloured by whether it was a refusal", () => {
    const lines = atmosphereProblemLines([
      {
        designation: "TEST PLANET",
        problem: { kind: "rejected", code: "internal", message: "store offline", refusal: false },
      },
      {
        designation: "TEST MOON",
        problem: { kind: "rejected", code: "unknown_body", message: "unknown body", refusal: true },
      },
    ]);
    expect(lines.plate).toEqual([
      {
        statement: "ATMOSPHERE REJECTED: for TEST MOON, unknown body, reopen VIEW to retry",
        refusal: true,
      },
      {
        statement: "ATMOSPHERE REJECTED: for TEST PLANET, store offline, reopen VIEW to retry",
        refusal: false,
      },
    ]);
  });

  it("puts the failed computations' line before the unreadable records' on the fault line", () => {
    const lines = atmosphereProblemLines([
      { designation: "A", problem: { kind: "invalid", check: ATMOSPHERE_ANSWER_UNREADABLE } },
      { designation: "B", problem: { kind: "failed" } },
    ]);
    expect(lines.fault).toEqual([
      "ATMOSPHERE NOT AVAILABLE: computation failed for B, reopen VIEW to retry",
      "ATMOSPHERE DATA INVALID: answer unreadable for A, reopen VIEW to retry",
    ]);
  });

  it("gives a kept scene's failed load relaunch to retry, naming every body with air (decision-r08-t10b-ux §4)", () => {
    const lines = atmosphereProblemLines(
      ["TEST SATURN", "TEST EARTH", "TEST MARS", "TEST EARTH O3 SLAB"].map((designation) => ({
        designation,
        problem: { kind: "loadFailed" } as const,
      })),
    );
    expect(lines.fault).toEqual([
      "ATMOSPHERE NOT AVAILABLE: computation failed for TEST EARTH, TEST EARTH O3 SLAB, TEST MARS " +
        "and TEST SATURN, relaunch to retry",
    ]);
  });

  it("states nothing for a view with no problems", () => {
    expect(atmosphereProblemLines([])).toEqual({ plate: [], fault: [] });
  });
});

describe("atmosphereStatements", () => {
  it("states nothing for bodies that carry no label", () => {
    expect(atmosphereStatements([])).toEqual([]);
  });

  it("states a label two bodies carry once", () => {
    expect(atmosphereStatements(["atmosphereNotYetModelled", "atmosphereNotYetModelled"])).toEqual([
      "ATMOSPHERE: NOT YET MODELLED",
    ]);
  });

  it("puts NOT RESOLVED after the NOT YET MODELLED notes and the annunciations last", () => {
    const labels: ReadonlyArray<AtmosphereLabel> = [
      "atmosphereApproximate",
      "atmospherePending",
      "atmosphereNotResolved",
      "atmosphereComputing",
      "silicateVapourNotYetModelled",
      "exosphereNotYetModelled",
      "aerosolsNotYetModelled",
      "atmosphereNotYetModelled",
    ];
    expect(atmosphereStatements(labels)).toEqual([
      "ATMOSPHERE: NOT YET MODELLED",
      "AEROSOLS AND ABSORBERS: NOT YET MODELLED",
      "EXOSPHERE: NOT YET MODELLED",
      "SILICATE VAPOUR: NOT YET MODELLED",
      "ATMOSPHERE: NOT RESOLVED",
      "ATMOSPHERE: PENDING",
      "ATMOSPHERE: COMPUTING",
      "ATMOSPHERE: APPROXIMATE",
    ]);
  });
});
