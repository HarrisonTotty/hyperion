//! The six synthetic worlds: four bodies' fields built directly from closed forms, a flat field and
//! a single crater (plan R09, T2).
//!
//! Each body's radius, figure and gravity are near the real one's (volumetric mean radii and
//! flattenings from NASA's planetary fact sheets, but Earth's flattening WGS 84's, and Ceres's
//! radius and flattening from Dawn's shape, Ermakov et al. 2017); everything else is
//! illustrative: plates, basins and climates placed by hand where the real ones are, sized to
//! give each field the character of its body (an Earth's two crusts, belts and trenches; a Mars's
//! dichotomy, Tharsis and dry valleys; a Moon's basins and maria; a Ceres's ice-rich craters), not
//! plan 14's values or the coarse pass's.

use hyperion_base::math;
use hyperion_base::units::{
    Gigayears, Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, Pascals, PerSquareKilometre, Radians, SquareMetres,
    consts::SECONDS_PER_JULIAN_YEAR,
};

use super::{CellSite, ClimateSample, FieldBuilder, PlateSpec, Routing, arc, dot, scale};
use crate::craters::{CraterParams, CraterParamsParts, Screening};
use crate::cube::unit_dir;
use crate::field::{BodyRef, BoundaryKind, ClimateModelKind, CoarseField, Crust};
use crate::spheroid::Spheroid;
use crate::synth::BandSpectrum;

/// One of the synthetic worlds [`synthetic_field`] builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyntheticWorld {
    /// An Earth-sized ocean world of ten plates (four continental), with belts, trenches, arcs
    /// and ridges at their boundaries, a seasonal climate, ice caps and rivers, and three coarse
    /// craters: level 8.
    EarthLike,
    /// A Mars-sized stagnant lid with a northern lowland, Tharsis and its shield, three great
    /// basins and twenty other coarse craters, a dry seasonal climate and drainage from a past
    /// wet epoch: level 7.
    MarsLike,
    /// A Moon-sized airless lid with highland and mare crust, five great basins and forty other
    /// coarse craters: level 6.
    MoonLike,
    /// A Ceres-sized ice-rich lid with three great craters and twenty-five others: level 5.
    CeresLike,
    /// A flat, dry, airless field at the Moon's radius with a one-month year, no relief finer than
    /// its cells and a crater density of zero, so that no crater, coarse or small, is on it: level
    /// 6.
    Flat,
    /// The flat field with one crater of 300 km on the edge between faces 0 and 2, its reach on
    /// both faces, and no other, small craters included: level 6.
    OneCrater,
}

impl SyntheticWorld {
    /// Every synthetic world.
    pub const ALL: &'static [Self] = &[
        Self::EarthLike,
        Self::MarsLike,
        Self::MoonLike,
        Self::CeresLike,
        Self::Flat,
        Self::OneCrater,
    ];
}

/// The field of `world`, the same to the bit every time.
#[must_use]
pub fn synthetic_field(world: SyntheticWorld) -> CoarseField {
    match world {
        SyntheticWorld::EarthLike => earth_like(),
        SyntheticWorld::MarsLike => mars_like(),
        SyntheticWorld::MoonLike => moon_like(),
        SyntheticWorld::CeresLike => ceres_like(),
        SyntheticWorld::Flat => flat(),
        SyntheticWorld::OneCrater => one_crater(),
    }
}

/// The raw system ID of the synthetic worlds' system, which no galaxy places.
const SYSTEM: u64 = 0x5e5e_0000_0000_0000;

/// Volumetric mean radii, metres (NASA's planetary fact sheets; Ceres from Dawn, about 470 km).
const EARTH_RADIUS_M: f64 = 6.371e6;
const MARS_RADIUS_M: f64 = 3.3895e6;
const MOON_RADIUS_M: f64 = 1.7374e6;
const CERES_RADIUS_M: f64 = 4.697e5;

/// Julian years a second, to turn rates in millimetres of water a year (kg m⁻² a⁻¹) into SI.
const PER_YEAR: f64 = 1.0 / SECONDS_PER_JULIAN_YEAR;

/// The unit vector at latitude `lat` and east longitude `lon`, degrees, in the body-fixed frame
/// with +z the north pole and +x the prime meridian.
fn lat_lon(lat: f64, lon: f64) -> [f64; 3] {
    let (sin_lat, cos_lat) = math::sin_cos(lat.to_radians());
    let (sin_lon, cos_lon) = math::sin_cos(lon.to_radians());
    [cos_lat * cos_lon, cos_lat * sin_lon, sin_lat]
}

