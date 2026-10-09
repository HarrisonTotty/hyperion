//! A census's parts merged into one sky (rendering plan R06, R06.T8.c; Design note 11).
//!
//! A census runs its plan's cells as jobs, [`census_cell`](super::census_cell) over each job's
//! cells with the job's own [`SkyContext`](super::SkyContext), and each job's stars and tallies are
//! a part. The parts are merged in [`sky_order`]: by apparent V, the brightest first, then by
//! system ID, then by star index. No two stars share a system and an index, so the order is
//! strict, and neither the split of the cells into jobs nor the order the jobs ran in can change
//! the answer. The brightest `n_max` are listed. The rest are the overflow, which the server's
//! band takes as points (Design note 15), so that each star's light is counted once, on one side.
//!
//! A census may arrive nearest first, shell by shell (R06.T8.i; Design notes 11 and 13): the parts
//! of the shells done are merged by [`merge_shells`] to the [`Completeness`] they reach, which
//! lists only the stars within each partial layer's radius and keeps the rest for a later census of
//! more shells. Merged once all are done, the shells give the one-shot census, star for star.

use std::cmp::Ordering;
use std::num::NonZeroU32;

use super::cell::{CensusTallies, SkyStar};
use super::query::Completeness;

/// A census: the stars it lists, the stars past `n_max`, the tallies of every cell it opened, and,
/// for a census of shells, how far it is complete (R06.T8.i).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkyCensus {
    listed: Vec<SkyStar>,
    overflow: Vec<SkyStar>,
    tallies: CensusTallies,
    completeness: Option<Completeness>,
}

impl SkyCensus {
    /// No stars and no cells: the band's census where the band is computed without one, as the
    /// eye cut's coarse pre-pass computes it (Design note 5).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// The listed stars, at most `n_max` of them, in [`sky_order`].
    #[must_use]
    pub fn listed(&self) -> &[SkyStar] {
        &self.listed
    }

    /// The stars kept past `n_max`, in [`sky_order`]: each fainter than every listed star, or as
    /// bright and after it in the order.
    #[must_use]
    pub fn overflow(&self) -> &[SkyStar] {
        &self.overflow
    }

    /// Every part's tallies added up, with each layer's [`listed`](super::LayerTally::listed)
    /// counted from [`listed`](Self::listed).
    ///
    /// A census of shells counts every cell of its shells done, and every star they kept, among
    /// them those it holds for a later census.
    #[must_use]
    pub const fn tallies(&self) -> &CensusTallies {
        &self.tallies
    }

    /// How far a census of shells ([`merge_shells`]) is complete, per layer and towards each
    /// direction, as its plan's [`CensusPlan::completeness`](super::CensusPlan::completeness) gave
    /// it; `None` for one merged whole ([`merge_census`]) and for [`empty`](Self::empty), which
    /// state no radius of their own: a census merged whole is its plan's, complete to its caps
    /// ([`CompleteTo::of_caps`](crate::sky::band::CompleteTo::of_caps)).
    #[must_use]
    pub const fn completeness(&self) -> Option<&Completeness> {
        self.completeness.as_ref()
    }
}

/// The census's order: by apparent V, the brightest (the least V) first, then by system ID, then
/// by star index.
///
/// V is compared by [`total_cmp`](f64::total_cmp), whose order is total; it is the order of the
/// stars' fluxes, which fall as V rises.
#[must_use]
pub fn sky_order(a: &SkyStar, b: &SkyStar) -> Ordering {
    a.v()
        .total_cmp(&b.v())
        .then_with(|| a.system().cmp(&b.system()))
        .then_with(|| a.star().cmp(&b.star()))
}

