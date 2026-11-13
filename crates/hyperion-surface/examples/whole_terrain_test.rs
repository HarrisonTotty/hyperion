//! Writes `TERRAIN TEST`'s whole body for the views check's whole-body run (plan R10, R10.T14.c;
//! research-leans-20 §2): the Moon-like world with every cell surveyed, in the terrain golden's
//! format (`hyperion_surface::testing::terrain_test_text`), to the path given, which `just
//! views-check --terrain --whole` names `target/views-check/terrain/whole.golden`. The file is
//! about 4.5 MB and never committed; the client's module and bundle carry nothing of it.
//!
//! ```text
//! cargo run -q -p hyperion-surface --features testing --example whole_terrain_test -- <path>
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use hyperion_surface::testing::{
    SyntheticWorld, TERRAIN_TEST_SEED, synthetic_field, terrain_test_text,
    whole_terrain_test_fixture,
};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(path), None) = (args.next().map(PathBuf::from), args.next()) else {
        eprintln!("usage: whole_terrain_test <path>");
        return ExitCode::from(2);
    };
    let field = synthetic_field(SyntheticWorld::MoonLike);
    let fixture = whole_terrain_test_fixture(&field);
    let text = terrain_test_text(&fixture, &field, TERRAIN_TEST_SEED);
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!("whole_terrain_test: {}: {error}", parent.display());
        return ExitCode::FAILURE;
    }
    match std::fs::write(&path, text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("whole_terrain_test: {}: {error}", path.display());
            ExitCode::FAILURE
        }
    }
}
