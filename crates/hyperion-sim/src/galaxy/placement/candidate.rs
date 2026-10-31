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

use super::cell::CellBound;
use super::marks;
use super::record::{SystemKind, SystemRecord};
use super::{CellKey, rogue_planet_saturation_density};
use crate::coords::GalacticPosition;
use crate::galaxy::catalogue_classes::lbv::lbv_claims;
use crate::galaxy::displaced::DisplacedClassId;
#[cfg(test)]
use crate::galaxy::displaced::site::explosion_site;
use crate::galaxy::displaced::site::{ExplosionSite, explosion_site_knowing, recent_death_claim};
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::imf::MassBand;
use crate::galaxy::motion::carved_by_the_loss_cone;
#[cfg(doc)]
use crate::galaxy::shares::ShareMatrix;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{Layer, SystemId};
use crate::rng::{Mark, ObjectKey, Seed, Stream, Threshold, tags};
use crate::units::Years;

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
    /// Its orbit about the black hole, from its epoch velocity, passes inside the loss cone, so
    /// there is no such system (plan 09, Design note 12: "one whose pericentre falls inside the
    /// loss cone resolves to 'no such system', as a member would"). Only a grid system in the
    /// Kepler regime of a galaxy with its kinematic tables can be, from generator version 25
    /// (P09.T28.b.b).
    InLossCone,
}

/// Evaluates one candidate of a cell: its position, the thinning, its marks, the carve-out hook
/// and, from generator version 25, the loss cone (P09.T28.b.b).
///
/// This is what resolving an ID does, and what generating a cell does for each index in turn, so
/// the two always agree. It costs one density evaluation and four words, and it never generates the
/// rest of the cell. [`generate_cell`](super::generate_cell) is the way to place a whole cell: it
/// evaluates the cell's bound once instead of once per candidate.
///
/// It does not check `index` against the cell's [`candidate_count`](super::candidate_count): an
/// index at or above the count names no candidate, yet evaluates like one. Anything that starts
/// from an ID goes through [`resolve`](super::resolve), which checks the count first.
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
    evaluate_candidate_from_bound(galaxy, key, index, &CellBound::of(galaxy, key))
}

/// [`evaluate_candidate`] with the cell's bound already evaluated, for the whole-cell walk.
#[must_use]
pub(super) fn evaluate_candidate_from_bound(
    galaxy: &Galaxy,
    key: CellKey,
    index: u32,
    bound: &CellBound,
) -> CandidateOutcome {
    evaluate_with_hook(galaxy, key, index, bound, catalogue_claims)
}

/// What the thinning picked for a candidate: a field component, or, in layers D and E of a galaxy
/// that holds them, one of plan 08's displaced classes (P08.T12.b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Picked {
    /// A field component, with the mark's slice of it.
    Field(Pick),
    /// A displaced class.
    Displaced(DisplacedClassId),
}

/// [`evaluate_candidate_from_bound`] with the carve-out hook given, so that a test can claim
/// candidates that no class claims yet (plan 03, Design note 5), and resolving can take the same
/// hook (P08.T13's seam).
#[must_use]
pub(super) fn evaluate_with_hook(
    galaxy: &Galaxy,
    key: CellKey,
    index: u32,
    bound: &CellBound,
    claims: impl Fn(&Galaxy, &SystemRecord, Option<Years>) -> bool,
) -> CandidateOutcome {
    let id = candidate_id(key, index);
    let position = candidate_position(galaxy.seed(), key, id);
    let mark = acceptance_mark(galaxy.seed(), id);
    let Some(picked) = pick_component(galaxy, key, bound, &position, mark) else {
        return CandidateOutcome::Thinned;
    };
    let drawn = galaxy
        .fields()
        .displaced()
        .and_then(|displaced| marks::draw(galaxy, displaced, key, id, &position, picked));
    let (record, lifetime) = if let Some(marks) = drawn {
        (
            SystemRecord::of_marks(galaxy, id, position, &marks),
            marks.lifetime,
        )
    } else {
        let Picked::Field(pick) = picked else {
            unreachable!("a displaced class is picked only in layers D and E, which draw marks")
        };
        (
            SystemRecord::of_candidate(galaxy, key, id, position, pick.component),
            None,
        )
    };
    // The field's share φ and layer D's carve-out thin a field pick inside its slice (P09.T2.c,
    // P09.T35), after its marks, since φ reads the record's age; a displaced class's weight is
    // its budget's, the features' members included, and no field factor thins it.
    if let Picked::Field(pick) = picked
        && !in_the_field(galaxy, key, bound.total(), mark, &pick, &record)
    {
        return CandidateOutcome::Thinned;
    }
    // Catalogue classes claim stellar hosts only: the hook is not called for the substellar
    // layers (plan 13, P13.T3.c).
    if record.kind() == SystemKind::Stellar && claims(galaxy, &record, lifetime) {
        CandidateOutcome::ClaimedByCatalogue
    } else if carved_by_the_loss_cone(galaxy, &record) {
        CandidateOutcome::InLossCone
    } else {
        CandidateOutcome::Accepted(record)
    }
}

/// The component a candidate's mark picked, and where the mark's slice of it lies: the running
/// sum of the weights before it, and its own weight, systems per cubic light-year.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Pick {
    pub(super) component: ComponentId,
    below: f64,
    weight: f64,
}

