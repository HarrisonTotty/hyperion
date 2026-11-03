//! The luminous blue variables' class: the young field's massive stars whose window near the
//! Humphreys–Davidson limit overlaps the source horizon, placed on 4,096 ly cells of their own
//! (plan 09, P09.T43).
//!
//! A star is a luminous blue variable while it lies beyond the Humphreys–Davidson limit and is
//! hotter than 8,000 K (plan 06's [`is_luminous_blue_variable`], after Hurley, Pols and Tout 2000,
//! section 7.1, and Humphreys and Davidson 1994), and its window ([`SystemStars::lbv_window`],
//! P06.T24.a) runs from the first to the last age at which it is one. A system whose window
//! overlaps the source horizon, −(H + L) to +H, is one that some observer in the root cube can see
//! as one during play, and the host of plan 06's giant eruptions, which plan 12's P12.T5.c finds
//! by walking this class. Such systems are a few hundred in a galaxy of 10¹¹ (471 at the Milky Way
//! fixture, the class's golden), so they are a catalogue class, as the recent core collapses are
//! (P09.T33.a): a Poisson process of
//! their own, which this module places by thinning on the class grid of P09.T32
//! ([`ClassProcess`]). The grid side's carve-out, which drops the same systems from the cells, is
//! P09.T35's, through `lbv_claims`.
//!
//! # What the class follows
//!
//! The grid's layer E places a system of component c at x with initial mass m in band E and age a
//! at the epoch at the intensity `s_c n_c(x) p(m) f_c(a)`, its \[Fe/H\] drawn from the component's
//! law there. Its window is a function of m, its composition and a alone: no draw of the star's
//! moves it (Reimers η, the one the living track reads, moves none; a test holds it). A window from
//! age `a_s` to `a_e` overlaps the horizon if and only if a lies in [`a_s` − H, `a_e` + H + L].
//! The sampler, [`tables::lbv`] (fitted by plan 15's P15.T10.a, its shape [`sampler`]'s), holds
//! five mass nodes from 32 M☉, below which no star has a window, and for each the ages within which
//! every window of its masses lies at any metallicity. The class's candidates are the grid's
//! intensity over the nodes' masses and ages so widened, and its test keeps those whose own window
//! overlaps. Only the young field has stars of those ages, the components whose age density is
//! positive there: the young thin disc, all of whose stars are young, and the nuclear disc's two
//! components, a twentieth of whose stars formed in the last gigayear
//! (`AgeDistribution::nuclear_disc`, after Nogueras-Lara et al. 2020, Nature Astronomy 4, 377).
//! From generator version 26 the field gives the features their share (plan 09, P09.T2.c), and the
//! class follows the field as the grid places it: each component's layer-E share is the field's,
//! and the young disc's ages the young field's.
//!
//! # The envelope and a candidate
//!
//! The conditional density of initial mass is the galaxy's mass function over each node's masses,
//! weighted by the node's span of ages at the epoch, [first − H, last + H + L]. A cell's bound is
//! the sum over the young field's components of each one's layer-E density bound over the cell's
//! closed box ([`Component::bound`](crate::galaxy::fields::Component::bound)) × its candidates per
//! layer-E system: its largest age density over the sampler's ages × Σ over the nodes of the
//! node's share of band E × its span. A candidate, on its own streams (Design note 23):
//!
//! 1. draws its site, its position at the epoch, uniformly in the cell and is thinned on the
//!    envelope there, each component's layer-E density × its candidates per system, ÷ the bound,
//!    the same mark picking its component by the odds of those terms (`class.lbv.site`);
//! 2. picks a node by the nodes' weights and draws its primary's initial mass among the node's
//!    from the galaxy's mass function (`class.lbv.mass`);
//! 3. draws its age at the epoch uniformly over the node's span and is thinned on its component's
//!    age density there ÷ the component's largest (`class.lbv.age`);
//! 4. draws its \[Fe/H\] from its component's law at its site and age (`class.lbv.metallicity`), as
//!    plan 06's `draw_metallicity` reads a grid record's;
//! 5. draws its velocity at the site from its component's law, plan 08's draw
//!    (`class.lbv.velocity`), the one velocity its test and its entry read (rule 6).
//!
//! The candidates kept are a Poisson process of intensity `s_c n_c(x) p(m) f_c(a)` over the nodes'
//! masses and ages, with the composition and velocity laws of the grid's systems: the grid's there,
//! exactly.
//!
//! # The test
//!
//! A candidate is an entry if and only if:
//!
//! - it passes the record-only prefilter [`may_host`], its initial mass in a node and its age in
//!   that node's span, which binds both sides: plan 08's rule 5, "its prefilter on the record alone
//!   binds its catalogue side in the same way" (every candidate passes it by construction);
//! - its primary's window at its draws of attempt 0 overlaps the source horizon
//!   ([`horizon_window`]), its stars a single star's at its site, as the grid's claim builds them;
//! - the core-collapse class does not claim it: a system that passes both claims is a
//!   core-collapse entry, so the two classes are disjoint. That claim is the one shared test on
//!   the grid's terms: plan 08's [`death_in_interval`] on its primary's marks, its explosion site
//!   its epoch position moved along its velocity to T, as plan 08's `explosion_site` moves a grid
//!   record's, then [`DeathMarks::of`] and [`claims`] in the field.
//!
//! On the grid's side the claim, `lbv_claims`, asks the same of a layer-E record, and from 27 of a
//! field system's alone: the class's candidates are the field's, so a displaced class's system
//! (a runaway, a walkaway, a kicked remnant) stays on the grid (P09.T38).
//!
//! **No redraw (lean, 2026-10-31).** The plan has a candidate redraw its star with
//! `StarDraws::for_attempt` until its window overlaps. No draw moves a window, so a redraw would
//! keep every candidate whose first attempt fails failing, at the cost of a track per attempt: the
//! class thins on attempt 0 instead, which is exact (P09.T33.a's lean, for the same reason). Its
//! records are read at attempt 0, as every catalogue system's.
//!
//! # The entry
//!
//! An entry has one member, its system, at its site at the epoch and moving at its velocity
//! throughout ([`CatalogueMotion::straight`]), as a grid record drifts: a primary that dies before
//! +H without a claimed shell, a collapse to a black hole, moves on as the grid's record would.
//! Its record is a catalogue system's ([`SystemOrigin::CatalogueSystem`]), its stars plan 06's at
//! its composition with the model's own multiplicity, its window the one the test read.
//!
//! [`is_luminous_blue_variable`]: crate::stellar::classify::is_luminous_blue_variable
//! [`tables::lbv`]: crate::tables::lbv
//! [`death_in_interval`]: crate::galaxy::displaced::death_in_interval

