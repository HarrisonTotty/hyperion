//! The last skies' final replies, kept (rendering plan R06, R06.T11.l; the owner's priority,
//! 2026-11-01): a sky asked again exactly as one already finished is answered from memory, with no
//! sky job on the pool, only the one that serialises its reply's frame.
//!
//! VIEW asks its camera's sky as each inspection opens, and a relaunched client or a reopened
//! inspection asks the same sky again. Near the Sun its census is the full cold one, hours of CPU
//! (R06's "the full cold census at the real boundary"), and the sky's cell cache is off by default
//! (R06.T8.o), so each paid it all again. A final reply is a pure function of its request and its
//! galaxy, the server's own configuration being fixed for its life, so it is kept by a key of every
//! field of the request, and served again as it was: the same JSON and the same payload.
//!
//! Only a final reply is kept, once made, so a sky asked again while the first is still under way
//! computes its own. [`SKY_REPLY_CACHE_BYTES`] bounds what is kept: the last one or two skies.
//!
//! Each is also written to disk under the data directory ([`disk`], R06.T11.n), so that a sky asked
//! again after a restart is answered from its file.
//!
//! VIEW asks at the scene's time and from the camera, so with the clock running a relaunch asks a
//! sky that differs from every kept one in its time, and from a moving frame in its observer too.
//! Such a sky is answered with a kept reply of its arrival, the same request but for those two
//! ([`SkyArrival`]), that still holds for it ([`SkyHold`], R06.T11.p): as it was sent, its own
//! `time`, `observer` and `valid_until` unchanged, so that the client holds it just as it would
//! have held it had it never asked again.

mod disk;

pub use disk::SkyFileCounters;
pub(crate) use disk::{SKY_FILES_DIR, SkyReplyFiles, SkySettings};

use std::cmp::Reverse;
use std::mem::size_of;

use hyperion_protocol::{
    ConeDto, EyeDto, GalacticPosition, HostDiscDto, ResponseBody, SkyGapDto, SkyLayerCensusDto,
    SkyRequest, SkyResponse, SystemIdHex, UniverseIdHex, UniverseTime,
};
use hyperion_sim::coords::{GalacticPosition as SimPosition, LyCell};
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;

use super::TENTH_OF_A_PIXEL_RAD;
use crate::bulk::sky::nearest_sky_star_ly;
use crate::bulk::{Answer, BulkPayload};
use crate::cache::{ByteLru, HeapBytes, Insertion, LruCounters, SharedByteLru};
use crate::compute::GalaxyKey;
use crate::config::KeptSkies;
use crate::limits::SKY_REPLY_CACHE_BYTES;

/// What a sky's final reply depends on: its galaxy and every field of its request, each float by its
/// bits, so that two keys are equal only where the requests are identical.
///
/// It is built by destructuring the request and each of its parts in full, so a field added to any
/// of them fails to compile here until the key holds it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SkyReplyKey {
    arrival: SkyArrival,
    /// The observer's cell, ly, and its offset in it, m, by its bits.
    observer: ([i32; 3], [u64; 3]),
    time: UniverseTime,
}

/// What a sky's key holds but where and when it is seen: its galaxy, its universe, the eye, the
/// limits, the cone and the system left out, each float by its bits (R06.T11.p). Two skies of one
/// arrival differ only in their observers and times, as a client's re-asks within one system do.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SkyArrival {
    galaxy: GalaxyKey,
    universe: UniverseIdHex,
    /// The eye's field factor, age and pigmentation, by their bits.
    eye: Option<[u64; 3]>,
    camera_limit_v: Option<u64>,
    n_max: Option<u32>,
    /// The cone's axis and half-angle, by their bits.
    cone: Option<([u64; 3], u64)>,
    exclude_system: Option<SystemIdHex>,
}

impl SkyReplyKey {
    /// The key of `request`'s final reply, in the galaxy `galaxy`: the one its universe names.
    #[must_use]
    pub(crate) fn of(galaxy: GalaxyKey, request: &SkyRequest) -> Self {
        let SkyRequest {
            universe,
            observer,
            time,
            eye,
            camera_limit_v,
            n_max,
            cone,
            exclude_system,
        } = request;
        let eye = eye.map(
            |EyeDto {
                 field_factor,
                 age_years,
                 pigmentation,
             }| [field_factor, age_years, pigmentation].map(f64::to_bits),
        );
        let cone = cone.map(
            |ConeDto {
                 axis,
                 half_angle_deg,
             }| (axis.map(f64::to_bits), half_angle_deg.to_bits()),
        );
        let arrival = SkyArrival {
            galaxy,
            universe: universe.clone(),
            eye,
            camera_limit_v: camera_limit_v.map(f64::to_bits),
            n_max: *n_max,
            cone,
            exclude_system: exclude_system.clone(),
        };
        arrival.at(observer, *time)
    }

