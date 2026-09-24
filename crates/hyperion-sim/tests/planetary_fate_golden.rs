//! Golden values of plan 14's fate transform (P14.T28.a–c): formation draws and ages, the
//! engulfment reach, and the histories of bodies on young, evolved and exploding hosts.
//!
//! They pin the arithmetic, not only the draws: a moved word, a changed scan point or a reordered
//! sum of host masses changes a line here, which is a generator-version change. CI checks the same
//! file on 64-bit Arm and on wasm32.

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{BodyId, Layer, SystemId};
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::planetary::fate::{BodyFate, BodyState, FateBody, FateHost};
use hyperion_sim::planetary::hosts::evolved::{Circularisation, engulfment_reach};
use hyperion_sim::planetary::hosts::young::{Formation, FormationDraws};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::system::StarModel;
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::{
    AstronomicalUnits, EarthMasses, GravitationalParameter, KilogramsPerCubicMetre, Megayears,
    Metres, Radians, SolarMasses, Years,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

const SEED: u64 = 0x0014_0028_0000_5eed;

fn planet(system: u32, slot: u16) -> BodyId {
    let cell = GenCell::new(CellSize::Ly8, [5, -3, 2]).expect("a cell of the grid");
    let system = SystemId::from_parts(Layer::A, cell, system).expect("a system of the cell");
    BodyId::new(system, slot << 8)
}

fn star(mass: f64, age: f64) -> StarModel {
    StarModel::new(
        SolarMasses::new(mass),
        Composition::SOLAR,
        StarDraws::median(),
        Years::new(age),
    )
    .expect("a valid star")
}

/// A body of `mass` M⊕ drawn as planet `slot` of system 1, on an orbit of `a_au` and `e` about
/// `host_mass` M☉, circularising with e-folding time `tidal_yr`.
fn body(slot: u16, a_au: f64, e: f64, mass: f64, host_mass: f64, tidal_yr: f64) -> FateBody {
    let mass = EarthMasses::new(mass);
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::from(AstronomicalUnits::new(a_au)),
        GravitationalParameter::from_solar_masses(SolarMasses::new(host_mass)),
        Eccentricity::new(e).expect("valid"),
        Orientation::new(Radians::new(0.5), Radians::new(2.5), Radians::new(4.0)).expect("valid"),
        Radians::new(1.5),
    )
    .expect("a valid orbit");
    let formation = Formation::draw(Seed::new(SEED), planet(1, slot), mass, Megayears::new(2.2))
        .expect("valid");
    let density = KilogramsPerCubicMetre::new(if mass.value() > 30.0 {
        1_326.0
    } else {
        5_513.0
    });
    FateBody::new(formation, orbit, mass, density)
        .expect("valid")
        .with_circularisation(Circularisation::new(Years::new(tidal_yr)).expect("positive"))
}

fn write_time(w: &mut GoldenWriter, label: &str, t: Option<UniverseTime>) {
    match t {
        Some(t) => w.line(&format!(
            "{label} = {} s {} ns",
            t.seconds(),
            t.subsec_nanos()
        )),
        None => w.line(&format!("{label} = none")),
    }
}

fn write_fate(w: &mut GoldenWriter, label: &str, fate: &BodyFate<'_>, times: &[UniverseTime]) {
    write_time(w, &format!("{label}.formed"), fate.formed_at());
    match fate.ending() {
        Some(BodyState::Destroyed { cause, at }) => {
            w.line(&format!("{label}.ending = destroyed {cause:?}"));
            write_time(w, &format!("{label}.ending.at"), Some(at));
        }
        Some(BodyState::Unbound { at }) => {
            w.line(&format!("{label}.ending = unbound"));
            write_time(w, &format!("{label}.ending.at"), Some(at));
        }
        Some(state @ (BodyState::NotYetFormed | BodyState::Present)) => {
            panic!("an ending is never {state:?}");
        }
        None => w.line(&format!("{label}.ending = none")),
    }
    for (i, t) in times.iter().enumerate() {
        let now = fate.at(*t);
        let key = format!("{label}.at[{i}]");
        w.line(&format!("{key}.state = {:?}", now.state()));
        write_time(w, &format!("{key}.valid_until"), now.valid_until());
        if let Some(orbit) = now.orbit() {
            w.f64(&format!("{key}.a_m"), orbit.semi_major_axis().value());
            w.f64(&format!("{key}.e"), orbit.eccentricity().value());
            w.f64(
                &format!("{key}.mu"),
                orbit.gravitational_parameter().value(),
            );
            w.f64(&format!("{key}.m0"), orbit.mean_anomaly_at_epoch().value());
        }
    }
}

