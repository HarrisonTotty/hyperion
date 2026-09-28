//! The displaced classes' conditional marks (plan 08, P08.T9.d and Design note 17): kind, initial
//! mass, birth component and time since death, drawn from per-class tables, the living and
//! retained records' marks of layer E ([`StayMarks`]), and the lifetime bracket of layer D
//! ([`LifetimeBracket`]).
//!
//! The class table ([`class_table`](super::class_table)) fills these tables once per galaxy from
//! its quadrature on [`MARK_MASS_NODES`] log-spaced masses per band ([`MassNodes`]). A class's mass
//! density is piecewise linear between the nodes (plan 01's [`PiecewiseLinear`]), and its birth
//! component, at a drawn mass, has the shares the nodes give, linear between them. The time since
//! death is then the component's age distribution restricted to the class's age bin after the
//! star's lifetime ([`time_since_death`]), so nothing is added to the distribution. P08.T12.c
//! draws the marks on the `displaced.*` tags; this module holds the tables and the maps from
//! uniforms to marks.

use super::{AgeBin, DisplacedKind, SPEED_BINS, SpeedBin};
use crate::galaxy::ages::AgeDistribution;
use crate::galaxy::fields::ComponentId;
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::math;
use crate::rng::{BuildPiecewiseError, Mark, PiecewiseLinear};
use crate::stellar::composition::Z_SOLAR;
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::MAX_INITIAL_MASS;
use crate::stellar::{Composition, lifetime};
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

/// The mass nodes per band of the class table's quadrature and of the marks' densities (plan 08,
/// P08.T9.a).
pub const MARK_MASS_NODES: usize = 33;

/// The kinds of a displaced system, in the order of [`ConditionalMarks`]' arrays.
pub const MARK_KINDS: [DisplacedKind; 4] = [
    DisplacedKind::Remnant,
    DisplacedKind::Runaway,
    DisplacedKind::Walkaway,
    DisplacedKind::HypervelocitySurvivor,
];

/// The index of `kind` in [`MARK_KINDS`].
#[must_use]
pub fn kind_index(kind: DisplacedKind) -> usize {
    match kind {
        DisplacedKind::Remnant => 0,
        DisplacedKind::Runaway => 1,
        DisplacedKind::Walkaway => 2,
        DisplacedKind::HypervelocitySurvivor => 3,
    }
}

/// The metallicity range the lifetime bracket spans: plan 06's fitted range, Z = 0.0001–0.03.
pub const BRACKET_Z_RANGE: (f64, f64) = (1e-4, 0.03);

/// The metallicities the bracket samples, log-spaced over [`BRACKET_Z_RANGE`].
pub const BRACKET_Z_SAMPLES: usize = 13;

/// The intervals between the bracket's metallicity samples.
const BRACKET_Z_SPAN: f64 = 12.0;
const _: () = assert!(BRACKET_Z_SAMPLES == 13);

/// The bracket's widening: its least lifetime is divided and its greatest multiplied by
/// `1 + BRACKET_WIDENING` (plan 08, P08.T9.d: "widened by a tenth").
pub const BRACKET_WIDENING: f64 = 0.1;

/// The 33 log-spaced masses of one band and their quadrature weights under a mass function
/// (plan 08, P08.T9.a).
///
/// The weights are Simpson's rule in ln m on `pdf(m) m`, normalised to sum to 1: the weight of
/// node i is the share of the band's systems the node stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct MassNodes {
    band: MassBand,
    masses: [f64; MARK_MASS_NODES],
    weights: [f64; MARK_MASS_NODES],
    pdf: [f64; MARK_MASS_NODES],
}

