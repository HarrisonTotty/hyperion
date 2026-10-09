//! The sky census's hierarchy bound (plan 11, P11.T16; rendering plan R06's ask A): the hierarchy
//! of every redraw attempt that [`SystemStars::generate`] can keep, drawn by the generator's own
//! draw, so exact by construction (decided 2026-10-05, `decision-r06-census-cost.md` §7; its form
//! ruled 2026-10-07, `decision-p11-t16-hierarchy-bound.md`).
//!
//! R06.T8.g bounds a record's light star by star before it generates the system. For that it
//! needs each star's initial mass, its body and the attempt it was drawn at (to read its η from its
//! own draws), and each star–star pair's periastron, for P11.T17's `pair_light_bound`. An interval
//! in their place was weighed and rejected on measurement: without the period a companion's mass
//! ratio is known only to a factor of about 2, and without the stability test the try that is kept
//! is unknown. So the bound runs the draw itself, at each attempt the generator can keep.
//!
//! It reads the generator's existing words only, through the draw's own functions: it opens no
//! stream that [`draw_hierarchy`](super::draw_hierarchy) does not, adds no domain tag and draws
//! no word of its own. Generated output does not move.

use super::direct::PERIOD_CORRECTION;
use super::hierarchy::{
    HierarchyNode, MultiplicityContext, NodeIndex, RecordDraw, RedrawAttempt, SlotKind, StarIndex,
    StarSlot, SystemHierarchy,
};
use super::substellar;
use crate::galaxy::Galaxy;
use crate::galaxy::placement::SystemRecord;
use crate::stellar::Composition;
use crate::stellar::binary::carve::grid_redraws;
#[cfg(doc)]
use crate::stellar::system::SystemStars;
use crate::stellar::system::grid_multiplicity;
use crate::units::Metres;

/// The hierarchy of every redraw attempt that [`SystemStars::generate`] can keep for `record`, at
/// `composition`: the census's bound star by star (plan 11, P11.T16; rendering plan R06,
/// R06.T8.g).
///
/// Each attempt listed is the generator's own draw there:
/// [`draw_hierarchy_of_composition`](super::draw_hierarchy_of_composition) at `composition` and
/// the record's [`grid_multiplicity`], the stars of `draw_hierarchy_with` and then the brown-dwarf
/// companion of `substellar::with_companion`, bit for bit. So a mass or a periastron the bound
/// lists is the generated one, not an interval about it.
///
/// **The cover.** The generator keeps the first attempt with no pair in a carved class
/// ([`SystemStars::carved_pair`]), and only a pair that the binary engine runs can be carved: one
/// of two stars, neither a brown dwarf ([`AttemptBound::pairs`]). So attempt n + 1 can be kept
/// only if attempt n holds such a pair ([`AttemptBound::may_carve`]). The bound lists attempt 0,
/// then attempt n + 1 for as long as attempt n may carve, through the last attempt, 7. If that one
/// may carve too, it lists the single star that the generator keeps after its last attempt, the
/// primary ([`HierarchyBound::fallback`]). A record that the grid does not redraw (a feature
/// member, or a brown dwarf or rogue planet, which is single) lists attempt 0 alone. So it lists
/// every attempt the generator can keep, and may list some it does not, never fewer; within an
/// attempt the stability test's tries are the draw's own, so it lists the try the draw keeps.
///
/// For a grid record with its own [`draw_metallicity`](crate::stellar::system::draw_metallicity)
/// it is exactly what `SystemStars::generate` draws. A feature member's system is its cluster's
/// (plan 09), drawn under the cluster's context, which this does not take.
///
/// It costs at most about the draws it repeats, since attempts share what they read alike
/// (`RecordDraw`): the limits and the direct construction's period law are built once a record.
/// Near the Sun that is about the draws in layers A–C and under half of them in D and E: 4.7,
/// 7.1, 15, 50 and 140 µs a record in A–E (P11.T16's bench, provisional under shared load).
///
/// # Examples
///
/// The census bounds a system's stars before it generates them, and the generated system is one
/// that the bound lists:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::multiplicity::hierarchy_bound;
/// use hyperion_sim::stellar::system::{SystemStars, draw_metallicity};
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::D, [0, 406, 0])?, &mut cell);
/// let record = cell.first().ok_or("the cell has systems")?;
/// let bound = hierarchy_bound(&galaxy, record, &draw_metallicity(&galaxy, record));
/// // Every star of every attempt listed, with the attempt that reads its draws.
/// for attempt in bound.attempts() {
///     for star in attempt.stars() {
///         assert!(star.initial_mass() <= bound.primary().initial_mass());
///     }
/// }
/// // The system the generator keeps is the draw of an attempt the bound lists, or, after eight
/// // attempts that all carve, the primary alone, which the bound then lists as its fallback.
/// let kept = SystemStars::generate(&galaxy, record);
/// let listed = bound.attempt(kept.attempt()).ok_or("the kept attempt is listed")?;
/// assert!(listed.hierarchy() == kept.hierarchy() || bound.fallback().is_some());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn hierarchy_bound(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
) -> HierarchyBound {
    bound_in(galaxy, record, composition, grid_multiplicity(record))
}

