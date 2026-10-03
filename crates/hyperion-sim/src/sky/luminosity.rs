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
//! - **Time.** The ages are taken at the emitted time: a table holds snapshots at the query's time
//!   less each of [`EMITTED_AGO_YEARS`], and [`LuminosityFunction`] interpolates linearly in the
//!   light's age between them.
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

use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::Population;
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
use crate::time::{Span, UniverseTime};
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

/// The light's ages at which a table holds a snapshot, Julian years: 0, 10³, 10⁴, 10⁵ and the
/// light-crossing bound L = 2¹⁸ (Design note 7). Between them a table is linear in the light's
/// age; beyond L it holds L's.
pub const EMITTED_AGO_YEARS: [f64; 5] = [0.0, 1e3, 1e4, 1e5, 262_144.0];

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

impl BuildOptions {
    /// The options the tables are built with.
    pub(crate) const STANDARD: Self = Self {
        samples_per_phase: SAMPLES_PER_PHASE,
        companions: Companions::Included,
    };
}

/// Every component and layer of a galaxy's luminosity functions at one time (Design note 7):
/// the server builds them once per galaxy and per time bucket and caches them.
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
    /// The tables of `galaxy` for light received at `time`: each snapshot holds the stars as they
    /// were at `time` less its light age.
    ///
    /// It builds a track for each mass node at each of the galaxy's reference metallicities, some
    /// thousand tracks: seconds of work, which the server does once per galaxy and time bucket.
    #[must_use]
    pub fn build(galaxy: &Galaxy, time: UniverseTime) -> Self {
        let all: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        Self::build_with(galaxy, time, &all, BuildOptions::STANDARD)
    }

    /// [`build`](Self::build) for `components` only (the others are left dark), with `options`.
    #[must_use]
    pub(crate) fn build_with(
        galaxy: &Galaxy,
        time: UniverseTime,
        components: &[ComponentId],
        options: BuildOptions,
    ) -> Self {
        let component_count = galaxy.fields().components().len();
        let mut layout: Vec<ComponentBins> = (0..component_count)
            .map(|_| ComponentBins {
                first: 0,
                means: vec![0.0],
                radial_means: Vec::new(),
            })
            .collect();
        let mut built: Vec<Option<Vec<Vec<LuminosityFunction>>>> = vec![None; component_count];
        let grid = MassGrid::new(galaxy, options);
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
        let mut tracks: Vec<(f64, Vec<NodeSamples>)> = Vec::new();
        let brown_dwarfs = BrownDwarfGrid::new(galaxy.mass_function());
        let mut brown_dwarf_samples: Vec<(f64, Vec<NodeSamples>)> = Vec::new();
        let shift_now = time.since_epoch().as_julian_years_f64();
        for &id in components {
            let component = galaxy.fields().component(id);
            let means = metallicity_bins(galaxy, component);
            let mut by_bin = Vec::with_capacity(means.len());
            for &mean in &means {
                let metallicities = metallicity_nodes(component, mean);
                for &(fe_h, _) in &metallicities {
                    if !tracks.iter().any(|(f, _)| f.total_cmp(&fe_h).is_eq()) {
                        tracks.push((fe_h, grid.samples(fe_h, max_age, options)));
                        brown_dwarf_samples.push((fe_h, brown_dwarfs.samples(fe_h, max_age)));
                    }
                }
                let ages = component.ages();
                let mut per_layer: Vec<Vec<Snapshot>> = vec![Vec::new(); LAYER_COUNT];
                for &ago in &EMITTED_AGO_YEARS {
                    // A star's age then is its age at the epoch plus `shift`.
                    let shift = shift_now - ago;
                    let weight_of = |lo: f64, hi: f64| {
                        ages.born_cdf(Years::new(hi - shift))
                            - ages.born_cdf(Years::new(lo - shift))
                    };
                    let mut bins: Vec<Bins> = (0..LAYER_COUNT).map(|_| Bins::new()).collect();
                    let mut layers = Vec::with_capacity(LAYER_COUNT);
                    for &(fe_h, share) in &metallicities {
                        let k = tracks
                            .iter()
                            .position(|(f, _)| f.total_cmp(&fe_h).is_eq())
                            .expect("the metallicity's samples were just made");
                        let (samples, bd_samples) = (&tracks[k].1, &brown_dwarf_samples[k].1);
                        for (node, node_samples) in grid.nodes.iter().zip(samples) {
                            layers.clear();
                            layers.extend(
                                node.weights
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, w)| **w > 0.0)
                                    .map(|(layer, &w)| (layer, share * w)),
                            );
                            node_samples.accumulate(&weight_of, &layers, &mut bins);
                        }
                        let bd = layer_index(Layer::BrownDwarf);
                        for (&weight, node_samples) in brown_dwarfs.weights.iter().zip(bd_samples) {
                            node_samples.accumulate(&weight_of, &[(bd, share * weight)], &mut bins);
                        }
                    }
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
                by_bin.push(functions);
            }
            if means.len() > 1 {
                layout[id.index()].radial_means = radial_means(galaxy, component);
            }
            layout[id.index()].means = means;
            built[id.index()] = Some(by_bin);
        }
        let mut functions = Vec::new();
        for (bins, built) in layout.iter_mut().zip(built) {
            bins.first = functions.len() / LAYER_COUNT;
            match built {
                Some(by_bin) => functions.extend(by_bin.into_iter().flatten()),
                None => functions.extend((0..LAYER_COUNT).map(|_| LuminosityFunction::zero())),
            }
        }
        Self {
            time,
            solar_radius: SOLAR_RADIUS_LENGTHS * galaxy.params().thin_disc().length().value(),
            layout,
            functions,
        }
    }

    /// The time the tables were built for.
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
        let bin = if bins.means.len() == 1 {
            0
        } else {
            nearest(
                &bins.means,
                at_radius(&bins.radial_means, self.solar_radius),
            )
        };
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
        let bin = if bins.means.len() == 1 {
            0
        } else {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            nearest(&bins.means, at_radius(&bins.radial_means, r))
        };
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

