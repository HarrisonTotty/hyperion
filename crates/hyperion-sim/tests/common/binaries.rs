//! The binary classes' helpers (plan 11's test helpers, P11.T8.a): the prior members of a carved
//! class, drawn with no carve-out, against which a class's sampler is checked. The members are
//! the sim's own `stellar::binary::testing::brute_force_class_members`, which plan 15's P15.T10.b
//! calls too; this splits the draw between threads.

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::stellar::binary::CarvedClass;
use hyperion_sim::stellar::binary::testing::{
    ClassMember, MergerMember, brute_force_class_members_in, brute_force_merger_members_in,
};

/// The streams a draw of members is split between: sixteen, whatever the cores, so that the
/// members are the same on every machine.
pub const MEMBER_STREAMS: u64 = 16;

/// `n` prior members of `class` near the solar circle, drawn on [`MEMBER_STREAMS`] streams at once
/// (one thread each; on wasm32-wasip1, in turn) and joined in stream order.
pub fn brute_force_class_members(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
) -> Vec<ClassMember> {
    let streams = usize::try_from(MEMBER_STREAMS).expect("sixteen streams");
    let share = n.div_ceil(streams);
    let mut members = Vec::with_capacity(n + streams);
    #[cfg(not(target_family = "wasm"))]
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..MEMBER_STREAMS)
            .map(|s| scope.spawn(move || brute_force_class_members_in(galaxy, class, share, s)))
            .collect();
        for handle in handles {
            members.extend(handle.join().expect("a member draw finishes"));
        }
    });
    #[cfg(target_family = "wasm")]
    for s in 0..MEMBER_STREAMS {
        members.extend(brute_force_class_members_in(galaxy, class, share, s));
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
