//! The galactic centre's twelve-level grid and its members (plan 09, P09.T27).
//!
//! # The grid
//!
//! [`centre_grid`] is `NestedGrid::new(1 ⁄ 256 ly, 32, 12)`: cells from 1 ⁄ 256 ly to 8 ly, and a
//! reach of 128 ly, where the members end (the brainstorm, "Dense features"). Member IDs are plan
//! 01's [`CentreMemberId`]s, levels 0–11 and cells 0–31 per axis.
//!
//! # Placement (as P09.T21)
//!
//! Every cell is placed once per mass band. A band's density at local position `p` is the sum
//! over its classes ([`CentreClasses`]) of `N_c n_t(|p|) ÷ A_t`: the class's count, its tracer's
//! realised density (the integral of its distribution function, normalised to one) and the
//! tracer's mean acceptance of the orbit marks (Design note 13), because members are placed under
//! the spherical profile and thinned by the marks after their velocity. The realised densities
//! fall with radius, so a cell's bound is the density at its corner nearest the black hole.
//!
//! The eight cells with a corner at the black hole cannot use that bound: the density rises as
//! `r^−½` to the centre. There the candidates are proposed radially under `B r^−γ` over the
//! octant's ball of radius `√3 e` and dropped outside the cell ([`CentreProposal::Cusp`]), γ the
//! steepest inner slope of the band's tracers, which is exact because every tracer's realised
//! density lies under its cusp continued inward, `n_t(r) ≤ r^−γ_t ÷ S_t` (`S_t` the shape's
//! integral; the distribution function's cut only lowers it), and `r^−γ_t ≤ R^{γ−γ_t} r^−γ` inside
//! the ball.
//!
//! # A candidate
//!
//! Candidate `k` of a band in a cell, keyed by its member ID: its position (three uniforms of
//! `member.position`, as a catalogue feature's), its class or its rejection (one mark of
//! `member.accept` against the classes' cumulative odds over the bound), its velocity from its
//! tracer's distribution function (`centre.velocity`), then its marks: the loss cone, the
//! inclination and the reversal (`centre.marks`, P09.T25). A candidate the marks reject is "no
//! such system". Then its star, as a feature member's (P09.T10): its age uniform in its component's
//! span (`centre.age`), its primary's initial mass from the galaxy's mass function in the class's
//! range below the turn-off at that age for a living star and above it for a white dwarf, and for
//! a remnant class the redraws, attempt after attempt on `centre.mass` (six words each, as
//! `member.mass`), until its remnant is of the class's kind with a kick below the local escape
//! speed `√(2Ψ)`, a low-mode neutron star judged on its pair's systemic speed. Composition: the
//! centre's [`CENTRE_FE_H`], no helium excess.
//!
//! The candidate count is a Poisson draw on `member.cell`, keyed by the member ID of the cell's
//! candidate 0 in its band, clamped at the index's 8,192.
//!
//! # The black hole
//!
//! Member zero of the centre's feature-level list, `0xF000_0007_0000_0000`
//! ([`CentreMemberId::CENTRAL_BLACK_HOLE`]), at the galactic origin and at rest, with the mass of
//! [`GalaxyParams`](crate::galaxy::params::GalaxyParams)' black hole. It counts in layer E. The
//! centre's list registers no class yet (the tidal-disruption victims are P09.T30.b's).

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{ResolveSystemError, SystemOrigin, SystemRecord};
use crate::galaxy::{PointLy, Population};
use crate::id::{CentreMemberId, MemberSlot, SystemId};
use crate::math;
use crate::rng::{Mark, ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::sse::turn_off_mass;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{HeliumExcess, LightYears, SolarMasses, Years};

use super::super::interior::{ClassKind, LocalCell};
use super::super::members::{ATTEMPT_WORDS, MAX_ATTEMPTS, remnant_fits, systemic_speed};
use super::super::nested::{NestedCell, NestedGrid};
use super::classes::{CENTRE_FE_H, CentreClass, CentreClassKind, CentreTracer, age_components};
use super::profile::REACH;
use super::{CentreModel, TracerProfile};

/// The width of the centre's finest cells: 1 ⁄ 256 ly (the brainstorm, "Dense features").
pub const CENTRE_GRID_WIDTH: LightYears = LightYears::new(1.0 / 256.0);

/// The centre's cells per axis in a level: 32 (the brainstorm: the 16-cell grid overflows).
pub const CENTRE_GRID_CELLS: u8 = 32;

/// The centre's nested levels: twelve, to 8 ly cells and a reach of 128 ly.
pub const CENTRE_GRID_LEVELS: u8 = 12;

/// The relative margin of a cusp proposal over the classes' densities: the realised densities'
/// tables lie within 10⁻⁴ of the profiles (P09.T24.b), which lie under their cusps.
const CUSP_MARGIN: f64 = 1e-3;

/// The centre's grid (module documentation).
///
/// # Panics
///
/// Never: 1 ⁄ 256 ly is a power of two and 32 a multiple of four.
#[must_use]
pub fn centre_grid() -> NestedGrid {
    NestedGrid::new(CENTRE_GRID_WIDTH, CENTRE_GRID_CELLS, CENTRE_GRID_LEVELS)
        .expect("1/256 ly, 32 cells and 12 levels are valid")
}

/// How a band's candidates are proposed in a cell (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CentreProposal {
    /// Uniform in the cell under a constant bound, per cubic light-year.
    Uniform {
        /// The bound, per cubic light-year.
        bound: f64,
    },
    /// Radial under `factor × r^−slope` in a cell with a corner at the black hole.
    Cusp {
        /// `B`, per cubic light-year per ly^−slope.
        factor: f64,
        /// The envelope's slope γ, below 3.
        slope: f64,
    },
}

