//! The stellar stage's entry points that keep no track: a star's state at one age, its lifetime,
//! and the turn-off mass of a population (plan 06, P06.T10.e).
//!
//! Each builds what it needs of the star's [`Track`] and returns one number or state. A caller that
//! asks about one star more than once keeps the [`Track`] instead, which is what the server's cache
//! holds (plan 06, design note 19).

use crate::galaxy::quad::bisect;
use crate::math;
use crate::stellar::draws::StarDraws;
use crate::stellar::substellar;
use crate::stellar::{Composition, StarState};
use crate::units::{SolarMasses, Years};

use super::coeffs::ZCoeffs;
use super::ms;
use super::track::{self, MAX_INITIAL_MASS, MIN_INITIAL_MASS, Track, TrackOptions};

/// The state at `age` (years since the onset of collapse) of a star of initial mass `m0`,
/// `composition` and `draws`: the track built to that age and read there.
///
/// `m0` is 0.01–100 M☉. From 0.1 M☉ up the state is the track's. Below it the object is read
/// from P06.T13's cooling fits, [`substellar::cooling`], and never leaves that phase within any
/// age the fields draw (ruling 33 of 2026-09-22).
///
/// # Panics
///
/// If `m0` is below [`substellar::MIN_MASS`], or `age` is negative or not finite, where the cooling
/// fits refuse; in debug builds also if `m0` lies above [`Track`]'s range.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::{Composition, Phase, evolve};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A 2 M☉ star of solar composition is a giant at 1.2 Gyr, off the main sequence at 1.16.
/// let giant = evolve(
///     SolarMasses::new(2.0),
///     &Composition::SOLAR,
///     &StarDraws::median(),
///     Years::new(1.2e9),
/// );
/// assert_eq!(giant.phase(), Phase::CoreHeliumBurning);
/// ```
#[must_use]
pub fn evolve(
    m0: SolarMasses,
    composition: &Composition,
    draws: &StarDraws,
    age: Years,
) -> StarState {
    if m0 < MIN_INITIAL_MASS {
        return substellar::cooling(m0, age, composition)
            .expect("a mass of 0.01-0.1 M_sun at a finite non-negative age is inside the fits");
    }
    Track::to_age(m0, composition, draws, age).state_at(age)
}

/// The lifetime of a star of initial mass `m0`, `composition` and `draws` under the generator's
/// options: the age at which it dies, years since the onset of collapse.
///
/// It integrates the same grid as [`Track::full`] and returns [`Track::lifetime`] bit for bit,
/// but keeps no track: no samples for the maxima and no remnant, the cost that plan 08's
/// placement pays for layers D and E (plan 06's "Verification" puts it at about 5 µs).
///
/// # Panics
///
/// In debug builds, if `m0` lies outside [`Track`]'s range, 0.1–100 M☉.
#[must_use]
pub fn lifetime(m0: SolarMasses, composition: &Composition, draws: &StarDraws) -> Years {
    track::lifetime_of(m0, composition, draws, TrackOptions::default())
}

