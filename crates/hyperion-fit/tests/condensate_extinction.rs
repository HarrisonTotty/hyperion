//! The committed aerosol-extinction tables are exactly what `run condensate_extinction --since
//! <current>` and its six other parts would write (plan 14, P14.T49.e; plan 15, P15.T2), from
//! rendering plan R08's material files as they stand.
//!
//! CI runs this, so a material file whose n or k changed without a rerun, or a table edited by
//! hand, fails it.

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The tables the sim compiles, as committed, by task.
const COMMITTED: [(&str, &str); 7] = [
    (
        "condensate_extinction",
        include_str!("../../hyperion-sim/src/tables/condensate_extinction.rs"),
    ),
    (
        "condensate_extinction_2",
        include_str!("../../hyperion-sim/src/tables/condensate_extinction_2.rs"),
    ),
    (
        "condensate_extinction_3",
        include_str!("../../hyperion-sim/src/tables/condensate_extinction_3.rs"),
    ),
    (
        "condensate_extinction_4",
        include_str!("../../hyperion-sim/src/tables/condensate_extinction_4.rs"),
    ),
    (
        "condensate_extinction_5",
        include_str!("../../hyperion-sim/src/tables/condensate_extinction_5.rs"),
    ),
    (
        "condensate_extinction_6",
        include_str!("../../hyperion-sim/src/tables/condensate_extinction_6.rs"),
    ),
    (
        "condensate_extinction_7",
        include_str!("../../hyperion-sim/src/tables/condensate_extinction_7.rs"),
    ),
];

/// Reruns `task` on one thread and compares its table with the committed `text`.
fn assert_reproduced(task: &str, text: &str) {
    let planned = rerender(
        registry(),
        find(task).expect("every part is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the task runs");
    if planned.text != text {
        let line = planned
            .text
            .lines()
            .zip(text.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/{task}.rs differs from the fit at {line}: run `just fit \
             {task}`"
        );
    }
    assert!(!planned.body_changed, "{task}");
}

// One test a table, so that the harness runs them side by side.

#[test]
fn condensate_extinction_table_is_reproduced() {
    assert_reproduced(COMMITTED[0].0, COMMITTED[0].1);
}

#[test]
fn condensate_extinction_2_table_is_reproduced() {
    assert_reproduced(COMMITTED[1].0, COMMITTED[1].1);
}

#[test]
fn condensate_extinction_3_table_is_reproduced() {
    assert_reproduced(COMMITTED[2].0, COMMITTED[2].1);
}

#[test]
fn condensate_extinction_4_table_is_reproduced() {
    assert_reproduced(COMMITTED[3].0, COMMITTED[3].1);
}

#[test]
fn condensate_extinction_5_table_is_reproduced() {
    assert_reproduced(COMMITTED[4].0, COMMITTED[4].1);
}

#[test]
fn condensate_extinction_6_table_is_reproduced() {
    assert_reproduced(COMMITTED[5].0, COMMITTED[5].1);
}

#[test]
fn condensate_extinction_7_table_is_reproduced() {
    assert_reproduced(COMMITTED[6].0, COMMITTED[6].1);
}
