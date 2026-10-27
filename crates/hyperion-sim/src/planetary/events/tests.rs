//! Tests of the shared pieces of `planetary::events`, the fixtures the kinds' tests share, and the
//! golden that pins all five kinds on plain inputs (P14.T31.a and T31.b).

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

use super::comets::{self, CometInputs, CometModel};
use super::dust_storms::{self, DustStormInputs, DustStormModel};
use super::eruptions::{self, EruptionInputs, EruptionModel};
use super::impacts::{
    self, ImpactInputs, ImpactModel, bombardment, icy_projectiles, rocky_projectiles,
};
use super::storms::{self, StormInputs, StormModel};
use super::*;
use crate::events::EventsPerSecond;
use crate::id::{BodyId, SystemId};
use crate::orbit::Eccentricity;
use crate::planetary::derive::SnowLineSide;
use crate::planetary::hooks::figures::interior::GENERATED_EARTH_HEAT_FLOW;
use crate::planetary::hooks::figures::tests::solar_sources;
use crate::planetary::hooks::figures::{
    HostOrbit, HostOrbiter, ImpactorBody, ImpactorOrbit, Impactors, impactors,
};
use crate::rng::{ObjectKey, tags};
use crate::time::{Span, UniverseTime};
use crate::units::consts::{EARTH_MASS_KG, GRAVITATIONAL_CONSTANT, METRES_PER_AU};
use crate::units::{
    Degrees, EarthMasses, Gigayears, GravitationalParameter, Kelvin, Kilograms,
    KilogramsPerCubicMetre, MetresPerSecond, MetresPerSecondSquared, SolarLuminosities,
    SolarMasses, WattsPerSquareMetre,
};
use crate::{GENERATOR_VERSION, Seed};

/// The seed of the kinds' tests.
pub(super) const SEED: Seed = Seed::new(0x0e7e_0014_0031_000a);

/// Seconds in a Julian year.
pub(super) const YEAR: i64 = 31_557_600;

/// The test system.
pub(super) fn system() -> SystemId {
    SystemId::from_raw(0x0200_0800_2000_0000).unwrap()
}

/// Body `index` of the test system.
pub(super) fn body(index: u16) -> BodyId {
    BodyId::new(system(), index)
}

/// The instant `years` Julian years from the epoch.
pub(super) fn year(years: i64) -> UniverseTime {
    UniverseTime::new(years * YEAR, 0).unwrap()
}

/// The window from `from` to `to` Julian years.
pub(super) fn years(from: i64, to: i64) -> TimeWindow {
    TimeWindow::new(year(from), year(to)).unwrap()
}

/// A planet of the Solar System as P14.T48.f's tests take it: its semi-major axis (au),
/// eccentricity and inclination (degrees) from JPL's Table 1 (Standish), and its mass (kg) and
/// mean radius (km) from JPL's "Planetary Physical Parameters".
pub(super) type SolarPlanet = (f64, f64, f64, f64, f64);

/// Earth (the Earth–Moon barycentre's orbit), as [`SolarPlanet`].
pub(super) const EARTH: SolarPlanet = (
    1.000_002_61,
    0.016_711_23,
    -0.000_015_31,
    5.972_17e24,
    6_371.008_4,
);

/// Jupiter, as [`SolarPlanet`].
pub(super) const JUPITER: SolarPlanet = (
    5.202_887,
    0.048_386_24,
    1.304_396_95,
    1.898_125e27,
    69_911.0,
);

/// The Moon: semi-major axis (km), GM (km³ s⁻²) and mean radius (km), JPL's, as P14.T48.f's tests
/// take it.
pub(super) const MOON: (f64, f64, f64) = (384_400.0, 4_902.800, 1_737.4);

