//! Plan 10's two global-list requests, checked, and their answers (P10.T10.a): `global_features`
//! and `stream_track`.
//!
//! Both read the galaxy's global list as P10.T4 builds it ([`GlobalList::build`]), at the epoch:
//!
//! - **`global_features`** lists every entry in the list's order, each with its kind, number, mass
//!   and bounding shell, a stream with the living globular it is stripped from, if it is; and the
//!   accreted progenitors the list's dwarf entries come from: the last major merger, then the
//!   recent ones in their history's order, each with the stream and the core it left, if any. The
//!   lesser old progenitors are not listed: their debris is the halo's smooth components, whose
//!   combined share `galaxy_parameters` sends.
//! - **`stream_track`** resolves the stream's number in the list, builds its tube table
//!   ([`TubeTable::build`], a second or two of spray in the job that asks) and sends each arm's
//!   knots, thinned evenly to at most [`TRACK_POINTS`] with both ends kept.
//!
//! This server holds no tube table between requests, so every stream's track status is `pending`
//! and `stream_track` always builds; P10.T10.b's cache and warm-up hold them, and its tracks are
//! `ready` once warmed.

use hyperion_protocol::{
    AccretedProgenitorWire, ErrorCode, GlobalEntryWire, GlobalFeatures, GlobalFeaturesRequest,
    ProgenitorKindWire, RequestError, ShellWire, StreamTrackRequest, StreamTrackWire,
    TrackPointWire, TrackStatusWire, TrackWire,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::global_list::spray::Arm;
use hyperion_sim::galaxy::global_list::tube::TubeKnot;
use hyperion_sim::galaxy::global_list::{
    BoundingShell, GlobalEntryKind, GlobalList, StreamNumber, StreamOrigin, TubeTable,
};
use hyperion_sim::galaxy::params::{Progenitor, ProgenitorKind};
use hyperion_sim::units::KilometresPerSecond;

use super::features::feature_id_hex;
use super::{galactic_position, gyr};

/// The most points an arm of a stream's track is sent with (plan 10, P10.T10.a): half a tube arm's
/// 256 knots.
pub(crate) const TRACK_POINTS: usize = 128;

/// The galaxy's global list as `global_features` sends it, for `request`.
#[must_use]
pub(crate) fn global_features(
    galaxy: &Galaxy,
    list: &GlobalList,
    request: GlobalFeaturesRequest,
) -> GlobalFeatures {
    let entries = list
        .entries()
        .iter()
        .map(|entry| {
            let (mass_msun, shell) = (entry.mass().value(), shell_wire(entry.shell()));
            match entry.kind() {
                GlobalEntryKind::Centre => GlobalEntryWire::GalacticCentre { mass_msun, shell },
                GlobalEntryKind::Stream(number) => {
                    let spec = list
                        .stream(number)
                        .expect("a listed stream is in the debris");
                    let progenitor = match spec.origin() {
                        StreamOrigin::LivingGlobular(feature) => Some(feature_id_hex(feature)),
                        StreamOrigin::Orphan | StreamOrigin::Dwarf(_) => None,
                    };
                    GlobalEntryWire::Stream {
                        number: number.get(),
                        mass_msun,
                        shell,
                        progenitor,
                        track: TrackStatusWire::Pending,
                    }
                }
                GlobalEntryKind::DwarfCore(number) => GlobalEntryWire::DwarfCore {
                    number: u16::from(number.get()),
                    mass_msun,
                    shell,
                },
            }
        })
        .collect();
    let progenitors = galaxy
        .params()
        .accretion()
        .progenitors()
        .iter()
        .filter_map(|progenitor| accreted_progenitor(list, progenitor))
        .collect();
    GlobalFeatures {
        universe: request.universe,
        entries,
        progenitors,
    }
}

/// `shell` as the wire carries it: its radii, in light-years.
#[must_use]
fn shell_wire(shell: BoundingShell) -> ShellWire {
    ShellWire {
        inner_ly: shell.inner().value(),
        outer_ly: shell.outer().value(),
    }
}

/// `progenitor` as `global_features` lists it, with the stream and the core it left on `list`, or
/// `None` for a lesser old progenitor, which is not listed.
fn accreted_progenitor(
    list: &GlobalList,
    progenitor: &Progenitor,
) -> Option<AccretedProgenitorWire> {
    let (kind, recent) = match progenitor.kind() {
        ProgenitorKind::DominantMerger => (ProgenitorKindWire::DominantMerger, None),
        ProgenitorKind::Recent(j) => (ProgenitorKindWire::Recent, Some(j)),
        ProgenitorKind::Lesser(_) => return None,
    };
    let debris = list.debris();
    let stream = recent.and_then(|j| {
        debris
            .streams()
            .iter()
            .find(|spec| spec.origin() == StreamOrigin::Dwarf(j))
            .map(|spec| spec.number().get())
    });
    let core = recent.and_then(|j| {
        debris
            .cores()
            .iter()
            .find(|spec| spec.progenitor() == j)
            .map(|spec| u16::from(spec.number().get()))
    });
    let orbit = progenitor.orbit();
    Some(AccretedProgenitorWire {
        kind,
        mass_msun: progenitor.mass().value(),
        accreted_gyr: gyr(progenitor.accreted()),
        pericentre_ly: orbit.pericentre().value(),
        apocentre_ly: orbit.apocentre().value(),
        stream,
        core,
    })
}

/// The stream `request` names on `list`.
///
/// # Errors
///
/// `unknown_feature` naming `stream` for a number the list has no stream for.
pub(crate) fn stream_track_number(
    list: &GlobalList,
    request: &StreamTrackRequest,
) -> Result<StreamNumber, RequestError> {
    // A number of 4,096 or more is no stream's either: it is refused as one the list lacks, not
    // as malformed, since the request names a stream either way.
    StreamNumber::new(request.stream)
        .ok()
        .filter(|&number| list.stream(number).is_some())
        .ok_or_else(|| RequestError {
            code: ErrorCode::UnknownFeature,
            message: format!(
                "stream {} names no stream of this universe's global list",
                request.stream
            ),
            field: Some("stream".to_owned()),
        })
}

/// Stream `number`'s track on `list` as `stream_track` sends it, for `request`, its tube table
/// built from `galaxy`.
///
/// # Errors
///
/// `internal` if the tube cannot be built, which no stream of a galaxy with the kinematic tables
/// the server builds meets (plan 10's Risks, "P10.T6.a as built").
///
/// # Panics
///
/// If the list has no stream `number`, which [`stream_track_number`] checks first.
pub(crate) fn stream_track(
    galaxy: &Galaxy,
    list: &GlobalList,
    number: StreamNumber,
    request: StreamTrackRequest,
) -> Result<StreamTrackWire, RequestError> {
    let spec = list.stream(number).expect("the stream was checked");
    let tube = TubeTable::build(galaxy, spec).map_err(|error| RequestError {
        code: ErrorCode::Internal,
        message: format!("stream {}'s tube could not be built: {error}", number.get()),
        field: None,
    })?;
    let [trailing, leading] =
        [Arm::Trailing, Arm::Leading].map(|arm| track_of(tube.arm(arm).knots()));
    Ok(StreamTrackWire {
        universe: request.universe,
        stream: request.stream,
        track: TrackWire::Ready { trailing, leading },
    })
}

/// The indices, in order, of the knots of an arm of `n` that its track sends: every one for an arm
/// of at most [`TRACK_POINTS`], else [`TRACK_POINTS`] spaced evenly in index with both ends kept.
fn track_indices(n: usize) -> impl Iterator<Item = usize> {
    let (points, last) = (n.min(TRACK_POINTS), n.saturating_sub(1));
    (0..points).map(move |k| {
        if n <= TRACK_POINTS {
            k
        } else {
            k * last / (TRACK_POINTS - 1)
        }
    })
}

/// An arm's knots, in their order, as a track's points ([`track_indices`]).
#[must_use]
fn track_of(knots: &[TubeKnot]) -> Vec<TrackPointWire> {
    track_indices(knots.len())
        .map(|k| {
            let knot = &knots[k];
            let [normal, binormal] = knot.widths();
            TrackPointWire {
                position: galactic_position(&knot.centre()),
                normal_width_ly: normal.value(),
                binormal_width_ly: binormal.value(),
                speed_km_s: KilometresPerSecond::from(knot.velocity().speed()).value(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_protocol::UniverseIdHex;
    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::params::GalaxyParams;

    use super::*;

    fn universe() -> UniverseIdHex {
        UniverseIdHex::from_u64(42)
    }

    /// The Milky Way fixture with its kinematic tables, and its list.
    fn fixture() -> &'static (Galaxy, GlobalList) {
        static FIXTURE: OnceLock<(Galaxy, GlobalList)> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())
                .expect("the fixture builds")
                .with_full_potential();
            let list = GlobalList::build(&galaxy);
            (galaxy, list)
        })
    }

    /// P10.T10.a: every entry is listed in the list's order with its kind, number, mass and
    /// shell; a living globular's stream names its globular, and every stream is `pending`, since
    /// this server holds no tube; and the progenitors are the last major merger, then the recent
    /// ones, each naming the stream and the core it left.
    #[test]
    #[ignore = "slow: builds a Milky Way galaxy's kinematic tables and its global list"]
    fn global_features_lists_every_entry_and_the_accreted_progenitors() {
        let (galaxy, list) = fixture();
        let answer = global_features(
            galaxy,
            list,
            GlobalFeaturesRequest {
                universe: universe(),
            },
        );
        assert_eq!(answer.universe, universe());
        assert_eq!(answer.entries.len(), list.entries().len());
        let mut living_streams = 0;
        for (wire, entry) in answer.entries.iter().zip(list.entries()) {
            let shell = shell_wire(entry.shell());
            match (entry.kind(), wire) {
                (
                    GlobalEntryKind::Centre,
                    GlobalEntryWire::GalacticCentre {
                        mass_msun,
                        shell: s,
                    },
                ) => {
                    assert_eq!(mass_msun.to_bits(), entry.mass().value().to_bits());
                    assert_eq!(*s, shell);
                }
                (
                    GlobalEntryKind::Stream(number),
                    GlobalEntryWire::Stream {
                        number: n,
                        mass_msun,
                        shell: s,
                        progenitor,
                        track,
                    },
                ) => {
                    assert_eq!(*n, number.get());
                    assert_eq!(mass_msun.to_bits(), entry.mass().value().to_bits());
                    assert_eq!((*s, *track), (shell, TrackStatusWire::Pending));
                    let living = matches!(
                        list.stream(number).expect("listed").origin(),
                        StreamOrigin::LivingGlobular(_)
                    );
                    assert_eq!(progenitor.is_some(), living);
                    living_streams += usize::from(living);
                }
                (
                    GlobalEntryKind::DwarfCore(number),
                    GlobalEntryWire::DwarfCore {
                        number: n,
                        mass_msun,
                        shell: s,
                    },
                ) => {
                    assert_eq!(*n, u16::from(number.get()));
                    assert_eq!(mass_msun.to_bits(), entry.mass().value().to_bits());
                    assert_eq!(*s, shell);
                }
                (kind, wire) => panic!("{kind:?} sent as {wire:?}"),
            }
        }
        assert!(living_streams > 0);
        let progenitors = &answer.progenitors;
        assert_eq!(progenitors[0].kind, ProgenitorKindWire::DominantMerger);
        assert_eq!((progenitors[0].stream, progenitors[0].core), (None, None));
        let recent = galaxy
            .params()
            .accretion()
            .progenitors()
            .iter()
            .filter(|p| matches!(p.kind(), ProgenitorKind::Recent(_)))
            .count();
        assert_eq!(progenitors.len(), 1 + recent);
        assert!(
            progenitors[1..]
                .iter()
                .all(|p| p.kind == ProgenitorKindWire::Recent)
        );
        for wire in progenitors {
            assert!(wire.pericentre_ly < wire.apocentre_ly, "{wire:?}");
            if let Some(stream) = wire.stream {
                let number = StreamNumber::new(stream).expect("a stream's number");
                let spec = list.stream(number).expect("listed");
                assert!(matches!(spec.origin(), StreamOrigin::Dwarf(_)));
            }
        }
        let cores = list.debris().cores().len();
        assert_eq!(
            progenitors.iter().filter(|p| p.core.is_some()).count(),
            cores
        );
    }

    /// P10.T10.a: a stream's track is its tube's knots, at most 128 an arm with both ends kept, each
    /// with its widths and its mean speed; a number the list has no stream for is
    /// `unknown_feature`, naming `stream`.
    #[test]
    #[ignore = "slow: builds a Milky Way galaxy's kinematic tables, its global list and a tube"]
    fn a_stream_s_track_is_its_tube_s_knots_thinned_to_128_an_arm() {
        let (galaxy, list) = fixture();
        let request = StreamTrackRequest {
            universe: universe(),
            stream: 0,
        };
        let number = stream_track_number(list, &request).expect("stream 0 is listed");
        let answer = stream_track(galaxy, list, number, request).expect("the tube builds");
        assert_eq!((answer.universe, answer.stream), (universe(), 0));
        let TrackWire::Ready { trailing, leading } = answer.track else {
            panic!("a built track is ready");
        };
        let tube = TubeTable::build(galaxy, list.stream(number).expect("listed")).expect("builds");
        for (points, arm) in [(&trailing, Arm::Trailing), (&leading, Arm::Leading)] {
            let knots = tube.arm(arm).knots();
            assert_eq!(points.len(), TRACK_POINTS.min(knots.len()));
            for (point, knot) in [
                (points.first(), knots.first()),
                (points.last(), knots.last()),
            ] {
                let (point, knot) = (point.expect("a point"), knot.expect("a knot"));
                assert_eq!(point.position, galactic_position(&knot.centre()));
                assert_eq!(
                    point.normal_width_ly.to_bits(),
                    knot.widths()[0].value().to_bits()
                );
            }
            assert!(
                points
                    .iter()
                    .all(|p| p.speed_km_s > 0.0 && p.binormal_width_ly > 0.0)
            );
        }
        let beyond = StreamTrackRequest {
            universe: universe(),
            stream: u16::try_from(list.debris().streams().len()).expect("small"),
        };
        // Past the list's streams, and past any stream's number.
        for stream in [beyond.stream, u16::MAX] {
            let request = StreamTrackRequest {
                universe: universe(),
                stream,
            };
            let refused = stream_track_number(list, &request).expect_err("no such stream");
            assert_eq!(refused.code, ErrorCode::UnknownFeature);
            assert_eq!(refused.field.as_deref(), Some("stream"));
        }
    }

    /// P10.T10.a: an arm of up to 128 knots is sent whole, and a longer one thinned evenly with
    /// both ends kept, in order.
    #[test]
    fn an_arm_is_thinned_evenly_with_its_ends_kept() {
        let indices = |n: usize| track_indices(n).collect::<Vec<_>>();
        assert_eq!(indices(5), vec![0, 1, 2, 3, 4]);
        assert_eq!(indices(TRACK_POINTS), (0..TRACK_POINTS).collect::<Vec<_>>());
        let thinned = indices(256);
        assert_eq!(thinned.len(), TRACK_POINTS);
        assert_eq!((thinned[0], thinned[TRACK_POINTS - 1]), (0, 255));
        assert!(thinned.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
