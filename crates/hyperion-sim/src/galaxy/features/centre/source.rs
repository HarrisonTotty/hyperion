//! The galactic centre's members in the range query: plan 03's merge hook for the centre's
//! twelve-level grid (plan 09, P09.T29; brainstorm, "The range query").
//!
//! [`CentreMemberSource`] answers a query sphere with the centre's members (P09.T27) and its black
//! hole, member zero, at the sphere's time:
//!
//! - **Census.** A band's expected members are the sum of the expected candidates
//!   ([`CentrePlacement::expected_candidates`]) of the owned cells touching the sphere's unpadded
//!   radius, which errs high, as [`SystemSource`]'s contract allows, and does not depend on the
//!   query's time; each band counts in the layer of its band, and the black hole, if the sphere
//!   holds it, one in layer E.
//! - **Hits.** Members move on their orbits (P09.T28.b.a's [`Trajectory`]: the Kepler orbit
//!   inside the influence radius, a straight line outside it), so the cells are chosen by how far
//!   their members can go, and every member of a chosen cell is propagated to the sphere's time
//!   and tested there against the unpadded radius. Members not yet born at that time (the young
//!   disc's, with ages from −H) are dropped.
//!
//! # Which cells
//!
//! The brainstorm's rule: "around the central black hole the pad per level is the distance a
//! radial plunge could cover, and everything inside a radius growing as |t|^⅔ is scanned in full".
//! As built it is applied per cell, with a μ that bounds what the members can do (plan 09's
//! Risks, "P09.T29 as built"):
//!
//! - **The speed bound.** A member's speed at any radius r it reaches is at most `√(2ε + 2μ ÷ r)`.
//!   Its μᵢ is G times the mass inside its epoch radius
//!   ([`CentreMotion::enclosed_mass`]), so a cell's members have μ ≤ G M(< `r_far`), `r_far` the
//!   cell's farthest corner. Their velocities are the distribution function's, drawn in the
//!   galaxy's whole potential, below `√(2(Ψ − E_out))`
//!   ([`DistributionFunction::max_speed`](super::DistributionFunction::max_speed)), so the energy
//!   of a member's Kepler orbit is under `ε = Ψ_out(r_near) − E_out`, the potential of the mass
//!   outside the cell's nearest radius and of the rest of the galaxy (`CentreProfile::outer_psi`,
//!   which falls outward). That is positive: the galaxy's well lets members near the black hole
//!   move faster than the escape speed from the mass inside their orbits, which the plan's
//!   `√(2GM)` with M the black hole's would not cover. A member outside the influence radius moves
//!   in a straight line at a speed under `√(2ε + 2μ ÷ r₀)` at its start, which bounds it at every
//!   radius inside its start.
//! - **The plunge pad.** Inside `r_far` that bound is under the escape speed of
//!   `μ′ = μ + ε r_far`, so a member's path over |t| is at most the radial plunge
//!   [`plunge_pad`](crate::galaxy::motion::plunge_pad) about μ′ from the cell's nearest radius, and
//!   the cell is visited if it lies within the sphere's radius plus that pad.
//! - **The full scan.** A cell whose nearest radius is inside the plunge's full-scan radius about
//!   its own μ′ ([`full_scan_radius`](crate::galaxy::motion::full_scan_radius)) has members that
//!   can pass the black hole within |t|. On its orbit a member's `r^(3⁄2)` grows by at most
//!   `3⁄2 |t| √(2εr + 2μ)`, so it stays within an outward reach of the black hole; every such
//!   cell is scanned in full whenever the sphere comes within that reach of the black hole, and
//!   none of them otherwise.
//!
//! Every pad and reach is an upper bound, so a cell that can hold a member inside the sphere at
//! its time is always visited: the hits equal a brute-force scan of every member, which a test
//! checks at ±100 years. At the epoch every pad is zero and the cells are those touching the
//! sphere.
//!
//! # Caches
//!
//! A cell's members and their trajectories ([`CentreCellOrbits`]) come through a
//! [`CentreOrbitCache`], the caller's: [`NoCentreOrbitCache`] places and prepares them for every
//! call, [`KeepCentreOrbits`] keeps them for one galaxy, and the server's byte-bounded cache is
//! plan 09's P09.T40.a. The census reads no cache, so it is the same whatever is cached. The
//! centre's model is the galaxy's own ([`Galaxy::centre`]).
//!
//! The source is not registered with the range query: plan 09's P09.T37 merges it, and until then
//! nothing generated reads it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::coords::GalacticPosition;
use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::imf::MassBand;
use crate::galaxy::motion::{CentreMotion, EpochState, Regime, SpeedBound, StateAt, Trajectory};
use crate::galaxy::placement::{Existence, SystemRecord};
use crate::galaxy::query::{LayerCounts, LayerSet, QuerySphere, SystemHit, SystemSource};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::time::UniverseTime;
use crate::units::consts::METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND;
use crate::units::{GravitationalParameter, LightYears, Seconds};

