//! The committed dwarf sequence is exactly what `run dwarf_sequence --since <current>` would write
//! from Mamajek's file (plan 06, P06.T23.a; plan 15, P15.T2), and its grids follow the file's rows.
//!
//! The file is not redistributed (`tasks::dwarf_sequence`), so these tests need it in
//! `crates/hyperion-fit/data/cache/mamajek_dwarf_sequence/`. Without it each test says so on
//! stderr and checks nothing more; the sim's own tests of the grids (`stellar::classify::pm13`'s
//! hold) run either way, against the rows it still holds.

mod common;

use std::num::NonZeroUsize;
use std::path::Path;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::dwarf_sequence::{
    ACCEPTANCE, DEFAULT_DATA_DIR, FILE, ROWS, fit, parse, passes,
};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/dwarf_sequence.rs");

/// The file's text, if it has been downloaded.
fn source() -> Option<String> {
    let path = Path::new(DEFAULT_DATA_DIR).join(FILE);
    if path.exists() {
        Some(std::fs::read_to_string(path).expect("the downloaded file reads"))
    } else {
        eprintln!("Mamajek's file is not in {DEFAULT_DATA_DIR}: the table is not re-rendered");
        None
    }
}

#[test]
fn dwarf_sequence_table_is_reproduced() {
    if source().is_none() {
        return;
    }
    let rendered = rerender(
        registry(),
        find("dwarf_sequence").expect("dwarf_sequence is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the dwarf_sequence task runs on the downloaded file")
    .text;
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/dwarf_sequence.rs",
        "dwarf_sequence",
        COMMITTED,
        &rendered,
    );
}

/// The file's 118 rows read, and every grid lies within its bound of them.
#[test]
fn the_grids_follow_the_rows() {
    let Some(text) = source() else {
        return;
    };
    let rows = parse(&text).expect("the file parses");
    assert_eq!(rows.len(), ROWS);
    let fit = fit(&rows);
    eprintln!("{:?}", fit.worst);
    assert!(passes(&fit), "{:?} against {ACCEPTANCE:?}", fit.worst);
}