#[cfg(test)]
impl Pick {
    /// A pick of `component` with an empty slice, for a test that hands a field pick to the
    /// conditional marks, which read only its component.
    pub(super) const fn of(component: ComponentId) -> Self {
        Self {
            component,
            below: 0.0,
            weight: 0.0,
        }
    }
}

/// Whether the field holds the system `record` that the budget's pick placed (plan 09, P09.T2.c):
/// always under the held arm of P09.T2.c's hold (`FIELD_SHARE_VERSION`), where the field is the
/// budget.
///
/// From version 26 the field keeps a share `f` of its component's systems of the band at the
/// record's age ([`FieldShare::keeps`](crate::galaxy::field_share::FieldShare::keeps)): the lowest
/// `f` of the component's slice of the acceptance mark, so the mark, uniform inside the slice
/// once it picked the component, keeps the candidate with probability `f` and nothing else. The
/// record's age, drawn on its own stream, is independent of the mark, so the young field's ages
/// come out as the budget's times `1 − φ(age)`. The bound, the candidates and their marks stay the
/// budget's, and a system the field keeps is the budget's record bit for bit: the field only
/// thins (brainstorm, "Exact placement by thinning"). The threshold is the pick's, in its
/// arithmetic, so `f` = 1 keeps every candidate the pick kept.
///
/// In layer D, from version 26 under P09.T35's carve-out (`displaced::site::CARVE_OUT_VERSION`),
/// `f` is also times what the field keeps of the component's layer D, 1 less its ancient Type Ia
/// ([`Galaxy::layer_d_kept`]), in the same slice: layer D's share less `ancient_share`, with only
/// counts moving. Under both holds' held arms it keeps every candidate, the code of version 25.
#[must_use]
fn in_the_field(
    galaxy: &Galaxy,
    key: CellKey,
    bound: f64,
    mark: Mark,
    pick: &Pick,
    record: &SystemRecord,
) -> bool {
    let field = galaxy.field_share();
    let layer_d = match key.band() {
        MassBand::D => galaxy.layer_d_kept(pick.component),
        MassBand::A
        | MassBand::B
        | MassBand::C
        | MassBand::E
        | MassBand::BrownDwarf
        | MassBand::RoguePlanet => None,
    };
    if field.is_none() && layer_d.is_none() {
        return true;
    }
    let keeps = field.map_or(1.0, |field| {
        field.keeps(pick.component, key.band(), record.age_at_epoch())
    }) * layer_d.unwrap_or(1.0);
    let padded_bound = bound * (1.0 + BOUND_PADDING);
    mark.is_below(Threshold::from_ratio(
        pick.below + keeps * pick.weight,
        padded_bound,
    ))
}

/// Whether a catalogue class claims this candidate (plan 03, Design note 5; plan 09, P09.T35).
///
/// The hook sits after the marks are drawn because a class is a deterministic test on a candidate's
/// own marks (brainstorm, "Rare events are found by carving out their hosts, not the events"), and
/// the cells draw conditional on not being in the class. It is asked for stellar records only
/// (plan 13, P13.T3.c). Under the claim plan 09 gives, from generator version 26
/// ([`recent_death_claim`], P09.T35), it computes the
/// [`explosion_site`](crate::galaxy::displaced::explosion_site) of every accepted layer-E record
/// and, where the primary dies inside the interval, passes the site to the core-collapse class's
/// claim (plan 08, P08.T13); then, for a record that claim did not take, with or without a site,
/// it asks the luminous blue variables' ([`lbv_claims`], P09.T43): most of their hosts do not die
/// in the interval, so the site's gate must not hold that claim back, and a record both would
/// claim is the core collapses' entry. Every other layer is answered no without a site. Under the
/// held arm it computes the site of no record (P08.T13.b), the field keeps every candidate it
/// accepts, and [`CandidateOutcome::ClaimedByCatalogue`] never appears.
///
/// `lifetime` is the primary's lifetime at its own composition and draws where its marks computed
/// one, which every layer-E record's of a galaxy holding plan 08's displaced classes did (P08.T12.c):
/// the site's exact step reads it instead of computing it again (P08.T13 as built, inheritance 3).
#[must_use]
pub(super) fn catalogue_claims(
    galaxy: &Galaxy,
    record: &SystemRecord,
    lifetime: Option<Years>,
) -> bool {
    recent_death_claim(galaxy.rules().carve_out).is_some_and(|claim| {
        claims_with(
            galaxy,
            record,
            |g, r| explosion_site_knowing(g, r, lifetime),
            claim,
        ) || lbv_claims(galaxy, record)
    })
}

/// The core-collapse half of [`catalogue_claims`], with the site and the claim given: for a
/// layer-E record, `claim` of its `site` where it has one, and no for every other layer, which
/// asks neither (P08.T13). Production passes
/// [`explosion_site`](crate::galaxy::displaced::explosion_site) and the claim
/// [`recent_death_claim`] gives (P08.T13.b, P09.T35); a test passes counting or always-yes
/// closures, which needs no hidden state.
#[must_use]
pub(super) fn claims_with(
    galaxy: &Galaxy,
    record: &SystemRecord,
    site: impl FnOnce(&Galaxy, &SystemRecord) -> Option<ExplosionSite>,
    claim: impl FnOnce(&Galaxy, &ExplosionSite, &SystemRecord) -> bool,
) -> bool {
    record.layer() == Layer::E
        && site(galaxy, record).is_some_and(|site| claim(galaxy, &site, record))
}

