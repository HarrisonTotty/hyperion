//! The habitable zone: where liquid water can persist on a rocky planet's surface (plan 14,
//! P14.T12.b).
//!
//! Kopparapu et al. (2013, ApJ 765, 131) give five limits as the stellar flux `S_eff`, in units of
//! the flux at 1 au from the present Sun, at which a cloud-free Earth-like planet reaches each
//! climate state, fitted over stellar effective temperatures of 2,600–7,200 K as `S_eff` = `S_eff☉` +
//! a T + b T² + c T³ + d T⁴ with T = `T_eff` − 5,780 K (their eq. 2). The published coefficients
//! were wrong; every one below is from the erratum's Table 3 (Kopparapu et al. 2013, ApJ 770, 82,
//! "Updated coefficients"), re-checked against the erratum itself, not the arXiv version, which
//! still carries the original ones. The limit lies at d = √(L ÷ `S_eff`) au.
//!
//! The conservative zone runs from the moist greenhouse to the maximum greenhouse, the limits the
//! paper's §5 bounds its zone by, and the optimistic one from recent Venus to early Mars. For the
//! Sun the erratum's coefficients give 0.99–1.69 au and 0.75–1.77 au; its Table 1 prints the
//! maximum and runaway greenhouse at 1.67 and 0.97 au (the paper's own Table 1, 1.70 and 0.97),
//! where the coefficients give 1.690 and 0.982.
//!
//! The fit is for main-sequence stars. About a giant it is applied through `T_eff` alone, which is
//! all the fit reads; [`HabitableZone::extrapolated`] flags only a temperature outside the fit.

use crate::math;
use crate::planetary::derive::irradiation::{HostLight, Illumination};
use crate::units::{AstronomicalUnits, EarthFluxes, Kelvin, Metres, SolarLuminosities};

/// The lowest effective temperature of Kopparapu et al.'s fit, 2,600 K.
pub const FIT_COOLEST: Kelvin = Kelvin::new(2_600.0);

/// The highest effective temperature of Kopparapu et al.'s fit, 7,200 K.
pub const FIT_HOTTEST: Kelvin = Kelvin::new(7_200.0);

/// The effective temperature the fit is centred on, 5,780 K (Kopparapu et al. 2013, eq. 2).
pub const FIT_CENTRE: Kelvin = Kelvin::new(5_780.0);

/// The five limits, in order from the star outwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HabitableLimit {
    /// Recent Venus: Venus has had no surface water for at least a billion years, when the Sun
    /// gave it 1.76 times Earth's flux. The optimistic inner edge.
    RecentVenus,
    /// Runaway greenhouse: the oceans evaporate entirely.
    RunawayGreenhouse,
    /// Moist greenhouse: water reaches the stratosphere and is lost to space. The conservative
    /// inner edge.
    MoistGreenhouse,
    /// Maximum greenhouse: more carbon dioxide cools rather than warms, by Rayleigh scattering.
    /// The conservative outer edge.
    MaximumGreenhouse,
    /// Early Mars: Mars had liquid water 3.8 billion years ago, when the Sun gave it 0.32 times
    /// Earth's flux. The optimistic outer edge.
    EarlyMars,
}

impl HabitableLimit {
    /// Every limit, from the star outwards.
    pub const ALL: [Self; 5] = [
        Self::RecentVenus,
        Self::RunawayGreenhouse,
        Self::MoistGreenhouse,
        Self::MaximumGreenhouse,
        Self::EarlyMars,
    ];

    /// The limit's coefficients `S_eff☉`, a, b, c and d (Kopparapu et al. 2013, erratum, Table 3).
    #[must_use]
    pub const fn coefficients(self) -> [f64; 5] {
        match self {
            Self::RecentVenus => [1.7763, 1.4335e-4, 3.3954e-9, -7.6364e-12, -1.1950e-15],
            Self::RunawayGreenhouse => [1.0385, 1.2456e-4, 1.4612e-8, -7.6345e-12, -1.7511e-15],
            Self::MoistGreenhouse => [1.0146, 8.1884e-5, 1.9394e-9, -4.3618e-12, -6.8260e-16],
            Self::MaximumGreenhouse => [0.3507, 5.9578e-5, 1.6707e-9, -3.0058e-12, -5.1925e-16],
            Self::EarlyMars => [0.3207, 5.4471e-5, 1.5275e-9, -2.1709e-12, -3.8282e-16],
        }
    }