use super::super::nested::{NestedCell, NestedLevel, box_distance};
use super::members::CentrePlacement;
use super::{BuildCentreError, CentreProfile, CentreTracer};

/// The bands from the coarsest down, the order the query walks its layers.
const BANDS_COARSEST_FIRST: [MassBand; 5] = [
    MassBand::E,
    MassBand::D,
    MassBand::C,
    MassBand::B,
    MassBand::A,
];

/// J kg⁻¹ in a (km/s)², the working units' specific energy in SI.
const J_PER_KG_PER_KM2_S2: f64 =
    METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND * METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND;

/// The relative margin the speed bound's inputs are widened by, 10⁻⁶: it keeps a level's bound
/// above each of its cells' although the mass and potential tables are monotone only to their
/// interpolation's 10⁻⁷ (P09.T24.a), and moves no pad measurably.
const SPEED_MARGIN: f64 = 1e-6;

/// The relative margin of a member's shell test, 10⁻⁶: a propagated distance from the black hole
/// keeps the orbit's energy and angular momentum to some 10⁻¹² of their scale (P09.T28.a as
/// built), so it lies inside the shell its elements give to far better than this.
const SHELL_MARGIN: f64 = 1e-6;

/// One cell's members of one band with their trajectories (module documentation): what a
/// [`CentreOrbitCache`] keeps, about 200 bytes a member.
///
/// Each member also carries the shell it never leaves: its orbit's pericentre and apocentre
/// ([`KeplerOrbit::pericentre`](crate::galaxy::motion::KeplerOrbit::pericentre),
/// [`apocentre`](crate::galaxy::motion::KeplerOrbit::apocentre)), its epoch distance twice if it
/// has no orbit, and nothing for a straight line. A query skips a member whose shell misses its
/// sphere without propagating it, which no member inside the sphere can do, so the hits are the
/// same.
#[derive(Debug, Clone, PartialEq)]
pub struct CentreCellOrbits {
    trajectories: Vec<Trajectory>,
    /// Per trajectory, the least and greatest distance from the black hole it ever has, ly.
    shells: Vec<[f64; 2]>,
}

impl CentreCellOrbits {
    /// The members of `band` in `cell` of `centre`, the centre of `galaxy`, each with its
    /// trajectory, in index order.
    #[must_use]
    pub fn new(
        galaxy: &Galaxy,
        centre: &CentrePlacement<'_>,
        band: MassBand,
        cell: NestedCell,
    ) -> Self {
        let motion = galaxy.centre_motion();
        let trajectories: Vec<Trajectory> = centre
            .members_in_cell(galaxy, band, cell)
            .iter()
            .map(|member| Trajectory::of(motion, EpochState::of_centre_member(member)))
            .collect();
        let shells = trajectories.iter().map(shell_of).collect();
        Self {
            trajectories,
            shells,
        }
    }

    /// The members' trajectories, in index order: each one's start holds its record and its
    /// velocity at the epoch.
    #[must_use]
    pub fn trajectories(&self) -> &[Trajectory] {
        &self.trajectories
    }

