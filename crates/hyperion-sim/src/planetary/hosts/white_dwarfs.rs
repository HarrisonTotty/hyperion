//! White dwarfs' pollution and dusty debris discs (plan 14, design note 11, P14.T28.d), behind
//! their hold at generator version 26 (`WHITE_DWARF_DEBRIS_VERSION`).
//!
//! A white dwarf that keeps a belt and at least one planet through its giant branches goes on
//! scattering asteroids inward, and those that cross its Roche limit are torn apart: their dust
//! forms a disc inside the limit and their metals rain onto its photosphere (Jura 2003; Farihi
//! 2016). So the white dwarf host of such a system is **polluted** with probability
//! [`POLLUTED_SHARE`], and a polluted one carries a **dusty debris disc** inside its Roche limit
//! from the start of its white dwarf phase for a drawn span of cooling age, so that the share of
//! such hosts with a disc falls from [`DEBRIS_DISC_SHARE_YOUNG`] at birth to
//! [`DEBRIS_DISC_SHARE_AT_GYR`] at 1 Gyr ([`debris_disc_share`]).
//!
//! The mark is the system's, not the star's (lean, 2026-10-31). Plan 06 already gives a white
//! dwarf metal lines from its own draw (`stellar::remnant::wd_spectral`: 8.0% of DAs and 15.8% of
//! the rest, the Gaia 40 pc census's optical rates, O'Brien et al. 2024). They measure different
//! things: those are lines strong enough to type a spectrum in the optical, of every white dwarf;
//! this is accretion of planetary debris, of a white dwarf with a planetary system left, which
//! Koester et al. (2014) measure in the ultraviolet. Both stand, and neither reads the other.
//!
//! Its geometry: the disc lies between the radius at which a flat, opaque dust ring heated by the
//! white dwarf reaches [`DUST_SUBLIMATION_TEMPERATURE`] ([`debris_disc_inner_edge`]) and the white
//! dwarf's rigid Roche limit for a rocky asteroid, about 1 R☉ ([`debris_disc_outer_edge`]).
//!
//! # Streams
//!
//! [`tags::BELT_POPULATION`], keyed by the system's ID, belt slot `0xEE`'s words (design note 3's
//! slot for the debris disc): word 112 the pollution mark, word 113 the disc's mark and word 114
//! the rank of its span of cooling age; words 115–119 are reserved ([`WhiteDwarfDraws`]).
//!
//! # Sources
//!
//! The figures are cited from memory: the pod that built this task could not reach the papers.
//! Re-check Koester, Gänsicke and Farihi (2014), Farihi (2016), Farihi, Jura and Zuckerman (2009)
//! and Jura (2003) at source.

use core::f64::consts::PI;