impl MassNodes {
    /// The nodes of `band` under the mass function `mf`.
    ///
    /// # Panics
    ///
    /// If the mass function has no weight in the band, which no function of plan 02 has.
    #[must_use]
    pub fn of(band: MassBand, mf: &dyn MassFunction) -> Self {
        let masses = band_masses(band);
        let pdf = masses.map(|m| mf.pdf(m));
        let mut weights = [0.0; MARK_MASS_NODES];
        let last = MARK_MASS_NODES - 1;
        for (i, w) in weights.iter_mut().enumerate() {
            let simpson = if i == 0 || i == last {
                1.0
            } else if i % 2 == 1 {
                4.0
            } else {
                2.0
            };
            *w = simpson * pdf[i] * masses[i];
        }
        let total: f64 = weights.iter().sum();
        assert!(total > 0.0, "a mass function with no systems in {band:?}");
        Self {
            band,
            masses,
            weights: weights.map(|w| w / total),
            pdf,
        }
    }

    /// The band.
    #[must_use]
    pub fn band(&self) -> MassBand {
        self.band
    }

    /// The masses, M☉, from the band's lower edge to its upper.
    #[must_use]
    pub fn masses(&self) -> &[f64; MARK_MASS_NODES] {
        &self.masses
    }

    /// The quadrature weights, summing to 1.
    #[must_use]
    pub fn weights(&self) -> &[f64; MARK_MASS_NODES] {
        &self.weights
    }

    /// The mass function's density at each node, per M☉ (unnormalised over the band).
    #[must_use]
    pub fn pdf(&self) -> &[f64; MARK_MASS_NODES] {
        &self.pdf
    }
}

/// The intervals between the mass nodes.
const NODE_SPAN: f64 = 32.0;
const _: () = assert!(MARK_MASS_NODES == 33);

/// The 33 masses, M☉, log-spaced over `band`'s edges.
#[must_use]
pub(crate) fn band_masses(band: MassBand) -> [f64; MARK_MASS_NODES] {
    let (lo, hi) = (
        MASS_BAND_EDGES[band.index()],
        MASS_BAND_EDGES[band.index() + 1],
    );
    let step = math::ln(hi / lo) / NODE_SPAN;
    let mut masses = [hi; MARK_MASS_NODES];
    let mut k = 0.0;
    for m in masses.iter_mut().take(MARK_MASS_NODES - 1) {
        *m = lo * math::exp(k * step);
        k += 1.0;
    }
    masses
}

/// The segment `[i, i + 1]` of `masses` holding `m`, and the linear weight of node `i + 1`,
/// clamped to the ends.
fn locate(masses: &[f64; MARK_MASS_NODES], m: f64) -> (usize, f64) {
    let i = masses
        .partition_point(|&x| x <= m)
        .saturating_sub(1)
        .min(MARK_MASS_NODES - 2);
    let t = ((m - masses[i]) / (masses[i + 1] - masses[i])).clamp(0.0, 1.0);
    (i, t)
}

/// The conditional marks of one displaced class in one band: its kinds' odds, each kind's mass
/// density on the nodes, the birth components' shares at each node, and, for the fastest class of
/// an age bin, the odds of the speed bin its kick was drawn in (plan 08, Design notes 17 and 23).
///
/// The densities are held per kind, component and node, in systems per M☉ (unnormalised): a
/// kind's mass density is their sum over components, and a component's share at a mass is its
/// part of that sum, each linear in mass between the nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalMarks {
    masses: [f64; MARK_MASS_NODES],
    components: Vec<ComponentId>,
    /// `[kind][component][node]`, systems per M☉.
    density: [Vec<[f64; MARK_MASS_NODES]>; 4],
    /// The class's weight of each kind, from the quadrature.
    kind_weight: [f64; 4],
    /// The weight by the speed bin the kick was drawn in.
    origin: [f64; SPEED_BINS],
}

impl ConditionalMarks {
    /// Empty marks over `masses` for a class born in `components`.
    #[must_use]
    pub(crate) fn empty(masses: &[f64; MARK_MASS_NODES], components: Vec<ComponentId>) -> Self {
        let n = components.len();
        Self {
            masses: *masses,
            density: core::array::from_fn(|_| vec![[0.0; MARK_MASS_NODES]; n]),
            components,
            kind_weight: [0.0; 4],
            origin: [0.0; SPEED_BINS],
        }
    }

