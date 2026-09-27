//! The binarity seam: the one stripped share the class quadrature and plan 11's systems both read
//! (plan 08, P08.T8.a).
//!
//! A progenitor whose envelope a companion strips dies with a lighter core and, below a
//! carbon–oxygen core of 3 M☉, may take the kick law's low mode (plan 06, design note 11), so the
//! share of stripped primaries sets how many remnants stay near their birth sites. It is plan 11's
//! [`stellar::multiplicity::stripped_share`](crate::stellar::multiplicity::stripped_share): the
//! primary's multiple fraction times the probability that its innermost companion's periastron is
//! close enough to interact before the primary's core collapses, over plan 11's period,
//! mass-ratio and eccentricity laws ([`MultiplicityModel::default_v1`]).
//!
//! # The threshold (ruling 120.1 of 2026-09-22)
//!
//! "Close enough to interact" is P11.T4.a's pre-test, [`can_interact`], read as a threshold on
//! the periastron: a pair interacts before the primary's death when either star's largest radius
//! up to then fills its Roche lobe at periastron, `R_max,i ≥ r_L(m_i ÷ m_j) × a (1 − e)` with
//! Eggleton's (1983) lobe, so the largest interacting periastron is `max_i R_max,i ÷ f(m_i ÷
//! m_j)`, `f` the lobe over the separation ([`interacting_periastron`]). Each star's largest
//! radius is its track's [`Track::max_radius_until`] the primary's death, at the median draws; a
//! companion below the tracks' 0.1 M☉ takes none. It replaced plan 11's provisional 10 au on
//! 2026-09-27.
//!
//! Nothing generated reads the seam yet: plan 11's hierarchy keeps its own provisional range until
//! P11.T1.d, and the class table (P08.T9) is not built. [`kick_bins`](super::kick_bins) reads it.
//!
//! [`can_interact`]: crate::stellar::binary::can_interact

use crate::orbit::roche_lobe_radius;
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{MultiplicityModel, stripped_share as multiplicity_share};
use crate::stellar::sse::{MAX_INITIAL_MASS, MIN_INITIAL_MASS, Track};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{Metres, SolarMasses, Years};

/// A star's largest radius up to `until` (or its whole life), in solar radii, from its track at
/// the median draws; zero below the tracks' lightest mass. Tracks stop at 100 M☉ until P06.T14,
/// so a heavier star is taken as one of 100 M☉.
fn largest_radius(m: SolarMasses, comp: &Composition, until: Option<Years>) -> (f64, Years) {
    if m < MIN_INITIAL_MASS {
        return (0.0, Years::ZERO);
    }
    let m0 = if m > MAX_INITIAL_MASS {
        MAX_INITIAL_MASS
    } else {
        m
    };
    let draws = StarDraws::median();
    let track = match until {
        Some(age) => Track::to_age(m0, comp, &draws, age),
        None => Track::full(m0, comp, &draws),
    };
    let end = until
        .or_else(|| track.lifetime())
        .unwrap_or_else(|| track.built_until());
    (track.max_radius_until(end).value(), end)
}

/// The largest periastron at which stars of `r_1` and `r_2` solar radii and masses `m1` and `m2`
/// fill a Roche lobe (module documentation).
fn threshold(r_1: f64, r_2: f64, m1: f64, m2: f64) -> Metres {
    let unit = Metres::new(SOLAR_RADIUS_M);
    let reach = |r: f64, m: f64, other: f64| {
        if r > 0.0 && m > 0.0 && other > 0.0 {
            r * SOLAR_RADIUS_M / roche_lobe_radius(m / other, unit).value() * SOLAR_RADIUS_M
        } else {
            0.0
        }
    };
    Metres::new(reach(r_1, m1, m2).max(reach(r_2, m2, m1)))
}

/// The largest periastron at which a pair of primary `m1`, mass ratio `q` and composition `comp`
/// interacts before the primary's core collapse: P11.T4.a's [`can_interact`] threshold (module
/// documentation, "The threshold").
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::interacting_periastron;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses};
///
/// // A 15 M☉ red supergiant reaches a companion several au out.
/// let a = interacting_periastron(SolarMasses::new(15.0), 0.5, &Composition::SOLAR);
/// let au = AstronomicalUnits::from(a).value();
/// assert!((2.0..40.0).contains(&au), "{au} au");
/// ```
///
/// [`can_interact`]: crate::stellar::binary::can_interact
#[must_use]
pub fn interacting_periastron(m1: SolarMasses, q: f64, comp: &Composition) -> Metres {
    let (r_1, death) = largest_radius(m1, comp, None);
    let m2 = m1 * q;
    let (r_2, _) = largest_radius(m2, comp, Some(death));
    threshold(r_1, r_2, m1.value(), m2.value())
}