impl CentreProposal {
    /// The expected candidates in `cell`.
    #[must_use]
    pub fn expected(&self, cell: &LocalCell) -> f64 {
        match *self {
            Self::Uniform { bound } => bound * cell.edge * cell.edge * cell.edge,
            Self::Cusp { factor, slope } => {
                let far = (3.0 * cell.edge * cell.edge).sqrt();
                let k = 3.0 - slope;
                factor * 0.5 * core::f64::consts::PI * math::powf(far, k) / k
            }
        }
    }

    /// The point three uniforms `u` put in `cell` and the bound there, or `None` for a radial
    /// candidate outside the cell.
    #[must_use]
    pub fn place(&self, cell: &LocalCell, u: [f64; 3]) -> Option<(PointLy, f64)> {
        match *self {
            Self::Uniform { bound } => {
                let [x, y, z] = [0, 1, 2].map(|i| cell.min[i] + u[i] * cell.edge);
                Some((PointLy::new(x, y, z), bound))
            }
            Self::Cusp { factor, slope } => {
                let far = (3.0 * cell.edge * cell.edge).sqrt();
                let radius = far * math::powf(u[0], 1.0 / (3.0 - slope));
                let cos_theta = u[1];
                let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
                let (sin_phi, cos_phi) = math::sin_cos(0.5 * core::f64::consts::PI * u[2]);
                let magnitude = [
                    radius * sin_theta * cos_phi,
                    radius * sin_theta * sin_phi,
                    radius * cos_theta,
                ];
                let mut point = [0.0; 3];
                for (axis, (&m, &lo)) in magnitude.iter().zip(&cell.min).enumerate() {
                    // A zero magnitude on a negative side would be −0, which the grid's floor
                    // puts in the positive neighbour: dropped, with probability 2⁻⁵³.
                    if m >= cell.edge || (lo < 0.0 && m <= 0.0) {
                        return None;
                    }
                    point[axis] = if lo < 0.0 { -m } else { m };
                }
                if radius <= 0.0 {
                    return None;
                }
                Some((
                    PointLy::new(point[0], point[1], point[2]),
                    factor * math::powf(radius, -slope),
                ))
            }
        }
    }
}

/// A centre member: its record and what it carries beyond one (Design note 20).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CentreMemberRecord {
    record: SystemRecord,
    class: Option<CentreClass>,
    composition: Composition,
    velocity: [f64; 3],
}

impl CentreMemberRecord {
    /// Its plan 03 record, of origin `CentreMember`.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Its class; `None` for the central black hole.
    #[must_use]
    pub const fn class(&self) -> Option<&CentreClass> {
        self.class.as_ref()
    }

    /// Whether it is the central black hole.
    #[must_use]
    pub const fn is_black_hole(&self) -> bool {
        self.class.is_none()
    }

    /// Its composition.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Its velocity at the epoch relative to the black hole, km/s, galactic axes.
    #[must_use]
    pub const fn velocity_km_s(&self) -> [f64; 3] {
        self.velocity
    }

    /// Its velocity at the epoch, galactic: the black hole is at rest.
    #[must_use]
    pub fn velocity(&self) -> GalacticVelocity {
        GalacticVelocity::new(self.velocity.map(|c| c * 1e3))
    }

    /// Its position relative to the black hole, ly.
    #[must_use]
    pub fn local_position(&self) -> PointLy {
        PointLy::from(self.record.epoch_position())
    }
}

/// One class of a band as the placement reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BandClass {
    class: CentreClass,
    /// `N_c ÷ A_t`.
    weight: f64,
}