/// The ID of candidate `index` of `key`.
///
/// # Panics
///
/// If `index` is at or above the layer's index capacity, which names no candidate.
#[must_use]
pub(super) fn candidate_id(key: CellKey, index: u32) -> SystemId {
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
/// 03, Design note 3); in layers D and E of a galaxy that holds plan 08's displaced classes, the
/// component or the class (P08.T12.b).
///
/// The weights are [`ShareMatrix::component_share`] times the component's density at the position,
/// in component index order, then, where the layer holds displaced classes,
/// [`ShareMatrix::displaced_share`] times each class's density in the band's order — the same
/// products, in the same order, that
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
/// of P03.T8.c exercises. Also in debug builds, if the lazy pick of a band but the rogue planets'
/// is not [`Mark::pick_weighted`]'s over every weight ([`pick_unsaturated`]).
#[must_use]
fn pick_component(
    galaxy: &Galaxy,
    key: CellKey,
    cell_bound: &CellBound,
    position: &GalacticPosition,
    mark: Mark,
) -> Option<Picked> {
    let bound = cell_bound.total();
    if bound.is_nan() || bound <= 0.0 {
        return None;
    }
    if key.band() != MassBand::RoguePlanet {
        return pick_unsaturated(galaxy, key, cell_bound, position, mark);
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
    let weights = &mut weights[..components.len()];
    // The rogue planets, the band this path is left with, saturate at the index limit (ruling
    // 125): where the weights sum past it, each is scaled so that they sum to it, which keeps the
    // component odds. Below it nothing is touched, so every unsaturated cell is bit for bit what it
    // was.
    let total = weighted_density(weights);
    let limit = rogue_planet_saturation_density();
    if total > limit {
        let scale = limit / total;
        for weight in weights.iter_mut() {
            *weight *= scale;
        }
    }
    let weights = &*weights;
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
        Picked::Field(Pick {
            component: fields
                .component_id(index)
                .expect("the weights are one per component, so a picked index is one of them"),
            // The running sum `pick_weighted` reached before the picked weight, in its order.
            below: weighted_density(&weights[..index]),
            weight: weights[index],
        })
    })
}

/// [`pick_component`] for a band whose weights never saturate, every band but the rogue planets'
/// (P03.T4.b).
///
/// It is [`Mark::pick_weighted`] over `share × density` in component order, each density evaluated
/// only when the running sum reaches it, so that none after the picked component is.
/// The sums and thresholds are the pick's own, in its order, so the pick is its pick bit for bit.
/// A cold 50 ly query at the Sun-like point accepts 99% of its candidates, nearly all in the first
/// seven components, the discs, and so skips most of the other twelve's densities, some 2 ms of
/// its grid's 10 (measured 2026-10-30). Where the layer holds plan 08's displaced classes and no
/// component is picked, the sum runs on over them, with the early exits of Design note 16
/// ([`pick_displaced`], P08.T12.b). Debug builds evaluate the rest too, and check that the whole
/// total is under the bound, each class under its own, and that `pick_weighted` over every weight
/// picks the same ([`check_the_lazy_pick`]).
#[must_use]
fn pick_unsaturated(
    galaxy: &Galaxy,
    key: CellKey,
    cell_bound: &CellBound,
    position: &GalacticPosition,
    mark: Mark,
) -> Option<Picked> {
    let fields = galaxy.fields();
    let shares = galaxy.shares();
    let band = key.band();
    let bound = cell_bound.total();
    let padded_bound = bound * (1.0 + BOUND_PADDING);
    let mut densities = fields.densities_lazily(&PointLy::from(position));
    let mut weights = [0.0; MAX_COMPONENTS];
    let mut sum = 0.0;
    let mut picked = None;
    let mut below = 0.0;
    for (i, (component, density)) in densities.by_ref().enumerate() {
        // `share × density`, the product plan 02's layer density and bound take.
        let weight = shares.component_share(band, component) * density;
        weights[i] = weight;
        below = sum;
        sum += weight;
        if mark.is_below(Threshold::from_ratio(sum, padded_bound)) {
            picked = Some(i);
            break;
        }
    }
    let mut picked = picked.map(|index| {
        Picked::Field(Pick {
            component: fields
                .component_id(index)
                .expect("the weights are one per component, so a picked index is one of them"),
            below,
            weight: weights[index],
        })
    });
    let displaced = fields.displaced().map(|d| (d, d.order(band)));
    if picked.is_none()
        && let Some((displaced, order)) = displaced
    {
        picked = pick_displaced(
            &DisplacedPick {
                galaxy,
                key,
                cell_bound,
                padded_bound,
                position,
                mark,
            },
            displaced,
            order,
            sum,
        );
    }
    if cfg!(debug_assertions) {
        check_the_lazy_pick(
            &DisplacedPick {
                galaxy,
                key,
                cell_bound,
                padded_bound,
                position,
                mark,
            },
            &mut weights,
            densities,
            picked,
        );
    }
    picked
}

/// What the thinning picks for candidate `index` of `key`: [`pick_component`] at its position and
/// with its mark, for the tests that sort a cell's records by their pick.
#[cfg(test)]
#[must_use]
pub(super) fn pick_of(galaxy: &Galaxy, key: CellKey, index: u32) -> Option<Picked> {
    let id = candidate_id(key, index);
    let position = candidate_position(galaxy.seed(), key, id);
    let mark = acceptance_mark(galaxy.seed(), id);
    pick_component(galaxy, key, &CellBound::of(galaxy, key), &position, mark)
}

