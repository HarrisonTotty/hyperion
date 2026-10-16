import type { MapPopulation } from "@hyperion/protocol";
import { type CSSProperties, useEffect, useId, useState, useSyncExternalStore } from "react";

import { formatScaleLength } from "../../lib/format";
import type { CentreLy } from "../../lib/galaxy/model";
import { populationLabel } from "../../lib/galaxy/wire";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import { isTextEntry } from "../../lib/textEntry";
import { useUniverse } from "../../lib/universe";
import { useElementSize } from "../../lib/useElementSize";
import { ScaleBar } from "../../spatial/ScaleBar";
import {
  DUST_OVERLAY_BRIGHTNESS,
  DUST_OVERLAY_LABEL,
  GalaxyMapView,
  type MapQuantity,
  takesOverlay,
} from "./GalaxyMapView";
import { MAP_ACROSS_LY } from "./mapCursor";
import { mapLayout, mapPictureWidth, mapRows, overlayHasRoom } from "./mapLayout";
import { StaleWaits, staleWaitText } from "./staleWaits";

/**
 * The name of each population a map may count, in the order offered.
 *
 * @remarks
 * `young` counts the young thin disc alone (plan 04's `MapPopulation`), so it takes that
 * population's one name, as `PARAMETERS` shows it.
 */
const POPULATION_LABEL: Readonly<Record<MapPopulation, string>> = {
  all: "ALL",
  young: populationLabel("young_thin_disc"),
};

function isMapPopulation(key: string): key is MapPopulation {
  return Object.hasOwn(POPULATION_LABEL, key);
}

const POPULATIONS: ReadonlyArray<MapPopulation> =
  Object.keys(POPULATION_LABEL).filter(isMapPopulation);

/** The name of each quantity the maps may show, in the order offered (plan 07, P07.T11.b). */
const QUANTITY_LABEL: Readonly<Record<MapQuantity, string>> = {
  column_density: "COLUMN DENSITY",
  extinction: "EXTINCTION",
};

const QUANTITIES: ReadonlyArray<MapQuantity> = ["column_density", "extinction"];

/** The key that steps the quantity to the next one, from anywhere on the page but a text field. */
export const QUANTITY_KEY = "Q";

/** The key that puts the dust overlay on or takes it off, under `COLUMN DENSITY`. */
export const OVERLAY_KEY = "D";

/**
 * Why the `DUST OVERLAY` is held back under `EXTINCTION`: it dims the column density, and only that
 * (decision-gg-port-d0 §2.1).
 */
export const OVERLAY_DENSITY_ONLY = "NOT AVAILABLE: dims COLUMN DENSITY only";

/**
 * Why the `DUST OVERLAY` is held back on a page with no room for its words beside the pictures
 * ({@link overlayHasRoom}), word for word the `VIEW` display's form for a slot it has no room for
 * (decision-gg-port-d0 §3.2.3).
 */
export const OVERLAY_NO_ROOM = "NOT AVAILABLE: no room for DUST OVERLAY at this window size";

/** The quantity after `quantity` in the order offered, the first after the last. */
function nextQuantity(quantity: MapQuantity): MapQuantity {
  const next = QUANTITIES[(QUANTITIES.indexOf(quantity) + 1) % QUANTITIES.length];
  return next ?? quantity;
}

/** The longest scale bar, as a share of the pictures' width. */
const SCALE_BAR_SHARE = 0.25;

interface GalaxyMapPanelProps {
  /** The map cursor, in the `GALACTIC` frame, shown on both views. */
  readonly cursorLy: CentreLy;
  /** Moves the cursor, after a pick on a map or an arrow key on it. */
  readonly onCursor: (cursorLy: CentreLy) => void;
  /** The chart's centre, marked on both maps, or `null` before one is chosen. */
  readonly centreLy: CentreLy | null;
  /**
   * Whether the page is on show: its single keys act only then, since a page behind its tab stays
   * mounted, so that its maps are kept.
   */
  readonly shown: boolean;
}

