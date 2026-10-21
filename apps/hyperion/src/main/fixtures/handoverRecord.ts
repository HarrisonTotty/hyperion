/**
 * A hand-over check's record (plan R10, R10.T10.g) that passes its verdict, on the high setting:
 * the main process's tests of its reader, its results and its session start from it and change
 * one thing each.
 */
import type {
  HandoverComparisonRecord,
  HandoverFrameRecord,
  HandoverLookRecord,
  HandoverSwitchName,
  HandoverCheckRecord,
  HandoverTexelRecord,
} from "../../preload/api";

/** A frame read back at `scriptS`, drawn in `regime`. */
function frame(
  scriptS: number,
  regime: HandoverFrameRecord["regime"],
  rangeRatio: number,
): HandoverFrameRecord {
  return {
    atMs: 10_000 + scriptS * 1000,
    scriptS,
    regime,
    rangeRatio,
    target: { widthPx: 1126, heightPx: 906 },
    bodyPixels: 44_000,
    bodyMisses: 300,
    surveyPixels: 300,
    angles: { incidenceDeg: 45, emissionDeg: 45.4, phaseDeg: 90.4 },
  };
}

/** Three cells: two of several shared pixels a little apart, and one of a single pixel. */
const TEXELS: ReadonlyArray<HandoverTexelRecord> = [
  { texel: 19_200, sharedPixels: 6, meanBefore: 0.2, meanAfter: 0.21 },
  { texel: 19_201, sharedPixels: 5, meanBefore: 0.18, meanAfter: 0.175 },
  { texel: 19_265, sharedPixels: 1, meanBefore: 0.2, meanAfter: 0.4 },
];

/** A switch within the bound everywhere. */
export function passingComparison(
  name: HandoverSwitchName,
  before: HandoverFrameRecord,
  after: HandoverFrameRecord,
): HandoverComparisonRecord {
  return {
    name,
    before,
    after,
    framesApart: 1,
    beforeWasLast: true,
    afterWasFirst: true,
    body: {
      own: { pixelsBefore: 44_000, pixelsAfter: 44_100, meanBefore: 0.19, meanAfter: 0.191 },
      shared: { pixels: 43_900, meanBefore: 0.19, meanAfter: 0.1905 },
    },
    survey: {
      own: { pixelsBefore: 300, pixelsAfter: 302, meanBefore: 0.19, meanAfter: 0.192 },
      shared: { pixels: 290, meanBefore: 0.19, meanAfter: 0.191 },
    },
    texels: TEXELS,
  };
}

/** A look whose three switches pass. */
export function passingLook(
  name: HandoverLookRecord["name"],
  scene: string,
  judged: boolean,
): HandoverLookRecord {
  return {
    name,
    scene,
    judged,
    nominal: { incidenceDeg: 45, emissionDeg: 45, phaseDeg: 90 },
    atRange: { incidenceDeg: 45, emissionDeg: 45, phaseDeg: 90 },
    exposure: {
      lockedEv100: 13.5,
      lockedReading: "EV100 13.5 INHIBITED · OPERATOR",
      endReading: "EV100 13.5 INHIBITED · OPERATOR",
      held: true,
    },
    surveyedCells: 48,
    mappingMismatches: 0,
    switchesS: { figure: 116.0, terrain: 116.6, disc: 149.2 },
    comparisons: [
      passingComparison(
        "disc-to-figure",
        frame(115.98, "disc", 0.9092),
        frame(116.0, "figure", 0.909),
      ),
      passingComparison(
        "disc-to-terrain",
        frame(115.98, "disc", 0.9092),
        frame(116.6, "terrain", 0.9036),
      ),
      passingComparison(
        "terrain-to-disc",
        frame(149.18, "terrain", 1.111),
        frame(149.2, "disc", 1.1112),
      ),
    ],
    failures: [],
  };
}

/** The passing record. */
export const HANDOVER_RECORD: HandoverCheckRecord = {
  world: "TERRAIN TEST",
  devicePixelRatio: 1,
  canvas: { widthPx: 1126, heightPx: 906 },
  target: { widthPx: 1126, heightPx: 906 },
  reliefM: 15_812,
  tauPx: 1,
  radPerPx: 1.0255e-3,
  rangeM: 1.5419e7,
  cellLevel: 6,
  approach: { ratePerS: 0.01, startRatio: 3, closestRatio: 0.85, holdS: 12, endRatio: 1.25 },
  looks: [
    passingLook("handover-30", "HAND-OVER 30°", true),
    passingLook("handover-90", "HAND-OVER 90°", true),
    passingLook("handover-terminator", "HAND-OVER TERMINATOR", false),
  ],
  faults: [],
};
