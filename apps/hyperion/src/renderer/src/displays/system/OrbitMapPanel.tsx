import { type ReactNode, useEffect, useId, useMemo } from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import {
  formatBearingDeg,
  formatLengthLy,
  formatSigned,
  formatUniverseTimeDhms,
} from "../../lib/format";
import { isTextEntry } from "../../lib/textEntry";
import { AXIS_TOLERANCE_LY, cylindrical, localFrameAt } from "../../spatial/frame";
import type { SpatialQuantity, SpatialReading } from "../../spatial/Reading";
import { SpatialView } from "../../spatial/SpatialView";
import { bodySymbol } from "../../lib/system/bodySymbols";
import { bodyKindLabel } from "../../lib/system/bodyWords";
import type { SpatialScene } from "../../spatial/marks";
import type { ZoneLayers } from "./bodyMap";
import { DISPLAY_TIME_LABEL, DISPLAY_TIME_WIDTH_CH } from "./displayTime";
import { type BodyLegendEntry, OrbitLegend } from "./OrbitLegend";
import { ZOOM_PRESETS } from "./orbitMap";
import {
  BODY_SCALE_UNITS,
  formatBodyScaleLength,
  formatOrbitScaleLength,
  ORBIT_SCALE_UNITS,
} from "./orbitScale";
import { useFrameTime } from "./useDisplayTime";
import type { SystemViewState } from "./useSystemView";

/** Room for the longest value of each of the centre's coordinates, in characters, as on the chart. */
const RADIUS_WIDTH_CH = 9;
const ANGLE_WIDTH_CH = 6;
const HEIGHT_WIDTH_CH = 9;

/** The precision the centre's radius is read at, which the core arrow's distance shares. */
const CENTRE_DECIMALS = 1;

/** The key of `FOCUS BODY`, which focuses the selected planet and returns the map from it. */
export const FOCUS_KEY = "C";

/** The zones' annuli the map can switch, in the order offered, with their labels. */
const ZONE_TOGGLES: ReadonlyArray<{ readonly layer: keyof ZoneLayers; readonly label: string }> = [
  { layer: "stable", label: "STABLE ZONE" },
  { layer: "snowLine", label: "SNOW LINE" },
  { layer: "habitable", label: "HABITABLE ZONE" },
  { layer: "optimistic", label: "OPTIMISTIC" },
];

/**
 * The legend's entry of each kind of body the scene draws, once each, in the order first drawn: a
 * gas or ice giant apart from a smaller planet, since its mark is larger.
 */
function bodyLegendEntries(
  view: SystemViewState,
  scene: SpatialScene,
): ReadonlyArray<BodyLegendEntry> {
  const drawn = new Set(scene.points.map((mark) => mark.id));
  const entries = new Map<string, BodyLegendEntry>();
  for (const body of view.bodies?.bodies ?? []) {
    const symbol = bodySymbol(body);
    if (symbol === null || !drawn.has(body.id)) {
      continue;
    }
    const giant =
      body.bulk.state === "ok" &&
      (body.bulk.value.planetClass === "gas_giant" || body.bulk.value.planetClass === "ice_giant");
    const label = body.kind.kind === "planet" && giant ? "GIANT PLANET" : bodyKindLabel(body.kind);
    if (!entries.has(label)) {
      entries.set(label, { label, ...symbol });
    }
  }
  return [...entries.values()];
}

/** Props of {@link OrbitMapPanel}. */
export interface OrbitMapPanelProps {
  readonly view: SystemViewState;
}

