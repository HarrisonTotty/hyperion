//! The member class table: each band's classes with their counts and profiles, their bound over a
//! cell and the pick of a class (plan 09, P09.T8.b; Design notes 9 and 10).
//!
//! Positions here are local: light-years from the cluster's centre, in the galactic axes. The
//! table's `bound` over a cell is the sum of each profiled class's count times its profile at the
//! cell's corner nearest the centre, which the profiles' monotony makes exact, plus the tail's
//! bound, the axis density at the point of the cell's bounding sphere nearest the tail's line
//! (Design note 10), a true bound because the Gaussian falls with distance from the line. One mark
//! then picks a class by cumulative odds in the table's fixed order or rejects the candidate,
//! through plan 01's [`Mark::pick_weighted`].
//!
//! # The tail (P09.T9.g)
//!
//! A band's tail is a straight tube along the cluster's bulk velocity through its centre, from the
//! tidal radius `r_t` out to the reach on both sides, Gaussian across with a width of one tidal
//! radius: `n(s, d) = λ exp(−d² ÷ 2w²) ÷ (2π w²)` for `r_t ≤ |s| ≤ reach`, with λ the members per
//! light-year of each side. Its members also lie within the reach of the centre (P09.T21), so that
//! a feature's reach holds all of its members: the density is zero beyond it, and the expected
//! count, the tube's to the reach along the axis, errs high by the Gaussian's share outside the
//! sphere, which the census allows.

use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::imf::MassBand;
use crate::math;
use crate::rng::Mark;
use crate::units::LightYears;

use super::super::cluster::ClusterModel;
use super::super::nested::{NestedCell, NestedGrid};
use super::MemberClass;
use super::counts::{ClassCounts, class_counts};
use super::profile::{CUSP_SOFTENING, ClassProfile, ProfileShape};

/// A cubic cell in the cluster's frame: its least corner and edge, ly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalCell {
    /// The corner of least x, y and z, ly from the centre.
    pub min: [f64; 3],
    /// The edge, ly.
    pub edge: f64,
}

impl LocalCell {
    /// The distance from the centre to the cell's nearest point, ly.
    #[must_use]
    pub fn nearest_radius(&self) -> f64 {
        let mut sum = 0.0;
        for &lo in &self.min {
            let hi = lo + self.edge;
            let d = if lo > 0.0 {
                lo
            } else if hi < 0.0 {
                -hi
            } else {
                0.0
            };
            sum += d * d;
        }
        sum.sqrt()
    }

    /// The cell's centre and the radius of its bounding sphere, ly.
    #[must_use]
    pub fn bounding_sphere(&self) -> (PointLy, f64) {
        let h = 0.5 * self.edge;
        (
            PointLy::new(self.min[0] + h, self.min[1] + h, self.min[2] + h),
            h * 3.0_f64.sqrt(),
        )
    }
}

/// One band's tail (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TailClass {
    class: MemberClass,
    expected: f64,
    axis: [f64; 3],
    inner: f64,
    reach: f64,
    width: f64,
}

impl TailClass {
    /// The tail's class.
    #[must_use]
    pub const fn class(&self) -> &MemberClass {
        &self.class
    }

    /// Its expected count over both sides.
    #[must_use]
    pub const fn expected(&self) -> f64 {
        self.expected
    }

    /// Members per light-year of each side.
    fn line_density(&self) -> f64 {
        self.expected / (2.0 * (self.reach - self.inner))
    }

    /// The axis density `λ ÷ (2π w²)`, per cubic light-year.
    fn axis_density(&self) -> f64 {
        self.line_density() / (2.0 * core::f64::consts::PI * self.width * self.width)
    }

