//! What sits inside a shell: the compact remnant's offset, its bow shock and its wind nebula
//! (P09.T16.b).
//!
//! The remnant leaves the explosion site at its natal kick, so it sits at kick × age from the
//! shell's centre (brainstorm, "What is inside"). A pulsar moving through its own remnant turns
//! supersonic against the hot interior, and drives a bow shock, once it has travelled 0.677 of the
//! shell's Sedov–Taylor radius (van der Swaluw, Downes and Keegan 2004, A&A 420, 937, §2 and Fig.
//! 4), which it keeps as it crosses the shell and after. The fraction is derived for a
//! Sedov–Taylor interior and is extrapolated here to the snowplough phases. A pulsar powers a
//! wind nebula while its spin-down luminosity stays above plan 06's [`WIND_NEBULA_THRESHOLD`],
//! 10³⁵ erg/s (ruling 136.4), which its birth spins and fields keep for a median of about 1.5 ×
//! 10⁴ years (plan 09, P09.T16.b: "10⁴–10⁵ yr").
//!
//! [`WIND_NEBULA_THRESHOLD`]: crate::stellar::remnant::neutron_star::WIND_NEBULA_THRESHOLD

use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::snr::shell::ShellState;
use crate::stellar::remnant::neutron_star::PulsarState;
use crate::units::{KilometresPerSecond, LightYears, Watts, Years};

/// The share of a shell's radius past which the remnant drives a bow shock: 0.68, van der Swaluw,
/// Downes and Keegan's (2004, A&A 420, 937, §2) 0.677 for a pulsar in a Sedov–Taylor remnant.
pub const BOW_SHOCK_SHARE: f64 = 0.68;

/// How far a remnant kicked at `kick` has moved from the explosion site `age` after it, in the
/// frame of the site: `kick × age`, on a straight line. A negative age gives zero.
///
/// # Examples
///
/// A typical neutron-star kick of 300 km/s carries the remnant about 300 ly in 3 × 10⁵ years.
///
/// ```
/// use hyperion_sim::galaxy::snr::remnant_offset;
/// use hyperion_sim::units::{KilometresPerSecond, Years};
///
/// let offset = remnant_offset(KilometresPerSecond::new(300.0), Years::new(3e5));
/// assert!((offset.value() - 300.2).abs() < 0.1);
/// ```
#[must_use]
pub fn remnant_offset(kick: KilometresPerSecond, age: Years) -> LightYears {
    LightYears::new(kick.value().abs() * age.value().max(0.0) * LIGHT_YEARS_PER_YEAR_PER_KM_S)
}

/// Whether a remnant `offset` from the centre of `shell` drives a bow shock: past
/// [`BOW_SHOCK_SHARE`] of the shell's radius, inside the shell or outside it.
#[must_use]
pub fn has_bow_shock(offset: LightYears, shell: &ShellState) -> bool {
    offset.value() >= BOW_SHOCK_SHARE * shell.radius().value()
}

/// A pulsar's wind nebula: the bubble of relativistic particles its spin-down powers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulsarWindNebula {
    power: Watts,
}

impl PulsarWindNebula {
    /// The nebula of the pulsar in `state`, while it has one ([`PulsarState::has_wind_nebula`],
    /// above plan 06's `WIND_NEBULA_THRESHOLD`);
    /// `None` after.
    #[must_use]
    pub fn of(state: &PulsarState) -> Option<Self> {
        state.has_wind_nebula().then(|| Self {
            power: state.spin_down_luminosity(),
        })
    }

