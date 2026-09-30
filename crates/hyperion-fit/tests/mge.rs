//! The committed Gaussian-expansion table is exactly what `run mge --since <current>` would write
//! (plan 02, P02.T6.a; plan 15, P15.T1 and P15.T2).
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::mge::{fit, residuals};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/mge.rs");

#[test]
fn mge_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("mge").expect("mge is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the mge task runs");
    if planned.text != COMMITTED {
        let line = planned
            .text
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/mge.rs differs from the fit at {line}: run \
             `just fit mge`"
        );
    }
    assert!(!planned.body_changed);
}

/// Two runs give the same bytes: the fit has no randomness and no tolerance test.
#[test]
fn the_fit_is_deterministic() {
    assert_eq!(fit(), fit());
}

/// The acceptance figures the header records hold on P15.T3.a's range, s = 0.05–10: the
/// exponential within 1.5% (plan 02's 1% holds on the fit's own samples, not out to s = 10), the
/// bar within 3% of its central value, and its mass within 0.5%.
#[test]
fn the_expansions_follow_their_profiles() {
    let r = residuals(&fit());
    assert!(r.exp_worst_relative < 0.015, "{r:?}");
    assert!(r.bar_worst < 0.03, "{r:?}");
    assert!(r.bar_mass_relative.abs() < 0.005, "{r:?}");
}
