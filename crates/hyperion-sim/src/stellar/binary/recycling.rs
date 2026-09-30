//! Recycled pulsars (plan 11, P11.T5): the spin and field a neutron star is left with after a
//! companion has fed it, as closed forms in the mass it accreted, composed with plan 06's
//! spin-down (P06.T21).
//!
//! A neutron star is born with plan 06's spin and field ([`NeutronStar::from_draws`]) and spins
//! down on its closed forms until a companion fills its Roche lobe. Accretion then does two things
//! (Bhattacharya and van den Heuvel 1991, Physics Reports 203, 1, section 5):
//!
//! - it buries the field, B = B₀ ÷ (1 + ΔM ÷ `m_B`) ([`recycled_field`]);
//! - it spins the star up, towards the period whose angular momentum the accreted mass carries
//!   ([`spin_up_period`]), but never past the equilibrium period of the buried field at the mean
//!   rate of accretion ([`equilibrium_period`]).
//!
//! The recycled star is plan 06's [`NeutronStar`] again, born at the end of accretion with that
//! period and field and its own spin axis and geometry, so that it spins down afterwards on the
//! same closed forms as every other pulsar ([`recycle`]). A field buried below plan 06's floor of
//! 10¹² G stays where it is, since that law's floor is the lower of 10¹² G and the birth field.
//!
//! Each figure was re-checked against its source for P11.T5, except Shibazaki et al.'s (1989),
//! read through the two papers that quote it; the constants' doc comments say where.

use crate::math;
use crate::stellar::Phase;
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::{NeutronStar, PulsarState, RemnantKind};
use crate::units::{Gauss, Seconds, SolarMasses, SolarMassesPerYear, Years};

use super::timeline::{BinaryTimeline, Component, SegmentKind};

/// The accreted mass that buries a field by a factor of two, `m_B`, M☉: 10⁻⁴, in B = B₀ ÷ (1 +
/// ΔM ÷ `m_B`) (Shibazaki, Murakami, Shaham and Nomoto 1989, Nature 342, 656, as Kiel et al. 2008,
/// MNRAS 388, 393, equation 9 quote it, "of order 10⁻⁴ M☉"; Zhang and Kojima 2006, MNRAS 366, 137,
/// section 2.2, give the fit's range as 10⁻⁵–10⁻³). The primary source is not re-read.
pub(crate) const FIELD_BURIAL_MASS: SolarMasses = SolarMasses::new(1.0e-4);

/// The weakest field accretion leaves, G: 10⁸, the bottom field reached when the magnetosphere
/// shrinks to the star's surface (Zhang and Kojima 2006, section 2.2; Kiel et al. 2008, section
/// 3.6, take 5 × 10⁷).
pub(crate) const MIN_RECYCLED_FIELD: Gauss = Gauss::new(1.0e8);

/// The accreted mass that spins a neutron star of one solar mass up to a period of one
/// millisecond, M☉: 0.22 in `ΔM_eq` = 0.22 M☉ (M ÷ M☉)^⅓ ÷ (P ÷ ms)^(4/3) (Tauris, Langer and
/// Kramer 2012, MNRAS 425, 1601, equation 14, with their f = 1 and I₄₅ = M ÷ M☉).
pub(crate) const SPIN_UP_MASS_AT_1_MS: SolarMasses = SolarMasses::new(0.22);

/// The equilibrium period of a neutron star of 1.4 M☉ and radius 13 km with a field of 10⁸ G,
/// accreting at a tenth of the Eddington rate, s: 1.40 ms in `P_eq` = 1.40 ms B₈^(6/7)
/// (Ṁ ÷ 0.1 `Ṁ_Edd`)^(−3/7) (M ÷ 1.4 M☉)^(−5/7) R₁₃^(18/7) (Tauris, Langer and Kramer 2012,
/// equation 7, with their sin α = φ = ξ = `ω_c` = 1).
pub(crate) const EQUILIBRIUM_PERIOD_REFERENCE: Seconds = Seconds::new(1.40e-3);

/// The Eddington accretion rate of a neutron star of radius 13 km, M☉ yr⁻¹: 3.0 × 10⁻⁸ R₁₃ ×
/// 1.3 ÷ (1 + X) (Tauris, Langer and Kramer 2012, equation 6), here for a hydrogen-rich donor's
/// X = 0.7; [`eddington_rate`] scales it by the star's radius.
pub(crate) const EDDINGTON_RATE: SolarMassesPerYear = SolarMassesPerYear::new(3.0e-8 * 1.3 / 1.7);

/// The radius of a neutron star of one solar mass in the equilibrium period, km: 15, in R =
/// 15 km (M ÷ M☉)^(−⅓) (Tauris, Langer and Kramer 2012, section 4.2.2), so that the Eddington rate
/// and `P_eq` read one radius.
const EQUILIBRIUM_RADIUS_KM_AT_1_MSUN: f64 = 15.0;