/// The latitude of the unit vector `dir`, radians.
fn latitude(dir: [f64; 3]) -> f64 {
    math::asin(dir[2].clamp(-1.0, 1.0))
}

/// Point `index` of `count` on a golden-angle spiral, evenly spread over the sphere.
fn spiral(index: u32, count: u32) -> [f64; 3] {
    let height = 1.0 - (2.0 * f64::from(index) + 1.0) / f64::from(count);
    let ring = (1.0 - height * height).sqrt();
    let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let (sine, cosine) = math::sin_cos(golden * f64::from(index));
    [ring * cosine, ring * sine, height]
}

/// `n` craters on a spiral, diameters spread geometrically from `smallest` to `largest` metres in
/// a shuffled order, ages spread from `youngest` to `oldest` Gyr, older ones more degraded.
fn spiral_craters(
    mut builder: FieldBuilder,
    n: u32,
    (smallest, largest): (f64, f64),
    (youngest, oldest): (f64, f64),
) -> FieldBuilder {
    let golden_fraction = (5.0_f64.sqrt() - 1.0) / 2.0;
    for k in 0..n {
        let rank = f64::from((k * 7) % n) / f64::from(n - 1);
        let diameter = smallest * math::powf(largest / smallest, rank);
        let spread = (f64::from(k) * golden_fraction).fract();
        let age = youngest + (oldest - youngest) * spread;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "200 × a fraction in [0, 1) rounds to 0 to 200"
        )]
        let degradation = (200.0 * spread).round() as u8;
        builder = builder.crater(
            spiral(k, n),
            Metres::new(diameter),
            Gigayears::new(age),
            degradation,
        );
    }
    builder
}

/// A sum of sinusoids of the projection of `dir` on fixed axes: smooth relief on the sphere.
fn undulation(dir: [f64; 3], terms: &[(f64, f64, f64, f64, f64)]) -> f64 {
    terms
        .iter()
        .map(|&(amplitude, lat, lon, frequency, phase)| {
            amplitude * math::sin(frequency * dot(dir, lat_lon(lat, lon)) + phase)
        })
        .sum()
}

/// exp(−(x ÷ width)²).
fn bell(x: f64, width: f64) -> f64 {
    math::exp(-(x / width) * (x / width))
}

/// The phase of the year at the middle of month `m` (from 0), 2π (m + ½) ÷ 12, radians.
fn season(m: u8) -> f64 {
    core::f64::consts::TAU * (f64::from(m) + 0.5) / 12.0
}

/// A three-cell wind pattern: trades below 30°, westerlies to 60°, polar easterlies beyond, each
/// blowing equatorward or poleward as on Earth, at `speeds` metres a second, the same each
/// quarter.
fn three_cell_wind(lat: f64, speeds: [f64; 3]) -> [(MetresPerSecond, Radians); 4] {
    let north = lat >= 0.0;
    let degrees = lat.abs().to_degrees();
    let (speed, towards) = if degrees < 30.0 {
        (speeds[0], if north { 225.0 } else { 315.0 })
    } else if degrees < 60.0 {
        (speeds[1], if north { 45.0 } else { 135.0 })
    } else {
        (speeds[2], if north { 225.0 } else { 315.0 })
    };
    let wind = (
        MetresPerSecond::new(speed),
        Radians::new(f64::to_radians(towards)),
    );
    [wind; 4]
}

/// The ten plates of the Earth-like world: seed latitude and longitude, crust, Euler pole latitude
/// and longitude, and rotation rate in degrees per million years.
const EARTH_PLATES: [(f64, f64, Crust, f64, f64, f64); 10] = [
    (45.0, 20.0, Crust::Continental, 50.0, -100.0, 0.25),
    (45.0, -100.0, Crust::Continental, -20.0, -80.0, 0.20),
    (-15.0, -60.0, Crust::Continental, -25.0, -130.0, 0.20),
    (5.0, 20.0, Crust::Continental, 50.0, -20.0, 0.25),
    (-25.0, 135.0, Crust::Oceanic, 15.0, 40.0, 0.60),
    (-80.0, 0.0, Crust::Oceanic, 60.0, -120.0, 0.20),
    (-5.0, -150.0, Crust::Oceanic, -60.0, 100.0, 0.90),
    (10.0, 75.0, Crust::Oceanic, 20.0, 30.0, 0.50),
    (0.0, -30.0, Crust::Oceanic, 70.0, 0.0, 0.30),
    (35.0, 160.0, Crust::Oceanic, 40.0, -60.0, 0.60),
];

/// The Earth-like world's annual mean sea-level temperature at latitude `lat`, kelvin.
fn earth_temperature(lat: f64) -> f64 {
    let s = math::sin(lat);
    301.0 - 48.0 * s * s
}