    /// The tail's density at local `p`, per cubic light-year: zero beyond the reach of the centre.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        let s = p.x * self.axis[0] + p.y * self.axis[1] + p.z * self.axis[2];
        let along = s.abs();
        let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
        if along < self.inner || along > self.reach || r2 > self.reach * self.reach {
            return 0.0;
        }
        let d2 = (r2 - s * s).max(0.0);
        self.axis_density() * math::exp(-0.5 * d2 / (self.width * self.width))
    }

    /// A bound on the tail's density over the sphere of `radius` about local `centre` (Design
    /// note 10): 0 if the sphere misses both ends' spans along the axis.
    #[must_use]
    pub fn bound(&self, centre: &PointLy, radius: f64) -> f64 {
        let s = centre.x * self.axis[0] + centre.y * self.axis[1] + centre.z * self.axis[2];
        let (lo, hi) = (s.abs() - radius, s.abs() + radius);
        if hi < self.inner || lo > self.reach {
            return 0.0;
        }
        let d2 = (centre.x * centre.x + centre.y * centre.y + centre.z * centre.z - s * s).max(0.0);
        let d = (d2.sqrt() - radius).max(0.0);
        self.axis_density() * math::exp(-0.5 * d * d / (self.width * self.width)) * (1.0 + 1e-12)
    }
}

/// One profiled class of a band: the class, its count and its profile.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    class: MemberClass,
    expected: f64,
    profile: ClassProfile,
}

/// A cluster's member classes band by band (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct MemberClassTable {
    bands: [Vec<Row>; 5],
    tails: [Option<TailClass>; 5],
    depleted_slope: f64,
    runaway_share: f64,
    tidal: f64,
}

impl MemberClassTable {
    /// The table of `model` in `galaxy`, its tails running from the tidal radius to `reach` along
    /// `tail_axis` (the bulk velocity's direction; normalised here, and along x if zero).
    #[must_use]
    pub fn new(
        galaxy: &Galaxy,
        model: &ClusterModel,
        reach: LightYears,
        tail_axis: [f64; 3],
    ) -> Self {
        let counts = class_counts(galaxy, model, reach);
        Self::from_counts(model, &counts, reach, tail_axis)
    }

    /// The table of the counts `counts` of `model`.
    #[must_use]
    pub fn from_counts(
        model: &ClusterModel,
        counts: &ClassCounts,
        reach: LightYears,
        tail_axis: [f64; 3],
    ) -> Self {
        let tidal = model.tidal_radius().value();
        let mut bands: [Vec<Row>; 5] = Default::default();
        for c in &counts.classes {
            if c.expected > 0.0 {
                bands[c.class.band.index()].push(Row {
                    class: c.class,
                    expected: c.expected,
                    profile: ClassProfile::new(c.shape, tidal, 1.0),
                });
            }
        }
        let norm = (tail_axis[0] * tail_axis[0]
            + tail_axis[1] * tail_axis[1]
            + tail_axis[2] * tail_axis[2])
            .sqrt();
        let axis = if norm > 0.0 {
            tail_axis.map(|a| a / norm)
        } else {
            [1.0, 0.0, 0.0]
        };
        let tails = counts.tails.map(|t| {
            t.filter(|t| t.expected > 0.0 && reach.value() > tidal)
                .map(|t| TailClass {
                    class: t.class,
                    expected: t.expected,
                    axis,
                    inner: tidal,
                    reach: reach.value(),
                    width: tidal,
                })
        });
        Self {
            bands,
            tails,
            depleted_slope: counts.depleted_slope,
            runaway_share: counts.runaway_share,
            tidal,
        }
    }

    /// The rows of `band`: none for plan 13's substellar bands, which a cluster's class table does
    /// not hold (their members are plan 13's to place).
    fn rows(&self, band: MassBand) -> &[Row] {
        self.bands.get(band.index()).map_or(&[], Vec::as_slice)
    }

    /// The profiled classes of `band` with their counts, in the table's fixed order.
    pub fn classes(&self, band: MassBand) -> impl Iterator<Item = (&MemberClass, f64)> {
        self.rows(band).iter().map(|r| (&r.class, r.expected))
    }

    /// The tail of `band`, if any.
    #[must_use]
    pub fn tail(&self, band: MassBand) -> Option<&TailClass> {
        self.tails.get(band.index()).and_then(Option::as_ref)
    }

