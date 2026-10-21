//! The committed liquids' bins are exactly what `run liquid_optics --since <current>` would write
//! (plan 14, P14.T49.h; plan 15, P15.T2), from rendering plan R08's material files as they stand.
//!
//! CI runs this, so a material file whose n or k changed without a rerun, or a table edited by
//! hand, fails it.

mod common;

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/liquid_optics.rs");

#[test]
fn liquid_optics_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("liquid_optics").expect("liquid_optics is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the liquid_optics task runs");
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/liquid_optics.rs",
        "liquid_optics",
        COMMITTED,
        &planned.text,
    );
    assert!(!planned.body_changed);
}
