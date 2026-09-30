//! Planetary nebulae: the shell a star ejects at the end of the asymptotic giant branch, lit while
//! the post-AGB core is hot enough to ionise it and not yet dispersed (plan 06, P06.T16.b).
//!
//! A star has a nebula when its track has the post-AGB crossing of P06.T16.a (it lost its envelope
//! on the AGB and leaves a white dwarf), while its central star, crossing or already a white dwarf,
//! is above 25,000 K, and until its edge reaches 0.9 pc (ruling 124.5). The shell's spectroscopic
//! expansion speed v is drawn once in 20–40 km/s; its edge moves at 1.3 v (Jacob, Schönberner and
//! Steffen 2013, A&A 558, A78, §5.1: `F_NII` = 1.3), so a nebula is visible for 0.9 pc ÷ 1.3 v =
//! 16,900–33,900 years, a mean of about 23,500, Jacob et al.'s 21,000 ± 5,000 and the brainstorm's
//! "some 20,000". 0.9 pc is the completeness radius every count of the Galaxy's nebulae uses (Moe
//! and De Marco 2006, ApJ 650, 916; Jacob et al. 2013): a maximum. The median is much smaller, 0.19
//! pc over Frew, Parker and Bojičić's (2016, MNRAS 455, 1459) catalogue and 0.26 pc within 2 kpc,
//! the brainstorm's "a light-year or two across".
//!
//! A slow ("lazy") core reaches 25,000 K only after its shell has dispersed, so it has no nebula
//! without any rule of its own (design note 8): with P06.T16.a's crossing times (ruling 124.3)
//! that is a core below 0.527–0.534 M☉ (for the fastest and slowest shells), the plan's "about
//! 0.53".

use crate::math;
use crate::stellar::sse::Track;
use crate::units::consts::{
    METRES_PER_LIGHT_YEAR, METRES_PER_PARSEC, METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND,
    SECONDS_PER_JULIAN_YEAR,
};
use crate::units::{Kelvin, KilometresPerSecond, LightYears, SolarMasses, Years};

/// The radius at which a nebula has dispersed, 0.9 pc (ruling 124.5), in light-years: 2.94.
///
/// The completeness radius of Moe and De Marco (2006), Frew (2008) and Jacob et al. (2013): a
/// maximum, not a typical size (see the module).
pub const MAX_RADIUS: LightYears = LightYears::new(0.9 * METRES_PER_PARSEC / METRES_PER_LIGHT_YEAR);

/// How much faster the shell's edge moves than its spectroscopic expansion speed: 1.3, Jacob et
/// al.'s (2013, §5.1) `F_NII`, a weighted mean of 1.2–1.6.
pub const EDGE_SPEED_FACTOR: f64 = 1.3;

/// The central star's temperature from which it ionises its shell, 25,000 K (P06.T16.b).
pub const IONISING_TEMPERATURE: Kelvin = Kelvin::new(25_000.0);

/// The slowest shell, km/s (P06.T16.b).
pub const MIN_EXPANSION_SPEED: KilometresPerSecond = KilometresPerSecond::new(20.0);

/// The fastest shell, km/s (P06.T16.b).
pub const MAX_EXPANSION_SPEED: KilometresPerSecond = KilometresPerSecond::new(40.0);

/// Knot intervals of the thermally pulsing AGB whose mass loss makes the shell: its last sixteen of
/// thirty-one, which the knots' clustering puts where the superwind strips the envelope's last
/// seventh or so. A generator default: it gives shells of about 0.04 M☉ at 1 M☉, 0.25 at 2 and
/// 1 M☉ at 7.
const SHELL_INTERVALS: usize = 16;

/// The share of the shell that is ionised: 0.3, a generator default. Frew and Parker (2010, §2)
/// give ionised masses of 0.005–3 M☉, with a median of 0.5 √ε M☉ for a filling factor ε, and no
/// share of the ejected envelope; young, ionisation-bounded nebulae ionise less of theirs.
const IONISED_SHARE: f64 = 0.3;

/// Light-years a shell expanding at 1 km/s covers in a Julian year.
const LIGHT_YEARS_PER_KM_S_YEAR: f64 =
    METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND * SECONDS_PER_JULIAN_YEAR / METRES_PER_LIGHT_YEAR;

/// The highest excitation class the scale is read to.
const MAX_EXCITATION_CLASS: u8 = 12;