use super::core_collapse::class_site_velocity;
use super::grid::{CatalogueMember, ClassProcess, site_stream};
use super::ids::CatalogueCellKey;
use super::registry::ClassId;
use super::supernova::{DeathMarks, Surroundings, claims};
use super::system::{CatalogueMotion, CatalogueSystem};
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::bounds::{BOUND_MARGIN, CellBox};
use crate::galaxy::displaced::{ExplosionSite, PlacementClass, death_in_interval_under};
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{CatalogueSystemId, Layer, SystemId};
use crate::rng::{ObjectKey, Stream, Threshold, tags};
use crate::stellar::Composition;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::{SystemStars, draw_metallicity, primary_draws, primary_eta};
use crate::tables::lbv::LBV_SAMPLER;
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L, SourceHorizon, UniverseTime};
use crate::units::{Dex, HeliumExcess, PerCubicLightYear, Seconds, SolarMasses, Years};

pub mod sampler;

/// The number of the sampler's mass nodes.
pub(crate) const NODES: usize = LBV_SAMPLER.nodes().len();

/// The ages at the epoch at which a star of the sampler's node `k` may host: from the node's first
/// age less H to its last plus H + L, the ages at which a window of the node's can overlap the
/// source horizon.
#[must_use]
pub(crate) fn host_ages(k: usize) -> (Years, Years) {
    let node = LBV_SAMPLER.nodes()[k];
    let h = CLOCK_WINDOW_H.as_julian_years_f64();
    let l = LIGHT_CROSSING_L.as_julian_years_f64();
    (
        Years::new(node.first().value() - h),
        Years::new(node.last().value() + h + l),
    )
}