/// What a candidate's pick reads of its cell beyond the field: the galaxy, the cell and its bound,
/// the padded bound the thresholds divide by, the position and the acceptance mark.
struct DisplacedPick<'a> {
    galaxy: &'a Galaxy,
    key: CellKey,
    cell_bound: &'a CellBound,
    padded_bound: f64,
    position: &'a GalacticPosition,
    mark: Mark,
}

/// The pick's run over the band's displaced classes after the field components, from the field's
/// running sum `sum` (plan 08, Design note 16; P08.T12.b).
///
/// The one acceptance mark is compared against one cumulative sum that runs on over the classes in
/// their order ([`DisplacedFields::order`](crate::galaxy::displaced::DisplacedFields::order)), each
/// `count × density`, with two early exits, each of which gives exactly the answer of the full sum
/// ([`Mark::pick_weighted`] over every weight):
///
/// - **accept**: the first class whose running sum passes the mark is picked, and no later class's
///   density is evaluated, as the full sum's threshold would pick it;
/// - **reject**: before a class is evaluated, if the sum so far plus the bounds of it and every
///   class after it ([`CellBound::rest`]) cannot reach the mark, the candidate is thinned. Each
///   density is under its class's bound, and the running sums only rise, so no later threshold
///   could have passed the mark either. The bounds' sum is padded by [`BOUND_PADDING`] of the cell's
///   bound, so that the rounding of two sums taken in different orders cannot make the exit fire
///   where the full sum would still accept.
///
/// The order is part of the generator version; the exits are not, since they change no answer.
fn pick_displaced(
    at: &DisplacedPick<'_>,
    displaced: &crate::galaxy::displaced::DisplacedFields,
    order: &[DisplacedClassId],
    mut sum: f64,
) -> Option<Picked> {
    let band = at.key.band();
    let shares = at.galaxy.shares();
    let point = PointLy::from(at.position);
    let margin = BOUND_PADDING * at.cell_bound.total();
    for (i, &id) in order.iter().enumerate() {
        // A reach at or past the padded bound rejects nothing, and its ratio is no threshold.
        let reach = sum + (at.cell_bound.rest(i) + margin);
        if reach < at.padded_bound
            && !at
                .mark
                .is_below(Threshold::from_ratio(reach, at.padded_bound))
        {
            return None;
        }
        sum += shares.displaced_share(band, id) * displaced.density(id, &point);
        if at
            .mark
            .is_below(Threshold::from_ratio(sum, at.padded_bound))
        {
            return Some(Picked::Displaced(id));
        }
    }
    None
}

/// In debug builds, the lazy pick's checks over every weight: the whole layer's weighted density is
/// under the padded bound, each displaced class's under its own bound over the cell (plan 08,
/// P08.T11's assertion), and [`Mark::pick_weighted`] over the field's weights and then the classes'
/// picks what the lazy pick and its early exits did. `weights` holds the field weights the lazy
/// pick read, and `densities` the components it did not reach.
fn check_the_lazy_pick<'a>(
    at: &DisplacedPick<'_>,
    weights: &mut [f64; MAX_COMPONENTS],
    densities: impl Iterator<Item = (&'a crate::galaxy::fields::Component, f64)>,
    picked: Option<Picked>,
) {
    let (galaxy, key) = (at.galaxy, at.key);
    let fields = galaxy.fields();
    let shares = galaxy.shares();
    let band = key.band();
    let count = fields.components().len();
    let field_picked = match picked {
        Some(Picked::Field(pick)) => Some(pick.component.index()),
        Some(Picked::Displaced(_)) | None => None,
    };
    // The lazy pick read the weights up to the picked component, or every one.
    let read = field_picked.map_or(count, |i| i + 1);
    for (slot, (component, density)) in weights[read..count].iter_mut().zip(densities) {
        *slot = shares.component_share(band, component) * density;
    }
    let classes = fields.displaced().map_or(0, |d| d.order(band).len());
    let mut all: Vec<f64> = Vec::with_capacity(count + classes);
    all.extend_from_slice(&weights[..count]);
    let point = PointLy::from(at.position);
    if let Some(displaced) = fields.displaced() {
        for (i, &id) in displaced.order(band).iter().enumerate() {
            let weight = shares.displaced_share(band, id) * displaced.density(id, &point);
            let class_bound = at.cell_bound.class(i);
            debug_assert!(
                weight <= class_bound,
                "displaced class {id:?}'s weighted density is {weight} at {:?}, above its bound \
                 {class_bound} over the {} ly cell {:?}",
                at.position.to_light_years_f64(),
                key.size_ly(),
                key.gen_cell().to_array(),
            );
            all.push(weight);
        }
    }
    let bound = at.cell_bound.total();
    debug_assert!(
        weighted_density(&all) <= at.padded_bound,
        "the density of layer {} is {} at {:?}, above its bound {bound} over the {} ly cell {:?}",
        key.layer().letter(),
        weighted_density(&all),
        at.position.to_light_years_f64(),
        key.size_ly(),
        key.gen_cell().to_array(),
    );
    let full = full_pick(galaxy, &all, at.padded_bound, at.mark, band);
    debug_assert_eq!(
        full,
        picked,
        "the lazy pick at {:?} is not the pick over every weight",
        at.position.to_light_years_f64()
    );
}