/**
 * The `ORBIT MAP` panel: the system on its own reference plane at the display time, true to scale
 * with bodies not to scale, and what its marks mean (plan 14, P14.T42).
 *
 * @remarks
 * The spatial view is handed its furniture: the frame `SYSTEM BARYCENTRIC`, the centre as the
 * barycentre's `RADIUS`, `ANGLE` and `HEIGHT` in the `GALACTIC` frame, which is the system's
 * position, and the display time; the core arrow's distance is the centre's `RADIUS`, and the triad
 * and the arrow point the galactic directions at the system while the plane, grid and presets
 * follow the system's (D21; the orchestrator's rulings 33 and 34.5). The zoom presets `INNER`,
 * `ALL` and, in a system with a belt, `BELTS`, keys `I`, `A` and `B`, fit the view again on each
 * press. `FOCUS BODY`, key `C`, draws the selected planet, or a selected moon's or ring's, in its own
 * frame, `BODY <designation>`, with its moons and rings about it, its equatorial plane for
 * reference and the scale bar in `Mm` and `km`; pressed again, or on a zoom preset, the map returns
 * to `SYSTEM BARYCENTRIC` (P14.T42.b). The zones' switches stand only in the system frame. Until there is an answer the panel
 * reads the request's state in words and draws nothing; a system not yet formed reads
 * `NOT YET FORMED`; a system with nothing to draw reads `NO BODIES`; an answer the display cannot
 * use reads `SYSTEM DATA INVALID` with `RETRY`. A newer request's state stands in the panel's head,
 * so that the map does not move as it comes and goes: the stars' or the bodies' request, whichever
 * is still pending or failed, the bodies' not counted while the server answers them `unsupported`
 * (the orchestrator's ruling 59.1); a bodies' answer the display cannot use reads
 * `BODY DATA INVALID` there, with `RETRY`, and the stars are drawn alone. With zones, the head also
 * offers `STABLE ZONE`, `SNOW LINE`, `HABITABLE ZONE` and `OPTIMISTIC`, each switching its annuli on and off. An answer that is a stale snapshot, its link
 * down or the newer request its display time asked for refused or timed out, is drawn and read as
 * stale. The chosen zoom preset is pressed while the view shows it: a zoom by hand releases it,
 * since the view no longer matches it, and `Z`, which fits the view to its radius again, presses it
 * once more (the orchestrator's ruling 59.6). The grid still covers its radius. While the display
 * time runs, the map follows it at each frame, which is the one thing on the display that does:
 * the readings and the time in the view's furniture change at the guide's 4 Hz. Under the map
 * stand the legend and the system note, which names what this generator version does not model so
 * that the space round the stars is not read as empty.
 */
