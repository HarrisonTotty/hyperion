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
//! - **Masses.** 16-point Gauss–Legendre panels in ln m no wider than
//!   [`MAX_PANEL_LN_MASS`](crate::galaxy::fates::MAX_PANEL_LN_MASS), with edges at the mass
//!   function's and the fates' breaks, at each layer's band edges and their multiples by the
//!   companion laws' mass-ratio kinks, and at each mass whose lifetime is an edge of a component's
//!   ages. The primaries of a layer are its band's nodes, weighted by the mass function. The
//!   companions of a layer's primaries are spread over the same nodes: node j takes
//!   H(b<sub>j+1</sub>) − H(b<sub>j</sub>), with H(c) the number of companions below c per
//!   primary of the layer ([`StellarFates::mean_companions`] and the mass-ratio distribution, as
//!   [`CompanionMasses`](crate::galaxy::fates::CompanionMasses) integrates them) and the
//!   b<sub>j</sub> the midpoints between consecutive nodes.
//! - **Metallicity.** Each component's \[Fe/H\] distribution, read at the solar circle
//!   ([`SOLAR_RADIUS_LENGTHS`] of the galaxy's thin-disc scale lengths) and at its mean age, is
//!   taken at three-point Gauss–Hermite nodes: the mean and the mean ± √3 σ, weighted 2/3, 1/6 and
//!   1/6, since a star's V light is convex in \[Fe/H\]. A node beyond the tracks' clamp (Z =
//!   10⁻⁴–0.03, \[Fe/H\] −2.30 to +0.18) is read at the clamp, which the tracks would read anyway.
//!   A disc's radial gradient is not followed: its tables are the solar circle's everywhere.
//! - **Ages.** Each node's track ([`Track::to_age`], at each metallicity node and the median draws)
//!   is cut at every phase segment's ends and knots and
//!   into [`SAMPLES_PER_PHASE`] equal parts of each phase, and the star's V at each part's middle
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
//! Every star is treated as single: binary evolution is not in the tables (decided 2026-10-02,
//! item 3; the census lists blue stragglers itself, and R06.T5.c measures the difference).
//!
//! Nothing here draws a random word or changes generated output: the tables only read.

use std::ops::Range;

use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::Population;
use crate::galaxy::ages::AgeDistribution;
use crate::galaxy::fates::{
    StellarFates, companion_share_below, fates_for, ln_mass_panels, mass_with_lifetime, panel_edges,
};
use crate::galaxy::fields::{Component, ComponentId, SOLAR_RADIUS_LENGTHS};
use crate::galaxy::imf::{MassBand, MassFunction};
use crate::galaxy::quad::{Gl16Panel, gl16};
use crate::id::Layer;
use crate::math;
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::{MIN_INITIAL_MASS, Track};
use crate::stellar::substellar;
use crate::stellar::{Composition, StarState};
use crate::tables::gauss_legendre::GL16_WEIGHTS;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SOLAR_ABSOLUTE_MAGNITUDE_V;
use crate::units::{HeliumExcess, Magnitudes, SolarLuminositiesV, SolarMasses, Years};

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