#[test]
fn fate_goldens() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());

    w.line("# formation draws and ages, planets of 1 M_earth and 1 M_jup, disc of 2.2 Myr");
    for slot in 1..=4 {
        let id = planet(1, slot);
        let draws = FormationDraws::for_body(Seed::new(SEED), id);
        w.f64(&format!("draws[{slot}].giant"), draws.giant.value());
        w.f64(
            &format!("draws[{slot}].magma_ocean"),
            draws.magma_ocean.value(),
        );
        for (kind, mass) in [("small", 1.0), ("giant", 317.8)] {
            let f = Formation::from_draws(EarthMasses::new(mass), Megayears::new(2.2), &draws)
                .expect("valid");
            w.f64(
                &format!("formation[{slot}].{kind}.formed_myr"),
                f.formed_at().value(),
            );
            w.f64(
                &format!("formation[{slot}].{kind}.molten_myr"),
                f.molten_until().value(),
            );
        }
    }

    w.line("# engulfment reach");
    for mass in [0.5, 1.0, 3.0, 17.1, 95.2, 317.8, 1_000.0] {
        w.f64(
            &format!("reach[{mass}]"),
            engulfment_reach(EarthMasses::new(mass)),
        );
    }

    let past = |years: i64| UniverseTime::from_julian_years(years).expect("on the clock");
    let times = [
        past(-12_000_000_000),
        past(-1_100_000_000),
        past(-1_040_000_000),
        UniverseTime::EPOCH,
        ClockWindow::END,
    ];

    w.line("# a 1 M_sun host 13.5 Gyr old, a white dwarf since 12.46 Gyr");
    let sun = star(1.0, 13.5e9);
    let host = FateHost::star(&sun);
    for (i, (a, e, mass, tidal)) in [
        (0.05, 0.2, 317.8, 1e8),
        (1.0, 0.017, 1.0, 1e12),
        (1.25, 0.0, 1.0, 1e12),
        (5.2, 0.048, 317.8, 1e15),
        (30.0, 0.01, 17.1, 1e15),
    ]
    .into_iter()
    .enumerate()
    {
        let slot = u16::try_from(i + 1).expect("a small slot");
        let b = body(slot, a, e, mass, 1.0, tidal);
        write_fate(
            &mut w,
            &format!("sun[{i}]"),
            &BodyFate::resolve(&b, &host),
            &times,
        );
    }

    w.line("# a 12 M_sun neutron star and a 25 M_sun fallback black hole, 50 and 30 Myr old");
    let near = [past(-40_000_000), past(-25_000_000), UniverseTime::EPOCH];
    for (name, mass, age) in [("ns", 12.0, 5e7), ("bh", 25.0, 3e7)] {
        let model = star(mass, age);
        let host = FateHost::star(&model);
        for (i, a) in [3.0, 60.0, 400.0].into_iter().enumerate() {
            let slot = u16::try_from(i + 1).expect("a small slot");
            let b = body(slot, a, 0.1, 317.8, mass, 1e15);
            write_fate(
                &mut w,
                &format!("{name}[{i}]"),
                &BodyFate::resolve(&b, &host),
                &near,
            );
        }
    }

    w.line("# a circumbinary planet of 20 + 3 M_sun, 30 Myr old");
    let (big, small) = (star(20.0, 3e7), star(3.0, 3e7));
    let pair = FateHost::stars([&big, &small]).expect("coeval");
    let b = body(1, 400.0, 0.05, 317.8, 23.0, 1e15);
    write_fate(&mut w, "pair[0]", &BodyFate::resolve(&b, &pair), &near);

    golden!("planetary/fate", w.as_str());
}