/// Merges a census's parts, each one job's stars and tallies, listing the brightest `n_max`.
///
/// The parts may hold their stars in any order, and may come in any order and in any split of the
/// cells. A star must appear in one part only, as each record belongs to one cell. The tallies are
/// the parts' added up ([`CensusTallies::add`]), and with no part they are the default's.
///
/// # Panics
///
/// Either is a census's bug:
///
/// - if a star's V is not finite: a NaN's sign, which places it in [`sky_order`], is not the same
///   on every target;
/// - if two parts hold the same star at the same V, which would let the parts' order choose
///   between them.
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::Seed;
/// use hyperion_sim::sky::census::{
///     CellOffsets, CensusTallies, NoSkyCellCache, SkyContext, SkyQuery, census_cell, census_plan,
///     merge_census,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let query = SkyQuery::builder(Observer::new(at, UniverseTime::EPOCH)?, Magnitudes::new(6.5))
///     .build()?;
/// let mut noise = NoiseCache::with_capacity(1 << 16);
/// let plan = census_plan(&galaxy, &tables, &envelope, &query, &mut noise);
/// // A server runs its jobs, the plan's slabs, on a pool, each with a context of its own; here
/// // they run in turn.
/// let parts = plan.slabs().map(|job| {
///     let mut ctx = SkyContext {
///         tables: &tables,
///         envelope: &envelope,
///         offsets: &offsets,
///         noise: NoiseCache::with_capacity(1 << 16),
///         cells: &NoSkyCellCache,
///         sources: &[],
///         modifiers: &NoModifiers,
///     };
///     let (mut stars, mut tallies) = (Vec::new(), CensusTallies::default());
///     for key in job.cells() {
///         tallies.add(&census_cell(&galaxy, &mut ctx, key, &query, &mut stars));
///     }
///     (stars, tallies)
/// });
/// let census = merge_census(parts, query.n_max());
/// // The naked-eye sky's stars, brightest first; the band takes any overflow.
/// assert!(census.listed().windows(2).all(|w| w[0].v().value() <= w[1].v().value()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn merge_census(
    parts: impl IntoIterator<Item = (Vec<SkyStar>, CensusTallies)>,
    n_max: NonZeroU32,
) -> SkyCensus {
    merge(parts, n_max, None)
}

/// Merges the parts of a census of some of its plan's shells, listing the brightest `n_max` of the
/// stars `completeness` lists (R06.T8.i).
///
/// Those are every star of a layer whose last shell is done, and of any other layer only those
/// within its complete-to radius towards their band texel ([`Completeness::lists`]). The census
/// carries `completeness`.
///
/// The parts are those of every cell of the shells done (and of no other), each cell's once, in any
/// order and split, as for [`merge_census`]. Their stars beyond a partial layer's radius are neither
/// listed nor in the overflow: they are in the band's light beyond the radius until a census of more
/// shells lists them. Merged with every shell done ([`CensusPlan::complete`](super::CensusPlan::complete)),
/// the census is [`merge_census`]'s of the same parts, star for star.
///
/// # Panics
///
/// As [`merge_census`].
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::Seed;
/// use hyperion_sim::sky::census::{
///     CellOffsets, CensusTallies, NoSkyCellCache, SkyContext, SkyQuery, census_cell, census_plan,
///     merge_shells,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let query = SkyQuery::builder(Observer::new(at, UniverseTime::EPOCH)?, Magnitudes::new(7.95))
///     .build()?;
/// let mut noise = NoiseCache::with_capacity(1 << 16);
/// let plan = census_plan(&galaxy, &tables, &envelope, &query, &mut noise);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise,
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// // Shell by shell, nearest first, a reply after each: the census of every shell done so far.
/// let (mut parts, mut done) = (Vec::new(), Vec::new());
/// for shell in plan.shells() {
///     let (mut stars, mut tallies) = (Vec::new(), CensusTallies::default());
///     for key in plan.shell_slabs(shell).flat_map(|slab| slab.cells()) {
///         tallies.add(&census_cell(&galaxy, &mut ctx, key, &query, &mut stars));
///     }
///     parts.push((stars, tallies));
///     done.push(shell);
///     let reply = merge_shells(parts.clone(), query.n_max(), plan.completeness(done.clone()));
///     let completeness = reply.completeness().ok_or("a census of shells")?;
///     // Every star listed lies within its layer's radius, until the last reply.
///     assert!(reply.listed().iter().all(|star| completeness.lists(star)));
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn merge_shells(
    parts: impl IntoIterator<Item = (Vec<SkyStar>, CensusTallies)>,
    n_max: NonZeroU32,
    completeness: Completeness,
) -> SkyCensus {
    merge(parts, n_max, Some(completeness))
}