/// The equal parts each living phase of a track is cut into (Design note 7), besides its knots.
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
        let m = m_v.value();
        let sums: ColourSums = std::array::from_fn(|i| {
            self.read(emitted_ago, |s| at_edges_by(&s.colour_fainter, m, |c| c[i]))
        });
        let light = self.read(emitted_ago, |s| at_edges(&s.light_fainter, m));
        LightColour::of_sums(sums, light)
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
    /// Whether companions are counted.
    pub(crate) companions: Companions,
    /// Which metallicity bins of a component with a radial gradient are built.
    pub(crate) bins: GradientBins,
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
    /// The options the tables are built with.
    pub(crate) const STANDARD: Self = Self {
        samples_per_phase: SAMPLES_PER_PHASE,
        companions: Companions::Included,
        bins: GradientBins::All,
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
    /// thousand tracks: a minute or more of work on one thread, which the server does once per
    /// galaxy.
    ///
    /// # Examples
    ///
    /// The light of layer C's stars fainter than M<sub>V</sub> 0 near the Sun, seen at the epoch
    /// from 500 ly away (`no_run`: a full build takes a minute or more).
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
    /// threads (the server's pool, R06.T11.c; the sim spawns none): see [`TablesPlan`].
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

/// The mass nodes of one [`SampleJob`] at most: some forty jobs per \[Fe/H\] node, each some tenths
/// of a second of tracks.
const SAMPLE_JOB_NODES: usize = 64;

/// [`LuminosityTables::build`] as jobs (decided 2026-10-03, `decision-r06-tables.md`), one
/// metallicity at a time. The plan's [`Stage`]s are its \[Fe/H\] nodes, from the lowest. Each stage
/// makes that metallicity's track samples, per chunk of mass nodes ([`SampleJob`]), put together by
/// [`track_samples`](Self::track_samples). It then adds them to every component bin that reads
/// the node ([`AccumulateJob`]), into the bin's running sums ([`BinSums`]), which the caller holds
/// from [`bin_sums`](Self::bin_sums) to [`assemble`](Self::assemble). Once its accumulation jobs
/// have run, a stage's samples are no longer needed and can be dropped.
///
/// One metallicity's samples are some 150 MiB at 32 samples a phase, and the Milky Way's tables
/// read 22 metallicities. Held to the build's end, as the split first did, they took a full build
/// to 2.7 GB. Stage by stage it peaks at 280 MiB (test profile, 2026-10-04). Within a stage only
/// the bins that read its node accumulate, 1 to 22 of the Milky Way's 53, so a pool keeps its
/// threads busy by sampling the next stage while this one accumulates. That holds two stages'
/// samples and changes no bit. On 16 threads, under shared load, it took 18.8 s and 439 MiB,
/// against 23.2 s and 237 MiB stage by stage and the first split's 18.8 s and 2.6 GB.
///
/// Every job is a pure function of the plan and its inputs. The chunks are put back in their jobs'
/// order, and every bin adds its nodes in their own order (an accumulation job panics otherwise),
/// so the tables are `build`'s, bit for bit, whatever threads run the jobs and in whatever order
/// they finish: the stages' sample jobs in any order, even ahead of their turn, and the
/// accumulation jobs of different bins in any order.
///
/// # Examples
///
/// The jobs run in turn here; a server hands each to its pool (`no_run`: a full build takes a
/// minute or more of CPU).
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
    grid: MassGrid,
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
        Self {
            time,
            options,
            solar_radius,
            grid: MassGrid::new(galaxy, options),
            brown_dwarfs: BrownDwarfGrid::new(galaxy.mass_function()),
            max_age,
            stages: stages_of(&bins),
            job_nodes: job_nodes.max(1),
            layout,
            bins,
        }
    }

    /// The mass nodes per \[Fe/H\] node: the stars', then the brown dwarfs'.
    fn nodes_per_fe_h(&self) -> usize {
        self.grid.nodes.len() + self.brown_dwarfs.masses.len()
    }

    /// The stages, in the order their accumulation jobs run: by \[Fe/H\], from the lowest.
    pub fn stages(&self) -> impl Iterator<Item = Stage> + use<> {
        (0..self.stages.len()).map(|index| Stage { index })
    }

    /// The sample jobs of `stage`, in index order: its \[Fe/H\] node's mass nodes in chunks. They
    /// may run in any order and at any time, ahead of the stage's turn too.
    pub fn sample_jobs(&self, stage: Stage) -> impl Iterator<Item = SampleJob> + use<> {
        let (per, job_nodes) = (self.nodes_per_fe_h(), self.job_nodes);
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
        let stars = self.grid.nodes.len();
        let samples = job
            .nodes
            .map(|k| {
                if k < stars {
                    NodeSamples::of_mass(
                        self.grid.nodes[k].mass,
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
        let per = self.nodes_per_fe_h();
        let stage = chunks.first().map_or(usize::MAX, |c| c.stage);
        assert!(
            stage < self.stages.len()
                && chunks.len() == per.div_ceil(self.job_nodes)
                && chunks
                    .iter()
                    .enumerate()
                    .all(|(i, c)| c.job == i && c.stage == stage),
            "one chunk of each of a stage's sample jobs"
        );
        let mut samples = Vec::with_capacity(per);
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
            samples.stage == job.stage && samples.samples.len() == self.nodes_per_fe_h(),
            "the samples are of the job's stage"
        );
        let planned = &self.bins[job.bin];
        let shift_now = self.time.since_epoch().as_julian_years_f64();
        for &(_, share) in &planned.metallicities[job.first..job.end] {
            for (&ago, snapshot) in EMITTED_AGO_YEARS.iter().zip(&mut sums.snapshots) {
                // A star's age then is its age at the epoch plus `shift`.
                let shift = shift_now - ago;
                let born_cdf = |age: Years| planned.ages.born_cdf(Years::new(age.value() - shift));
                samples.add(&self.grid, &self.brown_dwarfs, share, &born_cdf, snapshot);
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
                    let bin = sums.next().expect("the plan's bins match its layout");
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
enum Seen {
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
    fn of(state: &StarState) -> Self {
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

/// The mass nodes of the stellar range with their layer weights.
#[derive(Debug, Clone, PartialEq)]
struct MassGrid {
    nodes: Vec<Node>,
}

impl MassGrid {
    #[must_use]
    fn new(galaxy: &Galaxy, options: BuildOptions) -> Self {
        let f = galaxy.mass_function();
        // Every population's fates have the same companions (`fates_for`).
        let fates = fates_for(Population::OldThinDisc);
        let mut breaks: Vec<f64> = f.breaks().iter().chain(fates.breaks()).copied().collect();
        for band in MassBand::ALL {
            for edge in [band.lo(), band.hi()] {
                breaks.push(edge);
                breaks.extend(COMPANION_RATIO_KINKS.iter().map(|q| q * edge));
            }
        }
        for component in galaxy.fields().components() {
            let population_fates = fates_for(component.population());
            breaks.extend(
                component
                    .ages()
                    .edges()
                    .into_iter()
                    .filter(|&age| age > 0.0)
                    .filter_map(|age| mass_with_lifetime(population_fates, age)),
            );
        }
        let edges = panel_edges(&breaks);
        let panels = ln_mass_panels(&edges);
        let mut nodes = Vec::with_capacity(16 * panels.len());
        let mut boundaries = Vec::with_capacity(17 * panels.len());
        for &(lo, hi) in &panels {
            let ln_masses = Gl16Panel::nodes(lo, hi);
            let half = 0.5 * (hi - lo);
            for (k, (&u, &w)) in ln_masses.iter().zip(&GL16_WEIGHTS).enumerate() {
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
                boundaries.push(if k == 0 {
                    lo
                } else {
                    f64::midpoint(ln_masses[k - 1], u)
                });
            }
            boundaries.push(hi);
        }
        if options.companions == Companions::Included {
            for band in MassBand::ALL {
                let layer = layer_index(Layer::from(band));
                let below = |ln_c: f64| companions_below(f, fates, band, math::exp(ln_c), &edges);
                let mut node = 0;
                for (p, _) in panels.iter().enumerate() {
                    let cells = &boundaries[p * 17..p * 17 + 17];
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
        Self { nodes }
    }
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
    use super::*;
    use crate::galaxy::fates::mean_stars_per_system;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::sky::eye::REFERENCE_SP_RATIO;
    use crate::stellar::sse::main_sequence_state;

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
        // a bin edge beside one sees its share cross as the samples move.
        let fine = BuildOptions {
            samples_per_phase: 2 * SAMPLES_PER_PHASE,
            ..SOLAR_CIRCLE
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
        for (population, layers) in cases {
            let (id, coarse) = tables(population, SOLAR_CIRCLE);
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
                    assert!(
                        (la - lb).abs() <= 0.01 * light,
                        "{population:?} {layer:?} bin {k}: {la} against {lb} of {light}"
                    );
                    let (ca, cb) = (
                        a.count_brighter_than(edge(k), Span::ZERO),
                        b.count_brighter_than(edge(k), Span::ZERO),
                    );
                    assert!(
                        (ca - cb).abs() <= 0.01 * stars,
                        "{population:?} {layer:?} edge {k}: {ca} against {cb} of {stars}"
                    );
                }
            }
        }
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

    /// [`fingerprint`] of the halo's coarse build in `parallel_build_equals_serial`, made by the
    /// serial loop before the job split (fae1c11): neither the split nor its stages changed a bit.
    const SERIAL_FINGERPRINT: u64 = 0x6ca2_bafb_4c81_c7b4;

    #[test]
    fn parallel_build_equals_serial() {
        // Any partition of the sample jobs (their chunk size), any order, stages sampled ahead of
        // their turn and any threads give `build`'s bits (decided 2026-10-03), at a coarse
        // sampling and for components whose few metallicity nodes keep the test short: the halo
        // against the pre-split build, then the bulge and the long bar, which share two of their
        // four stages, [Fe/H] 0 and +0.18.
        let galaxy = milky_way_galaxy();
        let options = BuildOptions {
            samples_per_phase: 2,
            ..BuildOptions::STANDARD
        };
        let halo = component_of(galaxy, Population::Halo);
        let halo = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &[halo], options);
        assert_eq!(fingerprint(&halo), SERIAL_FINGERPRINT);
        let ids = [Population::Bulge, Population::LongBar].map(|p| component_of(galaxy, p));
        let serial = LuminosityTables::build_with(galaxy, REFERENCE_TIME, &ids, options);
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
}
