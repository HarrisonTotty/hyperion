//! The committed giant-planet cooling table is exactly what `run giant_cooling` produces (plan 13,
//! P13.T5.b), and it reproduces the grid it was fitted to.
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

use hyperion_fit::tasks::giant_cooling::{AGES, MASSES, fit, grid, render};

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/giant_cooling.rs");

#[test]
fn giant_cooling_table_is_reproduced() {
    let rendered = render(&fit());
    if rendered != COMMITTED {
        let line = rendered
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/giant_cooling.rs differs from the fit at {line}: run \
             `cargo run -p hyperion-fit -- run giant_cooling`"
        );
    }
}

/// Two runs give the same bytes: the fit has no randomness and no tolerance test.
#[test]
fn the_fit_is_deterministic() {
    assert_eq!(fit(), fit());
}

/// The table follows Bobcat's rows to 0.05 dex in luminosity and 1% in radius, and the grid's
/// interior far better.
#[test]
fn the_table_reproduces_the_grid() {
    let table = fit();
    let r = table.residuals;
    eprintln!("{r:?}");
    assert_eq!(usize::try_from(r.rows).unwrap(), grid().len());
    assert!(r.worst_log_luminosity.residual.abs() < 0.05, "{r:?}");
    assert!(r.worst_radius.residual.abs() < 0.01, "{r:?}");
    assert!(r.rms_log_luminosity < 0.01, "{r:?}");
    assert!(r.rms_radius < 0.002, "{r:?}");
}

/// Bilinear interpolation keeps whatever order the nodes have, so the sim's monotonicity rests on
/// this: at every mass node luminosity and radius fall with age, and at every age node
/// luminosity rises with mass.
#[test]
fn the_nodes_fall_with_age_and_luminosity_rises_with_mass() {
    let table = fit();
    for i in 0..MASSES {
        for j in 1..AGES {
            assert!(
                table.log_luminosity[i][j] < table.log_luminosity[i][j - 1],
                "L rises with age at node ({i}, {j})"
            );
            assert!(
                table.log_radius[i][j] < table.log_radius[i][j - 1],
                "R rises with age at node ({i}, {j})"
            );
        }
    }
    for j in 0..AGES {
        for i in 1..MASSES {
            assert!(
                table.log_luminosity[i][j] > table.log_luminosity[i - 1][j],
                "L falls with mass at node ({i}, {j})"
            );
        }
    }
}