    /// The millisecond pulsars the cluster's neutron stars hold: `model`'s encounter-rate law
    /// (P09.T9.e), never more than the neutron stars themselves, since a pulsar is a mark inside
    /// that class (ruling 126.3).
    #[must_use]
    pub fn millisecond_pulsars(&self, model: &ClusterModel) -> f64 {
        model
            .millisecond_pulsars()
            .min(self.expected_of(super::ClassKind::NeutronStar))
    }

    /// The depleted slope over 0.2–0.8 M☉ (P09.T9.a).
    #[must_use]
    pub const fn depleted_slope(&self) -> f64 {
        self.depleted_slope
    }

    /// A young cluster's band-E runaway share (P09.T9.f).
    #[must_use]
    pub const fn runaway_share(&self) -> f64 {
        self.runaway_share
    }

    /// The expected members of `band`, its tail included.
    #[must_use]
    pub fn expected(&self, band: MassBand) -> f64 {
        self.rows(band).iter().map(|r| r.expected).sum::<f64>()
            + self.tail(band).map_or(0.0, TailClass::expected)
    }

    /// The expected members of `class`'s kind, over every band, population and multiplicity.
    #[must_use]
    pub fn expected_of(&self, kind: super::ClassKind) -> f64 {
        let profiled: f64 = self
            .bands
            .iter()
            .flatten()
            .filter(|r| r.class.kind == kind)
            .map(|r| r.expected)
            .sum();
        let tails: f64 = self
            .tails
            .iter()
            .flatten()
            .filter(|t| t.class.kind == kind)
            .map(TailClass::expected)
            .sum();
        profiled + tails
    }

    /// The bound of `band`'s profiled classes at the corner radius `corner_radius` (ly): the sum
    /// of count × profile there, per cubic light-year. With [`TailClass::bound`] it bounds a cell
    /// ([`cell_bound`](Self::cell_bound)).
    #[must_use]
    pub fn bound(&self, band: MassBand, corner_radius: f64) -> f64 {
        self.rows(band)
            .iter()
            .map(|r| r.expected * r.profile.density_at_radius(corner_radius))
            .fold(0.0, |sum, d| sum + d)
    }

    /// The bound of `band` over `cell`: the profiled classes at its nearest corner and the tail
    /// over its bounding sphere, with a margin of 10⁻¹² over the rounding.
    #[must_use]
    pub fn cell_bound(&self, band: MassBand, cell: &LocalCell) -> f64 {
        let (centre, radius) = cell.bounding_sphere();
        (self.bound(band, cell.nearest_radius())
            + self.tail(band).map_or(0.0, |t| t.bound(&centre, radius)))
            * (1.0 + 1e-12)
    }

    /// The density of `band`'s members at local `p` by class, into `out` (cleared first), in the
    /// order [`pick`](Self::pick) reads: the profiled classes, then the tail.
    pub fn weights(&self, band: MassBand, p: &PointLy, out: &mut Vec<f64>) {
        out.clear();
        out.extend(
            self.rows(band)
                .iter()
                .map(|r| r.expected * r.profile.density(p)),
        );
        if let Some(tail) = self.tail(band) {
            out.push(tail.density(p));
        }
    }

    /// The class `mark` picks for a candidate of `band` at local `p` under `bound`, or `None` if
    /// it is rejected: the running odds over `bound` in the table's order (plan 01's
    /// [`Mark::pick_weighted`]).
    ///
    /// # Panics
    ///
    /// In debug builds, if the weights sum past `bound`.
    #[must_use]
    pub fn pick(&self, band: MassBand, p: &PointLy, mark: Mark, bound: f64) -> Option<MemberClass> {
        let mut weights = Vec::with_capacity(self.rows(band).len() + 1);
        self.weights(band, p, &mut weights);
        let sum: f64 = weights.iter().fold(0.0, |s, &w| s + w);
        debug_assert!(
            sum <= bound,
            "band {band:?}'s density {sum} at {p:?} exceeds its bound {bound}"
        );
        if bound <= 0.0 {
            return None;
        }
        let rows = self.rows(band).len();
        mark.pick_weighted(&weights, bound).map(|i| {
            if i < rows {
                self.rows(band)[i].class
            } else {
                *self
                    .tail(band)
                    .expect("an index past the profiled classes is the tail's")
                    .class()
            }
        })
    }