/// A body of `kg` and mean radius `radius_km` on `orbit` under [`solar_sources`], at 200 K and dry,
/// as P14.T48.f's tests build one.
fn solar_body(orbit: ImpactorOrbit, kg: f64, radius_km: f64) -> Impactors {
    let r = radius_km * 1e3;
    let volume = 4.0 / 3.0 * core::f64::consts::PI * r * r * r;
    impactors(&ImpactorBody {
        orbit,
        radius: Metres::new(r),
        density: KilogramsPerCubicMetre::new(kg / volume),
        surface_gravity: MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * kg / (r * r)),
        surface_temperature: Kelvin::new(200.0),
        water: Kilograms::ZERO,
        sources: solar_sources(),
    })
    .unwrap()
}

/// `planet` as a host orbiter.
fn orbiter((a_au, e, i_deg, kg, _): SolarPlanet) -> HostOrbiter {
    HostOrbiter {
        orbit: HostOrbit {
            semi_major_axis: Metres::new(a_au * METRES_PER_AU),
            eccentricity: e,
            inclination: Radians::from(Degrees::new(i_deg)),
        },
        mass: EarthMasses::new(kg / EARTH_MASS_KG),
        belt: None,
    }
}

/// The impactors of `planet` in the Solar System of P14.T48.f's tests.
pub(super) fn planet_impactors(planet: SolarPlanet) -> Impactors {
    solar_body(ImpactorOrbit::Host(orbiter(planet)), planet.3, planet.4)
}

/// The Moon's impactors in the Solar System of P14.T48.f's tests.
pub(super) fn moon_impactors() -> Impactors {
    let (a_km, gm, radius_km) = MOON;
    let orbit = ImpactorOrbit::Moon {
        parent: orbiter(EARTH),
        semi_major_axis: Metres::new(a_km * 1e3),
        formed: SnowLineSide::Inside,
    };
    solar_body(orbit, gm * 1e9 / GRAVITATIONAL_CONSTANT, radius_km)
}

/// The mean radius of `planet`, m.
pub(super) fn radius_of(planet: SolarPlanet) -> Metres {
    Metres::new(planet.4 * 1e3)
}

/// Jupiter's mean radius, m (JPL).
pub(super) const JUPITER_RADIUS: Metres = Metres::new(69_911e3);

/// A Jupiter-like giant on plain inputs, for the golden: `R_b` 42, struck by comets at 60.2 km/s.
pub(super) fn giant_inputs() -> ImpactInputs {
    ImpactInputs {
        radius: JUPITER_RADIUS,
        rate_over_moon: 42.0,
        velocity: MetresPerSecond::new(60_200.0),
        projectile_density: KilogramsPerCubicMetre::new(500.0),
        size_law: icy_projectiles(),
        system_age: Gigayears::new(4.567),
    }
}

/// An Earth-like planet on plain inputs struck by rocky projectiles in a system `age` Gyr old:
/// `R_b` 1.421 at 19.87 km/s (P14.T48.f's Earth; decision-p14-t31a-events's test 1).
pub(super) fn earth_inputs(age: f64) -> ImpactInputs {
    ImpactInputs {
        radius: Metres::new(6.371e6),
        rate_over_moon: 1.421,
        velocity: MetresPerSecond::new(19_870.0),
        projectile_density: KilogramsPerCubicMetre::new(3_000.0),
        size_law: rocky_projectiles(),
        system_age: Gigayears::new(age),
    }
}

/// The Sun's comets: the halo's rate for the Solar System's 7.5 × 10¹¹ comets, 3 × 2.9 a year per
/// 4 au of perihelion carried to 5 au (P14.T21.d, ruling 84.5), about a host of the Sun's
/// luminosity, mass and radius.
pub(super) fn solar_comets() -> CometInputs {
    CometInputs {
        rate: EventsPerSecond::per_julian_year(3.0 * 2.9 * 5.0 / 4.0),
        luminosity: SolarLuminosities::new(1.0),
        host: GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
        host_radius: Metres::new(6.957e8),
    }
}

/// An Earth's heat flow and radius: the generator's Earth's heat flow, whose outgassing is 1.
pub(super) fn earth_eruptions() -> EruptionInputs {
    EruptionInputs {
        heat_flow: GENERATED_EARTH_HEAT_FLOW,
        radius: Metres::new(6.371e6),
    }
}