    /// Its arrival: every field but the observer and the time.
    #[must_use]
    pub(crate) fn arrival(&self) -> &SkyArrival {
        &self.arrival
    }

    /// The time it is seen at.
    #[must_use]
    pub(crate) fn time(&self) -> UniverseTime {
        self.time
    }

    /// Appends the key's bytes to `out`, every field in turn, little-endian, each optional one
    /// after a byte saying whether it is there and each text after its length: what a kept sky's
    /// file holds and is named by ([`disk`]).
    pub(crate) fn write_bytes(&self, out: &mut Vec<u8>) {
        let Self {
            arrival,
            observer: (cell_ly, offset_m),
            time: UniverseTime { seconds, nanos },
        } = self;
        arrival.write_head(out);
        for cell in cell_ly {
            out.extend(cell.to_le_bytes());
        }
        for bits in offset_m {
            out.extend(bits.to_le_bytes());
        }
        out.extend(seconds.to_le_bytes());
        out.extend(nanos.to_le_bytes());
        arrival.write_tail(out);
    }
}

impl SkyArrival {
    /// The key of this arrival's sky seen from `observer` at `time`.
    #[must_use]
    pub(crate) fn at(&self, observer: &GalacticPosition, time: UniverseTime) -> SkyReplyKey {
        let GalacticPosition { cell_ly, offset_m } = observer;
        SkyReplyKey {
            arrival: self.clone(),
            observer: (*cell_ly, offset_m.map(f64::to_bits)),
            time,
        }
    }

    /// Appends the arrival's bytes to `out`, as [`SkyReplyKey::write_bytes`] writes them without
    /// the observer and the time: what a kept sky's file's name leads with ([`disk`]).
    pub(crate) fn write_bytes(&self, out: &mut Vec<u8>) {
        self.write_head(out);
        self.write_tail(out);
    }

    /// The galaxy and the universe, which a key's bytes lead with.
    fn write_head(&self, out: &mut Vec<u8>) {
        out.extend(self.galaxy.seed().to_le_bytes());
        out.extend(self.galaxy.generator_version().get().to_le_bytes());
        write_text(out, self.universe.as_str());
    }

    /// The eye, the limits, the cone and the system left out, which a key's bytes end with.
    fn write_tail(&self, out: &mut Vec<u8>) {
        let Self {
            galaxy: _,
            universe: _,
            eye,
            camera_limit_v,
            n_max,
            cone,
            exclude_system,
        } = self;
        write_option(out, eye.as_ref(), |out, eye| {
            for bits in eye {
                out.extend(bits.to_le_bytes());
            }
        });
        write_option(out, camera_limit_v.as_ref(), |out, bits| {
            out.extend(bits.to_le_bytes());
        });
        write_option(out, n_max.as_ref(), |out, n_max| {
            out.extend(n_max.to_le_bytes());
        });
        write_option(out, cone.as_ref(), |out, (axis, half_angle)| {
            for bits in axis {
                out.extend(bits.to_le_bytes());
            }
            out.extend(half_angle.to_le_bytes());
        });
        write_option(out, exclude_system.as_ref(), |out, system| {
            write_text(out, system.as_str());
        });
    }
}

/// Appends `text` after its length in bytes, a `u32`.
fn write_text(out: &mut Vec<u8>, text: &str) {
    let len =
        u32::try_from(text.len()).expect("an ID's hex digits are far fewer than a u32 counts");
    out.extend(len.to_le_bytes());
    out.extend(text.as_bytes());
}

/// Appends 0 where `value` is `None`, and otherwise 1 and what `write` appends of it.
fn write_option<T>(out: &mut Vec<u8>, value: Option<&T>, write: impl FnOnce(&mut Vec<u8>, &T)) {
    match value {
        Some(value) => {
            out.push(1);
            write(out, value);
        }
        None => out.push(0),
    }
}

/// Where and when a kept sky was seen, and how long after and how far from there it holds
/// (R06.T11.p; Design note 13): from its own time until its `valid_until`, and within a tenth of a
/// pixel at 1080p across 60° of its nearest listed star's parallax, the bound by which a client
/// holds the sky it has (`parallaxShiftPx`). A client measures that shift from its own request's
/// observer, so a kept sky holds only for an observer that near its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SkyHold {
    /// Its observer as its request asked it: what its key holds.
    observer: GalacticPosition,
    /// Its time, from which it holds.
    time: UniverseTime,
    /// Its `valid_until`, the first time it no longer holds.
    until: UniverseTime,
    /// Its observer, checked.
    position: SimPosition,
    /// How near an observer must be to its own, m: the tenth of a pixel's angle times its nearest
    /// listed star's distance; 0 where it lists none, so that it holds at its own observer alone.
    within_m: f64,
}