use crate::id::SystemId;
use crate::math;
use crate::planetary::belts::{BELT_WORDS_PER_SLOT, BeltComposition};
use crate::planetary::derive::limits::roche_limit_rigid;
use crate::rng::{Mark, ObjectKey, Seed, Stream, Threshold, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::{
    Kelvin, Kilograms, KilogramsPerCubicMetre, Metres, SolarMasses, SolarRadii, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which a white dwarf's system is marked polluted and may carry a
/// debris disc in belt slot `0xEE` (P14.T28.d): 26, the first galaxy train after the task lands
/// (the bump plan).
///
/// It is T28.d's one hold. Below it [`WhiteDwarfDebrisRules::in_force`] is
/// [`WhiteDwarfDebrisRules::Held`]: no system is marked and none has the body, and every generated
/// value is version 25's bit for bit. 26's bump deletes this hold, [`WhiteDwarfDebrisRules`] with
/// its `Held` arm, the tests' switch and its tripwire
/// (`the_white_dwarf_debris_is_held_only_until_the_26_bump`).
pub(crate) const WHITE_DWARF_DEBRIS_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Whether P14.T28.d's pollution and debris discs are in force (its hold,
/// [`WHITE_DWARF_DEBRIS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum WhiteDwarfDebrisRules {
    /// Version 25's and before: no white dwarf's system is marked.
    Held,
    /// Version 26's: the pollution mark and the debris disc.
    Debris,
}

impl WhiteDwarfDebrisRules {
    /// The rules this generator version gives: the debris's from [`WHITE_DWARF_DEBRIS_VERSION`],
    /// none below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_white_dwarf_debris`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = DEBRIS_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= WHITE_DWARF_DEBRIS_VERSION {
            Self::Debris
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The debris's rules a test sets on its thread, `None` for the version's.
    static DEBRIS_RULES: core::cell::Cell<Option<WhiteDwarfDebrisRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the debris's rules `rules` on this thread, for the tests to reach them
/// ahead of the bump; the version's rules are set back after, a panic in `run` included. 26's
/// bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_white_dwarf_debris<R>(
    rules: WhiteDwarfDebrisRules,
    run: impl FnOnce() -> R,
) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<WhiteDwarfDebrisRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DEBRIS_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(DEBRIS_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The belt slot of the debris disc: `Belt(14)`, slot `0xEE` (design note 3), which no belt
/// takes ([`LAST_BELT_SLOT`](crate::planetary::belts::LAST_BELT_SLOT) is 13).
pub const DEBRIS_DISC_SLOT: u8 = 14;

/// The share of white dwarfs with a surviving belt and at least one planet that are polluted
/// (design note 11): 0.375, the middle of the 25–50% the note fits to. Koester, Gänsicke and
/// Farihi (2014, A&A 566, A34) find, in the ultraviolet, at least 27% of DA white dwarfs of
/// 17,000–27,000 K accreting planetary debris, and up to about half.
pub const POLLUTED_SHARE: f64 = 0.375;

/// The share of white dwarfs with a surviving belt and at least one planet that carry a dusty
/// debris disc at the start of their white dwarf phase: 4%, the top of the 1–4% of white dwarfs
/// with an infrared excess from dust (Farihi 2016, New Astron. Rev. 71, 9), which the young ones
/// show most (Farihi, Jura and Zuckerman 2009, ApJ 694, 805). Read as the share at birth (lean,
/// 2026-10-31).
pub const DEBRIS_DISC_SHARE_YOUNG: f64 = 0.04;

/// The share of such white dwarfs that still carry a debris disc at a cooling age of 1 Gyr: 1%,
/// the bottom of Farihi's (2016) 1–4%, reached as the frequency falls with cooling age (Farihi,
/// Jura and Zuckerman 2009 detect none among the coolest). Read as the share at 1 Gyr (lean,
/// 2026-10-31), with an exponential fall between ([`debris_disc_share`]).
pub const DEBRIS_DISC_SHARE_AT_GYR: f64 = 0.01;

/// The temperature at which a white dwarf's dust ring's inner edge lies: 1,200 K, where Jura (2003,
/// ApJ 584, L91) puts G29-38's, the dust's sublimation.
pub const DUST_SUBLIMATION_TEMPERATURE: Kelvin = Kelvin::new(1_200.0);

/// Words of [`tags::BELT_POPULATION`] the debris disc's slot reads or reserves: its three draws,
/// then five reserved, the slot's [`BELT_WORDS_PER_SLOT`].
pub const WHITE_DWARF_WORDS: u64 = BELT_WORDS_PER_SLOT;

/// The draws of a white dwarf's system (P14.T28.d), from belt slot `0xEE`'s words of
/// [`tags::BELT_POPULATION`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhiteDwarfDraws {
    /// Whether it is polluted, against [`POLLUTED_SHARE`]. Word 112.
    pub pollution: Mark,
    /// Whether a polluted one has a debris disc at its birth, against the share at birth of the
    /// polluted. Word 113.
    pub disc: Mark,
    /// The rank of the disc's span of cooling age in its exponential law. Word 114.
    pub disc_span: UnitUniform,
}

impl WhiteDwarfDraws {
    /// The draws of the system `system` in the universe of `seed`.
    ///
    /// # Panics
    ///
    /// Never: an open uniform lies strictly inside (0, 1).
    #[must_use]
    pub fn for_system(seed: Seed, system: SystemId) -> Self {
        let mut stream = Stream::open(seed, tags::BELT_POPULATION, ObjectKey::from(system));
        stream.seek(u64::from(DEBRIS_DISC_SLOT) * WHITE_DWARF_WORDS);
        let pollution = Mark::from_word(stream.next_u64());
        let disc = Mark::from_word(stream.next_u64());
        let disc_span = UnitUniform::new(stream.uniform_open())
            .expect("an open uniform lies strictly inside (0, 1)");
        Self {
            pollution,
            disc,
            disc_span,
        }
    }
}

/// What a white dwarf's system keeps through its giant branches, as the pollution mark reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Survivors {
    /// A belt and at least one planet: the system can be polluted.
    BeltAndPlanet,
    /// Less: no belt, or no planet, and so nothing to pollute it.
    Fewer,
}

