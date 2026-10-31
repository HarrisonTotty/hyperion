//! The global list's messages (galaxy plan 10, P10.T10.a): the galaxy's streams, dwarf cores and
//! centre with its accreted progenitors (`global_features`), and one stream's track
//! (`stream_track`).
//!
//! These are wire types only, as every type of this crate is: the server converts plan 10's
//! global list to them, and nothing here depends on the simulation crate. The list is the
//! galaxy's at the epoch: its entries do not move in a clock window, so neither request names a
//! time. A type's shape is fixed by the first build that sends a value of it, this one
//! (`PROTOCOL_VERSION`'s documentation). Every enum here is closed, carrying all its variants
//! from this build.
//!
//! Plan 10's members of streams and cores are what a range row's
//! [`SystemOriginWire`](crate::SystemOriginWire) calls `stream` and `dwarf_core`; no row carries
//! either until P10.T10.b registers the list's source with the range query.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::features::FeatureKindWire;
use crate::primitives::{FeatureIdHex, GalacticPosition, UniverseIdHex};

/// Asks for a galaxy's global list (`global_features`), answered with [`GlobalFeatures`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GlobalFeaturesRequest {
    /// The universe whose galaxy is listed.
    pub universe: UniverseIdHex,
}

/// A galaxy's global list: its galactic centre, streams and dwarf cores, and the progenitors
/// accreted since its last major merger.
///
/// `universe` echoes the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GlobalFeatures {
    /// The universe listed.
    pub universe: UniverseIdHex,
    /// The list's entries in its own order: the galactic centre, then the streams by number,
    /// then the dwarf cores by number.
    pub entries: Vec<GlobalEntryWire>,
    /// The last major merger, then the recent progenitors in their accretion history's order.
    pub progenitors: Vec<AccretedProgenitorWire>,
}

/// One entry of the global list: what it is, its mass, and the shell about the galactic centre
/// that holds every member it places.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GlobalEntryWire {
    /// What the entry is: `galactic_centre`, `stream` or `dwarf_core`.
    pub kind: FeatureKindWire,
    /// The stream's or the core's number on the list, what `stream_track` and a member's origin
    /// name it by; `null` for the galactic centre.
    pub number: Option<u16>,
    /// Its mass, in solar masses: a stream's in its tube, a core's still bound, and the centre's
    /// within its reach.
    pub mass_msun: f64,
    /// The bounding shell's inner radius about the galactic centre, in light-years.
    pub shell_inner_ly: f64,
    /// The bounding shell's outer radius, in light-years: at most the root cube's corner.
    pub shell_outer_ly: f64,
    /// The living globular cluster a stream is still being stripped from; `null` for an orphan's
    /// stream, a dwarf's, and every other entry.
    pub progenitor: Option<FeatureIdHex>,
    /// Whether `stream_track` can answer the stream's track at once; `null` for every entry but a
    /// stream.
    pub track: Option<TrackStatusWire>,
}

/// Whether the server holds a stream's track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TrackStatusWire {
    /// The server holds the stream's tube, and `stream_track` answers its track at once.
    Ready,
    /// The server does not hold it yet. `stream_track` builds it, which takes it a second or
    /// more, or answers `pending` while it is being built.
    Pending,
}

/// One accreted galaxy of the galaxy's history, and the entries it left on the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AccretedProgenitorWire {
    /// Which progenitor it is.
    pub kind: ProgenitorKindWire,
    /// Its stellar mass, in solar masses.
    pub mass_msun: f64,
    /// How long before the epoch it was accreted, in gigayears.
    pub accreted_gyr: f64,
    /// Its orbit's pericentre about the galactic centre, in light-years.
    pub pericentre_ly: f64,
    /// Its orbit's apocentre, in light-years.
    pub apocentre_ly: f64,
    /// The number of the stream it left on the list, if it left one.
    pub stream: Option<u16>,
    /// The number of its core on the list, if any of it is still bound inside the root cube.
    pub core: Option<u16>,
}

