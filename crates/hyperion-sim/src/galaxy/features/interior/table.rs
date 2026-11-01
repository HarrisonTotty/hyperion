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

use std::collections::BTreeMap;

use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::imf::MassBand;
use crate::math;
use crate::rng::Mark;
use crate::units::LightYears;

use super::super::cluster::ClusterModel;
use super::super::nested::{NestedCell, NestedGrid, NestedLevel};
use super::MemberClass;
use super::counts::{CANONICAL_SLOPE, ClassCount, ClassCounts, class_counts};
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

    /// The distance from the centre to the cell's farthest corner, ly.
    #[must_use]
    pub(crate) fn farthest_radius(&self) -> f64 {
        let mut sum = 0.0;
        for &lo in &self.min {
            let d = lo.abs().max((lo + self.edge).abs());
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

/// The bounds of a band's profiled classes already found at a cell's corner radius
/// ([`MemberClassTable::bound`], a pure function of the band and the radius), for the cells of one
/// sphere of one table: the census and the walk of a range query read one per band and cell, and
/// the nested grid's cells, whose corners lie at exact multiples of a power-of-two edge, repeat
/// their corner radii across the grid's mirrors and permutations, so a 50 ly query in a globular's
/// core finds most of its 60,000 census bounds here (P09.T10; a pod of 16 vCPU). A radius is
/// looked up by its value, so a bound is reused only for the very radius it was found at.
#[derive(Debug, Default)]
pub(crate) struct BoundMemo(BTreeMap<(usize, Radius), f64>);

#[cfg(test)]
impl BoundMemo {
    /// The bounds it holds.
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// A corner radius as [`BoundMemo`]'s key, ly: ordered and equal by `total_cmp`, so that equal
/// keys are the same value.
#[derive(Debug, Clone, Copy)]
struct Radius(f64);

impl PartialEq for Radius {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Radius {}

impl PartialOrd for Radius {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Radius {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
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

    /// How far it reaches from the centre along its axis, ly.
    #[must_use]
    pub const fn reach(&self) -> f64 {
        self.reach
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
        self.bound_across(centre, radius, &mut Across::default())
    }

    /// [`bound`](Self::bound), bit for bit, with what it reads of the sphere shared (P09.T40.a).
    ///
    /// The sphere's place along the axis and the Gaussian across it come from `across`, filled by
    /// the first tail that reads them: every band's tail shares its axis and width, and so both.
    /// `across` serves one sphere.
    #[must_use]
    fn bound_across(&self, centre: &PointLy, radius: f64, across: &mut Across) -> f64 {
        let same = |a: f64, b: f64| a.total_cmp(&b).is_eq();
        let known = across.line.is_some_and(|(axis, width)| {
            same(width, self.width) && axis.iter().zip(self.axis).all(|(&a, b)| same(a, b))
        });
        if !known {
            *across = Across {
                line: Some((self.axis, self.width)),
                along: centre.x * self.axis[0] + centre.y * self.axis[1] + centre.z * self.axis[2],
                gauss: None,
            };
        }
        let s = across.along;
        let (lo, hi) = (s.abs() - radius, s.abs() + radius);
        if hi < self.inner || lo > self.reach {
            return 0.0;
        }
        let gauss = *across.gauss.get_or_insert_with(|| {
            let d2 =
                (centre.x * centre.x + centre.y * centre.y + centre.z * centre.z - s * s).max(0.0);
            let d = (d2.sqrt() - radius).max(0.0);
            math::exp(-0.5 * d * d / (self.width * self.width))
        });
        self.axis_density() * gauss * (1.0 + 1e-12)
    }
}

/// What [`TailClass::bound`] reads of one sphere for a tail's line, shared by the tails of one
/// axis and width: the sphere's centre along the axis and, once a tail's span meets it, the
/// Gaussian across at the sphere's nearest approach (P09.T40.a).
#[derive(Debug, Clone, Copy, Default)]
struct Across {
    /// The axis and width it was found for, or `None` before any.
    line: Option<([f64; 3], f64)>,
    along: f64,
    gauss: Option<f64>,
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
            t.filter(|t| t.expected > 0.0 && t.length.value() > 0.0)
                .map(|t| TailClass {
                    class: t.class,
                    expected: t.expected,
                    axis,
                    inner: tidal,
                    reach: (tidal + t.length.value()).min(reach.value()),
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

    /// The table of `classes` for a feature with no cluster model: plan 10's dwarf cores
    /// (P10.T7.a).
    ///
    /// Every class is profiled inside `tidal` and flattened along the grid's z by `flattening` (1
    /// for a sphere), with no tails, no depletion and no runaways. Classes of no expected members
    /// are left out, as in [`from_counts`](Self::from_counts); the rest keep their order within
    /// each band.
    ///
    /// # Panics
    ///
    /// If `tidal` is not positive and finite or `flattening` is not in (0, 1], and as
    /// [`ClassProfile::new`] if a shape's scale is not.
    #[must_use]
    pub fn from_classes(classes: &[ClassCount], tidal: LightYears, flattening: f64) -> Self {
        let tidal = tidal.value();
        assert!(
            tidal > 0.0 && tidal.is_finite(),
            "a class table inside {tidal} ly"
        );
        assert!(
            flattening > 0.0 && flattening <= 1.0,
            "a flattening of {flattening}"
        );
        let mut bands: [Vec<Row>; 5] = Default::default();
        for c in classes {
            if c.expected > 0.0 {
                bands[c.class.band.index()].push(Row {
                    class: c.class,
                    expected: c.expected,
                    profile: ClassProfile::new(c.shape, tidal, flattening),
                });
            }
        }
        Self {
            bands,
            tails: [None; 5],
            depleted_slope: CANONICAL_SLOPE,
            runaway_share: 0.0,
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

    /// The bytes the table owns on the heap: each band's rows, by their buffers' capacities, and
    /// each row's profile ([`ClassProfile::heap_bytes`]). A tail owns nothing on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.bands
            .iter()
            .map(|rows| {
                rows.capacity() * size_of::<Row>()
                    + rows
                        .iter()
                        .map(|row| row.profile.heap_bytes())
                        .sum::<usize>()
            })
            .sum()
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
        self.cell_bound_with(band, cell, None)
    }

    /// [`cell_bound`](Self::cell_bound), bit for bit, its profiled classes' bound read from
    /// `memo` where it holds the band's at the cell's corner radius, and kept there otherwise.
    #[must_use]
    fn cell_bound_with(
        &self,
        band: MassBand,
        cell: &LocalCell,
        memo: Option<&mut BoundMemo>,
    ) -> f64 {
        let (centre, radius) = cell.bounding_sphere();
        let nearest = cell.nearest_radius();
        let profiled = match memo {
            Some(memo) => *memo
                .0
                .entry((band.index(), Radius(nearest)))
                .or_insert_with(|| self.bound(band, nearest)),
            None => self.bound(band, nearest),
        };
        (profiled + self.tail(band).map_or(0.0, |t| t.bound(&centre, radius))) * (1.0 + 1e-12)
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
        self.proposal_with(band, cell, None)
    }

    /// [`proposal`](Self::proposal), bit for bit, a uniform cell's profiled bound read from and
    /// kept in `memo` ([`BoundMemo`]).
    #[must_use]
    pub(crate) fn proposal_with(
        &self,
        band: MassBand,
        cell: &LocalCell,
        memo: Option<&mut BoundMemo>,
    ) -> CellProposal {
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
                bound: self.cell_bound_with(band, cell, memo),
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
        let mut marks = PeakCells::new(*grid);
        grid.all_levels().fold(0.0_f64, |peak, level| {
            peak.max(self.level_peak(*grid, level, Admit::Owned, &mut marks, f64::INFINITY))
        })
    }

    /// The most candidates any band expects in the cells of `level` of `grid` that
    /// [`peak_candidates`](Self::peak_candidates) reads and `admit` admits.
    ///
    /// Each cell is read once, and the most so far is returned as soon as it passes `stop`.
    /// `marks` is scratch.
    #[must_use]
    fn level_peak(
        &self,
        grid: NestedGrid,
        level: NestedLevel,
        admit: Admit,
        marks: &mut PeakCells,
        stop: f64,
    ) -> f64 {
        marks.clear();
        let n = grid.cells_per_axis();
        let beside = [n / 2 - 1, n / 2];
        for along in 0..n {
            for a in beside {
                for b in beside {
                    for c in [[along, a, b], [a, along, b], [a, b, along]] {
                        marks.mark(c);
                    }
                }
            }
        }
        if let Some(tail) = self.tails.iter().flatten().next() {
            // Every cell within one cell of a point of the line, in steps of a quarter cell.
            let axis = tail.axis;
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
                        marks.mark(c);
                    }
                }
            }
        }
        let mut peak = 0.0_f64;
        for c in marks.cells() {
            let Some(cell) = grid.cell(level.level(), c) else {
                continue;
            };
            if admit == Admit::InnerHalf && !c.iter().all(|&v| grid.is_inner(v)) {
                continue;
            }
            peak = peak.max(self.cell_peak(&grid.local_cell(cell)));
            if peak > stop {
                break;
            }
        }
        peak
    }

    /// The most candidates any band expects in `cell` (P09.T40.a).
    ///
    /// It is the greatest over the bands of [`cell_candidates`](Self::cell_candidates), bit for
    /// bit, with the cell's nearest radius and bounding sphere and its tails' Gaussian found once
    /// for every band.
    #[must_use]
    fn cell_peak(&self, cell: &LocalCell) -> f64 {
        let nearest = cell.nearest_radius();
        let (centre, radius) = cell.bounding_sphere();
        let mut across = Across::default();
        MassBand::ALL.iter().fold(0.0_f64, |peak, &band| {
            let expected = match self.cusp_slope(band) {
                Some(_) if nearest <= 0.0 => self.proposal(band, cell).expected(cell),
                // `cell_bound` over the cell, as `proposal` takes it.
                Some(_) | None => {
                    let tail = self
                        .tail(band)
                        .map_or(0.0, |t| t.bound_across(&centre, radius, &mut across));
                    let bound = (self.bound(band, nearest) + tail) * (1.0 + 1e-12);
                    CellProposal::Uniform { bound }.expected(cell)
                }
            };
            peak.max(expected)
        })
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
        self.grid_width_from(GRID_WIDTH_MIN_LOG2)
    }

    /// [`grid_width`](Self::grid_width) tried from `2^floor_log2` ly up instead of
    /// [`GRID_WIDTH_MIN_LOG2`]'s 1 ⁄ 64 ly: plan 10's dwarf cores start at 16 ly (P10.T7.a).
    ///
    /// # Panics
    ///
    /// If `floor_log2` is outside the nested grid's range of widths, 2⁻³⁰ to 2³⁰ ly; never
    /// otherwise, as [`grid_width`](Self::grid_width) says.
    #[must_use]
    pub(crate) fn grid_width_from(&self, floor_log2: i32) -> LightYears {
        let reaches = |k: i32, r: f64| feature_grid(k).reach().value() >= r;
        let mut k = floor_log2;
        while !reaches(k, self.tidal) {
            k += 1;
        }
        let extent = self.extent().value();
        let mut marks = PeakCells::new(feature_grid(k));
        let mut read = Doubling::Whole;
        while !reaches(k, extent) && self.doubled_holds(feature_grid(k + 1), read, &mut marks) {
            k += 1;
            read = Doubling::Added;
        }
        feature_grid(k).width()
    }

    /// Whether [`peak_candidates`](Self::peak_candidates) of `grid` is at most
    /// [`CELL_CANDIDATE_TARGET`], stopping at the first cell over it (P09.T40.a).
    ///
    /// For [`Doubling::Added`], the grid half as wide is known to hold, and only the cells it
    /// lacks are read: the grid's level `j` is that grid's level `j + 1`, cell for cell, with the
    /// same cells read and owned, but for level 0's inner half, which that grid's level 1 leaves to
    /// its level 0, and the outermost level, which it has not got. A cell's expected candidates are
    /// a function of its box alone, so the answer is the whole grid's, bit for bit.
    ///
    /// A level whose [`level_ceiling`](Self::level_ceiling) is under the target holds without its
    /// cells being read, level 0 once its eight cells with a corner at the centre, which the
    /// ceiling leaves out, are read and hold. Debug builds read such a level too, and check that it
    /// holds.
    #[must_use]
    fn doubled_holds(&self, grid: NestedGrid, read: Doubling, marks: &mut PeakCells) -> bool {
        let target = CELL_CANDIDATE_TARGET;
        let outermost = grid.levels() - 1;
        grid.all_levels().all(|level| {
            let admit = match (read, level.level()) {
                (Doubling::Whole, _) => Admit::Owned,
                (Doubling::Added, 0) if outermost > 0 => Admit::InnerHalf,
                (Doubling::Added, j) if j == outermost => Admit::Owned,
                (Doubling::Added, _) => return true,
            };
            let centre_holds = || {
                let n = grid.cells_per_axis();
                let beside = [n / 2 - 1, n / 2];
                beside.iter().all(|&x| {
                    beside.iter().all(|&y| {
                        beside.iter().all(|&z| {
                            grid.cell(0, [x, y, z])
                                .is_none_or(|c| self.cell_peak(&grid.local_cell(c)) <= target)
                        })
                    })
                })
            };
            if self.level_ceiling(grid, level) <= target && (level.level() > 0 || centre_holds()) {
                debug_assert!(
                    self.level_peak(grid, level, admit, marks, f64::INFINITY) <= target,
                    "level {} of the {} ly grid passes its ceiling but not the target",
                    level.level(),
                    grid.width().value()
                );
                return true;
            }
            self.level_peak(grid, level, admit, marks, target) <= target
        })
    }

    /// A ceiling on what [`cell_peak`](Self::cell_peak) gives for any owned cell of `level` of
    /// `grid` but, on level 0, the eight with a corner at the centre, where a cusp's envelope
    /// peaks (P09.T40.a).
    ///
    /// A higher level owns no cell nearer the centre than a quarter of its cells per axis, and
    /// those cells of level 0 none nearer than one edge, each exactly so in floating point. Every
    /// profile is a power of `1 + (r ÷ a)²` or of `r² + ε²` with an exponent that is not positive
    /// times a taper falling with `r`, so it never rises with the radius, as computed, by more than
    /// `libm`'s last bits, which [`CEILING_MARGIN`] covers; a tail's Gaussian across is at most 1.
    /// So each band's cell bound is at most its profiles' bound at that radius plus its tail's axis
    /// density, and the cell's candidates that times its volume, every step a monotone rounding.
    /// A table with a profile whose exponents do not show it falling
    /// ([`ProfileShape::never_rises`]) has an infinite ceiling, and its levels are read.
    #[must_use]
    fn level_ceiling(&self, grid: NestedGrid, level: NestedLevel) -> f64 {
        if !self
            .bands
            .iter()
            .flatten()
            .all(|row| row.profile.shape().never_rises())
        {
            return f64::INFINITY;
        }
        let edge = level.edge().value();
        let nearest = if level.level() == 0 {
            edge
        } else {
            f64::from(grid.cells_per_axis() / 4) * edge
        };
        MassBand::ALL.iter().fold(0.0_f64, |ceiling, &band| {
            let tail = self
                .tail(band)
                .map_or(0.0, |t| t.axis_density() * (1.0 + 1e-12));
            let bound = (self.bound(band, nearest) * (1.0 + CEILING_MARGIN) + tail) * (1.0 + 1e-12);
            let expected = CellProposal::Uniform { bound }.expected(&LocalCell {
                min: [0.0; 3],
                edge,
            });
            ceiling.max(expected * (1.0 + CEILING_MARGIN))
        })
    }

    /// The cluster's nested grid: [`grid_width`](Self::grid_width), 16 cells and eight levels.
    ///
    /// # Panics
    ///
    /// Never: the width is a power of two within the grid's range.
    #[must_use]
    pub fn grid(&self) -> NestedGrid {
        self.grid_from(GRID_WIDTH_MIN_LOG2)
    }

    /// A bound on the reach of [`grid`](Self::grid) for a table whose tidal radius is `tidal` and
    /// whose tails run at most `tails` from the centre, without the table: the reach of the least
    /// grid from 1 ⁄ 64 ly that reaches both. [`grid_width`](Self::grid_width) stops doubling once
    /// its grid reaches the table's [`extent`](Self::extent), which is the larger of the two, so
    /// its grid is never wider (P09.T40.a).
    ///
    /// # Panics
    ///
    /// If the larger is not a finite number, or lies beyond the reach of the widest grid of the
    /// nested grid's range of widths (2³⁰ ly cells).
    #[must_use]
    pub(crate) fn grid_reach_bound(tidal: LightYears, tails: LightYears) -> LightYears {
        let far = tidal.value().max(tails.value());
        assert!(far.is_finite(), "a cluster reaching {far} ly");
        let mut k = GRID_WIDTH_MIN_LOG2;
        while feature_grid(k).reach().value() < far {
            k += 1;
        }
        feature_grid(k).reach()
    }

    /// [`grid`](Self::grid) at [`grid_width_from`](Self::grid_width_from)`(floor_log2)`.
    ///
    /// # Panics
    ///
    /// As [`grid_width_from`](Self::grid_width_from).
    #[must_use]
    pub(crate) fn grid_from(&self, floor_log2: i32) -> NestedGrid {
        NestedGrid::new(
            self.grid_width_from(floor_log2),
            FEATURE_GRID_CELLS,
            FEATURE_GRID_LEVELS,
        )
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

/// The relative margin of [`MemberClassTable::level_ceiling`] over a profile's last-bit rises
/// and the rounding of its own products: 10⁻⁹, against `libm`'s few units in 10¹⁶.
const CEILING_MARGIN: f64 = 1e-9;

/// Which cells of a doubled grid [`MemberClassTable::doubled_holds`] reads (P09.T40.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doubling {
    /// Every one: nothing is known of the grid half as wide.
    Whole,
    /// Those the grid half as wide, known to hold, lacks.
    Added,
}

/// Which owned cells of a level [`MemberClassTable::level_peak`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admit {
    /// Every one.
    Owned,
    /// Those of the level's inner half, every coordinate inner.
    InnerHalf,
}

/// One level's cells, as a bitmap over its `n³` cells, x-major: each cell where a table's bound
/// can peak once, however many times the axes and the tail's line visit it (P09.T40.a).
#[derive(Debug, Clone)]
struct PeakCells {
    n: usize,
    words: Vec<u64>,
}

impl PeakCells {
    /// An empty bitmap for a level of `grid`.
    #[must_use]
    fn new(grid: NestedGrid) -> Self {
        let n = usize::from(grid.cells_per_axis());
        Self {
            n,
            words: vec![0; (n * n * n).div_ceil(64)],
        }
    }

    fn clear(&mut self) {
        self.words.fill(0);
    }

    /// Marks cell `c`, each coordinate below `n`.
    fn mark(&mut self, c: [u8; 3]) {
        let i = (usize::from(c[0]) * self.n + usize::from(c[1])) * self.n + usize::from(c[2]);
        self.words[i / 64] |= 1 << (i % 64);
    }

    /// The marked cells, x-major.
    fn cells(&self) -> impl Iterator<Item = [u8; 3]> + '_ {
        let n = self.n;
        self.words.iter().enumerate().flat_map(move |(w, &word)| {
            (0..64).filter(move |b| word & (1 << b) != 0).map(move |b| {
                let i = w * 64 + b;
                [i / (n * n), (i / n) % n, i % n]
                    .map(|v| u8::try_from(v).expect("a cell coordinate is below 64"))
            })
        })
    }
}

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
///
/// # Panics
///
/// If `k` is outside the nested grid's range of widths, 2⁻³⁰ to 2³⁰ ly.
#[must_use]
fn feature_grid(k: i32) -> NestedGrid {
    let mut w = 1.0;
    for _ in 0..k.unsigned_abs() {
        w *= if k >= 0 { 2.0 } else { 0.5 };
    }
    NestedGrid::new(LightYears::new(w), FEATURE_GRID_CELLS, FEATURE_GRID_LEVELS)
        .expect("a width of 2^k ly within the nested grid's range is a power of two")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::testing::{M4, catalogue_parameters, named_cluster};
    use crate::galaxy::params::GalaxyParams;
    use crate::rng::Seed;

    /// A cell's farthest radius is the largest distance of its eight corners, on either side of
    /// the centre and across it.
    #[test]
    fn a_cell_s_farthest_radius_is_its_farthest_corner_s() {
        for (min, edge) in [
            ([0.25, 0.5, -1.0], 0.25),
            ([-0.25, -0.25, -0.25], 0.25),
            ([-2.0, 1.0, 0.0], 1.0),
            ([0.0, 0.0, 0.0], 0.0625),
        ] {
            let cell = LocalCell { min, edge };
            let mut far: f64 = 0.0;
            for corner in 0..8 {
                let c = [0, 1, 2].map(|a| min[a] + if corner >> a & 1 == 1 { edge } else { 0.0 });
                far = far.max((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt());
            }
            assert!((cell.farthest_radius() - far).abs() < 1e-15, "{min:?}");
            assert!(cell.nearest_radius() <= cell.farthest_radius());
        }
    }

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

    /// The peak as the search had it before P09.T40.a: every cell into one ordered set, then every
    /// band of every cell through `cell_candidates`.
    fn peak_as_built(table: &MemberClassTable, grid: NestedGrid) -> f64 {
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
        if let Some(tail) = table.tails.iter().flatten().next() {
            let axis = tail.axis;
            for level in grid.all_levels() {
                let edge = level.edge().value();
                let half = level.half_width().value();
                let steps = 8 * u32::from(n);
                for i in 0..=steps {
                    let along = -half + 2.0 * half * f64::from(i) / f64::from(steps);
                    let point = axis.map(|a| along * a);
                    let index = point.map(|c| (c / edge).floor() + f64::from(n / 2));
                    for dx in [-1.0, 0.0, 1.0] {
                        for dy in [-1.0, 0.0, 1.0] {
                            for dz in [-1.0, 0.0, 1.0] {
                                let c = [index[0] + dx, index[1] + dy, index[2] + dz];
                                if c.iter().all(|&v| (0.0..f64::from(n)).contains(&v)) {
                                    #[expect(
                                        clippy::cast_possible_truncation,
                                        clippy::cast_sign_loss,
                                        reason = "whole numbers checked to lie in 0..n"
                                    )]
                                    let c = c.map(|v| v as u8);
                                    cells.extend(grid.cell(level.level(), c));
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut peak = 0.0_f64;
        for cell in cells {
            for band in MassBand::ALL {
                peak = peak.max(table.cell_candidates(band, &grid, cell));
            }
        }
        peak
    }

    /// The width as the search had it before P09.T40.a: every doubled grid's whole peak.
    fn width_as_built(table: &MemberClassTable, floor_log2: i32) -> f64 {
        let reaches = |k: i32, r: f64| feature_grid(k).reach().value() >= r;
        let mut k = floor_log2;
        while !reaches(k, table.tidal) {
            k += 1;
        }
        while !reaches(k, table.extent().value())
            && peak_as_built(table, feature_grid(k + 1)) <= CELL_CANDIDATE_TARGET
        {
            k += 1;
        }
        feature_grid(k).width().value()
    }

    /// The peak read level by level from a bitmap, its tails' Gaussian shared, and the width
    /// searched from the cells each doubling adds, are the whole search's, bit for bit: for the
    /// named globulars with short and long tails along two axes, and a dwarf core of classes alone
    /// (P09.T40.a).
    #[test]
    fn the_width_search_is_the_whole_grids_search_bit_for_bit() {
        use crate::galaxy::features::testing::{M15, OMEGA_CEN, PAL_5, TUC_47};
        use hyperion_testkit::float::assert_same_bits;
        let galaxy =
            Galaxy::from_params(Seed::new(0x0921_0100), GalaxyParams::milky_way_like()).unwrap();
        let mut tables = Vec::new();
        for name in [M4, TUC_47, PAL_5, OMEGA_CEN, M15] {
            let model = ClusterModel::new(&galaxy, &catalogue_parameters(named_cluster(name)));
            for (radii, axis) in [(4.0, [1.0, 0.0, 0.0]), (40.0, [0.3, -0.8, 0.5])] {
                let reach = LightYears::new(radii * model.tidal_radius().value());
                tables.push(MemberClassTable::new(&galaxy, &model, reach, axis));
            }
        }
        let classes: Vec<ClassCount> = tables[0]
            .bands
            .iter()
            .flatten()
            .map(|row| ClassCount {
                class: row.class,
                expected: row.expected,
                shape: row.profile.shape(),
            })
            .collect();
        tables.push(MemberClassTable::from_classes(
            &classes,
            LightYears::new(tables[0].tidal),
            0.67,
        ));
        let mut doublings = 0;
        for table in &tables {
            let width = table.grid_width().value();
            assert_same_bits(width, width_as_built(table, GRID_WIDTH_MIN_LOG2));
            assert_same_bits(table.grid_width_from(2).value(), width_as_built(table, 2));
            let mut k = GRID_WIDTH_MIN_LOG2;
            while feature_grid(k).reach().value() < table.tidal {
                k += 1;
            }
            // Every grid the search may read, and the one past it.
            while feature_grid(k).width().value() <= 2.0 * width {
                let grid = feature_grid(k);
                assert_same_bits(table.peak_candidates(&grid), peak_as_built(table, grid));
                doublings += 1;
                k += 1;
            }
        }
        assert!(doublings > 30, "{doublings} grids compared");
    }

    /// The search's width is the whole search's for every cluster within 3,000 ly of the
    /// Sun-like point and the first hundred globulars of the walk, so that the levels its ceiling
    /// passes unread are never ones the whole search would refuse (P09.T40.a).
    #[test]
    #[ignore = "slow: some hundreds of cluster models and whole width searches"]
    fn every_cluster_near_the_sun_and_a_hundred_globulars_keep_the_whole_search_s_width() {
        use crate::coords::GalacticPosition;
        use crate::galaxy::features::FeatureProcess;
        use crate::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache, bulk_velocity};
        use hyperion_testkit::float::assert_same_bits;
        let galaxy = Galaxy::new(Seed::new(0x4d2)).with_full_potential();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let near = FeatureCatalogue::near(&galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache);
        let globulars = FeatureCatalogue::walk_process(&galaxy, FeatureProcess::Globular).take(100);
        let (mut clusters, mut by_target) = (0, 0);
        for feature in near.chain(globulars) {
            let (Some(model), Some(bulk)) = (
                ClusterModel::from_record(&galaxy, &feature),
                bulk_velocity(&galaxy, &feature),
            ) else {
                continue;
            };
            let table =
                MemberClassTable::new(&galaxy, &model, feature.reach(), bulk.metres_per_second());
            let width = table.grid_width().value();
            assert_same_bits(width, width_as_built(&table, GRID_WIDTH_MIN_LOG2));
            clusters += 1;
            if feature_grid_reach(width) < table.extent().value() {
                by_target += 1;
            }
        }
        println!("{clusters} clusters, {by_target} stopped by the target");
        assert!(clusters > 100 && by_target > 0, "{clusters} {by_target}");
    }

    /// The reach of the catalogue's grid `width` ly wide.
    fn feature_grid_reach(width: f64) -> f64 {
        NestedGrid::new(
            LightYears::new(width),
            FEATURE_GRID_CELLS,
            FEATURE_GRID_LEVELS,
        )
        .unwrap()
        .reach()
        .value()
    }

    /// The floor argument leaves a cluster's grid as it was (P10.T7.a), and a floor above the
    /// rule's width is the width, since the doubling stops at the tails' extent or the target.
    #[test]
    fn a_grid_tried_from_a_floor_is_the_rule_s_from_there() {
        let (_, table) = table();
        let w = table.grid_width();
        assert!(
            table
                .grid_width_from(GRID_WIDTH_MIN_LOG2)
                .value()
                .total_cmp(&w.value())
                .is_eq()
        );
        assert_eq!(table.grid_from(GRID_WIDTH_MIN_LOG2), table.grid());
        let above = table.grid_width_from(12).value();
        assert!(above >= 4_096.0 && above >= w.value(), "{above}");
        // A table of classes alone: no tails, so the grid is the least that holds the tidal radius.
        let classes = [ClassCount {
            class: MemberClass {
                kind: super::super::ClassKind::Living,
                band: MassBand::A,
                generation: super::super::Generation::First,
                multiplicity: super::super::Multiplicity::Single,
                initial_mass_range: (
                    crate::units::SolarMasses::new(0.08),
                    crate::units::SolarMasses::new(0.5),
                ),
                mean_mass: crate::units::SolarMasses::new(0.2),
            },
            expected: 1e6,
            shape: ProfileShape::Plummer { scale: 900.0 },
        }];
        let core = MemberClassTable::from_classes(&classes, LightYears::new(5_000.0), 0.67);
        assert!(MassBand::ALL.iter().all(|&b| core.tail(b).is_none()));
        assert!((core.expected(MassBand::A) - 1e6).abs() < 1e-6);
        assert!((core.extent().value() - 5_000.0).abs() < 1e-12);
        assert!((core.grid_width_from(4).value() - 8.0 * 2.0).abs() < 1e-12);
        assert!((core.grid_width_from(-6).value() - 8.0).abs() < 1e-12);
    }

    #[test]
    fn tails_cut_short_keep_their_line_density_and_the_grid_holds_the_cluster() {
        let (model, table) = table();
        let r_t = model.tidal_radius().value();
        let band = MassBand::A;
        let tail = *table.tail(band).expect("M4 has a tail");
        let (full, end) = (tail.expected(), tail.reach());
        assert!((table.extent().value() / end.max(r_t) - 1.0).abs() < 1e-12);
        let middle = f64::midpoint(r_t, end);
        let half = table.clone().with_tails_to(LightYears::new(middle));
        let cut = half.tail(band).unwrap().expected();
        // Half the tube kept.
        assert!((cut / full - 0.5).abs() < 1e-12, "{cut} of {full}");
        assert!(
            table
                .clone()
                .with_tails_to(LightYears::new(r_t))
                .tail(band)
                .is_none()
        );
        // A tail's member never lies beyond the reach of the centre.
        let tail = half.tail(band).unwrap();
        let s = 0.9 * middle;
        assert!(
            tail.density(&PointLy::new(
                s,
                (middle * middle - s * s).sqrt() * 1.01,
                0.0
            )) <= 0.0
        );
        assert!(tail.density(&PointLy::new(s, 0.1 * r_t, 0.0)) > 0.0);
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
