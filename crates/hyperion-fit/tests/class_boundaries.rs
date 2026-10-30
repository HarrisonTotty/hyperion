//! The committed class boundaries are exactly what `run class_boundaries --since <current>` would
//! write from the transcription of Straižys and Kuriliene's (1981) tables (plan 06, P06.T23.b; plan
//! 15, P15.T2).
//!
//! The transcription is not redistributed (`tasks::class_boundaries`), so this test needs it in
//! `crates/hyperion-fit/data/cache/straizys_kuriliene_1981/`. Without it the test says so on stderr
//! and checks nothing more; the sim's own tests of the boundaries (`stellar::classify::luminosity`)
//! run either way.

mod common;

use std::num::NonZeroUsize;
use std::path::Path;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::class_boundaries::{DEFAULT_DATA_DIR, FILE};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/class_boundaries.rs");

#[test]
fn class_boundaries_table_is_reproduced() {
    if !Path::new(DEFAULT_DATA_DIR).join(FILE).exists() {
        eprintln!("the transcription is not in {DEFAULT_DATA_DIR}: the table is not re-rendered");
        return;
    }
    let rendered = rerender(
        registry(),
        find("class_boundaries").expect("class_boundaries is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the class_boundaries task runs on the transcription")
    .text;
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/class_boundaries.rs",
        "class_boundaries",
        COMMITTED,
        &rendered,
    );
}
