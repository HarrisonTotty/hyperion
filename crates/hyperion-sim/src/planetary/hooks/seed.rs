//! The surface seed (plan 14, P14.T23): the one number from which a map generator draws a body's
//! surface.
//!
//! It is word 0 of the body's [`tags::BODY_SURFACE`] stream, a function of the universe seed and
//! the body's ID alone. Nothing else of the body goes into it, so no later change to placement or
//! derivation can alter a map's seed ("Generator version": "the map generator's output survives
//! any change to derivation"), and no other property reads the stream.
//!
//! The seed is the server's: the rendering plans' R04 amends plan 14 so that the client is given a
//! detail seed on its own tag in its place, and no wire type carries this one (the
//! [`hooks`](super) module documentation).
//!
//! The type itself is the foundation's, [`hyperion_base::rng::SurfaceSeed`], since the streams it
//! opens must be built where `Stream` is (the rendering plans' R09.T1.a); it is re-exported here
//! at its old path, and its value is unchanged.

use crate::Seed;
use crate::id::BodyId;
use crate::rng::{ObjectKey, Stream, tags};

pub use crate::rng::SurfaceSeed;

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

    #[test]
    fn a_seed_prints_as_sixteen_hex_digits() {
        assert_eq!(SurfaceSeed::new(0xab).to_string(), "00000000000000ab");
    }
}