/// Whether a system whose primary has initial mass `mass` and is `age_at_epoch` old at the epoch
/// may be a luminous-blue-variable host, on its record alone: its mass in one of the sampler's
/// nodes and its age within that node's [first − H, last + H + L] (plan 09, P09.T43).
///
/// The prefilter of both sides: the grid's claim (`lbv_claims`) asks a star's window only of a
/// record it passes, and the class keeps only candidates it passes (plan 08's rule 5). Every star
/// of the sampler's masses whose window overlaps the source horizon passes it, so it costs the
/// class nothing. At the Milky Way fixture 0.22% of the young disc's layer-E systems pass, and
/// some six in a million of all layer E, whose other components' ages are all beyond it.
///
/// # Examples
///
/// A 60 M☉ star at 4 Myr may be one; at 10 Myr it is long dead, and a 20 M☉ star never is:
///
/// ```
/// use hyperion_sim::galaxy::catalogue_classes::lbv::may_host;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// assert!(may_host(SolarMasses::new(60.0), Years::new(4.0e6)));
/// assert!(!may_host(SolarMasses::new(60.0), Years::new(1.0e7)));
/// assert!(!may_host(SolarMasses::new(20.0), Years::new(4.0e6)));
/// ```
#[must_use]
pub fn may_host(mass: SolarMasses, age_at_epoch: Years) -> bool {
    LBV_SAMPLER.node_of(mass).is_some_and(|k| {
        let (first, last) = host_ages(k);
        let age = age_at_epoch.value();
        first.value() <= age && age <= last.value()
    })
}

/// The primary's luminous-blue-variable window of `stars`, clock times, if it overlaps the source
/// horizon, −(H + L) to +H (plan 09, P09.T43): its start at or before +H and its end at or after
/// −(H + L).
///
/// The window is plan 06's [`SystemStars::lbv_window`], within the part of the primary's life its
/// track is built for, which may run past +H: a 150 M☉ star's main-sequence window can start after
/// it.
#[must_use]
pub fn horizon_window(stars: &SystemStars) -> Option<(UniverseTime, UniverseTime)> {
    stars
        .lbv_window()
        .filter(|&(start, end)| start <= SourceHorizon::END && end >= SourceHorizon::START)
}

/// Whether the luminous blue variables' class claims the grid record `record`: a layer-E record
/// that passes the prefilter [`may_host`] and whose primary's window overlaps the source horizon
/// ([`horizon_window`]), its stars a single star's at its own metallicity draw (plan 09, P09.T43).
///
/// It is asked after the core-collapse claim: a record that passes both is a core-collapse entry,
/// which this does not ask. It is asked of every layer-E record, with or without an explosion
/// site: most hosts do not die in the supernova test's interval, so plan 03's site gate
/// (`claims_with` asks its claim only of a record with a recent death) must not hold it. It is
/// the class's test on the grid's side: P09.T35's carve-out asks it under its claimed arm, from
/// generator version 26 (`placement::candidate::catalogue_claims`), and until then the grid keeps
/// every system the class places too. From 27 it claims the field's systems alone
/// ([`claims_its_placement`]).
///
/// # Panics
///
/// As [`draw_metallicity`], for a record that is not a grid record of `galaxy`.
#[must_use]
pub(crate) fn lbv_claims(galaxy: &Galaxy, record: &SystemRecord) -> bool {
    record.layer() == Layer::E
        && claims_its_placement(galaxy, record)
        && may_host(record.primary_initial_mass(), record.age_at_epoch())
        && horizon_window(&SystemStars::generate_with(
            galaxy,
            record,
            &draw_metallicity(galaxy, record),
            MultiplicityContext::ForcedSingle,
        ))
        .is_some()
}