/// [`merge_census`], listing only the stars `completeness` lists where it is given, and carrying it.
fn merge(
    parts: impl IntoIterator<Item = (Vec<SkyStar>, CensusTallies)>,
    n_max: NonZeroU32,
    completeness: Option<Completeness>,
) -> SkyCensus {
    let mut stars = Vec::new();
    // The first part's tallies start the sum, so that a flag every part clears stays clear.
    let mut summed: Option<CensusTallies> = None;
    for (part, tallies) in parts {
        stars.extend(part);
        match &mut summed {
            Some(sum) => sum.add(&tallies),
            None => summed = Some(tallies),
        }
    }
    let mut tallies = summed.unwrap_or_default();
    assert!(
        stars.iter().all(|s| s.v().value().is_finite()),
        "every kept star's V is finite"
    );
    // The order is strict, so an unstable sort, which sorts in place, gives the one answer.
    stars.sort_unstable_by(sky_order);
    assert!(
        stars
            .windows(2)
            .all(|w| sky_order(&w[0], &w[1]) == Ordering::Less),
        "no star is in two parts"
    );
    // A partial layer's stars beyond its radius wait for a later census; the order is kept.
    if let Some(completeness) = &completeness {
        stars.retain(|star| completeness.lists(star));
    }
    // A `u32` no `usize` holds is more stars than any `Vec` can, so saturating keeps them all.
    let keep = usize::try_from(n_max.get())
        .unwrap_or(usize::MAX)
        .min(stars.len());
    let overflow = stars.split_off(keep);
    // The listed would otherwise hold every kept star's room, the overflow's too, while it lives.
    stars.shrink_to_fit();
    tallies.set_listed(&stars);
    SkyCensus {
        listed: stars,
        overflow,
        tallies,
        completeness,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::placement::CellKey;
    use crate::id::{Layer, SystemId};
    use crate::observe::Observer;
    use crate::sky::caps::{CAPPED_LAYERS, LayerCap};
    use crate::sky::census::cache::NoSkyCellCache;
    use crate::sky::census::cell::census_cell;
    use crate::sky::census::query::{MAX_N_MAX, SkyContext, SkyQuery, plan_with_edges};
    use crate::sky::testing::{milky_way_dark_tables, milky_way_envelope, milky_way_offsets};
    use crate::stellar::multiplicity::StarIndex;
    use crate::time::UniverseTime;
    use crate::units::{LightYears, Magnitudes};

    /// The Sun's place in the fixture, ly.
    const SUN: [f64; 3] = [0.0, 26_000.0, 68.0];

    type Part = (Vec<SkyStar>, CensusTallies);

    fn n(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).expect("not zero")
    }

    fn unbounded() -> NonZeroU32 {
        n(MAX_N_MAX)
    }

    fn context(noise: usize) -> SkyContext<'static> {
        SkyContext {
            tables: milky_way_dark_tables(),
            envelope: milky_way_envelope(),
            offsets: milky_way_offsets(),
            noise: NoiseCache::with_capacity(noise),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        }
    }

    fn sun_query() -> SkyQuery {
        let at = GalacticPosition::from_light_years(SUN).expect("in the cube");
        SkyQuery::builder(
            Observer::new(at, UniverseTime::EPOCH).expect("an observer"),
            Magnitudes::new(9.0),
        )
        .build()
        .expect("a valid query")
    }

    /// Two cells of each stellar layer at the Sun: its own and the next along x.
    fn sun_cells() -> Vec<CellKey> {
        let mut cells = Vec::new();
        for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
            let size = f64::from(layer.cell_size_ly());
            for step in [0.0, 1.0] {
                let p = [SUN[0] + step * size, SUN[1], SUN[2]];
                let at = GalacticPosition::from_light_years(p).expect("in the cube");
                cells.push(CellKey::containing(layer, &at).expect("in the cube"));
            }
        }
        cells
    }

    /// The parts of `split`, each the cells' parts it names, joined in the order it names them.
    fn joined(parts: &[Part], split: &[Vec<usize>]) -> Vec<Part> {
        split
            .iter()
            .map(|job| {
                let mut part: Part = (Vec::new(), CensusTallies::default());
                for &i in job {
                    part.0.extend_from_slice(&parts[i].0);
                    part.1.add(&parts[i].1);
                }
                part
            })
            .collect()
    }

    /// Splits of `0..len` into jobs: whole, one cell each forwards and backwards, interleaved, and
    /// uneven parts in reverse with an empty one.
    fn splits(len: usize) -> Vec<Vec<Vec<usize>>> {
        let all: Vec<usize> = (0..len).collect();
        let half = len / 2;
        vec![
            vec![all.clone()],
            all.iter().map(|&i| vec![i]).collect(),
            all.iter().rev().map(|&i| vec![i]).collect(),
            vec![
                all.iter().copied().filter(|i| i % 2 == 1).collect(),
                all.iter().copied().filter(|i| i % 2 == 0).collect(),
            ],
            vec![
                all[half..].iter().rev().copied().collect(),
                Vec::new(),
                all[..1].to_vec(),
                all[1..half].iter().rev().copied().collect(),
            ],
        ]
    }

    /// Checks the census of `parts` at `n_max`: it lists the `n_max` brightest of the unbounded
    /// census, the rest are its overflow, and its tallies count the listed per layer.
    fn check_cut(parts: &[Part], n_max: NonZeroU32) {
        let whole = merge_census(parts.to_vec(), unbounded());
        let census = merge_census(parts.to_vec(), n_max);
        let all = whole.listed();
        assert!(whole.overflow().is_empty());
        assert!(all.windows(2).all(|w| sky_order(&w[0], &w[1]).is_lt()));
        let keep = all.len().min(usize::try_from(n_max.get()).expect("fits"));
        assert_eq!(census.listed(), &all[..keep], "the brightest are listed");
        assert_eq!(census.overflow(), &all[keep..], "the rest overflow");
        if let (Some(last), Some(next)) = (census.listed().last(), census.overflow().first()) {
            assert!(last.v().value() <= next.v().value());
        }
        let mut summed = CensusTallies::default();
        for part in parts {
            summed.add(&part.1);
        }
        for layer in Layer::ALL {
            let tally = census.tallies().layer(layer);
            let listed = census
                .listed()
                .iter()
                .filter(|s| s.layer() == layer)
                .count();
            assert_eq!(
                tally.listed(),
                u64::try_from(listed).expect("few"),
                "{layer:?}"
            );
            let part = summed.layer(layer);
            assert_eq!(
                (tally.cells(), tally.generated(), tally.accepted()),
                (part.cells(), part.generated(), part.accepted())
            );
        }
    }

    /// Stars of three layers' systems with V in 0.5 mag steps, many tied, and some of a system's
    /// stars as bright as another's.
    fn tied_stars() -> Vec<SkyStar> {
        let mut stars = Vec::new();
        for (k, layer) in [Layer::A, Layer::C, Layer::E].into_iter().enumerate() {
            let key = CellKey::new(layer, [0, 2, 0]).expect("in the cube");
            for index in 0..5_u32 {
                let system: SystemId = key.candidate_id(index).expect("a small index");
                for star in 0..3_u8 {
                    let step = (u32::from(star) * 7 + index * 3 + u32::try_from(k).expect("3")) % 6;
                    let v = 2.0 + 0.5 * f64::from(step);
                    let star = StarIndex::from_body(star).expect("a star of three");
                    stars.push(SkyStar::placeholder(system, star, v));
                }
            }
        }
        stars
    }

    /// The tied stars dealt into `jobs` parts round robin, each part reversed, with tallies of
    /// their cells.
    fn dealt(stars: &[SkyStar], jobs: usize) -> Vec<Part> {
        let mut parts: Vec<Part> = (0..jobs)
            .map(|_| (Vec::new(), CensusTallies::default()))
            .collect();
        for (i, star) in stars.iter().enumerate() {
            parts[i % jobs].0.push(*star);
        }
        for part in &mut parts {
            part.0.reverse();
        }
        parts
    }

    #[test]
    fn any_split_of_the_cells_in_any_order_gives_the_same_census() {
        let (galaxy, query) = (milky_way_galaxy(), sun_query());
        let cells = sun_cells();
        // Each cell's part does not depend on which cells a job censused before it, through a
        // small shared noise cache that evicts.
        let shared = RefCell::new(context(64));
        let census_of = |key: &CellKey| {
            let mut stars = Vec::new();
            let tallies = census_cell(galaxy, &mut shared.borrow_mut(), *key, &query, &mut stars);
            (stars, tallies)
        };
        assert_order_independent(&cells, census_of);
        // A cell's part, with a cache of its own, as a job holds.
        let mut ctx = context(1 << 12);
        let parts: Vec<Part> = cells
            .iter()
            .map(|&key| {
                let mut stars = Vec::new();
                let tallies = census_cell(galaxy, &mut ctx, key, &query, &mut stars);
                (stars, tallies)
            })
            .collect();
        for (key, part) in cells.iter().zip(&parts) {
            assert_eq!(&census_of(key), part, "{key:?}");
        }
        let parts = parts.as_slice();
        let stars: usize = parts.iter().map(|p| p.0.len()).sum();
        let unbounded_census = merge_census(parts.to_vec(), unbounded());
        let accepted: u64 = Layer::ALL
            .iter()
            .map(|&l| unbounded_census.tallies().layer(l).accepted())
            .sum();
        assert_eq!(accepted, u64::try_from(stars).expect("few"));
        assert!(stars > 20, "{stars} stars");
        // And merged in any split of the cells into jobs, in any order, at three cuts.
        let total = u32::try_from(stars).expect("few");
        for n_max in [n(1), n(total / 3), unbounded()] {
            let whole = merge_census(parts.to_vec(), n_max);
            for split in splits(parts.len()) {
                assert_eq!(merge_census(joined(parts, &split), n_max), whole);
            }
            check_cut(parts, n_max);
        }
        // A census of shells too, to C's, D's and E's first shell at 40 ly (R06.T8.i), which
        // holds back their stars beyond it, in any split and order.
        let caps: Vec<LayerCap> = CAPPED_LAYERS
            .iter()
            .map(|&layer| LayerCap::forced(layer, LightYears::new(160.0)))
            .collect();
        let plan = plan_with_edges(&query, caps, &[40, 80]);
        let first = plan.completeness(plan.shells().filter(|shell| shell.index() == 0));
        for n_max in [n(1), n(total / 3), unbounded()] {
            let whole = merge_shells(parts.to_vec(), n_max, first.clone());
            for split in splits(parts.len()) {
                let census = merge_shells(joined(parts, &split), n_max, first.clone());
                assert_eq!(census, whole);
            }
        }
        let shelled = merge_shells(parts.to_vec(), unbounded(), first);
        assert!(shelled.listed().len() < unbounded_census.listed().len());
    }

    #[test]
    fn tied_stars_merge_the_same_in_any_split() {
        let stars = tied_stars();
        let whole = merge_census([(stars.clone(), CensusTallies::default())], n(20));
        for jobs in [1, 2, 3, 7, 45, 60] {
            assert_eq!(
                merge_census(dealt(&stars, jobs), n(20)),
                whole,
                "{jobs} jobs"
            );
        }
        let mut reversed = stars.clone();
        reversed.reverse();
        assert_eq!(
            merge_census([(reversed, CensusTallies::default())], n(20)),
            whole
        );
    }

    #[test]
    fn ties_in_v_are_broken_by_system_then_star() {
        let stars = tied_stars();
        let census = merge_census(dealt(&stars, 4), unbounded());
        let listed = census.listed();
        assert_eq!(listed.len(), stars.len());
        let ties = listed
            .windows(2)
            .filter(|w| w[0].v().total_cmp(&w[1].v()).is_eq())
            .count();
        assert!(ties > 30, "{ties} ties");
        for w in listed.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let in_order = match a.v().total_cmp(&b.v()) {
                Ordering::Less => true,
                Ordering::Equal => (a.system(), a.star()) < (b.system(), b.star()),
                Ordering::Greater => false,
            };
            assert!(in_order, "{a:?} before {b:?}");
        }
    }

    #[test]
    fn n_max_keeps_the_brightest() {
        let stars = tied_stars();
        let parts = dealt(&stars, 3);
        for n_max in [1, 2, 9, 10, 11, 44, 45, 46, 1_000] {
            check_cut(&parts, n(n_max));
        }
        // A cut inside a tie keeps the lower system, then the lower star.
        let whole = merge_census(dealt(&stars, 3), unbounded());
        let tie = whole
            .listed()
            .windows(2)
            .position(|w| w[0].v().total_cmp(&w[1].v()).is_eq())
            .expect("a tie");
        let census = merge_census(dealt(&stars, 3), n(u32::try_from(tie + 1).expect("few")));
        let (last, first) = (census.listed()[tie], census.overflow()[0]);
        assert_eq!(last.v().total_cmp(&first.v()), Ordering::Equal);
        assert!((last.system(), last.star()) < (first.system(), first.star()));
    }

    #[test]
    fn overflow_plus_listed_is_the_unbounded_census() {
        let stars = tied_stars();
        let whole = merge_census(dealt(&stars, 5), unbounded());
        assert_eq!(whole.listed().len(), stars.len());
        assert!(whole.overflow().is_empty());
        for n_max in 1..=u32::try_from(stars.len()).expect("few") + 2 {
            let census = merge_census(dealt(&stars, 5), n(n_max));
            let mut both = census.listed().to_vec();
            both.extend_from_slice(census.overflow());
            assert_eq!(both, whole.listed(), "at {n_max}");
            let listed: u64 = Layer::ALL
                .iter()
                .map(|&l| census.tallies().layer(l).listed())
                .sum();
            assert_eq!(listed, u64::try_from(census.listed().len()).expect("few"));
        }
    }

    #[test]
    fn the_empty_census_holds_nothing() {
        let empty = SkyCensus::empty();
        assert!(empty.listed().is_empty() && empty.overflow().is_empty());
        assert_eq!(empty.tallies(), &CensusTallies::default());
        assert!(empty.tallies().feature_members_absent());
        assert_eq!(merge_census(Vec::new(), n(1)), empty);
        assert_eq!(
            merge_census([(Vec::new(), CensusTallies::default())], unbounded()),
            empty
        );
    }

    #[test]
    fn a_census_merged_again_counts_its_listed_afresh() {
        let stars = tied_stars();
        let first = merge_census(dealt(&stars, 3), n(30));
        let again = merge_census([(first.listed().to_vec(), *first.tallies())], n(12));
        assert_eq!(again.listed(), &first.listed()[..12]);
        for layer in Layer::ALL {
            let listed = again.listed().iter().filter(|s| s.layer() == layer).count();
            assert_eq!(
                again.tallies().layer(layer).listed(),
                u64::try_from(listed).expect("few"),
                "{layer:?}"
            );
        }
    }

    #[test]
    fn feature_members_are_absent_if_any_part_lacks_them() {
        let present = CensusTallies::default().with_feature_members_present();
        let parts = |flags: [CensusTallies; 2]| flags.map(|t| (Vec::new(), t));
        let both = merge_census(parts([present, present]), n(1));
        assert!(!both.tallies().feature_members_absent());
        let one = merge_census(parts([present, CensusTallies::default()]), n(1));
        assert!(one.tallies().feature_members_absent());
    }

    #[test]
    #[should_panic(expected = "no star is in two parts")]
    fn a_star_in_two_parts_is_refused() {
        let star = tied_stars()[0];
        let _ = merge_census(
            [
                (vec![star], CensusTallies::default()),
                (vec![star], CensusTallies::default()),
            ],
            n(1),
        );
    }

    #[test]
    #[should_panic(expected = "every kept star's V is finite")]
    fn a_star_without_a_finite_v_is_refused() {
        let stars = tied_stars();
        let nan = SkyStar::placeholder(stars[1].system(), stars[1].star(), f64::NAN);
        let _ = merge_census([(vec![stars[0], nan], CensusTallies::default())], n(1));
    }
}