/// Saturn's sidereal orbital period, s: 10,755.70 days (NASA's Saturn fact sheet, read for the
/// science check of P14.T31.b), 29.447 Julian years.
pub(super) const SATURN_PERIOD: Span = Span::from_seconds(929_292_480);

/// Mars's sidereal orbital period, s: 686.980 days (NASA's Mars fact sheet).
pub(super) const MARS_PERIOD: Span = Span::from_seconds(59_355_072);

/// Mars's orbital eccentricity, 0.0934 (NASA's Mars fact sheet).
pub(super) const MARS_ECCENTRICITY: f64 = 0.0934;

/// The clock origin the phase kinds' fixtures take, a little after the epoch, so that a phase's
/// cycles are not counted from the epoch itself.
pub(super) fn origin() -> UniverseTime {
    UniverseTime::new(41 * 86_400 + 7_211, 0).unwrap()
}

/// A Saturn-like giant on plain inputs: Saturn's period from [`origin`].
pub(super) fn saturn_storms() -> StormInputs {
    StormInputs {
        orbital_period: SATURN_PERIOD,
        origin: origin(),
    }
}

/// A Mars-like planet on plain inputs: Mars's period, perihelion at [`origin`] and eccentricity
/// `e`.
pub(super) fn mars_dust_storms(e: f64) -> DustStormInputs {
    DustStormInputs {
        orbital_period: MARS_PERIOD,
        perihelion: origin(),
        eccentricity: Eccentricity::new(e).unwrap(),
    }
}

#[test]
fn a_size_law_refuses_its_parts_out_of_range() {
    let km = |x: f64| Metres::new(x * 1e3);
    for slope in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            matches!(
                SizeLaw::new(slope, km(1.0), km(10.0)),
                Err(BuildSizeLawError::Slope(_))
            ),
            "{slope}"
        );
    }
    for reference in [km(0.0), km(-1.0), Metres::new(f64::NAN)] {
        assert!(
            matches!(
                SizeLaw::new(2.0, reference, km(10.0)),
                Err(BuildSizeLawError::Reference(_))
            ),
            "{reference:?}"
        );
    }
    for largest in [
        km(1.0),
        km(0.5),
        Metres::new(f64::INFINITY),
        Metres::new(f64::NAN),
    ] {
        assert!(
            matches!(
                SizeLaw::new(2.0, km(1.0), largest),
                Err(BuildSizeLawError::Largest(_))
            ),
            "{largest:?}"
        );
    }
    assert_eq!(
        SizeLaw::new(0.0, km(1.0), km(10.0))
            .unwrap_err()
            .to_string(),
        "size slope 0 is not finite and positive"
    );
}

#[test]
fn a_size_law_is_one_at_its_reference_and_inverts() {
    let law = SizeLaw::new(2.7, Metres::new(1.5e3), Metres::new(450e3)).unwrap();
    assert_eq!(law.share_above(Metres::new(1.5e3)), 1.0);
    assert_eq!(law.share_above(Metres::new(450e3)), 0.0);
    assert_eq!(law.share_above(Metres::new(1e6)), 0.0);
    assert_eq!(law.diameter_at_share(0.0), Metres::new(450e3));
    let mut last = f64::INFINITY;
    for d in [10.0, 100.0, 999.0, 1_000.0, 1_001.0, 1e4, 1e5, 4.4e5] {
        let share = law.share_above(Metres::new(d));
        assert!(share < last, "the share falls with the diameter, at {d} m");
        last = share;
        let back = law.diameter_at_share(share).value();
        assert!(
            (back / d - 1.0).abs() < 1e-12,
            "{d} m came back as {back} m"
        );
    }
    // Far below the largest, the truncation hardly matters: a pure power law.
    let ratio = law.share_above(Metres::new(150.0));
    assert!((ratio / math::powf(10.0, 2.7) - 1.0).abs() < 1e-6);
}

