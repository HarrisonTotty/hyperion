//! The cumulative luminosity function: per system of each density component and layer, the V light
//! of the stars fainter than an absolute magnitude and the number brighter (rendering plan R06,
//! Design note 7).
//!
//! The band (Design note 15) reads the light of every star the census did not list, and the caps
//! (Design note 9) read how many stars brighter than the cut a layer holds beyond a radius; both
//! are the density field times these tables. A table is a quadrature over the mass function and the
//! component's ages, the one [`mean_present_mass`](crate::galaxy::fates::mean_present_mass) does,
//! with the present mass replaced by the V light:
//!
//! - **Masses.** Gauss–Legendre panels in ln m no wider than
//!   [`MAX_PANEL_LN_MASS`](crate::galaxy::fates::MAX_PANEL_LN_MASS), a set for each \[Fe/H\] node,
//!   a [`TablesPlan`]'s [`Stage`] (R06.T5.e, decided 2026-10-05, `decision-r06-t5e-gate-2.md`).
//!   Their edges are the mass function's and the fates' breaks, each layer's band edges and their
//!   multiples by the companion laws' mass-ratio kinks, and the masses at which the node's tracks
//!   (the median draws) leave a living phase, as the track labels its segments, or die, at an age
//!   edge of any of the galaxy's components that read the node, whatever components a build asks
//!   for. An old population's giant branch, horizontal branch and AGB lie within some hundredths
//!   of its turnoff in ln m, and its light changes character at each phase end: left inside
//!   panels, as when the edges stood at the fates' fitted lifetimes, that structure put 16 nodes a
//!   panel 0.8% off in an old halo component's light and 1.1% in its colour against 32, and 4
//!   nodes 6.3% and 7.7% off against 16. The masses are found by
//!   a scan over the panels of the other edges and a fixed bisection, some 3 CPU-s for the Milky
//!   Way's 22 stages. Each panel takes a Gauss–Legendre order scaled with its width: 4 nodes on a
//!   panel at most a quarter of `MAX_PANEL_LN_MASS` wide, 8 on one at most half and 16 on a wider
//!   one, so never fewer than 32 nodes per unit of ln m. The Milky Way's stages take 138 to 188
//!   panels, all within a quarter, so 552 to 752 nodes. The primaries of a layer are its band's
//!   nodes, weighted by the mass function. The companions of a layer's primaries are spread over
//!   the same nodes: node j takes H(b<sub>j+1</sub>) − H(b<sub>j</sub>), with H(c) the number of
//!   companions below c per primary of the layer ([`StellarFates::mean_companions`] and the
//!   mass-ratio distribution, as [`CompanionMasses`](crate::galaxy::fates::CompanionMasses)
//!   integrates them, over the panels of the stages' shared edges) and the b<sub>j</sub> a panel's
//!   ends and the midpoints between its consecutive nodes.
//! - **Metallicity.** Each component's \[Fe/H\] distribution, read at the solar circle
//!   ([`SOLAR_RADIUS_LENGTHS`] of the galaxy's thin-disc scale lengths) and at its mean age, is
//!   taken at three-point Gauss–Hermite nodes: the mean and the mean ± √3 σ, weighted 2/3, 1/6 and
//!   1/6, since a star's V light is convex in \[Fe/H\]. A node beyond the tracks' clamp (Z =
//!   10⁻⁴–0.03, \[Fe/H\] −2.30 to +0.18) is read at the clamp, which the tracks would read anyway.
//!   A disc's radial gradient is not followed: its tables are the solar circle's everywhere.
//! - **Ages.** Each node's track ([`Track::to_age`], at each metallicity node and the median draws)
//!   is cut at every phase segment's ends and knots and
//!   into [`SAMPLES_PER_PHASE`] (32) equal parts of each phase, and the star's V at each part's middle
//!   is held over the part, weighted by the part's share of the component's born systems. A short
//!   bright phase, a post-AGB crossing or a blue loop, therefore counts for its duration and is
//!   never missed between two samples. An object below 0.1 M☉ follows plan 06's cooling fits,
//!   sampled evenly in log age.
//! - **Time.** The ages are taken at the emitted time. One table set serves a galaxy's whole clock
//!   window: it is built at [`REFERENCE_TIME`], +H, and holds snapshots at that time less each of
//!   [`EMITTED_AGO_YEARS`], between which [`LuminosityFunction`] interpolates linearly in the
//!   light's age. A query at time t reads light of age a at the age
//!   [`LuminosityTables::age_for`] gives, a + (t<sub>ref</sub> − t) (decided 2026-10-03).
//!   Within ±H a population's light changes by about 10⁻⁵, so one reference time costs nothing in
//!   accuracy.
//! - **Darkness.** A protostar and a white dwarf are dark ([`super::photometry`], asks A3 and A4),
//!   as are neutron stars, black holes and brown dwarfs cooler than plan 06's photometry reaches.
//!
//! The brown dwarfs' layer holds plan 13's objects of 0.0124–0.08 M☉, single, from the mass
//! function's substellar branch, per object; the rogue planets' layer is dark.
//!
//! Every star is evolved alone, and pair evolution enters as a correction (R06.T5.d, decided
//! 2026-10-03): [`super::binary_light`]'s fitted differences, pair-evolved less single-evolved,
//! added to layers C, D and E's light, colour and counts per snapshot, the counts taking only the
//! increase so that the caps stay conservative ([`LuminosityFunction::pair_light`]). A galaxy whose
//! mass function is not the default takes none.
//!
//! **Accuracy.** The tables are to be read as every reader reads them: summed over components and
//! layers, by density times each layer's share. The band and the limit map sum the light fainter
//! than a limit that moves with distance; the caps sum one layer's counts over its components and
//! 768 rays; T5.d's component guard sums a component's layers. Read so, the shipped nodes are held
//! within 1% of the full build, the same panels at 16 nodes a panel, wherever the tables are read:
//! R06.T5.e's gate, the slow test `standard_nodes_match_the_full_build_where_the_tables_are_read`
//! (decided 2026-10-05, `decision-r06-t5e-gate.md` and `-2.md`). On the Milky Way they came within
//! 0.08% of it in every component bin's light and colour, within 0.04% on every band ray at its
//! four points and within one radial node at every cap. The full build is held within 0.25% of
//! 32 nodes a panel there (`the_full_build_matches_thirty_two_nodes_a_panel_where_the_tables_are_read`;
//! 0.013% measured). A single function or bin is not good to 1%: one component's layer's light
//! fainter than a magnitude was off the full build's by up to 3% of the function's light, and one
//! 0.05-mag bin by up to 5% of it, since a main-sequence node holds one bin for gigayears and fewer
//! nodes move light between neighbouring bins. A new reader of one function or bin alone re-gates against the
//! full build.
//!
//! Nothing here draws a random word or changes generated output: the tables only read.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::Population;
use crate::galaxy::ages::AgeDistribution;
use crate::galaxy::fates::{
    MAX_PANEL_LN_MASS, StellarFates, companion_share_below, fates_for, ln_mass_panels, panel_edges,
};
use crate::galaxy::fields::{Component, ComponentId, SOLAR_RADIUS_LENGTHS};
use crate::galaxy::imf::{MassBand, MassFunction};
use crate::galaxy::quad::gl16;
use crate::id::Layer;
use crate::math;
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::{MIN_INITIAL_MASS, Track};
use crate::stellar::substellar;
use crate::stellar::{Composition, Phase, StarState};
use crate::tables::gauss_legendre::{
    GL4_NODES, GL4_WEIGHTS, GL8_NODES, GL8_WEIGHTS, GL16_NODES, GL16_WEIGHTS,
};
#[cfg(test)]
use crate::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SOLAR_ABSOLUTE_MAGNITUDE_V;
use crate::units::{HeliumExcess, Magnitudes, SolarLuminositiesV, SolarMasses, Years};

use super::binary_light::{self, Applied};
use super::photometry::{absolute_v_of_state, colour_of_state};

/// The brightest edge of the tables, M<sub>V</sub>: brighter than any star of the tracks (about
/// −10 at the top of layer E).
pub const BRIGHTEST_MAGNITUDE: f64 = -12.0;

/// The faintest edge of the tables, M<sub>V</sub>: the faintest M dwarfs and the warmest brown
/// dwarfs plan 06's photometry gives a V are near +19; fainter light is kept in one bin beyond.
pub const FAINTEST_MAGNITUDE: f64 = 20.0;

/// The width of a bin, mag.
pub const MAGNITUDE_STEP: f64 = 0.05;

/// The number of bins between [`BRIGHTEST_MAGNITUDE`] and [`FAINTEST_MAGNITUDE`].
pub const MAGNITUDE_BINS: usize = 640;

/// The equal parts each living phase of a track is cut into (Design note 7), besides its knots:
/// 32, kept by R06.T5.e (decided 2026-10-05), whose 16 moved layer A's cap at cut 11 one node in.
pub const SAMPLES_PER_PHASE: u32 = 32;

/// The light's ages at which a table holds a snapshot, Julian years: 0, 10³, 2 × 10³, 10⁴, 10⁵ and
/// the light-crossing bound L = 2¹⁸ (Design note 7). Between them a table is linear in the light's
/// age; beyond L it holds L's. The snapshot at 2 × 10³ years is the one [`REFERENCE_TIME`] adds: a
/// query at −H reads the reference time's tables 2H older.
pub const EMITTED_AGO_YEARS: [f64; 6] = [0.0, 1e3, 2e3, 1e4, 1e5, 262_144.0];

/// The time every galaxy's tables are built for, +H ([`ClockWindow::END`]): the latest a query
/// can ask, so that every query in the window reads a light age at or beyond its own
/// ([`LuminosityTables::age_for`]).
pub const REFERENCE_TIME: UniverseTime = ClockWindow::END;

/// Three-point Gauss–Legendre's nodes on [0, 1], (1 ∓ √(3/5)) ÷ 2 and ½.
const GL3_NODES: [f64; 3] = [0.112_701_665_379_258_31, 0.5, 0.887_298_334_620_741_7];

/// The ends of the three sub-parts on [0, 1] that hold three-point Gauss–Legendre's weights, 5/18,
/// 8/18 and 5/18: each contains its node.
const GL3_CELLS: [f64; 3] = [5.0 / 18.0, 13.0 / 18.0, 1.0];

/// Intervals of log age for an object below 0.1 M☉, which plan 06's cooling fits follow.
const COOLING_SAMPLES: u32 = 96;

/// The youngest age at which an object below 0.1 M☉ is sampled, years: younger ones are dark
/// protostars or a negligible share of any component.
const COOLING_FIRST_AGE: f64 = 1e5;

/// Intervals of ln m over the brown dwarfs' range.
const BROWN_DWARF_MASS_INTERVALS: u32 = 32;

/// The companions' mass-ratio kinks at which a companion integral is split: plan 02's lower edge,
/// plan 11's 0.3 and the twins' 0.95.
const COMPANION_RATIO_KINKS: [f64; 3] = [0.1, 0.3, 0.95];

/// The layers a table set holds, in [`Layer::ALL`]'s order.
const LAYER_COUNT: usize = Layer::ALL.len();

/// One snapshot of a table: per system, at one emitted time.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    /// The V light, L☉,V, of the stars fainter than each bin edge (light beyond the faintest edge
    /// included), [`MAGNITUDE_BINS`] + 1 values from the brightest edge.
    light_fainter: Vec<f64>,
    /// The number of stars brighter than each bin edge.
    count_brighter: Vec<f64>,
    /// The number of stars fainter than the faintest edge.
    beyond: f64,
    /// The number of stars dark in V.
    dark: f64,
    /// The number of stars that are remnants (all dark).
    remnants: f64,
    /// The colour sums of the stars fainter than each bin edge, as `light_fainter` is laid out.
    colour_fainter: Vec<ColourSums>,
    /// What the pair-evolved correction did (R06.T5.d); zero where none applies.
    pair: Applied,
}

/// Sums over stars of their V light times, in order: `lux_per_v0` (the photopic light), the
/// photopic light times each of the two chroma channels, and the photopic light times ρ.
type ColourSums = [f64; 4];

#[must_use]
fn add_sums(a: ColourSums, b: ColourSums) -> ColourSums {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

/// The colour of a population's light: the flux-weighted means the band's texels take (Design
/// note 15).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightColour {
    lux_per_v0: f64,
    chroma: [f64; 2],
    sp_ratio: f64,
}

impl LightColour {
    /// The photopic illuminance per unit of V-band illuminance: the stars' `lux_per_v0`
    /// ([`StarColour::lux_per_v0`](super::colour::StarColour::lux_per_v0)) weighted by V light.
    #[must_use]
    pub const fn lux_per_v0(&self) -> f64 {
        self.lux_per_v0
    }

    /// The chroma, linear Rec. 709 r and g of unit luminance, weighted by photopic light.
    #[must_use]
    pub const fn chroma(&self) -> [f64; 2] {
        self.chroma
    }

    /// The S/P ratio ρ, weighted by photopic light, so that ρ times the photopic light is the
    /// scotopic light.
    #[must_use]
    pub const fn sp_ratio(&self) -> f64 {
        self.sp_ratio
    }

    #[must_use]
    fn of_sums(sums: ColourSums, light: f64) -> Option<Self> {
        (light > 0.0 && sums[0] > 0.0).then(|| Self {
            lux_per_v0: sums[0] / light,
            chroma: [sums[1] / sums[0], sums[2] / sums[0]],
            sp_ratio: sums[3] / sums[0],
        })
    }
}

impl Snapshot {
    #[must_use]
    fn from_bins(bins: &Bins) -> Self {
        let mut light_fainter = vec![0.0; MAGNITUDE_BINS + 1];
        let mut count_brighter = vec![0.0; MAGNITUDE_BINS + 1];
        let mut running = bins.beyond_light;
        light_fainter[MAGNITUDE_BINS] = running;
        for k in (0..MAGNITUDE_BINS).rev() {
            running += bins.light[k];
            light_fainter[k] = running;
        }
        let mut running = 0.0;
        for k in 0..MAGNITUDE_BINS {
            running += bins.count[k];
            count_brighter[k + 1] = running;
        }
        let mut colour_fainter = vec![[0.0; 4]; MAGNITUDE_BINS + 1];
        let mut sums = bins.beyond_colour;
        colour_fainter[MAGNITUDE_BINS] = sums;
        for k in (0..MAGNITUDE_BINS).rev() {
            sums = add_sums(sums, bins.colour[k]);
            colour_fainter[k] = sums;
        }
        Self {
            light_fainter,
            count_brighter,
            colour_fainter,
            beyond: bins.beyond_count,
            dark: bins.dark,
            remnants: bins.remnants,
            pair: bins.pair,
        }
    }

    fn heap_bytes(&self) -> usize {
        (self.light_fainter.capacity() + self.count_brighter.capacity()) * size_of::<f64>()
            + self.colour_fainter.capacity() * size_of::<ColourSums>()
    }
}

/// A table's values at the bin edges, read at `m` by linear interpolation within a bin and held at
/// the ends.
#[must_use]
fn at_edges(values: &[f64], m: f64) -> f64 {
    at_edges_by(values, m, |&v| v)
}

/// [`at_edges`] over one field of each edge's value.
#[must_use]
fn at_edges_by<T>(values: &[T], m: f64, field: impl Fn(&T) -> f64) -> f64 {
    let x = (m - BRIGHTEST_MAGNITUDE) / MAGNITUDE_STEP;
    if x.is_nan() || x <= 0.0 {
        return field(&values[0]);
    }
    let last = values.len() - 1;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the table holds 641 edges, exact in f64"
    )]
    let top = last as f64;
    if x >= top {
        return field(&values[last]);
    }
    let floor = x.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "x lies in (0, 640), so its floor is a small non-negative integer"
    )]
    let k = floor as usize;
    let frac = x - floor;
    let (lo, hi) = (field(&values[k]), field(&values[k + 1]));
    lo + (hi - lo) * frac
}

/// The cumulative luminosity function of one component and layer: per system, the V light of its
/// stars fainter than an absolute magnitude and the number brighter, at any emitted time back to
/// the light-crossing bound (Design note 7).
///
/// One function alone is not good to 1% at the shipped nodes (R06.T5.e): its light fainter than
/// a magnitude was off the full build's by up to 3% of its light, and one 0.05-mag bin by up to
/// 5%.
/// Summed over components and layers, as the band, the caps and the star counts read them, the
/// tables are held within 1% of the full build: see the [module](self)'s accuracy. A new reader of
/// one function or bin alone re-gates against the full build.
#[derive(Debug, Clone, PartialEq)]
pub struct LuminosityFunction {
    snapshots: Vec<Snapshot>,
}

impl LuminosityFunction {
    /// The snapshots bracketing a light's age and the weight of the later one.
    fn bracket(&self, emitted_ago: Span) -> (&Snapshot, &Snapshot, f64) {
        let years = emitted_ago.as_julian_years_f64().max(0.0);
        let nodes = &EMITTED_AGO_YEARS;
        let last = nodes.len() - 1;
        if years >= nodes[last] {
            return (&self.snapshots[last], &self.snapshots[last], 0.0);
        }
        let k = nodes.partition_point(|&node| node <= years).max(1) - 1;
        let t = (years - nodes[k]) / (nodes[k + 1] - nodes[k]);
        (&self.snapshots[k], &self.snapshots[k + 1], t)
    }

    fn read(&self, emitted_ago: Span, value: impl Fn(&Snapshot) -> f64) -> f64 {
        let (a, b, t) = self.bracket(emitted_ago);
        let (va, vb) = (value(a), value(b));
        va + (vb - va) * t
    }

    /// The V light per system of the stars fainter than absolute magnitude `m_v`, for light that
    /// left them `emitted_ago` ago.
    #[must_use]
    pub fn light_fainter_than(&self, m_v: Magnitudes, emitted_ago: Span) -> SolarLuminositiesV {
        SolarLuminositiesV::new(self.read(emitted_ago, |s| at_edges(&s.light_fainter, m_v.value())))
    }

    /// The number of stars per system brighter than absolute magnitude `m_v`, for light that left
    /// them `emitted_ago` ago.
    #[must_use]
    pub fn count_brighter_than(&self, m_v: Magnitudes, emitted_ago: Span) -> f64 {
        self.read(emitted_ago, |s| at_edges(&s.count_brighter, m_v.value()))
    }

    /// The V light per system of all its stars.
    #[must_use]
    pub fn total_light(&self, emitted_ago: Span) -> SolarLuminositiesV {
        SolarLuminositiesV::new(self.read(emitted_ago, |s| s.light_fainter[0]))
    }

    /// The number of stars per system, dark ones included.
    #[must_use]
    pub fn stars_per_system(&self, emitted_ago: Span) -> f64 {
        self.read(emitted_ago, |s| {
            s.count_brighter[MAGNITUDE_BINS] + s.beyond + s.dark
        })
    }

    /// The colour of the light per system of the stars fainter than absolute magnitude `m_v`, for
    /// light that left them `emitted_ago` ago; `None` where there is no such light.
    #[must_use]
    pub fn colour_fainter_than(&self, m_v: Magnitudes, emitted_ago: Span) -> Option<LightColour> {
        let sums = self.colour_sums_fainter_than(m_v, emitted_ago);
        let light = self.read(emitted_ago, |s| at_edges(&s.light_fainter, m_v.value()));
        LightColour::of_sums(sums, light)
    }

    /// The colour sums per system of the stars fainter than absolute magnitude `m_v`, for light
    /// that left them `emitted_ago` ago, unnormalised.
    ///
    /// In order: the V light times `lux_per_v0` (the photopic light, L☉,V × `lux_per_v0`), then
    /// the photopic light times each chroma channel, then the photopic light times ρ. They are the
    /// sums [`colour_fainter_than`](Self::colour_fainter_than) divides, so that a reader adding
    /// the light of several functions (the band, R06.T9.b) weights each by its own light. An `m_v`
    /// of −∞ reads all of the light.
    #[must_use]
    pub(crate) fn colour_sums_fainter_than(
        &self,
        m_v: Magnitudes,
        emitted_ago: Span,
    ) -> ColourSums {
        let (a, b, t) = self.bracket(emitted_ago);
        let m = m_v.value();
        std::array::from_fn(|i| {
            let (va, vb) = (
                at_edges_by(&a.colour_fainter, m, |c| c[i]),
                at_edges_by(&b.colour_fainter, m, |c| c[i]),
            );
            va + (vb - va) * t
        })
    }