fn earth_elevation(site: &CellSite) -> Metres {
    let base = match site.crust {
        Crust::Continental => 400.0,
        Crust::Oceanic => -4_300.0,
        Crust::Lid | Crust::Province => 0.0,
    };
    let d = site.boundary_distance.map_or(f64::INFINITY, Metres::value);
    let feature = match site.boundary {
        BoundaryKind::Collision => 4_000.0 * bell(d, 200e3),
        BoundaryKind::Subduction if d < 0.0 => -3_000.0 * bell(d, 60e3),
        BoundaryKind::Subduction => 2_000.0 * bell(d - 150e3, 60e3),
        BoundaryKind::Divergent if site.crust == Crust::Oceanic => {
            2_000.0 * math::exp(-d.abs() / 400e3)
        }
        BoundaryKind::Divergent => -1_000.0 * bell(d, 100e3),
        BoundaryKind::Transform | BoundaryKind::Absent => 0.0,
    };
    let relief = undulation(
        site.dir,
        &[
            (600.0, 30.0, 40.0, 3.0, 0.4),
            (350.0, -50.0, 170.0, 7.0, 1.3),
            (200.0, 10.0, -80.0, 13.0, 2.1),
        ],
    );
    Metres::new(base + feature + relief)
}

fn earth_climate(dir: [f64; 3]) -> ClimateSample {
    let lat = latitude(dir);
    let s = math::sin(lat);
    let degrees = lat.to_degrees();
    let mut sample = ClimateSample::still(Kelvin::new(earth_temperature(lat)));
    for m in 0..12_u8 {
        // Coldest in the year's first month in the north, the south opposite.
        sample.month_anomaly_k[usize::from(m)] = -16.0 * s * math::cos(season(m));
        // A rain band that follows the sun north of the equator in the year's middle, storm
        // tracks near 48°, and dry belts between.
        let itcz = 6.0 - 8.0 * math::cos(season(m));
        let mm =
            2_200.0 * bell(degrees - itcz, 9.0) + 900.0 * bell(degrees.abs() - 48.0, 12.0) + 30.0;
        sample.month_precipitation_kg_m2_s[usize::from(m)] = mm * PER_YEAR;
    }
    sample.wind = three_cell_wind(lat, [6.0, 8.0, 4.0]);
    sample
}

fn earth_like() -> CoarseField {
    let plates = EARTH_PLATES
        .iter()
        .map(
            |&(lat, lon, crust, pole_lat, pole_lon, degrees_per_myr)| PlateSpec {
                seed: lat_lon(lat, lon),
                crust,
                rotation_rad_per_myr: scale(
                    lat_lon(pole_lat, pole_lon),
                    degrees_per_myr.to_radians(),
                ),
            },
        )
        .collect();
    let radius = Metres::new(EARTH_RADIUS_M);
    FieldBuilder::new(radius)
        .body(BodyRef::new(SYSTEM, 1))
        .figure(
            Spheroid::from_volumetric(EARTH_RADIUS_M, 1.0 / 298.257_223_563)
                .expect("Earth's figure is valid"),
        )
        .sea(Metres::ZERO)
        .lapse_rate(0.0065)
        .spectrum(spectrum(3.6e6))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(1e-4),
                screening: Screening::Atmosphere {
                    column_mass: KilogramsPerSquareMetre::new(101_325.0 / 9.81),
                    projectile_density: KilogramsPerCubicMetre::new(3_000.0),
                },
                gravity: MetresPerSecondSquared::new(9.81),
                k_target: 1.0,
                impact_velocity: None,
            })
            .expect("the Earth-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::EnergyBalance)
        .months(12)
        .surface_age(Gigayears::new(2.5))
        .surface_pressure(Pascals::new(101_325.0))
        .plates(plates)
        .elevation(earth_elevation)
        .ice(|site, elevation| {
            let t = earth_temperature(latitude(site.dir));
            // Sea ice poleward of about 66°, land ice where the ground's annual mean is below 263 K.
            let cold = if elevation.value() < 0.0 {
                t < 261.0
            } else {
                t - 0.0065 * elevation.value() < 263.0
            };
            if cold { 1.0 } else { 0.0 }
        })
        .climate(earth_climate)
        .routing(Routing::SteepestDescent)
        .crater(
            lat_lon(21.4, -89.5),
            Metres::new(180e3),
            Gigayears::new(0.066),
            10,
        )
        .crater(
            lat_lon(-27.0, 27.5),
            Metres::new(160e3),
            Gigayears::new(2.02),
            200,
        )
        .crater(
            lat_lon(46.6, -81.2),
            Metres::new(130e3),
            Gigayears::new(1.85),
            190,
        )
        .build()
}