    /// Adds `weight` (a share of the source's band budget) of `kind` at node `node`, born in the
    /// source's component number `component`, whose kick was drawn in speed bin `origin`;
    /// `density` is the same contribution per M☉.
    pub(crate) fn add(
        &mut self,
        kind: DisplacedKind,
        component: usize,
        node: usize,
        origin: usize,
        weight: f64,
        density: f64,
    ) {
        let k = kind_index(kind);
        self.density[k][component][node] += density;
        self.kind_weight[k] += weight;
        self.origin[origin] += weight;
    }

    /// The class's total weight, a share of its source's band budget.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.kind_weight.iter().sum()
    }

    /// The odds of each kind, in [`MARK_KINDS`] order, summing to 1; all zero for an empty class.
    #[must_use]
    pub fn kind_odds(&self) -> [f64; 4] {
        let total = self.total();
        if total > 0.0 {
            self.kind_weight.map(|w| w / total)
        } else {
            [0.0; 4]
        }
    }

    /// The odds of the speed bin a member's kick was drawn in: its own bin, except for the
    /// fastest class of an age bin, which also holds the unbound of every bin (Design note 23).
    #[must_use]
    pub fn origin_odds(&self) -> [f64; SPEED_BINS] {
        let total: f64 = self.origin.iter().sum();
        if total > 0.0 {
            self.origin.map(|w| w / total)
        } else {
            [0.0; SPEED_BINS]
        }
    }

    /// The mass nodes, M☉.
    #[must_use]
    pub fn masses(&self) -> &[f64; MARK_MASS_NODES] {
        &self.masses
    }

    /// The birth components, in the order of their densities.
    #[must_use]
    pub fn components(&self) -> &[ComponentId] {
        &self.components
    }

    /// `kind`'s mass density at each node, systems per M☉ (unnormalised).
    #[must_use]
    pub fn node_density(&self, kind: DisplacedKind) -> [f64; MARK_MASS_NODES] {
        let mut out = [0.0; MARK_MASS_NODES];
        for row in &self.density[kind_index(kind)] {
            for (o, d) in out.iter_mut().zip(row) {
                *o += d;
            }
        }
        out
    }

    /// `kind`'s density of component number `component` (its place in
    /// [`components`](Self::components)) at each node, systems per M☉.
    ///
    /// # Panics
    ///
    /// If `component` is not below the number of components.
    #[must_use]
    pub fn component_density(
        &self,
        kind: DisplacedKind,
        component: usize,
    ) -> &[f64; MARK_MASS_NODES] {
        &self.density[kind_index(kind)][component]
    }

    /// `kind`'s mass density as a sampler over the band, linear between the nodes.
    ///
    /// # Errors
    ///
    /// [`BuildPiecewiseError::ZeroTotal`] if the class holds none of `kind`.
    pub fn mass_density(
        &self,
        kind: DisplacedKind,
    ) -> Result<PiecewiseLinear, BuildPiecewiseError> {
        PiecewiseLinear::new(&self.masses, &self.node_density(kind))
    }

    /// The birth component of a member of `kind` of mass `m` picked by `mark`: the components'
    /// shares at `m`, linear between the nodes, in their order, by plan 01's integer thresholds
    /// ([`Mark::pick_weighted`]); the last component if rounding leaves the mark past every one.
    ///
    /// # Panics
    ///
    /// If the class holds no component, which a built class never does.
    #[must_use]
    pub fn component_at(&self, kind: DisplacedKind, m: SolarMasses, mark: Mark) -> ComponentId {
        let rows = &self.density[kind_index(kind)];
        let (i, t) = locate(&self.masses, m.value());
        let values: Vec<f64> = rows
            .iter()
            .map(|row| (row[i] + t * (row[i + 1] - row[i])).max(0.0))
            .collect();
        let total = values.iter().fold(0.0, |sum, &v| sum + v);
        let last = self
            .components
            .len()
            .checked_sub(1)
            .expect("a built class has at least one component");
        if total <= 0.0 {
            return self.components[last];
        }
        self.components[mark.pick_weighted(&values, total).unwrap_or(last)]
    }
}