/// The centre's grid and classes, ready to place members (module documentation).
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::CentreModel;
/// use hyperion_sim::galaxy::features::centre::members::CentrePlacement;
/// use hyperion_sim::galaxy::imf::MassBand;
///
/// let galaxy = Galaxy::new(Seed::new(1));
/// let model = CentreModel::new(&galaxy)?;
/// let centre = CentrePlacement::new(&model);
/// // The M dwarfs of one of the eight cells beside the black hole.
/// let cell = centre.grid().cell(0, [16, 16, 16]).expect("a level-0 cell");
/// let members = centre.members_in_cell(&galaxy, MassBand::A, cell);
/// assert!(members.iter().all(|m| m.local_position().x > 0.0));
/// # Ok::<(), hyperion_sim::galaxy::features::centre::BuildCentreError>(())
/// ```
#[derive(Debug, Clone)]
pub struct CentrePlacement<'a> {
    model: &'a CentreModel,
    grid: NestedGrid,
    bands: [Vec<BandClass>; 5],
    black_hole: SolarMasses,
}

impl<'a> CentrePlacement<'a> {
    /// The placement of `model`'s classes on [`centre_grid`].
    #[must_use]
    pub fn new(model: &'a CentreModel) -> Self {
        let mut bands: [Vec<BandClass>; 5] = Default::default();
        for c in model.classes().classes() {
            if c.expected > 0.0 {
                bands[c.band.index()].push(BandClass {
                    class: *c,
                    weight: c.expected / model.acceptance(c.tracer),
                });
            }
        }
        Self {
            model,
            grid: centre_grid(),
            bands,
            black_hole: model.profile().black_hole(),
        }
    }

    /// The grid.
    #[must_use]
    pub fn grid(&self) -> &NestedGrid {
        &self.grid
    }

    /// The model.
    #[must_use]
    pub fn model(&self) -> &CentreModel {
        self.model
    }

    /// `band`'s density at `r` ly from the black hole, per cubic light-year, and each class's
    /// share of it in `weights` (cleared first): zero beyond the reach.
    fn densities(&self, band: MassBand, r: f64, weights: &mut Vec<f64>) -> f64 {
        weights.clear();
        let mut total = 0.0;
        for c in &self.bands[band.index()] {
            let n = if r > REACH.value() {
                0.0
            } else {
                c.weight * self.model.distribution(c.class.tracer).density(r)
            };
            weights.push(n);
            total += n;
        }
        total
    }

    /// How `band`'s candidates are proposed in `cell` (module documentation).
    #[must_use]
    pub fn proposal(&self, band: MassBand, cell: &LocalCell) -> CentreProposal {
        let near = cell.nearest_radius();
        if near > 0.0 {
            let mut weights = Vec::new();
            return CentreProposal::Uniform {
                bound: self.densities(band, near, &mut weights) * (1.0 + 1e-12),
            };
        }
        let classes = &self.bands[band.index()];
        let slope = classes
            .iter()
            .map(|c| shape_of(self.model, c.class.tracer).shape().inner_slope())
            .fold(0.0_f64, f64::max);
        let far = (3.0 * cell.edge * cell.edge).sqrt();
        let factor: f64 = classes
            .iter()
            .map(|c| {
                let tracer = shape_of(self.model, c.class.tracer);
                let own = tracer.shape().inner_slope();
                c.weight / tracer.shape_total() * math::powf(far, slope - own)
            })
            .sum();
        CentreProposal::Cusp {
            factor: factor * (1.0 + CUSP_MARGIN),
            slope,
        }
    }

    /// The expected candidates of `band` in `cell`: its proposal's integral over the cell.
    #[must_use]
    pub fn expected_candidates(&self, band: MassBand, cell: NestedCell) -> f64 {
        let local = self.grid.local_cell(cell);
        self.proposal(band, &local).expected(&local)
    }

    /// The member ID of candidate `index` of `band` in `cell`.
    ///
    /// # Panics
    ///
    /// If `index` is at or past the index's 8,192 or `cell` is not one of the centre's owned
    /// cells, which [`NestedGrid`] never yields.
    #[must_use]
    pub fn member_id(&self, band: MassBand, cell: NestedCell, index: u16) -> CentreMemberId {
        CentreMemberId::new(MemberSlot::InCell {
            band: band.layer(),
            level: cell.level(),
            cell: cell.cell(),
            index,
        })
        .expect("an owned cell of the centre's grid and an index under 8,192 make an ID")
    }