/// Whether the class may claim a grid record of `record`'s placement class: from 27 (P09.T33.b's
/// runaways) a field system's alone, not one of plan 08's displaced classes (a runaway, a walkaway
/// or a kicked remnant).
///
/// The class draws the field's systems at their share of each component
/// ([`Galaxy::field_component_share`], the stay share under plan 08's displaced classes), so no
/// candidate of it can be a displaced system: up to 26 a displaced record whose window overlapped
/// the source horizon was claimed by nobody, a lost system (P09.T38's complementarity, a fault
/// fixed at 27). From 27 the grid keeps it, a host the class does not list, until the class follows
/// the runaways as the core collapses do (P09.T43's runaway half).
#[must_use]
pub(crate) fn claims_its_placement(_galaxy: &Galaxy, record: &SystemRecord) -> bool {
    !matches!(record.placement_class(), PlacementClass::Displaced { .. })
}

/// One of the young field's components, as the class thins on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvComponent {
    id: ComponentId,
    share: f64,
    rate_per_year: f64,
    candidates_per_system: f64,
}

impl LbvComponent {
    /// The component.
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        self.id
    }

    /// Its layer-E share: the fraction of its systems whose primaries are of band E.
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }

    /// The bound on its age density over the sampler's ages, per Julian year: the largest it has
    /// there, raised by 2⁻⁴⁰ ([`BOUND_MARGIN`]).
    #[must_use]
    pub const fn rate_per_year(&self) -> f64 {
        self.rate_per_year
    }

    /// The bound on its candidates per layer-E system, a count:
    /// [`rate_per_year`](Self::rate_per_year) × the nodes' summed weights.
    #[must_use]
    pub const fn candidates_per_system(&self) -> f64 {
        self.candidates_per_system
    }
}

/// One of the sampler's mass nodes, as the class draws from it in a galaxy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvMassNode {
    masses: (SolarMasses, SolarMasses),
    ages: (Years, Years),
    share: f64,
    weight: Years,
}

impl LbvMassNode {
    /// The initial masses the node covers, M☉: from its own to the next node's.
    #[must_use]
    pub const fn masses(&self) -> (SolarMasses, SolarMasses) {
        self.masses
    }

    /// The ages at the epoch at which a star of the node may host, Julian years: the node's
    /// first age less H, to its last plus H + L.
    #[must_use]
    pub const fn ages(&self) -> (Years, Years) {
        self.ages
    }

    /// The node's share of band E's systems by the galaxy's mass function.
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }

    /// The node's weight in the conditional density of mass: its share × its span of ages.
    #[must_use]
    pub const fn weight(&self) -> Years {
        self.weight
    }
}