/// A planetary nebula at one age of its central star.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanetaryNebula {
    radius: LightYears,
    expansion_speed: KilometresPerSecond,
    age: Years,
    ionised_mass: SolarMasses,
    excitation_class: u8,
}

impl PlanetaryNebula {
    /// The shell's radius, light-years: positive, and under [`MAX_RADIUS`]; its edge moves at
    /// [`EDGE_SPEED_FACTOR`] times the expansion speed.
    #[must_use]
    pub const fn radius(&self) -> LightYears {
        self.radius
    }

    /// The speed at which it expands as a spectrograph measures it, km/s: 20–40.
    #[must_use]
    pub const fn expansion_speed(&self) -> KilometresPerSecond {
        self.expansion_speed
    }

    /// The time since it was ejected, years: under its visibility time, [`MAX_RADIUS`] over the
    /// edge's speed.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// The mass of its ionised gas, M☉: 30% of what the star lost over the last sixteen knot
    /// intervals of its thermally pulsing AGB (a generator default).
    #[must_use]
    pub const fn ionised_mass(&self) -> SolarMasses {
        self.ionised_mass
    }

    /// Its excitation class, 0–12, from its central star's temperature on Reid and Parker's scale.
    ///
    /// Reid and Parker (2010, PASA 27, 187, eq. 5) relate their excitation class E to the central
    /// star's temperature for optically thick nebulae: log₁₀ `T_eff` = 4.439 + 0.1174 E − 0.00172 E²,
    /// which gives 27,500 K at E = 0, 96,000 K at 5 and 398,000 K at 12. The class is that
    /// relation's inverse at the central star's temperature, rounded, and held to 0–12.
    #[must_use]
    pub const fn excitation_class(&self) -> u8 {
        self.excitation_class
    }
}

/// The planetary nebula of `track`'s star at `age` (years since the onset of collapse), if it has
/// one: when the star lost its envelope on the AGB (its track has the post-AGB crossing), its
/// central star is above 25,000 K at `age`, and the time since the ejection is under the
/// visibility time [`MAX_RADIUS`] ÷ 1.3 v, v being 20–40 km/s by the star's
/// [`nebula`](crate::stellar::draws::StarDraws::nebula) rank (P06.T16.b).
///
/// # Panics
///
/// In debug builds, if `age` is negative or not finite, or beyond [`Track::built_until`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::nebula::planetary_nebula;
/// use hyperion_sim::stellar::sse::Track;
/// use hyperion_sim::stellar::{Composition, Phase};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A 2 M☉ star's white dwarf is born inside a nebula that has dispersed 30,000 years later.
/// let track = Track::full(SolarMasses::new(2.0), &Composition::SOLAR, &StarDraws::median());
/// let death = track.death().ok_or("a full track reaches the death")?.age().value();
/// assert_eq!(track.state_at(Years::new(death + 1.0)).phase(), Phase::CarbonOxygenWhiteDwarf);
/// let young = planetary_nebula(&track, Years::new(death + 1.0)).ok_or("a nebula")?;
/// assert!(young.radius().value() < 2.61);
/// assert!(planetary_nebula(&track, Years::new(death + 5e4)).is_none());
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn planetary_nebula(track: &Track, age: Years) -> Option<PlanetaryNebula> {
    let ejected = track.post_agb_start()?;
    let since = age.value() - ejected.value();
    if since < 0.0 {
        return None;
    }
    let speed = MIN_EXPANSION_SPEED.value()
        + (MAX_EXPANSION_SPEED.value() - MIN_EXPANSION_SPEED.value()) * track.nebula_rank().value();
    let radius = EDGE_SPEED_FACTOR * speed * since * LIGHT_YEARS_PER_KM_S_YEAR;
    if radius >= MAX_RADIUS.value() {
        return None;
    }
    let central = track.state_at(age).effective_temperature().value();
    if central <= IONISING_TEMPERATURE.value() {
        return None;
    }
    let shell = track.late_superwind_mass(SHELL_INTERVALS)?.value();
    Some(PlanetaryNebula {
        radius: LightYears::new(radius),
        expansion_speed: KilometresPerSecond::new(speed),
        age: Years::new(since),
        ionised_mass: SolarMasses::new(IONISED_SHARE * shell),
        excitation_class: excitation_class(central),
    })
}