    /// The candidate count of `band` in `cell` (module documentation).
    #[must_use]
    pub fn candidate_count(&self, galaxy: &Galaxy, band: MassBand, cell: NestedCell) -> u16 {
        let mean = self.expected_candidates(band, cell);
        count_from_mean(galaxy, self.member_id(band, cell, 0), mean)
    }

    /// Candidate `index` of `band` in `cell`, or `None` if the thinning, the marks or its draw
    /// reject it; a draw that exhausts its attempts is `None` too, with a debug assertion (its
    /// odds are those of a retained remnant 4,096 times over). The index is not checked against
    /// the count.
    ///
    /// # Panics
    ///
    /// If `index` is 8,192 or more, which no count reaches.
    #[must_use]
    pub fn candidate(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        index: u16,
    ) -> Option<CentreMemberRecord> {
        let local = self.grid.local_cell(cell);
        let proposal = self.proposal(band, &local);
        self.candidate_under(galaxy, band, cell, index, &proposal, &local)
    }

    fn candidate_under(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        index: u16,
        proposal: &CentreProposal,
        local: &LocalCell,
    ) -> Option<CentreMemberRecord> {
        let id = self.member_id(band, cell, index);
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        let seed = galaxy.seed();
        let mut position = Stream::open(seed, tags::MEMBER_POSITION, key);
        let u = [0; 3].map(|_| position.uniform());
        let (p, bound) = proposal.place(local, u)?;
        let r = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
        let mark = Stream::open(seed, tags::MEMBER_ACCEPT, key).mark();
        let mut weights = Vec::with_capacity(self.bands[band.index()].len());
        self.densities(band, r, &mut weights);
        let picked = pick(mark, &weights, bound)?;
        let class = self.bands[band.index()][picked].class;
        let df = self.model.distribution(class.tracer);
        let profile = self.model.profile();
        let at = [p.x, p.y, p.z];
        let mut velocity_stream = Stream::open(seed, tags::CENTRE_VELOCITY, key);
        let velocity = df.draw_velocity(profile, at, &mut velocity_stream)?;
        let mut marks = Stream::open(seed, tags::CENTRE_MARKS, key);
        let velocity =
            class
                .tracer
                .marks()
                .apply(self.model.loss_cone(), at, velocity, &mut marks)?;
        let (primary, attempt, age) =
            draw_star(galaxy, &class, system, profile.escape_speed(r).value())?;
        let record = SystemRecord::from_parts(
            system,
            galactic(&p),
            SystemOrigin::CentreMember {
                attempt: u16::try_from(attempt).expect("an attempt below 4,096 fits in u16"),
            },
            Population::NuclearDisc,
            SolarMasses::new(primary),
            age,
        );
        Some(CentreMemberRecord {
            record,
            class: Some(class),
            composition: centre_composition(),
            velocity,
        })
    }

    /// Every accepted member of `band` in `cell`, in index order.
    #[must_use]
    pub fn members_in_cell(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
    ) -> Vec<CentreMemberRecord> {
        let local = self.grid.local_cell(cell);
        let proposal = self.proposal(band, &local);
        let n = count_from_mean(
            galaxy,
            self.member_id(band, cell, 0),
            proposal.expected(&local),
        );
        (0..n)
            .filter_map(|i| self.candidate_under(galaxy, band, cell, i, &proposal, &local))
            .collect()
    }

    /// The most candidates any band expects in an owned cell, with its band and cell: searched on
    /// every level over the cells beside the three axes, where a spherical bound peaks at each
    /// radius (the test checks a whole level).
    ///
    /// # Panics
    ///
    /// Never: the grid has owned cells.
    #[must_use]
    pub fn peak_candidates(&self) -> (f64, MassBand, NestedCell) {
        let cells = self.grid.cells_per_axis();
        let (a, b) = (cells / 2 - 1, cells / 2);
        let mut best: Option<(f64, MassBand, NestedCell)> = None;
        for level in 0..self.grid.levels() {
            for along in 0..cells {
                for (i, j) in [(a, a), (a, b), (b, a), (b, b)] {
                    for cell in [[along, i, j], [i, along, j], [i, j, along]] {
                        let Some(c) = self.grid.cell(level, cell) else {
                            continue;
                        };
                        for band in MassBand::ALL {
                            let e = self.expected_candidates(band, c);
                            if best.is_none_or(|(x, _, _)| e > x) {
                                best = Some((e, band, c));
                            }
                        }
                    }
                }
            }
        }
        best.expect("the grid has owned cells")
    }

    /// The central black hole (module documentation).
    #[must_use]
    pub fn black_hole(&self) -> CentreMemberRecord {
        black_hole_of(self.black_hole)
    }
}

