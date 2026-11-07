use std::sync::OnceLock;

use super::*;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::CellKey;
use crate::id::Layer;
use crate::planetary::derive::atmosphere::VolatileDraws;
use crate::planetary::derive::rotation::SpinDraws;
use crate::planetary::record::Section;

/// The Milky Way fixture, built once per test binary.
pub(super) fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(
            Seed::new(0x0e15_0001_0000_0001),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture builds")
    })
}

/// The fixture's density grid, built once per test binary.
pub(super) fn grid() -> &'static DensityGrid {
    static GRID: OnceLock<DensityGrid> = OnceLock::new();
    GRID.get_or_init(|| DensityGrid::of(galaxy()))
}

/// An Earth about a Sun, every drawn input left to its draw.
const EARTH: RawPlanetInputs = RawPlanetInputs {
    seed: 0x0e15_0001,
    star_mass_msun: 1.0,
    fe_h_dex: 0.0,
    age_gyr: 4.57,
    mass_mearth: 1.0,
    semi_major_axis_au: 1.0,
    eccentricity: 0.0167,
    formed_at_au: None,
    day_hours: None,
    tilt_deg: None,
    water_rank: None,
    carbon_rank: None,
    nitrogen_rank: None,
};

fn index_of(body: BodyId) -> BodyIndex {
    BodyIndex::try_from(body.body_index()).expect("a planet's index")
}

fn inputs(raw: RawPlanetInputs) -> PlanetInputs {
    PlanetInputs::try_from(raw).expect("inputs within their ranges")
}

fn generated(raw: RawPlanetInputs) -> ViewerSystem {
    ViewerSystem::generate(inputs(raw)).expect("a system")
}

#[test]
fn the_same_inputs_give_the_same_system_bit_for_bit() {
    let (a, b) = (generated(EARTH), generated(EARTH));
    assert_eq!(a, b);
    let t = UniverseTime::EPOCH;
    assert_eq!(
        a.planets().snapshot_at(a.context(), t),
        b.planets().snapshot_at(b.context(), t)
    );
    assert_eq!(a.summary_at(t), b.summary_at(t));
    assert_eq!(a.host_discs_at(t), b.host_discs_at(t));
}