    /// The number of stars per system dark in V ([`super::photometry::is_dark_in_v`]).
    #[must_use]
    pub fn dark_per_system(&self, emitted_ago: Span) -> f64 {
        self.read(emitted_ago, |s| s.dark)
    }

    /// The number of stars per system that are remnants, all of them dark.
    #[must_use]
    pub fn remnants_per_system(&self, emitted_ago: Span) -> f64 {
        self.read(emitted_ago, |s| s.remnants)
    }

    /// The change pair evolution makes to the V light per system, as the tables apply it
    /// (R06.T5.d): the pair-evolved light less the single-star light, after the clamp that keeps
    /// every bin's light at zero or more; negative where interaction removes light, and zero for a
    /// layer or galaxy that takes no correction. [`total_light`](Self::total_light) less it is the
    /// single-star light.
    #[must_use]
    pub fn pair_light(&self, emitted_ago: Span) -> SolarLuminositiesV {
        SolarLuminositiesV::new(self.read(emitted_ago, |s| s.pair.light))
    }

    /// The standard error of the fitted change in V light per system, from the fit's sampling
    /// (`hyperion-fit`'s `sky_binary_light_*` tasks): a systematic shared by every system the
    /// function is read for.
    #[must_use]
    pub fn pair_light_sigma(&self, emitted_ago: Span) -> SolarLuminositiesV {
        SolarLuminositiesV::new(self.read(emitted_ago, |s| s.pair.sigma))
    }

    #[must_use]
    fn zero() -> Self {
        let bins = Bins::new();
        Self {
            snapshots: vec![Snapshot::from_bins(&bins); EMITTED_AGO_YEARS.len()],
        }
    }

    fn heap_bytes(&self) -> usize {
        self.snapshots.capacity() * size_of::<Snapshot>()
            + self
                .snapshots
                .iter()
                .map(Snapshot::heap_bytes)
                .sum::<usize>()
    }
}

/// How a table set is built: the knobs the tests turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuildOptions {
    /// Equal parts per living phase ([`SAMPLES_PER_PHASE`]).
    pub(crate) samples_per_phase: u32,
    /// The Gauss–Legendre nodes of each mass panel ([`MassNodes`]).
    pub(crate) mass_nodes: MassNodes,
    /// Whether companions are counted.
    pub(crate) companions: Companions,
    /// Which metallicity bins of a component with a radial gradient are built.
    pub(crate) bins: GradientBins,
}

/// How many Gauss–Legendre nodes each mass panel of the tables takes (R06.T5.e).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MassNodes {
    /// An order scaled with the panel's width, the shipped nodes (R06.T5.e, decided 2026-10-05):
    /// 16 on a panel wider than half of [`MAX_PANEL_LN_MASS`], 8 on one wider than a quarter and
    /// 4 on a narrower one, so never fewer than 32 nodes per unit of ln m.
    Scaled,
    /// 16 on every panel, as R06.T5 built them: the full build's.
    #[cfg(test)]
    Sixteen,
    /// 32 on every panel: the reference the full build's own convergence is measured against.
    #[cfg(test)]
    ThirtyTwo,
}

impl MassNodes {
    /// The rule on [−1, 1], nodes and weights, of a panel `width` wide in ln m.
    #[must_use]
    fn rule(self, width: f64) -> (&'static [f64], &'static [f64]) {
        match self {
            #[cfg(test)]
            Self::Sixteen => (&GL16_NODES, &GL16_WEIGHTS),
            #[cfg(test)]
            Self::ThirtyTwo => (&GL32_NODES, &GL32_WEIGHTS),
            Self::Scaled if width > 0.5 * MAX_PANEL_LN_MASS => (&GL16_NODES, &GL16_WEIGHTS),
            Self::Scaled if width > 0.25 * MAX_PANEL_LN_MASS => (&GL8_NODES, &GL8_WEIGHTS),
            Self::Scaled => (&GL4_NODES, &GL4_WEIGHTS),
        }
    }
}

/// Whether a table counts the companions of its layer's primaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Companions {
    /// Primaries and companions, as the tables are built.
    Included,
    /// Primaries alone, for the tests.
    #[cfg(test)]
    Omitted,
}

/// Which metallicity bins a component with a radial gradient builds ([`ComponentBins`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GradientBins {
    /// Every bin, as the tables are built.
    All,
    /// The solar circle's bin alone, which [`LuminosityTables::get`] reads, then read at every
    /// point: the full build's bin bit for bit, for the tests that read no other. The young thin
    /// disc's seven bins take 17 metallicities' samples, its solar circle's three.
    #[cfg(test)]
    SolarCircle,
}

impl BuildOptions {
    /// The options the tables are built with: R06.T5.e's nodes, [`MassNodes::Scaled`], and
    /// [`SAMPLES_PER_PHASE`] parts a phase.
    pub(crate) const STANDARD: Self = Self {
        samples_per_phase: SAMPLES_PER_PHASE,
        mass_nodes: MassNodes::Scaled,
        companions: Companions::Included,
        bins: GradientBins::All,
    };

    /// The reference R06.T5.e measures the shipped nodes against: 16 nodes on every mass panel,
    /// [`SAMPLES_PER_PHASE`] parts a phase and \[Fe/H\] nodes at 0.05 dex, T5's orders on T5.e's
    /// panels.
    #[cfg(test)]
    pub(crate) const FULL: Self = Self {
        mass_nodes: MassNodes::Sixteen,
        ..Self::STANDARD
    };
}

/// Every component and layer of a galaxy's luminosity functions (Design note 7), built at
/// [`REFERENCE_TIME`]: the server builds them once per galaxy and caches them.
#[derive(Debug, Clone, PartialEq)]
pub struct LuminosityTables {
    time: UniverseTime,
    /// The solar circle's radius, ly: where [`LuminosityTables::get`] reads.
    solar_radius: f64,
    /// Per component, by index: its metallicity bins.
    layout: Vec<ComponentBins>,
    /// `(the component's first function + bin) × LAYER_COUNT + layer index`.
    functions: Vec<LuminosityFunction>,
}

/// A component's tables by the mean \[Fe/H\] of its systems: one bin for a component whose
/// metallicity is the same everywhere, and one per [`GRADIENT_BIN_STEP`] of mean \[Fe/H\] for one
/// with a radial gradient (the thin discs), read by the mean at the point asked about.
#[derive(Debug, Clone, PartialEq)]
struct ComponentBins {
    /// The index of the component's first bin among the tables' bins.
    first: usize,
    /// Each bin's mean \[Fe/H\], ascending.
    means: Vec<f64>,
    /// For a gradient component, its mean \[Fe/H\] every [`RADIAL_STEP_LY`] of cylindrical
    /// radius from the centre (the last held beyond); empty otherwise.
    radial_means: Vec<f64>,
}

/// The radial step at which a gradient component's mean \[Fe/H\] is tabulated, ly.
const RADIAL_STEP_LY: f64 = 250.0;

impl LuminosityTables {
    /// The tables of `galaxy`, for every query of its clock window: built at [`REFERENCE_TIME`],
    /// each snapshot holding the stars as they were then less its light age, and read at a query's
    /// time through [`age_for`](Self::age_for).
    ///
    /// It builds a track for each mass node at each of the galaxy's reference metallicities, some
    /// fifteen thousand tracks: half a minute of work on one thread, which the server does once per
    /// galaxy.
    ///
    /// # Examples
    ///
    /// The light of layer C's stars fainter than M<sub>V</sub> 0 near the Sun, seen at the epoch
    /// from 500 ly away (`no_run`: a full build takes half a minute or more).
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::sky::luminosity::LuminosityTables;
    /// use hyperion_sim::time::{Span, UniverseTime};
    /// use hyperion_sim::units::Magnitudes;
    ///
    /// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
    /// let tables = LuminosityTables::build(&galaxy);
    /// let ago = tables.age_for(UniverseTime::EPOCH, Span::from_julian_years(500).ok_or("span")?);
    /// let thin = galaxy.fields().component_ids().next().ok_or("a component")?;
    /// let light = tables.get(thin, Layer::C).light_fainter_than(Magnitudes::new(0.0), ago);
    /// assert!(light.value() > 0.0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn build(galaxy: &Galaxy) -> Self {
        let all: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        Self::build_with(galaxy, REFERENCE_TIME, &all, BuildOptions::STANDARD)
    }

    /// Tables of `galaxy` that hold no star, built at [`REFERENCE_TIME`] without a track.
    ///
    /// Every component's functions are zero at every light age: no V light (0 L☉,V per system),
    /// no star brighter than any M<sub>V</sub>, no colour. They are laid out as
    /// [`build`](Self::build)'s are, but build no track, so they cost next to nothing. They suit
    /// a caller that needs a [`SkyContext`](crate::sky::census::SkyContext) but reads no table: a
    /// census with forced caps (R06.T8.e's oracle), whose skips read only the envelope and whose
    /// stars their own states, lists with these what it lists with `build`'s.
    ///
    /// # Examples
    ///
    /// The context of a census whose caps are forced, which reads no table:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
    /// use hyperion_sim::galaxy::gas::noise::NoiseCache;
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext};
    /// use hyperion_sim::sky::envelope::BrightnessEnvelope;
    /// use hyperion_sim::sky::luminosity::{LuminosityTables, REFERENCE_TIME};
    /// use hyperion_sim::time::Span;
    /// use hyperion_sim::units::Magnitudes;
    ///
    /// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
    /// let (dark, envelope) = (LuminosityTables::dark(&galaxy), BrightnessEnvelope::build(&galaxy));
    /// assert_eq!(dark.time(), REFERENCE_TIME);
    /// let thin = galaxy.fields().component_ids().next().ok_or("a component")?;
    /// let c = dark.get(thin, Layer::C);
    /// // No star at all, so no light to give a colour.
    /// assert!(c.stars_per_system(Span::ZERO) <= 0.0);
    /// assert!(c.colour_fainter_than(Magnitudes::new(20.0), Span::ZERO).is_none());
    /// let offsets = CellOffsets::build(&galaxy);
    /// let ctx = SkyContext {
    ///     tables: &dark,
    ///     envelope: &envelope,
    ///     offsets: &offsets,
    ///     noise: NoiseCache::with_capacity(1 << 12),
    ///     cells: &NoSkyCellCache,
    ///     sources: &[],
    ///     modifiers: &NoModifiers,
    /// };
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn dark(galaxy: &Galaxy) -> Self {
        Self::build_with(galaxy, REFERENCE_TIME, &[], BuildOptions::STANDARD)
    }

    /// The light age at which these tables hold light that left its stars `emitted_ago` before
    /// `t`: `emitted_ago` + (the tables' time − `t`), which every reader of a table passes as its
    /// `emitted_ago`. Within the clock window `t` is at or before the tables' time; a later `t`
    /// reads no younger than the tables' own present (the age held at zero).
    #[must_use]
    pub fn age_for(&self, t: UniverseTime, emitted_ago: Span) -> Span {
        match self
            .time
            .checked_since(t)
            .and_then(|lead| emitted_ago.checked_add(lead))
        {
            Some(age) if !age.is_negative() => age,
            Some(_) => Span::ZERO,
            // Only a span beyond ±2⁶³ s overflows, which no query's time and light age reach.
            None => emitted_ago,
        }
    }

    /// [`build`](Self::build) for `components` only (the others are left dark), with `options`.
    #[must_use]
    pub(crate) fn build_with(
        galaxy: &Galaxy,
        time: UniverseTime,
        components: &[ComponentId],
        options: BuildOptions,
    ) -> Self {
        TablesPlan::new(galaxy, time, components, options, SAMPLE_JOB_NODES).run_serially()
    }

    /// The work of [`build`](Self::build) split into jobs, for a caller that runs them on its own
    /// threads (the server's pool, R06.T11.c; the sim spawns none): see [`TablesPlan`]. Making
    /// the plan finds each stage's mass panels (R06.T5.e), some 5 CPU-s for the Milky Way, so a
    /// server calls this off its async runtime.
    #[must_use]
    pub fn plan(galaxy: &Galaxy) -> TablesPlan {
        let all: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        TablesPlan::new(
            galaxy,
            REFERENCE_TIME,
            &all,
            BuildOptions::STANDARD,
            SAMPLE_JOB_NODES,
        )
    }

    /// The time the tables were built for: [`REFERENCE_TIME`], or a test's own.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// The luminosity function of `component`'s systems in `layer` at the solar circle.
    ///
    /// # Panics
    ///
    /// If `component` is not one of the galaxy's the tables were built for.
    #[must_use]
    pub fn get(&self, component: ComponentId, layer: Layer) -> &LuminosityFunction {
        let bins = &self.layout[component.index()];
        let bin = bin_at_radius(&bins.means, &bins.radial_means, self.solar_radius);
        &self.functions[(bins.first + bin) * LAYER_COUNT + layer_index(layer)]
    }

    /// The luminosity function of `component`'s systems in `layer` at `p`: for a component with a
    /// radial metallicity gradient, the bin nearest its mean \[Fe/H\] at `p`'s radius.
    ///
    /// # Panics
    ///
    /// If `component` is not one of the galaxy's the tables were built for.
    #[must_use]
    pub fn get_at(&self, component: ComponentId, layer: Layer, p: &PointLy) -> &LuminosityFunction {
        let bins = &self.layout[component.index()];
        let bin = bin_at_radius(
            &bins.means,
            &bins.radial_means,
            (p.x * p.x + p.y * p.y).sqrt(),
        );
        &self.functions[(bins.first + bin) * LAYER_COUNT + layer_index(layer)]
    }

    /// The bytes the tables own on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.functions.capacity() * size_of::<LuminosityFunction>()
            + self
                .functions
                .iter()
                .map(LuminosityFunction::heap_bytes)
                .sum::<usize>()
    }
}

/// The mass nodes of one [`SampleJob`] at most: some ten jobs per \[Fe/H\] node (the Milky Way's
/// stages offer 10 to 13 at the shipped nodes), each some tenths of a second of tracks.
const SAMPLE_JOB_NODES: usize = 64;

/// [`LuminosityTables::build`] as jobs (decided 2026-10-03, `decision-r06-tables.md`), one
/// metallicity at a time. The plan's [`Stage`]s are its \[Fe/H\] nodes, from the lowest. Each stage
/// makes that metallicity's track samples, per chunk of mass nodes ([`SampleJob`]), put together by
/// [`track_samples`](Self::track_samples). It then adds them to every component bin that reads
/// the node ([`AccumulateJob`]), into the bin's running sums ([`BinSums`]), which the caller holds
/// from [`bin_sums`](Self::bin_sums) to [`assemble`](Self::assemble). Once its accumulation jobs
/// have run, a stage's samples are no longer needed and can be dropped.
///
/// One metallicity's samples are at most some 60 MiB at the shipped nodes (228 MiB at 16 nodes a
/// panel, the full build's; 151 MiB under T5's panels), and the Milky Way's tables read 22
/// metallicities. Held to the build's end, as the
/// split first did, they took a full build to 2.7 GB. Stage by stage a serial build peaks at
/// 155 MiB (R06.T5.e's nodes, release test binary, 2026-10-05). Within a stage only the bins that
/// read its node accumulate, 1 to 22 of the Milky Way's 53, so a pool keeps its threads busy by
/// sampling the next stage while this one accumulates. That holds two stages' samples and changes
/// no bit. On 16 threads, under shared load, it took 18.8 s and 439 MiB, against 23.2 s and
/// 237 MiB stage by stage and the first split's 18.8 s and 2.6 GB (T5's nodes, 2026-10-04).
///
/// Each stage has its own mass panels, cut where its tracks end their phases at the galaxy's age
/// edges (R06.T5.e), so a stage's sample jobs are counted from its own nodes. Making the plan,
/// serially, takes some 5 CPU-s for the Milky Way (the search for those panels some 3), of the
/// build's 31, so a server makes it off its async runtime.
///
/// Every job is a pure function of the plan and its inputs. The chunks are put back in their jobs'
/// order, and every bin adds its nodes in their own order (an accumulation job panics otherwise),
/// so the tables are `build`'s, bit for bit, whatever threads run the jobs and in whatever order
/// they finish: the stages' sample jobs in any order, even ahead of their turn, and the
/// accumulation jobs of different bins in any order.
///
/// # Examples
///
/// The jobs run in turn here; a server hands each to its pool (`no_run`: a full build takes half
/// a minute or more of CPU).
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
/// let plan = LuminosityTables::plan(&galaxy);
/// let mut sums = plan.bin_sums();
/// for stage in plan.stages() {
///     let chunks: Vec<_> = plan.sample_jobs(stage).map(|job| plan.run_samples(job)).collect();
///     let samples = plan.track_samples(chunks);
///     for job in plan.accumulate_jobs(stage) {
///         plan.run_accumulate(&samples, job, &mut sums[job.bin()]);
///     }
///     // The stage's samples are dropped here, before the next stage's are made.
/// }
/// let tables = plan.assemble(sums);
/// assert_eq!(tables, LuminosityTables::build(&galaxy));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TablesPlan {
    time: UniverseTime,
    options: BuildOptions,
    solar_radius: f64,
    /// Each stage's mass grid, in the stages' order (R06.T5.e).
    grids: Vec<MassGrid>,
    brown_dwarfs: BrownDwarfGrid,
    /// The oldest age any snapshot reads, years.
    max_age: f64,
    /// The stages, in their order.
    stages: Vec<PlannedStage>,
    /// The mass nodes of a sample job at most.
    job_nodes: usize,
    /// Per component of the galaxy, by index: its bins' means and radial means, or `None` for one
    /// left dark.
    layout: Vec<Option<(Vec<f64>, Vec<f64>)>>,
    /// The bins to accumulate, in the tables' order.
    bins: Vec<PlannedBin>,
    /// Whether the pair-evolved correction applies: only under the default mass function, which
    /// the fit drew from.
    pair_evolution: PairEvolution,
}

/// Whether a build adds the pair-evolved correction (R06.T5.d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairEvolution {
    /// The fitted differences are added.
    Corrected,
    /// Every star is evolved alone: a galaxy of another mass function than the fit's.
    Ignored,
}

/// One component bin of a [`TablesPlan`].
#[derive(Debug, Clone, PartialEq)]
struct PlannedBin {
    ages: AgeDistribution,
    /// The Gauss–Hermite nodes, \[Fe/H\] and weight, ascending ([`metallicity_nodes`]).
    metallicities: [(f64, f64); 3],
}

/// One stage of a [`TablesPlan`]: a metallicity and the bins that add it.
#[derive(Debug, Clone, PartialEq)]
struct PlannedStage {
    fe_h: f64,
    /// One job per bin whose next nodes are the stage's, in the bins' order.
    jobs: Vec<AccumulateJob>,
}

/// One metallicity of a [`TablesPlan`]: the samples of its \[Fe/H\] node and their accumulation
/// into the bins that read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Stage {
    index: usize,
}

impl Stage {
    /// The stage's place among the plan's stages.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }
}

/// One job of track samples: the mass nodes `nodes` (the stars' nodes, then the brown dwarfs')
/// at the \[Fe/H\] node of the plan's stage `stage`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SampleJob {
    stage: usize,
    index: usize,
    nodes: Range<usize>,
}

impl SampleJob {
    /// The job's place among its stage's sample jobs.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The stage the job samples for.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        Stage { index: self.stage }
    }
}

/// What a [`SampleJob`] made.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleChunk {
    stage: usize,
    job: usize,
    samples: Vec<NodeSamples>,
}

impl SampleChunk {
    /// The stage the chunk is of.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        Stage { index: self.stage }
    }

    /// The bytes the chunk owns on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        samples_heap_bytes(&self.samples)
    }
}

/// One stage's track samples: the stars' nodes, then the brown dwarfs'.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackSamples {
    stage: usize,
    samples: Vec<NodeSamples>,
}