/// [`hierarchy_bound`] under the multiplicity context `ctx`: the record's own for the census,
/// any other for the tests of the cover.
#[must_use]
fn bound_in(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
    ctx: MultiplicityContext,
) -> HierarchyBound {
    let redrawn = grid_redraws(record, ctx);
    let mut draw = RecordDraw::new(galaxy, record, Some(composition), ctx, &PERIOD_CORRECTION);
    let mut attempts = Vec::with_capacity(1);
    let mut attempt = RedrawAttempt::FIRST;
    loop {
        let stars = draw.stars_at(attempt);
        let listed = AttemptBound::new(
            attempt,
            substellar::with_companion(galaxy, record, ctx, attempt, stars),
        );
        let may_carve = listed.may_carve();
        attempts.push(listed);
        if !(redrawn && may_carve) {
            return HierarchyBound {
                attempts,
                fallback: false,
            };
        }
        let Some(next) = attempt.next() else {
            // `after_last_attempt`'s single star: the primary, which is never redrawn.
            return HierarchyBound {
                attempts,
                fallback: true,
            };
        };
        attempt = next;
    }
}

/// The hierarchies of every redraw attempt that [`SystemStars::generate`] can keep for one record
/// ([`hierarchy_bound`]): attempt 0 first, then each later attempt while the one before
/// [may carve](AttemptBound::may_carve), and, after a last attempt that may carve, the primary
/// alone.
#[derive(Debug, Clone, PartialEq)]
pub struct HierarchyBound {
    attempts: Vec<AttemptBound>,
    /// Whether the single star kept after the last attempt, the primary, is listed.
    fallback: bool,
}

impl HierarchyBound {
    /// Every attempt listed, in order from attempt 0, with no gap: one to
    /// [`MAX_REDRAWS`](super::MAX_REDRAWS).
    #[must_use]
    pub fn attempts(&self) -> &[AttemptBound] {
        &self.attempts
    }

    /// The attempt `attempt`, if it is listed.
    #[must_use]
    pub fn attempt(&self, attempt: RedrawAttempt) -> Option<&AttemptBound> {
        self.attempts.get(usize::from(attempt.get()))
    }

    /// The single star that the generator keeps when all of its attempts carve, if the bound
    /// lists it: the primary ([`HierarchyBound::primary`]), at the last attempt, when that attempt
    /// may carve too.
    ///
    /// It adds no star the attempts do not list: the primary is the same star, of the same mass,
    /// at every attempt, and its draws are its record's own, never a redraw's.
    #[must_use]
    pub fn fallback(&self) -> Option<&StarSlot> {
        self.fallback.then(|| self.primary())
    }

    /// The primary: attempt 0's star 0, whose initial mass is its record's
    /// [`primary_initial_mass`](SystemRecord::primary_initial_mass), bit for bit, at every attempt.
    #[must_use]
    pub fn primary(&self) -> &StarSlot {
        &self.attempts[0].stars()[0]
    }
}

