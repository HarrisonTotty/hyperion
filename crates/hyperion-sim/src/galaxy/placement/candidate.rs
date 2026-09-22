//! A candidate of a generation cell: where it lies, whether the thinning keeps it, and which
//! density component it belongs to (plan 03, P03.T4 and P03.T5.b).
//!
//! Steps 3 of the brainstorm's thinning ("Exact placement by thinning"): each candidate opens its
//! own streams, keyed by its ID, draws a uniform position in its cell, and survives with
//! probability density(position) ÷ bound. One mark does both halves of that at once. With the
//! weights `share × density` of the components at the candidate's position and the cell's bound B,
//! the mark is compared against the running sums of the weights over B: the candidate takes the
//! first component whose threshold exceeds the mark, and is thinned if none does (plan 03, Design
//! note 3). That is the brainstorm's "one uniform draw rejects a candidate or picks its class", and
//! the pick also settles the age distribution, since a component — an old thin sub-disc, a halo
//! component — has its own.
//!
//! Nothing here depends on the cell's other candidates, on the order they are evaluated in, or on
//! anything a caller has cached: a candidate is a pure function of the galaxy, its cell and its
//! index.

use super::record::SystemRecord;
use super::{CellKey, layer_bound};
use crate::coords::GalacticPosition;
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
#[cfg(doc)]
use crate::galaxy::shares::ShareMatrix;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::SystemId;
use crate::rng::{Mark, ObjectKey, Seed, Stream, tags};

/// How far outside plan 02's bound the thinning pads it before drawing, relative: 10⁻¹²
/// (P03.T4.b).
///
/// Plan 02's bound already holds bit for bit against the weights this module folds
/// (`bounds.rs`, "A layer's bound"), so the padding is not needed for
/// [`Mark::pick_weighted`]'s own requirement. It is there to order two assertions: a violation of
/// up to 10⁻¹² beyond a bound that already carries plan 02's margins trips neither, and a larger
/// one trips this module's, which names the cell and the position, before `pick_weighted`'s, which
/// names neither. It changes an acceptance probability by at most one part in 10¹², which no test
/// can see, and it belongs to the generator version like the rest of the thinning.
const BOUND_PADDING: f64 = 1e-12;

/// What became of a candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CandidateOutcome {
    /// The thinning kept it: here is the system.
    Accepted(SystemRecord),
    /// The thinning rejected it. Most candidates end here; nothing else records them.
    Thinned,
    /// A catalogue class claimed it, so the field does not place it (plan 09). No class claims
    /// anything in the first milestone.
    ClaimedByCatalogue,
}

/// Evaluates one candidate of a cell: its position, the thinning, its marks and the carve-out hook.
///
/// This is what resolving an ID does, and what generating a cell does for each index in turn, so
/// the two always agree. It costs one density evaluation and four words, and it never generates the
/// rest of the cell. [`generate_cell`](super::generate_cell) is the way to place a whole cell: it
/// evaluates the cell's bound once instead of once per candidate.
///
/// # Panics
///
/// If `index` is at or above [`CellKey::index_capacity`], which no candidate of the cell reaches:
/// [`candidate_count`](super::candidate_count) is clamped to the capacity.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CandidateOutcome, CellKey, candidate_count, evaluate_candidate};
/// use hyperion_sim::id::Layer;
///
/// let galaxy = Galaxy::new(Seed::new(3));
/// let key = CellKey::new(Layer::C, [0, 812, 0])?;
/// let accepted = (0..candidate_count(&galaxy, key))
///     .filter(|&index| matches!(evaluate_candidate(&galaxy, key, index), CandidateOutcome::Accepted(_)))
///     .count();
/// // Thinning keeps a good fraction of a cell's candidates, and the answer does not change.
/// assert!(accepted > 0);
/// assert!(matches!(evaluate_candidate(&galaxy, key, 0), CandidateOutcome::Accepted(_) | CandidateOutcome::Thinned));
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[must_use]
pub fn evaluate_candidate(galaxy: &Galaxy, key: CellKey, index: u32) -> CandidateOutcome {
    evaluate_candidate_from_bound(galaxy, key, index, layer_bound(galaxy, key))
}

