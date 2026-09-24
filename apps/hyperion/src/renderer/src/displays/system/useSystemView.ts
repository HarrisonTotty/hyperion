import { useCallback, useMemo, useState } from "react";

import { type HierarchyLayout, layoutHierarchy } from "../../lib/system/hierarchy";
import type { HostBody, SystemModel } from "../../lib/system/model";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import type { RequestState } from "../../lib/useServerRequest";
import type { SpatialScene } from "../../spatial/marks";
import {
  type FitRadii,
  fitRadiiAu,
  type OrbitPlane,
  orbitPlane,
  orbitScene,
  type ZoomPreset,
} from "./orbitMap";
import { type BodiesKnown, systemNote } from "./systemNote";
import type { SystemTarget } from "./systemTarget";
import { type DisplayTime, useDisplayTime } from "./useDisplayTime";
import { useSystemData } from "./useSystemData";

/** Everything one opening of the `SYSTEM` display is drawn from. */
export interface SystemViewState {
  readonly target: SystemTarget;
  /** The display's own time and the ways to step it. */
  readonly displayTime: DisplayTime;
  /** Where the latest `system_summary` request stands. */
  readonly summary: RequestState<"system_summary">;
  /** The answer on show, or `null` before the first or when it cannot be used. */
  readonly model: SystemModel | null;
  /** Why the answer on show cannot be used, in words; `null` when it can. */
  readonly fault: string | null;
  /**
   * Whether the answer on show is a stale snapshot: the link that backed it is down, or the newer
   * request the display time asked for was refused or timed out.
   */
  readonly stale: boolean;
  /** Where the stars are, from the answer's hierarchy; `null` without a formed system. */
  readonly layout: HierarchyLayout | null;
  /** The orbit map's reference plane; `null` without a formed system. */
  readonly plane: OrbitPlane | null;
  /** The radius each zoom preset fits; `null` without a formed system. */
  readonly fitRadii: FitRadii | null;
  readonly zoom: ZoomPreset;
  /** Counts the presses of a zoom preset, so that each press fits the view again. */
  readonly fitRequest: number;
  /** What the orbit map draws at the display time; `null` without a formed system. */
  readonly scene: SpatialScene | null;
  /** The body selected on the map or in the list: the primary until another is chosen. */
  readonly selected: HostBody | null;
  /** What the display says is not yet modelled, or `null` when nothing is. */
  readonly note: string | null;
  readonly select: (id: string) => void;
  readonly chooseZoom: (zoom: ZoomPreset) => void;
  readonly retry: () => void;
}

/**
 * What the display holds of the bodies beyond its hosts: this protocol cannot ask for them, so the
 * note names them all until `system_bodies` (P14.T35.b) sends their sections' tags.
 */
const BODIES: BodiesKnown = { kind: "unserved" };

/**
 * Owns one opening of the `SYSTEM` display: its time, its requests, its selection and zoom, and the
 * orbit map's scene (plan 14, P14.T41–T44).
 *
 * @remarks
 * Called once per opening: the display is given a new `key` for each, so that opening a system
 * again starts it afresh at the chart's time. The layout, the plane and the fitted radii depend on
 * the answer alone and keep their identity while it does; the scene depends on the display time,
 * the selection and the zoom too, and keeps its identity while none of them changes, so that the
 * map paints only when what it shows has changed. The selection is derived: the body chosen while
 * it is in the answer on show, and the primary otherwise.
 */
export function useSystemView(target: SystemTarget): SystemViewState {
  const displayTime = useDisplayTime(target.time);
  const [generation, setGeneration] = useState(0);
  const [chosenId, setChosenId] = useState<string | null>(null);
  const [zoom, setZoom] = useState<ZoomPreset>("all");
  const [fitRequest, setFitRequest] = useState(0);
  const { status } = useServerLink();
  const data = useSystemData(target, displayTime.time, generation);

  const shown = data.shown;
  const model = shown?.kind === "ok" ? shown.model : null;
  const fault = shown?.kind === "fault" ? shown.fault : null;
  const formed = model !== null && model.formed ? model : null;

  const layout = useMemo(
    () => (formed === null ? null : layoutHierarchy(formed.hierarchy, formed.hosts)),
    [formed],
  );
  const plane = useMemo(
    () => (layout === null ? null : orbitPlane(layout, target.positionLy)),
    [layout, target.positionLy],
  );
  const fitRadii = useMemo(
    () => (layout === null || formed === null ? null : fitRadiiAu(layout, formed.hosts)),
    [layout, formed],
  );

  const hosts = formed?.hosts ?? [];
  const selected = hosts.find((host) => host.id === chosenId) ?? hosts[0] ?? null;
  const selectedId = selected?.id ?? null;
  const time = displayTime.time;
  const scene = useMemo(
    () =>
      formed === null || layout === null || plane === null || fitRadii === null
        ? null
        : orbitScene({
            hosts: formed.hosts,
            layout,
            plane,
            time,
            selectedId,
            bands: target.bands,
            fitRadiusAu: fitRadii[zoom],
          }),
    [formed, layout, plane, fitRadii, time, selectedId, target.bands, zoom],
  );

  const chooseZoom = useCallback((next: ZoomPreset): void => {
    setZoom(next);
    setFitRequest((count) => count + 1);
  }, []);

  return {
    target,
    displayTime,
    summary: data.summary,
    model,
    fault,
    // The link's loss makes the answer on show a snapshot with nothing behind it, which the guide
    // reads as stale, as the chart's is; so does a newer request that failed, which leaves the
    // answer for a time the display has moved on from.
    stale:
      shown !== null &&
      (linkDownReason(status) !== null ||
        data.summary.kind === "rejected" ||
        data.summary.kind === "timed_out"),
    layout,
    plane,
    fitRadii,
    zoom,
    fitRequest,
    scene,
    selected,
    note: formed === null ? null : systemNote(BODIES),
    select: setChosenId,
    chooseZoom,
    retry: () => {
      setGeneration((count) => count + 1);
    },
  };
}
