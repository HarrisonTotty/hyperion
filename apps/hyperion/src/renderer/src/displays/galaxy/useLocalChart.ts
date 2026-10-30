import type { FeatureIdHex, MassLayer, SystemIdHex, UniverseIdHex } from "@hyperion/protocol";
import { useMemo, useState } from "react";

import type { DisplayMode } from "../../components/ModeControl";
import {
  type CentreLy,
  type ChartFeature,
  type ChartFeatures,
  type ChartResult,
  type ChartSystem,
  NOW_MODE,
  type QueryMode,
} from "../../lib/galaxy/model";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import { useUniverse } from "../../lib/universe";
import type { RequestState } from "../../lib/useServerRequest";
import { type LocalFrame, localFrameAt } from "../../geometry/frame";
import type { SpatialScene } from "../../spatial/marks";
import { vec3 } from "../../geometry/vec3";
import {
  chartDataFault,
  type ChartSelection,
  DEFAULT_DRIVE_RANGE_LY,
  distanceDecimalsFor,
  featureDataFault,
  featureMarkId,
  filterSystems,
  type LayerBand,
  layerBands,
  queryRadiusForDriveRange,
  selectionOfMark,
  type StarFilter,
  toScene,
} from "./chartModel";
import { type Extinction, useExtinction } from "./useExtinction";
import { useFeaturesInRange } from "./useFeaturesInRange";
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
  /**
   * The systems of the answer on show that pass the `STARS` filter, nearest first: what the chart,
   * the list and the HR diagram show. Empty before the first answer.
   */
  readonly systems: ReadonlyArray<ChartSystem>;
  /** Which systems are shown, by what their primary is now. */
  readonly starFilter: StarFilter;
  /** The mass bands of the last census, which label the floors and the legend. */
  readonly bands: ReadonlyArray<LayerBand> | null;
  /** The selected system, or `null` while none is, a feature among them. */
  readonly selected: ChartSystem | null;
  readonly selectedId: SystemIdHex | null;
  /**
   * The features of the chart on show (plan 09, P09.T41.a): an answer for the chart's own centre,
   * radius and time that the client can use, or `null` while there is none, before the first
   * answer, while a newer chart's is awaited, and when it cannot be charted.
   */
  readonly features: ChartFeatures | null;
  /** Where the features' query stands. */
  readonly featureState: RequestState<"features_in_range">;
  /** Why the features' answer cannot be charted, in words, or `null` when nothing is wrong. */
  readonly featureFault: string | null;
  /**
   * The selected feature, or `null` while none is, a system among them: from the features of the
   * chart on show, or while those are awaited, from the answer kept for the chart before.
   */
  readonly selectedFeature: ChartFeature | null;
  /**
   * Whether the selected feature is read from the answer kept for the chart before, while the
   * features of the chart on show are awaited: a snapshot, read as stale.
   */
  readonly selectedFeatureStale: boolean;
  /**
   * What the operator selected, a system or a feature, whether or not the chart on show holds it
   * yet: what the panel's list and its feature's detail follow, so that neither moves while the
   * chart's two answers arrive apart.
   */
  readonly selection: ChartSelection | null;
  /**
   * The extinction from the centre of the chart on show to the selected system, at the chart's
   * time (plan 07, P07.T11.c): `idle` with nothing selected.
   */
  readonly extinction: Extinction;
  /** Asks for the selection's extinction again, after a failure. */
  readonly retryExtinction: () => void;
  /** The radius the operator chose, or `null` while it follows the drive range. */
  readonly radiusChoiceLy: number | null;
  readonly queryRadiusLy: number;
  readonly minLayer: MassLayer;
  readonly driveRangeLy: number;
  readonly timeYr: number;
  /** The mode the operator chose: `now`, or `observed` from {@link LocalChartState.observerLy}. */
  readonly displayMode: DisplayMode;
  /**
   * The observer, kept across a new centre and dropped with the universe; `null` until the first
   * `OBSERVED` takes the centre of the chart on show.
   */
  readonly observerLy: CentreLy | null;
  /** The mode the chart is asked in now; the answer on show carries the one it was asked in. */
  readonly mode: QueryMode;
  /**
   * Why `OBSERVED` is held back, such as `NO CARRIER`, or that there is no chart on show to take an
   * observer from; `null` when it acts.
   */
  readonly observedHeldBack: string | null;
  /**
   * Why the observer cannot be moved to the chart's centre now, the link's reason or no chart on
   * show; `null` when it can.
   */
  readonly observerMoveHeldBack: string | null;
  /** Decimals every distance on this chart is written with. */
  readonly distanceDecimals: number;
  /** Why the chart's controls are held back, such as `NO CARRIER`; `null` when they act. */
  readonly heldBack: string | null;
  readonly select: (id: SystemIdHex) => void;
  /** Selects a feature of the chart on show, in place of any system. */
  readonly selectFeature: (id: FeatureIdHex) => void;
  /** Selects what a mark picked on the chart stands for: a system, or a feature. */
  readonly selectMark: (markId: string) => void;
  /** Asks for the features again, after a failure, without asking for the systems. */
  readonly retryFeatures: () => void;
  readonly chooseRadius: (radiusLy: number) => void;
  readonly chooseMinLayer: (layer: MassLayer) => void;
  readonly chooseStarFilter: (filter: StarFilter) => void;
  readonly setDriveRange: (rangeLy: number) => void;
  readonly setTime: (timeYr: number) => void;
  /**
   * Chooses the mode: the first `OBSERVED` takes the centre of the chart on show as the observer.
   * Does nothing while that choice is held back.
   */
  readonly chooseMode: (mode: DisplayMode) => void;
  /** Moves the observer to the centre of the chart on show; does nothing while held back. */
  readonly moveObserverToCentre: () => void;
  readonly retry: () => void;
}