/// [`Mark::pick_weighted`] over `weights`, the field components' then the band's displaced
/// classes' in their order: the full sum's answer, which the lazy pick and its exits must give.
fn full_pick(
    galaxy: &Galaxy,
    weights: &[f64],
    padded_bound: f64,
    mark: Mark,
    band: MassBand,
) -> Option<Picked> {
    let fields = galaxy.fields();
    let count = fields.components().len();
    mark.pick_weighted(weights, padded_bound).map(|index| {
        if index < count {
            Picked::Field(Pick {
                component: fields
                    .component_id(index)
                    .expect("an index below the component count is a component"),
                below: weighted_density(&weights[..index]),
                weight: weights[index],
            })
        } else {
            let order = fields
                .displaced()
                .expect("weights past the components are the displaced classes'")
                .order(band);
            Picked::Displaced(order[index - count])
        }
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
    use crate::galaxy::displaced::site::{Verdicts, take_verdicts};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{
        STELLAR_LAYERS, SystemOrigin, candidate_count, generate_cell, resolve,
    };
    use crate::id::SystemIdKind;
    use crate::rng::{Seed, Threshold};
    use crate::units::consts::METRES_PER_LIGHT_YEAR;

    /// The seed of the galaxy these tests place candidates in.
    const SEED: u64 = 0x0300_ca4d_0000_0000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
    fn sunlike() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// The densities read one at a time for the lazy pick are `Fields::densities`' entries, bit
    /// for bit, at points over the whole root cube, most of them near the plane (P09.T40.a).
    #[test]
    fn the_densities_read_one_at_a_time_are_the_densities_bit_for_bit() {
        let galaxy = galaxy();
        let fields = galaxy.fields();
        let mut stream = Stream::open(Seed::new(SEED), tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let mut all = [0.0; MAX_COMPONENTS];
        for i in 0..20_000 {
            let mut c = [0.0; 3].map(|_| 65_536.0 * (2.0 * stream.uniform() - 1.0));
            // Three in four within 2,000 ly of the plane, one in two of those within 200 ly.
            if i % 4 != 0 {
                c[2] *= if i % 2 == 0 { 200.0 } else { 2_000.0 } / 65_536.0;
            }
            let p = PointLy::new(c[0], c[1], c[2]);
            fields.densities(&p, &mut all);
            let lazy: Vec<f64> = fields.densities_lazily(&p).map(|(_, d)| d).collect();
            assert_eq!(lazy.len(), fields.components().len());
            for (&one, &every) in lazy.iter().zip(&all) {
                assert_same_bits(one, every);
            }
            for (component, (lazy_component, _)) in
                fields.components().iter().zip(fields.densities_lazily(&p))
            {
                assert!(std::ptr::eq(component, lazy_component));
            }
        }
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
        let cell_bound = CellBound::of(&galaxy, key);
        let bound = cell_bound.total();
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
            if pick_component(&galaxy, key, &cell_bound, &position, mark).is_some() {
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
        let cell_bound = CellBound::of(&galaxy, key);
        let bound = cell_bound.total();
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
            let picked = pick_component(&galaxy, key, &cell_bound, &position, mark);
            let index = picked.map(|picked| match picked {
                Picked::Field(pick) => pick.component.index(),
                Picked::Displaced(_) => unreachable!("the field galaxy holds no displaced class"),
            });
            counts[index.unwrap_or(weights.len())] += 1;
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
                assert_eq!(
                    pick_component(&galaxy, key, &CellBound::with_total(bound), &position, mark),
                    None
                );
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
                    weighted_density(&weights) <= CellBound::of(&galaxy, key).total(),
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
        let bound = CellBound::of(&galaxy, key);
        let claims_even = |_: &Galaxy, record: &SystemRecord, _: Option<Years>| {
            let SystemIdKind::Grid(grid) = record.id().kind() else {
                unreachable!("a grid record has a grid ID")
            };
            grid.index() % 2 == 0
        };
        let mut claimed = 0;
        let mut accepted = 0;
        for index in 0..candidate_count(&galaxy, key) {
            let plain = evaluate_candidate_from_bound(&galaxy, key, index, &bound);
            let hooked = evaluate_with_hook(&galaxy, key, index, &bound, claims_even);
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
                CandidateOutcome::InLossCone => {
                    unreachable!("a solar-circle cell is far from the loss cone")
                }
            }
        }
        assert!(
            claimed > 0 && accepted > 0,
            "{claimed} claimed and {accepted} left of a layer-C cell"
        );
    }

    /// The seam half of `hook_asks_for_a_site_once_per_accepted_layer_e_candidate`, T12.b's
    /// regression check (P08.T13): through `claims_with` and `evaluate_with_hook` with counting
    /// closures, the hook asks for a site exactly once for each candidate the thinning accepts in
    /// layer E and never in layers A–D, gives one verdict for each site it asks for, and passes
    /// each site there is to the claim exactly once. It returns every cell it walked, and each
    /// layer-E cell with its verdicts.
    fn walk_the_seam(galaxy: &Galaxy) -> (Vec<CellKey>, Vec<(CellKey, Verdicts)>) {
        let asked = std::cell::Cell::new(0_u32);
        let (sites, claimed) = (std::cell::Cell::new(0_u32), std::cell::Cell::new(0_u32));
        let site = |g: &Galaxy, r: &SystemRecord| {
            asked.set(asked.get() + 1);
            let site = explosion_site(g, r);
            sites.set(sites.get() + u32::from(site.is_some()));
            site
        };
        let claim = |_: &Galaxy, _: &ExplosionSite, _: &SystemRecord| {
            claimed.set(claimed.get() + 1);
            false
        };
        let hook = |g: &Galaxy, r: &SystemRecord, _: Option<Years>| claims_with(g, r, site, claim);
        let (mut cells, mut layer_e) = (Vec::new(), Vec::new());
        let mut accepted_e = 0_u32;
        take_verdicts();
        for spec in &STELLAR_LAYERS {
            let layer = spec.layer();
            let [x, y, z] = CellKey::containing(layer, &sunlike())
                .unwrap()
                .gen_cell()
                .to_array();
            // Enough layer-E cells that the exact tier runs in some: one of these 160 holds a death
            // in the interval.
            let count = if layer == Layer::E { 160 } else { 2 };
            for dx in 0..count {
                let key = CellKey::new(layer, [x + dx, y, z]).unwrap();
                cells.push(key);
                let bound = CellBound::of(galaxy, key);
                let asked_before = asked.get();
                for index in 0..candidate_count(galaxy, key) {
                    let before = asked.get();
                    let outcome = evaluate_with_hook(galaxy, key, index, &bound, hook);
                    let asks = asked.get() - before;
                    match outcome {
                        CandidateOutcome::Accepted(_) if layer == Layer::E => {
                            assert_eq!(asks, 1, "{key:?} candidate {index}");
                            accepted_e += 1;
                        }
                        CandidateOutcome::Accepted(_) | CandidateOutcome::Thinned => {
                            assert_eq!(asks, 0, "{key:?} candidate {index}");
                        }
                        CandidateOutcome::ClaimedByCatalogue => {
                            unreachable!("the test's claim says no")
                        }
                        CandidateOutcome::InLossCone => {
                            unreachable!("a solar-circle cell is far from the loss cone")
                        }
                    }
                }
                let verdicts = take_verdicts();
                assert_eq!(
                    verdicts.total(),
                    u64::from(asked.get() - asked_before),
                    "one verdict per site asked for in {key:?}"
                );
                if layer == Layer::E {
                    layer_e.push((key, verdicts));
                }
            }
        }
        assert_eq!(asked.get(), accepted_e);
        assert!(sites.get() > 0, "no site in {accepted_e} layer-E records");
        assert_eq!(claimed.get(), sites.get());
        (cells, layer_e)
    }

    /// P08.T13 and P08.T13.b: the seam half ([`walk_the_seam`]) reaches the exact tier; and over
    /// the same cells, through the whole-cell walk and through `resolve`, the production hook gives
    /// one verdict of the interval predicate per accepted layer-E candidate once a claim is set,
    /// placed or claimed, and none under the carve-out's held arm (`recent_death_claim`), even in
    /// the cells where the seam reached the exact tier.
    #[test]
    fn hook_asks_for_a_site_once_per_accepted_layer_e_candidate() {
        let galaxy = galaxy();
        let (cells, seam) = walk_the_seam(&galaxy);
        let sum = |tier: fn(&Verdicts) -> u64| seam.iter().map(|(_, v)| tier(v)).sum::<u64>();
        let seam_verdicts = Verdicts {
            no_death: sum(|v| v.no_death),
            bracket: sum(|v| v.bracket),
            table: sum(|v| v.table),
            exact_inside: sum(|v| v.exact_inside),
            exact_outside: sum(|v| v.exact_outside),
        };
        // The seam's sites imply it, since only the exact tier gives a site; said outright, these
        // cells reach tier 3.
        assert!(
            seam_verdicts.exact() > 0,
            "the seam reached no exact tier: {}",
            seam_verdicts.shares()
        );
        let exact_cells: Vec<CellKey> = seam
            .iter()
            .filter(|(_, v)| v.exact() > 0)
            .map(|&(key, _)| key)
            .collect();

        // The production hook over the same cells, through the whole-cell walk and then through
        // `resolve` of each record of the cells where the seam reached the exact tier: once a
        // claim is set, one verdict per accepted layer-E candidate, from the one site it computes,
        // and none before (P08.T13.b). A claimed candidate has its verdict and is not placed.
        let per_record = |records: u64| {
            if recent_death_claim(galaxy.rules().carve_out).is_some() {
                records
            } else {
                0
            }
        };
        // The seam asked for one site, and so one verdict, per accepted layer-E candidate.
        let accepted_e = seam_verdicts.total();
        let mut systems = Vec::new();
        let mut exact_records = Vec::new();
        let mut placed_e = 0;
        for &key in &cells {
            generate_cell(&galaxy, key, &mut systems);
            if key.layer() == Layer::E {
                placed_e += u64::try_from(systems.len()).unwrap();
            }
            if exact_cells.contains(&key) {
                exact_records.extend_from_slice(&systems);
            }
        }
        // The walk places every candidate the seam accepted, less any a claim takes.
        assert!(placed_e <= accepted_e);
        if recent_death_claim(galaxy.rules().carve_out).is_none() {
            assert_eq!(placed_e, accepted_e);
        }
        let walked = take_verdicts();
        assert_eq!(walked.total(), per_record(accepted_e));
        for record in &exact_records {
            assert_eq!(resolve(&galaxy, record.id()), Ok(*record));
        }
        let resolved = take_verdicts();
        let resolved_e = u64::try_from(exact_records.len()).unwrap();
        assert_eq!(resolved.total(), per_record(resolved_e));
        println!(
            "seam, {} layer-E cells: {}; production ({}): {} verdicts over {placed_e} placed \
             layer-E records, {} over {resolved_e} resolved in the {} cells the exact tier reached",
            seam.len(),
            seam_verdicts.shares(),
            if recent_death_claim(galaxy.rules().carve_out).is_some() {
                "a claim set"
            } else {
                "no claim set"
            },
            walked.total(),
            resolved.total(),
            exact_cells.len(),
        );
    }

    /// Every draw of a candidate, re-derived from plan 03's Design notes 1–4 with nothing of this
    /// module's: the streams opened by hand, their words read by number and the thresholds written
    /// out. The goldens pin what comes out; this pins which word of which stream feeds which
    /// decision, so that a swapped axis, a shifted word, a crossed tag or a changed padding fails
    /// here by name and not only as a golden diff.
    #[test]
    fn outcome_follows_the_plans_streams_word_for_word() {
        let galaxy = galaxy();
        let seed = galaxy.seed();
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        // The Sun, the bulge, the bar's end, an arm ridge and well off the plane: places where
        // every kind of component is picked and the thinning rejects something.
        let places = [
            [0.0, 26_000.0, 0.0],
            [300.0, 350.0, 100.0],
            [14_000.0, 200.0, 50.0],
            [-17_000.0, -19_700.0, 4.0],
            [4_000.0, 9_000.0, 6_000.0],
        ];
        let (mut accepted, mut thinned) = (0_u32, 0_u32);
        for spec in STELLAR_LAYERS {
            let (layer, band) = (spec.layer(), spec.band());
            for ly in places {
                let place = GalacticPosition::from_light_years(ly).unwrap();
                let key = CellKey::containing(layer, &place).unwrap();
                let cell = key.gen_cell();
                let bound = fields.layer_bound(shares, band, &key.cell_box());
                // Design note 1: one Poisson draw on the cell's stream, keyed by candidate 0's ID.
                let cell_word = SystemId::from_parts(layer, cell, 0).unwrap().raw();
                let mut cell_stream = Stream::open(
                    seed,
                    tags::GALAXY_CELL_CANDIDATES,
                    ObjectKey::cell(cell_word),
                );
                let count = candidate_count(&galaxy, key);
                assert_eq!(
                    u64::from(count),
                    cell_stream.poisson(bound * key.volume_ly3()),
                    "layer {} at {ly:?}",
                    layer.letter()
                );
                for index in 0..count.min(400) {
                    let id = SystemId::from_parts(layer, cell, index).unwrap();
                    // Design note 2: words 0, 1 and 2 of the position stream are x, y and z, and
                    // word 0 of the acceptance stream is the mark.
                    let words = Stream::open(seed, tags::GALAXY_CANDIDATE_POSITION, id.into());
                    let position = cell.position_from_words([0, 1, 2].map(|n| words.word_at(n)));
                    let accept = Stream::open(seed, tags::GALAXY_CANDIDATE_ACCEPT, id.into());
                    let mark = Mark::from_word(accept.word_at(0));
                    // Design note 3: the first component whose running sum of share × density,
                    // over the bound padded by 10⁻¹², lies above the mark.
                    let mut densities = [0.0; MAX_COMPONENTS];
                    fields.densities(&PointLy::from(&position), &mut densities);
                    let padded = bound * (1.0 + 1e-12);
                    let mut sum = 0.0;
                    let picked = fields.components().iter().zip(densities).position(
                        |(component, density)| {
                            sum += shares.component_share(band, component) * density;
                            mark.is_below(Threshold::from_probability((sum / padded).min(1.0)))
                        },
                    );
                    match (evaluate_candidate(&galaxy, key, index), picked) {
                        (CandidateOutcome::Thinned, None) => thinned += 1,
                        (CandidateOutcome::Accepted(record), Some(picked)) => {
                            let component = fields.component_id(picked).unwrap();
                            // Design note 4: one word of the mass stream and one of the age
                            // stream, the age from the picked component's distribution.
                            let mut mass = Stream::open(seed, tags::SYSTEM_PRIMARY_MASS, id.into());
                            let primary = galaxy.mass_function().sample_in_band(band, &mut mass);
                            let mut age = Stream::open(seed, tags::SYSTEM_AGE, id.into());
                            let laws = fields.component(component);
                            let age_at_epoch = laws.ages().sample(&mut age);
                            assert_eq!((mass.position(), age.position()), (1, 1));
                            assert_eq!(record.id(), id);
                            assert_eq!(record.origin(), SystemOrigin::Grid(component));
                            assert_eq!(record.population(), laws.population());
                            let (theirs, mine) = (record.epoch_position(), &position);
                            assert_eq!(theirs.cell(), mine.cell());
                            for (a, b) in
                                theirs.offset_metres().into_iter().zip(mine.offset_metres())
                            {
                                assert_same_bits(a, b);
                            }
                            assert_same_bits(record.primary_initial_mass().value(), primary);
                            assert_same_bits(record.age_at_epoch().value(), age_at_epoch.value());
                            accepted += 1;
                        }
                        (outcome, picked) => panic!(
                            "layer {} at {ly:?}, candidate {index}: {outcome:?}, but the plan's \
                             draws pick {picked:?}",
                            layer.letter()
                        ),
                    }
                }
            }
        }
        assert!(
            accepted > 1_000 && thinned > 50,
            "{accepted} accepted and {thinned} thinned: too few to pin both branches"
        );
    }

    #[test]
    #[should_panic(expected = "is past layer A's capacity of 65536")]
    fn outcome_of_an_index_no_id_names_panics() {
        let galaxy = galaxy();
        let key = CellKey::containing(Layer::A, &sunlike()).unwrap();
        let _ = evaluate_candidate(&galaxy, key, key.index_capacity());
    }

    // --- P08.T12.b: the displaced classes in the pick, and its early exits ---

    /// The cells of the exits test: layers D and E at the Sun-like point, in the outer bulge, on a
    /// young arm ridge outside the bar, 3,000 ly above the plane at the solar circle, in the halo
    /// 15,000 ly up and 500 ly from the centre.
    fn exit_cells(galaxy: &Galaxy) -> Vec<CellKey> {
        let arms = galaxy.fields().arms();
        let r = 1.15 * arms.bar_half_length().value();
        let (sin, cos) = crate::math::sin_cos(arms.ridge_azimuth(r, 0));
        let half = 0.5_f64.sqrt();
        let places = [
            [0.0, 26_000.0, 0.0],
            [3_200.0 * half, 3_200.0 * half, 0.0],
            [r * cos, r * sin, 0.0],
            [0.0, 26_000.0, 3_000.0],
            [0.0, 20_000.0, 15_000.0],
            [400.0, 300.0, 0.0],
        ];
        places
            .into_iter()
            .flat_map(|at| {
                let position = GalacticPosition::from_light_years(at).expect("in the cube");
                [Layer::D, Layer::E]
                    .map(|layer| CellKey::containing(layer, &position).expect("in the cube"))
            })
            .collect()
    }

    /// How a lazy pick ended, as the exits test tallies it.
    #[derive(Debug, Default)]
    struct ExitCounts {
        field: u64,
        displaced: u64,
        /// Thinned before any class's density was evaluated: the field's sum and every class's
        /// bound could not reach the mark.
        rejected_at_once: u64,
        /// Thinned later.
        rejected: u64,
    }

    /// `n` candidates of each of `cells` in `galaxy`, by index from 0 (an index past a cell's
    /// count evaluates like a candidate): the lazy pick with its two early exits against
    /// `pick_weighted` over every weight, field components' then the classes' in their order.
    fn exits_agree_with_the_full_sum(galaxy: &Galaxy, cells: &[CellKey], n: u32) -> ExitCounts {
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        let displaced = fields.displaced().expect("a galaxy under every 26 hold");
        let mut counts = ExitCounts::default();
        let mut densities = [0.0; MAX_COMPONENTS];
        for &key in cells {
            let band = key.band();
            let bound = CellBound::of(galaxy, key);
            let padded = bound.total() * (1.0 + BOUND_PADDING);
            let order = displaced.order(band);
            for index in 0..n {
                let id = candidate_id(key, index);
                let position = candidate_position(galaxy.seed(), key, id);
                let mark = acceptance_mark(galaxy.seed(), id);
                let lazy = pick_component(galaxy, key, &bound, &position, mark);
                let point = PointLy::from(&position);
                fields.densities(&point, &mut densities);
                let mut weights: Vec<f64> = fields
                    .components()
                    .iter()
                    .zip(densities)
                    .map(|(c, d)| shares.component_share(band, c) * d)
                    .collect();
                let field_sum = weighted_density(&weights);
                weights.extend(order.iter().map(|&class| {
                    shares.displaced_share(band, class) * displaced.density(class, &point)
                }));
                let full = full_pick(galaxy, &weights, padded, mark, band);
                assert_eq!(lazy, full, "{key:?} candidate {index}");
                match lazy {
                    Some(Picked::Field(_)) => counts.field += 1,
                    Some(Picked::Displaced(_)) => counts.displaced += 1,
                    None => {
                        let reach = field_sum + (bound.rest(0) + BOUND_PADDING * bound.total());
                        if reach >= padded || mark.is_below(Threshold::from_ratio(reach, padded)) {
                            counts.rejected += 1;
                        } else {
                            counts.rejected_at_once += 1;
                        }
                    }
                }
            }
        }
        counts
    }

    /// P08.T12.b: the two early exits give the full sum's answer, on 10⁴ candidates for the fast
    /// suite (the plan's 10⁶ are the slow test below), and every way out of the pick is taken: a
    /// component, a class, and rejection before and after the first class's density.
    #[test]
    fn the_early_exits_give_the_full_sum_s_answer() {
        let galaxy = crate::galaxy::displaced::hold::displaced();
        let cells = exit_cells(galaxy);
        let counts = exits_agree_with_the_full_sum(galaxy, &cells, 834);
        println!("{counts:?}");
        assert!(
            counts.field > 0
                && counts.displaced > 0
                && counts.rejected > 0
                && counts.rejected_at_once > 0,
            "every way out is taken: {counts:?}"
        );
    }

    /// P08.T12.b: the two early exits give the full sum's answer on 10⁶ candidates.
    #[test]
    #[ignore = "slow: 10⁶ candidates, each with every class's density evaluated twice"]
    fn the_early_exits_give_the_full_sum_s_answer_on_a_million_candidates() {
        let galaxy = crate::galaxy::displaced::hold::displaced();
        let cells = exit_cells(galaxy);
        let counts = exits_agree_with_the_full_sum(galaxy, &cells, 83_334);
        println!("{counts:?}");
        assert!(
            counts.field + counts.displaced + counts.rejected + counts.rejected_at_once
                >= 1_000_000
        );
        assert!(counts.displaced > 0 && counts.rejected > 0 && counts.rejected_at_once > 0);
    }
}
