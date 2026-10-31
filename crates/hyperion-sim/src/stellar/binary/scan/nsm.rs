//! The neutron-star mergers' scan marks (P11.T8.d; design note 10): what a galaxy-wide scan needs
//! of a merger of two compact stars, drawn first and on draws of its own, without the binary
//! engine.
//!
//! A neutron-star merger is a one-shot entry: one merger at its time T, uniform over the source
//! horizon ([`horizon_time`](super::horizon_time)), which P11.T9's kilonova and P12.T5.b's walk
//! read with the ejecta's mass ([`NsmScanMarks::ejecta_mass`]). The marks are drawn in this order,
//! each group on its own stream:
//!
//! 1. the site and its component, thinned on the class's intensity (`class.nsm.site`);
//! 2. T, and the system's age at the epoch from its component's own age law among its born
//!    systems (`class.nsm.time`);
//! 3. its \[Fe/H\] from its component's law at the site and that age (`class.nsm.metallicity`);
//! 4. its pair (`class.nsm.mass`): two neutron stars, or a black hole and a neutron star, by
//!    [`NSM_SAMPLER`]'s weights, each compact star's mass at the merger, and its progenitor's
//!    initial mass from the galaxy's mass function (the primary is the heavier progenitor);
//! 5. its velocity at the site, plan 08's draw of its component's law (`class.nsm.velocity`).
//!
//! What it leaves (the remnant) follows from the total mass by plan 11's rule for two compact
//! stars (P11.T4.d, the engine's `mix`): a neutron star's mass in a black hole above plan 06's
//! heaviest neutron star, [`MAX_NEUTRON_STAR_MASS`] (Mandel and Müller 2020's 2.0 M☉), so every
//! merger of two neutron stars leaves a black hole. Its ejecta are GW170817's
//! ([`GW170817_EJECTA_MASS`]): the class has no ejecta model of its own.
//!
//! A candidate is kept exactly when both progenitors have died by T: their lifetimes (plan 06's
//! [`lifetime`]) are both under the system's age at T, which refuses only a system of a young
//! population formed less than some 30 Myr before T. The class's own test cannot refuse a
//! candidate the scan keeps: a merger in the horizon of two compact stars of which one is a
//! neutron star is a neutron-star merger.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::CatalogueCellKey;
use crate::galaxy::catalogue_classes::binary::{ClassHosts, NeutronStarMergers};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::{PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId, SystemId};
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::binary::light_curves::GW170817_EJECTA_MASS;
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::collapse::MAX_NEUTRON_STAR_MASS;
use crate::stellar::system::track_mass_of;
use crate::stellar::{Composition, Phase, lifetime};
use crate::tables::binary::{
    NSM_BLACK_HOLE_PROGENITORS, NSM_NEUTRON_STAR_PROGENITORS, NSM_SAMPLER, NsmMark, NsmPairKind,
    NsmPick,
};
use crate::time::UniverseTime;
use crate::units::{Dex, HeliumExcess, PerCubicLightYear, SolarMasses, Years};

use super::super::sampler::ClassSampler;
use super::horizon_time;

/// What a galaxy-wide scan reads of a neutron-star merger (P11.T8.d): its place and motion, its
/// merger time T, its pair, its ejecta and its remnant.
///
/// # Examples
///
/// The neutron-star mergers of a galaxy are rare, about ten in the source horizon at Milky Way
/// parameters: the whole galaxy's, with their kilonovae's ejecta, as an alert scan reads them.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::NeutronStarMergers;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
/// use hyperion_sim::stellar::Phase;
/// use hyperion_sim::stellar::binary::scan::scan_nsm_cell;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = NeutronStarMergers::new(&galaxy);
/// let mut mergers = Vec::new();
/// for x in -4..4 {
///     for y in -4..4 {
///         for z in -1..1 {
///             let key = CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, [x, y, z])?;
///             scan_nsm_cell(&galaxy, &process, key, &mut mergers);
///         }
///     }
/// }
/// assert!(mergers.len() < 100);
/// for merger in &mergers {
///     assert!(merger.ejecta_mass().value() > 0.0);
///     assert_eq!(merger.remnant_phase(), Phase::BlackHole);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NsmScanMarks {
    id: CatalogueSystemId,
    component: ComponentId,
    population: Population,
    position: GalacticPosition,
    velocity: GalacticVelocity,
    merger_time: UniverseTime,
    age_at_epoch: Years,
    composition: Composition,
    kind: NsmPairKind,
    masses: [SolarMasses; 2],
    progenitor_masses: [SolarMasses; 2],
    collapse_ages: [Years; 2],
}

impl NsmScanMarks {
    /// The merger's ID: member 0 of its catalogue entry.
    #[must_use]
    pub const fn id(&self) -> CatalogueSystemId {
        self.id
    }