/** Why `OBSERVED`, or the observer's move, is held back with no chart on show to observe from. */
export const NO_CHART_TO_OBSERVE = "NO CHART: centre one to observe from it";

/** The mode the operator chose and the observer, with the universe they were chosen in. */
interface ModeChoice {
  readonly universe: UniverseIdHex | null;
  readonly displayMode: DisplayMode;
  readonly observerLy: CentreLy | null;
}

/**
 * Owns the local chart: what is asked of the server about the systems around a centre, and what is
 * selected among them.
 *
 * @remarks
 * Called by the display, since the chart's picture and the list and readout beside it stand in
 * different columns and share this state. The query radius follows the drive range until the
 * operator chooses one. The `STARS` filter is the client's own and asks nothing of the server: it
 * picks from the answer on show the systems the chart, the list and the HR diagram show. The
 * selection is derived from those, so a new result or filter keeps it while its system is still
 * shown and hides it otherwise; choosing a filter that shows it again brings it back. The
 * selection's extinction is asked for from the centre of the chart on show at its time (plan 07,
 * P07.T11.c). The scene and the frame keep their identity while nothing they are built from
 * changes, so that an idle chart paints nothing.
 *
 * The features in range are asked for beside the systems, for the same centre, radius and time
 * (plan 09, P09.T41.a), and drawn with the systems of an answer for those alone; one the client
 * cannot use is reported and not drawn. The selection is a system or a feature, never both: picking
 * either on the chart, or in its list, selects it in place of the other, and a feature drops out of
 * the selection as a system does when a new answer no longer holds it.
 *
 * The mode and the observer are the chart's too (plan 12, P12.T11), beside the radius and the time:
 * kept across a new centre and dropped with the universe. The query is asked in the mode, so a
 * changed mode or observer supersedes the one in flight, as a changed radius does. The first
 * `OBSERVED` takes the centre of the chart on show as the observer, and with no chart on show it
 * is held back and says why; the observer moves to that centre on request at any time. Both are
 * held back while the link is down, as every control that asks the server is.
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
  const [chosen, setChosen] = useState<ChartSelection | null>(null);
  const [starFilter, setStarFilter] = useState<StarFilter>("all");
  const [modeChoice, setModeChoice] = useState<ModeChoice | null>(null);
  const [generation, setGeneration] = useState(0);
  const [extinctionGeneration, setExtinctionGeneration] = useState(0);
  const [featureGeneration, setFeatureGeneration] = useState(0);

  const universe = open?.id ?? null;
  // Dropped with the universe it was chosen in, for good: adjusted during render, so that going
  // back to that universe does not bring it back.
  let choice = modeChoice;
  if (choice !== null && choice.universe !== universe) {
    choice = null;
    setModeChoice(null);
  }
  const displayMode = choice?.displayMode ?? "now";
  const observerLy = choice?.observerLy ?? null;
  const mode: QueryMode =
    displayMode === "observed" && observerLy !== null ? { kind: "observed", observerLy } : NOW_MODE;

  const queryRadiusLy = radiusChoiceLy ?? queryRadiusForDriveRange(driveRangeLy);
  const { state, shown: answered } = useRangeQuery({
    universe,
    centreLy,
    queryRadiusLy,
    minLayer,
    timeYr,
    mode,
    generation,
  });

  const featureQuery = useFeaturesInRange({
    universe,
    centreLy,
    queryRadiusLy,
    timeYr,
    generation: generation + featureGeneration,
  });

  // An answer the client cannot use is reported and not shown, as an unusable map is (T8.d).
  const fault = answered === null ? null : chartDataFault(answered);
  const shown = fault === null ? answered : null;
  // The features are drawn with the systems of an answer for the same centre, radius and time,
  // never beside another chart's (plan 09, P09.T41.a).
  const answeredFeatures =
    shown !== null && featureQuery.shown !== null && answersChart(featureQuery.shown, shown)
      ? featureQuery.shown
      : null;
  const featureFault = answeredFeatures === null ? null : featureDataFault(answeredFeatures);
  const features = featureFault === null ? answeredFeatures : null;
  const heldBack = linkDownReason(status);
  const shownCentreLy = shown?.centreLy ?? null;
  let observedHeldBack: string | null = heldBack;
  if (observedHeldBack === null && observerLy === null && shownCentreLy === null) {
    observedHeldBack = NO_CHART_TO_OBSERVE;
  }
  let observerMoveHeldBack: string | null = heldBack;
  if (observerMoveHeldBack === null && shownCentreLy === null) {
    observerMoveHeldBack = NO_CHART_TO_OBSERVE;
  }

  // Filtered once per answer or filter, since the chart's scene is memoised on it.
  const systems = useMemo(
    () => (shown === null ? [] : filterSystems(shown.systems, starFilter)),
    [shown, starFilter],
  );
  const selected =
    chosen?.kind === "system" ? (systems.find((system) => system.id === chosen.id) ?? null) : null;
  const selectedId = selected?.id ?? null;
  // While the features of a new chart are awaited, the selected feature is read from the answer
  // kept for the chart before, a snapshot, so that the selection does not drop out as the chart
  // refreshes; an answer for the chart on show that lacks it drops it, as a system's does.
  const kept = featureQuery.shown;
  const keptFeatures =
    shown !== null && answeredFeatures === null && kept !== null && featureDataFault(kept) === null
      ? kept
      : null;
  const selectedFeature =
    chosen?.kind === "feature"
      ? ((features ?? keptFeatures)?.features.find((feature) => feature.id === chosen.id) ?? null)
      : null;
  const selectedFeatureStale = selectedFeature !== null && features === null;
  // A feature read from the answer before is not drawn, so it brackets nothing.
  const selectedMarkId =
    selectedFeature === null || selectedFeatureStale
      ? selectedId
      : featureMarkId(selectedFeature.id);
  const centre = shown?.centreLy ?? null;
  const extinction = useExtinction({
    universe: open?.id ?? null,
    centreLy: centre,
    system: selectedId,
    timeYr: shown?.timeYr ?? timeYr,
    generation: extinctionGeneration,
  });
  const frame = useMemo(
    () => localFrameAt(centre === null ? vec3(0, 0, 0) : vec3(centre[0], centre[1], centre[2])),
    [centre],
  );
  const scene = useMemo(
    () =>
      shown === null
        ? null
        : toScene(shown, {
            frame,
            driveRangeLy,
            selectedId: selectedMarkId,
            destinationId: null,
            starFilter,
            features: features?.features,
          }),
    [shown, frame, driveRangeLy, selectedMarkId, starFilter, features],
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
    systems,
    starFilter,
    bands,
    selected,
    selectedId,
    features,
    featureState: featureQuery.state,
    featureFault,
    selectedFeature,
    selectedFeatureStale,
    selection: chosen,
    extinction,
    retryExtinction: () => {
      setExtinctionGeneration((count) => count + 1);
    },
    radiusChoiceLy,
    queryRadiusLy,
    minLayer,
    driveRangeLy,
    timeYr,
    displayMode,
    observerLy,
    mode,
    observedHeldBack,
    observerMoveHeldBack,
    distanceDecimals: distanceDecimalsFor(shown?.radiusLy ?? queryRadiusLy),
    heldBack,
    select: (id) => {
      setChosen({ kind: "system", id });
    },
    selectFeature: (id) => {
      setChosen({ kind: "feature", id });
    },
    selectMark: (markId) => {
      setChosen(selectionOfMark(markId));
    },
    retryFeatures: () => {
      setFeatureGeneration((count) => count + 1);
    },
    chooseRadius: setRadiusChoice,
    chooseMinLayer: setMinLayer,
    chooseStarFilter: setStarFilter,
    setDriveRange,
    setTime,
    chooseMode: (next) => {
      if (next === "now") {
        if (heldBack === null) {
          setModeChoice({ universe, displayMode: "now", observerLy });
        }
        return;
      }
      const observer = observerLy ?? shownCentreLy;
      if (observedHeldBack === null && observer !== null) {
        setModeChoice({ universe, displayMode: "observed", observerLy: observer });
      }
    },
    moveObserverToCentre: () => {
      if (observerMoveHeldBack === null && shownCentreLy !== null) {
        setModeChoice({ universe, displayMode, observerLy: shownCentreLy });
      }
    },
    retry: () => {
      setGeneration((count) => count + 1);
    },
  };
}

/** Whether a features answer is for a chart's own centre, radius and time. */
function answersChart(features: ChartFeatures, result: ChartResult): boolean {
  const [ax, ay, az] = features.centreLy;
  const [bx, by, bz] = result.centreLy;
  return (
    ax === bx &&
    ay === by &&
    az === bz &&
    features.radiusLy === result.radiusLy &&
    features.timeYr === result.timeYr
  );
}