/// Reid and Parker's (2010, eq. 5) excitation class at a central star of `temperature` K, rounded
/// and held to 0–12 (see [`PlanetaryNebula::excitation_class`]).
#[must_use]
fn excitation_class(temperature: f64) -> u8 {
    const A: f64 = 4.439;
    const B: f64 = 0.1174;
    const C: f64 = -0.00172;
    let y = math::log10(temperature) - A;
    // C E² + B E − y = 0, on the branch that rises from E = 0.
    let discriminant = B * B + 4.0 * C * y;
    let class = if discriminant <= 0.0 {
        f64::from(MAX_EXCITATION_CLASS)
    } else {
        (-B + discriminant.sqrt()) / (2.0 * C)
    };
    let rounded = class.round().clamp(0.0, f64::from(MAX_EXCITATION_CLASS));
    // Held to 0–12, so the conversion is exact.
    (0..=MAX_EXCITATION_CLASS)
        .find(|&k| f64::from(k) >= rounded)
        .unwrap_or(MAX_EXCITATION_CLASS)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::stellar::draws::{StarDraws, StarDrawsParts, UnitUniform};
    use crate::stellar::{Composition, Phase};

    fn draws(nebula: f64) -> StarDraws {
        StarDraws::from_parts(StarDrawsParts {
            nebula: UnitUniform::new(nebula).expect("a rank in (0, 1)"),
            ..StarDrawsParts::MEDIAN
        })
    }

    /// The years over which `track`'s star shows a nebula, at 25-year steps from the ejection,
    /// with every nebula checked against the module's rules.
    fn visible_years(track: &Track) -> f64 {
        let Some(ejected) = track.post_agb_start() else {
            return 0.0;
        };
        let mut visible = 0.0;
        for k in 0..2_400 {
            let age = ejected.value() + 25.0 * f64::from(k);
            let Some(nebula) = planetary_nebula(track, Years::new(age)) else {
                continue;
            };
            visible += 25.0;
            assert!(nebula.radius().value() < MAX_RADIUS.value());
            assert!(
                (MAX_RADIUS.value() - 2.935).abs() < 0.001,
                "0.9 pc is 2.94 ly"
            );
            assert!((20.0..=40.0).contains(&nebula.expansion_speed().value()));
            assert!(nebula.ionised_mass().value() > 0.0);
            assert!(nebula.excitation_class() <= 12);
            let central = track.state_at(Years::new(age));
            assert!(central.effective_temperature().value() > IONISING_TEMPERATURE.value());
            assert!(matches!(
                central.phase(),
                Phase::PostAgb | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
            ));
        }
        visible
    }

    /// A nebula is never larger than 0.9 pc (2.94 ly, ruling 124.5), lasts no longer than its
    /// dispersal time, and appears only once its central star ionises it.
    #[test]
    fn a_nebula_is_lit_only_while_hot_and_not_yet_dispersed() {
        for m in [1.5, 2.0, 3.0, 5.0] {
            for rank in [0.01, 0.5, 0.99] {
                let track = Track::full(SolarMasses::new(m), &Composition::SOLAR, &draws(rank));
                let years = visible_years(&track);
                let speed = 20.0 + 20.0 * rank;
                let dispersal =
                    MAX_RADIUS.value() / (EDGE_SPEED_FACTOR * speed * LIGHT_YEARS_PER_KM_S_YEAR);
                assert!(
                    years > 0.0 && years <= dispersal + 25.0,
                    "{m} M☉: {years} yr"
                );
            }
        }
        // A star that dies without the AGB's envelope loss has none.
        let track = Track::full(SolarMasses::new(20.0), &Composition::SOLAR, &draws(0.5));
        let death = track.death().expect("dies").age();
        assert!(planetary_nebula(&track, death).is_none());
    }

    /// Lazy cores show no nebula (ruling 124.3): a core reaches 25,000 K only after the fastest
    /// shell has dispersed (16,900 years) below 0.534 M☉, and after the slowest (33,900 years)
    /// below 0.527 M☉, the plan's "about 0.53". HYPERION's own 1 M☉ star at Z = 0.02 leaves a
    /// 0.512 M☉ core and shows none (a finding, rulings 92 and 99), where MB16's 0.528 does.
    #[test]
    fn lazy_cores_reach_the_ionising_temperature_too_late() {
        let visibility = |speed: KilometresPerSecond| {
            MAX_RADIUS.value() / (EDGE_SPEED_FACTOR * speed.value() * LIGHT_YEARS_PER_KM_S_YEAR)
        };
        let (slowest, fastest) = (
            visibility(MIN_EXPANSION_SPEED),
            visibility(MAX_EXPANSION_SPEED),
        );
        assert!((16_800.0..17_000.0).contains(&fastest) && (33_800.0..34_000.0).contains(&slowest));
        let ionising = crate::stellar::sse::post_agb_ionising_years;
        assert!(ionising(0.526) > slowest && ionising(0.533) > fastest);
        assert!(ionising(0.528) < slowest && ionising(0.535) < fastest);
        let sun = Track::full(SolarMasses::new(1.0), &Composition::SOLAR, &draws(0.5));
        assert!(visible_years(&sun) <= 0.0, "a lazy core shows a nebula");
    }

    /// The excitation class rises with the central star's temperature through Reid and Parker's
    /// (2010) scale: 0 at 27,500 K, 5 near 96,000 K and 12 near 398,000 K.
    #[test]
    fn the_excitation_class_follows_reid_and_parker() {
        assert_eq!(excitation_class(26_000.0), 0);
        assert_eq!(excitation_class(96_000.0), 5);
        assert_eq!(excitation_class(398_000.0), 12);
        assert_eq!(excitation_class(2.0e6), 12);
        let mut last = 0;
        for k in 0..400 {
            let class = excitation_class(25_000.0 * math::exp10(f64::from(k) / 200.0));
            assert!(class >= last);
            last = class;
        }
    }

    /// The Galaxy's planetary nebulae at today's star formation, a floor (ruling 124.4), and each
    /// nebula visible for 16–26 kyr on average (Jacob et al. 2013's 21 ± 5 kyr).
    ///
    /// The floor is 3,500 (ruling 127.2): 0.296 deaths a year × 0.70 of them showing a nebula ×
    /// 19.6 kyr, 4,050 as measured. 72 of the 240 stars, those of about 0.9–1.1 M☉ whose cores are
    /// the lighter ones rulings 92 and 99 accepted, are lazy under ruling 124.3's crossing and show
    /// none: a finding for the initial–final mass relation's check, not tuned here. The share of
    /// AGB deaths showing a nebula is held at 0.65–0.75 (0.70 measured); T31's birth-rate check,
    /// 0.6–0.95 of 1.2–3.0 a year, is the real test.
    ///
    /// The death rate is that of a constant star-formation rate of 1.65 M☉ per year (Licquia and
    /// Newman 2015, ApJ 806, 96) through Kroupa's (2001) initial mass function over 0.08–150 M☉:
    /// 0.179 stars of 0.9–8 M☉ per solar mass formed, 0.30 deaths a year, where the Galaxy has about
    /// 2.4 (Moe and De Marco 2006: 2.4 ± 0.5 post-AGB white dwarfs a year). So this count is the
    /// right product of too low an input, and the Galaxy's own, 5,000–50,000 (20,000–40,000
    /// expected; Jacoby et al. 2010, "typically ∼25 000"), waits for T31's sampler over the real
    /// star-formation history, with its birth-rate checks. Only stars that show a nebula count
    /// towards the mean visible time; the lazy cores are the count's.
    #[test]
    fn the_milky_way_holds_a_floor_of_planetary_nebulae() {
        const DEATHS_PER_YEAR: f64 = 1.65 * 0.179;
        let mut rng = Lcg::new(0x706e_6562);
        let (mut total, mut shown, n) = (0.0, 0_u32, 240_u32);
        for _ in 0..n {
            let (lo, hi) = (math::powf(0.9, -1.3), math::powf(8.0, -1.3));
            let m = math::powf(lo + rng.next_f64() * (hi - lo), -1.0 / 1.3);
            let rank = rng.next_f64().clamp(1e-6, 1.0 - 1e-6);
            let track = Track::full(SolarMasses::new(m), &Composition::SOLAR, &draws(rank));
            let years = visible_years(&track);
            total += years;
            shown += u32::from(years > 0.0);
        }
        let expected = DEATHS_PER_YEAR * total / f64::from(n);
        let mean = total / f64::from(shown.max(1));
        let share = f64::from(shown) / f64::from(n);
        assert!(
            expected >= 3_500.0
                && (16_000.0..26_000.0).contains(&mean)
                && (0.65..0.75).contains(&share),
            "{expected} planetary nebulae, {shown} of {n} shown, a mean visible time of {mean} years"
        );
    }
}