    /// The bytes it owns on the heap, for a byte-bounded cache (P09.T40.a).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        size_of_val(&*self.trajectories) + size_of_val(&*self.shells)
    }

    /// Appends the hits its members make in `sphere`, `from_black_hole` ly from the black hole.
    fn hits_in(&self, sphere: &QuerySphere, from_black_hole: f64, out: &mut Vec<SystemHit>) {
        let radius = sphere.radius().value();
        let (near, far) = (from_black_hole - radius, from_black_hole + radius);
        let t = sphere.time();
        for (trajectory, &[least, most]) in self.trajectories.iter().zip(&self.shells) {
            if near > most * (1.0 + SHELL_MARGIN) || far < least * (1.0 - SHELL_MARGIN) {
                continue;
            }
            if let Some(hit) = hit_at(trajectory, sphere, t) {
                out.push(hit);
            }
        }
    }
}

/// The least and greatest distance from the black hole that `trajectory` ever has, ly.
#[must_use]
fn shell_of(trajectory: &Trajectory) -> [f64; 2] {
    match (trajectory.regime(), trajectory.orbit()) {
        (Regime::Kepler, Some(orbit)) => [
            orbit.pericentre().value(),
            orbit.apocentre().map_or(f64::INFINITY, LightYears::value),
        ],
        (Regime::Kepler, None) => {
            let r = local_of(trajectory.start().record().epoch_position());
            let r = (r.x * r.x + r.y * r.y + r.z * r.z).sqrt();
            [r, r]
        }
        (Regime::Straight, _) => [0.0, f64::INFINITY],
    }
}

/// Where a caller keeps the centre's cells of members and their orbits between queries (P09.T29:
/// "the caller may supply a cache of orbital elements per cell").
///
/// It takes `&self`, so an implementation that keeps cells uses interior mutability, and it must
/// lend each exactly as [`CentreCellOrbits::new`] builds it. An implementation that keeps them
/// between calls keys them by galaxy as well as by band and cell. The `centre` it is given is the
/// galaxy's own ([`Galaxy::centre`]'s model, which [`CentreMemberSource`] places), so it is not
/// part of the key; an implementation may check that it is.
pub trait CentreOrbitCache {
    /// The members of `band` in `cell` of `centre`, the centre of `galaxy`, with their
    /// trajectories.
    fn orbits(
        &self,
        galaxy: &Galaxy,
        centre: &CentrePlacement<'_>,
        band: MassBand,
        cell: NestedCell,
    ) -> Arc<CentreCellOrbits>;
}

/// The cache that keeps nothing: every lookup places its cell's members.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoCentreOrbitCache;

impl CentreOrbitCache for NoCentreOrbitCache {
    fn orbits(
        &self,
        galaxy: &Galaxy,
        centre: &CentrePlacement<'_>,
        band: MassBand,
        cell: NestedCell,
    ) -> Arc<CentreCellOrbits> {
        Arc::new(CentreCellOrbits::new(galaxy, centre, band, cell))
    }
}

/// A cache that keeps every cell it builds for one galaxy, the one it is made for.
///
/// It is what a caller asking several queries of one centre passes, as the brainstorm's century
/// query "with cached orbits" does. It grows without bound, about 200 bytes a member, so a
/// long-lived caller uses plan 09's server cache (P09.T40.a) instead.
///
/// # Panics
///
/// If it is asked about another galaxy than its own (a bare galaxy and its
/// [`with_full_potential`](Galaxy::with_full_potential) are two values), or about a placement of
/// another model than that galaxy's [`Galaxy::centre`].
#[derive(Debug)]
pub struct KeepCentreOrbits<'a> {
    galaxy: &'a Galaxy,
    /// Keyed by band and cell in one integer ([`cell_key`]): over the 480,000 band-cells of a
    /// century's full scan, lookups alone took 46 ms against the `(MassBand, NestedCell)` tuple's
    /// 61 ms (opt-level 2, a shared machine, P09.T29 as built).
    kept: Mutex<BTreeMap<u64, Arc<CentreCellOrbits>>>,
}

impl<'a> KeepCentreOrbits<'a> {
    /// An empty cache for `galaxy`.
    #[must_use]
    pub fn new(galaxy: &'a Galaxy) -> Self {
        Self {
            galaxy,
            kept: Mutex::new(BTreeMap::new()),
        }
    }