/// [`evaluate_candidate`] with the cell's bound already evaluated, for the whole-cell walk.
#[must_use]
pub(super) fn evaluate_candidate_from_bound(
    galaxy: &Galaxy,
    key: CellKey,
    index: u32,
    bound: f64,
) -> CandidateOutcome {
    evaluate_with_hook(galaxy, key, index, bound, catalogue_claims)
}

/// [`evaluate_candidate_from_bound`] with the carve-out hook given, so that a test can claim
/// candidates that no class claims yet (plan 03, Design note 5).
#[must_use]
fn evaluate_with_hook(
    galaxy: &Galaxy,
    key: CellKey,
    index: u32,
    bound: f64,
    claims: impl Fn(&Galaxy, &SystemRecord) -> bool,
) -> CandidateOutcome {
    let id = candidate_id(key, index);
    let position = candidate_position(galaxy.seed(), key, id);
    let mark = acceptance_mark(galaxy.seed(), id);
    let Some(component) = pick_component(galaxy, key, bound, &position, mark) else {
        return CandidateOutcome::Thinned;
    };
    let record = SystemRecord::of_candidate(galaxy, key, id, position, component);
    if claims(galaxy, &record) {
        CandidateOutcome::ClaimedByCatalogue
    } else {
        CandidateOutcome::Accepted(record)
    }
}

/// Whether a catalogue class claims this candidate, which none does before plan 09 (plan 03, Design
/// note 5).
///
/// The hook sits after the marks are drawn because a class is a deterministic test on a candidate's
/// own marks (brainstorm, "Rare events are found by carving out their hosts, not the events"), and
/// the cells draw conditional on not being in the class. Until plan 09 fills it in, the field keeps
/// every candidate it accepts, and [`CandidateOutcome::ClaimedByCatalogue`] never appears.
#[must_use]
fn catalogue_claims(_galaxy: &Galaxy, _record: &SystemRecord) -> bool {
    false
}

/// The ID of candidate `index` of `key`.
///
/// # Panics
///
/// If `index` is at or above the layer's index capacity, which names no candidate.
#[must_use]
fn candidate_id(key: CellKey, index: u32) -> SystemId {
    key.candidate_id(index).unwrap_or_else(|| {
        panic!(
            "index {index} is past layer {}'s capacity of {}",
            key.layer().letter(),
            key.index_capacity()
        )
    })
}

/// A candidate's position, drawn uniformly inside its cell (plan 03, Design note 2).
///
/// It is words 0, 1 and 2 of the candidate's `galaxy.candidate.position` stream, one per axis,
/// handed to plan 01's [`GenCell::position_from_words`](crate::coords::GenCell::position_from_words),
/// whose layout — the whole light-year in the top log₂(cell size) bits and the offset fraction in
/// the next 52 — is plan 01's and is not restated here. Cell sizes are powers of two, so the draw is
/// exactly uniform, and no `f64` ever spans a whole cell.
///
/// It takes the candidate's ID rather than its index because the ID is what keys the stream, and
/// every caller has one already ([`candidate_id`] makes one from an index).
#[must_use]
fn candidate_position(seed: Seed, key: CellKey, id: SystemId) -> GalacticPosition {
    let mut stream = Stream::open(seed, tags::GALAXY_CANDIDATE_POSITION, ObjectKey::from(id));
    let x = stream.next_u64();
    let y = stream.next_u64();
    let z = stream.next_u64();
    key.gen_cell().position_from_words([x, y, z])
}

/// A candidate's acceptance mark: word 0 of its `galaxy.candidate.accept` stream (plan 03, Design
/// note 2).
#[must_use]
fn acceptance_mark(seed: Seed, id: SystemId) -> Mark {
    Stream::open(seed, tags::GALAXY_CANDIDATE_ACCEPT, ObjectKey::from(id)).mark()
}

