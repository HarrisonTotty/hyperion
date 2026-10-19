//! The committed thermochemistry is exactly what `run thermo_equilibrium --since <current>` would
//! write (plan 14, P14.T49.d; plan 15, P15.T2), from the committed subset of NASA's CEA library.
//!
//! CI runs this, so a library or a record list changed without a rerun, or a table edited by hand,
//! fails it.

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/thermo.rs");

#[test]
fn thermo_equilibrium_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("thermo_equilibrium").expect("thermo_equilibrium is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the thermo_equilibrium task runs");
    if planned.text != COMMITTED {
        let line = planned
            .text
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/thermo.rs differs from the fit at {line}: run `just fit \
             thermo_equilibrium`"
        );
    }
    assert!(!planned.body_changed);
}
