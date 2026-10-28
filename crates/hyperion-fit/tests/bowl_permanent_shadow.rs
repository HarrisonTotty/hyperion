//! The committed table of a spherical bowl's permanent shadow is exactly what
//! `run bowl_permanent_shadow --since <current>` would write (rendering plan R09, R09.T7.f).
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

mod common;

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The table the surface crate compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-surface/src/tables/bowl_permanent_shadow.rs");

/// The task's threads: a fixed few, not the machine's, since other tests run beside this one.
///
/// The table is the same for any count
/// ([`map_reduce_chunks`](hyperion_fit::parallel::map_reduce_chunks)).
const THREADS: NonZeroUsize = NonZeroUsize::new(2).expect("2 is not zero");

#[test]
fn bowl_permanent_shadow_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("bowl_permanent_shadow").expect("bowl_permanent_shadow is registered"),
        &Workspace::repository(),
        THREADS,
        GENERATOR_VERSION.get(),
    )
    .expect("the bowl_permanent_shadow task runs");
    common::assert_reproduced(
        "crates/hyperion-surface/src/tables/bowl_permanent_shadow.rs",
        "bowl_permanent_shadow",
        COMMITTED,
        &planned.text,
    );
    assert!(!planned.body_changed);
}