/// The spectrum V(l) = `v1` l^−1.9.
fn spectrum(v1: f64) -> BandSpectrum {
    BandSpectrum::new(BandSpectrum::DEFAULT_EXPONENT, SquareMetres::new(v1))
        .expect("a synthetic spectrum is valid")
}

/// Mars's Tharsis rise, Olympus Mons and Elysium.
fn tharsis() -> [f64; 3] {
    lat_lon(2.0, -112.0)
}

fn olympus() -> [f64; 3] {
    lat_lon(18.65, -133.8)
}

fn elysium() -> [f64; 3] {
    lat_lon(25.0, 147.0)
}

fn mars_elevation(site: &CellSite) -> Metres {
    let p = site.dir;
    // The dichotomy: a lowland about a pole near 60° N, 180° E.
    let north = dot(p, lat_lon(60.0, 180.0));
    let dichotomy = 1_500.0 - 4_500.0 * 0.5 * (1.0 + math::tanh((north - 0.35) / 0.12));
    let rise = 6_000.0 * bell(arc(p, tharsis()), 0.45) + 18_000.0 * bell(arc(p, olympus()), 0.08);
    let relief = undulation(
        p,
        &[
            (400.0, -20.0, 60.0, 5.0, 0.7),
            (250.0, 40.0, -30.0, 11.0, 1.9),
        ],
    );
    Metres::new(dichotomy + rise + relief)
}

fn mars_climate(dir: [f64; 3]) -> ClimateSample {
    let lat = latitude(dir);
    let s = math::sin(lat);
    let mut sample = ClimateSample::still(Kelvin::new(215.0 - 35.0 * s * s));
    for m in 0..12_u8 {
        sample.month_anomaly_k[usize::from(m)] = -30.0 * s * math::cos(season(m));
    }
    sample.wind = three_cell_wind(lat, [4.0, 6.0, 3.0]);
    sample
}

fn mars_like() -> CoarseField {
    let builder = FieldBuilder::new(Metres::new(MARS_RADIUS_M))
        .body(BodyRef::new(SYSTEM, 2))
        .figure(Spheroid::from_volumetric(MARS_RADIUS_M, 0.005_89).expect("Mars's figure is valid"))
        .lapse_rate(0.0025)
        .spectrum(spectrum(4.8e6))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.005),
                screening: Screening::Atmosphere {
                    column_mass: KilogramsPerSquareMetre::new(610.0 / 3.71),
                    projectile_density: KilogramsPerCubicMetre::new(3_000.0),
                },
                gravity: MetresPerSecondSquared::new(3.71),
                k_target: 1.0,
                impact_velocity: None,
            })
            .expect("the Mars-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::EnergyBalance)
        .months(12)
        .surface_age(Gigayears::new(3.7))
        .surface_pressure(Pascals::new(610.0))
        .crust(|p| {
            if arc(p, tharsis()) < 0.5 || arc(p, elysium()) < 0.2 {
                Crust::Province
            } else {
                Crust::Lid
            }
        })
        .elevation(mars_elevation)
        .ice(|site, _| {
            if latitude(site.dir).abs() > 80_f64.to_radians() {
                1.0
            } else {
                0.0
            }
        })
        .climate(mars_climate)
        .routing(Routing::SteepestDescent)
        .crater(
            lat_lon(-42.4, 70.5),
            Metres::new(2_300e3),
            Gigayears::new(4.0),
            120,
        )
        .crater(
            lat_lon(-49.7, -43.0),
            Metres::new(1_800e3),
            Gigayears::new(3.9),
            110,
        )
        .crater(
            lat_lon(12.9, 87.0),
            Metres::new(1_500e3),
            Gigayears::new(3.9),
            100,
        );
    spiral_craters(builder, 20, (100e3, 500e3), (3.5, 4.0)).build()
}

/// The Moon's great basins and its broadest mare.
fn imbrium() -> [f64; 3] {
    lat_lon(32.8, -15.6)
}

fn serenitatis() -> [f64; 3] {
    lat_lon(27.0, 18.4)
}

fn crisium() -> [f64; 3] {
    lat_lon(17.0, 59.1)
}

fn procellarum() -> [f64; 3] {
    lat_lon(18.4, -57.4)
}