    /// The members it keeps, over every cell and band.
    #[must_use]
    pub fn members(&self) -> usize {
        self.kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .map(|cell| cell.trajectories.len())
            .sum()
    }
}

impl CentreOrbitCache for KeepCentreOrbits<'_> {
    fn orbits(
        &self,
        galaxy: &Galaxy,
        centre: &CentrePlacement<'_>,
        band: MassBand,
        cell: NestedCell,
    ) -> Arc<CentreCellOrbits> {
        assert!(
            std::ptr::eq(galaxy, self.galaxy),
            "a centre orbit cache is for the galaxy it was made for"
        );
        assert!(
            galaxy
                .centre()
                .is_ok_and(|model| std::ptr::eq(centre.model(), model)),
            "a centre orbit cache places the galaxy's own centre"
        );
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(
            kept.entry(cell_key(band, cell))
                .or_insert_with(|| Arc::new(CentreCellOrbits::new(galaxy, centre, band, cell))),
        )
    }
}

/// The galactic centre's members as a source of the range query (module documentation).
///
/// # Panics
///
/// Its [`SystemSource`] methods panic if asked about another galaxy than the one it was made for,
/// whose centre it holds.
///
/// # Examples
///
/// The systems within 0.05 ly of Sgr A* a century from now:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::{CentreMemberSource, KeepCentreOrbits};
/// use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, SystemSource};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let orbits = KeepCentreOrbits::new(&galaxy);
/// let source = CentreMemberSource::new(&galaxy, &orbits)?;
/// let t = UniverseTime::from_julian_years(100).expect("in the clock window");
/// let sphere = QuerySphere::new(GalacticPosition::ORIGIN, LightYears::new(0.05), t, LightYears::ZERO)
///     .expect("a positive radius");
/// let stellar = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];
/// let layers = stellar.into_iter().fold(LayerSet::EMPTY, LayerSet::with);
/// let mut hits = Vec::new();
/// source.systems_in_sphere(&galaxy, &sphere, layers, &mut hits);
/// // The black hole, at rest at the origin, and some members on their orbits.
/// assert!(hits.iter().any(|hit| hit.distance().value() < 1e-12));
/// assert!(hits.iter().all(|hit| hit.distance().value() <= 0.05));
/// # Ok::<(), hyperion_sim::galaxy::features::centre::BuildCentreError>(())
/// ```
#[derive(Clone)]
pub struct CentreMemberSource<'a> {
    galaxy: &'a Galaxy,
    placement: CentrePlacement<'a>,
    orbits: &'a dyn CentreOrbitCache,
    /// `E_out`, the least binding energy any member has, (km/s)²: the four distribution
    /// functions' outer cut.
    floor_km2_s2: f64,
}

impl std::fmt::Debug for CentreMemberSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CentreMemberSource").finish_non_exhaustive()
    }
}

impl<'a> CentreMemberSource<'a> {
    /// The source of `galaxy`'s centre, whose model the galaxy holds ([`Galaxy::centre`]), reading
    /// cells through `orbits`.
    ///
    /// # Errors
    ///
    /// The [`BuildCentreError`] of a centre that cannot be built, which no galaxy the builder
    /// allows has ([`Galaxy::centre`]).
    pub fn new(
        galaxy: &'a Galaxy,
        orbits: &'a dyn CentreOrbitCache,
    ) -> Result<Self, BuildCentreError> {
        let model = galaxy.centre()?;
        let floor_km2_s2 = CentreTracer::ALL
            .iter()
            .map(|&t| model.distribution(t).floor_energy())
            .fold(f64::INFINITY, f64::min);
        Ok(Self {
            galaxy,
            placement: CentrePlacement::new(model),
            orbits,
            floor_km2_s2,
        })
    }