/// The three laws the kinds use, as decision-p14-t31a-events rules them.
#[test]
fn the_kinds_laws_are_the_ruling_s() {
    let rocky = rocky_projectiles();
    assert_eq!(
        (rocky.slope(), rocky.reference(), rocky.largest()),
        (2.25, Metres::new(1e3), Metres::new(450e3))
    );
    let icy = icy_projectiles();
    assert_eq!(
        (icy.slope(), icy.reference(), icy.largest()),
        (1.92, Metres::new(1.5e3), Metres::new(150e3))
    );
    let nuclei = comets::nuclei();
    assert_eq!(
        (nuclei.slope(), nuclei.reference(), nuclei.largest()),
        (1.92, Metres::new(1e3), Metres::new(150e3))
    );
}

/// The sine of a uniform point's latitude is uniform in [−1, 1] and its longitude in [0, 2π).
#[test]
fn surface_points_are_uniform_over_the_sphere() {
    let mut stream = crate::rng::Stream::open(SEED, tags::SELFTEST_STREAM, ObjectKey::galaxy());
    let n = 4_000;
    let (mut sines, mut longitudes) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for _ in 0..n {
        let point = SurfacePoint::uniform(&mut stream);
        let (lat, lon) = (point.latitude().value(), point.longitude().value());
        assert!((-core::f64::consts::FRAC_PI_2..=core::f64::consts::FRAC_PI_2).contains(&lat));
        assert!((0.0..TAU).contains(&lon));
        sines.push(math::sin(lat));
        longitudes.push(lon);
    }
    assert_p_value(
        "sine of the latitude",
        ks_one_sample(&mut sines, |x| f64::midpoint(x, 1.0).clamp(0.0, 1.0)).p_value,
        ALPHA,
    );
    assert_p_value(
        "longitude",
        ks_one_sample(&mut longitudes, |x| (x / TAU).clamp(0.0, 1.0)).p_value,
        ALPHA,
    );
}

/// Pinned, on plain inputs only (no generated body, so nothing the generator's bodies do moves it):
/// the kinds' constants and the bombardment factor at five ages; a Jupiter-like giant's impacts
/// over a year and a young Earth-like planet's over a tenth of a year, whose floor the bins' limit
/// raises; an Earth's eruptions over 60 days and those running at the epoch, with an Io's
/// and a lava world's rates; and the Sun's comets over a year, each with its orbit, nucleus and
/// listing, and its position at perihelion and a year later. P14.T31.b's sections follow, after
/// T31.a's, which stay as they were: the phase kinds' constants and their season and noise
/// figures; a Saturn-like giant's storms over 300 years and the one running mid-way through the
/// first; and a Mars-like planet's dust storms over 30 years and the one running mid-way through
/// the first.
#[test]
fn events_on_bodies_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    pin_constants(&mut w);
    pin_impacts(&mut w);
    pin_eruptions(&mut w);
    pin_comets(&mut w);
    pin_storms(&mut w);
    golden!("planetary/events", w.as_str());
}

/// The kinds' constants and the bombardment factor at five ages.
fn pin_constants(w: &mut GoldenWriter) {
    w.line("");
    w.line("# constants");
    w.f64("impacts: fixed floor (J)", impacts::FIXED_FLOOR_JOULES);
    w.f64(
        "impacts: lunar flux (per km2 per yr)",
        impacts::LUNAR_FLUX_PER_KM2_YEAR,
    );
    w.f64("impacts: rocky slope", impacts::ROCKY_IMPACTOR_SLOPE);
    w.f64(
        "impacts: icy reference (m)",
        impacts::ICY_REFERENCE_DIAMETER.value(),
    );
    w.f64(
        "eruptions: Earth's onsets (per yr)",
        eruptions::EARTH_ERUPTIONS_PER_YEAR,
    );
    w.f64(
        "eruptions: median duration (s)",
        eruptions::MEDIAN_DURATION_SECONDS,
    );
    w.f64(
        "eruptions: duration log sigma",
        eruptions::DURATION_LOG_SIGMA,
    );
    w.f64("comets: nucleus slope", comets::NUCLEUS_SIZE_SLOPE);
    w.f64(
        "comets: largest nucleus (m)",
        comets::LARGEST_NUCLEUS.value(),
    );
    w.f64("comets: shortest period (yr)", comets::MIN_PERIOD_YEARS);
    w.f64("comets: most listed at once", comets::MAX_LISTED_COMETS);
    for age in [0.0, 0.1, 1.0, 4.567, 10.0] {
        w.f64(
            &format!("bombardment at {age} Gyr"),
            bombardment(Gigayears::new(age)),
        );
    }
}