    /// The limit's position in [`ALL`](Self::ALL).
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::RecentVenus => 0,
            Self::RunawayGreenhouse => 1,
            Self::MoistGreenhouse => 2,
            Self::MaximumGreenhouse => 3,
            Self::EarlyMars => 4,
        }
    }

    /// The limit's flux `S_eff` about a host of effective temperature `t_eff`, held to the fit's
    /// 2,600–7,200 K.
    #[must_use]
    pub fn flux(self, t_eff: Kelvin) -> EarthFluxes {
        let [at_centre, linear, quadratic, cubic, quartic] = self.coefficients();
        let offset = t_eff
            .value()
            .clamp(FIT_COOLEST.value(), FIT_HOTTEST.value())
            - FIT_CENTRE.value();
        EarthFluxes::new(
            at_centre
                + linear * offset
                + quadratic * offset * offset
                + cubic * offset * offset * offset
                + quartic * math::powi(offset, 4),
        )
    }
}

/// A host's habitable zone: the distances of Kopparapu et al.'s five limits, and whether a host's
/// temperature lay outside their fit.
///
/// A limit is +∞ where companions at fixed distances alone give more than its flux, so that every
/// orbit lies inside it, and zero about a host that gives no light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HabitableZone {
    limits: [Metres; 5],
    extrapolated: bool,
}

impl HabitableZone {
    /// The distance of `limit`.
    #[must_use]
    pub const fn limit(&self, limit: HabitableLimit) -> Metres {
        self.limits[limit.index()]
    }

    /// The recent-Venus limit, the optimistic inner edge.
    #[must_use]
    pub const fn recent_venus(&self) -> Metres {
        self.limit(HabitableLimit::RecentVenus)
    }

    /// The runaway-greenhouse limit.
    #[must_use]
    pub const fn runaway_greenhouse(&self) -> Metres {
        self.limit(HabitableLimit::RunawayGreenhouse)
    }

    /// The moist-greenhouse limit, the conservative inner edge.
    #[must_use]
    pub const fn moist_greenhouse(&self) -> Metres {
        self.limit(HabitableLimit::MoistGreenhouse)
    }

    /// The maximum-greenhouse limit, the conservative outer edge.
    #[must_use]
    pub const fn maximum_greenhouse(&self) -> Metres {
        self.limit(HabitableLimit::MaximumGreenhouse)
    }

    /// The early-Mars limit, the optimistic outer edge.
    #[must_use]
    pub const fn early_mars(&self) -> Metres {
        self.limit(HabitableLimit::EarlyMars)
    }

    /// The conservative zone, moist greenhouse to maximum greenhouse.
    #[must_use]
    pub const fn conservative(&self) -> (Metres, Metres) {
        (self.moist_greenhouse(), self.maximum_greenhouse())
    }

    /// The optimistic zone, recent Venus to early Mars.
    #[must_use]
    pub const fn optimistic(&self) -> (Metres, Metres) {
        (self.recent_venus(), self.early_mars())
    }

    /// Whether a shining host's effective temperature lay outside the fit's 2,600–7,200 K, where
    /// it was held to the nearer end.
    #[must_use]
    pub const fn extrapolated(&self) -> bool {
        self.extrapolated
    }
}

/// Whether a host of effective temperature `t_eff` lies outside the fit.
#[must_use]
fn outside_fit(t_eff: Kelvin) -> bool {
    !(FIT_COOLEST.value()..=FIT_HOTTEST.value()).contains(&t_eff.value())
}

/// The habitable zone of a single host of luminosity `l` and effective temperature `t_eff`: each
/// limit at √(L ÷ `S_eff`) au, with `T_eff` held to the fit's range and the zone then flagged
/// [`extrapolated`](HabitableZone::extrapolated).
///
/// A host with no light has every limit at zero, and is never flagged.
///
/// # Panics
///
/// In debug builds, if `l` is negative or not finite.
///
/// # Examples
///
/// The Sun's conservative zone is 0.99–1.69 au:
///
/// ```
/// use hyperion_sim::planetary::derive::habitable_zone;
/// use hyperion_sim::units::{AstronomicalUnits, Kelvin, SolarLuminosities};
///
/// let zone = habitable_zone(SolarLuminosities::new(1.0), Kelvin::new(5_772.0));
/// let (inner, outer) = zone.conservative();
/// let au = |m| AstronomicalUnits::from(m).value();
/// assert!((au(inner) - 0.99).abs() < 0.01 && (au(outer) - 1.69).abs() < 0.01);
/// assert!(!zone.extrapolated());
/// ```
#[must_use]
pub fn habitable_zone(l: SolarLuminosities, t_eff: Kelvin) -> HabitableZone {
    debug_assert!(
        l.value().is_finite() && l.value() >= 0.0,
        "a luminosity is finite and not negative, got {l:?}"
    );
    let limits = HabitableLimit::ALL.map(|limit| {
        let d_au = (l.value() / limit.flux(t_eff).value()).sqrt();
        Metres::from(AstronomicalUnits::new(d_au))
    });
    HabitableZone {
        limits,
        extrapolated: l.value() > 0.0 && outside_fit(t_eff),
    }
}