    /// The density component the merger was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// The component's population.
    #[must_use]
    pub const fn population(&self) -> Population {
        self.population
    }

    /// Where the system is at the epoch: its site in its cell.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The system's velocity, galactic, the one its entry moves at (plan 09's rule 6).
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The system's position at `t`, on its straight line through its site at the epoch: at
    /// [`merger_time`](Self::merger_time), where its kilonova goes off.
    ///
    /// # Panics
    ///
    /// As plan 09's `CatalogueMotion::position_at`, beyond any time the clock reaches.
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        crate::galaxy::catalogue_classes::CatalogueMotion::straight(self.position, self.velocity)
            .position_at(t)
    }

    /// The merger time T, inside the source horizon, `[−(H + L), +H)`: the onset of its kilonova.
    #[must_use]
    pub const fn merger_time(&self) -> UniverseTime {
        self.merger_time
    }

    /// The system's age at the epoch, Julian years.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// The system's age at T, Julian years: the pair's `merger_age`, as plan 03's record counts
    /// its age at a time.
    #[must_use]
    pub fn merger_age(&self) -> Years {
        Years::new(self.age_at_epoch.value() + self.merger_time.since_epoch().as_julian_years_f64())
    }

    /// The composition the system formed with.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The pair's kind.
    #[must_use]
    pub const fn kind(&self) -> NsmPairKind {
        self.kind
    }

    /// Each compact star's mass at T, M☉, primary first: a neutron star of 1.13–2.0 M☉, or for a
    /// neutron star and a black hole, the black hole first.
    #[must_use]
    pub const fn masses(&self) -> [SolarMasses; 2] {
        self.masses
    }

    /// Each compact star's phase, primary first.
    #[must_use]
    pub const fn phases(&self) -> [Phase; 2] {
        match self.kind {
            NsmPairKind::DoubleNeutronStar => [Phase::NeutronStar, Phase::NeutronStar],
            NsmPairKind::NeutronStarBlackHole => [Phase::BlackHole, Phase::NeutronStar],
        }
    }

    /// The pair's total mass at T, M☉.
    #[must_use]
    pub fn total_mass(&self) -> SolarMasses {
        self.masses[0] + self.masses[1]
    }

    /// Each progenitor's initial mass, M☉, primary first, the primary the heavier: the record's
    /// primary is the first.
    #[must_use]
    pub const fn progenitor_masses(&self) -> [SolarMasses; 2] {
        self.progenitor_masses
    }

    /// The age at which each progenitor died, leaving its compact star, Julian years since the
    /// system formed: plan 06's lifetime of its initial mass.
    #[must_use]
    pub const fn collapse_ages(&self) -> [Years; 2] {
        self.collapse_ages
    }

    /// The mass the merger ejects, the kilonova's, M☉: [`GW170817_EJECTA_MASS`], since the class
    /// has no ejecta model of its own.
    #[must_use]
    pub const fn ejecta_mass(&self) -> SolarMasses {
        GW170817_EJECTA_MASS
    }

    /// What the merger leaves, by the total mass (P11.T4.d's rule): a black hole above plan 06's
    /// heaviest neutron star, [`MAX_NEUTRON_STAR_MASS`], or below it a neutron star; a black hole
    /// always where one merges.
    #[must_use]
    pub fn remnant_phase(&self) -> Phase {
        let neutron_stars = self.kind == NsmPairKind::DoubleNeutronStar;
        if neutron_stars && self.total_mass() <= MAX_NEUTRON_STAR_MASS {
            Phase::NeutronStar
        } else {
            Phase::BlackHole
        }
    }

    /// The remnant's mass, M☉: the pair's total, as the engine's merger of two compact stars keeps
    /// it.
    #[must_use]
    pub fn remnant_mass(&self) -> SolarMasses {
        self.total_mass()
    }
}