/// A Jupiter-like giant's impacts over a year and a young Earth-like planet's over a tenth of one.
fn pin_impacts(w: &mut GoldenWriter) {
    let pin = |w: &mut GoldenWriter, name: &str, model: &ImpactModel, window: TimeWindow| {
        w.line("");
        w.line(&format!("# impacts on {name}"));
        w.f64("  reference rate (per s)", model.reference_rate().value());
        w.f64("  floor diameter (m)", model.floor_diameter().value());
        w.f64("  floor energy (J)", model.floor_joules());
        w.f64("  listed rate (per s)", model.listed_rate().value());
        let mut listed = Vec::new();
        impacts::impacts(model, window, &mut listed);
        for impact in listed {
            w.line(&format!("{} at {}", impact.id(), impact.time()));
            w.f64("  diameter (m)", impact.diameter().value());
            w.f64("  energy (J)", impact.energy_joules());
            w.f64("  latitude (rad)", impact.site().latitude().value());
            w.f64("  longitude (rad)", impact.site().longitude().value());
        }
    };
    let giant = ImpactModel::new(SEED, body(0x0105), &giant_inputs()).unwrap();
    pin(w, "a Jupiter-like giant", &giant, years(0, 1));
    let young = ImpactModel::new(SEED, body(0x0103), &earth_inputs(0.1)).unwrap();
    let tenth = TimeWindow::new(
        UniverseTime::EPOCH,
        UniverseTime::new(YEAR / 10, 0).unwrap(),
    )
    .unwrap();
    pin(w, "a young Earth-like planet", &young, tenth);
}

/// An Earth's eruptions over 60 days and those running at the epoch, and an Io's and a lava
/// world's rates.
fn pin_eruptions(w: &mut GoldenWriter) {
    w.line("");
    w.line("# eruptions on an Earth");
    let earth = EruptionModel::new(SEED, body(0x0103), &earth_eruptions()).unwrap();
    w.f64("  listed rate (per s)", earth.listed_rate().value());
    let days = TimeWindow::new(
        UniverseTime::EPOCH,
        UniverseTime::new(60 * 86_400, 0).unwrap(),
    )
    .unwrap();
    let mut listed = Vec::new();
    eruptions::eruptions(&earth, days, &mut listed);
    for eruption in listed {
        w.line(&format!(
            "{} from {} to {} ({})",
            eruption.id(),
            eruption.onset(),
            eruption.end(),
            eruption.duration()
        ));
    }
    let mut running = Vec::new();
    eruptions::eruptions_active_at(&earth, UniverseTime::EPOCH, &mut running);
    w.line("  running at the epoch:");
    for eruption in running {
        w.line(&format!("  {} from {}", eruption.id(), eruption.onset()));
    }
    let io = EruptionInputs {
        heat_flow: WattsPerSquareMetre::new(2.24),
        radius: Metres::new(1_821.6e3),
    };
    let io = EruptionModel::new(SEED, body(0x0151), &io).unwrap();
    w.f64("an Io's rate (per s)", io.listed_rate().value());
    let lava = EruptionInputs {
        heat_flow: WattsPerSquareMetre::new(100.0),
        radius: Metres::new(6.371e6),
    };
    let lava = EruptionModel::new(SEED, body(0x0102), &lava).unwrap();
    w.f64("a lava world's law rate (per s)", lava.law_rate().value());
    w.f64(
        "a lava world's listed rate (per s)",
        lava.listed_rate().value(),
    );
}

