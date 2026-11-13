//! The helpers that the fitting crate's integration tests share: a table's declared items, and the
//! check that a rerendered table is the committed one.
#![allow(
    dead_code,
    reason = "each test binary compiles this module and uses its own subset of the helpers"
)]

/// The declared items of a table's `body`, in order: the name of every `pub const` or `pub static`
/// item, the text of its line before the first `:`.
#[must_use]
pub fn declared(body: &str) -> Vec<&str> {
    body.lines()
        .filter_map(|l| {
            l.strip_prefix("pub const ")
                .or_else(|| l.strip_prefix("pub static "))
        })
        .filter_map(|l| l.split(':').next())
        .collect()
}

/// The first line at which `committed` and `rendered` differ, compared by [`str::lines`]: its
/// number, counted from 1, then the committed line, then the rendered one.
///
/// `None` when every line of the shorter text matches the longer's: the texts are equal, differ
/// only in length, or differ only in line endings, which [`str::lines`] does not see.
#[must_use]
pub fn first_difference<'a>(
    committed: &'a str,
    rendered: &'a str,
) -> Option<(usize, &'a str, &'a str)> {
    committed
        .lines()
        .zip(rendered.lines())
        .enumerate()
        .find(|(_, (c, r))| c != r)
        .map(|(n, (c, r))| (n + 1, c, r))
}

/// Asserts that the table `rendered` by task `task` is `committed`, the text of the file at `path`
/// from the repository's root.
///
/// # Panics
///
/// If the texts differ. The message names the first line that differs ([`first_difference`]), or
/// the length when none does, and the `just fit` command that refits the table.
#[track_caller]
pub fn assert_reproduced(path: &str, task: &str, committed: &str, rendered: &str) {
    if rendered == committed {
        return;
    }
    let line = first_difference(committed, rendered)
        .map_or_else(|| "the length".to_owned(), |(n, _, _)| format!("line {n}"));
    panic!("{path} differs from the fit at {line}: run `just fit {task}`");
}