fn moon_like() -> CoarseField {
    let builder = FieldBuilder::new(Metres::new(MOON_RADIUS_M))
        .body(BodyRef::new(SYSTEM, 3))
        .figure(
            Spheroid::from_volumetric(MOON_RADIUS_M, 0.0012).expect("the Moon's figure is valid"),
        )
        .spectrum(spectrum(3.3e6))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.05),
                screening: Screening::None,
                gravity: MetresPerSecondSquared::new(1.62),
                k_target: 1.0,
                impact_velocity: None,
            })
            .expect("the Moon-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::RadiativeEquilibrium)
        .months(12)
        .surface_age(Gigayears::new(4.4))
        .crust(|p| {
            let mare = arc(p, imbrium()) < 0.25
                || arc(p, serenitatis()) < 0.15
                || arc(p, crisium()) < 0.12
                || arc(p, procellarum()) < 0.35;
            if mare { Crust::Province } else { Crust::Lid }
        })
        .elevation(|site| {
            let highlands = 1_800.0 * dot(site.dir, lat_lon(0.0, 180.0));
            let relief = undulation(
                site.dir,
                &[
                    (300.0, 15.0, -40.0, 6.0, 0.2),
                    (200.0, -35.0, 100.0, 13.0, 1.1),
                ],
            );
            Metres::new(highlands + relief)
        })
        .climate(|dir| {
            let lat = latitude(dir);
            let mut sample = ClimateSample::still(Kelvin::new(100.0 + 150.0 * math::cos(lat)));
            for m in 0..12_u8 {
                sample.month_anomaly_k[usize::from(m)] =
                    -0.5 * math::sin(lat) * math::cos(season(m));
            }
            sample
        })
        .crater(
            lat_lon(-53.0, -169.0),
            Metres::new(2_500e3),
            Gigayears::new(4.3),
            180,
        )
        .crater(imbrium(), Metres::new(1_145e3), Gigayears::new(3.9), 60)
        .crater(serenitatis(), Metres::new(740e3), Gigayears::new(3.9), 90)
        .crater(crisium(), Metres::new(1_060e3), Gigayears::new(3.9), 80)
        .crater(
            lat_lon(-19.4, -92.8),
            Metres::new(930e3),
            Gigayears::new(3.8),
            40,
        );
    spiral_craters(builder, 40, (100e3, 400e3), (3.0, 4.3)).build()
}

fn ceres_like() -> CoarseField {
    let builder = FieldBuilder::new(Metres::new(CERES_RADIUS_M))
        .body(BodyRef::new(SYSTEM, 4))
        .figure(Spheroid::from_volumetric(CERES_RADIUS_M, 0.075).expect("Ceres's figure is valid"))
        .spectrum(spectrum(2.0e6))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.003),
                screening: Screening::None,
                gravity: MetresPerSecondSquared::new(0.28),
                k_target: 0.12,
                impact_velocity: None,
            })
            .expect("the Ceres-like crater contract is valid"),
        )
        .climate_model(ClimateModelKind::RadiativeEquilibrium)
        .months(12)
        .surface_age(Gigayears::new(3.0))
        .elevation(|site| {
            Metres::new(undulation(
                site.dir,
                &[
                    (2_000.0, 20.0, 10.0, 4.0, 0.3),
                    (1_200.0, -60.0, -120.0, 9.0, 1.7),
                ],
            ))
        })
        .climate(|dir| {
            let lat = latitude(dir);
            let mut sample = ClimateSample::still(Kelvin::new(110.0 + 55.0 * math::cos(lat)));
            for m in 0..12_u8 {
                sample.month_anomaly_k[usize::from(m)] =
                    -3.0 * math::sin(lat) * math::cos(season(m));
            }
            sample
        })
        .crater(
            lat_lon(-10.8, 123.9),
            Metres::new(280e3),
            Gigayears::new(1.0),
            40,
        )
        .crater(
            lat_lon(-42.6, -67.5),
            Metres::new(260e3),
            Gigayears::new(1.5),
            60,
        )
        .crater(
            lat_lon(-46.3, -110.8),
            Metres::new(170e3),
            Gigayears::new(0.5),
            20,
        );
    spiral_craters(builder, 25, (55e3, 150e3), (1.0, 4.0)).build()
}

fn flat_builder(body_index: u16) -> FieldBuilder {
    FieldBuilder::new(Metres::new(MOON_RADIUS_M)).body(BodyRef::new(SYSTEM, body_index))
}

fn flat() -> CoarseField {
    flat_builder(5).build()
}

fn one_crater() -> CoarseField {
    // On the edge between faces 0 and 2 (x = z), which face 0 owns by the lowest-index rule.
    flat_builder(6)
        .crater(
            unit_dir([1.0, 0.3, 1.0]),
            Metres::new(300e3),
            Gigayears::new(3.9),
            0,
        )
        .build()
}