    /// Every member's density at local `p`, over bands and classes, per cubic light-year: what
    /// plan 14 reads.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        let mut weights = Vec::new();
        MassBand::ALL
            .iter()
            .map(|&band| {
                self.weights(band, p, &mut weights);
                weights.iter().sum::<f64>()
            })
            .sum()
    }

    /// The members' mean system mass at local `p`, M☉: density-weighted over bands and classes;
    /// 0 where there are none.
    #[must_use]
    pub fn mean_member_mass(&self, p: &PointLy) -> f64 {
        let (mut n, mut m) = (0.0, 0.0);
        for band in MassBand::ALL {
            for r in self.rows(band) {
                let d = r.expected * r.profile.density(p);
                n += d;
                m += d * r.class.mean_mass.value();
            }
            if let Some(t) = self.tail(band) {
                let d = t.density(p);
                n += d;
                m += d * t.class.mean_mass.value();
            }
        }
        if n > 0.0 { m / n } else { 0.0 }
    }

    /// The profile of the `i`-th profiled class of `band`, for the member draws and the tests.
    #[must_use]
    pub fn profile(&self, band: MassBand, i: usize) -> Option<&ClassProfile> {
        self.rows(band).get(i).map(|r| &r.profile)
    }

    /// How far the members reach from the centre: the tidal radius, where every profile ends, or
    /// the tails' reach beyond it.
    #[must_use]
    pub fn extent(&self) -> LightYears {
        LightYears::new(
            self.tails
                .iter()
                .flatten()
                .fold(self.tidal, |r, t| r.max(t.reach)),
        )
    }

    /// The table with its tails cut at `reach` where they run farther: a tail's expected count is
    /// its line density times its length, so it scales with the length kept, and a tail with no
    /// length left is dropped (P09.T21: the tails run to the grid's reach).
    #[must_use]
    pub fn with_tails_to(mut self, reach: LightYears) -> Self {
        let reach = reach.value();
        for tail in &mut self.tails {
            *tail = tail.and_then(|t| {
                if reach >= t.reach {
                    Some(t)
                } else if reach > t.inner {
                    Some(TailClass {
                        expected: t.expected * (reach - t.inner) / (t.reach - t.inner),
                        reach,
                        ..t
                    })
                } else {
                    None
                }
            });
        }
        self
    }

    /// The steepest cusp among `band`'s profiled classes, if any has one.
    #[must_use]
    fn cusp_slope(&self, band: MassBand) -> Option<f64> {
        self.rows(band)
            .iter()
            .filter_map(|r| match r.profile.shape() {
                ProfileShape::Cusp { slope } => Some(slope),
                ProfileShape::Core { .. } | ProfileShape::Plummer { .. } => None,
            })
            .reduce(f64::max)
    }

    /// How `band`'s candidates are proposed in `cell` (P09.T21; [`CellProposal`]): radially under
    /// a cusp's envelope in a cell with a corner at the centre when the band has a cusp, and
    /// uniformly under [`cell_bound`](Self::cell_bound) everywhere else.
    #[must_use]
    pub fn proposal(&self, band: MassBand, cell: &LocalCell) -> CellProposal {
        match self.cusp_slope(band) {
            Some(slope) if cell.nearest_radius() <= 0.0 => {
                debug_assert!(
                    cell.min
                        .iter()
                        .all(|&m| m.total_cmp(&0.0).is_eq() || m.total_cmp(&-cell.edge).is_eq()),
                    "a cusp's cell must have a corner at the centre: {cell:?}"
                );
                let far = (3.0 * cell.edge * cell.edge).sqrt();
                let scale = far.max(CUSP_SOFTENING);
                let (centre, radius) = cell.bounding_sphere();
                let mut factor = 0.0;
                for r in self.rows(band) {
                    let d0 = r.profile.density_at_radius(0.0);
                    let sup = match r.profile.shape() {
                        ProfileShape::Cusp { slope: own } => {
                            d0 * math::powf(CUSP_SOFTENING, own) * math::powf(scale, slope - own)
                        }
                        ProfileShape::Core { .. } | ProfileShape::Plummer { .. } => {
                            d0 * math::powf(scale, slope)
                        }
                    };
                    factor += r.expected * sup;
                }
                if let Some(tail) = self.tail(band) {
                    factor += tail.bound(&centre, radius) * math::powf(scale, slope);
                }
                CellProposal::Cusp {
                    factor: factor * (1.0 + CUSP_MARGIN),
                    slope,
                }
            }
            Some(_) | None => CellProposal::Uniform {
                bound: self.cell_bound(band, cell),
            },
        }
    }

    /// The expected candidates of `band` in the owned `cell` of `grid` under its proposal
    /// (P09.T21).
    #[must_use]
    pub fn cell_candidates(&self, band: MassBand, grid: &NestedGrid, cell: NestedCell) -> f64 {
        let local = grid.local_cell(cell);
        self.proposal(band, &local).expected(&local)
    }

    /// The most candidates any band expects in an owned cell of `grid` where the bound can peak
    /// (P09.T21): on every level the cells beside the grid's three axes, where the profiles' bound
    /// is greatest at each radius, and the cells within one cell of the tails' line, where theirs
    /// is.
    #[must_use]
    pub fn peak_candidates(&self, grid: &NestedGrid) -> f64 {
        let n = grid.cells_per_axis();
        let beside = [n / 2 - 1, n / 2];
        let mut cells = std::collections::BTreeSet::new();
        for level in 0..grid.levels() {
            for along in 0..n {
                for a in beside {
                    for b in beside {
                        cells.extend(
                            [[along, a, b], [a, along, b], [a, b, along]]
                                .into_iter()
                                .filter_map(|c| grid.cell(level, c)),
                        );
                    }
                }
            }
        }
        if let Some(tail) = self.tails.iter().flatten().next() {
            // Every cell within one cell of a point of the line, in steps of a quarter cell.
            let axis = tail.axis;
            for level in grid.all_levels() {
                let edge = level.edge().value();
                let half = level.half_width().value();
                let steps = 8 * u32::from(n);
                for i in 0..=steps {
                    let along = -half + 2.0 * half * f64::from(i) / f64::from(steps);
                    let point = axis.map(|a| along * a);
                    let index = point.map(|c| (c / edge).floor() + f64::from(n / 2));
                    for offset in [-1.0, 0.0, 1.0]
                        .iter()
                        .flat_map(|&dx| [-1.0, 0.0, 1.0].map(move |dy| (dx, dy)))
                        .flat_map(|(dx, dy)| [-1.0, 0.0, 1.0].map(move |dz| [dx, dy, dz]))
                    {
                        let c = [0, 1, 2].map(|k| index[k] + offset[k]);
                        if c.iter().all(|&v| (0.0..f64::from(n)).contains(&v)) {
                            #[expect(
                                clippy::cast_possible_truncation,
                                clippy::cast_sign_loss,
                                reason = "whole numbers checked to lie in 0..n, n at most 64"
                            )]
                            let c = c.map(|v| v as u8);
                            cells.extend(grid.cell(level.level(), c));
                        }
                    }
                }
            }
        }
        let mut peak = 0.0_f64;
        for cell in cells {
            for band in MassBand::ALL {
                peak = peak.max(self.cell_candidates(band, grid, cell));
            }
        }
        peak
    }

    /// The width of the cluster's nested grid of [`FEATURE_GRID_CELLS`] cells and
    /// [`FEATURE_GRID_LEVELS`] levels (P09.T21): from the least power of two from 1 ⁄ 64 ly whose
    /// grid reaches the tidal radius, where every profile ends, doubled while the doubled grid still
    /// expects no more than [`CELL_CANDIDATE_TARGET`] candidates in any cell and band
    /// ([`peak_candidates`](Self::peak_candidates)),
    /// and until it reaches the tails' [`extent`](Self::extent). The tails are then cut at the
    /// grid's reach ([`with_tails_to`](Self::with_tails_to)).
    ///
    /// # Panics
    ///
    /// Never: a table's extent is finite, so some power of two up to 2³⁰ ly reaches it.
    #[must_use]
    pub fn grid_width(&self) -> LightYears {
        let reaches = |k: i32, r: f64| feature_grid(k).reach().value() >= r;
        let mut k = GRID_WIDTH_MIN_LOG2;
        while !reaches(k, self.tidal) {
            k += 1;
        }
        let extent = self.extent().value();
        while !reaches(k, extent)
            && self.peak_candidates(&feature_grid(k + 1)) <= CELL_CANDIDATE_TARGET
        {
            k += 1;
        }
        feature_grid(k).width()
    }

    /// The cluster's nested grid: [`grid_width`](Self::grid_width), 16 cells and eight levels.
    ///
    /// # Panics
    ///
    /// Never: the width is a power of two within the grid's range.
    #[must_use]
    pub fn grid(&self) -> NestedGrid {
        NestedGrid::new(self.grid_width(), FEATURE_GRID_CELLS, FEATURE_GRID_LEVELS)
            .expect("a grid width is a power of two")
    }
}

