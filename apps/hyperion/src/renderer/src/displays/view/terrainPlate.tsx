import type { ReactNode } from "react";

import {
  type ReadoutLine,
  type TallestTerrainLines,
  type TerrainLabelModel,
  tallestTerrainLines,
} from "../../view/terrain/readouts";
import { SurveyLine } from "../../view/terrain/SurveyLine";
import { TerrainReadout } from "../../view/terrain/TerrainReadout";
import { GROUND_READOUT_OPTIONS } from "./groundReadout";
import type { TerrainLines } from "./useTerrainLines";
import type { LowerPlatePart } from "./viewRoom";

/** No lower plate. */
const NO_PARTS: ReadonlyArray<LowerPlatePart<ReactNode>> = [];

/** The survey's lines as the plate's parts: a reading gives way, a state never does. */
function surveyParts(
  lines: ReadonlyArray<ReadoutLine>,
  tallest: TallestTerrainLines | null,
): LowerPlatePart<ReactNode>[] {
  return lines.map((line): LowerPlatePart<ReactNode> => {
    let part: LowerPlatePart<ReactNode>;
    switch (line.kind) {
      case "reading":
        part = {
          kind: "survey",
          node: <SurveyLine line={line} />,
          tallest: <SurveyLine line={tallest?.survey ?? line} />,
        };
        break;
      case "status":
      case "heading":
        part = { kind: "state", key: line.text, node: <SurveyLine line={line} /> };
        break;
    }
    return part;
  });
}

/**
 * The `PRIMARY` view's lower plate's parts (plan R10, R10.T12; decision-r10-view-block-room §3, the
 * interface of R10.T12.b's `ViewLowerPlate`), in the guide's order: the survey line or its state in
 * its place, a state after it where a survey is held, `DATUM`, then `GROUND AT LINE OF SIGHT` and
 * its readings; none where the view states no terrain lines.
 *
 * @remarks
 * Each part that gives way carries its tallest form (`tallestTerrainLines`), which the room's
 * hidden sizer lays out, so that the plate's room never comes and goes with its readings: every
 * reading Estimated, stale and grazing, and `DATUM` with its `SEA LEVEL`. `DATUM` and the readout
 * are parts only while the readout may stand (`terrainLabelModel`). No part's markup carries an
 * `id`, since a copy of each stands in the sizer.
 */
export function terrainLowerParts(
  lines: TerrainLines | null,
): ReadonlyArray<LowerPlatePart<ReactNode>> {
  if (lines === null) {
    return NO_PARTS;
  }
  const model: TerrainLabelModel = lines.model;
  const tallest =
    lines.header === null
      ? null
      : tallestTerrainLines(lines.header, GROUND_READOUT_OPTIONS.padWidthM);
  const parts = surveyParts(model.survey, tallest);
  if (model.datum !== null) {
    parts.push({
      kind: "datum",
      node: <SurveyLine line={model.datum} />,
      tallest: <SurveyLine line={tallest?.datum ?? model.datum} />,
    });
  }
  if (model.readout.length > 0) {
    parts.push({
      kind: "readout",
      node: <TerrainReadout lines={model.readout} />,
      tallest: <TerrainReadout lines={tallest?.readout ?? model.readout} />,
    });
  }
  return parts;
}