    /// The centre's placement, whose members it returns.
    #[must_use]
    pub fn placement(&self) -> &CentrePlacement<'a> {
        &self.placement
    }

    /// Panics unless `galaxy` is the source's own.
    fn check(&self, galaxy: &Galaxy) {
        assert!(
            std::ptr::eq(galaxy, self.galaxy),
            "a centre member source is for the galaxy it was made for"
        );
    }

    /// The speed bound of members whose epoch radii lie between `near` and `far` (module
    /// documentation), its inputs widened by [`SPEED_MARGIN`] times `widen`.
    #[must_use]
    fn speeds(
        &self,
        motion: &CentreMotion,
        near: LightYears,
        far: LightYears,
        widen: f64,
    ) -> SpeedBound {
        let profile = self.placement.model().profile();
        let mass = motion.enclosed_mass(far) * (1.0 + widen * SPEED_MARGIN);
        let excess =
            excess_km2_s2(profile, self.floor_km2_s2, near.value()) * (1.0 + widen * SPEED_MARGIN);
        SpeedBound::new(
            GravitationalParameter::from_solar_masses(mass),
            excess * J_PER_KG_PER_KM2_S2,
        )
    }

    /// The span from the epoch to `t`, as the reaches read it.
    #[must_use]
    fn span(&self, t: UniverseTime) -> Span {
        let motion = self.galaxy.centre_motion();
        let dt = Seconds::new(t.since_epoch().as_seconds_f64());
        Span {
            dt,
            years: t.since_epoch().as_julian_years_f64().abs(),
            // μ′ is at least the black hole's, so every cell's full-scan edge is at least this.
            least: SpeedBound::new(
                GravitationalParameter::from_solar_masses(motion.profile().black_hole()),
                0.0,
            )
            .full_scan_edge(LightYears::ZERO, dt),
        }
    }

    /// How far the members of `cell` can go over `span` (module documentation).
    #[must_use]
    fn cell_reach(&self, motion: &CentreMotion, cell: NestedCell, span: &Span) -> CellReach {
        let local = self.placement.grid().local_cell(cell);
        let near = LightYears::new(local.nearest_radius());
        let far = LightYears::new(local.farthest_radius());
        if near.value() < span.least.value() {
            return CellReach::Full;
        }
        let bound = self.speeds(motion, near, far, 1.0);
        if near.value() < bound.full_scan_edge(far, span.dt).value() {
            return CellReach::Full;
        }
        CellReach::Plunge(bound.reach(near, far, span.dt).unwrap_or(near))
    }

    /// What every cell of `level` can do over `span`: a bound on the plunge pads of its cells
    /// outside the full scan, the full scan's edge, inside which every full-scan cell of the level
    /// has its nearest radius, and how far from the black hole their members can be (module
    /// documentation).
    #[must_use]
    fn level_reach(&self, motion: &CentreMotion, level: NestedLevel, span: &Span) -> LevelReach {
        let half = level.half_width().value();
        let inner = LightYears::new(if level.level() == 0 { 0.0 } else { 0.5 * half });
        let corner = LightYears::new(half * 3.0_f64.sqrt());
        // The level's bound holds every cell's: the mass inside its far corner and the potential
        // outside its nearest radius, each widened twice a cell's margin.
        let bound = self.speeds(motion, inner, corner, 2.0);
        let edge = bound.full_scan_edge(corner, span.dt) * (1.0 + SPEED_MARGIN);
        // A cell outside its full scan moves at most the plunge from the level's nearest radius,
        // or, if that plunge reaches the black hole, at most its own start, inside the edge.
        let pad = bound.reach(inner, corner, span.dt).unwrap_or(edge);
        if inner.value() >= edge.value() {
            return LevelReach {
                pad,
                edge,
                outward: LightYears::ZERO,
            };
        }
        // The full scan's cells start within `start` of the black hole.
        let start = LightYears::new(
            corner
                .value()
                .min(edge.value() + level.edge().value() * 3.0_f64.sqrt()),
        );
        let mut outward = bound.outward(start, span.dt).value();
        if start.value() >= motion.influence_radius().value() {
            // Members beyond the influence radius move in straight lines from their start.
            let straight = if inner.value() > 0.0 {
                let profile = self.placement.model().profile();
                start.value()
                    + max_speed_km_s(profile, self.floor_km2_s2, inner.value())
                        * LIGHT_YEARS_PER_YEAR_PER_KM_S
                        * span.years
            } else {
                f64::INFINITY
            };
            outward = outward.max(straight);
        }
        LevelReach {
            pad,
            edge,
            outward: LightYears::new(outward),
        }
    }

    /// Calls `visit` with every owned cell that can hold a member inside `sphere` at its time
    /// (module documentation), each once, level by level from the innermost.
    fn cells_reaching(&self, sphere: &QuerySphere, mut visit: impl FnMut(NestedCell)) {
        let motion = self.galaxy.centre_motion();
        let grid = self.placement.grid();
        let span = self.span(sphere.time());
        let radius = sphere.radius().value();
        let centre = local_of(sphere.centre());
        let from_black_hole =
            (centre.x * centre.x + centre.y * centre.y + centre.z * centre.z).sqrt();
        let at = [centre.x, centre.y, centre.z];
        for level in grid.all_levels() {
            let j = level.level();
            let reach = self.level_reach(motion, level, &span);
            // Whether the sphere comes within reach of the full scan's members.
            let outward = reach.outward.value();
            let full_scan = outward > 0.0 && from_black_hole <= radius + outward;
            let near = LightYears::new(radius + reach.pad.value());
            for cell in grid.level_cells_touching(j, &centre, near) {
                let away = box_distance(&grid.local_cell(cell), at);
                let reached = match self.cell_reach(motion, cell, &span) {
                    CellReach::Full => full_scan,
                    CellReach::Plunge(pad) => away <= radius + pad.value(),
                };
                if reached {
                    visit(cell);
                }
            }
            if !full_scan {
                continue;
            }
            // The full scan's cells the walk above did not reach.
            let origin = PointLy::default();
            for cell in grid.level_cells_touching(j, &origin, reach.edge) {
                if grid.level_touches(cell, &centre, near) {
                    continue;
                }
                match self.cell_reach(motion, cell, &span) {
                    CellReach::Full => visit(cell),
                    CellReach::Plunge(_) => {}
                }
            }
        }
    }
}