/// Which accreted galaxy a progenitor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ProgenitorKindWire {
    /// The last major merger, whose debris is the stellar halo's dominant component.
    DominantMerger,
    /// A dwarf galaxy accreted in the last 6 Gyr.
    Recent,
}

/// Asks for one stream's track (`stream_track`), answered with [`StreamTrackWire`].
///
/// The server resolves the number first: one the galaxy's list has no stream for is refused as
/// `unknown_feature`, naming `stream`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StreamTrackRequest {
    /// The universe the stream is in.
    pub universe: UniverseIdHex,
    /// The stream's number on the galaxy's global list.
    pub stream: u16,
}

/// One stream's track, or why it is not here yet.
///
/// `universe` and `stream` echo the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StreamTrackWire {
    /// The universe the stream is in.
    pub universe: UniverseIdHex,
    /// The stream's number.
    pub stream: u16,
    /// Its track, or `pending`.
    pub track: TrackWire,
}

/// A stream's track, tagged by `status` with the strings of [`TrackStatusWire`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
pub enum TrackWire {
    /// The track: each arm a polyline along its centre line, at the epoch.
    Ready {
        /// The trailing arm, from its tip to its root at the progenitor: at most 128 points.
        trailing: Vec<TrackPointWire>,
        /// The leading arm, from its root at the progenitor to its tip: at most 128 points.
        leading: Vec<TrackPointWire>,
    },
    /// Not built yet: ask again.
    Pending,
}