impl SkyHold {
    /// The hold of `answer`, a sky's final reply, its nearest star read from its payload as a
    /// client reads it; `None` for any other answer.
    #[must_use]
    pub(crate) fn of(answer: &Answer) -> Option<Self> {
        let ResponseBody::Sky(sky) = &answer.body else {
            return None;
        };
        if !sky.is_final {
            return None;
        }
        let stars = usize::try_from(sky.stars_bytes).ok()?;
        let stars = answer.bulk.as_ref()?.bytes().get(..stars)?;
        let GalacticPosition { cell_ly, offset_m } = sky.observer;
        let position = SimPosition::new(LyCell::new(cell_ly), offset_m).ok()?;
        let within_m = nearest_sky_star_ly(stars).map_or(0.0, |nearest_ly| {
            TENTH_OF_A_PIXEL_RAD * f64::from(nearest_ly) * METRES_PER_LIGHT_YEAR
        });
        Some(Self {
            observer: sky.observer,
            time: sky.time,
            until: sky.valid_until,
            position,
            within_m,
        })
    }

    /// The key of the sky it holds, of `arrival`: the one its file holds and its reply is kept by.
    #[must_use]
    pub(crate) fn key(&self, arrival: &SkyArrival) -> SkyReplyKey {
        arrival.at(&self.observer, self.time)
    }

    /// How far `at` is from its observer, m, if it holds for a sky seen from there at `time`: at its
    /// own time or later and before its `valid_until`, from its own observer or nearer to it than
    /// its bound.
    #[must_use]
    pub(crate) fn holds(&self, time: UniverseTime, at: &SimPosition) -> Option<f64> {
        if time < self.time || time >= self.until {
            return None;
        }
        let offset_m = self.position.distance_to(at).value();
        // A distance is never below zero, so at most zero is its own observer's.
        (offset_m <= 0.0 || offset_m < self.within_m).then_some(offset_m)
    }

    /// How a lookup ranks the kept skies that hold, at `offset_m` from the observer asked: the
    /// latest seen first, then the nearest.
    #[must_use]
    pub(crate) fn rank(&self, offset_m: f64) -> (UniverseTime, Reverse<u64>) {
        // A distance's bits order as it does, since it is finite and never negative.
        (self.time, Reverse(offset_m.to_bits()))
    }
}

/// A kept sky that holds for one asked ([`SkyHold::holds`]): its own key, its final reply, and how
/// far its observer is from the one asked, m.
#[derive(Debug)]
pub(crate) struct HeldSky {
    /// The key it is kept by: the arrival seen where and when it was.
    pub(crate) key: SkyReplyKey,
    /// Its final reply, as it was sent.
    pub(crate) answer: Answer,
    /// How far its observer is from the one asked, m.
    pub(crate) offset_m: f64,
}

/// A sky's final reply as it was sent, and where and when it holds, read from it once as it is kept.
#[derive(Debug)]
struct KeptReply {
    answer: Answer,
    hold: Option<SkyHold>,
}

impl HeapBytes for KeptReply {
    /// The payload's bytes and what the response's body holds on the heap ([`body_heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        let payload = self.answer.bulk.as_ref().map_or(0, BulkPayload::len);
        payload.saturating_add(body_heap_bytes(&self.answer.body))
    }
}

/// What a sky response's body holds on the heap: its box, its universe's text and its lists, each
/// layer's radii by ray the most of them, some 12 kB a layer at 1,536 rays. Counted from the lists'
/// own lengths, so that keeping a reply never serialises it on the runtime. A body of another kind
/// is never kept, and counts nothing.
fn body_heap_bytes(body: &ResponseBody) -> usize {
    let ResponseBody::Sky(sky) = body else {
        return 0;
    };
    let SkyResponse {
        universe,
        census,
        hosts,
        not_modelled,
        ..
    } = sky.as_ref();
    let radii: usize = census
        .iter()
        .map(|layer| layer.complete_to_rays_ly.capacity() * size_of::<f64>())
        .sum();
    size_of::<SkyResponse>()
        + universe.as_str().len()
        + census.capacity() * size_of::<SkyLayerCensusDto>()
        + radii
        + hosts.capacity() * size_of::<HostDiscDto>()
        + not_modelled.capacity() * size_of::<SkyGapDto>()
}