/// The habitable zone of a body that orbits all of `orbited` together and sees each of
/// `companions` along an orbit of its own: each limit where Σ Lᵢ ÷ (dᵢ² `S_eff,ᵢ`) = 1, on
/// orbit-averaged distances (plan 14, P14.T12.b).
///
/// A planet about a pair orbits both stars, at its own distance d from their barycentre. A planet
/// about one star of a binary orbits that star and sees the other along the binary's orbit, a
/// fixed contribution F = Σ Lⱼ ÷ (aⱼ² √(1 − eⱼ²) `S_eff,ⱼ`) averaged over it, so each limit lies at
/// d = √(Σ Lᵢ ÷ `S_eff,ᵢ` ÷ (1 − F)), and at +∞ once F reaches 1. With one orbited host and no
/// companions this is [`habitable_zone`]. Hosts that give no light are passed over.
///
/// # Examples
///
/// A planet about the Sun of a binary whose other star, a K dwarf, is 20 au away sees the zone
/// pushed outwards by the companion's light, and a planet about both would see both at its own
/// distance:
///
/// ```
/// use hyperion_sim::planetary::derive::habitable_zone::habitable_zone_of;
/// use hyperion_sim::planetary::derive::irradiation::{HostLight, Illumination};
/// use hyperion_sim::units::{AstronomicalUnits, Kelvin, Metres, SolarLuminosities, SolarRadii};
///
/// let sun = HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?;
/// let k_dwarf = HostLight::new(SolarLuminosities::new(0.3), Kelvin::new(4_600.0), SolarRadii::new(0.86))?;
/// let binary_orbit = Metres::from(AstronomicalUnits::new(20.0));
/// let companion = Illumination::new(k_dwarf, binary_orbit, 0.3).expect("a bound orbit");
/// let alone = habitable_zone_of(&[sun], &[]);
/// let circumstellar = habitable_zone_of(&[sun], &[companion]);
/// assert!(circumstellar.maximum_greenhouse() > alone.maximum_greenhouse());
/// let circumbinary = habitable_zone_of(&[sun, k_dwarf], &[]);
/// assert!(circumbinary.moist_greenhouse() > circumstellar.moist_greenhouse());
/// # Ok::<(), hyperion_sim::planetary::derive::irradiation::BuildHostLightError>(())
/// ```
#[must_use]
pub fn habitable_zone_of(orbited: &[HostLight], companions: &[Illumination]) -> HabitableZone {
    let shining = |light: &HostLight| light.luminosity().value() > 0.0;
    let limits = HabitableLimit::ALL.map(|limit| {
        let near = orbited
            .iter()
            .filter(|light| shining(light))
            .fold(0.0, |sum, light| {
                sum + light.luminosity().value() / limit.flux(light.effective_temperature()).value()
            });
        let far = companions
            .iter()
            .filter(|source| shining(source.host()))
            .fold(0.0, |sum, source| {
                sum + source.flux().value()
                    / limit.flux(source.host().effective_temperature()).value()
            });
        let d_au = if far >= 1.0 {
            f64::INFINITY
        } else {
            (near / (1.0 - far)).sqrt()
        };
        Metres::from(AstronomicalUnits::new(d_au))
    });
    let extrapolated = orbited
        .iter()
        .chain(companions.iter().map(Illumination::host))
        .any(|light| shining(light) && outside_fit(light.effective_temperature()));
    HabitableZone {
        limits,
        extrapolated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::SolarRadii;
    use crate::units::consts::METRES_PER_AU;

    fn au(m: Metres) -> f64 {
        AstronomicalUnits::from(m).value()
    }

    fn host(l: f64, t: f64) -> HostLight {
        let r = (l / math::powi(t / 5_772.0, 4)).sqrt();
        HostLight::new(
            SolarLuminosities::new(l),
            Kelvin::new(t),
            SolarRadii::new(r),
        )
        .unwrap()
    }

    #[test]
    fn the_sun_s_zones_are_kopparapu_s() {
        let zone = habitable_zone(SolarLuminosities::new(1.0), Kelvin::new(5_772.0));
        let (inner, outer) = zone.conservative();
        assert!((au(inner) - 0.99).abs() < 0.02, "{}", au(inner));
        assert!((au(outer) - 1.69).abs() < 0.02, "{}", au(outer));
        let (inner, outer) = zone.optimistic();
        assert!((au(inner) - 0.75).abs() < 0.02, "{}", au(inner));
        assert!((au(outer) - 1.77).abs() < 0.02, "{}", au(outer));
        // At the fit's own 5,780 K each limit is 1 ÷ √`S_eff☉` exactly.
        let centred = habitable_zone(SolarLuminosities::new(1.0), FIT_CENTRE);
        for limit in HabitableLimit::ALL {
            let expected = 1.0 / limit.coefficients()[0].sqrt();
            assert!(
                (au(centred.limit(limit)) - expected).abs() < 1e-12,
                "{limit:?}"
            );
        }
        assert!((au(centred.runaway_greenhouse()) - 0.981).abs() < 1e-3);
        let order = HabitableLimit::ALL.map(|limit| zone.limit(limit).value());
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    }

    #[test]
    fn the_erratum_s_coefficients_are_used_not_the_original_ones() {
        // The original Table 3's `S_eff☉` were 1.7753, 1.0512, 1.0140, 0.3438 and 0.3179.
        let [venus, runaway, moist, maximum, mars] =
            HabitableLimit::ALL.map(|limit| limit.coefficients()[0]);
        assert!((venus - 1.7763).abs() < 1e-12 && (runaway - 1.0385).abs() < 1e-12);
        assert!((moist - 1.0146).abs() < 1e-12 && (maximum - 0.3507).abs() < 1e-12);
        assert!((mars - 0.3207).abs() < 1e-12);
    }

    #[test]
    fn cool_and_hot_hosts_are_held_to_the_fit_and_flagged() {
        let cool = habitable_zone(SolarLuminosities::new(1e-3), Kelvin::new(2_300.0));
        assert!(cool.extrapolated());
        let edge = habitable_zone(SolarLuminosities::new(1e-3), FIT_COOLEST);
        assert!(!edge.extrapolated());
        assert_eq!(cool.limits, edge.limits);
        assert!(habitable_zone(SolarLuminosities::new(20.0), Kelvin::new(9_000.0)).extrapolated());
        // A dark host has no zone and is not flagged: a black hole's L is zero (ruling 40).
        let dark = habitable_zone(SolarLuminosities::ZERO, Kelvin::ZERO);
        assert!(!dark.extrapolated());
        assert!(
            HabitableLimit::ALL
                .iter()
                .all(|&limit| dark.limit(limit).value().abs() < 1e-300)
        );
    }

    #[test]
    fn a_pair_s_zone_adds_the_stars_and_a_companion_pushes_a_star_s_zone_out() {
        let sun = host(1.0, 5_772.0);
        let single = habitable_zone(SolarLuminosities::new(1.0), Kelvin::new(5_772.0));
        assert_eq!(habitable_zone_of(&[sun], &[]), single);
        // Two Suns orbited together: √2 as far.
        let pair = habitable_zone_of(&[sun, sun], &[]);
        for limit in HabitableLimit::ALL {
            let ratio = pair.limit(limit) / single.limit(limit);
            assert!((ratio - 2.0_f64.sqrt()).abs() < 1e-12, "{limit:?}");
        }
        // A companion Sun 20 au away (e = 0.5) gives a planet of the first 1 ÷ (400 √0.75) of
        // Earth's flux; each limit moves out by 1 ÷ √(1 − F ÷ `S_eff`).
        let companion = Illumination::new(sun, Metres::new(20.0 * METRES_PER_AU), 0.5).unwrap();
        let pushed = habitable_zone_of(&[sun], &[companion]);
        let floor = 1.0 / (400.0 * 0.75_f64.sqrt());
        for limit in HabitableLimit::ALL {
            let s = limit.flux(Kelvin::new(5_772.0)).value();
            let expected = single.limit(limit).value() / (1.0 - floor / s).sqrt();
            assert!((pushed.limit(limit).value() / expected - 1.0).abs() < 1e-12);
        }
        // So bright a companion that it alone exceeds the outer limits puts them beyond every
        // orbit, and a dark one changes nothing.
        let close = Illumination::new(sun, Metres::new(1.5 * METRES_PER_AU), 0.5).unwrap();
        let swamped = habitable_zone_of(&[sun], &[close]);
        assert!(swamped.early_mars().value().is_infinite());
        assert!(swamped.recent_venus().value().is_finite());
        let hole =
            HostLight::new(SolarLuminosities::ZERO, Kelvin::ZERO, SolarRadii::new(4e-5)).unwrap();
        let dark = Illumination::new(hole, Metres::new(1.5 * METRES_PER_AU), 0.5).unwrap();
        assert_eq!(habitable_zone_of(&[sun, hole], &[dark]), single);
    }

    /// A 1 M☉ star at Z = 0.02 from the zero-age main sequence to the tip of the giant branch:
    /// age (Myr), luminosity (L☉) and effective temperature (K), from Hurley, Pols and Tout's
    /// (2000) formulae as this crate's `stellar::sse` evaluates them (the main sequence at tenths of
    /// its 11,003 Myr, the Hertzsprung gap at quarters, the giant branch at fractions of its time
    /// to helium ignition), with the mass held at 1 M☉, since the track integrator of P06.T10.c–e,
    /// which applies the winds, is not merged. `T_eff` = 5,772 K (L ÷ R²)^¼.
    const SOLAR_TRACK: [(f64, f64, f64); 25] = [
        (0.0, 0.697_717, 5_597.3),
        (1_100.3, 0.741_017, 5_623.8),
        (2_200.6, 0.794_834, 5_660.1),
        (3_300.9, 0.861_039, 5_701.9),
        (4_401.3, 0.942_043, 5_745.1),
        (5_501.6, 1.040_971, 5_784.9),
        (6_601.9, 1.162_041, 5_815.8),
        (7_702.2, 1.311_525, 5_829.1),
        (8_802.5, 1.500_394, 5_808.0),
        (9_902.8, 1.751_536, 5_716.1),
        (11_003.1, 2.119_700, 5_465.2),
        (11_147.9, 2.212_375, 5_317.8),
        (11_292.7, 2.309_102, 5_174.4),
        (11_437.5, 2.410_058, 5_034.9),
        (11_582.2, 2.515_427, 4_899.2),
        (11_768.0, 3.548_369, 4_876.9),
        (11_953.7, 5.758_651, 4_831.1),
        (12_139.4, 13.137_600, 4_714.1),
        (12_250.8, 38.639_095, 4_491.2),
        (12_288.0, 85.816_877, 4_282.9),
        (12_310.3, 234.078_897, 3_982.6),
        (12_317.7, 465.264_535, 3_761.1),
        (12_321.4, 835.910_004, 3_567.0),
        (12_324.4, 1_937.075_380, 3_286.8),
        (12_325.1, 2_751.621_903, 3_170.7),
    ];

    #[test]
    fn the_zone_moves_outward_along_a_solar_track_to_the_tip_of_the_giant_branch() {
        let mut last = [0.0; 5];
        for (age, l, t) in SOLAR_TRACK {
            let zone = habitable_zone(SolarLuminosities::new(l), Kelvin::new(t));
            assert!(!zone.extrapolated(), "{age} Myr");
            let now = HabitableLimit::ALL.map(|limit| zone.limit(limit).value());
            for (i, (&d, &before)) in now.iter().zip(&last).enumerate() {
                assert!(
                    d > before,
                    "limit {i} moved in at {age} Myr: {d} after {before}"
                );
            }
            last = now;
        }
        // At the tip, 2,750 L☉ at 3,170 K, the conservative zone is 56.6–108.0 au.
        let tip = habitable_zone(SolarLuminosities::new(2_751.621_903), Kelvin::new(3_170.7));
        assert!(
            (au(tip.moist_greenhouse()) - 56.6).abs() < 0.1,
            "{}",
            au(tip.moist_greenhouse())
        );
        assert!(
            (au(tip.maximum_greenhouse()) - 108.0).abs() < 0.1,
            "{}",
            au(tip.maximum_greenhouse())
        );
    }
}