impl TrackSamples {
    /// The stage the samples are of.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        Stage { index: self.stage }
    }

    /// The bytes the samples own on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        samples_heap_bytes(&self.samples)
    }

    /// Adds every node's stars to one snapshot's `bins`, each layer's weighted by `share`, the
    /// metallicity node's weight, and every part by its share of the born systems (`born_cdf`, as
    /// [`NodeSamples::accumulate`] reads it).
    fn add(
        &self,
        grid: &MassGrid,
        brown_dwarfs: &BrownDwarfGrid,
        share: f64,
        born_cdf: &impl Fn(Years) -> f64,
        bins: &mut [Bins],
    ) {
        let (stars, bd_samples) = self.samples.split_at(grid.nodes.len());
        let mut layers = Vec::with_capacity(LAYER_COUNT);
        for (node, node_samples) in grid.nodes.iter().zip(stars) {
            layers.clear();
            layers.extend(
                node.weights
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| **w > 0.0)
                    .map(|(layer, &w)| (layer, share * w)),
            );
            node_samples.accumulate(born_cdf, &layers, bins);
        }
        let bd = layer_index(Layer::BrownDwarf);
        for (&weight, node_samples) in brown_dwarfs.weights.iter().zip(bd_samples) {
            node_samples.accumulate(born_cdf, &[(bd, share * weight)], bins);
        }
    }
}

/// The heap bytes of `samples`.
#[must_use]
fn samples_heap_bytes(samples: &Vec<NodeSamples>) -> usize {
    samples.capacity() * size_of::<NodeSamples>()
        + samples.iter().map(NodeSamples::heap_bytes).sum::<usize>()
}

/// One job of accumulation: the nodes `first..end` of the plan's component bin `bin`, all at its
/// stage's \[Fe/H\].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccumulateJob {
    stage: usize,
    bin: usize,
    first: usize,
    end: usize,
}

impl AccumulateJob {
    /// The stage whose samples the job adds.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        Stage { index: self.stage }
    }

    /// The bin the job adds to: its place among [`TablesPlan::bin_sums`].
    #[must_use]
    pub const fn bin(&self) -> usize {
        self.bin
    }
}

/// One component bin's running sums: per snapshot, in [`EMITTED_AGO_YEARS`]'s order, and per
/// layer, the bins of its stars, and how many of its metallicity nodes it has added.
#[derive(Debug, Clone, PartialEq)]
pub struct BinSums {
    bin: usize,
    added: usize,
    snapshots: Vec<Vec<Bins>>,
}

impl BinSums {
    #[must_use]
    fn new(bin: usize) -> Self {
        Self {
            bin,
            added: 0,
            snapshots: EMITTED_AGO_YEARS
                .iter()
                .map(|_| (0..LAYER_COUNT).map(|_| Bins::new()).collect())
                .collect(),
        }
    }

    /// The bin the sums are of: their place among [`TablesPlan::bin_sums`].
    #[must_use]
    pub const fn bin(&self) -> usize {
        self.bin
    }

    /// The bin's functions by layer, the rogue planets' dark: the finish step of
    /// [`TablesPlan::assemble`], once every node is in.
    #[must_use]
    fn into_functions(self) -> Vec<LuminosityFunction> {
        let mut per_layer: Vec<Vec<Snapshot>> = vec![Vec::new(); LAYER_COUNT];
        for bins in &self.snapshots {
            for (layer, b) in bins.iter().enumerate() {
                per_layer[layer].push(Snapshot::from_bins(b));
            }
        }
        let mut functions = vec![LuminosityFunction::zero(); LAYER_COUNT];
        for (layer, snapshots) in per_layer.into_iter().enumerate() {
            if Layer::ALL[layer] == Layer::RoguePlanet {
                continue;
            }
            functions[layer] = LuminosityFunction { snapshots };
        }
        functions
    }
}

impl TablesPlan {
    /// The plan of [`LuminosityTables::build_with`], with sample jobs of at most `job_nodes` mass
    /// nodes.
    #[must_use]
    fn new(
        galaxy: &Galaxy,
        time: UniverseTime,
        components: &[ComponentId],
        options: BuildOptions,
        job_nodes: usize,
    ) -> Self {
        let mut layout: Vec<Option<(Vec<f64>, Vec<f64>)>> =
            vec![None; galaxy.fields().components().len()];
        // The oldest age any snapshot reads: a component's oldest at the epoch plus the build's
        // time from the epoch where that is later, and a margin of 2,000 years for the rounding
        // of the snapshots' light ages.
        let max_age = galaxy
            .fields()
            .components()
            .iter()
            .map(|c| c.ages().max().value())
            .fold(0.0, f64::max)
            + time.since_epoch().as_julian_years_f64().max(0.0)
            + 2_000.0;
        debug_assert!(
            components
                .iter()
                .enumerate()
                .all(|(i, id)| !components[..i].contains(id)),
            "each component once"
        );
        let solar_radius = SOLAR_RADIUS_LENGTHS * galaxy.params().thin_disc().length().value();
        let mut planned: Vec<(usize, Vec<PlannedBin>)> = Vec::new();
        for &id in components {
            let component = galaxy.fields().component(id);
            let means = metallicity_bins(galaxy, component);
            let radial = if means.len() > 1 {
                radial_means(galaxy, component)
            } else {
                Vec::new()
            };
            let (means, radial) = match options.bins {
                GradientBins::All => (means, radial),
                #[cfg(test)]
                GradientBins::SolarCircle => solar_circle_bin(means, radial, solar_radius),
            };
            let by_bin = means
                .iter()
                .map(|&mean| PlannedBin {
                    ages: component.ages().clone(),
                    metallicities: metallicity_nodes(component, mean),
                })
                .collect();
            layout[id.index()] = Some((means, radial));
            planned.push((id.index(), by_bin));
        }
        // The bins in the tables' order: by component index.
        planned.sort_by_key(|(index, _)| *index);
        let bins: Vec<PlannedBin> = planned.into_iter().flat_map(|(_, bins)| bins).collect();
        let stages = stages_of(&bins);
        Self {
            time,
            options,
            solar_radius,
            grids: stage_grids(galaxy, time, options, &stages, Years::new(max_age)),
            brown_dwarfs: BrownDwarfGrid::new(galaxy.mass_function()),
            max_age,
            stages,
            job_nodes: job_nodes.max(1),
            layout,
            bins,
            pair_evolution: if binary_light::applies_to(galaxy) {
                PairEvolution::Corrected
            } else {
                PairEvolution::Ignored
            },
        }
    }

    /// The mass nodes of a stage: its stars', then the brown dwarfs'.
    #[must_use]
    fn stage_nodes(&self, stage: usize) -> usize {
        self.grids[stage].nodes.len() + self.brown_dwarfs.masses.len()
    }

    /// The stages, in the order their accumulation jobs run: by \[Fe/H\], from the lowest.
    pub fn stages(&self) -> impl Iterator<Item = Stage> + use<> {
        (0..self.stages.len()).map(|index| Stage { index })
    }

    /// The sample jobs of `stage`, in index order: its \[Fe/H\] node's mass nodes in chunks. They
    /// may run in any order and at any time, ahead of the stage's turn too.
    pub fn sample_jobs(&self, stage: Stage) -> impl Iterator<Item = SampleJob> + use<> {
        let (per, job_nodes) = (self.stage_nodes(stage.index), self.job_nodes);
        (0..per.div_ceil(job_nodes)).map(move |index| {
            let start = index * job_nodes;
            SampleJob {
                stage: stage.index,
                index,
                nodes: start..(start + job_nodes).min(per),
            }
        })
    }

    /// Runs one sample job: the tracks of its mass nodes at its stage's metallicity, cut into parts.
    ///
    /// # Panics
    ///
    /// If `job` is not one of this plan's.
    #[must_use]
    pub fn run_samples(&self, job: SampleJob) -> SampleChunk {
        let composition = composition_at(self.stages[job.stage].fe_h);
        let grid = &self.grids[job.stage];
        let stars = grid.nodes.len();
        let samples = job
            .nodes
            .map(|k| {
                if k < stars {
                    NodeSamples::of_mass(
                        grid.nodes[k].mass,
                        &composition,
                        self.max_age,
                        self.options.samples_per_phase,
                    )
                } else {
                    NodeSamples::of_cooling(
                        self.brown_dwarfs.masses[k - stars],
                        &composition,
                        self.max_age,
                    )
                }
            })
            .collect();
        SampleChunk {
            stage: job.stage,
            job: job.index,
            samples,
        }
    }

    /// One stage's samples from each of its sample jobs' chunks, in any order, put back in the
    /// jobs' order.
    ///
    /// # Panics
    ///
    /// If `chunks` are not exactly one of each of one stage's sample jobs.
    #[must_use]
    pub fn track_samples(&self, chunks: impl IntoIterator<Item = SampleChunk>) -> TrackSamples {
        let mut chunks: Vec<SampleChunk> = chunks.into_iter().collect();
        chunks.sort_by_key(|c| c.job);
        let stage = chunks.first().map_or(usize::MAX, |c| c.stage);
        assert!(
            stage < self.stages.len()
                && chunks.len() == self.stage_nodes(stage).div_ceil(self.job_nodes)
                && chunks
                    .iter()
                    .enumerate()
                    .all(|(i, c)| c.job == i && c.stage == stage),
            "one chunk of each of a stage's sample jobs"
        );
        let mut samples = Vec::with_capacity(self.stage_nodes(stage));
        for chunk in chunks {
            samples.extend(chunk.samples);
        }
        TrackSamples { stage, samples }
    }

    /// The accumulation jobs of `stage`: one per component bin whose next nodes are at its
    /// \[Fe/H\]. Those of different bins may run in any order; a bin's own run stage by stage.
    pub fn accumulate_jobs(&self, stage: Stage) -> impl Iterator<Item = AccumulateJob> + '_ {
        self.stages[stage.index].jobs.iter().copied()
    }

    /// Every bin's sums, empty, in the bins' order: the place of a job's [`AccumulateJob::bin`].
    #[must_use]
    pub fn bin_sums(&self) -> Vec<BinSums> {
        (0..self.bins.len()).map(BinSums::new).collect()
    }

    /// Runs one accumulation job: adds its stage's `samples` to its bin's `sums`, every layer and
    /// snapshot.
    ///
    /// # Panics
    ///
    /// If `samples` are not the job's stage's, or `sums` are not the job's bin's or have not added
    /// the bin's earlier stages, whose order the sums' bits depend on.
    pub fn run_accumulate(&self, samples: &TrackSamples, job: AccumulateJob, sums: &mut BinSums) {
        assert!(
            sums.bin == job.bin && sums.added == job.first,
            "a bin adds its metallicity nodes in their order"
        );
        assert!(
            samples.stage == job.stage && samples.samples.len() == self.stage_nodes(job.stage),
            "the samples are of the job's stage"
        );
        let planned = &self.bins[job.bin];
        let shift_now = self.time.since_epoch().as_julian_years_f64();
        for &(_, share) in &planned.metallicities[job.first..job.end] {
            for (&ago, snapshot) in EMITTED_AGO_YEARS.iter().zip(&mut sums.snapshots) {
                // A star's age then is its age at the epoch plus `shift`.
                let shift = shift_now - ago;
                let born_cdf = |age: Years| planned.ages.born_cdf(Years::new(age.value() - shift));
                samples.add(
                    &self.grids[job.stage],
                    &self.brown_dwarfs,
                    share,
                    &born_cdf,
                    snapshot,
                );
            }
        }
        sums.added = job.end;
    }

    /// The tables from every bin's sums, in any order, once each has added all its nodes: each
    /// bin's functions are finished from its sums, then put in the tables' order.
    ///
    /// # Panics
    ///
    /// If `sums` are not exactly one of each of this plan's bins, each with every node added.
    #[must_use]
    pub fn assemble(&self, sums: impl IntoIterator<Item = BinSums>) -> LuminosityTables {
        let mut sums: Vec<BinSums> = sums.into_iter().collect();
        sums.sort_by_key(|s| s.bin);
        assert!(
            sums.len() == self.bins.len()
                && sums
                    .iter()
                    .enumerate()
                    .all(|(i, s)| { s.bin == i && s.added == self.bins[i].metallicities.len() }),
            "every node of each of the plan's bins"
        );
        let mut sums = sums.into_iter();
        let mut layout = Vec::with_capacity(self.layout.len());
        let mut functions = Vec::new();
        for planned in &self.layout {
            let first = functions.len() / LAYER_COUNT;
            if let Some((means, radial_means)) = planned {
                for _ in means {
                    let mut bin = sums.next().expect("the plan's bins match its layout");
                    self.correct_for_pairs(&mut bin);
                    functions.extend(bin.into_functions());
                }
                layout.push(ComponentBins {
                    first,
                    means: means.clone(),
                    radial_means: radial_means.clone(),
                });
            } else {
                functions.extend((0..LAYER_COUNT).map(|_| LuminosityFunction::zero()));
                layout.push(ComponentBins {
                    first,
                    means: vec![0.0],
                    radial_means: Vec::new(),
                });
            }
        }
        LuminosityTables {
            time: self.time,
            solar_radius: self.solar_radius,
            layout,
            functions,
        }
    }

    /// Adds R06.T5.d's pair-evolved correction to a bin's finished single-star sums, at every
    /// snapshot, one [`Bins`] a layer: `binary_light`'s fitted differences, weighted by the bin's
    /// born systems in each fitted age bin then and by its metallicity nodes. It needs the bins'
    /// whole single-star sums, so it is the first part of [`assemble`](Self::assemble)'s finish
    /// step, once every node is in and before the cumulative sums are taken; applied once per bin,
    /// it keeps the bits whatever order the stages ran in.
    fn correct_for_pairs(&self, sums: &mut BinSums) {
        if self.pair_evolution == PairEvolution::Ignored {
            return;
        }
        let planned = &self.bins[sums.bin];
        let shift_now = self.time.since_epoch().as_julian_years_f64();
        for (&ago, bins) in EMITTED_AGO_YEARS.iter().zip(&mut sums.snapshots) {
            let shift = shift_now - ago;
            let weight_of = |lo: f64, hi: f64| {
                planned.ages.born_cdf(Years::new(hi - shift))
                    - planned.ages.born_cdf(Years::new(lo - shift))
            };
            for layer in binary_light::LAYERS {
                if let Some(c) = binary_light::correction(layer, &weight_of, &planned.metallicities)
                {
                    let b = &mut bins[layer_index(layer)];
                    b.pair = binary_light::apply(&c, &mut b.light, &mut b.count, &mut b.colour);
                }
            }
        }
    }

    /// Every job in turn, one stage's samples at a time: [`LuminosityTables::build_with`].
    fn run_serially(&self) -> LuminosityTables {
        let mut sums = self.bin_sums();
        for stage in self.stages() {
            let samples =
                self.track_samples(self.sample_jobs(stage).map(|job| self.run_samples(job)));
            for job in self.accumulate_jobs(stage) {
                self.run_accumulate(&samples, job, &mut sums[job.bin]);
            }
        }
        self.assemble(sums)
    }
}

/// The stages of `bins`: one metallicity at a time, the lowest \[Fe/H\] that any bin adds next,
/// with a job for each bin whose next nodes are at it. A bin's nodes ascend
/// ([`metallicity_nodes`]), so each \[Fe/H\] is one stage, and every bin adds its nodes in their
/// own order, which keeps the bits of its sums.
#[must_use]
fn stages_of(bins: &[PlannedBin]) -> Vec<PlannedStage> {
    let mut added = vec![0; bins.len()];
    let mut stages = Vec::new();
    while let Some(fe_h) = bins
        .iter()
        .zip(&added)
        .filter_map(|(bin, &k)| bin.metallicities.get(k).map(|&(fe_h, _)| fe_h))
        .min_by(f64::total_cmp)
    {
        let mut jobs = Vec::new();
        for (index, (bin, k)) in bins.iter().zip(&mut added).enumerate() {
            let first = *k;
            while bin
                .metallicities
                .get(*k)
                .is_some_and(|&(next, _)| next.total_cmp(&fe_h).is_eq())
            {
                *k += 1;
            }
            if *k > first {
                jobs.push(AccumulateJob {
                    stage: stages.len(),
                    bin: index,
                    first,
                    end: *k,
                });
            }
        }
        stages.push(PlannedStage { fe_h, jobs });
    }
    stages
}

/// The step \[Fe/H\] nodes are rounded to, dex, so that components of near metallicities share
/// their tracks.
const FE_H_STEP: f64 = 0.05;

/// The tracks' metallicity clamp, \[Fe/H\]: Z of 10⁻⁴ and 0.03 against the solar 0.02
/// ([`Composition::z_fit`]); the tracks read nothing of a composition beyond its clamped Z.
const FE_H_CLAMP: (f64, f64) = (-2.301_029_995_663_981, 0.176_091_259_055_681_24);

/// A metallicity bin of `component` as three-point Gauss–Hermite nodes and weights: the bin's
/// `mean` and the mean ± √3 σ, weighted 2/3, 1/6 and 1/6, with the component's σ at its mean age;
/// rounded to [`FE_H_STEP`] and held within [`FE_H_CLAMP`]. A star's V light is convex in
/// \[Fe/H\] (a poorer star of the same mass is hotter and brighter), so the light at the mean
/// metallicity alone falls short of the mean light: by 6% for the bulge's layer A (σ 0.4 dex).
#[must_use]
fn metallicity_nodes(component: &Component, mean: f64) -> [(f64, f64); 3] {
    let sigma = component
        .metallicity(&PointLy::new(0.0, 0.0, 0.0), component.ages().mean())
        .sigma()
        .value();
    let round = |v: f64| ((v / FE_H_STEP).round() * FE_H_STEP).clamp(FE_H_CLAMP.0, FE_H_CLAMP.1);
    let spread = 3.0_f64.sqrt() * sigma;
    [
        (round(mean - spread), 1.0 / 6.0),
        (round(mean), 2.0 / 3.0),
        (round(mean + spread), 1.0 / 6.0),
    ]
}

/// The step of mean \[Fe/H\] between a gradient component's bins, dex: a quarter dex, under which
/// a bin's light differs from the mean's by a few per cent.
const GRADIENT_BIN_STEP: f64 = 0.25;

/// The radii, in thin-disc scale lengths, at which a component's mean \[Fe/H\] is read to find
/// its range: the centre, the solar circle and beyond the disc's edge.
const GRADIENT_PROBES: [f64; 3] = [0.0, SOLAR_RADIUS_LENGTHS, 3.0 * SOLAR_RADIUS_LENGTHS];

/// The point `radius` ly out on the +y axis, in the plane.
#[must_use]
fn solar_circle(radius: f64) -> PointLy {
    PointLy::new(0.0, radius, 0.0)
}

/// A component's metallicity bins' means (see [`ComponentBins`]): its mean at the solar circle
/// alone where the mean is the same at every probe, otherwise every [`GRADIENT_BIN_STEP`] over the
/// range the probes find.
#[must_use]
fn metallicity_bins(galaxy: &Galaxy, component: &Component) -> Vec<f64> {
    let length = galaxy.params().thin_disc().length().value();
    let age = component.ages().mean();
    let means: Vec<f64> = GRADIENT_PROBES
        .iter()
        .map(|&r| {
            component
                .metallicity(&solar_circle(r * length), age)
                .mean()
                .value()
        })
        .collect();
    let (lo, hi) = means
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &m| {
            (a.min(m), b.max(m))
        });
    if hi - lo < 1e-9 {
        return vec![means[1]];
    }
    let first = (lo / GRADIENT_BIN_STEP).floor();
    let last = (hi / GRADIENT_BIN_STEP).ceil();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few quarter-dex bins"
    )]
    let count = (last - first) as usize + 1;
    (0..count)
        .map(|k| {
            #[expect(clippy::cast_precision_loss, reason = "a few bins")]
            let k = k as f64;
            (first + k) * GRADIENT_BIN_STEP
        })
        .collect()
}

/// A gradient component's mean \[Fe/H\] every [`RADIAL_STEP_LY`] to three solar radii.
#[must_use]
fn radial_means(galaxy: &Galaxy, component: &Component) -> Vec<f64> {
    let reach = GRADIENT_PROBES[2] * galaxy.params().thin_disc().length().value();
    let age = component.ages().mean();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few hundred radial steps"
    )]
    let steps = (reach / RADIAL_STEP_LY).ceil() as u32;
    (0..=steps)
        .map(|k| {
            component
                .metallicity(&solar_circle(f64::from(k) * RADIAL_STEP_LY), age)
                .mean()
                .value()
        })
        .collect()
}

