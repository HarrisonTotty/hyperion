//! The last skies' final replies, kept (rendering plan R06, R06.T11.l; the owner's priority,
//! 2026-11-01): a sky asked again exactly as one already finished is answered from memory, with no
//! job on the pool.
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

use hyperion_protocol::{
    ConeDto, EyeDto, GalacticPosition, SkyRequest, SystemIdHex, UniverseIdHex, UniverseTime,
};

use crate::bulk::{Answer, BulkPayload};
use crate::cache::{HeapBytes, Insertion, LruCounters, SharedByteLru};
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
    galaxy: GalaxyKey,
    universe: UniverseIdHex,
    /// The observer's cell, ly, and its offset in it, m, by its bits.
    observer: ([i32; 3], [u64; 3]),
    /// The time's seconds and nanoseconds.
    time: (i64, u32),
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
            observer: GalacticPosition { cell_ly, offset_m },
            time: UniverseTime { seconds, nanos },
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
        Self {
            galaxy,
            universe: universe.clone(),
            observer: (*cell_ly, offset_m.map(f64::to_bits)),
            time: (*seconds, *nanos),
            eye,
            camera_limit_v: camera_limit_v.map(f64::to_bits),
            n_max: *n_max,
            cone,
            exclude_system: exclude_system.clone(),
        }
    }
}

/// A sky's final reply as it was sent, with its JSON's length.
#[derive(Debug)]
struct KeptReply {
    answer: Answer,
    json_bytes: usize,
}

impl HeapBytes for KeptReply {
    /// The payload's bytes and the JSON's length, which stands for what the response's body holds
    /// on the heap, its layers' radii by ray the most of it.
    fn heap_bytes(&self) -> usize {
        let payload = self.answer.bulk.as_ref().map_or(0, BulkPayload::len);
        payload.saturating_add(self.json_bytes)
    }
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

    /// The final reply kept for `key`, if one is.
    #[must_use]
    pub(crate) fn get(&self, key: &SkyReplyKey) -> Option<Answer> {
        let kept = self.replies.as_ref()?.get(key)?;
        Some(kept.answer.clone())
    }

    /// Keeps `answer`, the final reply of the sky `key`, the least recently used going to make
    /// room. A reply larger than the whole budget is not kept.
    pub(crate) fn keep(&self, key: SkyReplyKey, answer: &Answer) {
        let Some(replies) = &self.replies else {
            return;
        };
        let json_bytes = serde_json::to_vec(&answer.body)
            .expect("a response body always serialises: it holds no map with non-string keys")
            .len();
        let kept = KeptReply {
            answer: answer.clone(),
            json_bytes,
        };
        if let Insertion::Refused(_) = replies.insert(key, kept.into()) {
            tracing::debug!(
                budget_bytes = replies.counters().budget(),
                json_bytes,
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

    use super::*;
    use crate::cache::ByteLru;

    /// A sky near the Sun at the epoch to a camera's V 10, with an eye.
    fn request() -> SkyRequest {
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

    fn galaxy() -> GalaxyKey {
        GalaxyKey::new(42, GeneratorVersion::new(24))
    }

    /// The request with one field changed, for each field and each part of one.
    fn each_field_changed() -> Vec<(&'static str, GalaxyKey, SkyRequest)> {
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

    /// The rays each layer's radii are stated along, as the served caps' lattice has them.
    const RAYS: usize = 1_536;

    /// A final reply of `listed` stars and the full band: its payload of `listed` star records and
    /// six faces of 64² texels, and each layer's radii along every ray, at 17 digits.
    fn answer(listed: u32) -> Answer {
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
        replies.keep(key(1), &sent);
        assert_eq!(replies.get(&key(1)), Some(sent.clone()));
        assert_eq!(replies.get(&key(2)), None);
        let counters = replies.counters();
        assert_eq!(
            (counters.entries(), counters.hits(), counters.misses()),
            (1, 1, 2)
        );

        let off = SkyReplyCache::new(KeptSkies::Off);
        off.keep(key(1), &sent);
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
            answer: full.clone(),
            json_bytes: serde_json::to_vec(&full.body).unwrap().len(),
        });
        assert!(charge > 7_494_912 + 5 * RAYS * 17, "{charge}");
        assert!(2 * charge <= SKY_REPLY_CACHE_BYTES, "{charge}");
        for n_max in 1..=3 {
            replies.keep(key(n_max), &full);
        }
        assert_eq!(replies.counters().entries(), 2, "{:?}", replies.counters());
        assert_eq!(replies.get(&key(1)), None, "the oldest went");
        assert!(replies.get(&key(2)).is_some() && replies.get(&key(3)).is_some());
    }

    /// A reply larger than the whole budget is not kept, and the cache is unchanged.
    #[test]
    fn a_reply_over_the_budget_is_not_kept() {
        let replies = SkyReplyCache::with_budget(KeptSkies::Kept, 1 << 20);
        replies.keep(key(1), &answer(10));
        replies.keep(key(2), &answer(300_000));
        assert_eq!(replies.get(&key(2)), None);
        assert!(replies.get(&key(1)).is_some());
        assert_eq!(replies.counters().refused(), 1);
    }
}
