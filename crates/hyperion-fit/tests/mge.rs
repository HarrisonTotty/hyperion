//! The committed Gaussian-expansion table is exactly what `run mge` produces (plan 02, P02.T6.a).
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

use hyperion_fit::tasks::mge::{fit, render};

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/mge.rs");

#[test]
fn mge_table_is_reproduced() {
    let rendered = render(&fit());
    if rendered != COMMITTED {
        let line = rendered
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/mge.rs differs from the fit at {line}: run \
             `cargo run -p hyperion-fit -- run mge`"
        );
    }
}

/// Two runs give the same bytes: the fit has no randomness and no tolerance test.
#[test]
fn the_fit_is_deterministic() {
    assert_eq!(fit(), fit());
}