/// The span from the epoch to a query's time, as the reaches read it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    dt: Seconds,
    /// |t|, Julian years.
    years: f64,
    /// A lower bound of every cell's full-scan edge: the black hole's own.
    least: LightYears,
}

/// How far a cell's members can go over a span (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
enum CellReach {
    /// Its nearest radius is inside its full scan: its members can pass the black hole.
    Full,
    /// Its members move at most this.
    Plunge(LightYears),
}

/// What every cell of a level can do over a span ([`CentreMemberSource::level_reach`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct LevelReach {
    /// At least every [`CellReach::Plunge`] of the level.
    pad: LightYears,
    /// Every [`CellReach::Full`] cell of the level has its nearest radius inside it.
    edge: LightYears,
    /// How far from the black hole a member of a full-scan cell can be; zero if the level has
    /// none.
    outward: LightYears,
}

impl SystemSource for CentreMemberSource<'_> {
    fn expected_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere) -> LayerCounts {
        self.check(galaxy);
        let mut counts = LayerCounts::ZERO;
        let centre = local_of(sphere.centre());
        for cell in self
            .placement
            .grid()
            .cells_touching(&centre, sphere.radius())
        {
            for band in BANDS_COARSEST_FIRST {
                let layer = band.layer();
                counts.set(
                    layer,
                    counts.get(layer) + self.placement.expected_candidates(band, cell),
                );
            }
        }
        if holds_black_hole(sphere) {
            counts.set(Layer::E, counts.get(Layer::E) + 1.0);
        }
        counts
    }

    fn systems_in_sphere(
        &self,
        galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        out: &mut Vec<SystemHit>,
    ) {
        self.check(galaxy);
        if layers.contains(Layer::E) && holds_black_hole(sphere) {
            let black_hole = self.placement.black_hole();
            out.push(SystemHit::new(
                *black_hole.record(),
                GalacticPosition::ORIGIN,
                LightYears::from(sphere.centre().distance_to(&GalacticPosition::ORIGIN)),
            ));
        }
        let from_black_hole =
            LightYears::from(sphere.centre().distance_to(&GalacticPosition::ORIGIN)).value();
        self.cells_reaching(sphere, |cell| {
            for band in BANDS_COARSEST_FIRST {
                if !layers.contains(band.layer()) {
                    continue;
                }
                self.orbits
                    .orbits(galaxy, &self.placement, band, cell)
                    .hits_in(sphere, from_black_hole, out);
            }
        });
    }

    fn suppresses(&self, galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
        self.check(galaxy);
        false
    }
}