/// The last skies' final replies, in [`SKY_REPLY_CACHE_BYTES`], or none where the switch is off
/// ([`KeptSkies`]).
#[derive(Debug)]
pub(crate) struct SkyReplyCache {
    replies: Option<SharedByteLru<SkyReplyKey, KeptReply>>,
}

impl SkyReplyCache {
    /// An empty cache, which keeps the last skies' replies if `kept` says so, and otherwise keeps
    /// and looks up nothing.
    #[must_use]
    pub(crate) fn new(kept: KeptSkies) -> Self {
        Self::with_budget(kept, SKY_REPLY_CACHE_BYTES)
    }

    /// [`SkyReplyCache::new`] in a budget of `bytes`, for the tests of what it holds.
    #[must_use]
    fn with_budget(kept: KeptSkies, bytes: usize) -> Self {
        let replies = match kept {
            KeptSkies::Kept => Some(SharedByteLru::new(bytes)),
            KeptSkies::Off => None,
        };
        Self { replies }
    }

    /// Whether it keeps replies at all: false where the switch is off, so that a caller need not
    /// copy a reply only for it to be dropped.
    #[must_use]
    pub(crate) fn is_kept(&self) -> bool {
        self.replies.is_some()
    }

    /// The final reply kept for `key`, if one is.
    #[must_use]
    pub(crate) fn get(&self, key: &SkyReplyKey) -> Option<Answer> {
        let kept = self.replies.as_ref()?.get(key)?;
        Some(kept.answer.clone())
    }

    /// The final reply kept of `asked`'s arrival that holds for it, seen from `at`, the observer
    /// it asks checked ([`SkyHold::holds`]; R06.T11.p): of several, the latest seen, then the
    /// nearest. Its own key is the one it is kept by, and it is counted as a hit.
    #[must_use]
    pub(crate) fn held(&self, asked: &SkyReplyKey, at: &SimPosition) -> Option<HeldSky> {
        let replies = self.replies.as_ref()?;
        let holds = |kept: &KeptReply| {
            let hold = kept.hold?;
            hold.holds(asked.time, at).map(|offset_m| (hold, offset_m))
        };
        let (key, kept) = replies.get_best(|key, kept| {
            let (hold, offset_m) = holds(kept).filter(|_| key.arrival == asked.arrival)?;
            Some(hold.rank(offset_m))
        })?;
        let (_, offset_m) = holds(&kept)?;
        Some(HeldSky {
            key,
            answer: kept.answer.clone(),
            offset_m,
        })
    }

    /// Keeps `answer`, the final reply of the sky `key`, the least recently used going to make
    /// room. A reply larger than the whole budget is not kept, and nothing is where the switch is
    /// off.
    pub(crate) fn keep(&self, key: SkyReplyKey, answer: Answer) {
        let Some(replies) = &self.replies else {
            return;
        };
        let hold = SkyHold::of(&answer);
        if let Insertion::Refused(refused) = replies.insert(key, KeptReply { answer, hold }.into())
        {
            tracing::debug!(
                budget_bytes = replies.counters().budget(),
                charged_bytes = ByteLru::<SkyReplyKey, KeptReply>::charge(&refused),
                "a sky's final reply is larger than the kept skies' budget, and is not kept"
            );
        }
    }

