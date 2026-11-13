//! Plan 02's guards on P02.T6.f's premise: no Jeans table reads the potential beyond the (R, |z|)
//! grid's box, [`GRID_EDGE_LY`] (2¹⁸ ly). `PotentialTables::forces` keeps its old arithmetic
//! inside the box and wherever both coordinates are at or beyond its edge, so a node rounded above
//! the edge with the other coordinate inside would take the far field's new branch and move the
//! tables that read it (plan 02's T6.f record).

use crate::Seed;
use crate::galaxy::Population;
use crate::galaxy::fields::{Fields, Shape};
use crate::galaxy::imf::MassFunctionKind;
use crate::galaxy::kinematics::{discs, spheroid};
use crate::galaxy::params::{BuildGalaxyParamsError, GalaxyParams, GalaxyParamsBuilder};
use crate::galaxy::potential::{GRID_EDGE_LY, MassModel};
use crate::units::{LightYears, SolarMasses, Years};

/// The spheroids' tables (the bulge's, the long bar's and the nuclear disc's) sit on one grid
/// fixed in light-years, so their last node is the same in every galaxy: just inside the box,
/// within 10⁻⁹ ly of its edge.
#[test]
fn the_spheroid_tables_last_node_lies_inside_the_box() {
    let last = spheroid::last_grid_point_ly();
    assert!(last <= GRID_EDGE_LY, "{last} ly");
    assert!(GRID_EDGE_LY - last < 1e-9, "{last} ly");
}

/// The disc tables of `params`' galaxy: the young disc's, the old thin disc's five sub-discs' and
/// the thick disc's, each with the farthest radius and tail height, ly, at which it reads the
/// potential. The nuclear disc's table is a spheroid's.
fn disc_reaches(params: &GalaxyParams) -> Vec<(Population, (f64, f64))> {
    let model = MassModel::new(params);
    let fields = Fields::new(params, &model);
    fields
        .components()
        .iter()
        .filter_map(
            |component| match (component.population(), component.shape()) {
                (
                    population @ (Population::YoungThinDisc
                    | Population::OldThinDisc
                    | Population::ThickDisc),
                    Shape::Disc(disc),
                ) => Some((population, discs::potential_reach_ly(disc))),
                (
                    Population::YoungThinDisc | Population::OldThinDisc | Population::ThickDisc,
                    Shape::Bulge(_) | Shape::Bar(_) | Shape::Halo(_),
                ) => panic!("a disc population has a disc's shape"),
                (
                    Population::Bulge
                    | Population::LongBar
                    | Population::NuclearDisc
                    | Population::Halo,
                    _,
                ) => None,
            },
        )
        .collect()
}

/// Whether the builder refuses `builder` for `parameter` lying outside its drawn range.
fn refuses(builder: GalaxyParamsBuilder, parameter: &str) -> bool {
    matches!(
        builder.build(),
        Err(BuildGalaxyParamsError::OutOfRange { parameter: p, .. }) if p == parameter
    )
}

/// Every edge the sweep takes is its drawn range's (`params/draws.rs`, which the builder
/// enforces): the builder refuses a value just beyond it.
#[test]
fn the_swept_edges_are_the_drawn_ranges() {
    let base = GalaxyParamsBuilder::new;
    let (above, below) = (|v: f64| v * (1.0 + 1e-9), |v: f64| v * (1.0 - 1e-9));
    let ly = LightYears::new;
    for (builder, parameter) in [
        (base().thin_length(ly(above(11_500.0))), "thin.length"),
        (base().thin_length(ly(below(7_000.0))), "thin.length"),
        (
            base().stellar_mass(SolarMasses::new(above(1e11))),
            "stellar_mass",
        ),
        (
            base().stellar_mass(SolarMasses::new(below(3e10))),
            "stellar_mass",
        ),
        (
            base().sfh_timescale(Years::new(above(9e9))),
            "sfh.timescale",
        ),
        (
            base().sfh_timescale(Years::new(below(5e9))),
            "sfh.timescale",
        ),
        (
            base().thin_mean_height(ly(above(1_150.0))),
            "thin.mean_height",
        ),
        (
            base().thin_mean_height(ly(below(850.0))),
            "thin.mean_height",
        ),
        (base().thick_height_ratio(above(3.3)), "thick.height_ratio"),
        (base().thick_height_ratio(below(2.7)), "thick.height_ratio"),
        (base().young_height(ly(above(345.0))), "young.height"),
        (base().thick_length_ratio(above(0.9)), "thick.length_ratio"),
        (base().dark_f_star(above(0.45)), "dark.f_star"),
        (base().dark_f_star(below(0.12)), "dark.f_star"),
        (base().gas_mass_fraction(above(0.35)), "gas.mass_fraction"),
        (base().gas_mass_fraction(below(0.175)), "gas.mass_fraction"),
    ] {
        assert!(refuses(builder, parameter), "{parameter}");
    }
}