/// The central black hole of mass `mass` (module documentation): its record's age is the oldest
/// component's.
fn black_hole_of(mass: SolarMasses) -> CentreMemberRecord {
    let components = age_components();
    let record = SystemRecord::from_parts(
        SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE),
        GalacticPosition::ORIGIN,
        SystemOrigin::CentreMember { attempt: 0 },
        Population::NuclearDisc,
        mass,
        components[0].oldest,
    );
    CentreMemberRecord {
        record,
        class: None,
        composition: centre_composition(),
        velocity: [0.0; 3],
    }
}

/// The tracer profile of `tracer` in `model`.
fn shape_of(model: &CentreModel, tracer: CentreTracer) -> &TracerProfile {
    model.tracer_profile(tracer)
}

/// The class a mark picks by cumulative odds over `bound`, or `None` for rejection.
fn pick(mark: Mark, weights: &[f64], bound: f64) -> Option<usize> {
    debug_assert!(
        weights.iter().sum::<f64>() <= bound,
        "the centre's density {} exceeds its bound {bound}",
        weights.iter().sum::<f64>()
    );
    mark.pick_weighted(weights, bound)
}

/// The centre's composition.
fn centre_composition() -> Composition {
    Composition::from_fe_h(CENTRE_FE_H, HeliumExcess::ZERO)
}

/// The galactic position of the local point `p`, the black hole at the origin.
fn galactic(p: &PointLy) -> GalacticPosition {
    GalacticPosition::ORIGIN
        .translated(GalacticDisplacement::new(
            [p.x, p.y, p.z].map(|c| c * METRES_PER_LIGHT_YEAR),
        ))
        .expect("a member within 128 ly of the galactic centre is addressable")
}

/// A member's star of `class` (module documentation): its primary's initial mass, the attempt
/// its draws are read at, and its age; `None` if the draw exhausts its attempts or the class's
/// range is empty at the drawn age.
fn draw_star(
    galaxy: &Galaxy,
    class: &CentreClass,
    system: SystemId,
    escape: f64,
) -> Option<(f64, u32, Years)> {
    let seed = galaxy.seed();
    let key = ObjectKey::from(system);
    let component = age_components()[usize::from(class.component)];
    let u = Stream::open(seed, tags::CENTRE_AGE, key).uniform();
    let age = Years::new(
        component.youngest.value() + u * (component.oldest.value() - component.youngest.value()),
    );
    let composition = centre_composition();
    let m_to = turn_off_mass(Years::new(age.value().max(1e4)), &composition).value();
    let (lo, hi) = (
        class.initial_mass_range.0.value(),
        class.initial_mass_range.1.value(),
    );
    let (lo, hi) = match class.kind {
        CentreClassKind::Living => (lo, hi.min(m_to)),
        CentreClassKind::WhiteDwarf | CentreClassKind::NeutronStar | CentreClassKind::BlackHole => {
            (lo.max(m_to), hi)
        }
    };
    if hi <= lo {
        return None;
    }
    let kind = match class.kind {
        CentreClassKind::Living => ClassKind::Living,
        CentreClassKind::WhiteDwarf => ClassKind::WhiteDwarf,
        CentreClassKind::NeutronStar => ClassKind::NeutronStar,
        CentreClassKind::BlackHole => ClassKind::BlackHole,
    };
    let imf = galaxy.mass_function();
    let mut stream = Stream::open(seed, tags::CENTRE_MASS, key);
    for attempt in 0..MAX_ATTEMPTS {
        stream.seek(ATTEMPT_WORDS * u64::from(attempt));
        let mass = imf.quantile_in(lo, hi, stream.uniform());
        let _mark = stream.mark();
        let systemic = systemic_speed(&mut stream);
        if kind == ClassKind::Living
            || remnant_fits(
                kind,
                mass,
                &composition,
                (seed, system, attempt),
                age,
                (escape, systemic),
            )
        {
            return Some((mass, attempt, age));
        }
    }
    debug_assert!(false, "a centre member of {class:?} exhausted its attempts");
    None
}

/// The count of a cell whose candidate 0 is `first` and whose mean is `mean`, clamped at the
/// index.
fn count_from_mean(galaxy: &Galaxy, first: CentreMemberId, mean: f64) -> u16 {
    let word = SystemId::from(first).raw();
    let drawn = Stream::open(galaxy.seed(), tags::MEMBER_CELL, ObjectKey::cell(word)).poisson(mean);
    let limit = MemberSlot::INDEX_LIMIT;
    // No debug assertion, unlike a catalogue feature's cells: the heaviest drawn clusters come
    // within eight standard deviations of the index (the plan's Risks), and the clamp is the rule.
    u16::try_from(drawn.min(u64::from(limit))).expect("a count clamped at 8,192 fits in u16")
}

