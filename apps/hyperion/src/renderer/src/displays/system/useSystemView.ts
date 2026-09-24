import type { DetailLevelDto, RequestKind } from "@hyperion/protocol";
import { useCallback, useMemo, useState } from "react";

import { stateAt } from "../../lib/orbit";
import { type HierarchyLayout, layoutHierarchy, orbitNormal } from "../../lib/system/hierarchy";
import type {
  BodyRecord,
  HostBody,
  SystemBodies,
  SystemBody,
  SystemModel,
  Zone,
} from "../../lib/system/model";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import type { RequestState } from "../../lib/useServerRequest";
import type { SpatialScene } from "../../spatial/marks";
import { dot, norm } from "../../spatial/vec3";
import {
  ALL_ZONE_LAYERS,
  type BodiesLayout,
  bodyReachesAu,
  habitableOuterAu,
  hostKeyOf,
  layoutBodies,
  primaryZone,
  type ZoneLayers,
} from "./bodyMap";
import { type BodyRow, hostRows, systemRows } from "./bodyRows";
import {
  type FitRadii,
  fitRadiiAu,
  type OrbitPlane,
  orbitPlane,
  orbitScene,
  type ZoomPreset,
} from "./orbitMap";
import { type BodiesKnown, smallBodySections, systemNote } from "./systemNote";
import type { SystemTarget } from "./systemTarget";
import { useBodyDetail } from "./useBodyDetail";
import { type DisplayTime, useDisplayTime } from "./useDisplayTime";
import { useSystemData } from "./useSystemData";

/** A zone as a host's readout reads it: the zone, and what it is about in words. */
export interface HostZone {
  readonly zone: Zone;
  /** Its host in words: a star's designation, `PAIR /0 /1`, or `BARYCENTRE`. */
  readonly about: string;
}

/** What the readout reads: a host with the zones that hold it, or a body with its record. */
export type Selected =
  | {
      readonly kind: "host";
      readonly host: HostBody;
      /** The zones that hold it, innermost first; none before the bodies arrive. */
      readonly zones: ReadonlyArray<HostZone>;
    }
  | {
      readonly kind: "body";
      /** Its whole record from `body_detail` once that is in, and its list entry until then. */
      readonly body: SystemBody | BodyRecord;
      /** Whether `body` is the whole record, with its surface and hooks. */
      readonly whole: boolean;
      /** The detail level `body` holds. */
      readonly granted: DetailLevelDto;
      /** What it orbits in words, or `null` for a free-floating object. */
      readonly parentName: string | null;
      /** Its distance from what it orbits at the display time, m; `null` without an orbit. */
      readonly distanceM: number | null;
      /**
       * Its orbit's inclination to the orbit map's reference plane, rad, for a body that orbits a
       * star, a pair or the barycentre; to its planet's equator, as the wire gives it, for a moon.
       */
      readonly inclinationRad: number | null;
    };