/// `band` and `cell` as one integer, distinct for every pair: the band's index, the level and the
/// three cell coordinates, a byte each.
#[must_use]
fn cell_key(band: MassBand, cell: NestedCell) -> u64 {
    let [x, y, z] = cell.cell();
    let band = u8::try_from(band.index()).expect("five bands");
    u64::from_be_bytes([0, 0, 0, band, cell.level(), x, y, z])
}

/// The hit a member on `trajectory` makes in `sphere` at `t`, its time, or `None` if it is outside
/// the unpadded radius then or not yet born.
#[must_use]
fn hit_at(trajectory: &Trajectory, sphere: &QuerySphere, t: UniverseTime) -> Option<SystemHit> {
    let record = trajectory.start().record();
    match record.existence_at(t) {
        Existence::NoSystemYet => return None,
        Existence::Exists => {}
    }
    let state = match trajectory.state_at(t) {
        StateAt::Exists(state) => state,
        StateAt::NoSuchSystem => return None,
    };
    let distance = LightYears::from(sphere.centre().distance_to(state.position()));
    (distance.value() <= sphere.radius().value())
        .then(|| SystemHit::new(*record, *state.position(), distance))
}

/// Whether `sphere` holds the black hole, at rest at the galactic origin.
#[must_use]
fn holds_black_hole(sphere: &QuerySphere) -> bool {
    LightYears::from(sphere.centre().distance_to(&GalacticPosition::ORIGIN)).value()
        <= sphere.radius().value()
}

/// `position` in the centre's frame, ly from the black hole at the galactic origin.
#[must_use]
fn local_of(position: &GalacticPosition) -> PointLy {
    PointLy::from(position)
}

/// The energy above zero of a Kepler orbit of a member drawn at `r` ly or beyond, (km/s)²: at most
/// `Ψ_out(r) − E_out` ([`CentreProfile::outer_psi`], `E_out` the outer cut `floor_km2_s2`), and
/// not below zero (module documentation).
#[must_use]
fn excess_km2_s2(profile: &CentreProfile, floor_km2_s2: f64, r: f64) -> f64 {
    (profile.outer_psi(r) - floor_km2_s2).max(0.0)
}