/// The Sun's comets over a year, each with its orbit, nucleus and listing, and its position at
/// perihelion and a year later.
fn pin_comets(w: &mut GoldenWriter) {
    w.line("");
    w.line("# the Sun's comets");
    let sun = CometModel::new(SEED, system(), &solar_comets()).unwrap();
    w.f64("  perihelion limit (m)", sun.perihelion_limit().value());
    w.f64("  listing radius (m)", sun.listing_radius().value());
    w.f64(
        "  smallest semi-major axis (m)",
        sun.min_semi_major_axis().value(),
    );
    w.line(&format!("  reach {}", sun.reach()));
    w.f64("  floor diameter (m)", sun.floor_diameter().value());
    w.f64("  listed rate (per s)", sun.listed_rate().value());
    let mut listed = Vec::new();
    comets::comets(&sun, years(0, 1), &mut listed);
    for comet in listed {
        w.line(&format!("{} at {}", comet.id(), comet.perihelion_time()));
        w.f64("  perihelion (m)", comet.perihelion().value());
        w.f64("  eccentricity", comet.eccentricity());
        w.f64(
            "  inclination (rad)",
            comet.orientation().inclination().value(),
        );
        w.f64("  node (rad)", comet.orientation().ascending_node().value());
        w.f64(
            "  periapsis (rad)",
            comet.orientation().argument_of_periapsis().value(),
        );
        w.f64("  nucleus (m)", comet.nucleus_diameter().value());
        w.line(&format!(
            "  listed from {} to {}",
            comet.listed().start(),
            comet.listed().end()
        ));
        w.line(&format!("  plunges {}", comet.plunges()));
        let later = comet
            .perihelion_time()
            .checked_add(Span::from_seconds(YEAR))
            .unwrap();
        for (label, t) in [
            ("at perihelion", comet.perihelion_time()),
            ("a year later", later),
        ] {
            let [x, y, z] = comet.position_at(t).metres();
            w.f64(&format!("  x {label} (m)"), x);
            w.f64(&format!("  y {label} (m)"), y);
            w.f64(&format!("  z {label} (m)"), z);
        }
    }
}

/// P14.T31.b's phase kinds: their constants and season and noise figures, a Saturn-like giant's
/// storms over 300 years and a Mars-like planet's dust storms over 30, each with the one running
/// mid-way through its first.
fn pin_storms(w: &mut GoldenWriter) {
    pin_phase_constants(w);
    pin_saturn_storms(w);
    pin_mars_dust_storms(w);
}

/// The phase kinds' constants, and the dust storms' season and noise at five eccentricities.
fn pin_phase_constants(w: &mut GoldenWriter) {
    w.line("");
    w.line("# the phase kinds' constants");
    w.f64(
        "storms: noise amplitude (cycles)",
        storms::NOISE_AMPLITUDE_CYCLES,
    );
    w.line(&format!(
        "storms: noise lattice (cycles) {}",
        storms::NOISE_LATTICE_CYCLES
    ));
    w.f64("storms: shortest (share)", storms::MIN_DURATION_SHARE);
    w.f64("storms: longest (share)", storms::MAX_DURATION_SHARE);
    for (band, weight) in ["equatorial", "mid-latitude", "polar"]
        .into_iter()
        .zip(storms::BAND_WEIGHTS)
    {
        w.f64(&format!("storms: {band} weight"), weight);
    }
    w.f64("dust storms: skip", dust_storms::SKIP_PROBABILITY);
    w.f64(
        "dust storms: season half-width (rad)",
        dust_storms::SEASON_HALF_WIDTH.value(),
    );
    w.f64("dust storms: season margin", dust_storms::SEASON_MARGIN);
    w.f64(
        "dust storms: largest noise amplitude (cycles)",
        dust_storms::MAX_NOISE_AMPLITUDE_CYCLES,
    );
    w.line(&format!(
        "dust storms: noise octaves {}, lattice (cycles) {}",
        dust_storms::NOISE_OCTAVES,
        dust_storms::NOISE_LATTICE_CYCLES
    ));
    w.f64(
        "dust storms: shortest (share)",
        dust_storms::MIN_DURATION_SHARE,
    );
    w.f64(
        "dust storms: longest (share)",
        dust_storms::MAX_DURATION_SHARE,
    );
    for e in [0.0, 0.016_7, MARS_ECCENTRICITY, 0.3, 0.9] {
        let e = Eccentricity::new(e).unwrap();
        w.f64(
            &format!(
                "dust storms: season half-width at e = {} (cycles)",
                e.value()
            ),
            dust_storms::season_half_width_cycles(e),
        );
        w.f64(
            &format!("dust storms: noise amplitude at e = {} (cycles)", e.value()),
            dust_storms::noise_amplitude_cycles(e),
        );
        w.f64(
            &format!("dust storms: perihelion contrast at e = {}", e.value()),
            dust_storms::perihelion_contrast(e),
        );
    }
}