    /// The cache's size and use: all zeros where it is off.
    #[must_use]
    pub(crate) fn counters(&self) -> LruCounters {
        self.replies
            .as_ref()
            .map_or_else(LruCounters::default, SharedByteLru::counters)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use axum::body::Bytes;
    use hyperion_protocol::{
        BandSpecDto, MassLayer, ResponseBody, SKY_STAR_BYTES, SKY_TEXEL_BYTES, SkyLayerCensusDto,
        SkyResponse,
    };
    use hyperion_sim::GeneratorVersion;
    use hyperion_sim::coords::GalacticDisplacement;

    use super::*;
    use crate::bulk::sky::{SkyStarWire, encode_sky_payload};

    /// A sky near the Sun at the epoch to a camera's V 10, with an eye.
    pub(super) fn request() -> SkyRequest {
        SkyRequest {
            universe: UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            observer: GalacticPosition {
                cell_ly: [-26_000, 0, 20],
                offset_m: [1.5, -2.5, 0.25],
            },
            time: UniverseTime {
                seconds: 1_000,
                nanos: 7,
            },
            eye: Some(EyeDto {
                field_factor: 1.4,
                age_years: 25.0,
                pigmentation: 0.5,
            }),
            camera_limit_v: Some(10.0),
            n_max: Some(50_000),
            cone: None,
            exclude_system: Some(SystemIdHex::from_u64(0x0200_0800_2000_0000)),
        }
    }

    pub(super) fn galaxy() -> GalaxyKey {
        GalaxyKey::new(42, GeneratorVersion::new(24))
    }

    /// The request with one field changed, for each field and each part of one.
    pub(super) fn each_field_changed() -> Vec<(&'static str, GalaxyKey, SkyRequest)> {
        let base = request();
        let with = |what, change: fn(&mut SkyRequest)| {
            let mut changed = base.clone();
            change(&mut changed);
            (what, galaxy(), changed)
        };
        vec![
            (
                "seed",
                GalaxyKey::new(43, GeneratorVersion::new(24)),
                base.clone(),
            ),
            (
                "generator version",
                GalaxyKey::new(42, GeneratorVersion::new(25)),
                base.clone(),
            ),
            with("universe", |r| r.universe = UniverseIdHex::from_u64(1)),
            with("cell x", |r| r.observer.cell_ly[0] += 1),
            with("cell z", |r| r.observer.cell_ly[2] -= 1),
            with("offset y, one ulp", |r| {
                r.observer.offset_m[1] = f64::from_bits(r.observer.offset_m[1].to_bits() + 1);
            }),
            with("offset z, zero", |r| r.observer.offset_m[2] = 0.0),
            with("offset z, negative zero", |r| r.observer.offset_m[2] = -0.0),
            with("seconds", |r| r.time.seconds += 1),
            with("nanos", |r| r.time.nanos += 1),
            with("field factor", |r| {
                r.eye.as_mut().unwrap().field_factor = 2.0;
            }),
            with("age", |r| r.eye.as_mut().unwrap().age_years = 60.0),
            with("pigmentation", |r| {
                r.eye.as_mut().unwrap().pigmentation = 1.0;
            }),
            with("no eye", |r| r.eye = None),
            with("camera limit", |r| r.camera_limit_v = Some(10.5)),
            with("no camera", |r| r.camera_limit_v = None),
            with("n_max", |r| r.n_max = Some(50_001)),
            with("no n_max", |r| r.n_max = None),
            with("cone", |r| {
                r.cone = Some(ConeDto {
                    axis: [1.0, 0.0, 0.0],
                    half_angle_deg: 5.0,
                });
            }),
            with("excluded system", |r| {
                r.exclude_system = Some(SystemIdHex::from_u64(0x0200_0800_2000_0001));
            }),
            with("no excluded system", |r| r.exclude_system = None),
        ]
    }

    /// The key holds every field the reply depends on: changing any one field of the request, or
    /// the galaxy, gives a key of its own, and the same request gives the same key.
    #[test]
    fn the_key_holds_every_field_the_reply_depends_on() {
        let base = SkyReplyKey::of(galaxy(), &request());
        assert_eq!(SkyReplyKey::of(galaxy(), &request()), base);
        let changed = each_field_changed();
        let mut keys = HashSet::from([base.clone()]);
        for (what, galaxy, request) in &changed {
            let key = SkyReplyKey::of(*galaxy, request);
            assert_ne!(key, base, "{what}");
            assert!(keys.insert(key), "{what} gives another change's key");
        }
        // The two cones differ in each of their parts too.
        let cone = |axis, half_angle_deg| {
            let mut request = request();
            request.eye = None;
            request.cone = Some(ConeDto {
                axis,
                half_angle_deg,
            });
            SkyReplyKey::of(galaxy(), &request)
        };
        let one = cone([1.0, 0.0, 0.0], 5.0);
        assert_ne!(one, cone([0.0, 1.0, 0.0], 5.0), "the axis");
        assert_ne!(one, cone([1.0, 0.0, 0.0], 5.5), "the half-angle");
    }

    /// The key's bytes, which a kept sky's file holds, differ wherever the keys do: for each
    /// changed field, and for an option moved between two fields of one length.
    #[test]
    fn the_keys_bytes_differ_for_each_field() {
        let bytes = |galaxy, request: &SkyRequest| {
            let mut out = Vec::new();
            SkyReplyKey::of(galaxy, request).write_bytes(&mut out);
            out
        };
        let base = bytes(galaxy(), &request());
        assert_eq!(bytes(galaxy(), &request()), base);
        let mut seen = HashSet::from([base.clone()]);
        for (what, galaxy, request) in &each_field_changed() {
            assert!(seen.insert(bytes(*galaxy, request)), "{what}");
        }
        let moved = SkyRequest {
            camera_limit_v: None,
            n_max: None,
            ..request()
        };
        let only_camera = SkyRequest {
            n_max: None,
            ..request()
        };
        assert_ne!(bytes(galaxy(), &moved), bytes(galaxy(), &only_camera));
    }

    /// The rays each layer's radii are stated along, as the served caps' lattice has them.
    const RAYS: usize = 1_536;

    /// A final reply of `listed` stars and the full band: its payload of `listed` star records and
    /// six faces of 64² texels, and each layer's radii along every ray, at 17 digits.
    pub(super) fn answer(listed: u32) -> Answer {
        let stars = usize::try_from(listed).unwrap() * SKY_STAR_BYTES;
        let band = 6 * 64 * 64 * SKY_TEXEL_BYTES;
        let payload = BulkPayload::new(Bytes::from(vec![7_u8; stars + band])).unwrap();
        let layer = |layer| SkyLayerCensusDto {
            layer,
            cap_ly: 2_000.0,
            rule_bound_ly: 2_000.0,
            expected_beyond: 0.5,
            bright_beyond: None,
            cells: 1_000,
            candidates_opened: 10_000,
            accepted: 5_000,
            listed: 4_000,
            without_photometry: 0,
            feature_members_absent: false,
            complete_to_ly: 1_999.123_456_789_012_3,
            complete_to_rays_ly: vec![1_999.123_456_789_012_3; RAYS],
            is_final: true,
        };
        let request = request();
        let body = SkyResponse {
            universe: request.universe,
            time: request.time,
            observer: request.observer,
            valid_until: request.time,
            cut_v: 10.0,
            synthetic_ceiling_v: None,
            real_limit_ly: None,
            census: [
                MassLayer::A,
                MassLayer::B,
                MassLayer::C,
                MassLayer::D,
                MassLayer::E,
            ]
            .map(layer)
            .into(),
            listed,
            overflow: 0,
            band: BandSpecDto { face_texels: 64 },
            hosts: Vec::new(),
            not_modelled: Vec::new(),
            bulk: payload.manifest(),
            stars_bytes: u64::try_from(stars).unwrap(),
            band_bytes: u64::try_from(band).unwrap(),
            is_final: true,
        };
        Answer {
            body: ResponseBody::Sky(Box::new(body)),
            bulk: Some(payload),
        }
    }

    fn key(n_max: u32) -> SkyReplyKey {
        SkyReplyKey::of(
            galaxy(),
            &SkyRequest {
                n_max: Some(n_max),
                ..request()
            },
        )
    }

    /// A reply kept is given back as it was; the switch off keeps and finds nothing.
    #[test]
    fn a_kept_reply_comes_back_as_it_was_and_none_is_kept_with_the_switch_off() {
        let replies = SkyReplyCache::new(KeptSkies::Kept);
        let sent = answer(1_000);
        assert_eq!(replies.get(&key(1)), None);
        replies.keep(key(1), sent.clone());
        assert_eq!(replies.get(&key(1)), Some(sent.clone()));
        assert_eq!(replies.get(&key(2)), None);
        let counters = replies.counters();
        assert_eq!(
            (counters.entries(), counters.hits(), counters.misses()),
            (1, 1, 2)
        );

        let off = SkyReplyCache::new(KeptSkies::Off);
        off.keep(key(1), sent.clone());
        assert_eq!(off.get(&key(1)), None);
        assert_eq!(off.counters(), LruCounters::default());
    }

    /// The budget holds the last two skies at `N_max`'s 3 × 10⁵ stars, and a third pushes out the
    /// least recently used.
    #[test]
    fn the_budget_keeps_the_last_two_skies_at_n_max() {
        let replies = SkyReplyCache::new(KeptSkies::Kept);
        let full = answer(300_000);
        let charge = ByteLru::<SkyReplyKey, KeptReply>::charge(&KeptReply {
            hold: SkyHold::of(&full),
            answer: full.clone(),
        });
        assert!(charge > 7_494_912 + 5 * RAYS * 8, "{charge}");
        assert!(2 * charge <= SKY_REPLY_CACHE_BYTES, "{charge}");
        for n_max in 1..=3 {
            replies.keep(key(n_max), full.clone());
        }
        assert_eq!(replies.counters().entries(), 2, "{:?}", replies.counters());
        assert_eq!(replies.get(&key(1)), None, "the oldest went");
        assert!(replies.get(&key(2)).is_some() && replies.get(&key(3)).is_some());
    }

    /// A reply larger than the whole budget is not kept, and the cache is unchanged.
    #[test]
    fn a_reply_over_the_budget_is_not_kept() {
        let replies = SkyReplyCache::with_budget(KeptSkies::Kept, 1 << 20);
        replies.keep(key(1), answer(10));
        replies.keep(key(2), answer(300_000));
        assert_eq!(replies.get(&key(2)), None);
        assert!(replies.get(&key(1)).is_some());
        assert_eq!(replies.counters().refused(), 1);
    }

    /// The arrival is the key but its observer and its time: changing either keeps it, changing
    /// any other field gives another, of other bytes, and the arrival seen where and when a
    /// request asks is that request's key.
    #[test]
    fn the_arrival_is_the_key_but_its_observer_and_its_time() {
        let base = SkyReplyKey::of(galaxy(), &request());
        assert_eq!(base.arrival().at(&request().observer, request().time), base);
        let bytes = |arrival: &SkyArrival| {
            let mut out = Vec::new();
            arrival.write_bytes(&mut out);
            out
        };
        let mut seen = HashSet::from([bytes(base.arrival())]);
        for (what, galaxy, request) in each_field_changed() {
            let key = SkyReplyKey::of(galaxy, &request);
            let place = ["cell", "offset", "seconds", "nanos"]
                .iter()
                .any(|field| what.starts_with(field));
            assert_eq!(key.arrival() == base.arrival(), place, "{what}");
            assert_eq!(
                base.arrival().at(&request.observer, request.time) == key,
                place,
                "{what}"
            );
            assert_eq!(!seen.insert(bytes(key.arrival())), place, "{what}");
        }
    }

    /// One Julian year, s: the longest a sky holds (Design note 13).
    pub(super) const YEAR_S: i64 = 31_557_600;

    /// An observer near the Sun, as a client asks from.
    pub(super) fn sun() -> GalacticPosition {
        GalacticPosition {
            cell_ly: [-26_000, 0, 20],
            offset_m: [1.5e15, 2.5e15, 0.25],
        }
    }

    /// [`request`] asked from `observer`, `seconds` after its time.
    pub(super) fn asked(observer: GalacticPosition, seconds: i64) -> SkyRequest {
        let time = request().time;
        SkyRequest {
            observer,
            time: UniverseTime {
                seconds: time.seconds + seconds,
                ..time
            },
            ..request()
        }
    }

    /// `request`'s final reply, seen as it asks and valid for `valid_for_s` after, of stars at
    /// `distances_ly` and no band.
    pub(super) fn reply_of(request: &SkyRequest, valid_for_s: i64, distances_ly: &[f32]) -> Answer {
        let stars: Vec<SkyStarWire> = distances_ly
            .iter()
            .map(|&distance_ly| SkyStarWire {
                direction: [1.0, 0.0, 0.0],
                distance_ly,
                v_mag: 5.0,
                chroma: [0.3, 0.3],
                eye_offset_mag: 0.0,
                camera_band_mag: 0.0,
            })
            .collect();
        let encoded = encode_sky_payload(&stars, &[]);
        let payload = BulkPayload::new(encoded.bytes).unwrap();
        let mut reply = answer(0);
        let ResponseBody::Sky(sky) = &mut reply.body else {
            unreachable!("answer is a sky's")
        };
        sky.universe = request.universe.clone();
        sky.time = request.time;
        sky.observer = request.observer;
        sky.valid_until = UniverseTime {
            seconds: request.time.seconds + valid_for_s,
            ..request.time
        };
        sky.listed = u32::try_from(stars.len()).unwrap();
        sky.bulk = payload.manifest();
        sky.stars_bytes = encoded.stars_bytes;
        sky.band_bytes = encoded.band_bytes;
        reply.bulk = Some(payload);
        reply
    }

    /// `observer`, checked.
    pub(super) fn position(observer: GalacticPosition) -> SimPosition {
        SimPosition::new(LyCell::new(observer.cell_ly), observer.offset_m).unwrap()
    }

    /// `observer` moved `metres` along x.
    pub(super) fn moved(observer: GalacticPosition, metres: f64) -> GalacticPosition {
        let at = position(observer)
            .translated(GalacticDisplacement::new([metres, 0.0, 0.0]))
            .unwrap();
        GalacticPosition {
            cell_ly: at.cell().to_array(),
            offset_m: at.offset_metres(),
        }
    }

    /// The tenth of a pixel at 1080p across 60° times 4.37 ly, Alpha Centauri's distance, m: some
    /// 26.8 au.
    pub(super) fn bound_m() -> f64 {
        0.1 * (60.0_f64.to_radians() / 1_080.0) * f64::from(4.37_f32) * METRES_PER_LIGHT_YEAR
    }

    /// A kept sky holds from its own time until its `valid_until`, from its own observer or
    /// within a tenth of a pixel at 1080p across 60° of its nearest listed star's parallax, and
    /// with none listed from its own observer alone (Design note 13). Only a final reply holds.
    #[test]
    fn a_kept_sky_holds_until_its_valid_until_within_a_tenth_of_a_pixel_of_its_nearest_star() {
        let kept = asked(sun(), 0);
        let hold = SkyHold::of(&reply_of(&kept, YEAR_S, &[8.6, 4.37, 11.0])).unwrap();
        let at = |observer, seconds| hold.holds(asked(observer, seconds).time, &position(observer));
        let au = 1.495_978_707e11;
        assert!((bound_m() / au - 26.8).abs() < 0.05, "{}", bound_m() / au);
        assert_eq!(at(sun(), 0), Some(0.0));
        assert_eq!(at(sun(), 3_600), Some(0.0), "an hour later");
        assert_eq!(at(sun(), YEAR_S - 1), Some(0.0), "a second before it ends");
        assert_eq!(at(sun(), YEAR_S), None, "at its valid_until");
        assert_eq!(at(sun(), -1), None, "before it was seen");
        let near = at(moved(sun(), 0.99 * bound_m()), 3_600).unwrap();
        assert!((near / bound_m() - 0.99).abs() < 1e-9, "{near}");
        assert_eq!(
            at(moved(sun(), 1.01 * bound_m()), 3_600),
            None,
            "past the bound"
        );
        let key = SkyReplyKey::of(galaxy(), &kept);
        assert_eq!(hold.key(key.arrival()), key, "the key it is kept by");

        let bare = SkyHold::of(&reply_of(&kept, YEAR_S, &[])).unwrap();
        let later = asked(sun(), 3_600).time;
        assert_eq!(bare.holds(later, &position(sun())), Some(0.0));
        assert_eq!(bare.holds(later, &position(moved(sun(), 1.0))), None);

        let mut partial = reply_of(&kept, YEAR_S, &[4.37]);
        if let ResponseBody::Sky(sky) = &mut partial.body {
            sky.is_final = false;
        }
        assert_eq!(SkyHold::of(&partial), None);
    }

    /// Of the kept skies of an arrival that hold for a sky asked, the latest seen is found, then
    /// the nearest, under its own key, as a hit; none past its time or bound is, nor any of another
    /// arrival, nor any where the switch is off.
    #[test]
    fn the_latest_kept_sky_of_the_arrival_that_holds_is_found() {
        let replies = SkyReplyCache::new(KeptSkies::Kept);
        let (early, late) = (asked(sun(), 0), asked(sun(), 600));
        for kept in [&early, &late] {
            replies.keep(
                SkyReplyKey::of(galaxy(), kept),
                reply_of(kept, YEAR_S, &[4.37]),
            );
        }
        let held = |replies: &SkyReplyCache, request: &SkyRequest| {
            replies.held(
                &SkyReplyKey::of(galaxy(), request),
                &position(request.observer),
            )
        };
        let later = held(&replies, &asked(sun(), 3_600)).unwrap();
        assert_eq!(later.key, SkyReplyKey::of(galaxy(), &late));
        assert_eq!(later.answer, reply_of(&late, YEAR_S, &[4.37]));
        assert_eq!(
            held(&replies, &asked(sun(), 300)).unwrap().key,
            SkyReplyKey::of(galaxy(), &early)
        );
        assert!(held(&replies, &asked(sun(), -1)).is_none(), "before both");
        let far = asked(moved(sun(), 1.01 * bound_m()), 3_600);
        assert!(held(&replies, &far).is_none(), "past the bound");
        let other = SkyRequest {
            n_max: Some(7),
            ..asked(sun(), 3_600)
        };
        assert!(held(&replies, &other).is_none(), "another arrival");
        let counters = replies.counters();
        assert_eq!((counters.hits(), counters.misses()), (2, 0), "{counters:?}");

        let both = SkyReplyCache::new(KeptSkies::Kept);
        let (here, there) = (asked(sun(), 0), asked(moved(sun(), 1.0e11), 0));
        for kept in [&here, &there] {
            both.keep(
                SkyReplyKey::of(galaxy(), kept),
                reply_of(kept, YEAR_S, &[4.37]),
            );
        }
        let between = held(&both, &asked(moved(sun(), 0.8e11), 60)).unwrap();
        assert_eq!(between.key, SkyReplyKey::of(galaxy(), &there), "the nearer");
        assert!(
            (between.offset_m - 0.2e11).abs() < 1.0,
            "{}",
            between.offset_m
        );

        let off = SkyReplyCache::new(KeptSkies::Off);
        off.keep(
            SkyReplyKey::of(galaxy(), &early),
            reply_of(&early, YEAR_S, &[4.37]),
        );
        assert!(held(&off, &asked(sun(), 3_600)).is_none());
    }
}