/// `layer`'s position in [`Layer::ALL`].
#[must_use]
fn layer_index(layer: Layer) -> usize {
    usize::from(layer.value())
}

/// One snapshot's bins, being filled.
#[derive(Debug, Clone)]
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
    /// Adds every part to the bins of `layers`, each `(layer, weight)` weighting the node's stars
    /// per system of the layer, and every part by `weight_of` its age span.
    fn accumulate(
        &self,
        weight_of: &impl Fn(f64, f64) -> f64,
        layers: &[(usize, f64)],
        bins: &mut [Bins],
    ) {
        for (span, &seen) in self.ends.windows(2).zip(&self.seen) {
            let share = weight_of(span[0], span[1]);
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

    /// Every node's life at metallicity `fe_h`.
    fn samples(&self, fe_h: f64, max_age: f64, options: BuildOptions) -> Vec<NodeSamples> {
        let composition = composition_at(fe_h);
        self.nodes
            .iter()
            .map(|node| {
                NodeSamples::of_mass(node.mass, &composition, max_age, options.samples_per_phase)
            })
            .collect()
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

    fn samples(&self, fe_h: f64, max_age: f64) -> Vec<NodeSamples> {
        let composition = composition_at(fe_h);
        self.masses
            .iter()
            .map(|&m| NodeSamples::of_cooling(m, &composition, max_age))
            .collect()
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

    const PRIMARIES: BuildOptions = BuildOptions {
        samples_per_phase: SAMPLES_PER_PHASE,
        companions: Companions::Omitted,
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
            companions: Companions::Included,
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
            let (id, coarse) = tables(population, BuildOptions::STANDARD);
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
        let weight_of =
            |lo: f64, hi: f64| ages.born_cdf(Years::new(hi)) - ages.born_cdf(Years::new(lo));
        let composition = composition_at(0.0);
        let mut bins = vec![Bins::new()];
        for mass in [8.0, 20.0, 60.0, 150.0] {
            let samples = NodeSamples::of_mass(mass, &composition, 1.4e10, SAMPLES_PER_PHASE);
            samples.accumulate(&weight_of, &[(0, 1.0)], &mut bins);
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
        let now = LuminosityTables::build_with(
            galaxy,
            UniverseTime::EPOCH,
            &[id],
            BuildOptions::STANDARD,
        );
        // Half-way through the buckets 10⁴–10⁵ and 10⁵–L years.
        for half in [55_000_i64, 181_072] {
            let then_time = UniverseTime::from_julian_years(-half).unwrap();
            let then =
                LuminosityTables::build_with(galaxy, then_time, &[id], BuildOptions::STANDARD);
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
        let tables = LuminosityTables::build_with(
            galaxy,
            UniverseTime::EPOCH,
            &[id],
            BuildOptions::STANDARD,
        );
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
        let (id, tables) = tables(Population::OldThinDisc, BuildOptions::STANDARD);
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
}
