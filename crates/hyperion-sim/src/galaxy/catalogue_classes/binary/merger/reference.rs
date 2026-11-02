//! The engine's own mergers against which the stellar merger class's channel holds measure it
//! (P11.T8.c's fix at 26 and its missing channel at 27): the members and the decile chi-square
//! that `channels_hold` and `engine_channels_hold` share.

use crate::galaxy::Galaxy;
use crate::stellar::binary::CarvedClass;
use crate::stellar::binary::testing::{MergerMember, brute_force_merger_members_in};

/// `n` engine mergers of the stellar merger class in `galaxy`, drawn on 16 streams at once (one
/// thread each; on WebAssembly, which has no threads, in turn) and joined in stream order, as
/// `tests/common/binaries.rs` draws them.
pub(super) fn members(galaxy: &Galaxy, n: usize) -> Vec<MergerMember> {
    members_as(galaxy, n, MergerMember::clone)
}

/// [`members`], each as `mark` makes it, every stream's members dropped once marked: a merger's
/// timeline is some 90 kB natively, so the 48,000 of `engine_channels_hold` held whole came to
/// 4.3 GB, past wasm32's 4 GiB, where the streams run in turn and so hold one stream's at a time.
pub(super) fn members_as<T: Send>(
    galaxy: &Galaxy,
    n: usize,
    mark: impl Fn(&MergerMember) -> T + Sync,
) -> Vec<T> {
    let share = n.div_ceil(16);
    let stream = |s: u64| -> Vec<T> {
        brute_force_merger_members_in(galaxy, CarvedClass::StellarMerger, share, s)
            .iter()
            .map(&mark)
            .collect()
    };
    let mut out = Vec::with_capacity(n + 16);
    #[cfg(not(target_family = "wasm"))]
    std::thread::scope(|scope| {
        let stream = &stream;
        let handles: Vec<_> = (0..16_u64)
            .map(|s| scope.spawn(move || stream(s)))
            .collect();
        for handle in handles {
            out.extend(handle.join().expect("a member draw finishes"));
        }
    });
    #[cfg(target_family = "wasm")]
    for s in 0..16_u64 {
        out.extend(stream(s));
    }
    out.truncate(n);
    out
}

/// The chi-square per degree of freedom of the weighted `observed` values in `expected`'s
/// deciles, the weighted counts scaled to the members' effective number, and that number.
pub(super) fn decile_chi_square(expected: &[f64], observed: &[(f64, f64)]) -> (f64, f64) {
    let mut sorted = expected.to_vec();
    sorted.sort_by(f64::total_cmp);
    let edges: Vec<f64> = (1..10).map(|k| sorted[k * sorted.len() / 10]).collect();
    let mut weights = [0.0_f64; 10];
    for &(v, w) in observed {
        weights[edges.partition_point(|&e| e <= v)] += w;
    }
    let total: f64 = observed.iter().map(|(_, w)| w).sum();
    let squares: f64 = observed.iter().map(|(_, w)| w * w).sum();
    let effective = total * total / squares;
    let each = effective / 10.0;
    let statistic: f64 = weights
        .iter()
        .map(|w| {
            let o = w / total * effective;
            (o - each) * (o - each) / each
        })
        .sum();
    (statistic / 9.0, effective)
}