/** Everything one opening of the `SYSTEM` display is drawn from. */
export interface SystemViewState {
  readonly target: SystemTarget;
  /** The display's own time and the ways to step it. */
  readonly displayTime: DisplayTime;
  /** Where the latest `system_summary` request stands. */
  readonly summary: RequestState<"system_summary">;
  /**
   * The request whose state the display stands on: the one its hosts come from while it has none,
   * and otherwise the first still pending or failed, `system_bodies` not counted while the server
   * does not serve it.
   */
  readonly status: RequestState<RequestKind>;
  /** Where the selected body's `body_detail` request stands; `idle` for a host. */
  readonly detailState: RequestState<"body_detail">;
  /** The answer on show, or `null` before the first or when it cannot be used. */
  readonly model: SystemModel | null;
  /** Why the answer on show cannot be used, in words; `null` when it can. */
  readonly fault: string | null;
  /** Why the bodies' answer cannot be used, in words; `null` when it can or there is none. */
  readonly bodiesFault: string | null;
  /** Why the selected body's record cannot be used, in words; `null` when it can or there is none. */
  readonly detailFault: string | null;
  /**
   * Whether the answer on show is a stale snapshot: the link that backed it is down, or the newer
   * request the display time asked for was refused or timed out.
   */
  readonly stale: boolean;
  /** Where the stars are, from the answer's hierarchy; `null` without a formed system. */
  readonly layout: HierarchyLayout | null;
  /** The system's bodies on show; `null` before them, or while the server does not serve them. */
  readonly bodies: SystemBodies | null;
  /** The orbit map's reference plane; `null` without a formed system. */
  readonly plane: OrbitPlane | null;
  /** The radius each zoom preset fits; `null` without a formed system. */
  readonly fitRadii: FitRadii | null;
  /** The zoom preset whose radius the view fits, and the grid covers. */
  readonly zoom: ZoomPreset;
  /**
   * Whether the view still shows the zoom preset: pressed until the operator zooms by hand, and
   * again once `Z` fits the view back to it (the orchestrator's ruling 59.6).
   */
  readonly zoomHeld: boolean;
  /** Counts the presses of a zoom preset, so that each press fits the view again. */
  readonly fitRequest: number;
  /** Which of the zones' annuli the map draws. */
  readonly zoneLayers: ZoneLayers;
  /** What the orbit map draws at the display time; `null` without a formed system. */
  readonly scene: SpatialScene | null;
  /** The body list's rows: the hosts, and the bodies under them. */
  readonly rows: ReadonlyArray<BodyRow>;
  /** The zone that holds the primary, whose class is the system's; `null` for none. */
  readonly primaryZone: Zone | null;
  /** What is selected on the map or in the list: the primary until another is chosen. */
  readonly selected: Selected | null;
  /** What the display says is not yet modelled, or `null` when nothing is. */
  readonly note: string | null;
  readonly select: (id: string) => void;
  readonly chooseZoom: (zoom: ZoomPreset) => void;
  /** Told by the view when the operator zooms by hand (`false`) or fits it again (`true`). */
  readonly fitChanged: (fitting: boolean) => void;
  /** Switches one of the zones' annuli on or off. */
  readonly toggleZoneLayer: (layer: keyof ZoneLayers) => void;
  readonly retry: () => void;
}

/** Whether a request has not answered: pending, refused, timed out, or cut off by the link. */
function unanswered(state: RequestState<RequestKind>): boolean {
  return state.kind !== "idle" && state.kind !== "ok";
}

/** Whether a newer request has failed, which leaves the answer on show for a time left behind. */
function failed(state: RequestState<RequestKind>): boolean {
  return state.kind === "rejected" || state.kind === "timed_out";
}

/** The suffix of a star's designation, ` /<index>`, by which a pair is named. */
function starSuffix(id: string, hosts: ReadonlyArray<HostBody>): string {
  const host = hosts.find((candidate) => candidate.id === id);
  return host === undefined ? id : `/${host.bodyIndex}`;
}

/** What a body or a zone is about, in words: a star, a pair, the barycentre or a body. */
function hostName(
  zoneOrBody: Zone["host"],
  model: SystemModel,
  layout: HierarchyLayout,
  bodies: ReadonlyArray<SystemBody>,
): string {
  let name: string;
  switch (zoneOrBody.kind) {
    case "star":
      name =
        model.hosts.find((host) => host.bodyIndex === zoneOrBody.bodyIndex)?.designation ??
        `/${zoneOrBody.bodyIndex}`;
      break;
    case "pair": {
      const key = hostKeyOf(zoneOrBody, model.system, layout);
      const stars = key === null ? [] : (layout.pairStars.get(key) ?? []);
      name = `PAIR ${stars.map((id) => starSuffix(id, model.hosts)).join(" ")}`;
      break;
    }
    case "barycentre":
      name = "BARYCENTRE";
      break;
    case "body":
      name = bodies.find((body) => body.id === zoneOrBody.id)?.designation ?? zoneOrBody.id;
      break;
  }
  return name;
}

/** The zones that hold a star, innermost first: its own, its pairs', and the barycentre's. */
function zonesHolding(
  host: HostBody,
  zones: ReadonlyArray<Zone>,
  model: SystemModel,
  layout: HierarchyLayout,
): ReadonlyArray<HostZone> {
  return zones
    .filter((zone) => {
      let result: boolean;
      switch (zone.host.kind) {
        case "star":
          result = zone.host.bodyIndex === host.bodyIndex;
          break;
        case "pair": {
          const key = hostKeyOf(zone.host, model.system, layout);
          result = key !== null && (layout.pairStars.get(key)?.includes(host.id) ?? false);
          break;
        }
        case "barycentre":
          result = true;
          break;
        case "body":
          result = false;
          break;
      }
      return result;
    })
    .map((zone) => ({ zone, about: hostName(zone.host, model, layout, []) }));
}