/// The initial mass whose main sequence ends at `age` for a star of `composition`: the inverse in
/// mass of `t_zams` + `t_MS` (HPT equation 5), found by 64 bisections in log mass (plan 02's
/// [`bisect`]). `t_zams`, the arrival on the zero-age main sequence, is zero until P06.T15.b.
///
/// Held to 0.1–100 M☉: an age beyond the main sequence of a 0.1 M☉ star gives 0.1, and one before
/// that of a 100 M☉ star gives 100.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::sse::turn_off_mass;
/// use hyperion_sim::units::Years;
///
/// // The Sun's main sequence lasts about 10.9 Gyr: at that age the turn-off is near 1 M☉.
/// let m = turn_off_mass(Years::new(1.09e10), &Composition::SOLAR);
/// assert!((m.value() - 1.0).abs() < 0.02, "{m:?}");
/// ```
#[must_use]
pub fn turn_off_mass(age: Years, composition: &Composition) -> SolarMasses {
    let c = ZCoeffs::new(composition.z_fit());
    let age_myr = age.value() * 1e-6;
    let (lo, hi) = (
        math::log10(MIN_INITIAL_MASS.value()),
        math::log10(MAX_INITIAL_MASS.value()),
    );
    let excess = |log_m: f64| ms::t_ms(SolarMasses::new(math::exp10(log_m)), &c).value() - age_myr;
    if excess(lo) <= 0.0 {
        return MIN_INITIAL_MASS;
    }
    if excess(hi) >= 0.0 {
        return MAX_INITIAL_MASS;
    }
    SolarMasses::new(math::exp10(bisect(excess, lo, hi, 64)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ruling 33: below 0.1 M☉ `evolve` reads the cooling fits, and the state is continuous across
    /// the hand-over, which P06.T13 pins to the ZAMS at 0.1 M☉ (0.53% in L from 1 Gyr).
    #[test]
    fn evolve_hands_substellar_masses_to_the_cooling_fits() {
        let draws = StarDraws::median();
        let age = Years::new(5.0e9);
        for m in [0.012, 0.05, 0.08, 0.099] {
            let m = SolarMasses::new(m);
            let expected =
                substellar::cooling(m, age, &Composition::SOLAR).expect("inside the fits");
            assert_eq!(
                evolve(m, &Composition::SOLAR, &draws, age),
                expected,
                "{m:?}"
            );
        }
        let below = evolve(
            SolarMasses::new(0.099_999),
            &Composition::SOLAR,
            &draws,
            age,
        );
        let above = evolve(MIN_INITIAL_MASS, &Composition::SOLAR, &draws, age);
        let ratio = below.luminosity().value() / above.luminosity().value();
        assert!(
            (ratio - 1.0).abs() < 0.02,
            "L jumps by {ratio} across 0.1 M☉"
        );
    }

    /// P06.T12.c, main-sequence lifetimes against the paper: HPT's Fig. 5 (journal page 548) plots
    /// log `t_BGB` of Pols et al.'s (1998) detailed models at Z = 10⁻⁴ (+) and 0.03 (*) with
    /// equation 4's fit, which HPT state is within 4.8% of every model. The models' log `t_BGB`
    /// (Myr) here are read from the journal's 799-dpi figure, the axes calibrated on their major
    /// ticks, to about ±0.005 dex, at the models' masses. Above 4 M☉ the two metallicities'
    /// markers overlap and cannot be read apart, and the model near 0.63 M☉ is left out, because
    /// at log t ∝ −3.7 log M its mass would need three digits. The tolerance is HPT's 4.8% with
    /// that reading error, 0.03 dex.
    #[test]
    fn main_sequence_timescales_match_the_detailed_models_of_hpt_figure_5() {
        const MASSES: [f64; 12] = [0.5, 0.8, 0.9, 1.0, 1.1, 1.25, 1.4, 1.6, 2.0, 2.5, 3.2, 4.0];
        const LOW_Z: [f64; 12] = [
            4.878, 4.132, 3.951, 3.793, 3.652, 3.468, 3.310, 3.130, 2.855, 2.612, 2.369, 2.158,
        ];
        const HIGH_Z: [f64; 12] = [
            5.165, 4.500, 4.290, 4.113, 3.955, 3.744, 3.575, 3.396, 3.108, 2.826, 2.525, 2.264,
        ];
        for (z, models) in [(1e-4, LOW_Z), (0.03, HIGH_Z)] {
            let c = ZCoeffs::new(crate::units::MetalFraction::new(z));
            for (m, log_t) in MASSES.into_iter().zip(models) {
                let ours = math::log10(ms::t_bgb(SolarMasses::new(m), &c).value());
                assert!(
                    (ours - log_t).abs() < 0.03,
                    "{m} M☉ at Z = {z}: log t_BGB {ours} against the model's {log_t}"
                );
            }
        }
    }

    #[test]
    fn the_turn_off_mass_inverts_the_main_sequence_lifetime() {
        let c = ZCoeffs::new(Composition::SOLAR.z_fit());
        for m in [0.3, 0.8, 1.0, 2.0, 7.0, 40.0] {
            let t = ms::t_ms(SolarMasses::new(m), &c).value() * 1e6;
            let back = turn_off_mass(Years::new(t), &Composition::SOLAR).value();
            assert!((back / m - 1.0).abs() < 1e-12, "{m}: {back}");
        }
        assert_eq!(
            turn_off_mass(Years::new(1e16), &Composition::SOLAR),
            MIN_INITIAL_MASS
        );
        assert_eq!(
            turn_off_mass(Years::new(1e3), &Composition::SOLAR),
            MAX_INITIAL_MASS
        );
    }
}