/// The luminous blue variables' process (plan 09, P09.T43): the young field's massive stars whose
/// window overlaps the source horizon (module documentation).
///
/// Built from the galaxy alone, cheaply, each time the production lookup visits the class: it
/// holds the young field's components, with their layer-E shares and age-density bounds, and the
/// sampler's nodes weighted by the galaxy's mass function.
///
/// # Examples
///
/// The class's cells along the disc, on both sides of the plane, hold some luminous-blue-variable
/// hosts, each resolving by its ID to itself, its window overlapping the source horizon. (The cells
/// about the centre are left out: each draws some 10⁶ candidates on the nuclear disc's peak
/// density.)
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::lbv::LbvProcess;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, resolve_entry,
/// };
/// use hyperion_sim::time::SourceHorizon;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = LbvProcess::new(&galaxy);
/// let mut hosts = 0;
/// for y in 1..8 {
///     for z in [-1, 0] {
///         let key = CatalogueCellKey::new(ClassId::LUMINOUS_BLUE_VARIABLE, [0, y, z])?;
///         for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
///             assert_eq!(resolve_entry(&galaxy, &process, entry.id())?.entry(), entry);
///             let (start, end) = entry.value().window();
///             assert!(start <= SourceHorizon::END && end >= SourceHorizon::START);
///             hosts += 1;
///         }
///     }
/// }
/// assert!(hosts > 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LbvProcess {
    /// The young field's components, in component order.
    components: Vec<LbvComponent>,
    /// The sampler's nodes in this galaxy, in ascending mass.
    nodes: [LbvMassNode; NODES],
    /// The nodes' weights summed in order.
    weight: Years,
}

/// A luminous-blue-variable candidate the thinning kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct LbvCandidate {
    id: CatalogueSystemId,
    site: GalacticPosition,
    component: ComponentId,
    population: Population,
    mass: SolarMasses,
    age_at_epoch: Years,
    composition: Composition,
    velocity: GalacticVelocity,
}

/// A luminous-blue-variable host: a system of the young field whose primary's window near the
/// Humphreys–Davidson limit overlaps the source horizon (plan 09, P09.T43).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvEntry {
    record: SystemRecord,
    component: ComponentId,
    composition: Composition,
    motion: CatalogueMotion,
    window: (UniverseTime, UniverseTime),
}

impl LbvEntry {
    /// The record of the entry's system, of origin [`SystemOrigin::CatalogueSystem`], at its site.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The young field's component the system was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// The composition every star of the system shares.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// How the system moves: at its velocity throughout, through its site at the epoch.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// The clock interval from the first to the last time at which the primary is a luminous blue
    /// variable, as its track is built for its age, which overlaps the source horizon: plan 06's
    /// [`SystemStars::lbv_window`]. Plan
    /// 06's giant eruptions (`stellar::events::lbv`) fall in it.
    #[must_use]
    pub const fn window(&self) -> (UniverseTime, UniverseTime) {
        self.window
    }

    /// The system's stars: plan 06's at its composition, with the model's own multiplicity. The
    /// primary's window is the entry's.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        SystemStars::generate_with(
            galaxy,
            &self.record,
            &self.composition,
            MultiplicityContext::Free,
        )
    }
}

impl LbvProcess {
    /// The process in `galaxy`: its young field's components, their age-density bounds, and the
    /// sampler's nodes weighted by the galaxy's mass function.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        let fields = galaxy.fields();
        let mass_function = galaxy.mass_function();
        let band = mass_function.integral(MassBand::E.lo(), MassBand::E.hi());
        let nodes: [LbvMassNode; NODES] = core::array::from_fn(|k| {
            let masses = LBV_SAMPLER.masses(k);
            let ages = host_ages(k);
            let share = mass_function.integral(masses.0.value(), masses.1.value()) / band;
            LbvMassNode {
                masses,
                ages,
                share,
                weight: Years::new(share * (ages.1.value() - ages.0.value())),
            }
        });
        let weight = Years::new(
            nodes
                .iter()
                .fold(0.0, |sum, node| sum + node.weight.value()),
        );
        let earliest = nodes
            .iter()
            .fold(f64::INFINITY, |a, node| a.min(node.ages.0.value()));
        let latest = nodes
            .iter()
            .fold(f64::NEG_INFINITY, |a, node| a.max(node.ages.1.value()));
        let components = fields
            .component_ids()
            .filter_map(|id| {
                // The field's share and ages: the budget's until P09.T2.c's version 27.
                let share = galaxy.field_component_share(MassBand::E, id);
                let rate = galaxy
                    .field_ages(id)
                    .max_pdf_between(Years::new(earliest), Years::new(latest));
                (share > 0.0 && rate > 0.0).then(|| {
                    let rate = rate * (1.0 + BOUND_MARGIN);
                    LbvComponent {
                        id,
                        share,
                        rate_per_year: rate,
                        candidates_per_system: rate * weight.value(),
                    }
                })
            })
            .collect();
        Self {
            components,
            nodes,
            weight,
        }
    }

    /// The young field's components, in component order: those with layer-E systems of the
    /// sampler's ages.
    #[must_use]
    pub fn components(&self) -> &[LbvComponent] {
        &self.components
    }

    /// The sampler's nodes in the galaxy, in ascending mass.
    #[must_use]
    pub fn nodes(&self) -> &[LbvMassNode] {
        &self.nodes
    }

    /// The envelope's terms at `point` into `out`, in the components' order, candidates per cubic
    /// light-year: each component's layer-E share × its density × its candidates per system.
    fn weights(&self, galaxy: &Galaxy, point: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> usize {
        let fields = galaxy.fields();
        for (slot, young) in out.iter_mut().zip(&self.components) {
            *slot = young.share
                * fields.component(young.id).density(point)
                * young.candidates_per_system;
        }
        self.components.len()
    }
}