/// What a field component's layer-E record is (plan 08, Design note 17): alive, or a remnant
/// retained in the field with its kick in a speed bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StayCategory {
    /// A living star, less the runaways and walkaways, which are displaced classes.
    Alive,
    /// A retained remnant, its kick in the bin.
    Retained(SpeedBin),
}

impl StayCategory {
    /// The category's place in [`StayMarks::odds`] and [`StayMarks::weights`]: 0 for alive, `1 +
    /// s` for retained in speed bin `s`.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Alive => 0,
            Self::Retained(bin) => 1 + bin.index(),
        }
    }
}

/// The marks of a field component's layer-E records (plan 08, Design note 17): the odds of alive
/// and of retained by speed bin, and each one's mass density on the nodes.
///
/// Category 0 is alive (less the runaways and walkaways, which are displaced classes); category
/// `1 + s` is retained with a kick in speed bin `s`.
#[derive(Debug, Clone, PartialEq)]
pub struct StayMarks {
    masses: [f64; MARK_MASS_NODES],
    /// `[category][node]`, systems per M☉ (unnormalised).
    density: [[f64; MARK_MASS_NODES]; 1 + SPEED_BINS],
    weight: [f64; 1 + SPEED_BINS],
}

impl StayMarks {
    /// Empty marks over `masses`.
    #[must_use]
    pub(crate) fn empty(masses: &[f64; MARK_MASS_NODES]) -> Self {
        Self {
            masses: *masses,
            density: [[0.0; MARK_MASS_NODES]; 1 + SPEED_BINS],
            weight: [0.0; 1 + SPEED_BINS],
        }
    }

    /// Adds `weight` (a share of the component's band budget) to `category` at `node`, `density`
    /// per M☉.
    pub(crate) fn add(&mut self, category: StayCategory, node: usize, weight: f64, density: f64) {
        self.density[category.index()][node] += density;
        self.weight[category.index()] += weight;
    }

    /// The component's stay share: alive and retained, a share of its band budget.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.weight.iter().sum()
    }

    /// The share of each category, a share of the component's band budget, at
    /// [`StayCategory::index`].
    #[must_use]
    pub fn weights(&self) -> &[f64; 1 + SPEED_BINS] {
        &self.weight
    }

    /// The odds of each category at [`StayCategory::index`], summing to 1.
    #[must_use]
    pub fn odds(&self) -> [f64; 1 + SPEED_BINS] {
        let total = self.total();
        if total > 0.0 {
            self.weight.map(|w| w / total)
        } else {
            [0.0; 1 + SPEED_BINS]
        }
    }

    /// The mass nodes, M☉.
    #[must_use]
    pub fn masses(&self) -> &[f64; MARK_MASS_NODES] {
        &self.masses
    }

    /// `category`'s mass density at each node, systems per M☉.
    #[must_use]
    pub fn node_density(&self, category: StayCategory) -> &[f64; MARK_MASS_NODES] {
        &self.density[category.index()]
    }

    /// `category`'s mass density as a sampler over the band.
    ///
    /// # Errors
    ///
    /// [`BuildPiecewiseError::ZeroTotal`] if the category is empty.
    pub fn mass_density(
        &self,
        category: StayCategory,
    ) -> Result<PiecewiseLinear, BuildPiecewiseError> {
        PiecewiseLinear::new(&self.masses, &self.density[category.index()])
    }
}

/// The age-bin edges of `bin`, years, for a time unit of `tau_unit` years: `(lo, hi)`, `hi` `None`
/// for the last, unbounded bin.
#[must_use]
pub fn age_bin_years(bin: AgeBin, tau_unit: Years) -> (Years, Option<Years>) {
    let (edges, bin) = (super::AGE_EDGES, bin.index());
    let lo = if bin == 0 { 0.0 } else { edges[bin - 1] };
    let hi = edges.get(bin).copied();
    (
        Years::new(lo * tau_unit.value()),
        hi.map(|h| Years::new(h * tau_unit.value())),
    )
}