/**
 * Owns one opening of the `SYSTEM` display: its time, its requests, its selection and zoom, and the
 * orbit map's scene (plan 14, P14.T41–T44).
 *
 * @remarks
 * Called once per opening: the display is given a new `key` for each, so that opening a system
 * again starts it afresh at the chart's time. The layouts, the plane and the fitted radii depend on
 * the answers alone and keep their identity while they do; the scene depends on the display time,
 * the selection, the zoom and the zones shown too, and keeps its identity while none of them
 * changes, so that the map paints only when what it shows has changed. The selection is derived:
 * the body chosen while it is in the answer on show, and the primary otherwise. The selected body's
 * record is asked for at the time the system last was, and its list entry is read until it comes.
 */
export function useSystemView(target: SystemTarget): SystemViewState {
  const displayTime = useDisplayTime(target.time);
  const [generation, setGeneration] = useState(0);
  const [chosenId, setChosenId] = useState<string | null>(null);
  const [zoom, setZoom] = useState<ZoomPreset>("all");
  const [zoomHeld, setZoomHeld] = useState(true);
  const [fitRequest, setFitRequest] = useState(0);
  const [zoneLayers, setZoneLayers] = useState<ZoneLayers>(ALL_ZONE_LAYERS);
  const { status: linkStatus } = useServerLink();
  const data = useSystemData(target, displayTime.time, generation);

  const shown = data.shown;
  const model = shown?.kind === "ok" ? shown.model : null;
  const fault = shown?.kind === "fault" ? shown.fault : null;
  const formed = model !== null && model.formed ? model : null;
  const bodiesResult = data.bodies;
  const bodies = formed !== null && bodiesResult?.kind === "ok" ? bodiesResult.bodies : null;

  const layout = useMemo(
    () => (formed === null ? null : layoutHierarchy(formed.hierarchy, formed.hosts)),
    [formed],
  );
  const bodiesLayout = useMemo<BodiesLayout | null>(
    () =>
      layout === null || formed === null || bodies === null
        ? null
        : layoutBodies(formed.system, bodies.bodies, layout),
    [layout, formed, bodies],
  );
  const plane = useMemo(
    () => (layout === null ? null : orbitPlane(layout, target.positionLy, bodies?.systemPlane)),
    [layout, target.positionLy, bodies],
  );
  const zoneOfPrimary = useMemo(
    () =>
      layout === null || formed === null || bodies === null
        ? null
        : primaryZone(bodies.zones, formed.system, layout),
    [layout, formed, bodies],
  );
  const fitRadii = useMemo(
    () =>
      layout === null || formed === null
        ? null
        : fitRadiiAu(
            layout,
            formed.hosts,
            bodies === null || bodiesLayout === null
              ? null
              : {
                  bodyReachesAu: bodyReachesAu(bodies.bodies, bodiesLayout),
                  habitableOuterAu: habitableOuterAu(zoneOfPrimary, formed.system, layout),
                },
          ),
    [layout, formed, bodies, bodiesLayout, zoneOfPrimary],
  );

  const hosts = formed?.hosts ?? [];
  const bodyList = bodies?.bodies ?? [];
  const chosenHost = hosts.find((host) => host.id === chosenId);
  const chosenBody =
    chosenHost === undefined ? bodyList.find((body) => body.id === chosenId) : undefined;
  const selectedHost = chosenBody === undefined ? (chosenHost ?? hosts[0] ?? null) : null;
  const selectedId = chosenBody?.id ?? selectedHost?.id ?? null;
  const detail = useBodyDetail(target, chosenBody?.id ?? null, data.requestTime, generation);
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
            bodies:
              bodies === null || bodiesLayout === null
                ? null
                : { system: formed.system, bodies, layout: bodiesLayout, zoneLayers },
          }),
    [
      formed,
      layout,
      plane,
      fitRadii,
      time,
      selectedId,
      target.bands,
      zoom,
      bodies,
      bodiesLayout,
      zoneLayers,
    ],
  );

  const rows =
    formed === null || layout === null
      ? []
      : bodies === null
        ? hostRows(formed.hosts, layout)
        : systemRows(formed.system, formed.hosts, layout, bodies.bodies);

  let selected: Selected | null = null;
  if (formed !== null && layout !== null && chosenBody !== undefined) {
    const answer =
      detail.shown?.kind === "ok" && detail.shown.detail.record.id === chosenBody.id
        ? detail.shown.detail
        : null;
    const record = answer?.record ?? null;
    const body = record ?? chosenBody;
    const orbit = body.orbit.state === "ok" ? body.orbit.value : null;
    const systemPlane = bodies?.systemPlane ?? null;
    let inclinationRad: number | null = null;
    if (orbit !== null) {
      inclinationRad =
        orbit.parent.kind === "body" || systemPlane === null
          ? orbit.orbit.inclinationRad
          : Math.acos(
              Math.min(1, Math.max(-1, dot(orbitNormal(orbit.orbit), orbitNormal(systemPlane)))),
            );
    }
    selected = {
      kind: "body",
      body,
      whole: record !== null,
      granted: answer?.granted ?? bodies?.granted ?? "contact",
      parentName: body.parent === null ? null : hostName(body.parent, formed, layout, bodyList),
      distanceM:
        orbit === null || body.state.kind !== "present"
          ? null
          : norm(stateAt(orbit.orbit, time).positionM),
      inclinationRad,
    };
  } else if (formed !== null && layout !== null && selectedHost !== null) {
    selected = {
      kind: "host",
      host: selectedHost,
      zones: bodies === null ? [] : zonesHolding(selectedHost, bodies.zones, formed, layout),
    };
  }

  // The request the hosts come from stands for the display until there is an answer; after, the
  // first that is still pending or has failed. `system_bodies` answered `unsupported` is the
  // transitional state of ruling 59.1, not a fault.
  const hostsFromBodies = bodiesResult?.kind === "ok";
  const bodiesCount = !data.bodiesUnserved;
  const hostsState: RequestState<RequestKind> = hostsFromBodies ? data.bodiesState : data.summary;
  let status: RequestState<RequestKind> = hostsState;
  if (!unanswered(hostsState) && bodiesCount && unanswered(data.bodiesState)) {
    status = data.bodiesState;
  }

  const chooseZoom = useCallback((next: ZoomPreset): void => {
    setZoom(next);
    setZoomHeld(true);
    setFitRequest((count) => count + 1);
  }, []);
  const toggleZoneLayer = useCallback((layer: keyof ZoneLayers): void => {
    setZoneLayers((layers) => ({ ...layers, [layer]: !layers[layer] }));
  }, []);

  let known: BodiesKnown | null = null;
  if (bodies !== null) {
    known = { kind: "tagged", sections: smallBodySections(bodies) };
  } else if (data.bodiesUnserved) {
    known = { kind: "unserved" };
  }

  return {
    target,
    displayTime,
    summary: data.summary,
    status,
    detailState: detail.state,
    model,
    fault,
    bodiesFault: bodiesResult?.kind === "fault" ? bodiesResult.fault : null,
    detailFault: detail.shown?.kind === "fault" ? detail.shown.fault : null,
    // The link's loss makes the answer on show a snapshot with nothing behind it, which the guide
    // reads as stale, as the chart's is; so does a newer request that failed, which leaves the
    // answer for a time the display has moved on from.
    stale:
      shown !== null &&
      (linkDownReason(linkStatus) !== null ||
        failed(hostsState) ||
        (bodies !== null && failed(data.bodiesState))),
    layout,
    bodies,
    plane,
    fitRadii,
    zoom,
    zoomHeld,
    fitRequest,
    zoneLayers,
    scene,
    rows,
    primaryZone: zoneOfPrimary,
    selected,
    note: formed === null || known === null ? null : systemNote(known),
    select: setChosenId,
    chooseZoom,
    fitChanged: setZoomHeld,
    toggleZoneLayer,
    retry: () => {
      setGeneration((count) => count + 1);
    },
  };
}
