//! Golden values of plan 14's built pieces: the disc's draws and discs, plan 06's disc-lifetime
//! law, and the closed forms of the limits and the spacing floor (P14.T3, T6.a, T15; P06.T15.c).
//!
//! They pin the arithmetic, not only the draws: a reordered sum, a changed power or a moved word
//! changes a line here, which is a generator-version change. CI checks the same file on 64-bit Arm
//! and on wasm32.

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::planetary::derive::limits::{TidalPlanet, moon_mass_limit};
use hyperion_sim::planetary::derive::{
    OrbitSense, hill_radius, maximum_surviving_moon_mass, roche_limit_fluid, roche_limit_rigid,
    satellite_stability_limit,
};
use hyperion_sim::planetary::disc::{self, Disc, DiscDraws, DiscHost, Truncation};
use hyperion_sim::planetary::placement::{
    Neighbour, mutual_hill_factor, mutual_hill_radius, next_semi_major_axis, satisfies_floor,
    spacing_floor,
};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::UnitUniform;
use hyperion_sim::stellar::premain::disc_lifetime;
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::units::consts::{
    EARTH_MASS_KG, JUPITER_MASS_KG, METRES_PER_AU, SECONDS_PER_GIGAYEAR, SOLAR_MASS_KG,
};
use hyperion_sim::units::{
    AstronomicalUnits, Dex, EarthMasses, HeliumExcess, Kilograms, KilogramsPerCubicMetre,
    Megayears, Metres, Seconds, SolarMasses,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn au(x: f64) -> Metres {
    Metres::from(AstronomicalUnits::new(x))
}

fn system(cell: [i32; 3], index: u32) -> SystemId {
    SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, cell).unwrap(), index).unwrap()
}

/// A host at the zero-age main sequence of plan 06's fits.
fn zams_host(mass: f64, fe_h: f64) -> DiscHost {
    let m = SolarMasses::new(mass);
    let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let coeffs = ZCoeffs::new(composition.z_fit());
    DiscHost::new(
        m,
        composition.fe_h(),
        zams::luminosity(m, &coeffs),
        zams::radius(m, &coeffs),
    )
    .unwrap()
}

fn write_draws(w: &mut GoldenWriter, d: &DiscDraws) {
    w.f64("gas_fraction", d.gas_fraction.value());
    w.f64("corotation_period", d.corotation_period.value());
    w.f64("characteristic_radius", d.characteristic_radius.value());
    w.f64(
        "circumbinary_lifetime_rank",
        d.circumbinary_lifetime_rank.value(),
    );
}

fn write_disc(w: &mut GoldenWriter, disc: &Disc) {
    let Some(p) = disc.profile() else {
        w.line("none");
        return;
    };
    w.f64("gas_mass_msun", p.gas_mass().value());
    w.f64("solid_mass_mearth", p.solid_mass().value());
    w.f64("lifetime_myr", p.lifetime().value());
    w.f64("snow_line_m", p.snow_line().value());
    w.f64("inner_edge_m", p.inner_edge().value());
    w.f64("outer_edge_m", p.outer_edge().value());
    w.f64("characteristic_radius_m", p.characteristic_radius().value());
    w.f64("corotation_period_s", p.corotation_period().value());
    w.f64(
        "solid_mass_0.5_to_5_au",
        p.solid_mass_between(au(0.5), au(5.0)).value(),
    );
    for a in [0.3, 1.0, 5.0] {
        w.f64(
            &format!("sigma_solids_{a}_au"),
            p.surface_density(au(a)).value(),
        );
        w.f64(
            &format!("sigma_gas_{a}_au"),
            p.gas_surface_density(au(a)).value(),
        );
        w.f64(
            &format!("isolation_mass_{a}_au"),
            p.isolation_mass(au(a)).value(),
        );
    }
}

#[test]
fn discs_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let a = system([652, -4_584, 2_047], 7);
    let b = system([-3, 10, 0], 0);
    let cases = [
        (Seed::new(0x0123_4567_89ab_cdef), a, 0),
        (Seed::new(0x0123_4567_89ab_cdef), a, 1),
        (Seed::new(42), b, 0),
        (Seed::new(42), b, 255),
    ];
    for (seed, id, host) in cases {
        w.line("");
        w.line(&format!("draws seed {seed} system {id} host {host}"));
        write_draws(&mut w, &DiscDraws::for_host(seed, id, host));
    }
    let seed = Seed::new(42);
    let hosts = [
        ("sun", zams_host(1.0, 0.0), Truncation::NONE),
        ("m dwarf", zams_host(0.3, -0.4), Truncation::NONE),
        ("a star", zams_host(2.2, 0.2), Truncation::NONE),
        (
            "truncated sun",
            zams_host(1.0, 0.0),
            Truncation::NONE.with_inner(au(0.2)).with_outer(au(4.0)),
        ),
        (
            "inverted",
            zams_host(1.0, 0.0),
            Truncation::NONE.with_inner(au(3.0)).with_outer(au(2.0)),
        ),
    ];
    for (i, (name, host, truncation)) in (0_u8..).zip(hosts) {
        let draws = DiscDraws::for_host(seed, b, i);
        let lifetime = disc_lifetime(host.mass(), draws.circumbinary_lifetime_rank);
        w.line("");
        w.line(&format!("disc {name}, host {i} of {b}"));
        write_disc(&mut w, &disc::derive(&host, lifetime, &draws, truncation));
    }
    w.line("");
    w.line("median disc of the sun");
    write_disc(
        &mut w,
        &disc::derive(
            &zams_host(1.0, 0.0),
            Megayears::new(2.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        ),
    );
    golden!("planetary/disc", w.as_str());
}