/// The time since death of a star of `lifetime` whose component's ages are `ages`, restricted to
/// `[lo, hi]` (years; `hi` `None` for no upper limit), at the rank `u` in `[0, 1]`: the age is the
/// born distribution restricted to `[lifetime + lo, lifetime + hi]`, by its closed-form CDF and
/// quantile (Design note 17), less the lifetime.
#[must_use]
pub fn time_since_death(
    ages: &AgeDistribution,
    lifetime: Years,
    lo: Years,
    hi: Option<Years>,
    u: f64,
) -> Years {
    let a = Years::new((lifetime.value() + lo.value()).max(0.0));
    let b = hi.map_or(ages.max(), |h| Years::new(lifetime.value() + h.value()));
    let age = age_between(ages, a, b, u);
    Years::new((age.value() - lifetime.value()).max(lo.value()))
}

/// The age on `[a, b]` of the distribution `ages` at the rank `u` in `[0, 1]`: `quantile(cdf(a) +
/// u (cdf(b) − cdf(a)))`, the distribution restricted to the interval (plan 08, Consumes).
pub(crate) fn age_between(ages: &AgeDistribution, a: Years, b: Years, u: f64) -> Years {
    let (lo, hi) = (ages.cdf(a), ages.cdf(b));
    ages.quantile(lo + u * (hi - lo))
}

/// Per mass node of bands D and E, the least and greatest [`lifetime`] over Z from 0.0001 to 0.03,
/// widened by a tenth (plan 08, Design note 17): whether a layer-D star of a given age can be
/// alive is decided from it, and `lifetime` called only for an age inside it.
///
/// The extremes are taken over [`BRACKET_Z_SAMPLES`] log-spaced metallicities at the median
/// star's draws; a mass between nodes takes the heavier node's least and the lighter node's
/// greatest, since lifetime falls with mass, and the tenth's widening covers the draws and the
/// metallicities between samples (P08.T9.d's test holds it over 10⁵ random stars). Masses above
/// [`MAX_INITIAL_MASS`] take the tracks' heaviest, as `StarModel` does.
#[derive(Debug, Clone, PartialEq)]
pub struct LifetimeBracket {
    masses: [[f64; MARK_MASS_NODES]; 2],
    /// `[band][node]`: (least, greatest), years.
    bounds: [[(f64, f64); MARK_MASS_NODES]; 2],
}

impl Default for LifetimeBracket {
    fn default() -> Self {
        Self::new()
    }
}

impl LifetimeBracket {
    /// The bracket, from 2 × 33 × 13 lifetimes.
    #[must_use]
    pub fn new() -> Self {
        let masses = [band_masses(MassBand::D), band_masses(MassBand::E)];
        let (z_lo, z_hi) = BRACKET_Z_RANGE;
        let step = math::ln(z_hi / z_lo) / BRACKET_Z_SPAN;
        let mut compositions = Vec::with_capacity(BRACKET_Z_SAMPLES);
        let mut k = 0.0;
        for _ in 0..BRACKET_Z_SAMPLES {
            let z = z_lo * math::exp(k * step);
            compositions.push(Composition::from_fe_h(
                Dex::new(math::log10(z / Z_SOLAR.value())),
                HeliumExcess::ZERO,
            ));
            k += 1.0;
        }
        let draws = StarDraws::median();
        let bounds = masses.map(|band| {
            band.map(|m| {
                let m = SolarMasses::new(m.min(MAX_INITIAL_MASS.value()));
                compositions
                    .iter()
                    .map(|comp| lifetime(m, comp, &draws).value())
                    .fold((f64::INFINITY, 0.0_f64), |(lo, hi), t| {
                        (lo.min(t), hi.max(t))
                    })
            })
        });
        Self { masses, bounds }
    }

