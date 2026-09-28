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
//! light-year of each side.

use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::imf::MassBand;
use crate::math;
use crate::rng::Mark;
use crate::units::LightYears;

use super::super::cluster::ClusterModel;
use super::MemberClass;
use super::counts::{ClassCounts, class_counts};
use super::profile::ClassProfile;

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

    /// The tail's density at local `p`, per cubic light-year.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        let s = p.x * self.axis[0] + p.y * self.axis[1] + p.z * self.axis[2];
        let along = s.abs();
        if along < self.inner || along > self.reach {
            return 0.0;
        }
        let d2 = (p.x * p.x + p.y * p.y + p.z * p.z - s * s).max(0.0);
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
}