#[test]
fn every_input_names_its_own_system_of_the_viewer_s_pinned_half() {
    let earth = inputs(EARTH).system_id();
    assert!(is_viewer_system(earth));
    let others = [
        RawPlanetInputs { seed: 2, ..EARTH },
        RawPlanetInputs {
            star_mass_msun: 0.9,
            ..EARTH
        },
        RawPlanetInputs {
            fe_h_dex: 0.1,
            ..EARTH
        },
        RawPlanetInputs {
            age_gyr: 4.0,
            ..EARTH
        },
        RawPlanetInputs {
            mass_mearth: 2.0,
            ..EARTH
        },
        RawPlanetInputs {
            semi_major_axis_au: 1.1,
            ..EARTH
        },
        RawPlanetInputs {
            eccentricity: 0.1,
            ..EARTH
        },
        RawPlanetInputs {
            formed_at_au: Some(1.0),
            ..EARTH
        },
        RawPlanetInputs {
            day_hours: Some(24.0),
            ..EARTH
        },
        RawPlanetInputs {
            tilt_deg: Some(0.0),
            ..EARTH
        },
        RawPlanetInputs {
            water_rank: Some(0.5),
            ..EARTH
        },
        RawPlanetInputs {
            carbon_rank: Some(0.5),
            ..EARTH
        },
        RawPlanetInputs {
            nitrogen_rank: Some(0.5),
            ..EARTH
        },
    ];
    let mut ids: Vec<SystemId> = others.iter().map(|&raw| inputs(raw).system_id()).collect();
    ids.push(earth);
    let count = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), count, "every input changes the ID");
    // A negative zero is the positive zero's input.
    assert_eq!(
        inputs(RawPlanetInputs {
            fe_h_dex: -0.0,
            ..EARTH
        })
        .system_id(),
        earth
    );
    let grid = CellKey::new(Layer::C, [0, 812, 0])
        .unwrap()
        .candidate_id(0)
        .unwrap();
    assert!(!is_viewer_system(grid));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one refusal an input, each a whole set of inputs"
)]
fn each_input_out_of_its_range_is_refused_naming_its_field() {
    let refused = [
        (
            RawPlanetInputs {
                star_mass_msun: 0.05,
                ..EARTH
            },
            InputField::StarMass,
        ),
        (
            RawPlanetInputs {
                star_mass_msun: f64::NAN,
                ..EARTH
            },
            InputField::StarMass,
        ),
        (
            RawPlanetInputs {
                fe_h_dex: 1.5,
                ..EARTH
            },
            InputField::FeH,
        ),
        (
            RawPlanetInputs {
                age_gyr: 14.0,
                ..EARTH
            },
            InputField::Age,
        ),
        (
            RawPlanetInputs {
                mass_mearth: 5_000.0,
                ..EARTH
            },
            InputField::Mass,
        ),
        (
            RawPlanetInputs {
                semi_major_axis_au: 0.0,
                ..EARTH
            },
            InputField::SemiMajorAxis,
        ),
        (
            RawPlanetInputs {
                eccentricity: 0.95,
                ..EARTH
            },
            InputField::Eccentricity,
        ),
        (
            RawPlanetInputs {
                formed_at_au: Some(-1.0),
                ..EARTH
            },
            InputField::FormedAt,
        ),
        (
            RawPlanetInputs {
                day_hours: Some(f64::INFINITY),
                ..EARTH
            },
            InputField::Day,
        ),
        (
            RawPlanetInputs {
                tilt_deg: Some(181.0),
                ..EARTH
            },
            InputField::Tilt,
        ),
        (
            RawPlanetInputs {
                water_rank: Some(1.0),
                ..EARTH
            },
            InputField::Water,
        ),
        (
            RawPlanetInputs {
                carbon_rank: Some(0.0),
                ..EARTH
            },
            InputField::Carbon,
        ),
        (
            RawPlanetInputs {
                nitrogen_rank: Some(2.0),
                ..EARTH
            },
            InputField::Nitrogen,
        ),
    ];
    for (raw, field) in refused {
        let error = PlanetInputs::try_from(raw).expect_err("out of range");
        assert_eq!(error.field(), field, "{error}");
    }
    let error = PlanetInputs::try_from(RawPlanetInputs {
        tilt_deg: Some(181.0),
        ..EARTH
    })
    .expect_err("out of range");
    assert_eq!(error.to_string(), "tilt_deg 181 is not within 0 to 180");
}

#[test]
fn an_orbit_beyond_the_reach_or_inside_its_star_is_refused_on_the_axis() {
    // The Sun's reach is some 138,000 au: an apoapsis of 190,000 au lies beyond it.
    let far = ViewerSystem::generate(inputs(RawPlanetInputs {
        semi_major_axis_au: 1.0e5,
        eccentricity: 0.9,
        ..EARTH
    }))
    .expect_err("beyond the reach");
    assert!(
        matches!(far, GenerateViewerError::BeyondReach { .. }),
        "{far}"
    );
    assert_eq!(far.field(), InputField::SemiMajorAxis);
    let inside = ViewerSystem::generate(inputs(RawPlanetInputs {
        semi_major_axis_au: 0.004,
        ..EARTH
    }))
    .expect_err("inside the Sun");
    assert!(
        matches!(inside, GenerateViewerError::InsideStar { .. }),
        "{inside}"
    );
    assert_eq!(inside.field(), InputField::SemiMajorAxis);
}