/// One redraw attempt of a [`HierarchyBound`]: the hierarchy the generator's draw gives there, and
/// the pairs of it that the binary engine may run.
#[derive(Debug, Clone, PartialEq)]
pub struct AttemptBound {
    attempt: RedrawAttempt,
    hierarchy: SystemHierarchy,
    pairs: Vec<BoundPair>,
}

impl AttemptBound {
    /// The attempt's `hierarchy`, with the pairs `SystemStars`' `run_pairs` may send to the binary
    /// engine: pairs whose two members are stars, neither a brown dwarf. A pair with a member that
    /// is itself a pair, or with a brown dwarf, is never run and so never carved.
    #[must_use]
    fn new(attempt: RedrawAttempt, hierarchy: SystemHierarchy) -> Self {
        let pairs = hierarchy
            .pairs()
            .filter_map(|(node, orbit)| {
                let HierarchyNode::Pair { inner, outer, .. } = *hierarchy.node(node) else {
                    unreachable!("pairs are pairs");
                };
                let (HierarchyNode::Star(a), HierarchyNode::Star(b)) =
                    (*hierarchy.node(inner), *hierarchy.node(outer))
                else {
                    return None;
                };
                [a, b]
                    .iter()
                    .all(|&s| hierarchy.star(s).kind() == SlotKind::Star)
                    .then(|| BoundPair {
                        node,
                        stars: [a, b],
                        periastron: orbit.periapsis(),
                    })
            })
            .collect();
        Self {
            attempt,
            hierarchy,
            pairs,
        }
    }

    /// The redraw attempt: the block of words its hierarchy was drawn on, and the attempt each
    /// companion's own draws are read at
    /// ([`StarDraws::for_attempt`](crate::stellar::draws::StarDraws::for_attempt)). The primary's
    /// draws are its record's at every attempt.
    #[must_use]
    pub const fn attempt(&self) -> RedrawAttempt {
        self.attempt
    }

    /// The hierarchy the generator's draw gives at this attempt, orbits included.
    #[must_use]
    pub const fn hierarchy(&self) -> &SystemHierarchy {
        &self.hierarchy
    }

    /// Every star of the attempt, by [`StarIndex`], the primary first: each with its body, its kind
    /// (a star or a brown-dwarf companion) and its initial mass, bit for bit.
    #[must_use]
    pub fn stars(&self) -> &[StarSlot] {
        self.hierarchy.stars()
    }

    /// Every pair the binary engine may run at this attempt, depth first: each of two stars,
    /// neither a brown dwarf, with its drawn periastron. The engine runs such a pair only if it
    /// can interact or holds a remnant by the end of the clock window, so it may run fewer.
    #[must_use]
    pub fn pairs(&self) -> &[BoundPair] {
        &self.pairs
    }

    /// Whether the attempt may carve, so that the generator may go on to the next: whether it
    /// holds a pair the engine may run ([`AttemptBound::pairs`]).
    #[must_use]
    pub fn may_carve(&self) -> bool {
        !self.pairs.is_empty()
    }
}

/// A pair of two stars, neither a brown dwarf, that the binary engine may run: its node, its two
/// stars and its drawn periastron.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundPair {
    node: NodeIndex,
    stars: [StarIndex; 2],
    periastron: Metres,
}

impl BoundPair {
    /// The pair's node in its attempt's hierarchy.
    #[must_use]
    pub const fn node(&self) -> NodeIndex {
        self.node
    }

    /// Its two stars: the inner member, the engine's primary, then the outer member, as
    /// [`PairTimeline::stars`](crate::stellar::system::PairTimeline::stars) gives a pair that was
    /// run.
    #[must_use]
    pub const fn stars(&self) -> [StarIndex; 2] {
        self.stars
    }

