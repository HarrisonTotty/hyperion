//! The committed condensed optics are exactly what `run substance_albedos --since <current>` would
//! write (plan 14, P14.T49.c; plan 15, P15.T2), from rendering plan R08's material files as they
//! stand.
//!
//! CI runs this, so a material file whose n or k changed without a rerun, or a table edited by
//! hand, fails it.

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/substance_albedo.rs");

#[test]
fn substance_albedos_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("substance_albedos").expect("substance_albedos is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the substance_albedos task runs");
    if planned.text != COMMITTED {
        let line = planned
            .text
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/substance_albedo.rs differs from the fit at {line}: run \
             `just fit substance_albedos`"
        );
    }
    assert!(!planned.body_changed);
}