export function OrbitMapPanel({ view }: OrbitMapPanelProps) {
  const titleId = useId();
  const focusHeldId = useId();
  const { target, status, model, fault, stale, plane, fitRadiusAu, zoom, displayTime } = view;
  const { chooseZoom, toggleFocus, hasBelts, sceneAt } = view;
  // While the display time runs, the map alone follows it frame by frame (P14.T44.b).
  const frameTime = useFrameTime(displayTime.frameTime);
  const scene = useMemo(
    () => (frameTime === null ? view.scene : sceneAt(frameTime)),
    [frameTime, view.scene, sceneAt],
  );
  const drawn = scene !== null && scene.points.length > 0;
  const focused = view.focused !== null;
  const focusHeld = !focused && view.focusable === null;
  const presets = ZOOM_PRESETS.filter((control) => control.name !== "belts" || hasBelts);

  // The zoom keys act from anywhere on the display but a text field, as the view's own keys do.
  useEffect(() => {
    if (!drawn) {
      return undefined;
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || event.repeat) {
        return;
      }
      if (isTextEntry(event.target)) {
        return;
      }
      const key = event.key.toUpperCase();
      if (key === FOCUS_KEY) {
        event.preventDefault();
        toggleFocus();
        return;
      }
      const preset = ZOOM_PRESETS.find((control) => control.key === key);
      if (preset === undefined || (preset.name === "belts" && !hasBelts)) {
        return;
      }
      event.preventDefault();
      chooseZoom(preset.name);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [drawn, chooseZoom, toggleFocus, hasBelts]);

  const centre = cylindrical(target.positionLy);
  const onAxis = !(centre.radiusLy > AXIS_TOLERANCE_LY);
  const radiusText = formatLengthLy(centre.radiusLy, CENTRE_DECIMALS);
  const centreReadings: ReadonlyArray<SpatialReading> = [
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
    label: DISPLAY_TIME_LABEL,
    value: formatUniverseTimeDhms(displayTime.time),
    unit: "",
    widthCh: DISPLAY_TIME_WIDTH_CH,
  };
  const coreDistance: SpatialQuantity = { value: radiusText, unit: "ly" };

  let body: ReactNode;
  if (fault !== null) {
    body = (
      <StatusLine
        text={`SYSTEM DATA INVALID: ${fault}`}
        standing="fault"
        action={{ label: "RETRY", onAction: view.retry }}
      />
    );
  } else if (model === null) {
    body = <RequestStatus state={status} onRetry={view.retry} />;
  } else if (!model.formed) {
    body = <p className="panel__empty orbit-map__empty">NOT YET FORMED</p>;
  } else if (scene === null || !drawn || plane === null || fitRadiusAu === null) {
    body = <p className="panel__empty orbit-map__empty">NO BODIES</p>;
  } else {
    body = (
      <>
        <SpatialView
          scene={scene}
          fitRadius={fitRadiusAu}
          fitRequest={view.fitRequest}
          onFitChange={view.fitChanged}
          formatLength={focused ? formatBodyScaleLength : formatOrbitScaleLength}
          scaleUnits={focused ? BODY_SCALE_UNITS : ORBIT_SCALE_UNITS}
          frameName={view.frameName}
          centre={centreReadings}
          time={time}
          coreDistance={coreDistance}
          axes={localFrameAt(target.positionLy)}
          accessibleName="Orbit map"
          stale={stale}
          onSelect={view.select}
        />
        <OrbitLegend
          // A body's frame draws no star, so its legend names none, nor what a star's size means.
          kinds={focused ? [] : model.hosts.map((host) => host.kind)}
          bodies={bodyLegendEntries(view, scene)}
          bands={focused ? [] : target.bands}
          plane={plane}
        />
      </>
    );
  }

  return (
    <section
      className={
        stale ? "panel system__map orbit-map orbit-map--stale" : "panel system__map orbit-map"
      }
      aria-labelledby={titleId}
    >
      <div className="orbit-map__head">
        <h2 className="panel__title" id={titleId}>
          Orbit map{stale ? <StaleMark /> : null}
        </h2>
        <span className="orbit-map__system">{target.designation}</span>
        {drawn ? (
          <fieldset className="orbit-map__zoom" aria-label="Zoom">
            {presets.map(({ name, label, key }) => (
              <button
                key={name}
                type="button"
                className="control orbit-map__zoom-button"
                aria-pressed={!focused && view.zoomHeld && zoom === name}
                aria-keyshortcuts={key}
                onClick={() => {
                  chooseZoom(name);
                }}
              >
                <span className="control__key">{key}</span> {label}
              </button>
            ))}
            <button
              type="button"
              className="control orbit-map__zoom-button"
              aria-pressed={focused}
              aria-keyshortcuts={FOCUS_KEY}
              // Held back rather than disabled, so that it keeps its focus and can say why.
              aria-disabled={focusHeld ? "true" : undefined}
              aria-describedby={focusHeld ? focusHeldId : undefined}
              onClick={toggleFocus}
            >
              <span className="control__key">{FOCUS_KEY}</span> FOCUS BODY
            </button>
          </fieldset>
        ) : null}
        {drawn && focusHeld ? (
          <p className="panel__inhibit" id={focusHeldId}>
            NO PLANET SELECTED
          </p>
        ) : null}
        {drawn && !focused && (view.bodies?.zones.length ?? 0) > 0 ? (
          <fieldset className="orbit-map__zoom" aria-label="Zones">
            {ZONE_TOGGLES.map(({ layer, label }) => (
              <button
                key={layer}
                type="button"
                className="control orbit-map__zoom-button"
                aria-pressed={view.zoneLayers[layer]}
                onClick={() => {
                  view.toggleZoneLayer(layer);
                }}
              >
                {label}
              </button>
            ))}
          </fieldset>
        ) : null}
        {model === null || fault !== null ? null : (
          <RequestStatus state={status} onRetry={view.retry} />
        )}
        {view.bodiesFault === null ? null : (
          <StatusLine
            text={`BODY DATA INVALID: ${view.bodiesFault}`}
            standing="fault"
            action={{ label: "RETRY", onAction: view.retry }}
          />
        )}
      </div>
      {body}
      {view.note === null ? null : <p className="orbit-map__note">{view.note}</p>}
    </section>
  );
}
