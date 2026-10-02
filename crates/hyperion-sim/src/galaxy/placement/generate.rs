//! Generating a whole cell: every system a generation cell holds, and what one costs to keep (plan
//! 03, P03.T6).

use super::candidate::candidate_id;
use super::cell::{candidate_count_from_bound, layer_bound};
use super::record::candidate_mass;
use super::{CandidateOutcome, CellKey, SystemRecord, evaluate_candidate_from_bound};
use crate::galaxy::Galaxy;
use crate::units::SolarMasses;

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

/// Places into `out` the systems of one generation cell whose initial mass `keep` accepts, in
/// candidate-index order: [`generate_cell`] followed by a filter on
/// [`SystemRecord::primary_initial_mass`], bit for bit (rendering plan R06, Design note 8).
///
/// It is cheaper because it draws each candidate's mass first. The mass is one word on the
/// candidate's own mass stream, keyed by its ID and read through the layer's band, so it depends on
/// neither the candidate's position, nor its acceptance mark, nor the component the thinning picks.
/// A candidate whose mass `keep` refuses is dropped before its position is drawn or the density
/// evaluated at it; one that `keep` accepts is evaluated whole, as [`generate_cell`] evaluates it,
/// which draws its mass a second time from the same word. So the records kept are the very records
/// [`generate_cell`] makes, and in the same order; nothing here opens a stream the generator does
/// not, or draws a word it would not, so no generated output depends on it.
///
/// `keep` sees the mass of every candidate of the cell, thinned or not, in index order; the sky's
/// census passes a mass floor. `out` is cleared first and reserved as [`generate_cell`] reserves it.
///
/// # Panics
///
/// As [`generate_cell`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell, generate_cell_where};
/// use hyperion_sim::id::Layer;
///
/// let galaxy = Galaxy::new(Seed::new(19));
/// let key = CellKey::new(Layer::E, [0, 203, 0])?;
/// let mut all = Vec::new();
/// generate_cell(&galaxy, key, &mut all);
/// let mut heavy = Vec::new();
/// generate_cell_where(&galaxy, key, |mass| mass.value() >= 2.0, &mut heavy);
/// all.retain(|system| system.primary_initial_mass().value() >= 2.0);
/// assert_eq!(heavy, all);
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
pub fn generate_cell_where(
    galaxy: &Galaxy,
    key: CellKey,
    keep: impl Fn(SolarMasses) -> bool,
    out: &mut Vec<SystemRecord>,
) {
    out.clear();
    let bound = layer_bound(galaxy, key);
    let candidates = candidate_count_from_bound(galaxy.seed(), key, bound);
    out.reserve(
        usize::try_from(candidates).expect("usize is at least 32 bits, so a u32 count fits"),
    );
    for index in 0..candidates {
        if !keep(candidate_mass(galaxy, key, candidate_id(key, index))) {
            continue;
        }
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
    use hyperion_testkit::float::assert_same_bits;

    use crate::galaxy::placement::{
        STELLAR_LAYERS, SUBSTELLAR_LAYERS, candidate_count, evaluate_candidate,
    };
    use crate::id::Layer;
    use crate::rng::Seed;

    /// The seed of the galaxy these tests generate cells of.
    const SEED: u64 = 0x0300_6e11_0000_0000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn sunlike() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// A point in the bulge, 3,000 ly out along +y, clear of the nuclear disc, whose cells hold
    /// about two thousand systems in layer E at most.
    fn in_the_bulge() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 3_000.0, 0.0]).unwrap()
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

    /// The ten mass floors the filtered walk is tested at, M☉: zero, then from the substellar
    /// masses through every stellar band to above the heaviest primary.
    const FLOORS: [f64; 10] = [0.0, 0.003, 0.03, 0.1, 0.5, 1.0, 2.0, 8.0, 30.0, 200.0];

    /// `count` cells of `layer` spread along a line through the root cube from edge to edge,
    /// stepping in x and z too, so that disc and halo cells, and cells with no candidate, are all
    /// among them. Cells within 2,000 ly of the centre are passed over: the nuclear disc's hold
    /// some 10⁵ systems each, which a debug build would take minutes over.
    fn spread_cells(layer: Layer, count: usize) -> Vec<CellKey> {
        let size = i32::try_from(layer.cell_size_ly()).unwrap();
        let edge = 65_536 / size;
        (0..)
            .map(|i| {
                let y = (i * 37) % (2 * edge) - edge;
                let x = (i * 11) % 7 - 3;
                let z = (i * 5) % 9 - 4;
                [x, y, z]
            })
            .filter(|[_, y, _]| (y * size).abs() >= 2_000)
            .map(|cell| CellKey::new(layer, cell).unwrap())
            .take(count)
            .collect()
    }

    #[test]
    fn generate_cell_where_is_generate_cell_filtered_by_mass() {
        let galaxy = galaxy();
        let mut all = Vec::new();
        let mut kept = Vec::new();
        let mut nonempty = 0;
        for spec in STELLAR_LAYERS.iter().chain(&SUBSTELLAR_LAYERS) {
            let layer = spec.layer();
            let mut keys = spread_cells(layer, 498);
            keys.push(CellKey::containing(layer, &sunlike()).unwrap());
            keys.push(CellKey::containing(layer, &in_the_bulge()).unwrap());
            for key in keys {
                generate_cell(&galaxy, key, &mut all);
                nonempty += usize::from(!all.is_empty());
                for floor in FLOORS {
                    generate_cell_where(&galaxy, key, |m| m.value() >= floor, &mut kept);
                    let filtered: Vec<_> = all
                        .iter()
                        .filter(|r| r.primary_initial_mass().value() >= floor)
                        .copied()
                        .collect();
                    assert_eq!(
                        kept,
                        filtered,
                        "layer {} {key:?} floor {floor}",
                        layer.letter()
                    );
                }
                generate_cell_where(&galaxy, key, |_| true, &mut kept);
                assert_eq!(kept, all, "layer {} {key:?}", layer.letter());
            }
        }
        assert!(
            nonempty > 500,
            "only {nonempty} of the cells hold any system"
        );
    }

    #[test]
    fn generate_cell_where_sees_every_candidate_once_in_index_order() {
        let galaxy = galaxy();
        let key = CellKey::containing(Layer::E, &sunlike()).unwrap();
        let seen = std::cell::RefCell::new(Vec::new());
        let mut kept = Vec::new();
        generate_cell_where(
            &galaxy,
            key,
            |m| {
                seen.borrow_mut().push(m);
                false
            },
            &mut kept,
        );
        assert!(kept.is_empty());
        let seen = seen.into_inner();
        let count = candidate_count(&galaxy, key);
        assert_eq!(seen.len(), usize::try_from(count).unwrap());
        let accepted: Vec<_> = (0..count)
            .filter_map(|index| match evaluate_candidate(&galaxy, key, index) {
                CandidateOutcome::Accepted(system) => Some((index, system.primary_initial_mass())),
                CandidateOutcome::Thinned | CandidateOutcome::ClaimedByCatalogue => None,
            })
            .collect();
        assert!(!accepted.is_empty());
        for (index, mass) in accepted {
            assert_same_bits(seen[usize::try_from(index).unwrap()].value(), mass.value());
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