/// A white dwarf's system's marks (P14.T28.d): whether it is polluted, and for how long its
/// debris disc lasts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhiteDwarfMarks {
    polluted: bool,
    disc_until: Option<Years>,
}

impl WhiteDwarfMarks {
    /// The marks of a white dwarf's system that keeps `survivors`, from its draws `draws`.
    ///
    /// A system with a belt and a planet is polluted where its pollution mark lies below
    /// [`POLLUTED_SHARE`]. A polluted one has a debris disc where its disc mark lies below
    /// [`DEBRIS_DISC_SHARE_YOUNG`] ÷ [`POLLUTED_SHARE`], until a cooling age drawn exponential with
    /// the e-fold [`debris_disc_e_fold`], so that the share with a disc is [`debris_disc_share`].
    #[must_use]
    pub fn decide(draws: &WhiteDwarfDraws, survivors: Survivors) -> Self {
        let polluted = match survivors {
            Survivors::BeltAndPlanet => draws
                .pollution
                .is_below(Threshold::from_probability(POLLUTED_SHARE)),
            Survivors::Fewer => false,
        };
        let disc = polluted
            && draws.disc.is_below(Threshold::from_probability(
                DEBRIS_DISC_SHARE_YOUNG / POLLUTED_SHARE,
            ));
        let disc_until = disc
            .then(|| Years::new(-debris_disc_e_fold().value() * math::ln(draws.disc_span.value())));
        Self {
            polluted,
            disc_until,
        }
    }

    /// Whether the system is polluted, from its white dwarf's birth on.
    #[must_use]
    pub const fn is_polluted(&self) -> bool {
        self.polluted
    }

    /// The cooling age until which its debris disc lasts, if it has one: present from the white
    /// dwarf's birth until then, and dispersed after.
    #[must_use]
    pub const fn disc_until(&self) -> Option<Years> {
        self.disc_until
    }
}

/// The e-fold in cooling age of the share of white dwarfs with a debris disc: 1 Gyr ÷
/// ln([`DEBRIS_DISC_SHARE_YOUNG`] ÷ [`DEBRIS_DISC_SHARE_AT_GYR`]), 0.721 Gyr.
#[must_use]
pub fn debris_disc_e_fold() -> Years {
    Years::new(1.0e9 / math::ln(DEBRIS_DISC_SHARE_YOUNG / DEBRIS_DISC_SHARE_AT_GYR))
}

/// The share of white dwarfs with a surviving belt and at least one planet that carry a debris
/// disc at cooling age `cooling_age`: [`DEBRIS_DISC_SHARE_YOUNG`] e^(−τ ÷ `τ_d`), with `τ_d`
/// [`debris_disc_e_fold`], so 4% at birth and 1% at 1 Gyr.
#[must_use]
pub fn debris_disc_share(cooling_age: Years) -> f64 {
    DEBRIS_DISC_SHARE_YOUNG * math::exp(-cooling_age.value() / debris_disc_e_fold().value())
}

