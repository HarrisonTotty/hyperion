import type { MassLayer, SystemIdHex } from "@hyperion/protocol";
import { useMemo, useState } from "react";

import type { CentreLy, ChartResult, ChartSystem } from "../../lib/galaxy/model";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import { useUniverse } from "../../lib/universe";
import type { RequestState } from "../../lib/useServerRequest";
import { type LocalFrame, localFrameAt } from "../../spatial/frame";
import type { SpatialScene } from "../../spatial/marks";
import { vec3 } from "../../spatial/vec3";
import {
  chartDataFault,
  DEFAULT_DRIVE_RANGE_LY,
  distanceDecimalsFor,
  type LayerBand,
  layerBands,
  queryRadiusForDriveRange,
  toScene,
} from "./chartModel";
import { useRangeQuery } from "./useRangeQuery";

/** Everything the `LOCAL CHART` panel and the panels beside it are drawn from. */
export interface LocalChartState {
  /** The answer on show, or `null` before the first one arrives or when it cannot be charted. */
  readonly result: ChartResult | null;
  readonly state: RequestState<"systems_in_range">;
  /** Why the answer cannot be charted, in words, or `null` when there is nothing wrong with it. */
  readonly fault: string | null;
  /** Whether the answer on show is no longer backed by a link, so the chart is a stale snapshot. */
  readonly stale: boolean;
  /** Whether a chart centre has been chosen at all. */
  readonly hasCentre: boolean;
  /** The named directions at the centre of the answer on show. */
  readonly frame: LocalFrame;
  /** What the spatial view draws, or `null` before the first answer. */
  readonly scene: SpatialScene | null;
  /** The mass bands of the last census, which label the floors and the legend. */
  readonly bands: ReadonlyArray<LayerBand> | null;
  readonly selected: ChartSystem | null;
  readonly selectedId: SystemIdHex | null;
  /** The radius the operator chose, or `null` while it follows the drive range. */
  readonly radiusChoiceLy: number | null;
  readonly queryRadiusLy: number;
  readonly minLayer: MassLayer;
  readonly driveRangeLy: number;
  readonly timeYr: number;
  /** Decimals every distance on this chart is written with. */
  readonly distanceDecimals: number;
  /** Why the chart's controls are held back, such as `NO CARRIER`; `null` when they act. */
  readonly heldBack: string | null;
  readonly select: (id: SystemIdHex) => void;
  readonly chooseRadius: (radiusLy: number) => void;
  readonly chooseMinLayer: (layer: MassLayer) => void;
  readonly setDriveRange: (rangeLy: number) => void;
  readonly setTime: (timeYr: number) => void;
  readonly retry: () => void;
}

/**
 * Owns the local chart: what is asked of the server about the systems around a centre, and what is
 * selected among them.
 *
 * @remarks
 * Called by the display, since the chart's picture and the list and readout beside it stand in
 * different columns and share this state. The query radius follows the drive range until the
 * operator chooses one. The selection is derived from the answer on show, so a new result keeps it
 * while its system is still there and drops it otherwise. The scene and the frame keep their
 * identity while nothing they are built from changes, so that an idle chart paints nothing.
 *
 * @param centreLy - The chart centre, which `CENTRE CHART` publishes; `null` until it does.
 */
export function useLocalChart(centreLy: CentreLy | null): LocalChartState {
  const { open } = useUniverse();
  const { status } = useServerLink();
  const [radiusChoiceLy, setRadiusChoice] = useState<number | null>(null);
  const [minLayer, setMinLayer] = useState<MassLayer>("a");
  const [driveRangeLy, setDriveRange] = useState(DEFAULT_DRIVE_RANGE_LY);
  const [timeYr, setTime] = useState(0);
  const [chosenId, setChosenId] = useState<SystemIdHex | null>(null);
  const [generation, setGeneration] = useState(0);

  const queryRadiusLy = radiusChoiceLy ?? queryRadiusForDriveRange(driveRangeLy);
  const { state, shown: answered } = useRangeQuery({
    universe: open?.id ?? null,
    centreLy,
    queryRadiusLy,
    minLayer,
    timeYr,
    generation,
  });

  // An answer the client cannot use is reported and not shown, as an unusable map is (T8.d).
  const fault = answered === null ? null : chartDataFault(answered);
  const shown = fault === null ? answered : null;
  const heldBack = linkDownReason(status);

  const selected = shown?.systems.find((system) => system.id === chosenId) ?? null;
  const selectedId = selected?.id ?? null;
  const centre = shown?.centreLy ?? null;
  const frame = useMemo(
    () => localFrameAt(centre === null ? vec3(0, 0, 0) : vec3(centre[0], centre[1], centre[2])),
    [centre],
  );
  const scene = useMemo(
    () =>
      shown === null
        ? null
        : toScene(shown, { frame, driveRangeLy, selectedId, destinationId: null }),
    [shown, frame, driveRangeLy, selectedId],
  );
  const bands = useMemo(() => (shown === null ? null : layerBands(shown.layers)), [shown]);

  return {
    result: shown,
    state,
    fault,
    // The link's loss makes the answer on show a snapshot with nothing behind it, which the guide
    // has read as stale, as a kept map picture does (T10, the orchestrator's ruling 3).
    stale: heldBack !== null && shown !== null,
    hasCentre: centreLy !== null,
    frame,
    scene,
    bands,
    selected,
    selectedId,
    radiusChoiceLy,
    queryRadiusLy,
    minLayer,
    driveRangeLy,
    timeYr,
    distanceDecimals: distanceDecimalsFor(shown?.radiusLy ?? queryRadiusLy),
    heldBack,
    select: setChosenId,
    chooseRadius: setRadiusChoice,
    chooseMinLayer: setMinLayer,
    setDriveRange,
    setTime,
    retry: () => {
      setGeneration((count) => count + 1);
    },
  };
}