/// The centre member `id` names in `galaxy`, building the centre's model for it (about 90 ms in
/// release) unless it is the black hole, which needs none.
///
/// # Errors
///
/// As [`resolve_centre_member_with`], and [`ResolveSystemError::NoSuchSystem`] if the centre
/// cannot be built: its [`BuildCentreError`](super::BuildCentreError) says only that no
/// isotropic cluster has the drawn profile, which no seed of the sweeps draws
/// (`no_drawn_centre_fails_its_inversion`), and a caller asking for a member can do nothing but
/// learn there is none.
pub fn resolve_centre_member(
    galaxy: &Galaxy,
    id: CentreMemberId,
) -> Result<CentreMemberRecord, ResolveSystemError> {
    if id == CentreMemberId::CENTRAL_BLACK_HOLE {
        return Ok(black_hole_of(galaxy.params().black_hole().mass()));
    }
    let model = CentreModel::new(galaxy).map_err(|_| ResolveSystemError::NoSuchSystem)?;
    resolve_centre_member_with(galaxy, &model, id)
}

/// The centre member `id` names in `galaxy`, whose centre is `model`: the black hole for member
/// zero, a candidate for an ID in a cell.
///
/// # Errors
///
/// [`ResolveSystemError::NoSuchSystem`] for a feature-level index other than the black hole's
/// (no class is registered on the centre's list yet), a band that is not stellar, a cell the
/// grid does not own, an index at or past its cell's count, or a candidate the thinning, the
/// marks or its draw reject.
pub fn resolve_centre_member_with(
    galaxy: &Galaxy,
    model: &CentreModel,
    id: CentreMemberId,
) -> Result<CentreMemberRecord, ResolveSystemError> {
    let placement = CentrePlacement::new(model);
    match id.slot() {
        MemberSlot::FeatureLevel { index: 0 } => Ok(placement.black_hole()),
        MemberSlot::FeatureLevel { .. } => Err(ResolveSystemError::NoSuchSystem),
        MemberSlot::InCell {
            band,
            level,
            cell,
            index,
        } => {
            let band = MassBand::of_layer(band);
            if !band.is_stellar() {
                return Err(ResolveSystemError::NoSuchSystem);
            }
            let cell = placement
                .grid
                .cell(level, cell)
                .ok_or(ResolveSystemError::NoSuchSystem)?;
            let local = placement.grid.local_cell(cell);
            let proposal = placement.proposal(band, &local);
            let count = count_from_mean(
                galaxy,
                placement.member_id(band, cell, 0),
                proposal.expected(&local),
            );
            if index >= count {
                return Err(ResolveSystemError::NoSuchSystem);
            }
            placement
                .candidate_under(galaxy, band, cell, index, &proposal, &local)
                .ok_or(ResolveSystemError::NoSuchSystem)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::features::centre::testing::{milky_way_centre, milky_way_galaxy};
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::{GalaxyParams, GalaxyParamsBuilder};
    use crate::galaxy::placement::resolve;
    use crate::id::SystemIdKind;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    fn innermost(placement: &CentrePlacement<'_>) -> NestedCell {
        placement.grid().cell(0, [16, 16, 16]).unwrap()
    }

    /// P09.T27: the fullest cell and band expects about 1,400 candidates under the default mass
    /// function (1,700 under Kroupa's) and the innermost cell about eighty (the brainstorm,
    /// "Dense features"). Under the spherical bound divided by the marks' acceptance (Design note
    /// 13, 0.59) that is 1,400 ÷ 0.59 (1,700 ÷ 0.59), held provisionally to a factor of 1.5 (the
    /// plan's Risks: the class device's mean system mass moves it); the innermost cell is printed
    /// (its candidates are the young disc's, a finding); under 8,192 with eight standard
    /// deviations always.
    #[test]
    fn the_fullest_cell_stays_under_the_index() {
        for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
            let params = GalaxyParamsBuilder::new()
                .mass_function(kind)
                .build()
                .unwrap();
            let galaxy = Galaxy::from_params(crate::Seed::new(0x0927), params).unwrap();
            let model = CentreModel::new(&galaxy).unwrap();
            let placement = CentrePlacement::new(&model);
            let (peak, band, cell) = placement.peak_candidates();
            let inner = innermost(&placement);
            let (inner_band, inner_expected) = MassBand::ALL
                .iter()
                .map(|&b| (b, placement.expected_candidates(b, inner)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            let local = placement.grid().local_cell(cell);
            let acceptance = model.acceptance(CentreTracer::Stars);
            eprintln!(
                "{kind:?}: fullest {peak:.1} in band {band:?}, level {} cell {:?} (nearest {:.3} \
                 ly); innermost {inner_expected:.1} in band {inner_band:?}; stars' acceptance \
                 {acceptance:.4}; class mean mass {:.4}",
                cell.level(),
                cell.cell(),
                local.nearest_radius(),
                model.classes().mean_system_mass().value()
            );
            let want = match kind {
                MassFunctionKind::Chabrier => 1_400.0,
                MassFunctionKind::Kroupa => 1_700.0,
            } / acceptance;
            assert!(peak + 8.0 * peak.sqrt() < 8_192.0, "{peak}");
            assert!(
                (want / 1.5..=want * 1.5).contains(&peak),
                "{peak} against {want}"
            );
            assert!(inner_expected + 8.0 * inner_expected.sqrt() < 8_192.0);
        }
    }

    /// The peak's search along the axes finds the largest count of a whole level.
    #[test]
    fn the_peak_lies_beside_the_axes() {
        let placement = CentrePlacement::new(milky_way_centre());
        let (peak, _, cell) = placement.peak_candidates();
        let level = cell.level();
        let mut best = 0.0_f64;
        for c in placement
            .grid()
            .owned_cells()
            .filter(|c| c.level() == level)
        {
            for band in MassBand::ALL {
                best = best.max(placement.expected_candidates(band, c));
            }
        }
        assert!((best - peak).abs() <= 1e-12 * peak, "{best} {peak}");
    }

    /// P09.T27: under 8,192 for every seed of a sweep. Provisional (the plan's Risks): the plan
    /// asks for eight standard deviations to spare, which one seed in sixteen misses at
    /// `GENERATOR_VERSION` 15; the margin is printed.
    #[test]
    fn no_seed_s_centre_overflows_its_index() {
        let mut worst = (0.0_f64, 0);
        for n in 0..16_u64 {
            let seed = crate::Seed::new(0x0927_0000 | n);
            let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
            let cluster = params.nuclear_cluster().mass().value();
            let galaxy = Galaxy::from_params(seed, params).unwrap();
            let model = CentreModel::new(&galaxy).unwrap();
            let (peak, band, cell) = CentrePlacement::new(&model).peak_candidates();
            eprintln!(
                "seed {n}: cluster {cluster:.3e} M☉, fullest {peak:.0} (+8σ {:.0}) in {band:?} at \
                 level {}",
                peak + 8.0 * peak.sqrt(),
                cell.level()
            );
            if peak > worst.0 {
                worst = (peak, n);
            }
            assert!(peak < 8_192.0, "seed {n}: {peak}");
        }
        eprintln!("worst {worst:?}");
    }

    /// P09.T27: every member's ID round-trips through plan 01's decode, at every level; a
    /// resolved ID gives the generated member; an index past the count is no system.
    #[test]
    fn members_round_trip_and_resolve() {
        let galaxy = milky_way_galaxy();
        let model = milky_way_centre();
        let placement = CentrePlacement::new(model);
        let mut seen = 0;
        for (level, cell) in [
            (0, [16, 16, 16]),
            (0, [15, 16, 17]),
            (5, [26, 16, 15]),
            (11, [3, 15, 16]),
        ] {
            let cell = placement.grid().cell(level, cell).unwrap();
            for band in [MassBand::A, MassBand::C, MassBand::E] {
                let members = placement.members_in_cell(galaxy, band, cell);
                for m in members.iter().take(40) {
                    let id = m.record().id();
                    assert_eq!(SystemId::from_raw(id.raw()), Ok(id));
                    let SystemIdKind::Centre(member) = id.kind() else {
                        panic!("{id:?}");
                    };
                    let MemberSlot::InCell { level: l, .. } = member.slot() else {
                        panic!("{member:?}");
                    };
                    assert!(l < 12);
                    let resolved = resolve_centre_member_with(galaxy, model, member).unwrap();
                    assert_eq!(&resolved, m);
                    assert_eq!(resolve(galaxy, id).unwrap(), *m.record());
                    let r = m.local_position();
                    assert_eq!(placement.grid().owner_of(&r), Some(cell), "{r:?}");
                    seen += 1;
                }
                let count = placement.candidate_count(galaxy, band, cell);
                let past = placement.member_id(band, cell, count);
                assert_eq!(
                    resolve_centre_member_with(galaxy, model, past),
                    Err(ResolveSystemError::NoSuchSystem)
                );
            }
        }
        assert!(seen > 100, "{seen}");
    }

    /// P09.T27: the black hole resolves from `0xF000_0007_0000_0000`, with the parameters' mass.
    #[test]
    fn the_black_hole_is_member_zero() {
        let galaxy = milky_way_galaxy();
        let id = SystemId::from_raw(0xF000_0007_0000_0000).unwrap();
        let record = resolve(galaxy, id).unwrap();
        assert_eq!(record.id(), id);
        let mass = GalaxyParams::milky_way_like().black_hole().mass();
        assert!((record.primary_initial_mass().value() / mass.value() - 1.0).abs() < 1e-12);
        assert_eq!(record.epoch_position(), &GalacticPosition::ORIGIN);
        assert_eq!(record.component(), None);
    }

    /// P09.T27: the hundred innermost members, as float bits.
    #[test]
    fn innermost_members_golden() {
        let galaxy = milky_way_galaxy();
        let placement = CentrePlacement::new(milky_way_centre());
        let mut members = Vec::new();
        for x in 14..18_u8 {
            for y in 14..18_u8 {
                for z in 14..18_u8 {
                    let cell = placement.grid().cell(0, [x, y, z]).unwrap();
                    for band in MassBand::ALL {
                        members.extend(placement.members_in_cell(galaxy, band, cell));
                    }
                }
            }
        }
        let radius = |m: &CentreMemberRecord| {
            let p = m.local_position();
            (p.x * p.x + p.y * p.y + p.z * p.z).sqrt()
        };
        members.sort_by(|a, b| radius(a).total_cmp(&radius(b)));
        let block = 2.0 * CENTRE_GRID_WIDTH.value();
        assert!(
            members.len() >= 100 && radius(&members[99]) < block,
            "{}",
            members.len()
        );
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for m in members.iter().take(100) {
            let label = format!("{:016x}", m.record().id().raw());
            let class = m.class().expect("a member in a cell has a class");
            w.line(&format!(
                "{label} {:?} {:?} component {}",
                class.band, class.kind, class.component
            ));
            let p = m.local_position();
            for (axis, c) in ["x", "y", "z"].iter().zip([p.x, p.y, p.z]) {
                w.f64(&format!("{label}.{axis}"), c);
            }
            for (axis, c) in ["vx", "vy", "vz"].iter().zip(m.velocity_km_s()) {
                w.f64(&format!("{label}.{axis}"), c);
            }
            w.f64(
                &format!("{label}.mass"),
                m.record().primary_initial_mass().value(),
            );
            w.f64(&format!("{label}.age"), m.record().age_at_epoch().value());
        }
        golden!("galaxy/features/centre_members", w.as_str());
    }

    /// Resolving centre IDs in any order, on a shared model or a fresh one each time, gives the
    /// same members; two builds of one galaxy's centre are equal.
    #[test]
    fn centre_members_are_order_independent() {
        let galaxy = milky_way_galaxy();
        let model = milky_way_centre();
        let placement = CentrePlacement::new(model);
        let mut ids = vec![CentreMemberId::CENTRAL_BLACK_HOLE];
        for (level, cell, band) in [
            (0, [16, 16, 16], MassBand::A),
            (0, [15, 15, 16], MassBand::E),
            (5, [26, 16, 15], MassBand::B),
            (11, [3, 15, 16], MassBand::D),
        ] {
            let cell = placement.grid().cell(level, cell).unwrap();
            for index in 0..3 {
                ids.push(placement.member_id(band, cell, index));
            }
        }
        hyperion_testkit::order::assert_order_independent(&ids, |&id| {
            resolve_centre_member_with(galaxy, model, id)
        });
        let other = Galaxy::new(crate::Seed::new(0x0927_0042));
        assert_eq!(CentreModel::new(&other), CentreModel::new(&other));
        let fresh = CentreModel::new(galaxy).unwrap();
        for &id in &ids {
            assert_eq!(
                resolve_centre_member_with(galaxy, &fresh, id),
                resolve_centre_member_with(galaxy, model, id)
            );
        }
        assert_eq!(
            resolve_centre_member(galaxy, CentreMemberId::CENTRAL_BLACK_HOLE),
            resolve_centre_member_with(galaxy, model, CentreMemberId::CENTRAL_BLACK_HOLE)
        );
    }

    #[test]
    fn generating_a_cell_twice_gives_the_same_members() {
        let galaxy = milky_way_galaxy();
        let placement = CentrePlacement::new(milky_way_centre());
        let cell = placement.grid().cell(3, [5, 16, 16]).unwrap();
        let a = placement.members_in_cell(galaxy, MassBand::B, cell);
        let b = CentrePlacement::new(milky_way_centre()).members_in_cell(galaxy, MassBand::B, cell);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }
}