/// The inner edge of a white dwarf's debris disc: where a flat, opaque dust ring about a star of
/// radius `radius` and effective temperature `effective_temperature` reaches
/// [`DUST_SUBLIMATION_TEMPERATURE`].
///
/// Such a ring's temperature at r is (2 ÷ 3π)^¼ (R★ ÷ r)^¾ T★ (Chiang and Goldreich 1997, ApJ 490,
/// 368, their flat disc; Jura 2003, eq. 1), so the edge is R★ (2 ÷ 3π)^⅓ (T★ ÷ T)^(4⁄3): about
/// 0.1 R☉ for a 10,000 K white dwarf of 0.0125 R☉.
#[must_use]
pub fn debris_disc_inner_edge(radius: SolarRadii, effective_temperature: Kelvin) -> Metres {
    let ratio = effective_temperature.value() / DUST_SUBLIMATION_TEMPERATURE.value();
    let ratio_squared = ratio * ratio;
    Metres::new(
        Metres::from(radius).value()
            * math::cbrt(2.0 / (3.0 * PI))
            * math::cbrt(ratio_squared * ratio_squared),
    )
}

/// The outer edge of a white dwarf's debris disc: its rigid Roche limit for a rocky asteroid of a
/// belt's [`BeltComposition::Rocky`] member density, 1.26 R (`ρ_wd` ÷ ρ)^⅓, about 0.86 R☉ for a
/// white dwarf of 0.6 M☉ and 0.0125 R☉ (design note 11's "about 1 R☉").
///
/// # Panics
///
/// In debug builds, if the radius is not positive.
#[must_use]
pub fn debris_disc_outer_edge(mass: SolarMasses, radius: SolarRadii) -> Metres {
    let r = Metres::from(radius);
    debug_assert!(r.value() > 0.0, "a white dwarf has a radius");
    let volume = 4.0 / 3.0 * PI * r.value() * r.value() * r.value();
    let density = KilogramsPerCubicMetre::new(Kilograms::from(mass).value() / volume);
    roche_limit_rigid(r, density, BeltComposition::Rocky.member_density())
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::Layer;
    use crate::planetary::testing::with_every_26_hold;

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [3, -2, 1]).expect("a cell of the grid");
        SystemId::from_parts(Layer::A, cell, index).expect("a system of the cell")
    }

    /// P14.T28.d is held until the 25 → 26 bump, inside 26's helper alone.
    #[test]
    fn the_white_dwarf_debris_is_held_only_until_the_26_bump() {
        assert!(
            GENERATOR_VERSION < WHITE_DWARF_DEBRIS_VERSION,
            "the 25 → 26 bump marks white dwarfs' systems polluted and gives some a debris disc \
             (P14.T28.d): delete `WHITE_DWARF_DEBRIS_VERSION`, `WhiteDwarfDebrisRules` with its \
             `Held` arm and the held arm of `SystemDiscs::find`, the tests' \
             `with_white_dwarf_debris`, its switch in `with_every_26_hold` and this test, and \
             re-bless the goldens whose systems have a white dwarf"
        );
        assert_eq!(
            WhiteDwarfDebrisRules::in_force(),
            WhiteDwarfDebrisRules::Held
        );
        assert_eq!(
            with_every_26_hold(WhiteDwarfDebrisRules::in_force),
            WhiteDwarfDebrisRules::Debris
        );
        assert_eq!(
            with_every_26_hold(|| {
                with_white_dwarf_debris(
                    WhiteDwarfDebrisRules::Held,
                    WhiteDwarfDebrisRules::in_force,
                )
            }),
            WhiteDwarfDebrisRules::Held
        );
        assert_eq!(
            WhiteDwarfDebrisRules::in_force(),
            WhiteDwarfDebrisRules::Held
        );
    }

    #[test]
    fn the_draws_are_slot_0xee_s_words_of_the_system_s_belt_stream() {
        let seed = Seed::new(0x0014_0028_d000_0001);
        let draws = WhiteDwarfDraws::for_system(seed, system(5));
        assert_eq!(draws, WhiteDwarfDraws::for_system(seed, system(5)));
        assert_ne!(draws, WhiteDwarfDraws::for_system(seed, system(6)));
        let mut stream = Stream::open(seed, tags::BELT_POPULATION, ObjectKey::from(system(5)));
        stream.seek(112);
        assert_eq!(draws.pollution, Mark::from_word(stream.next_u64()));
        assert_eq!(draws.disc, Mark::from_word(stream.next_u64()));
        assert_same_bits(draws.disc_span.value(), stream.uniform_open());
    }

    /// Design note 11's shares over many systems' draws: 25–50% polluted, the 37.5% to a
    /// binomial 4σ, none without a belt and a planet; a disc only on a polluted one, at 4% of all
    /// at birth falling to 1% at 1 Gyr.
    #[test]
    fn a_quarter_to_a_half_are_polluted_and_one_to_four_per_cent_carry_a_disc() {
        let seed = Seed::new(0x0014_0028_d000_0002);
        let n = 40_000_u32;
        let marks: Vec<WhiteDwarfMarks> = (0..n)
            .map(|i| {
                let draws = WhiteDwarfDraws::for_system(seed, system(i));
                assert_eq!(
                    WhiteDwarfMarks::decide(&draws, Survivors::Fewer),
                    WhiteDwarfMarks {
                        polluted: false,
                        disc_until: None
                    }
                );
                WhiteDwarfMarks::decide(&draws, Survivors::BeltAndPlanet)
            })
            .collect();
        let total = f64::from(n);
        let count = |keep: &dyn Fn(&WhiteDwarfMarks) -> bool| {
            f64::from(u32::try_from(marks.iter().filter(|m| keep(m)).count()).expect("fits"))
        };
        let polluted = count(&|m| m.is_polluted()) / total;
        let sigma = (POLLUTED_SHARE * (1.0 - POLLUTED_SHARE) / total).sqrt();
        assert!((0.25..=0.5).contains(&polluted), "{polluted}");
        assert!(
            (polluted - POLLUTED_SHARE).abs() < 4.0 * sigma,
            "{polluted}"
        );
        assert!(
            marks
                .iter()
                .all(|m| m.disc_until().is_none() || m.is_polluted())
        );
        for gyr in [0.0, 0.25, 0.5, 1.0, 2.0] {
            let age = Years::new(gyr * 1.0e9);
            let share = count(&|m| m.disc_until().is_some_and(|until| until > age)) / total;
            let expected = debris_disc_share(age);
            let sigma = (expected * (1.0 - expected) / total).sqrt();
            assert!(
                (share - expected).abs() < 4.0 * sigma,
                "{share} with a disc at {gyr} Gyr against {expected}"
            );
            if gyr <= 1.0 {
                assert!((0.009..=0.041).contains(&share), "{share} at {gyr} Gyr");
            }
        }
        assert!((debris_disc_share(Years::new(1.0e9)) - 0.01).abs() < 1e-15);
        assert!((debris_disc_e_fold().value() - 7.213_475_204_444_817e8).abs() < 1.0);
    }

    /// The disc lies inside the Roche limit, about 1 R☉, and outside the sublimation radius,
    /// about 0.1 R☉ at 10,000 K.
    #[test]
    fn the_disc_lies_inside_the_roche_limit_and_outside_the_sublimation_radius() {
        let solar_radius = Metres::from(SolarRadii::new(1.0)).value();
        let outer = debris_disc_outer_edge(SolarMasses::new(0.6), SolarRadii::new(0.0125));
        assert!(
            (0.8..1.0).contains(&(outer.value() / solar_radius)),
            "{outer:?}"
        );
        let inner = debris_disc_inner_edge(SolarRadii::new(0.0125), Kelvin::new(10_000.0));
        assert!(
            (0.08..0.15).contains(&(inner.value() / solar_radius)),
            "{inner:?}"
        );
        // At the sublimation temperature itself the edge is (2 ÷ 3π)^⅓ radii out.
        let at = debris_disc_inner_edge(SolarRadii::new(1.0), DUST_SUBLIMATION_TEMPERATURE);
        assert!((at.value() / solar_radius - math::cbrt(2.0 / (3.0 * PI))).abs() < 1e-12);
    }
}