/**
 * The `GALAXY MAP` page: the open universe's galaxy face-on above edge-on, and the choice of
 * population they count.
 *
 * @remarks
 * The page is measured once and both pictures are given one width, as large as the page allows: the
 * edge-on picture is half the face-on picture's height, which is their true proportion (the map
 * extents of plan 04, design note 12), so both are at one scale, and one scale bar and one frame
 * name serve them. Each view's words (title, key hint, density under the cursor, legend) stand in a
 * column beside its picture, so that the pictures take the page's height. `ALL` counts every
 * system; `YOUNG THIN DISC` the population where the arms show. The page owns the population;
 * changing it asks for both maps again. It needs the server, so while the link is down it is held
 * back, described by the link's reason, and the maps on show stay, marked stale (plan 05's T11
 * ruling (8); decision-gg-port-a2 §3). The link's reason stands in the page's one line for the
 * views' wait, the tabs' row compact and the controls' first line stacked; with the link up the
 * same line states a stale view's new map in flight, `PENDING`, or `EXTINCTION PENDING` while the
 * views await only the overlay's extinction map, as each view reports it (`StaleWaits`), so that a
 * stale view sets no line among its own words and the page's height never depends on it
 * (decision-gg-port-a2 §2). The page owns the quantity too (plan 07, P07.T11.b): `COLUMN DENSITY`,
 * or `EXTINCTION`, the visual extinction through the galaxy, which `Q` steps through; and under
 * `COLUMN DENSITY` the `DUST OVERLAY`, which dims the density by the dust along each pixel's line
 * of sight and which `D` puts on and takes off. Both keys are held back with the choices while the
 * link is down. While the overlay is on, the page names it and its quantity once, beside its
 * toggle, `DUST OVERLAY: A(V), WHOLE LINE OF SIGHT`, as the guide requires, and says after it what
 * the pictures' brightness then shows, `Brightness is COLUMN DENSITY dimmed by A(V)`, a sentence
 * (decision-gg-port-a2 §5). The overlay is offered only where the page has room for its words
 * beside the pictures ({@link overlayHasRoom}, a rule on the page's size alone); elsewhere, and
 * under `EXTINCTION`, its toggle is held back rather than removed, keeping its focus and described
 * by why, `OVERLAY_NO_ROOM` taking precedence over `OVERLAY_DENSITY_ONLY`: `D` does nothing, and
 * the choice is kept for when it is offered again (decision-gg-port-d0 §3.2.3). The map cursor and
 * the chart's centre belong to the display, which reads the cursor out and enters it in its
 * `CURSOR` panel beside the map; both are marked on both maps.
 *
 * A page shorter than the local chart's threshold, as at 1280 × 720, is laid out `compact`
 * ({@link mapLayout}): the controls stand in a row across the page above the pictures, the words
 * column is wider, so that each view's title and hint and each legend's floor take one line, the
 * pictures narrower to match and never so wide that a plain state's words would take the page past
 * its foot, and each view's row as tall as its picture or its words. Stacked or compact, under the
 * overlay a view's two legends stand side by side, each view is as tall as its picture or, on a
 * page too narrow for that, its tallest plain words ({@link mapRows}), nothing on the page scrolls,
 * the pictures stand clear of the panel's page tabs, and the page's parts, and so its tab order,
 * are in one order.
 */