/// The scan marks of candidate `id` of the neutron-star merger class cell `cell` under its
/// `bound`, or `None` if the class keeps no entry there (P11.T8.d; module documentation). It never
/// runs the binary engine.
///
/// The marks are the entry's own, bit for bit: the class's candidate draws them through this
/// function first.
///
/// # Panics
///
/// If `cell` is not a neutron-star merger cell; in debug builds, on a density above the bound.
#[must_use]
pub fn nsm_scan_marks(
    galaxy: &Galaxy,
    process: &NeutronStarMergers,
    cell: &CatalogueCellKey,
    id: CatalogueSystemId,
    bound: PerCubicLightYear,
) -> Option<NsmScanMarks> {
    let (position, component) =
        process
            .hosts()
            .site(galaxy, cell, id, bound, tags::CLASS_NSM_SITE)?;
    let seed = galaxy.seed();
    let system = SystemId::from(id);
    let key = ObjectKey::from(system);
    let sampler = ClassSampler::new(&NSM_SAMPLER);
    let laws = galaxy.fields().component(component);
    // 2. T and the age.
    let mut stream = Stream::open(seed, tags::CLASS_NSM_TIME, key);
    let merger_time = horizon_time(stream.uniform());
    let ages = laws.ages();
    let unborn = ages.cdf(Years::ZERO);
    let rank = sampler.draw(NsmMark::AgeRank, 0.0, &mut stream);
    let age_at_epoch = ages.quantile(unborn + (1.0 - unborn) * rank);
    let merger_age = age_at_epoch.value() + merger_time.since_epoch().as_julian_years_f64();
    // 3. The composition, at the site and the age.
    let feh = laws.metallicity(&PointLy::from(&position), age_at_epoch);
    let mut stream = Stream::open(seed, tags::CLASS_NSM_METALLICITY, key);
    let composition = Composition::from_fe_h(
        Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
        HeliumExcess::ZERO,
    );
    // 4. The pair: its kind, the compact stars' masses, their progenitors.
    let mut stream = Stream::open(seed, tags::CLASS_NSM_MASS, key);
    let kind = NsmPairKind::ALL[sampler.pick(NsmPick::PairKind, &mut stream)];
    let neutron_star =
        |stream: &mut Stream| SolarMasses::new(sampler.draw(NsmMark::NeutronStarMass, 0.0, stream));
    let first = match kind {
        NsmPairKind::DoubleNeutronStar => neutron_star(&mut stream),
        NsmPairKind::NeutronStarBlackHole => SolarMasses::new(math::exp10(sampler.draw(
            NsmMark::LogBlackHoleMass,
            0.0,
            &mut stream,
        ))),
    };
    let second = neutron_star(&mut stream);
    let progenitor = |range: (f64, f64), mark: NsmMark, stream: &mut Stream| {
        let rank = sampler.draw(mark, 0.0, stream);
        galaxy.mass_function().quantile_in(range.0, range.1, rank)
    };
    let ns_mark = NsmMark::NeutronStarProgenitorRank;
    let (a, b) = match kind {
        NsmPairKind::DoubleNeutronStar => (
            progenitor(NSM_NEUTRON_STAR_PROGENITORS, ns_mark, &mut stream),
            progenitor(NSM_NEUTRON_STAR_PROGENITORS, ns_mark, &mut stream),
        ),
        NsmPairKind::NeutronStarBlackHole => (
            progenitor(
                NSM_BLACK_HOLE_PROGENITORS,
                NsmMark::BlackHoleProgenitorRank,
                &mut stream,
            ),
            progenitor(NSM_NEUTRON_STAR_PROGENITORS, ns_mark, &mut stream),
        ),
    };
    let progenitor_masses = [SolarMasses::new(a.max(b)), SolarMasses::new(a.min(b))];
    // Both progenitors have died by T.
    let collapse_ages = [0_u16, 1].map(|body| {
        let draws = StarDraws::for_star(seed, BodyId::new(system, body));
        lifetime(
            track_mass_of(progenitor_masses[usize::from(body)]),
            &composition,
            &draws,
        )
    });
    if !(collapse_ages[0].value() < merger_age && collapse_ages[1].value() < merger_age) {
        return None;
    }
    // 5. The velocity.
    let velocity =
        ClassHosts::velocity(galaxy, component, &position, tags::CLASS_NSM_VELOCITY, key);
    Some(NsmScanMarks {
        id,
        component,
        population: laws.population(),
        position,
        velocity,
        merger_time,
        age_at_epoch,
        composition,
        kind,
        masses: [first, second],
        progenitor_masses,
        collapse_ages,
    })
}

/// Appends to `out` the scan marks of every entry of the neutron-star merger class cell `key`, in
/// index order: one bound, one count and [`nsm_scan_marks`] of each candidate, the cell's entries
/// exactly.
///
/// # Panics
///
/// As [`nsm_scan_marks`], and as plan 09's count on a bound past the Poisson sampler's limit.
pub fn scan_nsm_cell(
    galaxy: &Galaxy,
    process: &NeutronStarMergers,
    key: CatalogueCellKey,
    out: &mut Vec<NsmScanMarks>,
) {
    let bound = process.hosts().bound(galaxy, &key);
    let candidates =
        crate::galaxy::catalogue_classes::grid::candidate_count(galaxy.seed(), &key, bound);
    out.extend((0..candidates).filter_map(|index| {
        let id = key
            .candidate_id(index, 0)
            .expect("a count held under 2^24 numbers every candidate");
        nsm_scan_marks(galaxy, process, &key, id, bound)
    }));
}