    /// The drawn orbit's periastron, a (1 − e), m: its
    /// [`periapsis`](crate::orbit::KeplerElements::periapsis), bit for bit.
    #[must_use]
    pub const fn periastron(&self) -> Metres {
        self.periastron
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::super::hierarchy::{MAX_REDRAWS, draw_hierarchy_of_composition};
    use super::super::testing::records_near;
    use super::*;
    use crate::Seed;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::id::Layer;
    use crate::stellar::binary::carve::testing as carve_testing;
    use crate::stellar::system::{SystemStars, draw_metallicity};

    /// The galaxy of the bound's tests: the Milky Way fixture of the ruling's measurement
    /// (`decision-p11-t16-hierarchy-bound.md`) and of P11.T16's probe.
    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// Near the Sun: 26,000 ly out on the +y axis and 68 ly above the plane, the probe's point.
    fn sun() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("inside the root cube")
    }

    /// In the bulge: 2,000 ly out on the +y axis and 300 ly above the plane, the probe's point.
    fn bulge() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 2_000.0, 300.0]).expect("inside the root cube")
    }

    /// The stellar layers.
    const LAYERS: [Layer; 5] = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];

    /// What the bound and the generator gave one record.
    #[derive(Debug, Clone, Copy, Default)]
    struct Checked {
        /// The attempts the bound lists.
        listed: u8,
        /// Whether the bound lists the fallback.
        fallback: bool,
        /// Whether the generator kept a later attempt than the first.
        redrawn: bool,
        /// Whether the generator kept the fallback.
        fell_back: bool,
    }

    /// `bound`'s cover is the ruling's for a record that the grid redraws if `redrawn`: attempts
    /// from 0 with no gap, each but the last one that may carve, the last one that cannot unless it
    /// is attempt 7, the fallback after a last attempt 7 that may carve, and the record's primary
    /// at every attempt and as the fallback.
    fn assert_cover(bound: &HierarchyBound, record: &SystemRecord, redrawn: bool) {
        let id = record.id();
        let attempts = bound.attempts();
        let (last, earlier) = attempts.split_last().expect("attempt 0 is always listed");
        for (listed, n) in attempts.iter().zip(0..) {
            assert_eq!(listed.attempt().get(), n, "{id:?}: attempts in order");
            let primary = listed.stars()[0];
            assert_eq!(primary.body(), crate::id::BodyId::new(id, 0), "{id:?}");
            assert_eq!(primary.kind(), bound.primary().kind(), "{id:?}");
            assert_same_bits(
                primary.initial_mass().value(),
                record.primary_initial_mass().value(),
            );
        }
        assert!(
            earlier.iter().all(AttemptBound::may_carve),
            "{id:?}: a later attempt is listed after one that cannot carve"
        );
        if redrawn {
            let last_attempt = last.attempt().next().is_none();
            assert!(
                !last.may_carve() || last_attempt,
                "{id:?}: the cover stops at attempt {} that may carve",
                last.attempt().get()
            );
            assert_eq!(
                bound.fallback().is_some(),
                last_attempt && last.may_carve(),
                "{id:?}: the fallback is listed after a last attempt that may carve, and only then"
            );
        } else {
            assert_eq!(
                attempts.len(),
                1,
                "{id:?}: a record not redrawn lists one attempt"
            );
            assert_eq!(bound.fallback(), None, "{id:?}");
        }
        if let Some(fallback) = bound.fallback() {
            assert_eq!(
                fallback,
                bound.primary(),
                "{id:?}: the fallback is the primary"
            );
        }
    }

    /// `kept`'s stars and the pairs the engine may run are `listed`'s, each mass and periastron
    /// bit for bit, and every pair the generator ran is one of them.
    fn assert_listed(listed: &AttemptBound, kept: &SystemStars) {
        let id = kept.record().id();
        let hierarchy = kept.hierarchy();
        assert_eq!(listed.hierarchy(), hierarchy, "{id:?}: the attempt's draw");
        assert_eq!(listed.stars().len(), hierarchy.stars().len(), "{id:?}");
        for (bound, drawn) in listed.stars().iter().zip(hierarchy.stars()) {
            assert_eq!(
                (bound.body(), bound.kind()),
                (drawn.body(), drawn.kind()),
                "{id:?}"
            );
            assert_same_bits(bound.initial_mass().value(), drawn.initial_mass().value());
        }
        let star_pairs: Vec<(NodeIndex, [StarIndex; 2])> = hierarchy
            .pairs()
            .filter_map(|(node, _)| match *hierarchy.node(node) {
                HierarchyNode::Pair { inner, outer, .. } => {
                    match (*hierarchy.node(inner), *hierarchy.node(outer)) {
                        (HierarchyNode::Star(a), HierarchyNode::Star(b))
                            if hierarchy.star(a).kind() == SlotKind::Star
                                && hierarchy.star(b).kind() == SlotKind::Star =>
                        {
                            Some((node, [a, b]))
                        }
                        _ => None,
                    }
                }
                HierarchyNode::Star(_) => None,
            })
            .collect();
        let bound_pairs: Vec<(NodeIndex, [StarIndex; 2])> = listed
            .pairs()
            .iter()
            .map(|p| (p.node(), p.stars()))
            .collect();
        assert_eq!(bound_pairs, star_pairs, "{id:?}: the star–star pairs");
        for pair in listed.pairs() {
            let HierarchyNode::Pair { orbit, .. } = hierarchy.node(pair.node()) else {
                panic!("{id:?}: a bound pair's node is a pair");
            };
            assert_same_bits(pair.periastron().value(), orbit.periapsis().value());
        }
        for run in kept.pairs() {
            assert!(
                bound_pairs.contains(&(run.node(), run.stars())),
                "{id:?}: the generator ran pair {:?} that the bound does not list",
                run.node()
            );
        }
    }

    /// The bound of `record` at its own composition, checked against the system the generator
    /// keeps: the cover is the ruling's, the kept attempt is listed, and its stars and pairs are
    /// the bound's, bit for bit.
    fn check(galaxy: &Galaxy, record: &SystemRecord) -> Checked {
        let bound = hierarchy_bound(galaxy, record, &draw_metallicity(galaxy, record));
        assert_cover(
            &bound,
            record,
            grid_redraws(record, grid_multiplicity(record)),
        );
        let kept = SystemStars::generate(galaxy, record);
        let id = record.id();
        let listed = bound.attempt(kept.attempt()).unwrap_or_else(|| {
            panic!(
                "{id:?}: the generator kept attempt {} and the bound lists {}",
                kept.attempt().get(),
                bound.attempts().len()
            )
        });
        let fell_back = listed.hierarchy() != kept.hierarchy();
        if fell_back {
            let fallback = bound.fallback().unwrap_or_else(|| {
                panic!("{id:?}: the kept system is not its attempt's and no fallback is listed")
            });
            assert_eq!(
                kept.hierarchy().stars(),
                [*fallback],
                "{id:?}: the fallback"
            );
            assert!(kept.pairs().is_empty(), "{id:?}");
            assert_eq!(
                kept.attempt().next(),
                None,
                "{id:?}: the fallback is kept only after the last attempt"
            );
        } else {
            assert_listed(listed, &kept);
        }
        Checked {
            listed: u8::try_from(bound.attempts().len()).expect("at most eight attempts"),
            fallback: bound.fallback().is_some(),
            redrawn: kept.attempt() > RedrawAttempt::FIRST,
            fell_back,
        }
    }

    /// What one stratum's records came to.
    #[derive(Debug, Clone, Copy, Default)]
    struct Tally {
        records: u64,
        /// Records by the number of attempts listed, 1 to 8.
        listed: [u64; MAX_REDRAWS as usize],
        fallbacks: u64,
        redrawn: u64,
        fell_back: u64,
    }

    impl Tally {
        fn add(&mut self, checked: Checked) {
            self.records += 1;
            self.listed[usize::from(checked.listed) - 1] += 1;
            self.fallbacks += u64::from(checked.fallback);
            self.redrawn += u64::from(checked.redrawn);
            self.fell_back += u64::from(checked.fell_back);
        }

        fn merge(&mut self, other: &Self) {
            self.records += other.records;
            for (sum, n) in self.listed.iter_mut().zip(other.listed) {
                *sum += n;
            }
            self.fallbacks += other.fallbacks;
            self.redrawn += other.redrawn;
            self.fell_back += other.fell_back;
        }
    }

    /// [`check`] over `records`, in four shares on threads of their own, or one after another on
    /// wasm32-wasip1, which has none; share k takes the records k, k + 4, …. The tallies are
    /// counts, the same on any number of shares.
    fn check_all(galaxy: &Galaxy, records: &[SystemRecord]) -> Tally {
        const SHARES: usize = 4;
        let share = |k: usize| {
            let mut tally = Tally::default();
            for record in records.iter().skip(k).step_by(SHARES) {
                tally.add(check(galaxy, record));
            }
            tally
        };
        #[cfg(not(target_family = "wasm"))]
        let parts: Vec<Tally> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..SHARES)
                .map(|k| {
                    let share = &share;
                    scope.spawn(move || share(k))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a share's thread"))
                .collect()
        });
        #[cfg(target_family = "wasm")]
        let parts: Vec<Tally> = (0..SHARES).map(share).collect();
        let mut total = Tally::default();
        for part in &parts {
            total.merge(part);
        }
        total
    }

    /// 10³ records of `layer` near the Sun: the cover is the ruling's, and every system the
    /// generator keeps is the bound's at its attempt.
    fn the_bound_holds_near_the_sun(layer: Layer) {
        let galaxy = galaxy();
        let records = records_near(&galaxy, layer, &sun(), 1_000);
        assert_eq!(records.len(), 1_000);
        let tally = check_all(&galaxy, &records);
        println!("layer {layer:?} near the Sun: {tally:?}");
        assert_eq!(tally.records, 1_000);
    }

    #[test]
    fn the_bound_holds_for_layer_a_near_the_sun() {
        the_bound_holds_near_the_sun(Layer::A);
    }

    #[test]
    fn the_bound_holds_for_layer_b_near_the_sun() {
        the_bound_holds_near_the_sun(Layer::B);
    }

    #[test]
    fn the_bound_holds_for_layer_c_near_the_sun() {
        the_bound_holds_near_the_sun(Layer::C);
    }

    #[test]
    fn the_bound_holds_for_layer_d_near_the_sun() {
        the_bound_holds_near_the_sun(Layer::D);
    }

    #[test]
    fn the_bound_holds_for_layer_e_near_the_sun() {
        the_bound_holds_near_the_sun(Layer::E);
    }

    /// P11.T16's slow test: 10⁵ records of each of layers A, B and C and 2 × 10⁴ of each of D
    /// and E, near the Sun and again in the bulge. Every system the generator keeps is the bound's
    /// at an attempt it lists, its stars and the pairs the engine may run bit for bit, and the
    /// shares of records by the number of attempts listed are recorded, with the redrawn systems
    /// found: at least 50 in all (the ruling expects redraws of 0.25–0.5% in C to E).
    #[test]
    #[ignore = "slow: 6.8 × 10⁵ systems generated, their pairs run through the binary engine"]
    fn the_hierarchy_bound_holds_for_generated_systems() {
        let galaxy = galaxy();
        let mut total = Tally::default();
        for (place, at) in [("near the Sun", sun()), ("in the bulge", bulge())] {
            for layer in LAYERS {
                let n = match layer {
                    Layer::A | Layer::B | Layer::C => 100_000,
                    _ => 20_000,
                };
                let records = records_near(&galaxy, layer, &at, n);
                assert_eq!(records.len(), n, "{layer:?} {place}");
                let tally = check_all(&galaxy, &records);
                let shares: Vec<String> = tally
                    .listed
                    .iter()
                    .map(|&k| format!("{:.4}", u64_ratio(k, tally.records)))
                    .collect();
                println!(
                    "layer {layer:?} {place}: {} records; listing 1–8 attempts {:?} (shares \
                     {}); fallbacks listed {}; redrawn {}; fallbacks kept {}",
                    tally.records,
                    tally.listed,
                    shares.join(", "),
                    tally.fallbacks,
                    tally.redrawn,
                    tally.fell_back
                );
                total.merge(&tally);
            }
        }
        println!("all: {total:?}");
        assert!(
            total.redrawn >= 50,
            "only {} redrawn systems were found, too few to test the cover",
            total.redrawn
        );
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "counts of at most 10⁵, which an f64 holds exactly"
    )]
    fn u64_ratio(n: u64, of: u64) -> f64 {
        n as f64 / of as f64
    }

    /// `carve.rs`'s carved record, a 12 M☉ primary 30 Myr old whose first attempt holds an X-ray
    /// binary: the bound lists the later attempt the generator keeps, and that attempt's system.
    #[test]
    fn the_carved_record_lists_the_attempt_it_keeps() {
        let galaxy = carve_testing::galaxy();
        let (index, mass, age) = carve_testing::CARVED;
        let record = carve_testing::record(&galaxy, index, mass, age);
        let kept = SystemStars::generate(&galaxy, &record);
        assert!(
            kept.attempt() > RedrawAttempt::FIRST,
            "the record is redrawn"
        );
        let checked = check(&galaxy, &record);
        assert!(checked.redrawn && !checked.fell_back);
        assert!(usize::from(checked.listed) > usize::from(kept.attempt().get()));
        // Under a forced context the grid does not redraw it, so attempt 0 alone is listed,
        // though it may carve.
        let ctx = MultiplicityContext::ForcedMultiple {
            max_separation: None,
        };
        let forced = bound_in(&galaxy, &record, &draw_metallicity(&galaxy, &record), ctx);
        assert_cover(&forced, &record, false);
        let kept = SystemStars::generate_in(&galaxy, &record, ctx);
        assert_eq!(kept.attempt(), RedrawAttempt::FIRST);
        assert_listed(&forced.attempts()[0], &kept);
    }

    /// A record whose eight attempts all hold a pair the engine may run lists all eight, then the
    /// fallback; one whose attempt n holds none, after earlier ones that all do, lists attempts 0
    /// to n. Found among layer E's records near the Sun, where nine in ten attempts hold such a
    /// pair, by a test of each attempt's draw written apart from the bound's.
    #[test]
    fn eight_attempts_that_may_carve_are_all_listed_then_the_fallback() {
        let galaxy = galaxy();
        let holds_a_pair = |h: &SystemHierarchy| {
            h.pairs().any(|(node, _)| {
                let HierarchyNode::Pair { inner, outer, .. } = *h.node(node) else {
                    return false;
                };
                [inner, outer].iter().all(|&member| match *h.node(member) {
                    HierarchyNode::Star(s) => h.star(s).kind() == SlotKind::Star,
                    HierarchyNode::Pair { .. } => false,
                })
            })
        };
        let mut found_eight = false;
        let mut stopped = [false; MAX_REDRAWS as usize];
        for record in records_near(&galaxy, Layer::E, &sun(), 400) {
            let composition = draw_metallicity(&galaxy, &record);
            let carving = RedrawAttempt::all()
                .take_while(|&attempt| {
                    holds_a_pair(&draw_hierarchy_of_composition(
                        &galaxy,
                        &record,
                        &composition,
                        MultiplicityContext::Free,
                        attempt,
                    ))
                })
                .count();
            let bound = hierarchy_bound(&galaxy, &record, &composition);
            assert_cover(&bound, &record, true);
            if carving == usize::from(MAX_REDRAWS) {
                assert_eq!(bound.attempts().len(), carving, "{:?}", record.id());
                // What `after_last_attempt` keeps.
                let alone = SystemHierarchy::single(
                    record.id(),
                    record.primary_initial_mass(),
                    SlotKind::Star,
                );
                assert_eq!(bound.fallback(), Some(&alone.stars()[0]));
                found_eight = true;
            } else {
                assert_eq!(bound.attempts().len(), carving + 1, "{:?}", record.id());
                assert_eq!(bound.fallback(), None);
                stopped[carving] = true;
            }
        }
        assert!(
            found_eight,
            "no record of 400 has eight attempts that may carve"
        );
        assert!(
            stopped[0] && stopped[1],
            "records stopping at attempts 0 and 1 are found: {stopped:?}"
        );
    }

    /// A brown dwarf is single and is never redrawn: its bound is attempt 0 alone, its one star.
    #[test]
    fn a_brown_dwarf_lists_attempt_0_alone() {
        let galaxy = galaxy();
        for record in records_near(&galaxy, Layer::BrownDwarf, &sun(), 50) {
            let bound = hierarchy_bound(&galaxy, &record, &draw_metallicity(&galaxy, &record));
            assert_cover(&bound, &record, false);
            assert_eq!(bound.attempts()[0].stars().len(), 1, "{:?}", record.id());
            assert!(!bound.attempts()[0].may_carve());
        }
    }

    /// Each attempt the bound lists is the draw at that attempt alone, bit for bit, though the
    /// bound's attempts share their limits and the direct construction's period law
    /// (`RecordDraw`): every word read is the draw's own, and no attempt depends on those drawn
    /// before it.
    #[test]
    fn every_attempt_listed_is_the_draw_at_that_attempt_alone() {
        let galaxy = galaxy();
        let mut redrawn_direct = 0_u32;
        for layer in [Layer::C, Layer::D, Layer::E] {
            for record in records_near(&galaxy, layer, &sun(), 150) {
                let composition = draw_metallicity(&galaxy, &record);
                let bound = hierarchy_bound(&galaxy, &record, &composition);
                for listed in bound.attempts() {
                    let alone = draw_hierarchy_of_composition(
                        &galaxy,
                        &record,
                        &composition,
                        MultiplicityContext::Free,
                        listed.attempt(),
                    );
                    assert_eq!(listed.hierarchy(), &alone, "{:?}", record.id());
                    for (a, b) in listed.stars().iter().zip(alone.stars()) {
                        assert_same_bits(a.initial_mass().value(), b.initial_mass().value());
                    }
                    for ((_, a), (_, b)) in listed.hierarchy().pairs().zip(alone.pairs()) {
                        assert_same_bits(a.periapsis().value(), b.periapsis().value());
                        assert_same_bits(a.semi_major_axis().value(), b.semi_major_axis().value());
                        assert_same_bits(a.period().value(), b.period().value());
                        assert_same_bits(a.eccentricity().value(), b.eccentricity().value());
                    }
                }
                redrawn_direct += u32::from(
                    bound.attempts().len() > 1 && record.primary_initial_mass().value() >= 3.0,
                );
            }
        }
        assert!(
            redrawn_direct > 50,
            "{redrawn_direct} direct records list later attempts"
        );
    }

    /// The bound reads no word of its own: its code (this file up to its tests, comments left
    /// out) opens no stream, reads no word or mark and takes no star's draws. It draws only
    /// through the draw's own functions, the stars of `RecordDraw::stars_at` and the companion of
    /// `substellar::with_companion`, so every word it reads is one the draw reads (P11.T16).
    #[test]
    fn the_bound_reads_no_word_of_its_own() {
        let source = include_str!("bound.rs");
        let (code, _) = source
            .split_once("#[cfg(test)]\nmod tests")
            .expect("bound.rs has a test module to stop at");
        let code: String = code
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(code.len() > 1_000, "bound.rs was not read");
        for read in [
            "Stream",
            "stream",
            "tags::",
            "DomainTag",
            "ObjectKey",
            "word",
            "Mark",
            "mark(",
            "uniform",
            "StarDraws",
            "seed(",
            "rng::",
        ] {
            assert!(!code.contains(read), "the bound's code mentions `{read}`");
        }
        for draw in [".stars_at(", "substellar::with_companion("] {
            assert!(code.contains(draw), "the bound draws through `{draw}`");
        }
    }

    /// The bound is a pure function of its record: the same in any order, and twice the same.
    #[test]
    fn the_bound_is_the_same_in_any_order() {
        let galaxy = galaxy();
        let records: Vec<SystemRecord> = LAYERS
            .iter()
            .flat_map(|&layer| records_near(&galaxy, layer, &sun(), 20))
            .collect();
        assert_order_independent(&records, |record| {
            hierarchy_bound(&galaxy, record, &draw_metallicity(&galaxy, record))
        });
    }
}