/// The longest period a millisecond pulsar has, s: 30 ms, "1.4 ms ≲ P ≲ 30 ms" (Lorimer 2008,
/// Living Reviews in Relativity 11, 8, section 2.2; Manchester 2017 draws the line at 100 ms, so
/// no single standard exists).
pub const MILLISECOND_PULSAR_MAX_PERIOD: Seconds = Seconds::new(0.030);

/// The least accreted mass that recycles a neutron star, M☉: below it the star keeps plan 06's
/// spin and field, which a trace of wind can never have changed measurably (10⁻⁹ M☉ buries a
/// field by 10⁻⁵ of itself).
const MIN_ACCRETED_MASS: f64 = 1.0e-9;

/// The field left after accreting `accreted` onto a field of `before`: B₀ ÷ (1 + ΔM ÷ `m_B`)
/// (Shibazaki et al. 1989), never below [`MIN_RECYCLED_FIELD`] nor above `before`.
#[must_use]
pub(crate) fn recycled_field(before: Gauss, accreted: SolarMasses) -> Gauss {
    let dm = accreted.value().max(0.0);
    let buried = before.value() / (1.0 + dm / FIELD_BURIAL_MASS.value());
    Gauss::new(buried.max(MIN_RECYCLED_FIELD.value().min(before.value())))
}

/// The period a neutron star of `mass` reaches once it has accreted `accreted`, if it started much
/// slower: the inverse of ΔM = 0.22 M☉ (M ÷ M☉)^⅓ ÷ (P ÷ ms)^(4/3) (Tauris, Langer and Kramer 2012,
/// equation 14), infinite for no accretion.
#[must_use]
pub(crate) fn spin_up_period(accreted: SolarMasses, mass: SolarMasses) -> Seconds {
    let dm = accreted.value();
    if dm <= 0.0 {
        return Seconds::new(f64::INFINITY);
    }
    let scale = SPIN_UP_MASS_AT_1_MS.value() * math::cbrt(mass.value().max(1e-6)) / dm;
    Seconds::new(1.0e-3 * math::powf_positive(scale, 0.75))
}

/// The radius of a neutron star of `mass` in units of 13 km, for the equilibrium period:
/// 15 km (M ÷ M☉)^(−⅓) (Tauris, Langer and Kramer 2012, section 4.2.2).
#[must_use]
fn radius_13_km(mass: SolarMasses) -> f64 {
    EQUILIBRIUM_RADIUS_KM_AT_1_MSUN / math::cbrt(mass.value().max(1e-6)) / 13.0
}

/// The Eddington accretion rate of a neutron star of `mass`: [`EDDINGTON_RATE`] × R₁₃ (Tauris,
/// Langer and Kramer 2012, equation 6).
#[must_use]
pub(crate) fn eddington_rate(mass: SolarMasses) -> SolarMassesPerYear {
    SolarMassesPerYear::new(EDDINGTON_RATE.value() * radius_13_km(mass))
}

/// The equilibrium period of a neutron star of `mass` with field `field` accreting at `rate`, the
/// shortest period accretion can spin it to: `P_eq` = 1.40 ms B₈^(6/7) (Ṁ ÷ 0.1 `Ṁ_Edd`)^(−3/7)
/// (M ÷ 1.4 M☉)^(−5/7) R₁₃^(18/7) (Tauris, Langer and Kramer 2012, equation 7), the rate held to
/// the Eddington rate ([`eddington_rate`]), which the star cannot take more than. Infinite for no
/// accretion.
#[must_use]
pub(crate) fn equilibrium_period(
    field: Gauss,
    rate: SolarMassesPerYear,
    mass: SolarMasses,
) -> Seconds {
    let eddington = eddington_rate(mass).value();
    let rate = rate.value().min(eddington);
    if rate <= 0.0 {
        return Seconds::new(f64::INFINITY);
    }
    let period = EQUILIBRIUM_PERIOD_REFERENCE.value()
        * math::powf_positive(field.value() / 1.0e8, 6.0 / 7.0)
        * math::powf_positive(rate / (0.1 * eddington), -3.0 / 7.0)
        * math::powf_positive(mass.value().max(1e-6) / 1.4, -5.0 / 7.0)
        * math::powf_positive(radius_13_km(mass), 18.0 / 7.0);
    Seconds::new(period)
}