/// How a band's candidates are proposed in one nested cell (P09.T21).
///
/// Most cells take the nearest-corner bound and a uniform position. A core-collapsed cluster's
/// cusp, softened only at [`CUSP_SOFTENING`] = 10⁻³ ly, makes that bound absurd in the cells with a
/// corner at the centre: millions of candidates in a light-year cell against the 8,192 index. There
/// the candidates are proposed radially instead, from the envelope `B h(r)`, `h(r) = max(ε, r)^−γ`
/// with γ the band's steepest cusp, over the octant of the ball of radius `√3 e` that holds the
/// cell, and a candidate outside the cell is dropped: thinning with a non-uniform proposal, exact
/// by the same theorem (Design note 21 does the same for the catalogue's heights). `B` bounds every
/// class's density over `h` in the cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellProposal {
    /// Uniform in the cell under a constant bound, per cubic light-year.
    Uniform {
        /// The bound, per cubic light-year.
        bound: f64,
    },
    /// Radial under `factor × h(r)` in a cell with a corner at the centre.
    Cusp {
        /// `B`, per cubic light-year per unit of `h`.
        factor: f64,
        /// The envelope's slope γ.
        slope: f64,
    },
}

impl CellProposal {
    /// The expected candidates in `cell`.
    #[must_use]
    pub fn expected(&self, cell: &LocalCell) -> f64 {
        match *self {
            Self::Uniform { bound } => bound * cell.edge * cell.edge * cell.edge,
            Self::Cusp { factor, slope } => {
                let far = (3.0 * cell.edge * cell.edge).sqrt();
                factor * 0.5 * core::f64::consts::PI * envelope_mass(far, slope)
            }
        }
    }

