//! The engine's own mergers against which the stellar merger class's channel holds measure it
//! (P11.T8.c's fix at 26 and its missing channel at 27): the members and the decile chi-square
//! that `channels_hold` and `engine_channels_hold` share.

use crate::galaxy::Galaxy;
use crate::stellar::binary::CarvedClass;
use crate::stellar::binary::testing::{MergerMember, brute_force_merger_members_in};

/// `n` engine mergers of the stellar merger class in `galaxy`, drawn on 16 streams at once and
/// joined in stream order, as `tests/common/binaries.rs` draws them.
pub(super) fn members(galaxy: &Galaxy, n: usize) -> Vec<MergerMember> {
    let share = n.div_ceil(16);
    let mut out = Vec::with_capacity(n + 16);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16_u64)
            .map(|s| {
                scope.spawn(move || {
                    brute_force_merger_members_in(galaxy, CarvedClass::StellarMerger, share, s)
                })
            })
            .collect();
        for handle in handles {
            out.extend(handle.join().expect("a member draw finishes"));
        }
    });
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