    /// The spin-down luminosity that powers it.
    #[must_use]
    pub const fn power(&self) -> Watts {
        self.power
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::Seed;
    use crate::coords::GalacticPosition;
    use crate::galaxy::Galaxy;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::snr::shell::shell_state_at;
    use crate::galaxy::snr::window::{ExplosionEnergy, SiteGas, shell_window};
    use crate::math;
    use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
    use crate::stellar::remnant::KickLawParams;
    use crate::stellar::remnant::neutron_star::NeutronStar;
    use crate::units::Dex;

    /// P09.T16.b: field remnants sit a median of about 200 ly from their shells' centres, and
    /// about half have left their shells. The sites are the gas of the young disc's plane, the
    /// ages uniform over each window, the kicks the ordinary mode's at uniform ranks (plan 06's
    /// law, whose median is 270 km/s).
    #[test]
    fn field_remnants_are_a_median_of_about_two_hundred_light_years_out() {
        let galaxy =
            Galaxy::from_params(Seed::new(0x0916_0b01), GalaxyParams::milky_way_like()).unwrap();
        let law = KickLawParams::default();
        let mut lcg = Lcg::new(0x0916_0b02);
        let mut cache = NoiseCache::with_capacity(1_024);
        let (mut offsets, mut outside, mut shells) = (Vec::new(), 0_u32, 0_u32);
        while shells < 4_000 {
            let r = 15_000.0 + 20_000.0 * lcg.next_f64();
            let phi = core::f64::consts::TAU * lcg.next_f64();
            let z = 200.0 * (2.0 * lcg.next_f64() - 1.0);
            let p = GalacticPosition::from_light_years([r * math::cos(phi), r * math::sin(phi), z])
                .unwrap();
            let site = SiteGas::at(galaxy.gas(), &p, &mut cache);
            let window = shell_window(
                &site,
                ExplosionEnergy::from_uniform(lcg.next_f64()),
                Dex::new(0.0),
            );
            let age = Years::new(window.duration().value() * lcg.next_f64());
            let Some(shell) = shell_state_at(&window, age) else {
                continue;
            };
            shells += 1;
            let kick = KilometresPerSecond::new(law.ordinary_speed_km_s(lcg.next_f64()));
            let offset = remnant_offset(kick, age);
            outside += u32::from(offset > shell.radius());
            offsets.push(offset.value());
        }
        offsets.sort_by(f64::total_cmp);
        let median = offsets[offsets.len() / 2];
        let share = f64::from(outside) / f64::from(shells);
        eprintln!("field remnants: median offset {median:.0} ly, {share:.3} outside their shells");
        assert!((100.0..=400.0).contains(&median), "{median}");
        assert!((0.3..=0.7).contains(&share), "{share}");
    }

    /// P09.T16.b: most shells older than 10⁵ yr have no pulsar wind nebula, and a pulsar born
    /// with one keeps it for a median of 10⁴–10⁵ yr.
    #[test]
    fn most_old_shells_have_no_nebula() {
        let (mut born, mut old, mut total) = (0_u32, 0_u32, 0_u32);
        let mut ends = Vec::new();
        let mut lcg = Lcg::new(0x0916_0b03);
        for k in 0..4_000_u32 {
            let mut normal = || {
                StandardNormal::new(math::normal_quantile(
                    lcg.next_f64().clamp(1e-12, 1.0 - 1e-12),
                ))
                .expect("finite")
            };
            let ns_spin = core::array::from_fn(|_| normal());
            let ns_field = normal();
            let ns = NeutronStar::from_draws(&StarDraws::from_parts(StarDrawsParts {
                ns_spin,
                ns_field,
                ..StarDrawsParts::MEDIAN
            }));
            total += 1;
            let age = Years::new(math::exp10(5.0 + f64::from(k % 100) / 100.0));
            old += u32::from(PulsarWindNebula::of(&ns.state_at(age)).is_some());
            if PulsarWindNebula::of(&ns.state_at(Years::ZERO)).is_none() {
                continue;
            }
            born += 1;
            // Ė falls with age, so the nebula ends at one root, found by bisection in log age.
            let (mut lo, mut hi) = (0.0_f64, 9.0_f64);
            for _ in 0..60 {
                let mid = f64::midpoint(lo, hi);
                if PulsarWindNebula::of(&ns.state_at(Years::new(math::exp10(mid)))).is_some() {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            ends.push(math::exp10(lo));
        }
        ends.sort_by(f64::total_cmp);
        let median_end = ends[ends.len() / 2];
        let old = f64::from(old) / f64::from(total);
        eprintln!(
            "nebulae: {born} of {total} born with one, median end {median_end:.3e} yr; {old:.4} \
             of pulsars 10⁵–10⁶ yr old have one"
        );
        assert!(old < 0.5, "{old}");
        assert!((1e4..=1e5).contains(&median_end), "{median_end}");
    }

    #[test]
    fn a_bow_shock_forms_past_two_thirds_of_the_radius() {
        let site = SiteGas::uniform(
            crate::units::HydrogenPerCm3::new(1.0),
            crate::units::KelvinPerCm3::new(3_800.0),
        )
        .unwrap();
        let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
        let shell = shell_state_at(&window, Years::new(1e5)).unwrap();
        let r = shell.radius().value();
        assert!(!has_bow_shock(LightYears::new(0.6 * r), &shell));
        assert!(has_bow_shock(LightYears::new(0.7 * r), &shell));
        assert!(has_bow_shock(LightYears::new(2.0 * r), &shell));
        assert_eq!(
            remnant_offset(KilometresPerSecond::new(500.0), Years::new(-3.0)),
            LightYears::ZERO
        );
    }
}
