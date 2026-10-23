//! The committed streams table is exactly what `run streams --since <current>` would write (plan
//! 15, P15.T11.a), and its multiplier is the brainstorm's 1.5, bit for bit.
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

mod common;

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::tables::streams::ORPHAN_STREAMS_PER_GLOBULAR;
use hyperion_testkit::float::assert_same_bits;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/streams.rs");

#[test]
fn streams_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("streams").expect("streams is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the streams task runs");
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/streams.rs",
        "streams",
        COMMITTED,
        &planned.text,
    );
}

/// The multiplier the sim compiles is the brainstorm's 1.5 ("Streams and accreted structure"),
/// bit for bit. The literal is the test's own, so that a changed manifest cannot pass by its own
/// re-emission.
#[test]
fn the_committed_multiplier_is_the_brainstorms_one_and_a_half() {
    assert_same_bits(ORPHAN_STREAMS_PER_GLOBULAR, 1.5);
}