/// The value of `table` (every [`RADIAL_STEP_LY`] from 0) at radius `r`, linearly.
#[must_use]
fn at_radius(table: &[f64], r: f64) -> f64 {
    let x = (r / RADIAL_STEP_LY).max(0.0);
    let floor = x.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative radius in steps, held below the table's length"
    )]
    let k = (floor as usize).min(table.len() - 1);
    if k + 1 >= table.len() {
        return table[table.len() - 1];
    }
    table[k] + (table[k + 1] - table[k]) * (x - floor)
}

/// The index of the value of ascending `values` nearest `x`.
#[must_use]
fn nearest(values: &[f64], x: f64) -> usize {
    let k = values.partition_point(|&v| v < x);
    if k == 0 {
        return 0;
    }
    if k >= values.len() {
        return values.len() - 1;
    }
    if x - values[k - 1] <= values[k] - x {
        k - 1
    } else {
        k
    }
}

/// The index of the bin that a point `radius_ly` from the axis reads, of a component's bins
/// `means` and, for a gradient component, its `radial` means ([`ComponentBins`]).
#[must_use]
fn bin_at_radius(means: &[f64], radial: &[f64], radius_ly: f64) -> usize {
    if means.len() == 1 {
        0
    } else {
        nearest(means, at_radius(radial, radius_ly))
    }
}

/// A component's bin `means` and `radial` means cut to the one bin that [`LuminosityTables::get`]
/// reads at `solar_radius_ly` ([`GradientBins::SolarCircle`]); one bin is kept as it is.
#[cfg(test)]
#[must_use]
fn solar_circle_bin(
    means: Vec<f64>,
    radial: Vec<f64>,
    solar_radius_ly: f64,
) -> (Vec<f64>, Vec<f64>) {
    if means.len() > 1 {
        let bin = bin_at_radius(&means, &radial, solar_radius_ly);
        (vec![means[bin]], Vec::new())
    } else {
        (means, radial)
    }
}

/// `layer`'s position in [`Layer::ALL`].
#[must_use]
fn layer_index(layer: Layer) -> usize {
    usize::from(layer.value())
}

/// One snapshot's bins, being filled.
#[derive(Debug, Clone, PartialEq)]
struct Bins {
    light: Vec<f64>,
    count: Vec<f64>,
    beyond_light: f64,
    beyond_count: f64,
    dark: f64,
    remnants: f64,
    colour: Vec<ColourSums>,
    beyond_colour: ColourSums,
    pair: Applied,
}

impl Bins {
    #[must_use]
    fn new() -> Self {
        Self {
            light: vec![0.0; MAGNITUDE_BINS],
            count: vec![0.0; MAGNITUDE_BINS],
            beyond_light: 0.0,
            beyond_count: 0.0,
            dark: 0.0,
            remnants: 0.0,
            colour: vec![[0.0; 4]; MAGNITUDE_BINS],
            beyond_colour: [0.0; 4],
            pair: Applied::default(),
        }
    }

    fn add(&mut self, what: Seen, weight: f64) {
        match what {
            Seen::Bin { bin, light, colour } => {
                let k = usize::from(bin);
                self.count[k] += weight;
                self.light[k] += weight * light;
                self.colour[k] = add_sums(self.colour[k], colour.map(|c| weight * light * c));
            }
            Seen::Beyond { light, colour } => {
                self.beyond_count += weight;
                self.beyond_light += weight * light;
                self.beyond_colour =
                    add_sums(self.beyond_colour, colour.map(|c| weight * light * c));
            }
            Seen::Dark => self.dark += weight,
            Seen::Remnant => {
                self.dark += weight;
                self.remnants += weight;
            }
        }
    }
}

/// What a star shows in V over one part of its life.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Seen {
    /// Its magnitude's bin, its light, L☉,V, and its colour per unit V light ([`ColourSums`]).
    Bin {
        bin: u16,
        light: f64,
        colour: ColourSums,
    },
    /// Fainter than the faintest edge, with its light and colour.
    Beyond { light: f64, colour: ColourSums },
    /// Dark in V while living (a protostar, or too cool for plan 06's photometry).
    Dark,
    /// A remnant, dark.
    Remnant,
}

impl Seen {
    #[must_use]
    pub(super) fn of(state: &StarState) -> Self {
        if state.phase().is_remnant() {
            return Self::Remnant;
        }
        let Some(m_v) = absolute_v_of_state(state) else {
            return Self::Dark;
        };
        let m = m_v.value();
        let light = math::exp10(-0.4 * (m - SOLAR_ABSOLUTE_MAGNITUDE_V));
        let x = (m - BRIGHTEST_MAGNITUDE) / MAGNITUDE_STEP;
        if x.is_nan() {
            return Self::Dark;
        }
        let star = colour_of_state(state);
        let lux = star.lux_per_v0();
        let [r, g] = star.red_green();
        let colour = [lux, lux * r, lux * g, lux * star.sp_ratio()];
        #[expect(
            clippy::cast_precision_loss,
            reason = "the number of bins, 640, is exact in f64"
        )]
        let bins = MAGNITUDE_BINS as f64;
        if x >= bins {
            return Self::Beyond { light, colour };
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "x is clamped to [0, 640), so its floor fits a u16"
        )]
        let bin = x.max(0.0).floor() as u16;
        Self::Bin { bin, light, colour }
    }
}

/// One mass node's life, cut into parts: the ages each part spans and what the star shows in it.
#[derive(Debug, Clone, PartialEq)]
struct NodeSamples {
    /// The parts' ends, ascending: part k spans `ends[k]` to `ends[k + 1]`.
    ends: Vec<f64>,
    /// What each part shows.
    seen: Vec<Seen>,
}

impl NodeSamples {
    fn heap_bytes(&self) -> usize {
        self.ends.capacity() * size_of::<f64>() + self.seen.capacity() * size_of::<Seen>()
    }

    /// Adds every part to the bins of `layers`, each `(layer, weight)` weighting the node's stars
    /// per system of the layer, and every part by its share of the born systems: the rise across
    /// its age span of `born_cdf`, the share of the systems whose stars are younger than an age at
    /// the snapshot's emitted time.
    fn accumulate(
        &self,
        born_cdf: &impl Fn(Years) -> f64,
        layers: &[(usize, f64)],
        bins: &mut [Bins],
    ) {
        // Each end is read once, for the part it closes and the part it opens: half the calls,
        // which took a young thin-disc bin's accumulation from 3.2-4.6 s to 1.8-1.9 s (test
        // profile, 2026-10-04).
        let mut ends = self.ends.iter().map(|&age| born_cdf(Years::new(age)));
        let Some(mut lo) = ends.next() else {
            return;
        };
        for (hi, &seen) in ends.zip(&self.seen) {
            let share = hi - lo;
            lo = hi;
            if share > 0.0 {
                for &(layer, weight) in layers {
                    bins[layer].add(seen, weight * share);
                }
            }
        }
    }

    /// The parts of a track's life to `max_age`.
    #[must_use]
    fn of_track(track: &Track, max_age: f64, samples_per_phase: u32) -> Self {
        let built = track.built_until().value().min(max_age);
        let mut ends = vec![0.0];
        let mut seen = Vec::new();
        let mut cuts = Vec::new();
        for (start, end, knots) in track.segment_ages() {
            let end = end.min(built);
            if end <= start || end.is_nan() {
                continue;
            }
            cuts.clear();
            cuts.extend(
                (0..=samples_per_phase)
                    .map(|k| start + (end - start) * f64::from(k) / f64::from(samples_per_phase)),
            );
            cuts.extend(knots.filter(|&age| age > start && age < end));
            cuts.sort_by(f64::total_cmp);
            cuts.dedup();
            for pair in cuts.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if ends.last().is_some_and(|&last| last < a) {
                    // A gap between segments is never expected; close it with the next part's
                    // view so that every age is covered once.
                    ends.push(a);
                    seen.push(Seen::of(&track.state_at(Years::new(f64::midpoint(a, b)))));
                }
                // Three sub-parts holding three-point Gauss–Legendre's weights, 5:8:5, each read at
                // its node: a fourth-order rule for the light of a smooth phase.
                for (&cell, &node) in GL3_CELLS.iter().zip(&GL3_NODES) {
                    ends.push(if cell >= 1.0 { b } else { a + (b - a) * cell });
                    seen.push(Seen::of(&track.state_at(Years::new(a + (b - a) * node))));
                }
            }
        }
        // A star that dies before `max_age` is a remnant from its death on: its track is built to
        // its death, and the remnant's segment, which never ends, is read past it.
        let last = ends.last().copied().unwrap_or(0.0);
        if last < max_age {
            ends.push(max_age);
            seen.push(Seen::of(
                &track.state_at(Years::new(f64::midpoint(last, max_age))),
            ));
        }
        Self { ends, seen }
    }

    /// The parts of the life of an object below 0.1 M☉, on plan 06's cooling fits, to `max_age`.
    #[must_use]
    fn of_cooling(mass: f64, composition: &Composition, max_age: f64) -> Self {
        let state_at = |age: f64| {
            substellar::cooling(SolarMasses::new(mass), Years::new(age), composition)
                .expect("a mass of 0.01-0.1 M_sun at a finite positive age is inside the fits")
        };
        let mut ends = vec![0.0, COOLING_FIRST_AGE];
        let mut seen = vec![Seen::Dark];
        let (lo, hi) = (math::ln(COOLING_FIRST_AGE), math::ln(max_age));
        for k in 1..=COOLING_SAMPLES {
            let a = math::exp(lo + (hi - lo) * f64::from(k - 1) / f64::from(COOLING_SAMPLES));
            let b = if k == COOLING_SAMPLES {
                max_age
            } else {
                math::exp(lo + (hi - lo) * f64::from(k) / f64::from(COOLING_SAMPLES))
            };
            ends.push(b);
            seen.push(Seen::of(&state_at((a * b).sqrt())));
        }
        Self { ends, seen }
    }

    /// The parts of the life of a star of initial mass `mass`.
    #[must_use]
    fn of_mass(mass: f64, composition: &Composition, max_age: f64, samples_per_phase: u32) -> Self {
        if mass < MIN_INITIAL_MASS.value() {
            Self::of_cooling(mass, composition, max_age)
        } else {
            let track = Track::to_age(
                SolarMasses::new(mass),
                composition,
                &StarDraws::median(),
                Years::new(max_age),
            );
            Self::of_track(&track, max_age, samples_per_phase)
        }
    }
}

/// A mass node of the stellar range and its weight in each layer: stars of the node per system of
/// the layer, as primaries and as companions.
#[derive(Debug, Clone, PartialEq)]
struct Node {
    mass: f64,
    weights: [f64; LAYER_COUNT],
}

/// The mass nodes of the stellar range with their layer weights, at one \[Fe/H\] node.
#[derive(Debug, Clone, PartialEq)]
struct MassGrid {
    nodes: Vec<Node>,
    /// The panels' edges, M☉, ascending.
    #[cfg(test)]
    edges: Vec<f64>,
}

/// The mass breaks every stage's panels are cut at, M☉: the mass function's and the fates'
/// breaks, and each layer's band edges with their multiples by the companion laws' mass-ratio
/// kinks.
#[must_use]
fn global_breaks(galaxy: &Galaxy) -> Vec<f64> {
    // Every population's fates have the same companions and breaks (`fates_for`).
    let fates = fates_for(Population::OldThinDisc);
    let mut breaks: Vec<f64> = galaxy
        .mass_function()
        .breaks()
        .iter()
        .chain(fates.breaks())
        .copied()
        .collect();
    for band in MassBand::ALL {
        for edge in [band.lo(), band.hi()] {
            breaks.push(edge);
            breaks.extend(COMPANION_RATIO_KINKS.iter().map(|q| q * edge));
        }
    }
    breaks
}

/// An `f64` ordered by [`f64::total_cmp`]: a memo's key, equal only to the same value bit for bit.
#[derive(Debug, Clone, Copy)]
struct Exact(f64);

impl PartialEq for Exact {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Exact {}

impl PartialOrd for Exact {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Exact {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// H(c) of [`companions_below`] across a plan's stages, by layer and ln c: it reads the mass
/// function, the companions and the edges it is integrated over alone, which the memo holds, so
/// every stage's cells share it.
#[derive(Debug, Clone, PartialEq)]
struct CompanionsMemo {
    /// The edges every H is integrated over, M☉: the global breaks' ([`global_breaks`]).
    edges: Vec<f64>,
    /// H by layer index and ln c, `None` from the band's top, where H is flat.
    values: BTreeMap<(usize, Option<Exact>), f64>,
}

impl CompanionsMemo {
    /// An empty memo over `edges`.
    #[must_use]
    fn new(edges: Vec<f64>) -> Self {
        Self {
            edges,
            values: BTreeMap::new(),
        }
    }

    /// H(c) of `band`'s primaries at c = e<sup>`ln_c`</sup> M☉, from `f` and `fates`, whose
    /// companions every population shares.
    fn below(
        &mut self,
        f: &dyn MassFunction,
        fates: &(impl StellarFates + ?Sized),
        band: MassBand,
        ln_c: f64,
    ) -> f64 {
        // From the band's top H is flat, every companion being lighter than its primary: the
        // same quadrature, bit for bit, whatever c beyond it.
        let c = math::exp(ln_c).min(band.hi());
        let key = (
            layer_index(Layer::from(band)),
            (c < band.hi()).then_some(Exact(ln_c)),
        );
        let edges = &self.edges;
        *self
            .values
            .entry(key)
            .or_insert_with(|| companions_below(f, fates, band, c, edges))
    }
}

impl MassGrid {
    /// The grid of one stage, on the panels of `edges` (M☉, [`panel_edges`]' form, split by
    /// [`ln_mass_panels`]): each panel takes `options`' nodes, and the companions' cells read H(c)
    /// through `memo`.
    #[must_use]
    fn new(
        f: &dyn MassFunction,
        options: BuildOptions,
        edges: &[f64],
        memo: &mut CompanionsMemo,
    ) -> Self {
        // Every population's fates have the same companions (`fates_for`).
        let fates = fates_for(Population::OldThinDisc);
        let panels = ln_mass_panels(edges);
        let mut nodes = Vec::with_capacity(
            panels
                .iter()
                .map(|&(lo, hi)| options.mass_nodes.rule(hi - lo).0.len())
                .sum(),
        );
        // Per panel, its companions' cells' ends: one more than its nodes.
        let mut boundaries: Vec<Vec<f64>> = Vec::with_capacity(panels.len());
        for &(lo, hi) in &panels {
            let (rule_nodes, rule_weights) = options.mass_nodes.rule(hi - lo);
            let half = 0.5 * (hi - lo);
            let mid = lo + half;
            let ln_masses: Vec<f64> = rule_nodes.iter().map(|&x| mid + half * x).collect();
            let mut cells = Vec::with_capacity(ln_masses.len() + 1);
            for (k, (&u, &w)) in ln_masses.iter().zip(rule_weights).enumerate() {
                let m = math::exp(u);
                let mut weights = [0.0; LAYER_COUNT];
                for band in MassBand::ALL {
                    if m > band.lo() && m < band.hi() {
                        let norm = f.integral(band.lo(), band.hi());
                        weights[layer_index(Layer::from(band))] = w * half * f.pdf(m) * m / norm;
                    }
                }
                nodes.push(Node { mass: m, weights });
                // The companions' cells: from the panel's start, through the midpoints between
                // nodes, to its end.
                cells.push(if k == 0 {
                    lo
                } else {
                    f64::midpoint(ln_masses[k - 1], u)
                });
            }
            cells.push(hi);
            boundaries.push(cells);
        }
        if options.companions == Companions::Included {
            for band in MassBand::ALL {
                let layer = layer_index(Layer::from(band));
                let mut below = |ln_c: f64| memo.below(f, fates, band, ln_c);
                let mut node = 0;
                for cells in &boundaries {
                    let mut previous = below(cells[0]);
                    for &end in &cells[1..] {
                        let next = below(end);
                        nodes[node].weights[layer] += next - previous;
                        previous = next;
                        node += 1;
                    }
                }
            }
        }
        Self {
            nodes,
            #[cfg(test)]
            edges: edges.to_vec(),
        }
    }
}

/// Bisections in ln m of each mass at which a stage's tracks end a phase at an age edge
/// ([`phase_end_masses`]): from a scan interval at most [`MAX_PANEL_LN_MASS`] wide to 2⁻¹⁰ of it,
/// at most 4.9 × 10⁻⁴ in ln m (some 6 × 10⁻⁵ for the Milky Way's intervals of about 0.06),
/// across which the end's age is then interpolated linearly in ln age: every end the Milky Way's
/// stages solve lies within 4 × 10⁻⁹ of its edge's age (R06.T5.e, 2026-10-05).
const PHASE_END_BISECTIONS: u32 = 10;

/// How far past an age edge a bisection's tracks are built ([`phase_end_masses`]), as a fraction
/// of the edge: far enough that both masses the last bisection leaves have reached the end, whose
/// ages there lie within some 10⁻³ of the edge, and no further, since a track's late phases cost
/// most of its build.
const PHASE_END_REACH: f64 = 0.01;

/// A place in a track's life where a stage's panels are cut ([`phase_end_masses`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseEnd {
    /// The end of a living phase, as the track labels it: the end of its last segment in it.
    Leaves(Phase),
    /// The star's death.
    Dies,
}

/// When a track reaches one of its [`PhaseEnd`]s.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EndAge {
    /// At this age since the onset of collapse.
    At(Years),
    /// After the age the track is built to.
    Later,
    /// Never: the track is built through the star's death and has no such end.
    Never,
}

impl EndAge {
    /// Whether the end has come by `age`, or `None` for an end the track never has.
    #[must_use]
    fn by(self, age: Years) -> Option<bool> {
        match self {
            Self::At(end) => Some(end <= age),
            Self::Later => Some(false),
            Self::Never => None,
        }
    }
}

/// The [`PhaseEnd`]s of one track, at the median draws.
#[derive(Debug, Clone, PartialEq)]
struct TrackEnds {
    /// Each living phase's end and its age since the onset of collapse, in the track's order,
    /// then the death if the track reaches it.
    ends: Vec<(PhaseEnd, Years)>,
    /// Whether the track is built through the star's death.
    complete: bool,
}

impl TrackEnds {
    /// The ends of the track of initial mass `mass` (held at 0.1 M☉ or above, where the tracks
    /// start) and `composition`, built to the age `reach`: a prefix of the stage's samples'
    /// tracks, bit for bit ([`Track::to_age`]).
    #[must_use]
    fn of(mass: SolarMasses, composition: &Composition, reach: Years) -> Self {
        let track = Track::to_age(
            SolarMasses::new(mass.value().max(MIN_INITIAL_MASS.value())),
            composition,
            &StarDraws::median(),
            reach,
        );
        let built = track.built_until().value();
        let mut ends: Vec<(PhaseEnd, Years)> = Vec::new();
        for (start, end, _) in track.segment_ages() {
            if !end.is_finite() {
                continue;
            }
            let phase = track
                .state_at(Years::new(f64::midpoint(start, end.min(built))))
                .phase();
            if !phase.is_living() {
                continue;
            }
            let label = PhaseEnd::Leaves(phase);
            match ends.iter_mut().find(|(l, _)| *l == label) {
                Some(slot) => slot.1 = Years::new(end),
                None => ends.push((label, Years::new(end))),
            }
        }
        let death = track.lifetime();
        if let Some(death) = death {
            ends.push((PhaseEnd::Dies, death));
        }
        Self {
            ends,
            complete: death.is_some(),
        }
    }

    /// When the track reaches `end`.
    #[must_use]
    fn age(&self, end: PhaseEnd) -> EndAge {
        match self.ends.iter().find(|(label, _)| *label == end) {
            Some(&(_, age)) => EndAge::At(age),
            None if self.complete => EndAge::Never,
            None => EndAge::Later,
        }
    }
}

/// Every [`PhaseEnd`] a track can have, in a fixed order.
fn phase_end_labels() -> impl Iterator<Item = PhaseEnd> {
    Phase::ALL
        .into_iter()
        .filter(|phase| phase.is_living())
        .map(PhaseEnd::Leaves)
        .chain(std::iter::once(PhaseEnd::Dies))
}

/// A mass at which a stage's track ends a phase at an age edge, found by [`phase_end_masses`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct SolvedEnd {
    /// The initial mass.
    mass: SolarMasses,
    /// The end the track reaches there.
    end: PhaseEnd,
    /// The age edge it reaches it at.
    age: Years,
}