/// One point of a stream's track: a knot of its tube.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TrackPointWire {
    /// The centre line's point, in the `GALACTIC` frame.
    pub position: GalacticPosition,
    /// The tube's rms width there along the frame's normal, which lies in the stream's orbital
    /// plane, in light-years.
    pub normal_width_ly: f64,
    /// Its rms width along the binormal, near the orbit's pole, in light-years.
    pub binormal_width_ly: f64,
    /// The tracers' mean speed there, galactic, in km/s.
    pub speed_km_s: f64,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::envelope::{RequestBody, ResponseBody};
    use crate::testing::{assert_wire_form, assert_wire_strings};

    const UNIVERSE: u64 = 0x0123_4567_89ab_cdef;

    /// A living globular's feature ID.
    const GLOBULAR: u64 = 0xe842_0002_8000_0000;

    fn universe() -> UniverseIdHex {
        UniverseIdHex::from_u64(UNIVERSE)
    }

    /// The list's centre and a living globular's stream.
    pub(crate) fn entries() -> Vec<GlobalEntryWire> {
        vec![
            GlobalEntryWire {
                kind: FeatureKindWire::GalacticCentre,
                number: None,
                mass_msun: 2.5e7,
                shell_inner_ly: 0.0,
                shell_outer_ly: 128.0,
                progenitor: None,
                track: None,
            },
            GlobalEntryWire {
                kind: FeatureKindWire::Stream,
                number: Some(3),
                mass_msun: 1.25e4,
                shell_inner_ly: 28_689.0,
                shell_outer_ly: 64_267.5,
                progenitor: Some(FeatureIdHex::from_u64(GLOBULAR)),
                track: Some(TrackStatusWire::Pending),
            },
        ]
    }

    fn entries_json() -> Value {
        json!([
            {
                "kind": "galactic_centre",
                "number": null,
                "mass_msun": 2.5e7,
                "shell_inner_ly": 0.0,
                "shell_outer_ly": 128.0,
                "progenitor": null,
                "track": null,
            },
            {
                "kind": "stream",
                "number": 3,
                "mass_msun": 1.25e4,
                "shell_inner_ly": 28_689.0,
                "shell_outer_ly": 64_267.5,
                "progenitor": "e842000280000000",
                "track": "pending",
            },
        ])
    }

    /// The last major merger, which left no entry, and a recent dwarf that left a stream and a
    /// core.
    pub(crate) fn progenitors() -> Vec<AccretedProgenitorWire> {
        vec![
            AccretedProgenitorWire {
                kind: ProgenitorKindWire::DominantMerger,
                mass_msun: 3.0e8,
                accreted_gyr: 10.0,
                pericentre_ly: 2_500.0,
                apocentre_ly: 80_000.0,
                stream: None,
                core: None,
            },
            AccretedProgenitorWire {
                kind: ProgenitorKindWire::Recent,
                mass_msun: 4.5e6,
                accreted_gyr: 2.75,
                pericentre_ly: 30_000.0,
                apocentre_ly: 120_000.0,
                stream: Some(7),
                core: Some(0),
            },
        ]
    }

    fn progenitors_json() -> Value {
        json!([
            {
                "kind": "dominant_merger",
                "mass_msun": 3.0e8,
                "accreted_gyr": 10.0,
                "pericentre_ly": 2_500.0,
                "apocentre_ly": 80_000.0,
                "stream": null,
                "core": null,
            },
            {
                "kind": "recent",
                "mass_msun": 4.5e6,
                "accreted_gyr": 2.75,
                "pericentre_ly": 30_000.0,
                "apocentre_ly": 120_000.0,
                "stream": 7,
                "core": 0,
            },
        ])
    }

    /// A track point near the Sun's distance.
    fn point() -> TrackPointWire {
        TrackPointWire {
            position: GalacticPosition {
                cell_ly: [26_000, 0, 50],
                offset_m: [0.0, 0.0, 4_730_365_236_290_400.0],
            },
            normal_width_ly: 154.0,
            binormal_width_ly: 251.5,
            speed_km_s: 212.5,
        }
    }

    fn point_json() -> Value {
        json!({
            "position": { "cell_ly": [26_000, 0, 50], "offset_m": [0.0, 0.0, 4_730_365_236_290_400.0] },
            "normal_width_ly": 154.0,
            "binormal_width_ly": 251.5,
            "speed_km_s": 212.5,
        })
    }

    #[test]
    fn global_features_request_wire_form() {
        assert_wire_form(
            &RequestBody::GlobalFeatures(GlobalFeaturesRequest {
                universe: universe(),
            }),
            json!({ "kind": "global_features", "universe": "0123456789abcdef" }),
        );
    }

    #[test]
    fn global_features_wire_form() {
        assert_wire_form(
            &ResponseBody::GlobalFeatures(GlobalFeatures {
                universe: universe(),
                entries: entries(),
                progenitors: progenitors(),
            }),
            json!({
                "kind": "global_features",
                "universe": "0123456789abcdef",
                "entries": entries_json(),
                "progenitors": progenitors_json(),
            }),
        );
    }

    #[test]
    fn stream_track_request_wire_form() {
        assert_wire_form(
            &RequestBody::StreamTrack(StreamTrackRequest {
                universe: universe(),
                stream: 3,
            }),
            json!({ "kind": "stream_track", "universe": "0123456789abcdef", "stream": 3 }),
        );
    }

    #[test]
    fn a_ready_stream_track_wire_form() {
        assert_wire_form(
            &ResponseBody::StreamTrack(StreamTrackWire {
                universe: universe(),
                stream: 3,
                track: TrackWire::Ready {
                    trailing: vec![point()],
                    leading: vec![point(), point()],
                },
            }),
            json!({
                "kind": "stream_track",
                "universe": "0123456789abcdef",
                "stream": 3,
                "track": {
                    "status": "ready",
                    "trailing": [point_json()],
                    "leading": [point_json(), point_json()],
                },
            }),
        );
    }

    #[test]
    fn a_pending_stream_track_wire_form() {
        assert_wire_form(
            &ResponseBody::StreamTrack(StreamTrackWire {
                universe: universe(),
                stream: 3,
                track: TrackWire::Pending,
            }),
            json!({
                "kind": "stream_track",
                "universe": "0123456789abcdef",
                "stream": 3,
                "track": { "status": "pending" },
            }),
        );
    }

    #[test]
    fn the_track_statuses_and_progenitor_kinds_wire_strings() {
        assert_wire_strings(&[
            (TrackStatusWire::Ready, "ready"),
            (TrackStatusWire::Pending, "pending"),
        ]);
        assert_wire_strings(&[
            (ProgenitorKindWire::DominantMerger, "dominant_merger"),
            (ProgenitorKindWire::Recent, "recent"),
        ]);
    }
}