/// The period a neutron star of `mass` spinning at `before` is left with after accreting
/// `accreted` at a mean `rate` onto a field that was buried to `field`: the spin-up period of
/// the accreted mass, held above the equilibrium period, and never slower than it started.
#[must_use]
pub(crate) fn recycled_period(
    before: Seconds,
    accreted: SolarMasses,
    mass: SolarMasses,
    field: Gauss,
    rate: SolarMassesPerYear,
) -> Seconds {
    let spun = spin_up_period(accreted, mass)
        .value()
        .max(equilibrium_period(field, rate, mass).value());
    Seconds::new(before.value().min(spun))
}

/// What a neutron star accreted from a companion: from the onset of the first Roche-lobe overflow
/// onto it after its birth to the end of the last, ages of the pair, and the mass it gained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Accretion {
    onset: Years,
    end: Years,
    accreted: SolarMasses,
}

impl Accretion {
    /// Accretion from `onset` to `end` (ages of the pair, `end` not before `onset`) of `accreted`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `end` precedes `onset` or either is not finite.
    #[must_use]
    pub(crate) fn new(onset: Years, end: Years, accreted: SolarMasses) -> Self {
        debug_assert!(
            onset.value().is_finite() && end.value().is_finite() && end >= onset,
            "accretion runs forward: {onset:?} to {end:?}"
        );
        Self {
            onset,
            end,
            accreted,
        }
    }

    /// The mean rate over the episode; zero for an episode of no length.
    #[must_use]
    pub(crate) fn mean_rate(&self) -> SolarMassesPerYear {
        let span = self.end.value() - self.onset.value();
        if span > 0.0 {
            SolarMassesPerYear::new(self.accreted.value() / span)
        } else {
            SolarMassesPerYear::ZERO
        }
    }
}

/// The neutron star that `born` (plan 06's, born at age `birth` of the pair with `draws`) becomes
/// once `accretion` has recycled it to `mass`: plan 06's star again, whose own age is counted from
/// the end of accretion, with the recycled period and field, and `born`'s spin axis, magnetic
/// inclination and pulse phase.
///
/// Its state at an age t of the pair after the accretion is `recycle(..).state_at(t − end)`.
#[must_use]
pub(crate) fn recycle(
    born: &NeutronStar,
    draws: &StarDraws,
    birth: Years,
    accretion: &Accretion,
    mass: SolarMasses,
) -> NeutronStar {
    let before = born.state_at(Years::new(accretion.onset.value() - birth.value()));
    let field = recycled_field(before.field(), accretion.accreted);
    let period = recycled_period(
        before.period(),
        accretion.accreted,
        mass,
        field,
        accretion.mean_rate(),
    );
    NeutronStar::new(
        period,
        field,
        born.spin_axis(),
        born.inclination(),
        draws.ns_phase().value(),
    )
}

/// Whether a pulsar in `state` is a millisecond pulsar: faster than
/// [`MILLISECOND_PULSAR_MAX_PERIOD`] and above the death line.
#[must_use]
pub fn is_millisecond_pulsar(state: &PulsarState) -> bool {
    state.period() < MILLISECOND_PULSAR_MAX_PERIOD && state.is_radio_alive()
}

/// A recycled neutron star at an age: its pulsar state, and whether accretion recycled it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulsarAt {
    state: PulsarState,
    recycled: bool,
}

impl PulsarAt {
    /// The pulsar's state.
    #[must_use]
    pub const fn state(&self) -> &PulsarState {
        &self.state
    }

    /// Whether a companion's accretion recycled it.
    #[must_use]
    pub const fn recycled(&self) -> bool {
        self.recycled
    }
}

