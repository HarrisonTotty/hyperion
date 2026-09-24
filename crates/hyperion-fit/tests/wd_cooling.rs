//! The committed white-dwarf cooling table is exactly what `run wd_cooling` produces from the
//! Montreal sequences (plan 06, P06.T20.a), and it reproduces them, the held-out models included.
//!
//! The sequences are not redistributed (`tasks::wd_cooling`), so these tests need them in
//! `target/data/montreal_cooling/`, downloaded from the Montreal group's site. Without them each
//! test says so on stderr and checks nothing more; the sim's own tests of the table
//! (`stellar::remnant::cooling`) run either way, against held-out models they quote.

use std::path::Path;

use hyperion_fit::tasks::wd_cooling::{
    DEFAULT_DATA_DIR, LUMINOSITIES, SEQUENCES, Sequences, fit, read, render,
};

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/wd_cooling.rs");

/// The sequences, if they have been downloaded.
fn sequences() -> Option<Sequences> {
    let dir = Path::new(DEFAULT_DATA_DIR);
    if dir.join("seq_020_thick.txt").exists() {
        Some(read(dir).expect("the downloaded sequences read"))
    } else {
        eprintln!(
            "the Montreal sequences are not in {DEFAULT_DATA_DIR}: the table is not re-rendered"
        );
        None
    }
}

#[test]
fn wd_cooling_table_is_reproduced() {
    let Some(sequences) = sequences() else {
        return;
    };
    let digest = format!("{:#018x}", sequences.digest);
    assert!(
        COMMITTED.contains(&digest),
        "the downloaded sequences' digest {digest} is not the committed table's: they differ from \
         the files it was fitted to"
    );
    let rendered = render(&fit(&sequences));
    if rendered != COMMITTED {
        let line = rendered
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/wd_cooling.rs differs from the fit at {line}: run \
             `cargo run -p hyperion-fit -- run wd_cooling`"
        );
    }
}

/// The table follows the sequences to 0.03 dex in luminosity at every model, held out or not,
/// and to 0.002 dex in the mean; and every column falls with luminosity.
#[test]
fn the_table_reproduces_the_sequences() {
    let Some(sequences) = sequences() else {
        return;
    };
    let table = fit(&sequences);
    eprintln!("{:?}\n{:?}", table.fitted, table.held_out);
    let rows: usize = sequences.models.iter().map(Vec::len).sum();
    assert_eq!(
        usize::try_from(table.fitted.rows + table.held_out.rows).unwrap(),
        rows
    );
    assert!(table.held_out.rows > 1_000, "{:?}", table.held_out);
    for r in [table.fitted, table.held_out] {
        assert!(r.worst.residual.abs() < 0.03, "{r:?}");
        assert!(r.rms < 0.002, "{r:?}");
    }
    for column in &table.log_clock {
        assert_eq!(column.len(), LUMINOSITIES);
        assert!(column.windows(2).all(|pair| pair[1] < pair[0]));
    }
    assert_eq!(table.log_clock.len(), SEQUENCES);
    assert_eq!(fit(&sequences), table, "the fit is deterministic");
}
