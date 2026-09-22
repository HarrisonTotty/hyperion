import type { MapPopulation } from "@hyperion/protocol";
import { type CSSProperties, useId, useState } from "react";

import { formatScaleLength } from "../../lib/format";
import type { CentreLy } from "../../lib/galaxy/model";
import { populationLabel } from "../../lib/galaxy/wire";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import { useUniverse } from "../../lib/universe";
import { type ElementSize, useElementSize } from "../../lib/useElementSize";
import { scaleBar } from "../../spatial/scale";
import { GalaxyMapView } from "./GalaxyMapView";
import { MAP_ACROSS_LY } from "./mapCursor";

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

/*
 * The page's layout, as `styles.css` lays out `.galaxy-map__grid`: a column for the words, at
 * least SIDE_REM wide, the gutter for the vertical axes' labels, the pictures, and the gutter for
 * the horizontal axes' labels; face-on above edge-on, VIEW_GAP_REM apart.
 */
const SIDE_REM = 24;
const AXIS_BEFORE_REM = 3.75;
const AXIS_AFTER_REM = 1.75;
const VIEW_GAP_REM = 0.5;

/** The longest scale bar, as a share of the pictures' width. */
const SCALE_BAR_SHARE = 0.25;

/**
 * The pictures' width in CSS pixels for a page of `size`: as wide as the page leaves beside the
 * words and axes, and no taller together than the page, the edge-on picture being half the face-on
 * picture's height; even, so that the edge-on picture is a whole number of pixels tall.
 */
function pictureWidthFor(size: ElementSize): number {
  const acrossPx = size.widthPx - (SIDE_REM + AXIS_BEFORE_REM + AXIS_AFTER_REM) * size.remPx;
  const downPx = ((size.heightPx - VIEW_GAP_REM * size.remPx) * 2) / 3;
  return Math.max(0, 2 * Math.floor(Math.min(acrossPx, downPx) / 2));
}

interface MapScaleBarProps {
  readonly lengthLy: number;
  readonly lengthPx: number;
}

/** A 1-2-5 length drawn as a bar with end ticks, and its label. */
function MapScaleBar({ lengthLy, lengthPx }: MapScaleBarProps) {
  return (
    <div className="scale-bar">
      {/* One pixel wider, so that the centres of the 1 px end ticks are the length apart. */}
      <div className="scale-bar__bar" style={{ width: `${lengthPx + 1}px` }} />
      <span className="scale-bar__label">{formatScaleLength(lengthLy)}</span>
    </div>
  );
}

interface GalaxyMapPanelProps {
  /** The map cursor, in the `GALACTIC` frame, shown on both views. */
  readonly cursorLy: CentreLy;
  /** Moves the cursor, after a pick on a map or an arrow key on it. */
  readonly onCursor: (cursorLy: CentreLy) => void;
  /** The chart's centre, marked on both maps, or `null` before one is chosen. */
  readonly centreLy: CentreLy | null;
}

/**
 * The `GALAXY MAP` page: the open universe's galaxy face-on above edge-on, and the choice of
 * population they count.
 *
 * @remarks
 * The page is measured once and both pictures are given one width, as large as the page allows:
 * the edge-on picture is half the face-on picture's height, which is their true proportion (the
 * map extents of plan 04, design note 12), so both are at one scale, and one scale bar and one
 * frame name serve them. Each view's words (title, key hint, density under the cursor, legend)
 * stand in a column beside its picture, so that the pictures take the page's height. `ALL` counts
 * every system; `YOUNG THIN DISC` the population where the arms show. The page owns the population;
 * changing it asks for both maps again. It needs the server, so while the link is down it is held
 * back, described by the link's reason, and the maps on show stay (plan 05, design note D4). The
 * map cursor and the chart's centre belong to the display, which reads the cursor out and enters it
 * in its `CURSOR` panel beside the map; both are marked on both maps.
 */
export function GalaxyMapPanel({ cursorLy, onCursor, centreLy }: GalaxyMapPanelProps) {
  const populationName = useId();
  const linkReasonId = useId();
  const { status } = useServerLink();
  const { open } = useUniverse();
  const [population, setPopulation] = useState<MapPopulation>("all");
  const { ref, size } = useElementSize();
  const linkReason = linkDownReason(status);
  const inhibited = linkReason !== null;
  const pictureWidthPx = size === null ? 0 : pictureWidthFor(size);
  const devicePixelRatio = size?.devicePixelRatio ?? 1;
  const bar =
    pictureWidthPx > 0
      ? scaleBar(pictureWidthPx / MAP_ACROSS_LY, pictureWidthPx * SCALE_BAR_SHARE)
      : null;
  // The pictures' width, which the stylesheet lays the page out by.
  const gridStyle: CSSProperties & { readonly "--map-picture": string } = {
    "--map-picture": `${pictureWidthPx}px`,
  };

  return (
    <div className="galaxy-map" ref={ref}>
      {open === null ? (
        <p className="panel__empty galaxy-map__empty">NO UNIVERSE OPEN</p>
      ) : (
        <div className="galaxy-map__grid" style={gridStyle}>
          <div className="galaxy-map__controls">
            {linkReason === null ? null : (
              <p className="panel__inhibit" id={linkReasonId}>
                {linkReason}
              </p>
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
                      aria-describedby={inhibited ? linkReasonId : undefined}
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
            <div className="galaxy-map__furniture">
              <p className="field">
                <span className="field__label">FRAME</span> <span>GALACTIC</span>
              </p>
              {bar === null ? null : <MapScaleBar lengthLy={bar.length} lengthPx={bar.lengthPx} />}
            </div>
          </div>
          <GalaxyMapView
            universe={open.id}
            view="face_on"
            population={population}
            pictureWidthPx={pictureWidthPx}
            devicePixelRatio={devicePixelRatio}
            cursorLy={cursorLy}
            onCursor={onCursor}
            centreLy={centreLy}
          />
          <GalaxyMapView
            universe={open.id}
            view="edge_on"
            population={population}
            pictureWidthPx={pictureWidthPx}
            devicePixelRatio={devicePixelRatio}
            cursorLy={cursorLy}
            onCursor={onCursor}
            centreLy={centreLy}
          />
        </div>
      )}
    </div>
  );
}
