import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ViewLabelBlock } from "../ViewLabelBlock";
import type { LabelStatement } from "../viewRun";
import { primaryPlate } from "./atmosphereCheckRun";

/** The primary's canvas described by its label block, as `VIEW` sets them. */
function primaryWith(statements: ReadonlyArray<LabelStatement>, faults: ReadonlyArray<string>) {
  return render(
    <>
      <canvas
        // `VIEW`'s canvas's own role (`ViewCanvas`): the rule counts a canvas as interactive, which
        // HTML does not.
        // oxlint-disable-next-line jsx-a11y/no-interactive-element-to-noninteractive-role
        role="application"
        aria-label="VIEW, PHOTOREALISTIC, PRIMARY, SEAT"
        aria-describedby="view-legend-label view-legend"
      />
      <ViewLabelBlock
        id="view-legend-label"
        lines={[
          { label: "CAMERA", value: "FREE · RATE 1.00 km/s" },
          { label: "EXPOSURE", value: "EV100 -1.0 AUTO" },
        ]}
        statements={statements}
        countLine={null}
        faults={faults}
      />
    </>,
  );
}

describe("the atmosphere run's reading of the primary's label block (R08.T10.b)", () => {
  it("reads its lines, statements and fault lines as the operator reads them", () => {
    primaryWith(
      ["ROTATION: NOT YET MODELLED", "ATMOSPHERE: PENDING"],
      ["ATMOSPHERE NOT AVAILABLE: computation failed for TEST MARS, reopen VIEW to retry"],
    );
    const plate = primaryPlate(document);
    expect([
      plate?.line("CAMERA"),
      plate?.line("EXPOSURE"),
      plate?.line("FOV"),
      plate?.statements,
      plate?.faults,
    ]).toEqual([
      "FREE · RATE 1.00 km/s",
      "EV100 -1.0 AUTO",
      null,
      ["ROTATION: NOT YET MODELLED", "ATMOSPHERE: PENDING"],
      ["ATMOSPHERE NOT AVAILABLE: computation failed for TEST MARS, reopen VIEW to retry"],
    ]);
  });

  it("reads a request's status as a statement, though it is set as a status line", () => {
    const rejected = "ATMOSPHERE REJECTED: for TEST MARS, internal error, reopen VIEW to retry";
    primaryWith(["ATMOSPHERE: NOT RESOLVED", { text: rejected, standing: "fault" }], []);
    const plate = primaryPlate(document);
    expect([plate?.statements, plate?.faults]).toEqual([
      ["ATMOSPHERE: NOT RESOLVED", rejected],
      [],
    ]);
  });

  it("is none while no primary is shown", () => {
    expect(primaryPlate(document)).toBeNull();
  });
});