    /// The candidate three uniforms `u` put in `cell` and the bound there, per cubic light-year, or
    /// `None` for a radial candidate that falls outside the cell.
    #[must_use]
    pub fn place(&self, cell: &LocalCell, uniforms: [f64; 3]) -> Option<(PointLy, f64)> {
        match *self {
            Self::Uniform { bound } => {
                let [x, y, z] = [0, 1, 2].map(|i| cell.min[i] + uniforms[i] * cell.edge);
                Some((PointLy::new(x, y, z), bound))
            }
            Self::Cusp { factor, slope } => {
                let far = (3.0 * cell.edge * cell.edge).sqrt();
                let radius = envelope_radius(uniforms[0] * envelope_mass(far, slope), slope);
                let cos_theta = uniforms[1];
                let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
                let (sin_phi, cos_phi) = math::sin_cos(0.5 * core::f64::consts::PI * uniforms[2]);
                let magnitude = [
                    radius * sin_theta * cos_phi,
                    radius * sin_theta * sin_phi,
                    radius * cos_theta,
                ];
                let mut point = [0.0; 3];
                for (axis, (&m, &lo)) in magnitude.iter().zip(&cell.min).enumerate() {
                    if m >= cell.edge {
                        return None;
                    }
                    point[axis] = if lo < 0.0 { -m } else { m };
                }
                let envelope = math::powf(radius.max(CUSP_SOFTENING), -slope);
                Some((
                    PointLy::new(point[0], point[1], point[2]),
                    factor * envelope,
                ))
            }
        }
    }
}