/// The initial masses at which the track of `composition` (the median draws) ends a living phase
/// ([`PhaseEnd::Leaves`], as the track labels its segments) or dies at one of `ages`, each at most
/// `max_age` (R06.T5.e, decided 2026-10-05, `decision-r06-t5e-gate-2.md`).
///
/// For each end and age, the end's age is read at each of the ascending masses `scan_ln_masses`
/// (ln m, m in M☉), on
/// tracks built to twice `max_age`, and wherever it crosses the age between neighbours, the
/// crossing is bisected in ln m [`PHASE_END_BISECTIONS`] times, on tracks built [`PHASE_END_REACH`]
/// past the age, then placed by the end's ages at the two last masses, linearly in ln age. A phase
/// end that is not monotone in mass, or that a track has over only part of the range, is followed
/// wherever the scan sees it cross; a bisection that meets a track without the end gives no mass.
/// A track is a pure function of its mass and the age it is built to, so each is built once.
#[must_use]
fn phase_end_masses(
    composition: &Composition,
    ages: &[Years],
    scan_ln_masses: &[f64],
    max_age: Years,
) -> Vec<SolvedEnd> {
    let mut tracks: BTreeMap<(Exact, Exact), TrackEnds> = BTreeMap::new();
    let mut end_at = |u: f64, reach: Years, end: PhaseEnd| {
        tracks
            .entry((Exact(u), Exact(reach.value())))
            .or_insert_with(|| TrackEnds::of(SolarMasses::new(math::exp(u)), composition, reach))
            .age(end)
    };
    let scan_reach = Years::new(2.0 * max_age.value());
    let mut solved = Vec::new();
    for end in phase_end_labels() {
        for &age in ages {
            let reach = Years::new(age.value() * (1.0 + PHASE_END_REACH));
            for pair in scan_ln_masses.windows(2) {
                let (Some(lo_ended), Some(hi_ended)) = (
                    end_at(pair[0], scan_reach, end).by(age),
                    end_at(pair[1], scan_reach, end).by(age),
                ) else {
                    continue;
                };
                if lo_ended == hi_ended {
                    continue;
                }
                // The bracket's ends, each with the age its track is built to.
                let (mut lo, mut hi) = ((pair[0], scan_reach), (pair[1], scan_reach));
                let mut found = true;
                for _ in 0..PHASE_END_BISECTIONS {
                    let mid = (lo.0 + 0.5 * (hi.0 - lo.0), reach);
                    match end_at(mid.0, mid.1, end).by(age) {
                        Some(ended) if ended == lo_ended => lo = mid,
                        Some(_) => hi = mid,
                        None => {
                            found = false;
                            break;
                        }
                    }
                }
                if !found {
                    continue;
                }
                let (lo_end, hi_end) = (end_at(lo.0, lo.1, end), end_at(hi.0, hi.1, end));
                let (lo, hi) = (lo.0, hi.0);
                let ln = |t: Years| math::ln(t.value());
                let u = match (lo_end, hi_end) {
                    (EndAge::At(a), EndAge::At(b)) if ln(a).total_cmp(&ln(b)).is_ne() => {
                        let x = (ln(age) - ln(a)) / (ln(b) - ln(a));
                        lo + (hi - lo) * x.clamp(0.0, 1.0)
                    }
                    _ => lo + 0.5 * (hi - lo),
                };
                debug_assert!(u.is_finite(), "a finite ln m for {end:?} at {age:?}");
                solved.push(SolvedEnd {
                    mass: SolarMasses::new(math::exp(u)),
                    end,
                    age,
                });
            }
        }
    }
    solved
}

/// The ages at `time`, for light of age 0, at which the stars of the components of `galaxy` that
/// read the \[Fe/H\] node `fe_h` reach an edge of their ages, ascending: every component of the
/// galaxy whose bins' nodes ([`metallicity_bins`], [`metallicity_nodes`], every bin) include it,
/// whatever components a build asks for, and every edge of its ages after the epoch's zero.
#[must_use]
fn stage_ages(galaxy: &Galaxy, time: UniverseTime, fe_h: f64) -> Vec<Years> {
    // A star's age at `time` is its age at the epoch plus `shift` (`TablesPlan::run_accumulate`).
    let shift = time.since_epoch().as_julian_years_f64();
    let mut ages = Vec::new();
    for component in galaxy.fields().components() {
        let reads = metallicity_bins(galaxy, component).iter().any(|&mean| {
            metallicity_nodes(component, mean)
                .iter()
                .any(|&(node, _)| node.total_cmp(&fe_h).is_eq())
        });
        if reads {
            ages.extend(
                component
                    .ages()
                    .edges()
                    .into_iter()
                    .filter(|&age| age > 0.0)
                    .map(|age| Years::new(age + shift)),
            );
        }
    }
    ages.sort_by(Years::total_cmp);
    ages.dedup_by(|a, b| a.total_cmp(b).is_eq());
    ages
}

/// The ln m at which a stage's phase ends are scanned ([`phase_end_masses`]): the ends of the
/// panels of the global breaks, from 0.1 M☉, where the tracks start.
#[must_use]
fn phase_end_scan(global_edges: &[f64]) -> Vec<f64> {
    let floor = math::ln(MIN_INITIAL_MASS.value());
    let panels = ln_mass_panels(global_edges);
    let mut scan = vec![floor];
    scan.extend(panels.iter().map(|&(_, hi)| hi).filter(|&u| u > floor));
    scan
}

/// Each stage's mass grid (R06.T5.e): its panels cut at the global breaks ([`global_breaks`]) and
/// at the masses where the tracks at its \[Fe/H\] end a phase or die at an age edge of a component
/// that reads it ([`stage_ages`], [`phase_end_masses`]). A stage's grid is a function of the
/// galaxy, its \[Fe/H\] and `time` alone, never of the components or bins a build asks for.
#[must_use]
fn stage_grids(
    galaxy: &Galaxy,
    time: UniverseTime,
    options: BuildOptions,
    stages: &[PlannedStage],
    max_age: Years,
) -> Vec<MassGrid> {
    let breaks = global_breaks(galaxy);
    let global_edges = panel_edges(&breaks);
    let scan = phase_end_scan(&global_edges);
    let mut memo = CompanionsMemo::new(global_edges);
    stages
        .iter()
        .map(|stage| {
            let ages = stage_ages(galaxy, time, stage.fe_h);
            let ends = phase_end_masses(&composition_at(stage.fe_h), &ages, &scan, max_age);
            let masses: Vec<f64> = ends.iter().map(|end| end.mass.value()).collect();
            MassGrid::new(
                galaxy.mass_function(),
                options,
                &panel_edges(breaks.iter().chain(&masses)),
                &mut memo,
            )
        })
        .collect()
}

/// The composition at the reference metallicity `fe_h`, with no helium excess.
#[must_use]
fn composition_at(fe_h: f64) -> Composition {
    Composition::from_fe_h(crate::units::Dex::new(fe_h), HeliumExcess::ZERO)
}