/// The greatest speed a member drawn at `r` ly has, km/s: `√(2(Ψ(r) − E_out))`, as
/// [`DistributionFunction::max_speed`](super::DistributionFunction::max_speed).
#[must_use]
fn max_speed_km_s(profile: &CentreProfile, floor_km2_s2: f64, r: f64) -> f64 {
    (2.0 * (profile.psi(r) - floor_km2_s2)).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::centre::testing;

    /// Each level's reach holds every one of its owned cells', the pre-selection the hits rest on:
    /// a cell outside the full scan moves at most the level's pad, and a full-scan cell lies inside
    /// the level's edge, at a decade, a century and the whole clock window either way. It reports
    /// the full scan's extent at a century, the plan's `r_full` as built.
    #[test]
    fn each_level_s_reach_holds_its_cells() {
        let galaxy = testing::milky_way_galaxy();
        let source = CentreMemberSource::new(galaxy, &NoCentreOrbitCache).unwrap();
        let motion = galaxy.centre_motion();
        let grid = source.placement().grid();
        for years in [-1_000, -100, -10, 10, 100, 1_000] {
            let span = source.span(UniverseTime::from_julian_years(years).unwrap());
            let (mut full, mut farthest) = (0_u32, 0.0_f64);
            for level in grid.all_levels() {
                let reach = source.level_reach(motion, level, &span);
                let n = grid.cells_per_axis();
                for x in 0..n {
                    for y in 0..n {
                        for cell in (0..n).filter_map(|z| grid.cell(level.level(), [x, y, z])) {
                            let near = grid.local_cell(cell).nearest_radius();
                            match source.cell_reach(motion, cell, &span) {
                                CellReach::Full => {
                                    full += 1;
                                    farthest = farthest.max(near);
                                    assert!(near < reach.edge.value(), "{cell:?} at {years} yr");
                                    assert!(reach.outward.value() > 0.0, "{cell:?} at {years} yr");
                                }
                                CellReach::Plunge(pad) => {
                                    assert!(
                                        pad.value() <= reach.pad.value(),
                                        "{cell:?} at {years} yr"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            println!(
                "{years:+} yr: {full} cells scanned in full, out to a nearest radius of \
                 {farthest:.4} ly"
            );
            // The plan's 0.31 ly at a century, as built about the members' own bound (P09.T29 as
            // built): 0.389 ly over 95,696 cells.
            if years.abs() == 100 {
                assert!((0.385..0.395).contains(&farthest), "{farthest} ly");
                assert!((94_000..97_500).contains(&full), "{full} cells");
            }
        }
    }

    /// A cell's orbits hold every member of the cell in index order, each with the shell its orbit
    /// keeps to; the keeping cache lends them once and counts them; and its bytes are its tables'.
    #[test]
    fn a_cell_s_orbits_are_its_members_on_their_shells() {
        let galaxy = testing::milky_way_galaxy();
        let kept = KeepCentreOrbits::new(galaxy);
        let source = CentreMemberSource::new(galaxy, &kept).unwrap();
        let placement = source.placement();
        // The level-5 cell 1 ly out along z, whose band A holds some tens of members.
        let cell = placement.grid().cell(5, [16, 16, 24]).unwrap();
        let members = placement.members_in_cell(galaxy, MassBand::A, cell);
        let orbits = kept.orbits(galaxy, placement, MassBand::A, cell);
        assert!(members.len() > 10, "{} members", members.len());
        assert_eq!(orbits.trajectories().len(), members.len());
        for ((m, t), &[least, most]) in members
            .iter()
            .zip(orbits.trajectories())
            .zip(&orbits.shells)
        {
            assert_eq!(t.start(), &EpochState::of_centre_member(m));
            let r = local_of(m.record().epoch_position());
            let r = (r.x * r.x + r.y * r.y + r.z * r.z).sqrt();
            assert!(
                least <= r * (1.0 + 1e-9) && r <= most * (1.0 + 1e-9),
                "{r} in [{least}, {most}]"
            );
        }
        assert!(Arc::ptr_eq(
            &orbits,
            &kept.orbits(galaxy, placement, MassBand::A, cell)
        ));
        assert_eq!(kept.members(), members.len());
        assert_eq!(
            orbits.heap_bytes(),
            members.len() * (size_of::<Trajectory>() + size_of::<[f64; 2]>())
        );
    }

    /// The excess energy falls outward, is positive near the black hole, and holds the energy of
    /// a member at the distribution function's greatest speed on its Kepler orbit.
    #[test]
    fn the_excess_holds_the_fastest_member_s_orbit() {
        let galaxy = testing::milky_way_galaxy();
        let model = galaxy.centre().unwrap();
        let profile = model.profile();
        let motion = galaxy.centre_motion();
        let floor_km2_s2 = CentreTracer::ALL
            .iter()
            .map(|&t| model.distribution(t).floor_energy())
            .fold(f64::INFINITY, f64::min);
        let mut last = f64::INFINITY;
        for r in [0.0, 1e-5, 1e-3, 0.03, 0.3, 3.0, 30.0, 128.0] {
            let e = excess_km2_s2(profile, floor_km2_s2, r);
            assert!(
                e <= last,
                "the excess rises outward at {r} ly: {e} after {last}"
            );
            last = e;
            if r > 0.0 {
                // The Kepler energy of the fastest member there, about all the mass inside.
                let v = max_speed_km_s(profile, floor_km2_s2, r);
                let mu =
                    crate::galaxy::consts::G * motion.enclosed_mass(LightYears::new(r)).value();
                let energy = 0.5 * v * v - mu / r;
                assert!(energy <= e * (1.0 + 1e-9), "{r} ly: {energy} above {e}");
            }
        }
        assert!(excess_km2_s2(profile, floor_km2_s2, 0.0) > 0.0);
    }
}