/// The component a candidate at `position` belongs to, or `None` if the thinning rejects it (plan
/// 03, Design note 3).
///
/// The weights are [`ShareMatrix::component_share`] times the component's density at the position,
/// in component index order — the same products, in the same order, that
/// [`Fields::layer_density`](crate::galaxy::fields::Fields::layer_density) and plan 02's bound take
/// — and [`Mark::pick_weighted`] turns their running sums over the bound into 53-bit thresholds.
///
/// A bound of zero accepts nothing, and such a cell draws no candidate at all
/// ([`candidate_count`](super::candidate_count)), so the guard is reached only by a caller that asks
/// about a candidate the cell has not got; it keeps a division by zero out of the thresholds. A
/// bound that is not a number cannot arrive here, because the candidate count would have panicked on
/// it first; the guard covers it so that this function cannot pass a NaN threshold on.
///
/// # Panics
///
/// In debug builds, if the weighted density at the position exceeds the padded bound. This is the
/// brainstorm's bound-check assertion ("Exact placement by thinning": a density above its bound
/// leaves "a cell-shaped patch that no ordinary test would notice"), and the one the arm-ridge test
/// of P03.T8.c exercises.
#[must_use]
fn pick_component(
    galaxy: &Galaxy,
    key: CellKey,
    bound: f64,
    position: &GalacticPosition,
    mark: Mark,
) -> Option<ComponentId> {
    if bound.is_nan() || bound <= 0.0 {
        return None;
    }
    let fields = galaxy.fields();
    let shares = galaxy.shares();
    let mut densities = [0.0; MAX_COMPONENTS];
    fields.densities(&PointLy::from(position), &mut densities);
    // `share × density` per component, in component order: the products plan 02's layer density
    // and layer bound take, in the same order and the same direction.
    let band = key.band();
    let mut weights = [0.0; MAX_COMPONENTS];
    let components = fields.components();
    for (slot, (component, density)) in weights.iter_mut().zip(components.iter().zip(densities)) {
        *slot = shares.component_share(band, component) * density;
    }
    let weights = &weights[..components.len()];
    let padded_bound = bound * (1.0 + BOUND_PADDING);
    debug_assert!(
        weighted_density(weights) <= padded_bound,
        "the density of layer {} is {} at {:?}, above its bound {bound} over the {} ly cell {:?}",
        key.layer().letter(),
        weighted_density(weights),
        position.to_light_years_f64(),
        key.size_ly(),
        key.gen_cell().to_array(),
    );
    mark.pick_weighted(weights, padded_bound).map(|index| {
        fields
            .component_id(index)
            .expect("the weights are one per component, so a picked index is one of them")
    })
}