impl ClassProcess for LbvProcess {
    type Candidate = LbvCandidate;
    type Entry = LbvEntry;

    fn class(&self) -> ClassId {
        ClassId::LUMINOUS_BLUE_VARIABLE
    }

    /// Σ, over the young field's components in order, its layer-E share × its density bound over
    /// the cell's closed box ([`Component::bound`](crate::galaxy::fields::Component::bound)) × its
    /// candidates per system: each term bounds the matching weight of the site's thinning, in the
    /// same arithmetic, so their sum bounds the weights' running total.
    ///
    /// # Panics
    ///
    /// Never: a class cell is a cell box of the root cube that straddles no plane.
    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let cube = CellBox::new(cell.origin_ly(), cell.edge_ly())
            .expect("a class cell is a cell box of the root cube");
        let fields = galaxy.fields();
        PerCubicLightYear::new(self.components.iter().fold(0.0, |sum, young| {
            sum + young.share
                * fields.component_bound(young.id, &cube)
                * young.candidates_per_system
        }))
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<LbvCandidate> {
        let seed = galaxy.seed();
        let key = ObjectKey::from(SystemId::from(id));
        // 1. The site, thinned on the envelope there, the mark picking the component.
        if bound.value().is_nan() || bound.value() <= 0.0 {
            return None;
        }
        let (site, mut stream) = site_stream(galaxy, cell, id, tags::CLASS_LBV_SITE);
        let point = PointLy::from(&site);
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, &point, &mut weights);
        let young = self.components[stream.mark().pick_weighted(&weights[..n], bound.value())?];
        let component = young.id;
        let laws = galaxy.fields().component(component);
        // 2. The node, by its weight, and the primary's initial mass among its masses.
        let mut stream = Stream::open(seed, tags::CLASS_LBV_MASS, key);
        let node_weights: [f64; NODES] = core::array::from_fn(|k| self.nodes[k].weight.value());
        let node = self.nodes[stream
            .mark()
            .pick_weighted(&node_weights, self.weight.value())?];
        let (lo, hi) = node.masses;
        let mass = SolarMasses::new(galaxy.mass_function().quantile_in(
            lo.value(),
            hi.value(),
            stream.uniform(),
        ));
        // 3. The age at the epoch over the node's span, thinned on the component's age density
        //    there against its largest.
        let mut stream = Stream::open(seed, tags::CLASS_LBV_AGE, key);
        let (first, last) = (node.ages.0.value(), node.ages.1.value());
        let age_at_epoch = Years::new(first + stream.uniform() * (last - first));
        if !stream.decide(Threshold::from_ratio(
            galaxy.field_ages(component).pdf(age_at_epoch),
            young.rate_per_year,
        )) {
            return None;
        }
        // 4. The [Fe/H] of the component's law at the site and age, as a grid record reads its own.
        let feh = laws.metallicity(&point, age_at_epoch);
        let mut stream = Stream::open(seed, tags::CLASS_LBV_METALLICITY, key);
        let composition = Composition::from_fe_h(
            Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
            HeliumExcess::ZERO,
        );
        // 5. The one velocity.
        let velocity = class_site_velocity(galaxy, component, &site, tags::CLASS_LBV_VELOCITY, key);
        Some(LbvCandidate {
            id,
            site,
            component,
            population: laws.population(),
            mass,
            age_at_epoch,
            composition,
            velocity,
        })
    }

    fn test(&self, galaxy: &Galaxy, candidate: LbvCandidate) -> Option<LbvEntry> {
        match Verdict::of(galaxy, &candidate) {
            Verdict::Host(entry) => Some(*entry),
            Verdict::Prefiltered | Verdict::NoWindow | Verdict::CoreCollapse => None,
        }
    }

    fn members(&self, _: &LbvEntry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<LbvEntry>) -> CatalogueSystem {
        let entry = member.entry().value();
        CatalogueSystem::new(entry.record, entry.motion)
            .with_stars(entry.composition, MultiplicityContext::Free)
    }

    /// The site, the host's epoch position (module documentation, "Sites at the epoch").
    fn epoch_place(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
    ) -> Option<GalacticPosition> {
        Some(site_stream(galaxy, cell, id, tags::CLASS_LBV_SITE).0)
    }

    fn reads_place_first(&self) -> bool {
        true
    }
}