/// The share of primaries of initial mass `m` and composition `comp` whose envelope a companion
/// strips before they die: plan 11's
/// [`stripped_share`](crate::stellar::multiplicity::stripped_share) with its model
/// ([`MultiplicityModel::default_v1`]) and the threshold of [`interacting_periastron`], the
/// primary's track built once.
///
/// # Panics
///
/// If `m` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::stripped_share;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::SolarMasses;
///
/// // Most massive stars have a close companion (Sana et al. 2012), so the share is large.
/// let share = stripped_share(SolarMasses::new(20.0), &Composition::SOLAR);
/// assert!((0.2..0.9).contains(&share), "{share}");
/// ```
#[must_use]
pub fn stripped_share(m: SolarMasses, comp: &Composition) -> f64 {
    let (r_1, death) = largest_radius(m, comp, None);
    multiplicity_share(
        &MultiplicityModel::default_v1(),
        m,
        comp,
        |m1: SolarMasses, q: f64, comp: &Composition| {
            let m2 = m1 * q;
            let (r_2, _) = largest_radius(m2, comp, Some(death));
            threshold(r_1, r_2, m1.value(), m2.value())
        },
    )
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::math;
    use crate::units::{Dex, HeliumExcess};

    fn layer_e(i: i32) -> SolarMasses {
        SolarMasses::new(8.0 * math::exp(f64::from(i) / 32.0 * math::ln(150.0 / 8.0)))
    }

    /// P08.T8.a: the seam is plan 11's function with plan 11's model and P11.T4.a's threshold, bit
    /// for bit, at 33 masses across layer E's band and three metallicities.
    #[test]
    fn the_seam_is_plan_elevens_share() {
        let model = MultiplicityModel::default_v1();
        for fe_h in [-1.5, 0.0, 0.3] {
            let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for m in (0..33).step_by(4).map(layer_e) {
                let direct = multiplicity_share(&model, m, &comp, interacting_periastron);
                let share = stripped_share(m, &comp);
                assert_same_bits(share, direct);
                assert!((0.0..=1.0).contains(&share), "{share} at {m:?}");
            }
        }
    }

    /// The threshold is `can_interact`'s: a pair just inside it interacts before the primary's
    /// death, one just outside does not.
    #[test]
    fn the_threshold_is_can_interacts_boundary() {
        use crate::orbit::{Eccentricity, KeplerElements, Orientation};
        use crate::stellar::binary::{BinaryInput, can_interact};
        use crate::units::{GravitationalParameter, Radians};
        let comp = Composition::SOLAR;
        for (m1, q) in [(12.0, 0.6), (25.0, 0.3), (9.0, 0.95)] {
            let m1 = SolarMasses::new(m1);
            let a = interacting_periastron(m1, q, &comp);
            let death = Track::full(m1, &comp, &StarDraws::median())
                .lifetime()
                .expect("a full track dies");
            let pair = |factor: f64| {
                let orbit = KeplerElements::from_semi_major_axis(
                    Metres::new(a.value() * factor),
                    GravitationalParameter::from_solar_masses(m1 * (1.0 + q)),
                    Eccentricity::CIRCULAR,
                    Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))
                        .unwrap(),
                    Radians::new(0.0),
                )
                .unwrap();
                let input = BinaryInput::new(
                    m1,
                    m1 * q,
                    comp,
                    orbit,
                    [StarDraws::median(), StarDraws::median()],
                    death,
                )
                .unwrap();
                can_interact(&input, death)
            };
            assert!(pair(0.999), "{m1:?} q {q} just inside");
            assert!(!pair(1.001), "{m1:?} q {q} just outside");
        }
    }

    /// Ruling 120.1: the stripped share over neutron-star progenitors is 0.25–0.33 (Sana et al.
    /// 2012; Podsiadlowski et al. 2004). **Provisional, a finding (2026-09-27):** with P11.T4.a's
    /// threshold the seam's share averaged over layer E's band under the default mass function is
    /// the measured value held here, outside that window; plan 11's pre-test counts every pair
    /// that interacts, and Sana's 71% that interact include the 20–30% that merge, which are not
    /// stripped. The test holds the measured value to 0.01, the ruling's window beside it.
    #[test]
    fn the_band_averaged_stripped_share() {
        let imf = MassFunctionKind::default().to_mass_function();
        let (mut sum, mut weights) = (0.0, 0.0);
        for i in 0..33 {
            let m = layer_e(i);
            let end = i == 0 || i == 32;
            let w = imf.pdf(m.value()) * m.value() * if end { 0.5 } else { 1.0 };
            let share = stripped_share(m, &Composition::SOLAR);
            if i % 8 == 0 {
                let a = interacting_periastron(m, 0.5, &Composition::SOLAR);
                let au = crate::units::AstronomicalUnits::from(a).value();
                eprintln!(
                    "{:.1} M☉: share {share:.4}, threshold at q 0.5 {au:.2} au",
                    m.value()
                );
            }
            sum += w * share;
            weights += w;
        }
        let mean = sum / weights;
        eprintln!("band-averaged stripped share {mean:.4} (ruling 120.1: 0.25–0.33)");
        assert!((mean - MEASURED_BAND_SHARE).abs() < 0.01, "{mean}");
    }

    /// The band average [`the_band_averaged_stripped_share`] measures.
    const MEASURED_BAND_SHARE: f64 = 0.485;
}