#[test]
fn an_earth_about_a_sun_is_a_present_rocky_planet_of_the_given_mass_and_orbit() {
    let system = generated(EARTH);
    let planet = system.planet();
    assert_eq!(planet.system(), system.id());
    let record = system
        .planets()
        .body_at(system.context(), index_of(planet), UniverseTime::EPOCH)
        .expect("the planet's record");
    assert_eq!(record.identity().state(), BodyState::Present);
    let Section::Ok(bulk) = record.bulk() else {
        panic!("the planet's bulk is derived");
    };
    assert!(!bulk.class().is_giant(), "{:?}", bulk.class());
    let parts = system
        .planets()
        .planet_parts(index_of(planet))
        .expect("a planet");
    assert_eq!(parts.placed.mass(), EarthMasses::new(1.0));
    assert_eq!(
        parts.placed.orbit().semi_major_axis(),
        Metres::from(AstronomicalUnits::new(1.0))
    );
    assert_eq!(parts.placed.orbit().eccentricity().value(), 0.0167);
    // Formed where it lies, inside the snow line.
    assert_eq!(
        parts.placed.formation_distance(),
        parts.placed.orbit().semi_major_axis()
    );
    assert!(!parts.placed.formed_beyond_snow_line());
    let summary = system.summary_at(UniverseTime::EPOCH);
    assert_eq!(summary.stars().len(), 1);
    assert_eq!(system.host_discs_at(UniverseTime::EPOCH).len(), 1);
}

#[test]
fn a_giant_forms_where_giants_cores_grow_unless_its_formation_is_given() {
    let hot = RawPlanetInputs {
        mass_mearth: 318.0,
        semi_major_axis_au: 0.05,
        eccentricity: 0.0,
        ..EARTH
    };
    let parts = |raw| {
        let system = generated(raw);
        system
            .planets()
            .planet_parts(index_of(system.planet()))
            .expect("a planet")
    };
    let migrated = parts(hot);
    assert!(migrated.placed.formation_distance() > migrated.placed.orbit().semi_major_axis());
    assert!(migrated.placed.formed_beyond_snow_line());
    let in_situ = parts(RawPlanetInputs {
        formed_at_au: Some(0.05),
        ..hot
    });
    assert_eq!(
        in_situ.placed.formation_distance(),
        Metres::from(AstronomicalUnits::new(0.05))
    );
    assert!(!in_situ.placed.formed_beyond_snow_line());
}

#[test]
fn a_given_spin_and_ranks_stand_in_place_of_their_draws_and_nothing_else() {
    let drawn = generated(EARTH);
    let given = generated(RawPlanetInputs {
        day_hours: Some(30.0),
        tilt_deg: Some(60.0),
        water_rank: Some(0.9),
        carbon_rank: Some(0.2),
        nitrogen_rank: Some(0.7),
        ..EARTH
    });
    let values = given.drawn();
    assert_eq!(values.day_hours, Some(30.0));
    assert!((values.tilt_deg.expect("a rotation law") - 60.0).abs() < 1e-12);
    assert_eq!(
        (values.water_rank, values.carbon_rank, values.nitrogen_rank),
        (0.9, 0.2, 0.7)
    );
    // Left to the draws, the inputs are the planet's own draws of its ID.
    let index = index_of(drawn.planet());
    let own = VolatileDraws::for_body(drawn.inputs().seed(), drawn.planet());
    let shown = drawn.drawn();
    assert_eq!(
        (shown.water_rank, shown.carbon_rank, shown.nitrogen_rank),
        (own.water.value(), own.carbon.value(), own.nitrogen.value())
    );
    let spin = SpinDraws::for_body(drawn.inputs().seed(), index.body_id(drawn.id()));
    let tilt = crate::planetary::derive::rotation::obliquity(
        crate::planetary::derive::rotation::ObliquityLaw::Rayleigh,
        spin.obliquity_rank(),
    );
    let isotropic = crate::planetary::derive::rotation::obliquity(
        crate::planetary::derive::rotation::ObliquityLaw::Isotropic,
        spin.obliquity_rank(),
    );
    let shown_tilt = Radians::from(Degrees::new(shown.tilt_deg.expect("a rotation law")));
    assert!(
        [tilt, isotropic]
            .iter()
            .any(|t| (t.value() - shown_tilt.value()).abs() < 1e-12),
        "the drawn tilt is the body's own obliquity draw"
    );
}

