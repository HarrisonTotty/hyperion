import { useLayoutEffect, useRef } from "react";

import {
  AU_PER_LY,
  formatBearingDeg,
  formatLengthLy,
  formatScaleLength,
  formatSigned,
  formatUniverseTimeYr,
  SCALE_AU_BELOW_LY,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import { StatusLine } from "../../components/StatusLine";
import { AXIS_TOLERANCE_LY, cylindrical } from "../../spatial/frame";
import type { SpatialQuantity, SpatialReading } from "../../spatial/Reading";
import type { ScaleUnit } from "../../spatial/scale";
import { SpatialView } from "../../spatial/SpatialView";
import { vec3 } from "../../spatial/vec3";
import { CensusReadout } from "./CensusReadout";
import { ChartControls } from "./ChartControls";
import { usePageTabFocus } from "./pageTabFocus";
import { SymbolLegend } from "./SymbolLegend";
import type { LocalChartState } from "./useLocalChart";

/**
 * The chart's unit ladder: light-years, then astronomical units below 0.01 ly, where a 1-2-5 step
 * in light-years would read 0.005 ly (plan 05, design note D21).
 */
const SCALE_UNITS: ReadonlyArray<ScaleUnit> = [
  { perSceneUnit: 1, minSceneLength: SCALE_AU_BELOW_LY },
  { perSceneUnit: AU_PER_LY, minSceneLength: 0 },
];

/** Room for the longest value of each of the centre's coordinates, in characters. */
const RADIUS_WIDTH_CH = 9;
const ANGLE_WIDTH_CH = 6;
const HEIGHT_WIDTH_CH = 9;
const TIME_WIDTH_CH = 9;

/** The precision the centre's radius is read at, which the core arrow's distance shares. */
const CENTRE_DECIMALS = 1;

interface LocalChartPanelProps {
  readonly chart: LocalChartState;
}

/**
 * The `LOCAL CHART` page: a rotatable 3D chart of the systems around the chosen centre, with what
 * the chart is asked for and the census of what came back.
 *
 * @remarks
 * A page of the panel it shares with `PARAMETERS` and `GALAXY MAP`, since the chart is the display's
 * other large picture and needs that column's width and height; the list and the readout it is
 * paired with stand in the column beside it. The scene is the query's answer, and the circles
 * measure the answer's own radius, never what was asked for. The view is handed its furniture: the
 * frame name `GALACTIC`, the centre as `RADIUS`, `ANGLE` and `HEIGHT`, and the chart time in
 * universe time. The core arrow's distance is the centre's `RADIUS` written the same way, so that one
 * quantity does not carry two precisions. Under the chart stand what it was asked for, the census of
 * what came back, and the legend, each on as few lines as it can be, so that the picture keeps the
 * height. With no centre chosen the page says how to choose one, and an answer the client cannot
 * chart reads as a fault with `RETRY`, as an unusable map does. That `RETRY` stays on show while
 * the query it sends is pending and goes once an answer the chart can use arrives; the focus it
 * held then goes to the page's tab. An answer the link no longer backs is drawn and read as stale,
 * in `--text-muted` with a trailing `S`.
 */
export function LocalChartPanel({ chart }: LocalChartPanelProps) {
  const { result, scene, bands, state, hasCentre, driveRangeLy, heldBack, fault, stale } = chart;
  const focusPageTab = usePageTabFocus();
  // The fault is the last answer's, so its RETRY stays on show while the query it sends is pending,
  // and goes with the fault once an answer the chart can use arrives. Had it the focus then, the
  // focus is left on nothing; before paint it goes to the page's tab (the orchestrator's ruling
  // 18).
  const faultShown = useRef(fault !== null);
  useLayoutEffect(() => {
    const cleared = faultShown.current && fault === null;
    faultShown.current = fault !== null;
    const focused = document.activeElement;
    if (cleared && (focused === null || focused === document.body)) {
      focusPageTab?.();
    }
  }, [fault, focusPageTab]);
  const centre = result === null ? null : cylindrical(vec3(...result.centreLy));
  const onAxis = centre !== null && !(centre.radiusLy > AXIS_TOLERANCE_LY);
  const radiusText = centre === null ? null : formatLengthLy(centre.radiusLy, CENTRE_DECIMALS);
  const centreReadings: ReadonlyArray<SpatialReading> =
    centre === null
      ? []
      : [
          { label: "RADIUS", value: radiusText, unit: "ly", widthCh: RADIUS_WIDTH_CH },
          {
            label: "ANGLE",
            value: onAxis ? null : formatBearingDeg(centre.angleDeg, CENTRE_DECIMALS),
            unit: "",
            widthCh: ANGLE_WIDTH_CH,
          },
          {
            label: "HEIGHT",
            value: formatSigned(centre.heightLy, CENTRE_DECIMALS),
            unit: "ly",
            widthCh: HEIGHT_WIDTH_CH,
          },
        ];
  const time: SpatialReading = {
    label: TIME_SYSTEM_LABEL,
    value: result === null ? null : formatUniverseTimeYr(result.timeYr),
    unit: "yr",
    widthCh: TIME_WIDTH_CH,
  };
  const coreDistance: SpatialQuantity = { value: radiusText ?? "", unit: "ly" };

  return (
    <div className={stale ? "local-chart local-chart--stale" : "local-chart"}>
      {hasCentre ? null : (
        <p className="panel__empty local-chart__empty">NO CENTRE: pick on the map and press C</p>
      )}
      {fault === null ? null : (
        <StatusLine
          text={`CHART DATA INVALID: ${fault}`}
          standing="fault"
          action={{ label: "RETRY", onAction: chart.retry }}
        />
      )}
      {scene === null || result === null ? null : (
        <SpatialView
          scene={scene}
          fitRadius={result.radiusLy}
          formatLength={formatScaleLength}
          scaleUnits={SCALE_UNITS}
          frameName="GALACTIC"
          centre={centreReadings}
          time={time}
          coreDistance={coreDistance}
          accessibleName="Local chart"
          stale={stale}
          onSelect={chart.select}
        />
      )}
      <ChartControls
        radiusChoiceLy={chart.radiusChoiceLy}
        queryRadiusLy={chart.queryRadiusLy}
        onQueryRadius={chart.chooseRadius}
        minLayer={chart.minLayer}
        onMinLayer={chart.chooseMinLayer}
        driveRangeLy={driveRangeLy}
        onDriveRange={chart.setDriveRange}
        timeYr={chart.timeYr}
        onTime={chart.setTime}
        bands={bands}
        heldBack={heldBack}
      />
      <CensusReadout
        result={result}
        state={state}
        driveRangeLy={driveRangeLy}
        stale={stale}
        onRetry={chart.retry}
      />
      <SymbolLegend bands={bands} />
    </div>
  );
}