/// The pulsar that `component` of `timeline` is at `age`, if it is a neutron star then with a
/// birth the timeline recorded, from the component's draws `draws`: plan 06's star spun down from
/// its birth, recycled by the Roche-lobe overflow onto it since ([`recycle`]).
///
/// The accretion is every stable transfer from the other star that began after the birth and
/// before `age`, cut at `age`; the mass gained is the star's mass at the end less its mass at
/// birth. Wind accretion alone recycles nothing.
#[must_use]
pub(crate) fn pulsar_at(
    timeline: &BinaryTimeline,
    component: Component,
    draws: &StarDraws,
    age: Years,
) -> Option<PulsarAt> {
    let t = age.value();
    let state = timeline.state_at(age);
    if state.stars()[component.index()].phase() != Phase::NeutronStar {
        return None;
    }
    let birth = timeline.supernovae().iter().rfind(|s| {
        s.component() == component
            && s.age().value() <= t
            && s.remnant().kind() == RemnantKind::NeutronStar
    })?;
    let born = NeutronStar::from_draws(draws);
    let birth_age = birth.age();
    let fed = SegmentKind::StableTransfer {
        donor: component.other(),
    };
    let episodes = timeline
        .segments()
        .iter()
        .filter(|s| s.kind() == fed && s.end() > birth_age && s.start().value() < t);
    let mut span: Option<(f64, f64)> = None;
    for s in episodes {
        let onset = s.start().value().max(birth_age.value());
        let end = s.end().value().min(t);
        span = Some(span.map_or((onset, end), |(first, _)| (first, end)));
    }
    let plain = |state: PulsarState| PulsarAt {
        state,
        recycled: false,
    };
    let Some((onset, end)) = span else {
        return Some(plain(born.state_at(Years::new(t - birth_age.value()))));
    };
    let mass_after = timeline.state_at(Years::new(end)).stars()[component.index()].mass();
    let accreted = mass_after.value() - birth.remnant().mass().value();
    if accreted < MIN_ACCRETED_MASS {
        return Some(plain(born.state_at(Years::new(t - birth_age.value()))));
    }
    let accretion = Accretion::new(
        Years::new(onset),
        Years::new(end),
        SolarMasses::new(accreted),
    );
    let recycled = recycle(&born, draws, birth_age, &accretion, mass_after);
    Some(PulsarAt {
        state: recycled.state_at(Years::new(t - end)),
        recycled: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::UnitVector;
    use crate::units::Radians;

    #[test]
    fn a_fifth_of_a_solar_mass_makes_a_millisecond_pulsar() {
        let born = NeutronStar::new(
            Seconds::new(0.5),
            Gauss::new(1.0e12),
            UnitVector::NORTH,
            Radians::new(1.0),
            0.25,
        );
        // A fifth of a solar mass over 10⁸ years, 2 × 10⁻⁹ M☉/yr, a tenth of Eddington's.
        let accretion = Accretion::new(Years::new(1.0e8), Years::new(2.0e8), SolarMasses::new(0.2));
        let recycled = recycle(
            &born,
            &StarDraws::median(),
            Years::new(0.0),
            &accretion,
            SolarMasses::new(1.4),
        );
        let now = recycled.state_at(Years::new(1.0e9));
        assert!(
            is_millisecond_pulsar(&now),
            "P {:?}, B {:?}",
            now.period(),
            now.field()
        );
        assert!((1.0e8..3.0e9).contains(&now.field().value()));
        // Recycled pulsars live for many Gyr: the spin-down time is far beyond the age.
        assert!(now.characteristic_age().value() > 1.0e9);
    }

    #[test]
    fn a_trace_of_accretion_leaves_a_slow_pulsar_slow() {
        let before = Seconds::new(2.0);
        let period = recycled_period(
            before,
            SolarMasses::new(1.0e-6),
            SolarMasses::new(1.4),
            Gauss::new(1.0e12),
            SolarMassesPerYear::new(1.0e-10),
        );
        assert!(period <= before);
        assert!(period.value() > 0.03, "{period:?}");
    }

    #[test]
    fn the_spin_up_period_inverts_the_accreted_mass() {
        let m = SolarMasses::new(1.4);
        for p_ms in [1.5, 3.0, 10.0] {
            let dm = SPIN_UP_MASS_AT_1_MS.value() * math::cbrt(1.4) / math::powf(p_ms, 4.0 / 3.0);
            let p = spin_up_period(SolarMasses::new(dm), m).value() * 1.0e3;
            assert!((p / p_ms - 1.0).abs() < 1e-12, "{p} against {p_ms}");
        }
    }

    #[test]
    fn the_equilibrium_period_falls_with_the_rate_and_rises_with_the_field() {
        let m = SolarMasses::new(1.4);
        let at = |b: f64, r: f64| {
            equilibrium_period(Gauss::new(b), SolarMassesPerYear::new(r), m).value()
        };
        assert!(at(1.0e8, 1.0e-9) > at(1.0e8, 1.0e-8));
        assert!(at(1.0e9, 1.0e-9) > at(1.0e8, 1.0e-9));
        assert!(at(1.0e8, 0.0).is_infinite());
        // Above the Eddington rate nothing more is taken.
        assert!((at(1.0e8, 1.0e-6) - at(1.0e8, eddington_rate(m).value())).abs() < 1e-18);
    }

    #[test]
    fn a_tenth_of_a_solar_mass_buries_a_field_a_thousandfold() {
        let b = recycled_field(Gauss::new(1.0e12), SolarMasses::new(0.1));
        assert!((b.value() / 1.0e9 - 1.0).abs() < 1e-2, "{b:?}");
    }

    #[test]
    fn burial_never_raises_a_field_nor_drops_it_below_the_floor() {
        let before = Gauss::new(3.0e11);
        assert_eq!(recycled_field(before, SolarMasses::ZERO), before);
        let deep = recycled_field(before, SolarMasses::new(1.0));
        assert_eq!(deep, MIN_RECYCLED_FIELD);
        let weak = Gauss::new(5.0e7);
        assert_eq!(recycled_field(weak, SolarMasses::new(1.0)), weak);
    }
}