/// The relative margin of a cusp's envelope over its classes' densities, for the rounding of the
/// powers it is built from.
const CUSP_MARGIN: f64 = 1e-9;

/// `∫₀^r h(s) s² ds` for `h(s) = max(ε, s)^−γ`, γ < 3 (a cusp's slope is 1.6–2).
#[must_use]
fn envelope_mass(r: f64, slope: f64) -> f64 {
    debug_assert!(slope < 3.0, "an envelope of slope {slope}");
    let eps = CUSP_SOFTENING;
    let inner = math::powf(eps, -slope) * r.min(eps) * r.min(eps) * r.min(eps) / 3.0;
    if r <= eps {
        return inner;
    }
    let k = 3.0 - slope;
    inner + (math::powf(r, k) - math::powf(eps, k)) / k
}

/// The radius at which [`envelope_mass`] reaches `m`.
#[must_use]
fn envelope_radius(m: f64, slope: f64) -> f64 {
    let eps = CUSP_SOFTENING;
    let inner = envelope_mass(eps, slope);
    if m <= inner {
        return math::cbrt(3.0 * m * math::powf(eps, slope));
    }
    let k = 3.0 - slope;
    math::powf((m - inner) * k + math::powf(eps, k), 1.0 / k)
}

/// The expected candidates a cell and band of a cluster's grid is sized to stay under: 2,000
/// (brainstorm, "Dense features": "no cell expects more than a thousand or two members").
pub const CELL_CANDIDATE_TARGET: f64 = 2_000.0;

/// A catalogue feature's cells per axis in a nested level: 16 (brainstorm, "Dense features").
pub const FEATURE_GRID_CELLS: u8 = 16;

/// A catalogue feature's nested levels: eight (brainstorm, "Dense features").
pub const FEATURE_GRID_LEVELS: u8 = 8;

/// The least width a cluster's grid is tried at, as a power of two: 1 ⁄ 64 ly (P09.T21).
pub const GRID_WIDTH_MIN_LOG2: i32 = -6;

