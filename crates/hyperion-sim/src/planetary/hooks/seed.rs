//! The surface seed (plan 14, P14.T23), the one number from which a map generator draws a body's
//! surface, and the detail seed (the rendering plans' R09.T1.b), from which the local synthesis
//! elaborates what has been surveyed of it.
//!
//! The surface seed is word 0 of the body's [`tags::BODY_SURFACE`] stream, a function of the
//! universe seed and the body's ID alone. Nothing else of the body goes into it, so no later change
//! to placement or derivation can alter a map's seed ("Generator version": "the map generator's
//! output survives any change to derivation"), and no other property reads the stream.
//!
//! The surface seed is the server's: the rendering plans' R04 amends plan 14 so that the client is
//! given the detail seed in its place, and no wire type carries the surface seed (the
//! [`hooks`](super) module documentation). The detail seed is word 0 of the body's own
//! [`tags::BODY_SURFACE_DETAIL`] stream, likewise a function of the universe seed and the body's ID
//! alone, so the two share nothing but the universe seed and neither can be had from the other
//! (the rendering brainstorm's "Knowledge, and the surface seed"; R09, Design note 2).
//!
//! Both types are the foundation's, [`hyperion_base::rng::SurfaceSeed`] and
//! [`hyperion_base::rng::DetailSeed`], since the streams they open must be built where `Stream` is
//! (the rendering plans' R09.T1.a); they are re-exported here, and the surface seed's value is
//! unchanged.

use crate::Seed;
use crate::id::BodyId;
use crate::rng::{ObjectKey, Stream, tags};

pub use crate::rng::{DetailSeed, SurfaceSeed};

/// The surface seed of `body` in the universe of `seed`: word 0 of its [`tags::BODY_SURFACE`]
/// stream (P14.T23).
///
/// # Examples
///
/// The seed depends on the body's ID and nothing else, so it can be had without generating the
/// system:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::hooks::surface_seed;
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let earth = BodyId::new(system, 0x0300);
/// assert_eq!(surface_seed(Seed::new(7), earth), surface_seed(Seed::new(7), earth));
/// assert_ne!(surface_seed(Seed::new(7), earth), surface_seed(Seed::new(8), earth));
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[must_use]
pub fn surface_seed(seed: Seed, body: BodyId) -> SurfaceSeed {
    SurfaceSeed::new(Stream::open(seed, tags::BODY_SURFACE, ObjectKey::from(body)).word_at(0))
}

/// The detail seed of `body` in the universe of `seed`: word 0 of its
/// [`tags::BODY_SURFACE_DETAIL`] stream (R09.T1.b).
///
/// A client is given it in the [`surface_seed`]'s place, in the hooks section of the body's record
/// ([`record::Hooks`](crate::planetary::record::Hooks)). It keys the local synthesis of the
/// surveyed coarse field ([`DetailSeed::stream`]) and cannot regenerate the field itself.
///
/// # Examples
///
/// Like the surface seed, it depends on the body's ID and nothing else, and it is not the surface
/// seed:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::hooks::{detail_seed, surface_seed};
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let earth = BodyId::new(system, 0x0300);
/// let seed = Seed::new(7);
/// assert_eq!(detail_seed(seed, earth), detail_seed(seed, earth));
/// assert_ne!(detail_seed(seed, earth), detail_seed(Seed::new(8), earth));
/// assert_ne!(detail_seed(seed, earth).get(), surface_seed(seed, earth).get());
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[must_use]
pub fn detail_seed(seed: Seed, body: BodyId) -> DetailSeed {
    DetailSeed::new(Stream::open(seed, tags::BODY_SURFACE_DETAIL, ObjectKey::from(body)).word_at(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::SystemId;
    use crate::planetary::record::BodyKind;
    use crate::planetary::system::generate;
    use crate::planetary::testing::synthetic_star;
    use crate::units::{Dex, SolarMasses, Years};

    /// The seed of a body is unchanged when everything else about its system changes: the same
    /// system ID about hosts of different masses and metallicities draws other classes and other
    /// planets, and every body both systems hold keeps its seed.
    #[test]
    fn a_seed_ignores_everything_but_the_body() {
        let seed = Seed::new(0x0014_0023);
        let id = SystemId::from_raw(0x0200_0800_2000_0031).unwrap();
        let hosts = [
            (1.0, 0.0, 4.6e9),
            (0.6, -0.4, 8e9),
            (1.4, 0.3, 1e9),
            (0.3, 0.1, 3e9),
        ];
        let systems: Vec<_> = hosts
            .iter()
            .map(|&(m, fe_h, age)| {
                let ctx = synthetic_star(id, SolarMasses::new(m), Dex::new(fe_h), Years::new(age))
                    .unwrap();
                generate(seed, &ctx)
            })
            .collect();
        let mut shared = 0;
        for (i, a) in systems.iter().enumerate() {
            for body in a.bodies().iter().filter(|b| b.kind() != BodyKind::Ring) {
                let index = body.index();
                let expected = surface_seed(seed, index.body_id(id));
                assert_eq!(a.surface_seed(index), Ok(Some(expected)));
                for b in &systems[i + 1..] {
                    if let Some(other) = b.body(index) {
                        assert_eq!(b.surface_seed(other.index()), Ok(Some(expected)));
                        shared += 1;
                    }
                }
            }
        }
        assert!(shared > 0, "the systems share no body index");
    }

    /// The seeds of 10⁶ bodies, over many systems and every sub-index, have no duplicates.
    #[test]
    fn a_million_seeds_are_distinct() {
        let seed = Seed::new(0x0014_0023);
        let mut seeds: Vec<u64> = (0..1_000_u32)
            .flat_map(|n| {
                let system = SystemId::from_raw(0x0200_0800_2000_0000 + u64::from(n)).unwrap();
                (0..1_000_u16).map(move |i| surface_seed(seed, BodyId::new(system, i * 65 + 1)))
            })
            .map(SurfaceSeed::get)
            .collect();
        assert_eq!(seeds.len(), 1_000_000);
        seeds.sort_unstable();
        seeds.dedup();
        assert_eq!(seeds.len(), 1_000_000);
    }

    /// The detail seeds of 10⁶ bodies, over many systems and every sub-index, have no duplicates,
    /// and none is its body's surface seed (R09.T1.b).
    #[test]
    fn a_million_detail_seeds_are_distinct_and_none_is_its_surface_seed() {
        let seed = Seed::new(0x0014_0023);
        let mut seeds: Vec<u64> = (0..1_000_u32)
            .flat_map(|n| {
                let system = SystemId::from_raw(0x0200_0800_2000_0000 + u64::from(n)).unwrap();
                (0..1_000_u16).map(move |i| BodyId::new(system, i * 65 + 1))
            })
            .map(|body| {
                let detail = detail_seed(seed, body).get();
                assert_ne!(
                    detail,
                    surface_seed(seed, body).get(),
                    "{body}: the detail seed is the surface seed"
                );
                detail
            })
            .collect();
        assert_eq!(seeds.len(), 1_000_000);
        seeds.sort_unstable();
        seeds.dedup();
        assert_eq!(seeds.len(), 1_000_000);
    }

    #[test]
    fn a_seed_prints_as_sixteen_hex_digits() {
        assert_eq!(SurfaceSeed::new(0xab).to_string(), "00000000000000ab");
    }
}
