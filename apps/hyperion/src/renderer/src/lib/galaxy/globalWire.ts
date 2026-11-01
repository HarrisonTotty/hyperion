/**
 * The adapter between plan 10's global-list messages and the client's model of galaxy data
 * (P10.T11): `global_features`, the galaxy's streams, dwarf cores and accreted progenitors, and
 * `stream_track`, one stream's centre line.
 *
 * @remarks
 * With `wire.ts` and `featureWire.ts`, one of the modules outside `test/` that read the wire's
 * unit-suffixed field names (plan 05, design note D8). Positions are exact on the wire as a
 * light-year cell plus a metre offset, and are taken from the frame's origin with
 * `galacticDeltaLy`, as the map's features are.
 */
import {
  type AccretedProgenitorWire,
  type GalacticPosition,
  type GlobalFeatures,
  galacticDeltaLy,
  type RequestOf,
  type StreamTrackWire,
  type TrackPointWire,
  type UniverseIdHex,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import type {
  AccretedProgenitor,
  GlobalDwarfCore,
  GlobalListModel,
  GlobalStream,
  StreamTrack,
  TrackPoint,
} from "./model";

/** The origin of the `GALACTIC` frame, from which absolute positions are measured. */
const ORIGIN: GalacticPosition = { cell_ly: [0, 0, 0], offset_m: [0, 0, 0] };

/** A position on the wire, in light-years from the frame's origin. */
function positionLy(position: GalacticPosition): Vec3 {
  const [x, y, z] = galacticDeltaLy(ORIGIN, position);
  return { x, y, z };
}

/** The galaxy's global list, asked for the universe `universe`. */
export function toGlobalFeaturesRequest(universe: UniverseIdHex): RequestOf<"global_features"> {
  return { kind: "global_features", universe };
}

/** Stream `stream`'s track, asked for the universe `universe`. */
export function toStreamTrackRequest(
  universe: UniverseIdHex,
  stream: number,
): RequestOf<"stream_track"> {
  return { kind: "stream_track", universe, stream };
}

function toProgenitor(wire: AccretedProgenitorWire): AccretedProgenitor {
  return {
    // Not one of plan 04's growing enums, so it carries all its variants (design note 15).
    kind: wire.kind,
    massMsun: wire.mass_msun,
    accretedGyr: wire.accreted_gyr,
    pericentreLy: wire.pericentre_ly,
    apocentreLy: wire.apocentre_ly,
    stream: wire.stream,
    core: wire.core,
  };
}

/**
 * The global list's streams, dwarf cores and accreted progenitors, each in the list's own order.
 *
 * @remarks
 * The galactic centre, the list's first entry, is the map's own feature (plan 09, P09.T41.b), and
 * is left out. A core from a server before P10.T11 has no place or extent, which read `null`.
 */
export function toGlobalList(response: GlobalFeatures): GlobalListModel {
  const streams: GlobalStream[] = [];
  const cores: GlobalDwarfCore[] = [];
  for (const entry of response.entries) {
    switch (entry.kind) {
      case "galactic_centre":
        break;
      case "stream":
        streams.push({
          number: entry.number,
          massMsun: entry.mass_msun,
          progenitor: entry.progenitor,
          trackReady: entry.track === "ready",
        });
        break;
      case "dwarf_core":
        cores.push({
          number: entry.number,
          massMsun: entry.mass_msun,
          positionLy: entry.position === undefined ? null : positionLy(entry.position),
          extentLy: entry.extent_ly ?? null,
        });
        break;
    }
  }
  return { streams, cores, progenitors: response.progenitors.map(toProgenitor) };
}

function toTrackPoint(wire: TrackPointWire): TrackPoint {
  return {
    positionLy: positionLy(wire.position),
    normalWidthLy: wire.normal_width_ly,
    binormalWidthLy: wire.binormal_width_ly,
    speedKmS: wire.speed_km_s,
    dispersionKmS: wire.dispersion_km_s ?? null,
  };
}

/** A stream's track as `stream_track` answers it: ready, or not built yet. */
export type StreamTrackAnswer =
  { readonly kind: "ready"; readonly track: StreamTrack } | { readonly kind: "pending" };

/**
 * A stream's track, or `pending` while the server has not built it.
 *
 * @remarks
 * A track from a server before P10.T11 has no length or dispersions, which read `null`.
 */
export function toStreamTrack(response: StreamTrackWire): StreamTrackAnswer {
  const { track } = response;
  let answer: StreamTrackAnswer;
  switch (track.status) {
    case "pending":
      answer = { kind: "pending" };
      break;
    case "ready":
      answer = {
        kind: "ready",
        track: {
          number: response.stream,
          lengthLy: track.length_ly ?? null,
          trailing: track.trailing.map(toTrackPoint),
          leading: track.leading.map(toTrackPoint),
        },
      };
      break;
  }
  return answer;
}