/// The number of stellar companions below mass `c`, per primary of `band`: H(c) of the module
/// documentation, `∫ ξ n F(c ÷ m₁) dm₁ ÷ ∫ ξ dm₁` over the band.
#[must_use]
fn companions_below(
    f: &dyn MassFunction,
    fates: &(impl StellarFates + ?Sized),
    band: MassBand,
    c: f64,
    edges: &[f64],
) -> f64 {
    let (lo, hi) = (band.lo(), band.hi());
    let mut cuts: Vec<f64> = edges
        .iter()
        .copied()
        .filter(|&m| m > lo && m < hi)
        .collect();
    cuts.push(lo);
    cuts.push(hi);
    for q in COMPANION_RATIO_KINKS.iter().chain(&[1.0]) {
        let m1 = c / q;
        if m1 > lo && m1 < hi {
            cuts.push(m1);
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let integrand = |u: f64| {
        let m1 = math::exp(u);
        f.pdf(m1) * fates.mean_companions(m1) * companion_share_below(fates, m1, c) * m1
    };
    let mut sum = 0.0;
    for pair in cuts.windows(2) {
        sum += gl16(integrand, math::ln(pair[0]), math::ln(pair[1]));
    }
    sum / f.integral(lo, hi)
}

/// The brown dwarfs' mass nodes: even in ln m over 0.0124–0.08 M☉, each weighted by its share of
/// the mass function's substellar branch.
#[derive(Debug, Clone, PartialEq)]
struct BrownDwarfGrid {
    masses: Vec<f64>,
    weights: Vec<f64>,
}

impl BrownDwarfGrid {
    #[must_use]
    fn new(f: &dyn MassFunction) -> Self {
        let (lo, hi) = (MassBand::BrownDwarf.lo(), MassBand::BrownDwarf.hi());
        let (ln_lo, ln_hi) = (math::ln(lo), math::ln(hi));
        let total = f.substellar_integral(lo, hi);
        let steps = f64::from(BROWN_DWARF_MASS_INTERVALS);
        let edge = |k: u32| {
            if k == BROWN_DWARF_MASS_INTERVALS {
                hi
            } else {
                math::exp(ln_lo + (ln_hi - ln_lo) * f64::from(k) / steps)
            }
        };
        let intervals = usize::try_from(BROWN_DWARF_MASS_INTERVALS).expect("32 fits");
        let mut masses = Vec::with_capacity(intervals);
        let mut weights = Vec::with_capacity(intervals);
        for k in 0..BROWN_DWARF_MASS_INTERVALS {
            let (a, b) = (edge(k), edge(k + 1));
            masses.push((a * b).sqrt());
            weights.push(f.substellar_integral(a, b) / total);
        }
        Self { masses, weights }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::fates::mean_stars_per_system;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::quad::Gl16Panel;
    use crate::sky::eye::REFERENCE_SP_RATIO;
    use crate::stellar::sse::main_sequence_state;
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

    fn component_of(galaxy: &Galaxy, population: Population) -> ComponentId {
        galaxy
            .fields()
            .component_ids()
            .find(|&id| galaxy.fields().component(id).population() == population)
            .expect("the Milky Way has the population")
    }

    fn tables(population: Population, options: BuildOptions) -> (ComponentId, LuminosityTables) {
        let galaxy = milky_way_galaxy();
        let id = component_of(galaxy, population);
        (
            id,
            LuminosityTables::build_with(galaxy, UniverseTime::EPOCH, &[id], options),
        )
    }

    /// The standard build of a component's solar-circle bin alone: the one bin
    /// [`LuminosityTables::get`] reads, and the only one the tests here read.
    const SOLAR_CIRCLE: BuildOptions = BuildOptions {
        bins: GradientBins::SolarCircle,
        ..BuildOptions::STANDARD
    };

    const PRIMARIES: BuildOptions = BuildOptions {
        companions: Companions::Omitted,
        ..SOLAR_CIRCLE
    };

    #[test]
    fn layer_a_primaries_shine_as_their_main_sequence() {
        let galaxy = milky_way_galaxy();
        // The oldest of the old thin disc's sub-discs, whose M dwarfs have long left the
        // pre-main sequence (a 0.1 M☉ star contracts for some hundreds of megayears).
        let id = galaxy
            .fields()
            .component_ids()
            .filter(|&id| galaxy.fields().component(id).population() == Population::OldThinDisc)
            .max_by(|&a, &b| {
                let mean = |id| galaxy.fields().component(id).ages().mean();
                mean(a).total_cmp(&mean(b))
            })
            .expect("the Milky Way has an old thin disc");
        let tables = LuminosityTables::build_with(galaxy, UniverseTime::EPOCH, &[id], PRIMARIES);
        let table = tables.get(id, Layer::A).total_light(Span::ZERO).value();
        // A direct quadrature of the main sequence alone over 0.1–0.5 M☉, the component's ages and
        // its metallicity nodes.
        let f = galaxy.mass_function();
        let ages = galaxy.fields().component(id).ages();
        let component = galaxy.fields().component(id);
        let bins = metallicity_bins(galaxy, component);
        let solar = SOLAR_RADIUS_LENGTHS * galaxy.params().thin_disc().length().value();
        let mean = bins[nearest(&bins, at_radius(&radial_means(galaxy, component), solar))];
        let metallicities = metallicity_nodes(component, mean);
        let draws = StarDraws::median();
        let age_edges: Vec<f64> = ages.edges().into_iter().filter(|&a| a > 0.0).collect();
        let light_at = |m: f64, composition: &Composition| {
            let mut sum = 0.0;
            let mut lo = 0.0;
            for &hi in &age_edges {
                for k in 0..64 {
                    let a = lo + (hi - lo) * f64::from(k) / 64.0;
                    let b = lo + (hi - lo) * f64::from(k + 1) / 64.0;
                    let share = ages.born_cdf(Years::new(b)) - ages.born_cdf(Years::new(a));
                    let state = main_sequence_state(
                        SolarMasses::new(m),
                        composition,
                        &draws,
                        Years::new(f64::midpoint(a, b)),
                    );
                    if let Some(v) = state.as_ref().and_then(absolute_v_of_state) {
                        sum += share * math::exp10(-0.4 * (v.value() - SOLAR_ABSOLUTE_MAGNITUDE_V));
                    }
                }
                lo = hi;
            }
            sum
        };
        let direct = metallicities
            .iter()
            .map(|&(fe_h, share)| {
                let composition = composition_at(fe_h);
                share
                    * crate::galaxy::quad::gl_log_panels(
                        |m| f.pdf(m) * light_at(m, &composition),
                        &[0.1, 0.2, 0.3, 0.4, 0.5],
                    )
            })
            .sum::<f64>()
            / f.integral(MassBand::A.lo(), MassBand::A.hi());
        assert!(
            ((table - direct) / direct).abs() < 0.01,
            "table {table} against the main sequence's {direct}"
        );
    }

    #[test]
    fn doubling_the_samples_moves_no_bin_above_one_percent() {
        // A bin's light and the stars brighter than each edge move by under 1% of the function's
        // light and stars: mass nodes on the main sequence sit at one magnitude for gigayears, so
        // a bin edge beside one sees its share cross as the samples move. This checks the phase
        // sampling of the full build, T5.e's panels at 16 nodes a panel, the reference the
        // shipped nodes are measured against; their own accuracy is
        // `standard_nodes_match_the_full_build_where_the_tables_are_read`'s, on what the tables'
        // readers integrate (R06.T5.e, decided 2026-10-05).
        let full = BuildOptions {
            bins: GradientBins::SolarCircle,
            ..BuildOptions::FULL
        };
        let fine = BuildOptions {
            samples_per_phase: 2 * SAMPLES_PER_PHASE,
            ..full
        };
        let cases = [
            (
                Population::ThickDisc,
                [Layer::A, Layer::B, Layer::C, Layer::D],
            ),
            (
                Population::YoungThinDisc,
                [Layer::B, Layer::C, Layer::D, Layer::E],
            ),
        ];
        let mut worst = (0.0_f64, 0.0_f64);
        for (population, layers) in cases {
            let (id, coarse) = tables(population, full);
            let (_, finer) = tables(population, fine);
            for layer in layers {
                let (a, b) = (coarse.get(id, layer), finer.get(id, layer));
                let light = a.total_light(Span::ZERO).value();
                let stars = a.stars_per_system(Span::ZERO);
                let edge = |k: usize| {
                    #[expect(clippy::cast_precision_loss, reason = "k ≤ 640")]
                    let m = BRIGHTEST_MAGNITUDE + MAGNITUDE_STEP * k as f64;
                    Magnitudes::new(m)
                };
                for k in 0..MAGNITUDE_BINS {
                    let bin = |f: &LuminosityFunction| {
                        f.light_fainter_than(edge(k), Span::ZERO).value()
                            - f.light_fainter_than(edge(k + 1), Span::ZERO).value()
                    };
                    let (la, lb) = (bin(a), bin(b));
                    worst.0 = worst.0.max((la - lb).abs() / light);
                    assert!(
                        (la - lb).abs() <= 0.01 * light,
                        "{population:?} {layer:?} bin {k}: {la} against {lb} of {light}"
                    );
                    let (ca, cb) = (
                        a.count_brighter_than(edge(k), Span::ZERO),
                        b.count_brighter_than(edge(k), Span::ZERO),
                    );
                    worst.1 = worst.1.max((ca - cb).abs() / stars);
                    assert!(
                        (ca - cb).abs() <= 0.01 * stars,
                        "{population:?} {layer:?} edge {k}: {ca} against {cb} of {stars}"
                    );
                }
            }
        }
        eprintln!(
            "worst bin {:.3}% of its function's light, worst edge {:.3}% of its stars",
            100.0 * worst.0,
            100.0 * worst.1
        );
    }

    #[test]
    fn layer_c_has_a_post_agb_tail_brighter_than_minus_3() {
        let (id, tables) = tables(Population::ThickDisc, BuildOptions::STANDARD);
        let bright = tables
            .get(id, Layer::C)
            .count_brighter_than(Magnitudes::new(-3.0), Span::ZERO);
        assert!(bright > 0.0, "{bright}");
    }

    #[test]
    fn a_component_too_young_for_any_death_has_no_remnants() {
        // Ages of 0.5–3 Myr: younger than any lifetime, the shortest some 3.3 Myr at 150 M☉.
        let ages = crate::galaxy::ages::AgeDistribution::uniform(Years::new(5e5), Years::new(3e6))
            .unwrap();
        let born_cdf = |age: Years| ages.born_cdf(age);
        let composition = composition_at(0.0);
        let mut bins = vec![Bins::new()];
        for mass in [8.0, 20.0, 60.0, 150.0] {
            let samples = NodeSamples::of_mass(mass, &composition, 1.4e10, SAMPLES_PER_PHASE);
            samples.accumulate(&born_cdf, &[(0, 1.0)], &mut bins);
        }
        let snapshot = Snapshot::from_bins(&bins[0]);
        assert!(snapshot.remnants.abs() < 1e-300, "{}", snapshot.remnants);
        assert!(snapshot.light_fainter[0] > 0.0);
        assert!(
            (snapshot.count_brighter[MAGNITUDE_BINS] + snapshot.beyond + snapshot.dark - 4.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn interpolation_between_snapshots_matches_a_table_built_there() {
        let galaxy = milky_way_galaxy();
        let id = component_of(galaxy, Population::YoungThinDisc);
        let now = LuminosityTables::build_with(galaxy, UniverseTime::EPOCH, &[id], SOLAR_CIRCLE);
        // Half-way through the buckets 10⁴–10⁵ and 10⁵–L years.
        for half in [55_000_i64, 181_072] {
            let then_time = UniverseTime::from_julian_years(-half).unwrap();
            let then = LuminosityTables::build_with(galaxy, then_time, &[id], SOLAR_CIRCLE);
            let ago = Span::from_julian_years(half).unwrap();
            for layer in [Layer::A, Layer::C, Layer::E] {
                let (a, b) = (now.get(id, layer), then.get(id, layer));
                let light = b.total_light(Span::ZERO).value();
                let stars = b.stars_per_system(Span::ZERO);
                for m in [-8.0, -5.0, -2.0, 0.0, 2.0, 5.0, 8.0, 12.0] {
                    let m = Magnitudes::new(m);
                    let (la, lb) = (
                        a.light_fainter_than(m, ago).value(),
                        b.light_fainter_than(m, Span::ZERO).value(),
                    );
                    assert!(
                        (la - lb).abs() <= 0.01 * lb.max(1e-3 * light),
                        "{layer:?} at {m:?}, {half} yr: {la} against {lb}"
                    );
                    let (ca, cb) = (
                        a.count_brighter_than(m, ago),
                        b.count_brighter_than(m, Span::ZERO),
                    );
                    assert!(
                        (ca - cb).abs() <= 0.01 * cb.max(1e-3 * stars),
                        "{layer:?} at {m:?}, {half} yr: {ca} against {cb} stars"
                    );
                }
            }
        }
    }

    /// Every value of the tables, as bits, in their order.
    fn bits(tables: &LuminosityTables) -> Vec<u64> {
        let mut out = vec![
            tables.time.seconds().cast_unsigned(),
            hyperion_testkit::float::bits(tables.solar_radius),
        ];
        for bins in &tables.layout {
            out.push(u64::try_from(bins.first).unwrap());
            out.extend(
                bins.means
                    .iter()
                    .chain(&bins.radial_means)
                    .map(|&v| hyperion_testkit::float::bits(v)),
            );
        }
        for function in &tables.functions {
            for s in &function.snapshots {
                out.extend(
                    s.light_fainter
                        .iter()
                        .chain(&s.count_brighter)
                        .chain(s.colour_fainter.iter().flatten())
                        .chain([&s.beyond, &s.dark, &s.remnants])
                        .chain([&s.pair.light, &s.pair.sigma, &s.pair.clamped])
                        .map(|&v| hyperion_testkit::float::bits(v)),
                );
            }
        }
        out
    }

    /// The pieces of `jobs` run through `run`, handed back in the order they finish: on four
    /// threads natively, each taking every fourth job, and in turn under wasm32-wasip1, which has
    /// no threads.
    fn spread<J: Clone + Send + Sync, R: Send>(
        jobs: &[J],
        run: &(impl Fn(J) -> R + Sync),
    ) -> Vec<R> {
        #[cfg(not(target_family = "wasm"))]
        {
            let out = std::sync::Mutex::new(Vec::new());
            std::thread::scope(|scope| {
                for t in 0..4 {
                    let out = &out;
                    scope.spawn(move || {
                        for job in jobs.iter().skip(t).step_by(4) {
                            let r = run(job.clone());
                            out.lock().unwrap().push(r);
                        }
                    });
                }
            });
            out.into_inner().unwrap()
        }
        #[cfg(target_family = "wasm")]
        {
            jobs.iter().map(|job| run(job.clone())).collect()
        }
    }

    /// FNV-1a over the tables' bits: the fingerprint `parallel_build_equals_serial` pins.
    fn fingerprint(tables: &LuminosityTables) -> u64 {
        bits(tables)
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, &v| {
                v.to_le_bytes()
                    .iter()
                    .fold(h, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
            })
    }

    /// [`fingerprint`] of the halo's coarse build in `parallel_build_equals_serial`, under
    /// [`BuildOptions::STANDARD`]'s other options. Before R06.T5.d it was `0x6ca2_bafb_4c81_c7b4`,
    /// made by the serial loop before the job split (fae1c11), so neither the split nor its stages
    /// changed a bit. T5.d moved it to `0x8c44_443c_67ac_13bf` twice over: by the correction's
    /// values, and by [`bits`] hashing each snapshot's applied light, its error and its clamped
    /// light. R06.T5.e moved it to `0xe56b_4376_5fe8_1768` (decided 2026-10-05,
    /// `decision-r06-t5e-gate-2.md`): by its panel edges at the tracks' phase ends, one grid per
    /// stage, and by its node rule, [`MassNodes::Scaled`]. Generator version 21 moved it here, by
    /// plan 11's P11.T4.h alone (the early AGB's core and small-envelope remnant at SSE's τ, which
    /// the single-star tracks read; the rest of the version-21 batch leaves it). A refit of the
    /// `sky_binary_light_*` tables moves it again, as it moves [`FULL_SERIAL_FINGERPRINT`].
    const SERIAL_FINGERPRINT: u64 = 0x5c11_7d93_64a9_5339;

    /// [`fingerprint`] of the same halo build under [`BuildOptions::FULL`], the reference the
    /// shipped nodes are measured against: R06.T5.e's panels at 16 nodes a panel (decided
    /// 2026-10-05). It is new with T5.e's panel edges, which moved it from T5's
    /// `0x8c44_443c_67ac_13bf` to `0xa022_ae9e_49f0_c996`, and P11.T4.h moved it here at generator
    /// version 21, as it moved [`SERIAL_FINGERPRINT`]; a refit of the `sky_binary_light_*` tables
    /// moves it again.
    const FULL_SERIAL_FINGERPRINT: u64 = 0xb581_e39e_74e3_414c;

    /// [`fingerprint`] of the bulge's and the long bar's coarse build in
    /// `parallel_build_equals_serial`, under [`BuildOptions::STANDARD`]'s other options: their
    /// stages read the thin discs' young edges too, so it pins the search's pre-main-sequence
    /// ends and the deaths of intermediate-mass stars (some 5 M☉ at the 10⁸-year edge, which
    /// reach the white dwarf through the post-AGB crossing), which the halo's old edges do not
    /// (R06.T5.e). It moves with [`SERIAL_FINGERPRINT`]: P11.T4.h moved it from
    /// `0x9bc9_1279_f62c_9962` at generator version 21.
    const PAIR_SERIAL_FINGERPRINT: u64 = 0xffe0_8298_a09d_eb2e;

    #[test]
    fn parallel_build_equals_serial() {
        // Any partition of the sample jobs (their chunk size), any order, stages sampled ahead of
        // their turn and any threads give `build`'s bits (decided 2026-10-03), at a coarse
        // sampling and for components whose few metallicity nodes keep the test short: the halo
        // against its pinned build, under the shipped nodes and the full build's, then the bulge
        // and the long bar, pinned too, which share two of their four stages, [Fe/H] 0 and
        // +0.18, and so those stages' grids (R06.T5.e).
        let galaxy = milky_way_galaxy();
        let options = BuildOptions {
            samples_per_phase: 2,
            ..BuildOptions::STANDARD
        };
        let halo = component_of(galaxy, Population::Halo);
        let standard = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &[halo], options);
        assert_eq!(fingerprint(&standard), SERIAL_FINGERPRINT);
        let full = BuildOptions {
            samples_per_phase: 2,
            ..BuildOptions::FULL
        };
        let full = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &[halo], full);
        assert_eq!(fingerprint(&full), FULL_SERIAL_FINGERPRINT);
        let ids = [Population::Bulge, Population::LongBar].map(|p| component_of(galaxy, p));
        let serial = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &ids, options);
        assert_eq!(fingerprint(&serial), PAIR_SERIAL_FINGERPRINT);
        // Chunks of 7 mass nodes rather than the serial build's 64: every stage's sample jobs at
        // once, ahead of their turn, run reversed on four threads.
        let plan = TablesPlan::new(galaxy, REFERENCE_TIME, &ids, options, 7);
        let stages: Vec<Stage> = plan.stages().collect();
        assert!(
            stages.iter().any(|&s| plan.accumulate_jobs(s).count() > 1)
                && stages.iter().any(|&s| plan.accumulate_jobs(s).count() == 1),
            "some stages add to both bins and some to one"
        );
        let mut sample_jobs: Vec<SampleJob> =
            stages.iter().flat_map(|&s| plan.sample_jobs(s)).collect();
        sample_jobs.reverse();
        let mut chunks = spread(&sample_jobs, &|job| plan.run_samples(job));
        let mut samples = Vec::with_capacity(stages.len());
        for &stage in &stages {
            let (mine, rest): (Vec<SampleChunk>, Vec<SampleChunk>) =
                chunks.into_iter().partition(|c| c.stage() == stage);
            chunks = rest;
            samples.push(plan.track_samples(mine));
        }
        // Each stage's accumulation jobs reversed on four threads, each with its bin's sums, and
        // the sums assembled in reverse.
        let mut sums = plan.bin_sums();
        let mut jobs = Vec::new();
        for (&stage, samples) in stages.iter().zip(&samples) {
            let mut stage_jobs: Vec<(AccumulateJob, BinSums)> = plan
                .accumulate_jobs(stage)
                .map(|job| (job, sums[job.bin()].clone()))
                .collect();
            stage_jobs.reverse();
            jobs.extend(stage_jobs.iter().cloned());
            let run = |(job, mut bin): (AccumulateJob, BinSums)| {
                plan.run_accumulate(samples, job, &mut bin);
                bin
            };
            for bin in spread(&stage_jobs, &run) {
                let k = bin.bin();
                sums[k] = bin;
            }
        }
        sums.reverse();
        let parallel = plan.assemble(sums);
        assert!(bits(&parallel) == bits(&serial), "the bits differ");
        // Each accumulation job's result, from its bin's sums before it, does not depend on which
        // ran before it.
        hyperion_testkit::order::assert_order_independent(&jobs, |(job, bin)| {
            let mut bin = bin.clone();
            plan.run_accumulate(&samples[job.stage().index()], *job, &mut bin);
            bin
        });
    }

    /// A grid's masses and weights, bit for bit.
    fn grid_bits(grid: &MassGrid) -> Vec<u64> {
        grid.nodes
            .iter()
            .flat_map(|node| std::iter::once(node.mass).chain(node.weights))
            .map(hyperion_testkit::float::bits)
            .collect()
    }

    #[test]
    fn a_stages_grid_is_the_same_whatever_components_are_built() {
        // R06.T5.e (decided 2026-10-05): a stage's panels are cut where the tracks at its [Fe/H]
        // end their phases at the age edges of every component that reads it in the galaxy's full
        // plan, so a build of one component has the full build's grids, bit for bit. The bulge
        // alone and with the long bar share [Fe/H] 0 and +0.18, which the thin discs read too.
        let galaxy = milky_way_galaxy();
        let [bulge, bar] =
            [Population::Bulge, Population::LongBar].map(|p| component_of(galaxy, p));
        let alone = TablesPlan::new(
            galaxy,
            REFERENCE_TIME,
            &[bulge],
            BuildOptions::STANDARD,
            SAMPLE_JOB_NODES,
        );
        let pair = TablesPlan::new(
            galaxy,
            REFERENCE_TIME,
            &[bar, bulge],
            BuildOptions::STANDARD,
            SAMPLE_JOB_NODES,
        );
        for (stage, grid) in alone.stages.iter().zip(&alone.grids) {
            let k = pair
                .stages
                .iter()
                .position(|s| s.fe_h.total_cmp(&stage.fe_h).is_eq())
                .expect("the pair reads every metallicity of the bulge");
            let (ours, theirs) = (grid_bits(grid), grid_bits(&pair.grids[k]));
            assert_eq!(ours.len(), theirs.len(), "[Fe/H] {}", stage.fe_h);
            assert_eq!(
                ours.iter().zip(&theirs).position(|(a, b)| a != b),
                None,
                "[Fe/H] {}: the grids' first difference",
                stage.fe_h
            );
        }
        // Neither build asks for a thin disc, yet [Fe/H] 0's ages are theirs too: the old thin
        // disc's 1 Gyr edge among them.
        let shift = REFERENCE_TIME.since_epoch().as_julian_years_f64();
        let ages = stage_ages(galaxy, REFERENCE_TIME, 0.0);
        let has = |age: f64| ages.iter().any(|a| a.total_cmp(&Years::new(age)).is_eq());
        assert!(has(1e9 + shift), "the old thin disc's 1 Gyr: {ages:?}");
        assert!(has(1.2e10 + shift), "the bulge's oldest: {ages:?}");
    }

    #[test]
    fn a_stages_panels_end_where_its_tracks_end_their_phases_at_the_age_edges() {
        // R06.T5.e: every mass the search finds for the halo's [Fe/H] −1.5, at its 11 and 12 Gyr
        // edges, is a panel edge of that stage's grid, and there the track leaves its phase or
        // dies at the edge to 10⁻⁶ of its age. Each edge has the main sequence's end, the giant
        // branch's tip and the death, and the first two are where the track's own states change
        // phase.
        let galaxy = milky_way_galaxy();
        let fe_h = -1.5;
        let reads = |id: &ComponentId| {
            let component = galaxy.fields().component(*id);
            metallicity_bins(galaxy, component).iter().any(|&mean| {
                metallicity_nodes(component, mean)
                    .iter()
                    .any(|&(node, _)| node.total_cmp(&fe_h).is_eq())
            })
        };
        let halo = galaxy
            .fields()
            .component_ids()
            .find(reads)
            .expect("a halo component reads [Fe/H] −1.5");
        let plan = TablesPlan::new(
            galaxy,
            REFERENCE_TIME,
            &[halo],
            BuildOptions::STANDARD,
            SAMPLE_JOB_NODES,
        );
        let stage = plan
            .stages
            .iter()
            .position(|s| s.fe_h.total_cmp(&fe_h).is_eq())
            .expect("the halo component's stage at [Fe/H] −1.5");
        let ages = stage_ages(galaxy, REFERENCE_TIME, fe_h);
        let shift = REFERENCE_TIME.since_epoch().as_julian_years_f64();
        let bits = |ages: &[Years]| -> Vec<u64> {
            ages.iter()
                .map(|a| hyperion_testkit::float::bits(a.value()))
                .collect()
        };
        assert_eq!(
            bits(&ages),
            bits(&[Years::new(1.1e10 + shift), Years::new(1.2e10 + shift)]),
            "{ages:?}"
        );
        let composition = composition_at(fe_h);
        let scan = phase_end_scan(&panel_edges(&global_breaks(galaxy)));
        let solved = phase_end_masses(&composition, &ages, &scan, Years::new(plan.max_age));
        for &age in &ages {
            for end in [
                PhaseEnd::Leaves(Phase::MainSequence),
                PhaseEnd::Leaves(Phase::FirstGiantBranch),
                PhaseEnd::Dies,
            ] {
                assert!(
                    solved
                        .iter()
                        .any(|s| s.end == end && s.age.total_cmp(&age).is_eq()),
                    "{end:?} at {age:?}: {solved:?}"
                );
            }
        }
        let edges = &plan.grids[stage].edges;
        let reach = Years::new(2.0 * plan.max_age);
        for s in &solved {
            assert!(
                edges.iter().any(|e| e.total_cmp(&s.mass.value()).is_eq()),
                "{s:?} is no panel edge"
            );
            let at = match TrackEnds::of(s.mass, &composition, reach).age(s.end) {
                EndAge::At(at) => at,
                other => panic!("{s:?}: {other:?}"),
            };
            assert!(
                (at.value() - s.age.value()).abs() <= 1e-6 * s.age.value(),
                "{s:?}: the track's end is at {at:?}"
            );
            if let PhaseEnd::Leaves(phase @ (Phase::MainSequence | Phase::FirstGiantBranch)) = s.end
            {
                let track = Track::to_age(s.mass, &composition, &StarDraws::median(), reach);
                let phase_at = |f: f64| track.state_at(Years::new(at.value() * f)).phase();
                assert_eq!(phase_at(1.0 - 1e-9), phase, "{s:?}, just before its end");
                assert_ne!(phase_at(1.0 + 1e-9), phase, "{s:?}, just after its end");
            }
        }
    }

    #[test]
    fn companions_below_is_flat_beyond_each_bands_top() {
        // The companions' memo reads H(c) at a band's top for every c beyond it (R06.T5.e): the
        // same quadrature bit for bit, since every companion is lighter than its primary. Held
        // against the direct integral just past the top, at twice it, at the stellar range's
        // top, and at every panel edge and node beyond it of a stage's grid.
        let galaxy = milky_way_galaxy();
        let (f, fates) = (galaxy.mass_function(), fates_for(Population::OldThinDisc));
        let edges = panel_edges(&global_breaks(galaxy));
        let plan = bulge_plan();
        let grid = &plan.grids[0];
        let masses: Vec<f64> = grid
            .edges
            .iter()
            .copied()
            .chain(grid.nodes.iter().map(|node| node.mass))
            .collect();
        for band in MassBand::ALL {
            let hi = band.hi();
            let top = hyperion_testkit::float::bits(companions_below(f, fates, band, hi, &edges));
            let beyond = [
                hi * (1.0 + f64::EPSILON),
                2.0 * hi,
                crate::galaxy::imf::MASS_LIMIT_HI,
            ]
            .into_iter()
            .chain(masses.iter().copied())
            .filter(|&c| c > hi);
            for c in beyond {
                assert_eq!(
                    hyperion_testkit::float::bits(companions_below(f, fates, band, c, &edges)),
                    top,
                    "{band:?} at {c} M☉"
                );
            }
        }
    }

    /// The plan of the bulge's tables, whose one bin adds three metallicities.
    fn bulge_plan() -> TablesPlan {
        let galaxy = milky_way_galaxy();
        let id = component_of(galaxy, Population::Bulge);
        let plan = TablesPlan::new(
            galaxy,
            REFERENCE_TIME,
            &[id],
            BuildOptions::STANDARD,
            SAMPLE_JOB_NODES,
        );
        assert!(
            plan.stages().count() > 1,
            "the bulge has more than one metallicity"
        );
        plan
    }

    #[test]
    #[should_panic(expected = "every node of each of the plan's bins")]
    fn assembling_without_every_bin_panics() {
        let _ = bulge_plan().assemble(Vec::new());
    }

    #[test]
    #[should_panic(expected = "every node of each of the plan's bins")]
    fn assembling_before_every_node_is_added_panics() {
        let plan = bulge_plan();
        let _ = plan.assemble(plan.bin_sums());
    }

    #[test]
    #[should_panic(expected = "a bin adds its metallicity nodes in their order")]
    fn adding_a_bins_nodes_out_of_order_panics() {
        // The bin's last stage before its first; the check comes before any sample is read.
        let plan = bulge_plan();
        let last = plan.stages().last().expect("a stage");
        let job = plan
            .accumulate_jobs(last)
            .next()
            .expect("the bin adds its last node");
        let samples = TrackSamples {
            stage: job.stage,
            samples: Vec::new(),
        };
        let mut sums = plan.bin_sums();
        plan.run_accumulate(&samples, job, &mut sums[job.bin()]);
    }

    #[test]
    #[should_panic(expected = "the samples are of the job's stage")]
    fn adding_another_stages_samples_panics() {
        let plan = bulge_plan();
        let mut stages = plan.stages();
        let (first, second) = (stages.next().unwrap(), stages.next().unwrap());
        let job = plan
            .accumulate_jobs(first)
            .next()
            .expect("the bin adds its first node");
        let samples = TrackSamples {
            stage: second.index(),
            samples: Vec::new(),
        };
        let mut sums = plan.bin_sums();
        plan.run_accumulate(&samples, job, &mut sums[job.bin()]);
    }

    #[test]
    #[should_panic(expected = "one chunk of each of a stage's sample jobs")]
    fn one_stages_samples_take_no_other_stages_chunk() {
        // The first stage's chunks, one of them the second stage's; none is read.
        let plan = bulge_plan();
        let mut stages = plan.stages();
        let (first, second) = (stages.next().unwrap(), stages.next().unwrap());
        let mut chunks: Vec<SampleChunk> = plan
            .sample_jobs(first)
            .map(|job| SampleChunk {
                stage: job.stage,
                job: job.index,
                samples: Vec::new(),
            })
            .collect();
        chunks[0].stage = second.index();
        let _ = plan.track_samples(chunks);
    }

    #[test]
    fn tables_at_the_reference_time_read_at_the_epoch_as_a_build_there() {
        // Every edge's light and count, read at the epoch through `age_for`, within 10⁻³ of the
        // function's total light and stars of a build at the epoch (decided 2026-10-03).
        let galaxy = milky_way_galaxy();
        let id = component_of(galaxy, Population::YoungThinDisc);
        let reference = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &[id], SOLAR_CIRCLE);
        let epoch = LuminosityTables::build_with(galaxy, UniverseTime::EPOCH, &[id], SOLAR_CIRCLE);
        for years in [0_i64, 1_000, 30_000, 200_000] {
            let ago = Span::from_julian_years(years).unwrap();
            let shifted = reference.age_for(UniverseTime::EPOCH, ago);
            for layer in [Layer::A, Layer::C, Layer::D, Layer::E] {
                let (a, b) = (reference.get(id, layer), epoch.get(id, layer));
                let light = b.total_light(ago).value();
                let stars = b.stars_per_system(ago);
                for k in 0..=MAGNITUDE_BINS {
                    #[expect(clippy::cast_precision_loss, reason = "k ≤ 640")]
                    let m = Magnitudes::new(BRIGHTEST_MAGNITUDE + MAGNITUDE_STEP * k as f64);
                    let (la, lb) = (
                        a.light_fainter_than(m, shifted).value(),
                        b.light_fainter_than(m, ago).value(),
                    );
                    assert!(
                        (la - lb).abs() <= 1e-3 * light,
                        "{layer:?} edge {k}, {years} yr: {la} against {lb} of {light}"
                    );
                    let (ca, cb) = (
                        a.count_brighter_than(m, shifted),
                        b.count_brighter_than(m, ago),
                    );
                    assert!(
                        (ca - cb).abs() <= 1e-3 * stars,
                        "{layer:?} edge {k}, {years} yr: {ca} against {cb} of {stars}"
                    );
                }
            }
        }
    }

    #[test]
    fn age_for_adds_the_lead_of_the_reference_time() {
        let galaxy = milky_way_galaxy();
        let tables =
            LuminosityTables::build_with(galaxy, REFERENCE_TIME, &[], BuildOptions::STANDARD);
        let years = |y: i64| Span::from_julian_years(y).unwrap();
        let at = |y: i64| UniverseTime::from_julian_years(y).unwrap();
        assert_eq!(tables.age_for(at(0), years(500)), years(1_500));
        assert_eq!(tables.age_for(at(-1_000), Span::ZERO), years(2_000));
        assert_eq!(tables.age_for(REFERENCE_TIME, years(7)), years(7));
        // Past the reference time the light is read no younger than the tables' present.
        assert_eq!(tables.age_for(at(1_500), years(100)), Span::ZERO);
    }

    /// The stars brighter than V near the Sun from the tables and the density field alone, with no
    /// extinction: radial Gauss–Legendre panels in ln r to 30,000 ly over a Fibonacci sphere of
    /// directions.
    fn stars_brighter_than(galaxy: &Galaxy, tables: &LuminosityTables, v: &[f64]) -> Vec<f64> {
        use crate::galaxy::PointLy;
        use crate::galaxy::fields::MAX_COMPONENTS;
        const DIRECTIONS: u32 = 400;
        let sun = [0.0, 26_000.0, 68.0];
        let fields = galaxy.fields();
        let mut counts = vec![0.0; v.len()];
        let mut densities = [0.0; MAX_COMPONENTS];
        let (ln_lo, ln_hi) = (math::ln(1.0), math::ln(30_000.0));
        let panels = 24;
        let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        for d in 0..DIRECTIONS {
            let z = 1.0 - (2.0 * f64::from(d) + 1.0) / f64::from(DIRECTIONS);
            let rho = (1.0 - z * z).sqrt();
            let (sin, cos) = math::sin_cos(golden * f64::from(d));
            let dir = [rho * cos, rho * sin, z];
            for p in 0..panels {
                let lo = ln_lo + (ln_hi - ln_lo) * f64::from(p) / f64::from(panels);
                let hi = ln_lo + (ln_hi - ln_lo) * f64::from(p + 1) / f64::from(panels);
                let half = 0.5 * (hi - lo);
                for (&u, &w) in Gl16Panel::nodes(lo, hi).iter().zip(&GL16_WEIGHTS) {
                    let r = math::exp(u);
                    let point = PointLy::new(
                        sun[0] + r * dir[0],
                        sun[1] + r * dir[1],
                        sun[2] + r * dir[2],
                    );
                    fields.densities(&point, &mut densities);
                    let volume =
                        w * half * r * r * r * 4.0 * core::f64::consts::PI / f64::from(DIRECTIONS);
                    let modulus = 5.0 * math::log10(r / 3.261_563_777) - 5.0;
                    for id in fields.component_ids() {
                        let component = fields.component(id);
                        let n = densities[id.index()] * volume;
                        for band in MassBand::ALL {
                            let share = galaxy.shares().component_share(band, component);
                            let table = tables.get(id, Layer::from(band));
                            for (count, &limit) in counts.iter_mut().zip(v) {
                                *count += n
                                    * share
                                    * table.count_brighter_than(
                                        Magnitudes::new(limit - modulus),
                                        Span::ZERO,
                                    );
                            }
                        }
                    }
                }
            }
        }
        counts
    }

    #[test]
    fn star_counts_near_the_sun_rise_0_49_dex_a_magnitude() {
        let galaxy = milky_way_galaxy();
        let tables = crate::sky::testing::milky_way_tables();
        let counts = stars_brighter_than(galaxy, tables, &[5.0, 6.5]);
        let slope = (math::log10(counts[1]) - math::log10(counts[0])) / 1.5;
        // Hipparcos: 1,608 stars to V 5 and 8,874 to V 6.5 (the brainstorm's figures).
        assert!(
            (slope - 0.49).abs() < 0.1,
            "slope {slope} dex a magnitude from {counts:?}"
        );
    }

    /// The band's read, the colour sums unnormalised, is each snapshot's sum read at the
    /// magnitude and interpolated in the light's age, bit for bit, between and beyond the bins and
    /// at every light age; the colour divides those sums, and −∞ reads all of the light.
    #[test]
    fn the_colour_sums_are_the_sums_the_colour_divides() {
        let galaxy = milky_way_galaxy();
        let tables = crate::sky::testing::milky_way_tables();
        let all = Magnitudes::new(f64::NEG_INFINITY);
        for id in galaxy.fields().component_ids() {
            for layer in Layer::ALL {
                let f = tables.get(id, layer);
                for years in [0.0, 500.0, 2e3, 5e4, 3e5] {
                    let ago =
                        Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR).expect("a span");
                    for m in [-13.0, -4.321, 0.0, 7.777, 19.99, 25.0, f64::NEG_INFINITY] {
                        let sums = f.colour_sums_fainter_than(Magnitudes::new(m), ago);
                        for (i, &sum) in sums.iter().enumerate() {
                            let read = f.read(ago, |s| at_edges_by(&s.colour_fainter, m, |c| c[i]));
                            assert_same_bits(sum, read);
                        }
                        let m = Magnitudes::new(m);
                        let light = f.light_fainter_than(m, ago).value();
                        match f.colour_fainter_than(m, ago) {
                            Some(colour) => {
                                assert_same_bits(sums[0] / light, colour.lux_per_v0());
                                assert_same_bits(sums[1] / sums[0], colour.chroma()[0]);
                                assert_same_bits(sums[2] / sums[0], colour.chroma()[1]);
                                assert_same_bits(sums[3] / sums[0], colour.sp_ratio());
                            }
                            None => assert!(light <= 0.0 || sums[0] <= 0.0, "{sums:?}"),
                        }
                    }
                    assert_same_bits(
                        f.light_fainter_than(all, ago).value(),
                        f.total_light(ago).value(),
                    );
                }
            }
        }
    }

    /// The cells that carry most of `planned`'s correction error at light age `now`, each layer's
    /// part weighted by its share of `component`'s systems: where to raise the fit's systems.
    fn error_cells(
        plan: &TablesPlan,
        planned: &PlannedBin,
        now: Span,
        galaxy: &Galaxy,
        component: &Component,
    ) -> String {
        let shift = plan.time.since_epoch().as_julian_years_f64() - now.as_julian_years_f64();
        let ages = &planned.ages;
        let weight_of = |lo: f64, hi: f64| {
            ages.born_cdf(Years::new(hi - shift)) - ages.born_cdf(Years::new(lo - shift))
        };
        let mut cells = Vec::new();
        for layer in binary_light::LAYERS {
            let share = galaxy
                .shares()
                .component_share(MassBand::from(layer), component);
            for (cell, error) in
                binary_light::cell_errors(layer, &weight_of, &planned.metallicities)
            {
                cells.push((share * error, layer, cell));
            }
        }
        cells.sort_by(|a, b| b.0.total_cmp(&a.0));
        let total: f64 = cells.iter().map(|c| c.0).sum();
        cells
            .iter()
            .take(4)
            .map(|(error, layer, cell)| {
                format!(
                    "{layer:?} cell {cell} ([Fe/H] {}, age bin {}) {:.0}%",
                    binary_light::FE_H_NODES[cell / binary_light::AGE_BINS],
                    cell % binary_light::AGE_BINS,
                    100.0 * error / total
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[test]
    fn the_pair_correction_is_known_to_two_percent_and_clamps_under_half_a_percent() {
        // At T5.c's two sites, for each layer, summed over the components there by their systems:
        // the fit's born-weighted correction has a 1σ under 2% of the layer's light, and the
        // clamp at zero keeps under 0.5% of it (decided 2026-10-03, `decision-r06-tables.md`).
        // The fit's error is shared by every system a function is read for, so the components'
        // errors add linearly. Then the component guard (decided 2026-10-04, "T5.d gate
        // reading"): every component bin alone, as a site of that component, with its layers
        // weighted by their shares of its systems and folded together, as a band texel sums them.
        use crate::galaxy::fields::MAX_COMPONENTS;
        let galaxy = milky_way_galaxy();
        let tables = crate::sky::testing::milky_way_tables();
        let now = tables.age_for(UniverseTime::EPOCH, Span::ZERO);
        let mut densities = [0.0; MAX_COMPONENTS];
        let mut failures = Vec::new();
        for (site, at) in [
            ("solar circle", PointLy::new(0.0, 26_000.0, 0.0)),
            ("bulge", PointLy::new(0.0, 3_000.0, 0.0)),
        ] {
            galaxy.fields().densities(&at, &mut densities);
            for layer in binary_light::LAYERS {
                let band = MassBand::from(layer);
                let (mut light, mut pair, mut sigma, mut clamped) = (0.0, 0.0, 0.0, 0.0);
                for id in galaxy.fields().component_ids() {
                    let component = galaxy.fields().component(id);
                    let n =
                        densities[id.index()] * galaxy.shares().component_share(band, component);
                    let function = tables.get_at(id, layer, &at);
                    let (own_light, own_pair, own_sigma, own_clamped) = (
                        function.total_light(now).value(),
                        function.pair_light(now).value(),
                        function.pair_light_sigma(now).value(),
                        function.read(now, |s| s.pair.clamped),
                    );
                    eprintln!(
                        "{site} {:?} {layer:?}: {n:.3e} systems ly⁻³, light {own_light:.4e}, \
                         pair {:+.2}% ± {:.2}%, clamped {:.3}%",
                        component.population(),
                        100.0 * own_pair / (own_light - own_pair),
                        100.0 * own_sigma / own_light,
                        100.0 * own_clamped / own_light
                    );
                    light += n * own_light;
                    pair += n * own_pair;
                    sigma += n * own_sigma;
                    clamped += n * own_clamped;
                }
                eprintln!(
                    "{site} {layer:?}: pair {:+.2}% ± {:.2}%, clamped {:.3}% of the light",
                    100.0 * pair / (light - pair),
                    100.0 * sigma / light,
                    100.0 * clamped / light
                );
                if sigma >= 0.02 * light || clamped >= 0.005 * light {
                    failures.push(format!(
                        "{site} {layer:?}: σ {sigma} and clamped {clamped} of {light}"
                    ));
                }
            }
        }
        let all: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        let plan = TablesPlan::new(
            galaxy,
            REFERENCE_TIME,
            &all,
            BuildOptions::STANDARD,
            SAMPLE_JOB_NODES,
        );
        let mut planned = plan.bins.iter();
        for id in all {
            let component = galaxy.fields().component(id);
            let bins = &tables.layout[id.index()];
            for (bin, mean) in bins.means.iter().enumerate() {
                let planned = planned.next().expect("one planned bin a table bin");
                let (mut light, mut sigma, mut clamped) = (0.0, 0.0, 0.0);
                for layer in Layer::ALL {
                    if layer == Layer::RoguePlanet {
                        continue;
                    }
                    let share = galaxy
                        .shares()
                        .component_share(MassBand::from(layer), component);
                    let function =
                        &tables.functions[(bins.first + bin) * LAYER_COUNT + layer_index(layer)];
                    light += share * function.total_light(now).value();
                    sigma += share * function.pair_light_sigma(now).value();
                    clamped += share * function.read(now, |s| s.pair.clamped);
                }
                eprintln!(
                    "{:?} [Fe/H] {mean:+.2}: 1σ {:.3}%, clamped {:.4}% of its light",
                    component.population(),
                    100.0 * sigma / light,
                    100.0 * clamped / light
                );
                if sigma >= 0.02 * light || clamped >= 0.005 * light {
                    failures.push(format!(
                        "{:?} [Fe/H] {mean}: σ {sigma} and clamped {clamped} of {light}; its \
                         error is in {}",
                        component.population(),
                        error_cells(&plan, planned, now, galaxy, component)
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn the_light_of_m_dwarfs_is_redder_and_less_scotopic_than_that_of_b_stars() {
        let galaxy = milky_way_galaxy();
        let id = component_of(galaxy, Population::YoungThinDisc);
        let tables = LuminosityTables::build_with(galaxy, UniverseTime::EPOCH, &[id], SOLAR_CIRCLE);
        let all = Magnitudes::new(BRIGHTEST_MAGNITUDE);
        let dwarfs = tables
            .get(id, Layer::A)
            .colour_fainter_than(all, Span::ZERO)
            .expect("layer A shines");
        let massive = tables
            .get(id, Layer::E)
            .colour_fainter_than(all, Span::ZERO)
            .expect("layer E shines");
        assert!(dwarfs.chroma()[0] > dwarfs.chroma()[1], "{dwarfs:?}");
        assert!(
            massive.sp_ratio() > dwarfs.sp_ratio(),
            "{massive:?} {dwarfs:?}"
        );
        assert!(
            dwarfs.sp_ratio() > 0.5 && dwarfs.sp_ratio() < REFERENCE_SP_RATIO,
            "{dwarfs:?}"
        );
        assert!((dwarfs.lux_per_v0() - 1.0).abs() < 0.5, "{dwarfs:?}");
        // Nothing fainter than the faintest edge in layer E: no colour.
        assert_eq!(
            tables
                .get(id, Layer::E)
                .colour_fainter_than(Magnitudes::new(FAINTEST_MAGNITUDE + 1.0), Span::ZERO),
            None
        );
    }

    #[test]
    fn every_star_is_counted_once() {
        let galaxy = milky_way_galaxy();
        let (id, tables) = tables(Population::OldThinDisc, SOLAR_CIRCLE);
        let f = galaxy.mass_function();
        let fates = fates_for(Population::OldThinDisc);
        // Summed over the layers, weighted by their primaries, the stars per system are plan 02's.
        let mut stars = 0.0;
        let mut primaries = 0.0;
        for band in MassBand::ALL {
            let share = f.integral(band.lo(), band.hi());
            stars += share
                * tables
                    .get(id, Layer::from(band))
                    .stars_per_system(Span::ZERO);
            primaries += share;
        }
        let expected = mean_stars_per_system(f, fates);
        assert!(
            ((stars / primaries - expected) / expected).abs() < 2e-3,
            "{} against {expected}",
            stars / primaries
        );
    }

    /// The bits of every value a snapshot holds.
    fn snapshot_bits(s: &Snapshot) -> Vec<u64> {
        use hyperion_testkit::float::bits;
        s.light_fainter
            .iter()
            .chain(&s.count_brighter)
            .chain(s.colour_fainter.iter().flatten())
            .chain(&[s.beyond, s.dark, s.remnants])
            .chain(&[s.pair.light, s.pair.sigma, s.pair.clamped])
            .map(|&v| bits(v))
            .collect()
    }

    #[test]
    #[ignore = "slow: builds the young thin disc's seven metallicity bins at twice the samples"]
    fn a_solar_circle_build_is_the_full_builds_bin_bit_for_bit() {
        // The tests above read the solar circle's bin built alone (`SOLAR_CIRCLE`); this pins it to
        // that bin of a build of every bin, whose sums must not depend on the bins beside it. At
        // twice the samples it also guards the build's memory: holding every metallicity's samples
        // to the end, as the build once did, took 4.1 GB here, past wasm32's 4 GiB (2026-10-04).
        let galaxy = milky_way_galaxy();
        let id = component_of(galaxy, Population::YoungThinDisc);
        let fine = |bins| BuildOptions {
            samples_per_phase: 2 * SAMPLES_PER_PHASE,
            bins,
            ..BuildOptions::STANDARD
        };
        let all =
            LuminosityTables::build_with(galaxy, REFERENCE_TIME, &[id], fine(GradientBins::All));
        assert!(
            all.layout[id.index()].means.len() > 1,
            "the young thin disc has a radial gradient"
        );
        let solar = LuminosityTables::build_with(
            galaxy,
            REFERENCE_TIME,
            &[id],
            fine(GradientBins::SolarCircle),
        );
        for layer in Layer::ALL {
            let (a, b) = (all.get(id, layer), solar.get(id, layer));
            assert_eq!(a.snapshots.len(), b.snapshots.len(), "{layer:?}");
            for (k, (sa, sb)) in a.snapshots.iter().zip(&b.snapshots).enumerate() {
                let (x, y) = (snapshot_bits(sa), snapshot_bits(sb));
                assert_eq!(x.len(), y.len(), "{layer:?} snapshot {k}");
                let differs = x.iter().zip(&y).position(|(x, y)| x != y);
                assert_eq!(
                    differs, None,
                    "{layer:?} snapshot {k}: the first value that differs"
                );
            }
        }
    }

    /// The eye's cut near the Sun, apparent V: 7.4 + 0.45 + 0.1 (R06.T7).
    const EYE_CUT_V: f64 = 7.95;

    /// The cuts R06.T5.e's gate reads the tables at, apparent V: the eye's near the Sun, and
    /// [`MAX_CUT_V`](crate::sky::MAX_CUT_V), the deepest a camera's view may ask, to which the
    /// band and the caps are computed (Design note 5).
    const GATE_CUTS: [f64; 2] = [EYE_CUT_V, crate::sky::MAX_CUT_V];

    /// The layers that shine: every layer but the rogue planets', which are dark.
    const LIT_LAYERS: [Layer; 6] = [
        Layer::A,
        Layer::B,
        Layer::C,
        Layer::D,
        Layer::E,
        Layer::BrownDwarf,
    ];

    /// A V light, L☉,V, and its four colour sums ([`ColourSums`]), in that order.
    type LightSums = [f64; 5];

    /// The names of a [`LightSums`]' values.
    const LIGHT_SUMS: [&str; 5] = ["light", "lux", "lux r", "lux g", "lux ρ"];

    /// `f`'s V light per system fainter than `m_v` and its colour sums, for light `ago` old:
    /// [`LuminosityFunction::light_fainter_than`] and the sums behind
    /// [`LuminosityFunction::colour_fainter_than`], read as they read them.
    fn fainter(f: &LuminosityFunction, m_v: f64, ago: Span) -> LightSums {
        let (a, b, t) = f.bracket(ago);
        let at = |s: &Snapshot| -> LightSums {
            let colour = |i: usize| at_edges_by(&s.colour_fainter, m_v, |c| c[i]);
            [
                at_edges(&s.light_fainter, m_v),
                colour(0),
                colour(1),
                colour(2),
                colour(3),
            ]
        };
        let (va, vb) = (at(a), at(b));
        std::array::from_fn(|i| va[i] + (vb[i] - va[i]) * t)
    }

    /// |`a` − `b`| ÷ |`b`|, and 0 where both are 0.
    fn relative(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
    }

    /// The largest of `a`'s values' differences from `reference`'s, relative, and which value.
    fn worst_of(a: &LightSums, reference: &LightSums) -> (f64, usize) {
        a.iter()
            .zip(reference)
            .map(|(&x, &y)| relative(x, y))
            .enumerate()
            .fold(
                (0.0, 0),
                |worst, (i, d)| if d > worst.0 { (d, i) } else { worst },
            )
    }

    /// Every component bin's name and, per snapshot, its light and colour sums per system over
    /// its layers, each layer weighted by its share of the component's systems: a sky made of
    /// that component alone, as T5.d's component guard reads it.
    fn component_bin_light(
        galaxy: &Galaxy,
        tables: &LuminosityTables,
    ) -> Vec<(String, Vec<LightSums>)> {
        let mut out = Vec::new();
        for id in galaxy.fields().component_ids() {
            let component = galaxy.fields().component(id);
            let bins = &tables.layout[id.index()];
            for (bin, mean) in bins.means.iter().enumerate() {
                let mut sums = vec![[0.0; 5]; EMITTED_AGO_YEARS.len()];
                for layer in LIT_LAYERS {
                    let share = galaxy
                        .shares()
                        .component_share(MassBand::from(layer), component);
                    let function =
                        &tables.functions[(bins.first + bin) * LAYER_COUNT + layer_index(layer)];
                    for (sum, s) in sums.iter_mut().zip(&function.snapshots) {
                        let all = [s.light_fainter[0]].into_iter().chain(s.colour_fainter[0]);
                        for (v, x) in sum.iter_mut().zip(all) {
                            *v += share * x;
                        }
                    }
                }
                let name = format!("{:?} [Fe/H] {mean:+.2}", component.population());
                out.push((name, sums));
            }
        }
        out
    }

    /// R06.T5.e's G1: every component bin's light and colour sums over its layers
    /// ([`component_bin_light`]), at every snapshot, within `tolerance` of `full`'s, relative. The
    /// failures.
    fn component_light_gate(
        galaxy: &Galaxy,
        full: &LuminosityTables,
        tables: &LuminosityTables,
        tolerance: f64,
    ) -> Vec<String> {
        let mut failures = Vec::new();
        let mut worst = (0.0, String::new());
        let theirs = component_bin_light(galaxy, full);
        for ((name, ours), (_, theirs)) in component_bin_light(galaxy, tables).iter().zip(&theirs) {
            // The bin's worst light, and its worst colour sum, over the snapshots.
            let (mut light, mut colour) = (0.0_f64, (0.0, 0));
            for (s, (a, b)) in ours.iter().zip(theirs).enumerate() {
                light = light.max(relative(a[0], b[0]));
                for (i, (&x, &y)) in a.iter().zip(b).enumerate().skip(1) {
                    if relative(x, y) > colour.0 {
                        colour = (relative(x, y), i);
                    }
                }
                let (d, i) = worst_of(a, b);
                if d > tolerance {
                    failures.push(format!("G1 {name} snapshot {s} {}: {d:.5}", LIGHT_SUMS[i]));
                }
                if d > worst.0 {
                    worst = (d, format!("{name} snapshot {s} {}", LIGHT_SUMS[i]));
                }
            }
            eprintln!(
                "G1 {name}: light {:.4}%, colour {:.4}% ({})",
                100.0 * light,
                100.0 * colour.0,
                LIGHT_SUMS[colour.1]
            );
        }
        eprintln!("G1 worst {:.4}% at {}", 100.0 * worst.0, worst.1);
        failures
    }

    /// The band's points (R06.T17), ly in the galactic frame ([`GalacticPosition`](crate::coords::GalacticPosition)):
    /// near the Sun, the inner disc, the bulge 3,000 ly from the centre and the halo 15,000 ly
    /// above the Sun.
    const BAND_POINTS: [[f64; 3]; 4] = [
        [0.0, 26_000.0, 68.0],
        [0.0, 8_000.0, 0.0],
        [0.0, 3_000.0, 0.0],
        [0.0, 26_000.0, 15_000.0],
    ];

    /// The walls a texel near the Sun is read behind, ly: clear to the wall and opaque beyond. The
    /// first [`RECORDED_WALLS`] are recorded only: a texel holds under one system within them.
    const WALLS_LY: [f64; 5] = [30.0, 100.0, 300.0, 1_000.0, 3_000.0];

    /// The walls of [`WALLS_LY`] recorded but not gated.
    const RECORDED_WALLS: usize = 2;

    /// The solid angle of one texel of a 64² cube face, sr: 4π ÷ (6 × 64²).
    const TEXEL_SR: f64 = 4.0 * core::f64::consts::PI / (6.0 * 64.0 * 64.0);

    /// The band's radial quadrature from 1 ly to 120,000 ly: 16-point Gauss–Legendre panels in
    /// ln r at most 0.35 wide (0.76 mag of distance modulus), with an edge at every wall. Each
    /// node's distance, ly, and weight in ln r; and per wall, the number of nodes within it.
    fn band_nodes() -> (Vec<(f64, f64)>, [usize; WALLS_LY.len()]) {
        let edges = [1.0, 30.0, 100.0, 300.0, 1_000.0, 3_000.0, 120_000.0];
        let mut nodes = Vec::new();
        let mut within = [0; WALLS_LY.len()];
        for (k, pair) in edges.windows(2).enumerate() {
            let (lo, hi) = (math::ln(pair[0]), math::ln(pair[1]));
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the edges ascend, so the quotient is positive, and at most ln 40 ÷ 0.35 < 11"
            )]
            let panels = ((hi - lo) / 0.35).ceil() as u32;
            for p in 0..panels {
                let a = lo + (hi - lo) * f64::from(p) / f64::from(panels);
                let b = lo + (hi - lo) * f64::from(p + 1) / f64::from(panels);
                let half = 0.5 * (b - a);
                for (&u, &w) in Gl16Panel::nodes(a, b).iter().zip(&GL16_WEIGHTS) {
                    nodes.push((math::exp(u), w * half));
                }
            }
            if let Some(wall) = within.get_mut(k) {
                *wall = nodes.len();
            }
        }
        (nodes, within)
    }

    /// One ray's band from two table sets, per set ([`ray_band`]).
    #[derive(Debug, Default)]
    struct RayBand {
        /// Per set, cut ([`GATE_CUTS`]) and extinction (none, then the ray's realised profile): the
        /// light per steradian fainter than the cut less the distance modulus and the extinction,
        /// and its colour sums, to 120,000 ly.
        open: [[[LightSums; 2]; 2]; 2],
        /// Per set and cut, the same with no extinction within each wall of [`WALLS_LY`].
        walled: [[[LightSums; WALLS_LY.len()]; 2]; 2],
        /// The expected systems within each wall in one texel ([`TEXEL_SR`]) along the ray.
        systems: [f64; WALLS_LY.len()],
        /// Per set and cut, with no extinction, the light by layer ([`LIT_LAYERS`]).
        layers: [[[f64; LIT_LAYERS.len()]; 2]; 2],
    }

    /// The band along `direction` from `origin` (ly) through `sets`, at each distance the
    /// density field times each component's and layer's light fainter than the cut less the
    /// distance modulus and `extinction` at that distance, read at that distance's light age, as
    /// Design note 15's texels read them.
    fn ray_band(
        galaxy: &Galaxy,
        sets: [&LuminosityTables; 2],
        origin: [f64; 3],
        direction: [f64; 3],
        extinction: impl Fn(f64) -> f64,
        (nodes, within): &(Vec<(f64, f64)>, [usize; WALLS_LY.len()]),
    ) -> RayBand {
        use crate::galaxy::fields::MAX_COMPONENTS;
        let fields = galaxy.fields();
        let mut densities = [0.0; MAX_COMPONENTS];
        let mut band = RayBand::default();
        let mut systems = 0.0;
        let mut wall = 0;
        for (i, &(r, w)) in nodes.iter().enumerate() {
            while wall < WALLS_LY.len() && within[wall] == i {
                for (walled, open) in band.walled.iter_mut().zip(&band.open) {
                    for (walled, open) in walled.iter_mut().zip(open) {
                        walled[wall] = open[0];
                    }
                }
                band.systems[wall] = systems;
                wall += 1;
            }
            let p = PointLy::new(
                origin[0] + r * direction[0],
                origin[1] + r * direction[1],
                origin[2] + r * direction[2],
            );
            systems += fields.densities(&p, &mut densities) * w * r * r * r * TEXEL_SR;
            let ago = Span::from_seconds_f64(r * crate::units::consts::SECONDS_PER_JULIAN_YEAR)
                .unwrap_or(Span::ZERO);
            let modulus =
                5.0 * math::log10(r / (10.0 * crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC));
            let a_v = extinction(r);
            for id in fields.component_ids() {
                let rho = densities[id.index()];
                if rho <= 0.0 {
                    continue;
                }
                let component = fields.component(id);
                for (l, &layer) in LIT_LAYERS.iter().enumerate() {
                    let share = galaxy
                        .shares()
                        .component_share(MassBand::from(layer), component);
                    let n = rho * share * w * r;
                    for (t, set) in sets.iter().enumerate() {
                        let function = set.get_at(id, layer, &p);
                        let age = set.age_for(UniverseTime::EPOCH, ago);
                        for (c, &cut) in GATE_CUTS.iter().enumerate() {
                            let clear = fainter(function, cut - modulus, age);
                            let dimmed = if a_v > 0.0 {
                                fainter(function, cut - modulus - a_v, age)
                            } else {
                                clear
                            };
                            for (mode, v) in [clear, dimmed].iter().enumerate() {
                                for (sum, x) in band.open[t][c][mode].iter_mut().zip(v) {
                                    *sum += n * x;
                                }
                            }
                            band.layers[t][c][l] += n * clear[0];
                        }
                    }
                }
            }
        }
        band
    }

    /// The sums of `values` over the rays.
    fn all_sky(bands: &[RayBand], values: impl Fn(&RayBand) -> LightSums) -> LightSums {
        bands.iter().fold([0.0; 5], |sum, band| {
            let v = values(band);
            std::array::from_fn(|i| sum[i] + v[i])
        })
    }

    /// G2's open sky at one point: every ray's light and colour sums within `tolerance` of the
    /// full set's (set 1), relative, at each cut and with and without extinction; prints the
    /// all-sky figures, the worst ray and the all-sky light by layer.
    fn open_sky_gate(
        point: [f64; 3],
        bands: &[RayBand],
        tolerance: f64,
        failures: &mut Vec<String>,
    ) {
        for (c, cut) in GATE_CUTS.iter().enumerate() {
            for (mode, name) in ["no extinction", "realised extinction"].iter().enumerate() {
                let sky = |t: usize| all_sky(bands, |b| b.open[t][c][mode]);
                let (d, i) = worst_of(&sky(0), &sky(1));
                let mut worst = (0.0, 0, 0);
                for (ray, band) in bands.iter().enumerate() {
                    let (d, i) = worst_of(&band.open[0][c][mode], &band.open[1][c][mode]);
                    if d > worst.0 {
                        worst = (d, i, ray);
                    }
                    if d > tolerance {
                        failures.push(format!(
                            "G2 {point:?} cut {cut} {name} ray {ray} {}: {d:.5}",
                            LIGHT_SUMS[i]
                        ));
                    }
                }
                eprintln!(
                    "G2 {point:?} cut {cut} {name}: all-sky {:.4}% ({}), worst ray {} {:.4}% ({})",
                    100.0 * d,
                    LIGHT_SUMS[i],
                    worst.2,
                    100.0 * worst.0,
                    LIGHT_SUMS[worst.1]
                );
            }
            let layers: Vec<String> = LIT_LAYERS
                .iter()
                .enumerate()
                .map(|(l, layer)| {
                    let (a, b) = bands.iter().fold((0.0, 0.0), |(a, b), band| {
                        (a + band.layers[0][c][l], b + band.layers[1][c][l])
                    });
                    format!(
                        "{layer:?} {:+.3}%",
                        100.0 * (a - b) / b.max(f64::MIN_POSITIVE)
                    )
                })
                .collect();
            eprintln!(
                "G2 {point:?} cut {cut} by layer, no extinction: {}",
                layers.join(", ")
            );
        }
    }

    /// G2's walled texels near the Sun: per ray, the light and colour sums within each wall of
    /// [`WALLS_LY`] within the larger of 1% and a third of the texel's Poisson scatter,
    /// 1 ÷ (3 √N) for its N expected systems; the first [`RECORDED_WALLS`] are recorded only.
    fn walled_gate(bands: &[RayBand], failures: &mut Vec<String>) {
        for (c, cut) in GATE_CUTS.iter().enumerate() {
            for (w, wall) in WALLS_LY.iter().enumerate() {
                let sky = |t: usize| all_sky(bands, |b| b.walled[t][c][w]);
                let (d, _) = worst_of(&sky(0), &sky(1));
                // The ray nearest its tolerance: its difference, N and tolerance.
                let mut worst = (0.0, 0.0, 0.0, 0.0);
                let (mut least, mut most) = (f64::INFINITY, 0.0_f64);
                for (ray, band) in bands.iter().enumerate() {
                    let (diff, i) = worst_of(&band.walled[0][c][w], &band.walled[1][c][w]);
                    let n = band.systems[w];
                    let tolerance = (1.0 / (3.0 * n.sqrt())).max(0.01);
                    (least, most) = (least.min(n), most.max(n));
                    if diff / tolerance > worst.0 {
                        worst = (diff / tolerance, diff, n, tolerance);
                    }
                    if w >= RECORDED_WALLS && diff > tolerance {
                        failures.push(format!(
                            "G2 wall {wall} ly cut {cut} ray {ray} {}: {diff:.5} against {tolerance:.5} \
                             (N {n:.1})",
                            LIGHT_SUMS[i]
                        ));
                    }
                }
                eprintln!(
                    "G2 wall {wall} ly cut {cut}{}: all-sky {:.4}%, N {least:.1}-{most:.1}, \
                     nearest its tolerance {:.4}% against {:.3}% (N {:.1})",
                    if w < RECORDED_WALLS {
                        " (recorded)"
                    } else {
                        ""
                    },
                    100.0 * d,
                    100.0 * worst.1,
                    100.0 * worst.3,
                    worst.2
                );
            }
        }
    }

    /// Whether [`band_gate`] gates the walled texels near the Sun ([`walled_gate`]).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Walls {
        /// Gated, as a node cut is.
        Gated,
        /// Not read, as the reference check needs them not.
        Unread,
    }

    /// R06.T5.e's G2: the band along each of the caps' 768 rays at T17's four points
    /// ([`BAND_POINTS`]), both cuts, without extinction and through each ray's realised profile,
    /// each ray within `tolerance` of `full`'s, relative, and near the Sun behind walls
    /// ([`walled_gate`]) if `walls` asks, for `tables` against `full`. The failures.
    fn band_gate(
        galaxy: &Galaxy,
        full: &LuminosityTables,
        tables: &LuminosityTables,
        tolerance: f64,
        walls: Walls,
    ) -> Vec<String> {
        use crate::coords::GalacticPosition;
        use crate::galaxy::gas::noise::NoiseCache;
        use crate::sky::caps::{CAP_RAYS, RayExtinctions};
        let nodes = band_nodes();
        let mut failures = Vec::new();
        for (k, point) in BAND_POINTS.into_iter().enumerate() {
            let origin = GalacticPosition::from_light_years(point).expect("in the cube");
            let mut cache = NoiseCache::with_capacity(1 << 16);
            let rays = RayExtinctions::measure(galaxy, &origin, CAP_RAYS, &mut cache);
            let bands: Vec<RayBand> = rays
                .directions()
                .enumerate()
                .map(|(ray, direction)| {
                    let extinction = |r: f64| rays.along(ray, r);
                    ray_band(
                        galaxy,
                        [tables, full],
                        point,
                        direction.components(),
                        extinction,
                        &nodes,
                    )
                })
                .collect();
            open_sky_gate(point, &bands, tolerance, &mut failures);
            if k == 0 && walls == Walls::Gated {
                walled_gate(&bands, &mut failures);
            }
        }
        failures
    }

    /// The points of `caps_converge_in_rays` (R06.T7), ly in the galactic frame
    /// ([`GalacticPosition`](crate::coords::GalacticPosition)).
    const CAP_POINTS: [[f64; 3]; 6] = [
        [0.0, 26_000.0, 68.0],
        [0.0, 150.0, 0.0],
        [26_000.0, 0.0, 68.0],
        [-18_385.0, -18_385.0, 68.0],
        [0.0, 8_000.0, 0.0],
        [0.0, 26_000.0, 2_000.0],
    ];

    /// R06.T5.e's G3 (a) and (b): at [`CAP_POINTS`] and both cuts, every layer's cap from
    /// `tables` within one radial node of `full`'s, and `full`'s expected count of stars beyond
    /// it under 1.5. The failures.
    fn caps_gate(
        galaxy: &Galaxy,
        full: &LuminosityTables,
        tables: &LuminosityTables,
    ) -> Vec<String> {
        use crate::coords::GalacticPosition;
        use crate::galaxy::gas::noise::NoiseCache;
        use crate::observe::Observer;
        use crate::sky::caps::{
            CapResolution, RADIAL_STEPS_PER_DECADE, expected_beyond_caps, layer_caps,
        };
        let envelope = crate::sky::testing::milky_way_envelope();
        let mut failures = Vec::new();
        for point in CAP_POINTS {
            let at = GalacticPosition::from_light_years(point).expect("in the cube");
            let observer = Observer::new(at, UniverseTime::EPOCH).expect("an observer");
            for cut in GATE_CUTS {
                let mut cache = NoiseCache::with_capacity(1 << 16);
                let v = Magnitudes::new(cut);
                let ours = layer_caps(galaxy, tables, envelope, &observer, v, &mut cache);
                let theirs = layer_caps(galaxy, full, envelope, &observer, v, &mut cache);
                let beyond = expected_beyond_caps(
                    galaxy,
                    full,
                    envelope,
                    &observer,
                    v,
                    CapResolution::STANDARD,
                    &ours,
                    &mut cache,
                );
                for ((a, b), beyond) in ours.iter().zip(&theirs).zip(&beyond) {
                    let (ra, rb) = (a.radius().value(), b.radius().value());
                    let steps = (f64::from(RADIAL_STEPS_PER_DECADE) * math::log10(ra / rb)).round();
                    eprintln!(
                        "G3 {point:?} cut {cut} {:?}: cap {ra:.0} ly against {rb:.0} ({steps:+} \
                         nodes), beyond it {:.3} by these tables and {beyond:.3} by the full",
                        a.layer(),
                        a.expected_beyond()
                    );
                    if steps.abs() > 1.0 || *beyond >= 1.5 {
                        failures.push(format!(
                            "G3 {point:?} cut {cut} {:?}: {ra} ly against {rb}, {beyond} beyond",
                            a.layer()
                        ));
                    }
                }
            }
        }
        failures
    }

    /// R06.T5.e's G3 (c): the stars brighter than V 5 and 6.5 near the Sun
    /// ([`stars_brighter_than`]) within 1% of `full`'s. The failures.
    fn count_gate(
        galaxy: &Galaxy,
        full: &LuminosityTables,
        tables: &LuminosityTables,
    ) -> Vec<String> {
        let limits = [5.0, 6.5];
        let ours = stars_brighter_than(galaxy, tables, &limits);
        let theirs = stars_brighter_than(galaxy, full, &limits);
        let mut failures = Vec::new();
        for ((v, a), b) in limits.iter().zip(&ours).zip(&theirs) {
            let d = (a - b) / b;
            eprintln!(
                "G3 stars brighter than V {v}: {a:.1} against {b:.1} ({:+.4}%)",
                100.0 * d
            );
            if d.abs() > 0.01 {
                failures.push(format!("G3 stars brighter than V {v}: {a} against {b}"));
            }
        }
        failures
    }

    #[test]
    #[ignore = "slow: builds the full tables and the same at 32 nodes a panel"]
    fn the_full_build_matches_thirty_two_nodes_a_panel_where_the_tables_are_read() {
        // R06.T5.e's reference check (decided 2026-10-05, `decision-r06-t5e-gate-2.md`): the
        // full build, which the shipped nodes are measured against, is within a quarter of their
        // gate's 1% of twice its nodes, on G1 and the open band of G2. T5's panels failed this by
        // 1.08% in an old halo component's colour (`lux·r`; 0.78% in its light), the structure
        // the panels left unresolved.
        let galaxy = milky_way_galaxy();
        let all: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        let fine = BuildOptions {
            mass_nodes: MassNodes::ThirtyTwo,
            ..BuildOptions::FULL
        };
        let full = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &all, BuildOptions::FULL);
        let finer = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &all, fine);
        let mut failures = component_light_gate(galaxy, &finer, &full, 0.0025);
        failures.extend(band_gate(galaxy, &finer, &full, 0.0025, Walls::Unread));
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    #[ignore = "slow: builds the full and the standard tables"]
    fn standard_nodes_match_the_full_build_where_the_tables_are_read() {
        // R06.T5.e's gate (decided 2026-10-05, `decision-r06-t5e-gate.md` and `-2.md`): the
        // shipped nodes against the full build, on what the tables' readers integrate, since none
        // reads one 0.05-mag bin or one component's layer alone. G1 a sky of one component bin;
        // G2 the band's texels, open, through the realised dust and behind near walls; G3 the
        // caps and the counts.
        let galaxy = milky_way_galaxy();
        let all: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        let full = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &all, BuildOptions::FULL);
        let standard = crate::sky::testing::milky_way_tables();
        let mut failures = component_light_gate(galaxy, &full, standard, 0.01);
        failures.extend(band_gate(galaxy, &full, standard, 0.01, Walls::Gated));
        failures.extend(caps_gate(galaxy, &full, standard));
        failures.extend(count_gate(galaxy, &full, standard));
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
