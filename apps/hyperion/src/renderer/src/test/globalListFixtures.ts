/**
 * Plan 10's global-list answers for tests (P10.T11): `global_features` and `stream_track`, as a
 * server sends them.
 */
import type {
  AccretedProgenitorWire,
  GalacticPosition,
  GlobalEntryWire,
  ResponseFor,
  TrackPointWire,
  UniverseIdHex,
} from "@hyperion/protocol";

import { UNIVERSE_ID } from "./galaxyFixtures";

/** A position on the wire at whole light-years. */
export function atLy(x: number, y: number, z: number): GalacticPosition {
  return { cell_ly: [x, y, z], offset_m: [0, 0, 0] };
}

/** A stream entry of the list, as the wire tags it. */
export type StreamEntryWire = Extract<GlobalEntryWire, { kind: "stream" }>;

/** A dwarf core entry of the list, as the wire tags it. */
export type CoreEntryWire = Extract<GlobalEntryWire, { kind: "dwarf_core" }>;

/** A stream of the list: number 0, an orphan's, its track not yet held. */
export function aStreamEntry(overrides: Partial<StreamEntryWire> = {}): StreamEntryWire {
  return {
    kind: "stream",
    number: 0,
    mass_msun: 12_500,
    shell: { inner_ly: 20_000, outer_ly: 60_000 },
    progenitor: null,
    track: "pending",
    ...overrides,
  };
}

/** A dwarf core of the list: number 0, at (-30,000, 12,000, 8,000) ly, 3,375 ly in radius. */
export function aCoreEntry(overrides: Partial<CoreEntryWire> = {}): CoreEntryWire {
  return {
    kind: "dwarf_core",
    number: 0,
    mass_msun: 2.0e6,
    shell: { inner_ly: 30_000, outer_ly: 36_000 },
    position: atLy(-30_000, 12_000, 8_000),
    extent_ly: 3_375,
    ...overrides,
  };
}

/** The last major merger, which left nothing, and a recent dwarf that left stream 1 and core 0. */
export function someProgenitors(): AccretedProgenitorWire[] {
  return [
    {
      kind: "dominant_merger",
      mass_msun: 3.0e8,
      accreted_gyr: 10,
      pericentre_ly: 2_500,
      apocentre_ly: 80_000,
      stream: null,
      core: null,
    },
    {
      kind: "recent",
      mass_msun: 4.5e6,
      accreted_gyr: 2.75,
      pericentre_ly: 30_000,
      apocentre_ly: 120_000,
      stream: 1,
      core: 0,
    },
  ];
}

/** A galaxy's global list: the centre, then `entries`, and its progenitors. */
export function aGlobalFeatures({
  universe = UNIVERSE_ID,
  entries = [aStreamEntry()],
  progenitors = someProgenitors(),
}: {
  readonly universe?: UniverseIdHex;
  readonly entries?: GlobalEntryWire[];
  readonly progenitors?: AccretedProgenitorWire[];
} = {}): ResponseFor<"global_features"> {
  return {
    kind: "global_features",
    universe,
    entries: [
      { kind: "galactic_centre", mass_msun: 2.5e7, shell: { inner_ly: 0, outer_ly: 128 } },
      ...entries,
    ],
    progenitors,
  };
}

/** A track point at `position`, 150 and 250 ly wide, 210 km/s, 0.75 km/s of dispersion. */
export function aTrackPoint(position: GalacticPosition): TrackPointWire {
  return {
    position,
    normal_width_ly: 150,
    binormal_width_ly: 250,
    speed_km_s: 210,
    dispersion_km_s: 0.75,
  };
}

/**
 * Stream `stream`'s track, ready: its trailing arm `trailing` and its leading arm `leading`, as
 * points of whole light-years, 100,000 ly long.
 */
export function aStreamTrack(
  stream: number,
  trailing: ReadonlyArray<readonly [number, number, number]>,
  leading: ReadonlyArray<readonly [number, number, number]>,
): ResponseFor<"stream_track"> {
  return {
    kind: "stream_track",
    universe: UNIVERSE_ID,
    stream,
    track: {
      status: "ready",
      length_ly: 100_000,
      trailing: trailing.map(([x, y, z]) => aTrackPoint(atLy(x, y, z))),
      leading: leading.map(([x, y, z]) => aTrackPoint(atLy(x, y, z))),
    },
  };
}