/// A single star of `galaxy` at the Sun-like point with a planet present at the epoch, and the
/// planet.
fn real_planet(galaxy: &Galaxy) -> (SystemContext, PlanetarySystem, BodyIndex) {
    let key = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell at the Sun-like point");
    let mut cell = Vec::new();
    generate_cell(galaxy, key, &mut cell);
    cell.iter()
        .find_map(|record| {
            let ctx = SystemContext::from_record(galaxy, record);
            if ctx.stars().len() != 1 || ctx.host_kind() != HostKind::Stellar {
                return None;
            }
            let planets = generate(galaxy.seed(), &ctx);
            let index = planets
                .planets()
                .map(crate::planetary::Body::index)
                .find(|&index| {
                    planets
                        .body_at(&ctx, index, UniverseTime::EPOCH)
                        .is_ok_and(|body| body.identity().state() == BodyState::Present)
                })?;
            Some((ctx, planets, index))
        })
        .expect("a single star with a present planet in the cell")
}

#[test]
fn a_real_planet_s_inputs_regenerate_its_host_mass_orbit_spin_and_ranks() {
    let (ctx, planets, index) = real_planet(galaxy());
    let raw = inputs_of(&ctx, &planets, index).expect("a planet's inputs");
    let real = planets.planet_parts(index).expect("a planet");
    assert_eq!(raw.seed, galaxy().seed().get());
    assert_eq!(raw.fe_h_dex, ctx.fe_h().value());
    let system = generated(raw);
    let copy = system
        .planets()
        .planet_parts(index_of(system.planet()))
        .expect("a planet");
    assert_eq!(copy.host_mass, real.host_mass);
    assert_eq!(copy.placed.mass(), real.placed.mass());
    let close = |a: Metres, b: Metres| (a.value() / b.value() - 1.0).abs() < 1e-12;
    assert!(close(
        copy.placed.orbit().semi_major_axis(),
        real.placed.orbit().semi_major_axis()
    ));
    assert!(close(
        copy.placed.formation_distance(),
        real.placed.formation_distance()
    ));
    assert_eq!(
        copy.placed.orbit().eccentricity(),
        real.placed.orbit().eccentricity()
    );
    assert_eq!(copy.volatiles.water, real.volatiles.water);
    assert_eq!(copy.volatiles.carbon, real.volatiles.carbon);
    assert_eq!(copy.volatiles.nitrogen, real.volatiles.nitrogen);
    let spin = |parts: &PlanetParts| {
        let rotation = parts.rotation.expect("a rotation law");
        (
            rotation.primordial_period().value(),
            rotation.obliquity().value(),
        )
    };
    let ((p1, o1), (p2, o2)) = (spin(&copy), spin(&real));
    assert!((p1 / p2 - 1.0).abs() < 1e-12, "{p1} s against {p2} s");
    assert!((o1 - o2).abs() < 1e-12, "{o1} against {o2}");
    // A moon, a star or no body has no planet's inputs.
    assert_eq!(inputs_of(&ctx, &planets, BodyIndex::PRIMARY), None);
}