/// What the class's test makes of a candidate, step by step.
#[derive(Debug, Clone, PartialEq)]
enum Verdict {
    /// The record-only prefilter refuses it.
    Prefiltered,
    /// Its primary's window does not overlap the source horizon, or it has none.
    NoWindow,
    /// Its window overlaps, but the core-collapse class claims it.
    CoreCollapse,
    /// A host: the entry the class keeps, boxed, since the other verdicts are empty and hosts
    /// are rare.
    Host(Box<LbvEntry>),
}

impl Verdict {
    /// The test's verdict on `candidate` (module documentation, "The test").
    #[must_use]
    fn of(galaxy: &Galaxy, candidate: &LbvCandidate) -> Self {
        Self::of_record(galaxy, candidate, &candidate.record())
    }

    /// [`Verdict::of`] with the primary's stars and draws read on `drawn`'s: the candidate's own
    /// record in production; a test passes a grid record whose marks the candidate carries
    /// (P09.T38). The entry's record is the candidate's either way.
    #[must_use]
    fn of_record(galaxy: &Galaxy, candidate: &LbvCandidate, drawn: &SystemRecord) -> Self {
        // The prefilter binds this side as it binds the grid's: a candidate whose age rounded
        // past its node's span is one the grid's claim would not ask either.
        if !may_host(candidate.mass, candidate.age_at_epoch) {
            return Self::Prefiltered;
        }
        // The primary alone, as the grid's claim builds it: the window is its, and the death
        // marks read no companion.
        let stars = SystemStars::generate_with(
            galaxy,
            drawn,
            &candidate.composition,
            MultiplicityContext::ForcedSingle,
        );
        let Some(window) = horizon_window(&stars) else {
            return Self::NoWindow;
        };
        if core_collapse_claims(
            galaxy,
            drawn,
            &candidate.composition,
            &stars,
            candidate.velocity,
        ) {
            return Self::CoreCollapse;
        }
        Self::Host(Box::new(LbvEntry {
            record: candidate.record(),
            component: candidate.component,
            composition: candidate.composition,
            motion: CatalogueMotion::straight(candidate.site, candidate.velocity),
            window,
        }))
    }
}