#[test]
fn disc_lifetimes_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for m in [0.08, 0.3, 1.0, 2.0, 8.0, 40.0] {
        for u in [1e-9, 0.1, 0.5, 0.9, 0.999_999] {
            let rank = UnitUniform::new(u).unwrap();
            w.f64(
                &format!("m {m} rank {u}"),
                disc_lifetime(SolarMasses::new(m), rank).value(),
            );
        }
    }
    golden!("stellar/disc_lifetime", w.as_str());
}

#[test]
fn limits_and_spacing_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let (radius, density, ice) = (
        Metres::new(58_232e3),
        KilogramsPerCubicMetre::new(687.0),
        KilogramsPerCubicMetre::new(600.0),
    );
    w.f64(
        "roche_fluid_saturn",
        roche_limit_fluid(radius, density, ice).value(),
    );
    w.f64(
        "roche_rigid_saturn",
        roche_limit_rigid(radius, density, ice).value(),
    );
    let (sun, earth) = (Kilograms::new(SOLAR_MASS_KG), Kilograms::new(EARTH_MASS_KG));
    let hill = hill_radius(Metres::new(METRES_PER_AU), 0.0167, earth, sun);
    w.f64("hill_earth_pericentre", hill.value());
    let circular = hill_radius(Metres::new(METRES_PER_AU), 0.0, earth, sun);
    for (sense, name) in [
        (OrbitSense::Prograde, "prograde"),
        (OrbitSense::Retrograde, "retrograde"),
    ] {
        for (e_p, e_s) in [(0.0, 0.0), (0.0167, 0.0549), (0.2, 0.5)] {
            w.f64(
                &format!("stability_{name}_{e_p}_{e_s}"),
                satellite_stability_limit(circular, e_p, e_s, sense).value(),
            );
        }
    }
    let planet = TidalPlanet::new(
        Kilograms::new(0.69 * JUPITER_MASS_KG),
        Metres::new(1.35 * 7.1492e7),
        0.51,
        1e5,
    )
    .unwrap();
    let star = Kilograms::new(1.1 * SOLAR_MASS_KG);
    let age = Seconds::new(5.0 * SECONDS_PER_GIGAYEAR);
    let hd_hill = hill_radius(au(0.0468), 0.0, planet.mass(), star);
    w.f64(
        "moon_limit_hd209458b_0.36",
        moon_mass_limit(hd_hill * 0.36, &planet, age).value(),
    );
    w.f64(
        "surviving_moon_hd209458b",
        maximum_surviving_moon_mass(&planet, star, au(0.0468), 0.0, age).value(),
    );
    let (jupiter, saturn) = (EarthMasses::new(317.83), EarthMasses::new(95.16));
    let one = SolarMasses::new(1.0);
    w.f64(
        "mutual_hill_jupiter_saturn",
        mutual_hill_radius(jupiter, saturn, one, au(5.2029), au(9.5367)).value(),
    );
    let chi = mutual_hill_factor(EarthMasses::new(3.0), EarthMasses::new(5.0), one);
    w.f64("hill_factor_3_5", chi.value());
    let next = next_semi_major_axis(au(0.1), 17.0, chi).unwrap();
    w.f64("next_semi_major_axis_0.1_au_17", next.value());
    for (m1, m2, e1, e2) in [
        (3.0, 5.0, 0.0, 0.0),
        (3.0, 5.0, 0.013, 0.007),
        (317.83, 95.16, 0.05, 0.05),
    ] {
        w.f64(
            &format!("floor_{m1}_{m2}_{e1}_{e2}"),
            spacing_floor(EarthMasses::new(m1), EarthMasses::new(m2), e1, e2),
        );
    }
    let inner = Neighbour::new(jupiter, au(5.2029), 0.0484);
    let outer = Neighbour::new(saturn, au(9.5367), 0.0539);
    w.line(&format!(
        "jupiter_saturn_satisfy_floor = {}",
        satisfies_floor(&inner, &outer, one)
    ));
    golden!("planetary/limits", w.as_str());
}
