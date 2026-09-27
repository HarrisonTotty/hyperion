//! What a feature shines as: its emission class (plan 09, P09.T4.d; Design note 17).
//!
//! A nursery is an **H II region** while its expected number of living stars above 15 M☉ is at
//! least one half, and a **reflection nebula** while it holds gas and no such stars; walking band
//! E of every young feature to check would cost more than the display is worth, so the test is on
//! the expectation (Design note 17). A cloud is a **dark cloud**. The remnant shell and the pulsar
//! wind nebula belong to the supernova remnants of phase 4 (P09.T16), which no catalogue feature of
//! phase 1 is.

use crate::galaxy::Galaxy;
use crate::stellar::Composition;
use crate::stellar::sse::{MAX_INITIAL_MASS, turn_off_mass};
use crate::units::{HeliumExcess, SolarMasses, Years};

use super::catalogue::{FeatureMarks, FeatureRecord};
use super::kinds::nursery::{NurseryMarks, NurseryStage};

/// The least mass of a star that ionises an H II region, M☉ (Design note 17).
pub const IONISING_STAR_MASS: SolarMasses = SolarMasses::new(15.0);

/// The expected number of ionising stars at which a nursery is an H II region (Design note 17).
pub const IONISING_STARS_THRESHOLD: f64 = 0.5;

/// The upper limit of the mass function, M☉.
const MASS_FUNCTION_MAX: f64 = 150.0;

/// A feature's emission class (brainstorm, "Large features").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EmissionClass {
    /// Gas ionised by hot young stars.
    HiiRegion,
    /// Dust lit by stars too cool to ionise it.
    ReflectionNebula,
    /// Gas and dust seen against what lies behind.
    DarkCloud,
    /// A supernova's expanding shell (phase 4).
    RemnantShell,
    /// A pulsar's wind nebula inside its shell (phase 4).
    PulsarWindNebula,
}

impl EmissionClass {
    /// The emission class of `feature` `t` years after the epoch, or `None` for a feature that
    /// shines as nothing but its stars (an old cluster, a gas-free association without ionising
    /// stars) or that has dissolved.
    ///
    /// It takes the galaxy for its mass function, which sets the expected ionising stars.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, FeatureMarks};
    /// use hyperion_sim::galaxy::features::emission::EmissionClass;
    /// use hyperion_sim::id::FeatureCell;
    /// use hyperion_sim::units::Years;
    ///
    /// let galaxy = Galaxy::new(Seed::new(9));
    /// let cell = FeatureCatalogue::cell(&galaxy, FeatureCell::new([0, 6, 0])?);
    /// for feature in cell.features() {
    ///     if let FeatureMarks::Cloud(_) = feature.marks() {
    ///         assert_eq!(EmissionClass::of(&galaxy, feature, Years::ZERO), Some(EmissionClass::DarkCloud));
    ///     }
    /// }
    /// # Ok::<(), hyperion_sim::id::BuildSystemIdError>(())
    /// ```
    #[must_use]
    pub fn of(galaxy: &Galaxy, feature: &FeatureRecord, t: Years) -> Option<Self> {
        match feature.marks() {
            FeatureMarks::Cloud(_) => Some(Self::DarkCloud),
            FeatureMarks::OpenCluster(_) => None,
            FeatureMarks::Nursery(marks) => nursery_class(galaxy, marks, marks.age_at_epoch() + t),
        }
    }
}

/// The class of a nursery at `age`.
fn nursery_class(galaxy: &Galaxy, marks: &NurseryMarks, age: Years) -> Option<EmissionClass> {
    let stage = NurseryStage::at(marks, age);
    if matches!(stage, NurseryStage::Unborn | NurseryStage::Dissolved) {
        return None;
    }
    if expected_ionising_stars(galaxy, marks, age) >= IONISING_STARS_THRESHOLD {
        Some(EmissionClass::HiiRegion)
    } else if marks.gas_mass_at(age).value() > 0.0 {
        Some(EmissionClass::ReflectionNebula)
    } else {
        None
    }
}

/// The expected living stars above 15 M☉ in a nursery at `age`: its birth mass times the mass
/// function's share of primaries between 15 M☉ and the turn-off, per solar mass formed. Before the
/// heaviest track's star dies the turn-off is held at 100 M☉, plan 06's top, and the stars up to
/// 150 M☉ count too.
#[must_use]
pub fn expected_ionising_stars(galaxy: &Galaxy, marks: &NurseryMarks, age: Years) -> f64 {
    let composition = Composition::from_fe_h(marks.fe_h(), HeliumExcess::ZERO);
    let turn_off = turn_off_mass(Years::new(age.value().max(0.0)), &composition).value();
    let hi = if turn_off >= MAX_INITIAL_MASS.value() {
        MASS_FUNCTION_MAX
    } else {
        turn_off
    };
    let lo = IONISING_STAR_MASS.value();
    if hi <= lo {
        return 0.0;
    }
    let f = galaxy.mass_function();
    let share = f.integral(lo, hi) / f.integral(0.0, MASS_FUNCTION_MAX);
    marks.mass().value() * share / galaxy.mean_formed_mass().value()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::consts::YEARS_PER_MEGAYEAR;
    use crate::galaxy::features::kinds::nursery::BUBBLE_INTERIOR_MEDIAN;
    use crate::units::{Dex, KilometresPerSecond};

    fn nursery(mass: f64) -> NurseryMarks {
        NurseryMarks {
            age_at_epoch: Years::ZERO,
            mass: SolarMasses::new(mass),
            bound: false,
            embedded_duration: Years::new(4.0 * YEARS_PER_MEGAYEAR),
            dissolution_age: Years::new(60.0 * YEARS_PER_MEGAYEAR),
            expansion_speed: KilometresPerSecond::new(3.0),
            age_spread: Years::ZERO,
            efficiency: 0.2,
            fe_h: Dex::new(0.0),
            bubble_interior: BUBBLE_INTERIOR_MEDIAN,
            half_mass_radius: crate::units::LightYears::new(8.0),
            concentration: 1.0,
        }
    }

    #[test]
    fn classes_follow_the_table() {
        let galaxy = Galaxy::new(Seed::new(0x0900_0004));
        let myr = |value: f64| Years::new(value * YEARS_PER_MEGAYEAR);
        // (mass, age, class)
        let table = [
            (1e4, 1.0, Some(EmissionClass::HiiRegion)),
            (1e4, 3.0, Some(EmissionClass::HiiRegion)),
            (1e4, 20.0, None),
            (1e2, 1.0, Some(EmissionClass::ReflectionNebula)),
            (1e2, 20.0, None),
            (1e4, 200.0, None),
        ];
        for (mass, age, expected) in table {
            let marks = nursery(mass);
            assert_eq!(
                nursery_class(&galaxy, &marks, myr(age)),
                expected,
                "{mass} M☉ at {age} Myr"
            );
        }
    }

    #[test]
    fn ionising_stars_scale_with_mass_and_vanish_past_their_lifetimes() {
        let galaxy = Galaxy::new(Seed::new(0x0900_0004));
        let young = expected_ionising_stars(&galaxy, &nursery(1e3), Years::new(1e6));
        let twice = expected_ionising_stars(&galaxy, &nursery(2e3), Years::new(1e6));
        assert!((twice / young - 2.0).abs() < 1e-12);
        // About one star above 15 M☉ per 400–600 M☉ formed.
        assert!((1.5..3.5).contains(&young), "{young}");
        let old = expected_ionising_stars(&galaxy, &nursery(1e3), Years::new(3e7));
        assert!(old.abs() < 1e-15, "{old}");
    }
}
