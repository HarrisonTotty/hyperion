//! The committed luminous-blue-variable sampler is exactly what `run lbv --since <current>` would
//! write (plan 15, P15.T10.a).
//!
//! The rerun scans some 55,000 tracks, so it is slow here; `check --rerun-fast` reruns it in the
//! slow tier too. The sim's slow `the_table_is_the_scan_of_plan_06_s_tracks` rebuilds the nodes
//! from the table's own scan constants, and its slow
//! `the_class_test_holds_every_host_of_a_million_layer_e_stars` is the plan's acceptance.

mod common;

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/lbv.rs");

#[test]
#[ignore = "slow: the scan of some 55,000 tracks that writes the table"]
fn lbv_table_is_reproduced() {
    let threads = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let planned = rerender(
        registry(),
        find("lbv").expect("lbv is registered"),
        &Workspace::repository(),
        threads,
        GENERATOR_VERSION.get(),
    )
    .expect("the lbv task runs");
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/lbv.rs",
        "lbv",
        COMMITTED,
        &planned.text,
    );
}
