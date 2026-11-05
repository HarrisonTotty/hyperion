//! The binary classes' helpers (plan 11's test helpers, P11.T8.a–d): the prior members of a carved
//! class, drawn with no carve-out, against which a class's sampler is checked, the mergers among
//! them weighted in the source horizon, and the chi-square that checks a sampler
//! ([`decile_chi_square`]). The members are the sim's own
//! `stellar::binary::testing::brute_force_class_members`, which plan 15's P15.T10.b calls too;
//! this splits the draw between threads.

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::stellar::binary::CarvedClass;
use hyperion_sim::stellar::binary::testing::{
    ClassMember, MemberCells, MergerMember, brute_force_merger_members_in,
};
use hyperion_testkit::stats::chi_square_gof;

/// The streams a draw of members is split between: sixteen, whatever the cores, so that the
/// members are the same on every machine.
pub const MEMBER_STREAMS: u64 = 16;

/// `n` prior members of `class` near the solar circle, drawn on [`MEMBER_STREAMS`] streams at once
/// (one thread each; on wasm32-wasip1, in turn) from cells they share (`MemberCells`), and joined
/// in stream order.
///
/// A stream draws its share of `n` layer by layer, each layer's part of it rounded
/// (`MemberCells::members_in`), so a row whose layers' parts round down gives a stream fewer: the
/// share is raised until the streams give `n` (from 27 the fast accreting white dwarfs' fitted
/// row, 0.108, 0.868 and 0.024 of layers C–E, gives 18 of a stream's 19, and 288 of 300). A larger
/// share keeps each stream's members of a layer and draws more after them, from the cells kept.
pub fn brute_force_class_members(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
) -> Vec<ClassMember> {
    let streams = usize::try_from(MEMBER_STREAMS).expect("sixteen streams");
    let cells = MemberCells::new(galaxy);
    let draw = |share: usize| {
        let mut members = Vec::with_capacity(share * streams);
        #[cfg(not(target_family = "wasm"))]
        std::thread::scope(|scope| {
            let cells = &cells;
            let handles: Vec<_> = (0..MEMBER_STREAMS)
                .map(|s| scope.spawn(move || cells.members_in(class, share, s)))
                .collect();
            for handle in handles {
                members.extend(handle.join().expect("a member draw finishes"));
            }
        });
        #[cfg(target_family = "wasm")]
        for s in 0..MEMBER_STREAMS {
            members.extend(cells.members_in(class, share, s));
        }
        members
    };
    let mut share = n.div_ceil(streams);
    let mut members = draw(share);
    while members.len() < n {
        share += 1;
        members = draw(share);
    }
    members.truncate(n);
    members
}

/// `n` of the engine's mergers of `class` near the solar circle, each weighted as a merger in the
/// source horizon (`stellar::binary::testing::brute_force_merger_members_in`, P11.T8.c), drawn on
/// [`MEMBER_STREAMS`] streams at once and joined in stream order.
pub fn brute_force_merger_members(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
) -> Vec<MergerMember> {
    let streams = usize::try_from(MEMBER_STREAMS).expect("sixteen streams");
    let share = n.div_ceil(streams);
    let mut members = Vec::with_capacity(n + streams);
    #[cfg(not(target_family = "wasm"))]
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..MEMBER_STREAMS)
            .map(|s| scope.spawn(move || brute_force_merger_members_in(galaxy, class, share, s)))
            .collect();
        for handle in handles {
            members.extend(handle.join().expect("a member draw finishes"));
        }
    });
    #[cfg(target_family = "wasm")]
    for s in 0..MEMBER_STREAMS {
        members.extend(brute_force_merger_members_in(galaxy, class, share, s));
    }
    members.truncate(n);
    members
}

/// The chi-square per degree of freedom of `observed` against `expected`'s deciles, over the finite
/// values of both, its degrees of freedom and the observed values counted: the samplers' test
/// against the engine's members (P11.T8.a and P11.T8.b). `None` with fewer than 50 finite expected
/// values or 10 observed ones, too few to bin.
pub fn decile_chi_square(expected: &[f64], observed: &[f64]) -> Option<(f64, u32, usize)> {
    let mut sorted: Vec<f64> = expected.iter().copied().filter(|v| v.is_finite()).collect();
    let observed: Vec<f64> = observed.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.len() < 50 || observed.len() < 10 {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    let edges: Vec<f64> = (1..10).map(|k| sorted[k * sorted.len() / 10]).collect();
    let mut counts = [0_u64; 10];
    for &v in &observed {
        counts[edges.partition_point(|&e| e <= v)] += 1;
    }
    #[expect(clippy::cast_precision_loss, reason = "a count of a few hundred")]
    let each = observed.len() as f64 / 10.0;
    let fit = chi_square_gof(&counts, &[each; 10]);
    Some((fit.statistic / f64::from(fit.dof), fit.dof, observed.len()))
}