/// The catalogue features' grid of width `2^k` ly.
#[must_use]
fn feature_grid(k: i32) -> NestedGrid {
    let mut w = 1.0;
    for _ in 0..k.unsigned_abs() {
        w *= if k >= 0 { 2.0 } else { 0.5 };
    }
    NestedGrid::new(LightYears::new(w), FEATURE_GRID_CELLS, FEATURE_GRID_LEVELS)
        .expect("a width of 2^k ly from 2^-6 up is a power of two")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::testing::{M4, catalogue_parameters, named_cluster};
    use crate::galaxy::params::GalaxyParams;
    use crate::rng::Seed;

    #[test]
    fn the_envelope_s_radius_inverts_its_mass_on_both_sides_of_the_softening() {
        for slope in [1.6, 1.8, 2.0] {
            for r in [1e-4, 5e-4, 1e-3, 2e-3, 0.1, 3.0] {
                let back = envelope_radius(envelope_mass(r, slope), slope);
                assert!((back / r - 1.0).abs() < 1e-12, "{slope} {r}: {back}");
            }
        }
        // Inside ε the envelope is flat, so the mass is ε^−γ r³ ÷ 3.
        let m = envelope_mass(5e-4, 2.0);
        assert!((m / (1e6 * 1.25e-10 / 3.0) - 1.0).abs() < 1e-12, "{m}");
    }

    #[test]
    fn a_cusp_cell_s_candidates_stay_under_its_envelope_and_inside_it() {
        let cell = LocalCell {
            min: [-0.25, 0.0, -0.25],
            edge: 0.25,
        };
        let proposal = CellProposal::Cusp {
            factor: 2.0,
            slope: 1.8,
        };
        let mut kept = 0;
        for i in 0..1_000_u32 {
            let u = [0.37, 0.61, 0.83].map(|a| (a * f64::from(i + 1)).fract());
            if let Some((p, bound)) = proposal.place(&cell, u) {
                kept += 1;
                assert!(p.x <= 0.0 && p.y >= 0.0 && p.z <= 0.0, "{p:?}");
                assert!(p.x > -0.25 && p.y < 0.25 && p.z > -0.25, "{p:?}");
                let r = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
                let h = math::powf(r.max(CUSP_SOFTENING), -1.8);
                assert!((bound / (2.0 * h) - 1.0).abs() < 1e-12);
            }
        }
        // The cube is 6 ÷ (π 3^1.5) of the octant ball's volume, and more of the envelope's
        // weight, which gathers at the centre.
        assert!(kept > 380, "{kept} of 1,000 inside the cell");
    }

    fn table() -> (ClusterModel, MemberClassTable) {
        let galaxy =
            Galaxy::from_params(Seed::new(0x0921_0100), GalaxyParams::milky_way_like()).unwrap();
        let model = ClusterModel::new(&galaxy, &catalogue_parameters(named_cluster(M4)));
        let reach = LightYears::new(4.0 * model.tidal_radius().value());
        let table = MemberClassTable::new(&galaxy, &model, reach, [1.0, 0.0, 0.0]);
        (model, table)
    }

    #[test]
    fn tails_cut_short_keep_their_line_density_and_the_grid_holds_the_cluster() {
        let (model, table) = table();
        let r_t = model.tidal_radius().value();
        let band = MassBand::A;
        let full = table.tail(band).expect("M4 has a tail").expected();
        assert!((table.extent().value() / (4.0 * r_t) - 1.0).abs() < 1e-12);
        let half = table.clone().with_tails_to(LightYears::new(2.5 * r_t));
        let cut = half.tail(band).unwrap().expected();
        // Three tidal radii of tube kept of the full four.
        assert!((cut / full - 1.5 / 3.0).abs() < 1e-12, "{cut} of {full}");
        assert!(
            table
                .clone()
                .with_tails_to(LightYears::new(r_t))
                .tail(band)
                .is_none()
        );
        // A tail's member never lies beyond the reach of the centre.
        let tail = half.tail(band).unwrap();
        assert!(tail.density(&PointLy::new(2.4 * r_t, 0.9 * r_t, 0.0)) <= 0.0);
        assert!(tail.density(&PointLy::new(2.0 * r_t, 0.5 * r_t, 0.0)) > 0.0);
        // The grid reaches the tidal radius; it stopped doubling at the tails' extent or because
        // the doubled grid would expect too many candidates somewhere; and it doubled past the
        // tidal radius only while under the target.
        let grid = table.grid();
        let w = grid.width().value();
        let at = |width: f64| {
            NestedGrid::new(
                LightYears::new(width),
                FEATURE_GRID_CELLS,
                FEATURE_GRID_LEVELS,
            )
            .unwrap()
        };
        assert!(grid.reach().value() >= r_t);
        assert!(
            grid.reach().value() >= table.extent().value()
                || table.peak_candidates(&at(2.0 * w)) > CELL_CANDIDATE_TARGET
        );
        if at(0.5 * w).reach().value() >= r_t {
            assert!(table.peak_candidates(&grid) <= CELL_CANDIDATE_TARGET);
        }
    }
}