    /// The bracket on the lifetime of a star of initial mass `m`, or `None` outside bands D and E
    /// (2.5–150 M☉).
    #[must_use]
    pub fn bracket(&self, m: SolarMasses) -> Option<(Years, Years)> {
        let m = m.value();
        let band = if (MASS_BAND_EDGES[3]..MASS_BAND_EDGES[4]).contains(&m) {
            0
        } else if (MASS_BAND_EDGES[4]..=MASS_BAND_EDGES[5]).contains(&m) {
            1
        } else {
            return None;
        };
        let (i, _) = locate(&self.masses[band], m);
        let (lo, hi) = (self.bounds[band][i + 1].0, self.bounds[band][i].1);
        let widen = 1.0 + BRACKET_WIDENING;
        Some((Years::new(lo / widen), Years::new(hi * widen)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;

    /// P08.T1: for every component of the Milky Way fixture, 1,000 ranks on intervals inside and
    /// across its pieces land inside the interval, at the CDF the rank asks for, to 10⁻⁹.
    #[test]
    fn age_cdf_inverts_on_an_interval() {
        let galaxy = Galaxy::from_params(Seed::new(8), GalaxyParams::milky_way_like())
            .expect("the fixture's gas is mostly neutral");
        for component in galaxy.fields().components() {
            let ages = component.ages();
            let (min, max) = (ages.min().value(), ages.max().value());
            for (fa, fb) in [(0.0, 1.0), (0.1, 0.4), (0.35, 0.95), (0.02, 0.03)] {
                let a = Years::new(min + fa * (max - min));
                let b = Years::new(min + fb * (max - min));
                let (lo, hi) = (ages.cdf(a), ages.cdf(b));
                if hi - lo < 1e-9 {
                    continue;
                }
                for k in 0..1_000 {
                    let u = (f64::from(k) + 0.5) / 1_000.0;
                    let age = age_between(ages, a, b, u);
                    let span = (b.value() - a.value()).abs();
                    assert!(
                        age.value() >= a.value() - 1e-9 * span
                            && age.value() <= b.value() + 1e-9 * span,
                        "{:?}: {age:?} outside [{a:?}, {b:?}]",
                        component.population()
                    );
                    let target = lo + u * (hi - lo);
                    let got = ages.cdf(age);
                    assert!(
                        (got - target).abs() <= 1e-9 * target.max(1e-3),
                        "{:?}: cdf {got} against {target}",
                        component.population()
                    );
                }
            }
        }
    }

    #[test]
    fn mass_nodes_span_the_band_and_their_weights_sum_to_one() {
        let mf = MassFunctionKind::default().to_mass_function();
        for band in [MassBand::D, MassBand::E] {
            let nodes = MassNodes::of(band, mf.as_ref());
            let m = nodes.masses();
            assert!((m[0] - MASS_BAND_EDGES[band.index()]).abs() < 1e-12);
            assert!((m[MARK_MASS_NODES - 1] - MASS_BAND_EDGES[band.index() + 1]).abs() < 1e-12);
            assert!(m.windows(2).all(|w| w[0] < w[1]));
            assert!((nodes.weights().iter().sum::<f64>() - 1.0).abs() < 1e-14);
            assert!(nodes.weights().iter().all(|&w| w > 0.0));
        }
    }

    #[test]
    fn a_time_since_death_lies_in_its_bin() {
        let ages = AgeDistribution::uniform(Years::new(0.0), Years::new(1e10)).unwrap();
        let life = Years::new(2e7);
        for k in 0..100 {
            let u = (f64::from(k) + 0.5) / 100.0;
            let s = time_since_death(&ages, life, Years::new(1e6), Some(Years::new(3e6)), u);
            assert!((1e6..=3e6).contains(&s.value()), "{s:?}");
            let s = time_since_death(&ages, life, Years::new(8e7), None, u);
            assert!(s.value() >= 8e7 && s.value() <= 1e10, "{s:?}");
        }
    }
}
