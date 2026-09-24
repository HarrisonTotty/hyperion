//! Properties of plan 14's derivation over many bodies (P14.T16.b).
//!
//! T8's placer is not built, so the bodies are placed by hand about the present Sun, in the
//! zero-age Sun's disc, each formed where it orbits. The property T16.b names first, that no
//! planet is hotter than its star, needs plan 06's `StarModel` for its hosts and waits for it.

use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::planetary::derive::rocky::{
    HIGH_CORE_MASS_FRACTION, LOW_CORE_MASS_FRACTION, MEDIAN_CORE_MASS_FRACTION,
    core_mass_fraction_rank,
};
use hyperion_sim::planetary::derive::{
    BodyHosts, HostLight, PlacedBody, PlanetClass, SnowLineSide, derive_body,
};
use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, DiscProfile, Truncation};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::UnitUniform;
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_MASS_KG};
use hyperion_sim::units::{
    EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres, Radians,
    SolarLuminosities, SolarMasses, SolarRadii, Years,
};

/// The zero-age Sun's disc, from plan 06's zero-age luminosity and radius at solar composition.
fn solar_disc() -> DiscProfile {
    let (mass, composition) = (SolarMasses::new(1.0), Composition::SOLAR);
    let coeffs = ZCoeffs::new(composition.z_fit());
    let host = DiscHost::new(
        mass,
        composition.fe_h(),
        zams::luminosity(mass, &coeffs),
        zams::radius(mass, &coeffs),
    )
    .unwrap();
    *disc::derive(
        &host,
        Megayears::new(3.0),
        &DiscDraws::MEDIAN,
        Truncation::NONE,
    )
    .profile()
    .unwrap()
}

/// A body of `mass` M⊕ on a circular orbit of `a_au` about the Sun, formed there, at `rank`.
fn placed(mass: f64, a_au: f64, rank: f64) -> PlacedBody {
    let a = Metres::new(a_au * METRES_PER_AU);
    let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap();
    let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
    let orbit = KeplerElements::from_semi_major_axis(
        a,
        mu,
        Eccentricity::CIRCULAR,
        orientation,
        Radians::ZERO,
    )
    .unwrap();
    PlacedBody::new(
        EarthMasses::new(mass),
        orbit,
        a,
        UnitUniform::new(rank).unwrap(),
    )
    .unwrap()
}

/// The value at quantile `ppm` parts per million of the sorted `values`, the nearest rank.
fn quantile(values: &[f64], ppm: usize) -> f64 {
    values[((values.len() - 1) * ppm + 500_000) / 1_000_000]
}

#[test]
fn rocky_core_mass_fractions_follow_plotnykov_and_valencia() {
    // Ruling 53: below 2 M⊕ inside the snow line, the rocky outcomes' core mass fractions are the
    // observed spread of Plotnykov and Valencia (2020, abstract), 0.24 +0.33 −0.18, where Chen and
    // Kipping's radii alone, confined by ruling 47.1, gave a median of 0.42.
    let disc = solar_disc();
    let sun = [HostLight::new(
        SolarLuminosities::new(1.0),
        Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K),
        SolarRadii::new(1.0),
    )
    .unwrap()];
    let hosts =
        BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[]).unwrap();
    let mut rocky = Vec::new();
    let mut bodies = 0_u32;
    for mass in [0.1, 0.3, 0.5, 0.8, 1.0, 1.2, 1.5, 1.7, 1.9] {
        for a_au in [0.1, 0.5, 1.0, 1.5] {
            for i in 1..1_000_u32 {
                let body = placed(mass, a_au, f64::from(i) / 1_000.0);
                let derived = derive_body(
                    &body,
                    &hosts,
                    &disc,
                    Years::new(4.57e9),
                    UniverseTime::EPOCH,
                )
                .unwrap();
                assert_eq!(derived.formed(), SnowLineSide::Inside);
                bodies += 1;
                let f = derived.fractions();
                if derived.class() == PlanetClass::Rocky && f.envelope() <= 0.0 && f.water() <= 0.0
                {
                    rocky.push(derived.core().core_mass_fraction());
                }
            }
        }
    }
    rocky.sort_by(f64::total_cmp);
    let count = u32::try_from(rocky.len()).unwrap();
    assert!(
        count * 10 > bodies * 7,
        "{count} of {bodies} bodies are rocky"
    );
    for (ppm, expected) in [
        (158_655, LOW_CORE_MASS_FRACTION),
        (500_000, MEDIAN_CORE_MASS_FRACTION),
        (841_345, HIGH_CORE_MASS_FRACTION),
    ] {
        let got = quantile(&rocky, ppm);
        assert!((got - expected).abs() < 0.01, "quantile {ppm} ppm: {got}");
    }
    let over_half = rocky.iter().filter(|&&cmf| cmf > 0.5).count();
    let share = f64::from(u32::try_from(over_half).unwrap()) / f64::from(count);
    assert!((share - 0.21).abs() < 0.01, "over half iron: {share}");
    // The whole distribution, not three points of it: the largest gap between the sample's
    // cumulative distribution and the source's.
    let n = f64::from(count);
    let gap = rocky
        .iter()
        .enumerate()
        .map(|(k, &cmf)| {
            let below = f64::from(u32::try_from(k).unwrap()) / n;
            (core_mass_fraction_rank(cmf) - below).abs()
        })
        .fold(0.0, f64::max);
    assert!(gap < 0.01, "Kolmogorov distance {gap}");
}
