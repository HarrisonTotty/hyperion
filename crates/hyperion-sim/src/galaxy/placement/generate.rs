//! Generating a whole cell: every system a generation cell holds, and what one costs to keep (plan
//! 03, P03.T6).

use super::cell::{candidate_count_from_bound, layer_bound};
use super::{CandidateOutcome, CellKey, SystemRecord, evaluate_candidate_from_bound};
use crate::galaxy::Galaxy;

/// Places every system of one generation cell into `out`, in candidate-index order.
///
/// `out` is cleared first and then reserved from the cell's candidate count, so a caller that
/// generates many cells reuses one buffer. The cell's density bound is evaluated once and shared by
/// the candidate count and every candidate's thinning, which is what makes this cheaper than
/// [`evaluate_candidate`](super::evaluate_candidate) in a loop.
///
/// A cell is always generated whole. That is what a cache must hold: eviction has to be safe, so a
/// partly generated cell may never be stored (brainstorm, "Runtime and code shape"). It also makes
/// the result independent of any query: two queries that reach the same cell see the same systems in
/// the same order, whatever their spheres.
///
/// # Panics
///
/// If the cell's mean candidate count exceeds
/// [`POISSON_MAX_MEAN`](crate::rng::POISSON_MAX_MEAN), which
/// [`check_index_headroom`](super::check_index_headroom) rules out for every galaxy that is played.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, cell_heap_bytes, generate_cell};
/// use hyperion_sim::id::Layer;
///
/// let galaxy = Galaxy::new(Seed::new(19));
/// let mut cell = Vec::new();
/// // A 128 ly cell at the solar circle: layer E holds some tens of systems.
/// generate_cell(&galaxy, CellKey::new(Layer::E, [0, 203, 0])?, &mut cell);
/// assert!(!cell.is_empty());
/// // Records come out in candidate-index order, so a cache can search them by ID.
/// assert!(cell.windows(2).all(|pair| pair[0].id() < pair[1].id()));
/// // What a cache pays to keep the cell.
/// assert_eq!(cell_heap_bytes(&cell), cell.len() * size_of_val(&cell[0]));
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
pub fn generate_cell(galaxy: &Galaxy, key: CellKey, out: &mut Vec<SystemRecord>) {
    out.clear();
    let bound = layer_bound(galaxy, key);
    let candidates = candidate_count_from_bound(galaxy.seed(), key, bound);
    // Most candidates are accepted, so the count is the right order for the reservation; a cache
    // that keeps the buffer should shrink it, since `cell_heap_bytes` counts records and not spare
    // capacity.
    out.reserve(
        usize::try_from(candidates).expect("usize is at least 32 bits, so a u32 count fits"),
    );
    for index in 0..candidates {
        if let CandidateOutcome::Accepted(system) =
            evaluate_candidate_from_bound(galaxy, key, index, bound)
        {
            out.push(system);
        }
    }
}

/// What a cell's systems cost to keep: the bytes of the records themselves.
///
/// This is what a cache bounded by bytes adds up (brainstorm, "Runtime and code shape": the caches
/// "are bounded by bytes, not entries, because a bulge cell is ten thousand times heavier than a rim
/// cell"). It counts the records and neither the spare capacity of the vector they arrived in nor
/// the cache's own per-entry overhead, so a cache that keeps a generating buffer either shrinks it to
/// fit or adds that spare capacity itself.
#[must_use]
pub fn cell_heap_bytes(systems: &[SystemRecord]) -> usize {
    systems.len().saturating_mul(size_of::<SystemRecord>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{STELLAR_LAYERS, candidate_count, evaluate_candidate};
    use crate::id::Layer;
    use crate::rng::Seed;

    /// The seed of the galaxy these tests generate cells of.
    const SEED: u64 = 0x0300_6e11_0000_0000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
    }

    fn sunlike() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    #[test]
    fn generate_cell_holds_exactly_the_accepted_candidates_in_index_order() {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        for spec in &STELLAR_LAYERS {
            let key = CellKey::containing(spec.layer(), &sunlike()).unwrap();
            generate_cell(&galaxy, key, &mut cell);
            let one_at_a_time: Vec<_> = (0..candidate_count(&galaxy, key))
                .filter_map(|index| match evaluate_candidate(&galaxy, key, index) {
                    CandidateOutcome::Accepted(system) => Some(system),
                    CandidateOutcome::Thinned | CandidateOutcome::ClaimedByCatalogue => None,
                })
                .collect();
            assert_eq!(cell, one_at_a_time, "{:?}", spec.layer());
            assert!(cell.windows(2).all(|pair| pair[0].id() < pair[1].id()));
        }
    }

    #[test]
    fn generate_cell_clears_what_it_was_given() {
        let galaxy = galaxy();
        let sparse = CellKey::new(Layer::A, [-8_000, 40, 6_000]).unwrap();
        let dense = CellKey::containing(Layer::C, &sunlike()).unwrap();
        let mut cell = Vec::new();
        generate_cell(&galaxy, dense, &mut cell);
        let expected = cell.clone();
        assert!(!expected.is_empty());
        generate_cell(&galaxy, sparse, &mut cell);
        generate_cell(&galaxy, dense, &mut cell);
        assert_eq!(cell, expected);
    }

    #[test]
    fn cell_heap_bytes_counts_the_records() {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::containing(Layer::D, &sunlike()).unwrap(),
            &mut cell,
        );
        assert!(!cell.is_empty());
        assert_eq!(
            cell_heap_bytes(&cell),
            cell.len() * size_of::<SystemRecord>()
        );
        assert_eq!(cell_heap_bytes(&[]), 0);
    }
}
