//! The committed table of a spherical bowl's permanent shadow is exactly what
//! `run bowl_permanent_shadow --since <current>` would write (rendering plan R09, R09.T7.f).
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The table the surface crate compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-surface/src/tables/bowl_permanent_shadow.rs");

#[test]
fn bowl_permanent_shadow_table_is_reproduced() {
    let threads = std::thread::available_parallelism().map_or(NonZeroUsize::MIN, |n| {
        n.min(NonZeroUsize::new(4).expect("four"))
    });
    let planned = rerender(
        registry(),
        find("bowl_permanent_shadow").expect("bowl_permanent_shadow is registered"),
        &Workspace::repository(),
        threads,
        GENERATOR_VERSION.get(),
    )
    .expect("the bowl_permanent_shadow task runs");
    if planned.text != COMMITTED {
        let line = planned
            .text
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-surface/src/tables/bowl_permanent_shadow.rs differs from the fit at \
             {line}: run `just fit bowl_permanent_shadow`"
        );
    }
    assert!(!planned.body_changed);
}