export function GalaxyMapPanel({ cursorLy, onCursor, centreLy, shown }: GalaxyMapPanelProps) {
  const populationName = useId();
  const quantityName = useId();
  const overlayReasonId = useId();
  const waitId = useId();
  const { status } = useServerLink();
  const { open } = useUniverse();
  const [population, setPopulation] = useState<MapPopulation>("all");
  const [quantity, setQuantity] = useState<MapQuantity>("column_density");
  const [overlay, setOverlay] = useState(false);
  const { ref, size } = useElementSize();
  const [staleWaits] = useState(() => new StaleWaits());
  const waits = useSyncExternalStore(staleWaits.subscribe, staleWaits.getSnapshot);
  const linkReason = linkDownReason(status);
  // The views' stale wait, which the page states where the link's state stands, and never with it.
  const staleWait = linkReason === null ? staleWaitText(waits) : null;
  const inhibited = linkReason !== null;
  const layout = mapLayout(size);
  const pictureWidthPx = size === null ? 0 : mapPictureWidth(size, layout);
  // Why the overlay is not offered, shown beside its toggle, or `null` where it is: the page's room
  // first, since no quantity changes it.
  let overlayReason: string | null = null;
  if (size === null || !overlayHasRoom(size, layout)) {
    overlayReason = OVERLAY_NO_ROOM;
  } else if (!takesOverlay(quantity)) {
    overlayReason = OVERLAY_DENSITY_ONLY;
  }
  const overlayOffered = overlayReason === null;
  const overlayShown = overlay && overlayOffered;
  // Why the overlay's toggle does not act, as the description of it (the link's reason and the
  // overlay's own, each where it holds), or `null` when it does.
  const heldBy = [inhibited ? waitId : null, overlayOffered ? null : overlayReasonId].filter(
    (id) => id !== null,
  );
  const overlayHeldBy = heldBy.length > 0 ? heldBy.join(" ") : null;

  // The keys reach the choices from anywhere on the display but a text field while the page is
  // on show (the display is under `Activity`, so the listener goes while another display is shown);
  // held back with the choices while the link is down, and absent with them while no universe is
  // open.
  const keysLive = shown && open !== null;
  useEffect(() => {
    if (!keysLive) {
      return undefined;
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      const modified = event.ctrlKey || event.altKey || event.metaKey || event.shiftKey;
      if (event.repeat || modified || isTextEntry(event.target)) {
        return;
      }
      const key = event.key.toUpperCase();
      if (key === QUANTITY_KEY) {
        event.preventDefault();
        if (!inhibited) {
          setQuantity(nextQuantity);
        }
      } else if (key === OVERLAY_KEY && overlayOffered) {
        event.preventDefault();
        if (!inhibited) {
          setOverlay((on) => !on);
        }
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [inhibited, overlayOffered, keysLive]);
  const devicePixelRatio = size?.devicePixelRatio ?? 1;
  const rows = size === null ? null : mapRows(size, layout);
  // The pictures' width and the views' heights, which the stylesheet lays the page out by.
  const gridStyle: CSSProperties & {
    readonly "--map-picture": string;
    readonly "--map-face-row": string;
    readonly "--map-edge-row": string;
  } = {
    "--map-picture": `${pictureWidthPx}px`,
    "--map-face-row": `${rows?.faceOnPx ?? 0}px`,
    "--map-edge-row": `${rows?.edgeOnPx ?? 0}px`,
  };

  return (
    <div className="galaxy-map" ref={ref}>
      {open === null ? (
        <p className="panel__empty galaxy-map__empty">NO UNIVERSE OPEN</p>
      ) : (
        <div className={`galaxy-map__grid galaxy-map__grid--${layout}`} style={gridStyle}>
          <div className="galaxy-map__controls">
            {/*
             * The page's one line for the views' wait: the link's state while it is down, which
             * holds the choices back, and otherwise a stale view's new map in flight; each stale
             * picture is described by it (decision-gg-port-a2 §2).
             */}
            {linkReason === null ? null : (
              <p className="panel__inhibit galaxy-map__wait" id={waitId}>
                {linkReason}
              </p>
            )}
            {staleWait === null ? null : (
              <output className="panel__inhibit galaxy-map__wait" id={waitId}>
                {staleWait}
              </output>
            )}
            <fieldset className="form-choice galaxy-map__population">
              <legend className="form-field__label">POPULATION</legend>
              <div className="form-choice__options">
                {POPULATIONS.map((value) => (
                  <label key={value} className="form-choice__option">
                    <input
                      type="radio"
                      name={populationName}
                      value={value}
                      checked={population === value}
                      // Held back rather than disabled, so that it keeps focus and can say why.
                      aria-disabled={inhibited ? "true" : undefined}
                      aria-describedby={inhibited ? waitId : undefined}
                      onChange={() => {
                        if (!inhibited) {
                          setPopulation(value);
                        }
                      }}
                    />
                    {POPULATION_LABEL[value]}
                  </label>
                ))}
              </div>
            </fieldset>
            <div className="galaxy-map__quantity">
              <fieldset className="form-choice" aria-keyshortcuts={QUANTITY_KEY}>
                <legend className="form-field__label">
                  <span className="control__key">{QUANTITY_KEY}</span> QUANTITY
                </legend>
                <div className="form-choice__options">
                  {QUANTITIES.map((value) => (
                    <label key={value} className="form-choice__option">
                      <input
                        type="radio"
                        name={quantityName}
                        value={value}
                        checked={quantity === value}
                        aria-disabled={inhibited ? "true" : undefined}
                        aria-describedby={inhibited ? waitId : undefined}
                        onChange={() => {
                          if (!inhibited) {
                            setQuantity(value);
                          }
                        }}
                      />
                      {QUANTITY_LABEL[value]}
                    </label>
                  ))}
                </div>
              </fieldset>
              {/*
               * Held back where the page has no room for it and under EXTINCTION, rather than
               * removed, so that it keeps its focus and says why; the choice is kept for when it is
               * offered again.
               */}
              <button
                type="button"
                className="control galaxy-map__overlay"
                aria-pressed={overlayShown}
                aria-keyshortcuts={OVERLAY_KEY}
                aria-disabled={overlayHeldBy === null ? undefined : "true"}
                aria-describedby={overlayHeldBy ?? undefined}
                onClick={() => {
                  if (overlayHeldBy === null) {
                    setOverlay((on) => !on);
                  }
                }}
              >
                <span className="control__key">{OVERLAY_KEY}</span> DUST OVERLAY
              </button>
              {overlayReason === null ? null : (
                <span className="galaxy-map__overlay-reason" id={overlayReasonId}>
                  {overlayReason}
                </span>
              )}
              {overlayShown ? (
                <>
                  <p className="galaxy-map__overlay-name">{DUST_OVERLAY_LABEL}</p>
                  <p className="galaxy-map__overlay-brightness">{DUST_OVERLAY_BRIGHTNESS}</p>
                </>
              ) : null}
            </div>
            <div className="galaxy-map__furniture">
              <p className="field">
                <span className="field__label">FRAME</span> <span>GALACTIC</span>
              </p>
              {pictureWidthPx > 0 ? (
                <ScaleBar
                  pxPerUnit={pictureWidthPx / MAP_ACROSS_LY}
                  maxBarPx={pictureWidthPx * SCALE_BAR_SHARE}
                  formatLength={formatScaleLength}
                />
              ) : null}
            </div>
          </div>
          <GalaxyMapView
            universe={open.id}
            view="face_on"
            population={population}
            quantity={quantity}
            dustOverlay={overlayShown}
            pictureWidthPx={pictureWidthPx}
            devicePixelRatio={devicePixelRatio}
            cursorLy={cursorLy}
            onCursor={onCursor}
            centreLy={centreLy}
            staleWaits={staleWaits}
            pageWaitId={waitId}
          />
          <GalaxyMapView
            universe={open.id}
            view="edge_on"
            population={population}
            quantity={quantity}
            dustOverlay={overlayShown}
            pictureWidthPx={pictureWidthPx}
            devicePixelRatio={devicePixelRatio}
            cursorLy={cursorLy}
            onCursor={onCursor}
            centreLy={centreLy}
            staleWaits={staleWaits}
            pageWaitId={waitId}
          />
        </div>
      )}
    </div>
  );
}