/// The weights added from 0 in component order: the running total [`Mark::pick_weighted`] reaches,
/// bit for bit, and not the unweighted sum
/// [`Fields::densities`](crate::galaxy::fields::Fields::densities) returns.
#[must_use]
fn weighted_density(weights: &[f64]) -> f64 {
    weights.iter().fold(0.0, |sum, &weight| sum + weight)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};

    use super::*;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{STELLAR_LAYERS, candidate_count};
    use crate::id::{Layer, SystemIdKind};
    use crate::rng::Seed;
    use crate::units::consts::METRES_PER_LIGHT_YEAR;

    /// The seed of the galaxy these tests place candidates in.
    const SEED: u64 = 0x0300_ca4d_0000_0000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
    }

    /// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
    fn sunlike() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// `n` candidates of `layer` at the Sun-like point, as `(cell, index)`, running on into the
    /// next cell along x once a cell's index field is full.
    fn candidates(layer: Layer, n: u32) -> impl Iterator<Item = (CellKey, u32)> {
        let first = CellKey::containing(layer, &sunlike()).unwrap();
        let [x, y, z] = first.gen_cell().to_array();
        let capacity = first.index_capacity();
        (0..n).map(move |i| {
            let along = i32::try_from(i / capacity).unwrap();
            let key = CellKey::new(layer, [x + along, y, z]).unwrap();
            (key, i % capacity)
        })
    }

    // --- P03.T4.a: the position draw ---

    #[test]
    fn position_lies_inside_its_cell_in_every_layer() {
        let galaxy = galaxy();
        for spec in &STELLAR_LAYERS {
            let layer = spec.layer();
            let size = i32::try_from(layer.cell_size_ly()).unwrap();
            let edge = 65_536 / size;
            // Cells at the Sun-like point, at the origin and at both faces of the root cube.
            let keys = [
                CellKey::containing(layer, &sunlike()).unwrap(),
                CellKey::new(layer, [0, 0, 0]).unwrap(),
                CellKey::new(layer, [-edge, -edge, -edge]).unwrap(),
                CellKey::new(layer, [edge - 1, edge - 1, edge - 1]).unwrap(),
            ];
            for key in keys {
                let origin = key.origin_ly();
                for index in 0..200 {
                    let position = candidate_position(galaxy.seed(), key, candidate_id(key, index));
                    let ly = position.cell().to_array();
                    let offsets = position.offset_metres();
                    for axis in 0..3 {
                        assert!(
                            (origin[axis]..origin[axis] + size).contains(&ly[axis]),
                            "layer {} cell {:?}: light-year {} outside the cell on axis {axis}",
                            layer.letter(),
                            key.gen_cell().to_array(),
                            ly[axis]
                        );
                        assert!(
                            (0.0..METRES_PER_LIGHT_YEAR).contains(&offsets[axis]),
                            "offset {} m is not inside one light-year",
                            offsets[axis]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn position_is_the_same_on_every_call() {
        let galaxy = galaxy();
        for (key, index) in candidates(Layer::D, 50) {
            let id = candidate_id(key, index);
            let first = candidate_position(galaxy.seed(), key, id);
            assert_eq!(candidate_position(galaxy.seed(), key, id), first);
        }
    }

    /// The whole light-year of a position is uniform over the cell on every axis (P03.T4.a).
    fn position_light_years_are_uniform(draws: u32) {
        let galaxy = galaxy();
        for spec in &STELLAR_LAYERS {
            let layer = spec.layer();
            let bins = layer.cell_size_ly();
            let mut counts = vec![[0_u64; 3]; usize::try_from(bins).unwrap()];
            for (key, index) in candidates(layer, draws) {
                let origin = key.origin_ly();
                let id = candidate_id(key, index);
                let ly = candidate_position(galaxy.seed(), key, id).cell().to_array();
                for axis in 0..3 {
                    let bin = usize::try_from(ly[axis] - origin[axis]).unwrap();
                    counts[bin][axis] += 1;
                }
            }
            let expected = vec![f64::from(draws) / f64::from(bins); counts.len()];
            for axis in 0..3 {
                let observed: Vec<u64> = counts.iter().map(|bins| bins[axis]).collect();
                let fit = chi_square_gof(&observed, &expected);
                assert_eq!(fit.dof, bins - 1);
                assert_p_value(
                    &format!("layer {} light-years on axis {axis}", layer.letter()),
                    fit.p_value,
                    ALPHA,
                );
            }
        }
    }

    #[test]
    fn position_light_years_are_uniform_over_a_hundred_thousand_draws() {
        position_light_years_are_uniform(100_000);
    }

    #[test]
    #[ignore = "slow: 10⁶ position draws per layer"]
    fn position_light_years_are_uniform_over_a_million_draws() {
        position_light_years_are_uniform(1_000_000);
    }

    // --- P03.T4.b: acceptance and the component pick ---

    #[test]
    fn accept_frequency_matches_the_mean_density_over_the_bound() {
        let galaxy = galaxy();
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        let key = CellKey::containing(Layer::A, &sunlike()).unwrap();
        let bound = layer_bound(&galaxy, key);
        let draws = 20_000;
        // A candidate at position p is accepted with probability density(p) ÷ bound, so the number
        // accepted is a sum of Bernoulli trials whose probabilities the positions decide.
        let mut mean = 0.0;
        let mut variance = 0.0;
        let mut accepted = 0.0;
        for index in 0..draws {
            let position = candidate_position(galaxy.seed(), key, candidate_id(key, index));
            let p = fields.layer_density(shares, key.band(), &PointLy::from(&position))
                / (bound * (1.0 + BOUND_PADDING));
            assert!((0.0..=1.0).contains(&p), "an acceptance probability of {p}");
            mean += p;
            variance += p * (1.0 - p);
            let mark = acceptance_mark(galaxy.seed(), candidate_id(key, index));
            if pick_component(&galaxy, key, bound, &position, mark).is_some() {
                accepted += 1.0;
            }
        }
        // α = 10⁻³ two-sided is 3.29 standard deviations.
        let sigma = variance.sqrt();
        assert!(
            (accepted - mean).abs() <= 3.29 * sigma,
            "{accepted} of {draws} candidates accepted against {mean} ± {sigma}"
        );
    }

    #[test]
    fn accept_picks_components_with_the_odds_of_their_densities() {
        let galaxy = galaxy();
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        let key = CellKey::containing(Layer::A, &sunlike()).unwrap();
        let bound = layer_bound(&galaxy, key);
        let position = sunlike();
        // The odds at this one position: share × density per component, and the rest is rejection.
        let mut densities = [0.0; MAX_COMPONENTS];
        fields.densities(&PointLy::from(&position), &mut densities);
        let weights: Vec<f64> = fields
            .components()
            .iter()
            .zip(densities)
            .map(|(component, density)| shares.component_share(key.band(), component) * density)
            .collect();
        let padded_bound = bound * (1.0 + BOUND_PADDING);
        let draws = 50_000;
        let n = f64::from(draws);
        // One bin per component, and a last bin for the candidates the thinning rejects.
        let mut counts = vec![0_u64; weights.len() + 1];
        for (cell, index) in candidates(Layer::A, draws) {
            let mark = acceptance_mark(galaxy.seed(), candidate_id(cell, index));
            let picked = pick_component(&galaxy, key, bound, &position, mark);
            counts[picked.map_or(weights.len(), ComponentId::index)] += 1;
        }
        let kept = weights.iter().fold(0.0, |sum, w| sum + w) / padded_bound;
        let mut expected: Vec<f64> = weights.iter().map(|w| n * w / padded_bound).collect();
        expected.push(n * (1.0 - kept));
        // `chi_square_gof` merges the bins whose expectation is too small, the halo's above all.
        let fit = chi_square_gof(&counts, &expected);
        assert_p_value("component pick at the Sun-like point", fit.p_value, ALPHA);
    }

    #[test]
    fn accept_rejects_every_candidate_of_a_cell_with_no_bound() {
        let galaxy = galaxy();
        let key = CellKey::containing(Layer::A, &sunlike()).unwrap();
        let position = sunlike();
        for index in 0..16 {
            let mark = acceptance_mark(galaxy.seed(), candidate_id(key, index));
            for bound in [0.0, -0.0, f64::NAN] {
                assert_eq!(pick_component(&galaxy, key, bound, &position, mark), None);
            }
        }
    }

    #[test]
    fn accept_weights_are_folded_in_component_order() {
        let galaxy = galaxy();
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        // Places where every kind of component contributes: the Sun, the bulge, the bar's end, an
        // arm ridge and well off the plane.
        let places = [
            [0.0, 26_000.0, 0.0],
            [300.0, 350.0, 100.0],
            [14_000.0, 200.0, 50.0],
            [-17_000.0, -19_700.0, 4.0],
            [4_000.0, 9_000.0, 6_000.0],
        ];
        for spec in STELLAR_LAYERS {
            for ly in places {
                let position = GalacticPosition::from_light_years(ly).unwrap();
                let key = CellKey::containing(spec.layer(), &position).unwrap();
                let point = PointLy::from(&position);
                let mut densities = [0.0; MAX_COMPONENTS];
                fields.densities(&point, &mut densities);
                let weights: Vec<f64> = fields
                    .components()
                    .iter()
                    .zip(densities)
                    .map(|(component, density)| {
                        shares.component_share(key.band(), component) * density
                    })
                    .collect();
                // The fold is plan 02's layer density, bit for bit, and it never passes the bound.
                assert_same_bits(
                    weighted_density(&weights),
                    fields.layer_density(shares, key.band(), &point),
                );
                assert!(
                    weighted_density(&weights) <= layer_bound(&galaxy, key),
                    "layer {} at {ly:?}",
                    spec.layer().letter()
                );
            }
        }
    }

    // --- P03.T5.b: the outcome and the carve-out hook ---

    #[test]
    fn outcome_is_the_same_on_every_call_and_in_any_order_of_indices() {
        let galaxy = galaxy();
        for spec in &STELLAR_LAYERS {
            let key = CellKey::containing(spec.layer(), &sunlike()).unwrap();
            let count = candidate_count(&galaxy, key).min(200);
            let forwards: Vec<_> = (0..count)
                .map(|index| evaluate_candidate(&galaxy, key, index))
                .collect();
            let backwards: Vec<_> = (0..count)
                .rev()
                .map(|index| evaluate_candidate(&galaxy, key, index))
                .collect();
            let again: Vec<_> = (0..count)
                .map(|index| evaluate_candidate(&galaxy, key, index))
                .collect();
            assert_eq!(forwards, again, "{:?}", spec.layer());
            let reversed: Vec<_> = backwards.into_iter().rev().collect();
            assert_eq!(forwards, reversed, "{:?}", spec.layer());
        }
    }

    #[test]
    fn outcome_of_an_accepted_candidate_carries_its_cells_layer_and_band() {
        let galaxy = galaxy();
        for spec in &STELLAR_LAYERS {
            let key = CellKey::containing(spec.layer(), &sunlike()).unwrap();
            for index in 0..candidate_count(&galaxy, key) {
                if let CandidateOutcome::Accepted(record) = evaluate_candidate(&galaxy, key, index)
                {
                    assert_eq!(record.layer(), spec.layer());
                    assert_eq!(record.id(), candidate_id(key, index));
                    assert_eq!(CellKey::of(record.id()), Ok(key));
                    let mass = record.primary_initial_mass().value();
                    assert!(
                        (spec.band().lo()..=spec.band().hi()).contains(&mass),
                        "layer {}: a primary of {mass} M☉",
                        spec.layer().letter()
                    );
                    assert!(record.component().is_some());
                }
            }
        }
    }

    #[test]
    fn outcome_is_claimed_by_the_catalogue_for_exactly_what_the_hook_claims() {
        let galaxy = galaxy();
        let key = CellKey::containing(Layer::C, &sunlike()).unwrap();
        let bound = layer_bound(&galaxy, key);
        let claims_even = |_: &Galaxy, record: &SystemRecord| {
            let SystemIdKind::Grid(grid) = record.id().kind() else {
                unreachable!("a grid record has a grid ID")
            };
            grid.index() % 2 == 0
        };
        let mut claimed = 0;
        let mut accepted = 0;
        for index in 0..candidate_count(&galaxy, key) {
            let plain = evaluate_candidate_from_bound(&galaxy, key, index, bound);
            let hooked = evaluate_with_hook(&galaxy, key, index, bound, claims_even);
            match plain {
                CandidateOutcome::Accepted(record) if index % 2 == 0 => {
                    assert_eq!(hooked, CandidateOutcome::ClaimedByCatalogue);
                    assert_eq!(record.id(), candidate_id(key, index));
                    claimed += 1;
                }
                CandidateOutcome::Accepted(_) => {
                    assert_eq!(hooked, plain);
                    accepted += 1;
                }
                CandidateOutcome::Thinned => assert_eq!(hooked, CandidateOutcome::Thinned),
                CandidateOutcome::ClaimedByCatalogue => {
                    unreachable!("no class claims a candidate before plan 09")
                }
            }
        }
        assert!(
            claimed > 0 && accepted > 0,
            "{claimed} claimed and {accepted} left of a layer-C cell"
        );
    }

    #[test]
    #[should_panic(expected = "is past layer A's capacity of 65536")]
    fn outcome_of_an_index_no_id_names_panics() {
        let galaxy = galaxy();
        let key = CellKey::containing(Layer::A, &sunlike()).unwrap();
        let _ = evaluate_candidate(&galaxy, key, key.index_capacity());
    }
}