impl LbvCandidate {
    /// The candidate's record, at its site at the epoch.
    #[must_use]
    fn record(&self) -> SystemRecord {
        SystemRecord::from_parts(
            SystemId::from(self.id),
            self.site,
            SystemOrigin::CatalogueSystem(ClassId::LUMINOUS_BLUE_VARIABLE),
            self.population,
            self.mass,
            self.age_at_epoch,
        )
    }
}

/// Whether the core-collapse class claims the system of `record`, at its epoch position, whose
/// stars are `stars` at `composition` and whose velocity is `velocity`: the one shared test on the
/// grid's terms (P09.T33.a, P09.T35).
///
/// Plan 08's interval predicate on the primary's marks ([`death_in_interval_under`]), under the
/// galaxy's stellar rules as `stars` are, gives T; the explosion site is the epoch position moved
/// along `velocity` to T by straight line, as plan 08's `explosion_site` moves a grid record's;
/// and the class claims the death if [`DeathMarks::of`] gives it a shell and [`claims`] keeps it
/// in the field.
///
/// # Panics
///
/// Never in the root cube: a system inside it moving under 3,000 km/s for at most 4.27 Myr travels
/// under 43,000 ly, which leaves the addressable range nowhere.
#[must_use]
fn core_collapse_claims(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
    stars: &SystemStars,
    velocity: GalacticVelocity,
) -> bool {
    let Some(death) = death_in_interval_under(
        record.primary_initial_mass(),
        composition,
        primary_eta(galaxy, record),
        record.age_at_epoch(),
        || primary_draws(galaxy, record),
        galaxy.rules().stellar,
    ) else {
        return false;
    };
    let elapsed = Seconds::new(death.since_epoch().as_seconds_f64());
    let position = record
        .epoch_position()
        .translated(velocity.displacement_over(elapsed))
        .expect(
            "a system inside the root cube moving under 3,000 km/s for at most 4.27 Myr travels \
             under 43,000 ly, which leaves the addressable range nowhere",
        );
    let site = ExplosionSite::new(position, death);
    DeathMarks::of(galaxy, stars, &site, velocity)
        .is_some_and(|marks| claims(galaxy, &site, &marks, Surroundings::Field))
}

/// The class's test on the marks of the grid record `record` (plan 09, P09.T38's
/// complementarity): a candidate of the record's component at its epoch position carrying its
/// primary's initial mass, its age at the epoch, its metallicity draw and its epoch velocity,
/// through the class's test, its stars drawn on the record's own. The entry the class keeps of
/// those marks, or `None`.
///
/// # Panics
///
/// As [`lbv_claims`], for a record that is not a grid record of `galaxy`, and as plan 08's
/// `epoch_velocity`.
#[cfg(test)]
#[must_use]
pub(crate) fn test_on_grid_marks(galaxy: &Galaxy, record: &SystemRecord) -> Option<LbvEntry> {
    let site = *record.epoch_position();
    let id = crate::galaxy::catalogue_classes::CatalogueCellKey::containing(
        ClassId::LUMINOUS_BLUE_VARIABLE,
        &site,
    )
    .expect("a site in the root cube")
    .candidate_id(0, 0)
    .expect("the first candidate of a cell");
    let component = record.component().expect("a grid record has its component");
    let candidate = LbvCandidate {
        id,
        site,
        component,
        population: record.population(),
        mass: record.primary_initial_mass(),
        age_at_epoch: record.age_at_epoch(),
        composition: draw_metallicity(galaxy, record),
        velocity: crate::galaxy::query::epoch_velocity(galaxy, record),
    };
    match Verdict::of_record(galaxy, &candidate, record) {
        Verdict::Host(entry) => Some(*entry),
        Verdict::Prefiltered | Verdict::NoWindow | Verdict::CoreCollapse => None,
    }
}

#[cfg(test)]
mod tests;