/// A Saturn-like giant's storms over 300 years, and the one running mid-way through the first.
fn pin_saturn_storms(w: &mut GoldenWriter) {
    w.line("");
    w.line("# storms on a Saturn-like giant");
    let saturn = StormModel::new(SEED, body(0x0106), &saturn_storms()).unwrap();
    w.line(&format!("  octaves {}", saturn.noise().octaves()));
    w.f64(
        "  noise bound (cycles)",
        saturn.noise().noise_bound_cycles(),
    );
    let mut listed = Vec::new();
    storms::storms(&saturn, years(0, 300), &mut listed);
    for storm in &listed {
        w.line(&format!(
            "{} cycle {} from {} to {} ({}), {:?}",
            storm.id(),
            storm.cycle(),
            storm.onset(),
            storm.end(),
            storm.duration(),
            storm.band()
        ));
    }
    let midway = mid_way(listed[0].onset(), listed[0].duration());
    let mut running = Vec::new();
    storms::storms_active_at(&saturn, midway, &mut running);
    w.line(&format!("  running at {midway}:"));
    for storm in running {
        w.line(&format!("  {} from {}", storm.id(), storm.onset()));
    }
}

/// A Mars-like planet's dust storms over 30 years, and the one running mid-way through the first.
fn pin_mars_dust_storms(w: &mut GoldenWriter) {
    w.line("");
    w.line("# dust storms on a Mars-like planet");
    let mars =
        DustStormModel::new(SEED, body(0x0104), &mars_dust_storms(MARS_ECCENTRICITY)).unwrap();
    w.f64(
        "  noise amplitude (cycles)",
        mars.noise().amplitude_cycles(),
    );
    w.f64("  noise bound (cycles)", mars.noise().noise_bound_cycles());
    let mut listed = Vec::new();
    dust_storms::dust_storms(&mars, years(0, 30), &mut listed);
    for storm in &listed {
        w.line(&format!(
            "{} cycle {} from {} to {} ({})",
            storm.id(),
            storm.cycle(),
            storm.onset(),
            storm.end(),
            storm.duration()
        ));
        w.f64(
            "  true anomaly at onset (rad)",
            storm.onset_true_anomaly().value(),
        );
    }
    let midway = mid_way(listed[0].onset(), listed[0].duration());
    let mut running = Vec::new();
    dust_storms::dust_storms_active_at(&mars, midway, &mut running);
    w.line(&format!("  running at {midway}:"));
    for storm in running {
        w.line(&format!("  {} from {}", storm.id(), storm.onset()));
    }
}

/// The instant half of `duration` after `onset`, to the whole second.
fn mid_way(onset: UniverseTime, duration: Span) -> UniverseTime {
    onset
        .checked_add(Span::from_seconds(duration.seconds() / 2))
        .unwrap()
}