/// The sweep's galaxies: every corner of the thin disc's length, the stellar mass, the formation
/// timescale, the old thin disc's mean height and the thick disc's height ratio; then, at the
/// tallest corner, every corner of the stellar mass, the timescale, the dark halo's f★ and the gas
/// disc's mass fraction. The young disc's height and the thick disc's length ratio are at their
/// tops, where the reach is greatest.
fn sweep() -> Vec<GalaxyParamsBuilder> {
    let base = GalaxyParamsBuilder::new;
    let ly = LightYears::new;
    let tops = || base().young_height(ly(345.0)).thick_length_ratio(0.9);
    let mut sweep = Vec::new();
    for length in [7_000.0, 11_500.0] {
        for mass in [3e10, 1e11] {
            for timescale in [5e9, 9e9] {
                for mean_height in [850.0, 1_150.0] {
                    for ratio in [2.7, 3.3] {
                        sweep.push(
                            tops()
                                .thin_length(ly(length))
                                .stellar_mass(SolarMasses::new(mass))
                                .sfh_timescale(Years::new(timescale))
                                .thin_mean_height(ly(mean_height))
                                .thick_height_ratio(ratio),
                        );
                    }
                }
            }
        }
    }
    for mass in [3e10, 1e11] {
        for timescale in [5e9, 9e9] {
            for f_star in [0.12, 0.45] {
                for gas in [0.175, 0.35] {
                    sweep.push(
                        tops()
                            .thin_length(ly(11_500.0))
                            .stellar_mass(SolarMasses::new(mass))
                            .sfh_timescale(Years::new(timescale))
                            .thin_mean_height(ly(1_150.0))
                            .thick_height_ratio(3.3)
                            .dark_f_star(f_star)
                            .gas_mass_fraction(gas),
                    );
                }
            }
        }
    }
    sweep
}

/// The disc tables' largest radius and tail height lie inside the box over the ranges the seeded
/// galaxies draw, at their edges ([`sweep`]): the thin disc's length at both ends (the coupled law
/// clamps into its range), the stellar mass and the formation timescale (which, with the
/// potential, set the old thin disc's spread of heights about its drawn mean), the old thin disc's
/// mean height, the thick disc's height ratio, and the dark halo's f★ and the gas disc's mass
/// fraction, which shape the vertical force too. Then 16 seeded galaxies, each as drawn.
#[test]
fn the_disc_tables_reach_lies_inside_the_box_over_the_drawn_ranges() {
    let mut galaxies: Vec<GalaxyParams> = sweep()
        .into_iter()
        .map(|builder| {
            builder
                .build()
                .expect("every value lies inside its drawn range")
        })
        .collect();
    galaxies.extend(
        (0..16).map(|s| GalaxyParams::from_seed(Seed::new(s), MassFunctionKind::default())),
    );
    let mut farthest = [(0.0_f64, 0.0_f64); 3];
    for params in &galaxies {
        let reaches = disc_reaches(params);
        assert_eq!(reaches.len(), 7, "seven disc tables: {reaches:?}");
        for (population, (r, z)) in reaches {
            assert!(r > 0.0 && r <= GRID_EDGE_LY, "{population:?}: R = {r} ly");
            assert!(z > 0.0 && z <= GRID_EDGE_LY, "{population:?}: |z| = {z} ly");
            let k = match population {
                Population::YoungThinDisc => 0,
                Population::OldThinDisc => 1,
                Population::ThickDisc => 2,
                Population::Bulge
                | Population::LongBar
                | Population::NuclearDisc
                | Population::Halo => unreachable!("only the discs' tables are listed"),
            };
            farthest[k] = (farthest[k].0.max(r), farthest[k].1.max(z));
        }
    }
    eprintln!(
        "over {} galaxies the disc tables read the potential to (R, |z|) = {farthest:.0?} ly (young, \
         old thin, thick), against the box's {GRID_EDGE_LY} ly",
        galaxies.len()
    );
}
