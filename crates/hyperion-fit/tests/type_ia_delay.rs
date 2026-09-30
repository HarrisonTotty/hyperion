//! The committed Type Ia delay table is exactly what `run type_ia_delay --since <current>` would
//! write (plan 15, P15.T9.a), and its acceptance passes.
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::type_ia_delay::{acceptance, fit};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::{GENERATOR_VERSION, Seed};

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/type_ia_delay.rs");

#[test]
fn type_ia_delay_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("type_ia_delay").expect("type_ia_delay is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the type_ia_delay task runs");
    let rendered = planned.text;
    if rendered != COMMITTED {
        let line = rendered
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/type_ia_delay.rs differs from the fit at {line}: run \
             `just fit type_ia_delay`"
        );
    }
}

/// P15.T9.a's acceptance at the manifest's Milky Way: the yield to 1%, a fifth under 0.1 Gyr and
/// about 62% under 1 Gyr, no primary under 2.5 M☉, 0.42–0.53 a century (ruling 141.4) and the old
/// populations' exploded share of layer D in 2–4.5%.
#[test]
fn the_acceptance_passes() {
    let galaxy = Galaxy::from_params(Seed::new(0x0918_0001), GalaxyParams::milky_way_like())
        .expect("the Milky Way's parameters build");
    let table = fit(&galaxy);
    let figures = acceptance(&table, &galaxy);
    assert_eq!(figures.passed(), [true; 6], "{figures:?}");
    assert!(
        table.worst_quantile_error < 0.02,
        "{}",
        table.worst_quantile_error
    );
}