#[test]
fn a_draw_is_repeatable_by_its_seed_and_reads_a_real_planet_of_the_galaxy() {
    let galaxy = galaxy();
    let drawn = draw_inputs(galaxy, grid(), 7).expect("a planet near the Sun-like point");
    assert_eq!(draw_inputs(galaxy, grid(), 7), Ok(drawn));
    let other = draw_inputs(galaxy, grid(), 8).expect("a planet near the Sun-like point");
    assert_ne!(other.inputs, drawn.inputs);
    // The inputs are the source's, but for the draws left to the viewer and the seed.
    let ctx = SystemContext::for_system(galaxy, drawn.source.system()).expect("a real system");
    let planets = generate(galaxy.seed(), &ctx);
    let source = inputs_of(&ctx, &planets, index_of(drawn.source)).expect("a planet");
    let raw = drawn.inputs.raw();
    assert_eq!(
        (
            raw.star_mass_msun,
            raw.fe_h_dex,
            raw.age_gyr,
            raw.mass_mearth
        ),
        (
            source.star_mass_msun,
            source.fe_h_dex,
            source.age_gyr,
            source.mass_mearth
        )
    );
    assert_eq!(
        (raw.semi_major_axis_au, raw.eccentricity, raw.formed_at_au),
        (
            source.semi_major_axis_au,
            source.eccentricity,
            source.formed_at_au
        )
    );
    assert_eq!(
        (
            raw.day_hours,
            raw.tilt_deg,
            raw.water_rank,
            raw.carbon_rank,
            raw.nitrogen_rank
        ),
        (None, None, None, None, None)
    );
    assert_eq!(ctx.stars().len(), 1);
    // And they make a planet.
    let system = ViewerSystem::generate(drawn.inputs).expect("a drawn planet's system");
    assert!(is_viewer_system(system.id()));
}

#[test]
fn the_canonical_text_names_each_given_input_in_order_in_its_shortest_decimal() {
    assert_eq!(
        inputs(EARTH).canonical(),
        "PV1;seed=000000000e150001;star_mass_msun=1;fe_h_dex=0;age_gyr=4.57;mass_mearth=1;\
         semi_major_axis_au=1;eccentricity=0.0167"
    );
    let given = RawPlanetInputs {
        fe_h_dex: -0.0,
        mass_mearth: 1.0e-4,
        day_hours: Some(24.5),
        water_rank: Some(0.25),
        ..EARTH
    };
    assert_eq!(
        inputs(given).canonical(),
        "PV1;seed=000000000e150001;star_mass_msun=1;fe_h_dex=0;age_gyr=4.57;mass_mearth=0.0001;\
         semi_major_axis_au=1;eccentricity=0.0167;day_hours=24.5;water_rank=0.25"
    );
}

#[test]
fn draws_reach_the_galaxy_s_variety_beyond_the_solar_neighbourhood() {
    let galaxy = galaxy();
    let sun = GalacticPosition::from_light_years(VIEWER_PLACE_LY).expect("in the root cube");
    let far = Metres::from(LightYears::new(5_000.0));
    let mut beyond = 0;
    let mut metallicities = Vec::new();
    for draw_seed in 0..8 {
        let drawn = draw_inputs(galaxy, grid(), draw_seed).expect("a planet of the galaxy");
        let record = crate::galaxy::placement::resolve(galaxy, drawn.source.system())
            .expect("a real system");
        if sun.distance_to(record.epoch_position()) > far {
            beyond += 1;
        }
        metallicities.push(drawn.inputs.raw().fe_h_dex);
    }
    assert!(
        beyond > 0,
        "every draw within 5,000 ly of the Sun-like point"
    );
    metallicities.sort_by(f64::total_cmp);
    assert!(
        metallicities[7] - metallicities[0] > 0.2,
        "[Fe/H] spans {metallicities:?}"
    );
}

#[test]
fn the_density_grid_puts_its_points_where_the_galaxy_s_systems_are() {
    let grid = grid();
    // Most of the galaxy's systems lie within the disc's few scale lengths and near the plane.
    let near = (0..64_u32)
        .map(|k| {
            let u = f64::from(k) / 64.0 + 1.0 / 128.0;
            grid.point([u, 0.5, 0.5, 0.5])
        })
        .filter(|p| (p[0] * p[0] + p[1] * p[1]).sqrt() < 40_000.0 && p[2].abs() < 10_000.0)
        .count();
    assert!(near > 56, "{near} of 64 points in the disc's body");
}
