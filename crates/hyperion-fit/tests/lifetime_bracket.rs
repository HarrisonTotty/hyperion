//! The committed lifetime bracket is exactly what `run lifetime_bracket --since <current>` would
//! write (plan 08, P08.T9.d's follow-up, main's ruling on P08.T13's cost, 2026-10-15).
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it. The sim's
//! `the_shared_bracket_is_new_bit_for_bit` holds the table to a fresh `LifetimeBracket::new()`.

mod common;

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/lifetime_bracket.rs");

#[test]
fn lifetime_bracket_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("lifetime_bracket").expect("lifetime_bracket is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the lifetime_bracket task runs");
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/lifetime_bracket.rs",
        "lifetime_bracket",
        COMMITTED,
        &planned.text,
    );
}
