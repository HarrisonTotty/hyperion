//! Plan 11's binary tables (P11.T6–T8): their shapes are plan 11's, their contents plan 15's
//! (P15.T9.b, P15.T10.b), which P11.T15 swaps in with a version bump.
//!
//! [`IA_YIELD`] (P11.T6) is the Type Ia explosion mark's yield η: the share of the binary engine's
//! detonatable pooled Type Ia candidates (ruling 129.1's rule) that explode, by delay bin and
//! channel. **Scratch:** 1 everywhere. As built (`α_CE` λ = 0.5), the detonatable pool is about a
//! third of the delay-time distribution's yield to date (0.354: mergers 0.062, accretions 0.292,
//! galaxy-wide on a drawn Kroupa galaxy, printed by the slow test of `stellar/rules/ia_hold.rs` on
//! 2026-11-01; research `research-p11-t6-ia-rate` measured 0.33 and no bin over its target), so
//! P15.T9.b's η = min(1, target ÷ pool) is 1 in every bin; it fits it against the engine, in this
//! shape.
//!
//! @provisional: hand-made scratch values (plan 11, Design note 13), not written by
//! `hyperion-fit`, so not in [`MANIFEST`](super::MANIFEST) until plan 15's task for them exists.
//! since-generator-version: 26 (the formed mass restated at 26; 16, the batch that lands P11.T7)
//! source: the brainstorm's counts at Milky Way parameters (plan 11, P11.T7): 9 × 10⁶ accreting
//!   white dwarfs (Pala et al. 2020, MNRAS 494, 3799; the brainstorm's 6–12 × 10⁶, split in
//!   halves by design note 12), 10⁴ X-ray binaries (Corral-Santana et al. 2016, A&A 587, A61),
//!   9 × 10⁴ stellar mergers in the source horizon (0.34 a year, Kochanek et al. 2014, MNRAS 443,
//!   1319, × 264,144 years) and 10 neutron-star mergers
//!
//! [`CLASS_SHARES`] (P11.T7) is the share matrix's row for each carved class: how many hosts a
//! solar mass formed makes, by population, and which layer gives each host up. Plan 09's
//! [`FeatureShares::set_class_shares`](crate::galaxy::features::shares::FeatureShares::set_class_shares)
//! folds it into the field factor, so that the grid, which redraws every binary that falls into a
//! class ([`SystemStars::generate`](crate::stellar::system::SystemStars::generate)), gives up
//! exactly what P11.T8's catalogue classes hold.
//!
//! **Fitted** for the accreting white dwarfs and the X-ray binaries, by plan 15's P15.T10.b against
//! the engine at generator version 27, and **scratch** for the two merger classes: every
//! population makes the same hosts per solar mass formed, scaled so that the Milky Way fixture's
//! formed mass ([`MILKY_WAY_FORMED_MASS`]) gives the counts above, and each class's layers are a
//! guess from the stars that make it: a neutron-star merger's primary is the first compact star's,
//! 8 M☉ and up; a stellar merger's is spread over the layers with most of it in C, where the
//! contact binaries of W Ursae Majoris type lie.
//!
//! [`ClassSamplerTable`] (P11.T8) is a catalogue class's conditional laws of the marks that define
//! it now and of its history, which plan 11's `stellar::binary::sampler::ClassSampler` draws by
//! inverse transform. Each mark is a residual given by its quantiles at the 17 ranks k ÷ 16, as
//! plan 15's `PRIMARY_MASS_CDF` gives its laws, added to a median that follows a condition on a
//! 17-point grid. [`AWD_FAST_SAMPLER`] and [`AWD_SLOW_SAMPLER`] are the accreting white dwarfs'
//! and [`XRB_SAMPLER`] the X-ray binaries': P15.T10.b's fits against the engine's own members, a
//! mark too few members define kept from plan 11's hand-made scratch (Design note 13);
//! [`MERGER_SAMPLER`] is the stellar mergers' (P11.T8.c) and [`NSM_SAMPLER`] the neutron-star
//! mergers' (P11.T8.d).
//!
//! The fitted tables above are 27's, frozen in the `refit_27` module, since their census drew
//! every record's first attempt under `Free` multiplicity on its primary's own draws, which the
//! generator does not publish (train 27's finding). Tasks `binary_awd` and `binary_xrb` refit them
//! on the generator's own pairs into their blocks below, [`AWD_FAST_SAMPLER_29`],
//! [`AWD_SLOW_SAMPLER_29`], [`XRB_SAMPLER_29`] and [`CLASS_SHARES_29`], which a galaxy reads from
//! generator version 29 (`galaxy::catalogue_classes::binary`'s `tables_hold`).

use std::marker::PhantomData;

use crate::galaxy::POPULATIONS;
use crate::galaxy::Population;
use crate::galaxy::imf::MassBand;
pub use crate::stellar::binary::IaPoolChannel;
use crate::stellar::binary::light_curves::XrayAccretor;
use crate::stellar::binary::{BinaryClass, CarvedClass, HmxbKind, XrbKind};
use crate::tables::type_ia_delay::DELAY_EDGES;
use crate::units::Years;

mod refit_27;

pub use refit_27::{
    AWD_FAST_DONOR_KINDS_27, AWD_FAST_HOSTS_PER_FORMED_MASS_27, AWD_FAST_LAYER_SHARES_27,
    AWD_FAST_LIMITS_27, AWD_FAST_MEDIAN_SPANS_27, AWD_FAST_MEDIANS_27, AWD_FAST_NODE_RESIDUALS_27,
    AWD_FAST_RESIDUALS_27, AWD_SLOW_DONOR_KINDS_27, AWD_SLOW_HOSTS_PER_FORMED_MASS_27,
    AWD_SLOW_LAYER_SHARES_27, AWD_SLOW_LIMITS_27, AWD_SLOW_MEDIAN_SPANS_27, AWD_SLOW_MEDIANS_27,
    AWD_SLOW_NODE_RESIDUALS_27, AWD_SLOW_RESIDUALS_27, XRB_DONOR_MASS_NODE_RESIDUALS_27,
    XRB_HIGH_MASS_KINDS_27, XRB_HOSTS_PER_FORMED_MASS_27, XRB_LAYER_SHARES_27, XRB_LIMITS_27,
    XRB_LOW_MASS_KINDS_27, XRB_MEDIAN_SPANS_27, XRB_MEDIANS_27, XRB_RESIDUALS_27,
};

/// The Milky Way fixture's initial mass formed, M☉, that the scratch rates are scaled to: its
/// components' systems (with those not yet born) times the mass formed per system.
pub const MILKY_WAY_FORMED_MASS: f64 = 9.430_766_802_241_24e10;

/// One carved class's row of [`ClassShareTable`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassShareRow {
    class: CarvedClass,
    /// Hosts per solar mass formed, by population in [`POPULATIONS`] order: for the two merger
    /// classes, mergers inside the source horizon.
    hosts_per_formed_mass: [f64; 7],
    /// The share of the hosts whose primary lies in each stellar layer, A–E; they sum to 1.
    layer_shares: [f64; 5],
}

impl ClassShareRow {
    /// The class.
    #[must_use]
    pub const fn class(&self) -> CarvedClass {
        self.class
    }

    /// Hosts of the class per solar mass formed in `population`.
    #[must_use]
    pub fn hosts_per_formed_mass(&self, population: Population) -> f64 {
        self.hosts_per_formed_mass[population_index(population)]
    }

    /// The share of the class's hosts whose primary lies in `band`: 0 for a substellar band.
    #[must_use]
    pub const fn layer_share(&self, band: MassBand) -> f64 {
        match band {
            MassBand::A | MassBand::B | MassBand::C | MassBand::D | MassBand::E => {
                self.layer_shares[band.index()]
            }
            MassBand::BrownDwarf | MassBand::RoguePlanet => 0.0,
        }
    }
}

/// The share matrix's rows for the carved binary classes (plan 11's Provides): per class and
/// population, hosts per solar mass formed, and the share of them that each of layers A–E gives up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassShareTable {
    rows: [ClassShareRow; 5],
}

impl ClassShareTable {
    /// The rows, in [`CarvedClass`] order.
    #[must_use]
    pub const fn rows(&self) -> &[ClassShareRow; 5] {
        &self.rows
    }

    /// The row of `class`.
    ///
    /// # Panics
    ///
    /// Never: the table holds a row for every carved class.
    #[must_use]
    pub fn row(&self, class: CarvedClass) -> &ClassShareRow {
        self.rows
            .iter()
            .find(|row| row.class == class)
            .expect("the table holds a row for every carved class")
    }
}

/// A population's place in [`POPULATIONS`].
fn population_index(population: Population) -> usize {
    POPULATIONS
        .iter()
        .position(|&p| p == population)
        .expect("every population is in POPULATIONS")
}

/// A row whose hosts, `count` at Milky Way parameters, every population makes alike per solar
/// mass formed.
const fn uniform(class: CarvedClass, count: f64, layer_shares: [f64; 5]) -> ClassShareRow {
    let rate = count / MILKY_WAY_FORMED_MASS;
    ClassShareRow {
        class,
        hosts_per_formed_mass: [rate; 7],
        layer_shares,
    }
}

/// The scratch share rows (P11.T7): 4.5 × 10⁶ fast and 4.5 × 10⁶ slow accreting white dwarfs and
/// 10⁴ X-ray binaries at Milky Way parameters, every population alike, an accreting white dwarf's
/// primary its white dwarf's progenitor, 0.75–8 M☉, so in layers C and D, an X-ray binary's the
/// compact star's, 8 M☉ and up; the merger classes' rows are [`CLASS_SHARES`]' own. Only the
/// census galaxy of `hyperion-fit`'s binary tasks reads them
/// (`Galaxy::from_params_for_binary_refit`), so that the field its records come from does not
/// read the rows the tasks fit.
#[cfg(any(test, feature = "testing"))]
pub(crate) const SCRATCH_CLASS_SHARES: ClassShareTable = ClassShareTable {
    rows: [
        uniform(
            CarvedClass::AccretingWdFast,
            4.5e6,
            [0.0, 0.0, 0.75, 0.25, 0.0],
        ),
        uniform(
            CarvedClass::AccretingWdSlow,
            4.5e6,
            [0.0, 0.0, 0.75, 0.25, 0.0],
        ),
        uniform(CarvedClass::XrayBinary, 1.0e4, [0.0, 0.0, 0.0, 0.0, 1.0]),
        CLASS_SHARES.rows[3],
        CLASS_SHARES.rows[4],
    ],
};

/// The number of delay bins of [`IaYieldTable`]: those of [`DELAY_EDGES`], plan 15's.
pub const IA_DELAY_BINS: usize = DELAY_EDGES.len() - 1;

/// The Type Ia explosion mark's yield (plan 11's Provides; P11.T6): η, the share of the binary
/// engine's pooled Type Ia candidates that explode, by delay bin of [`DELAY_EDGES`] and by
/// [`IaPoolChannel`], each in [0, 1]. Plan 11's
/// [`IaExplosionMark`](crate::stellar::binary::ia::IaExplosionMark) reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IaYieldTable {
    /// η by delay bin, then by channel: [`IaPoolChannel::Merger`], [`IaPoolChannel::Accretion`].
    eta: [[f64; 2]; IA_DELAY_BINS],
}

impl IaYieldTable {
    /// The table of `eta`, by delay bin of [`DELAY_EDGES`] and then by channel, the merger first.
    ///
    /// # Panics
    ///
    /// If a share is not in [0, 1]: at compile time for a constant.
    #[must_use]
    pub const fn new(eta: [[f64; 2]; IA_DELAY_BINS]) -> Self {
        let mut bin = 0;
        while bin < IA_DELAY_BINS {
            let [merger, accretion] = eta[bin];
            assert!(
                merger >= 0.0 && merger <= 1.0 && accretion >= 0.0 && accretion <= 1.0,
                "a yield is a share, in [0, 1]"
            );
            bin += 1;
        }
        Self { eta }
    }

    /// η of delay bin `bin` and `channel`.
    ///
    /// # Panics
    ///
    /// If `bin` is not under [`IA_DELAY_BINS`].
    #[must_use]
    pub const fn eta_in(&self, bin: usize, channel: IaPoolChannel) -> f64 {
        self.eta[bin][match channel {
            IaPoolChannel::Merger => 0,
            IaPoolChannel::Accretion => 1,
        }]
    }

    /// η of a candidate pooled `delay` after its stars formed, by `channel`: its bin of
    /// [`DELAY_EDGES`] ([`ia_delay_bin`]), a delay outside the edges in the end bin on its side.
    ///
    /// # Panics
    ///
    /// In debug builds, if `delay` is negative or NaN, which no pooled candidate's is.
    #[must_use]
    pub fn eta(&self, delay: Years, channel: IaPoolChannel) -> f64 {
        debug_assert!(
            delay.value() >= 0.0,
            "a pooled candidate's delay, {delay:?}"
        );
        self.eta_in(ia_delay_bin(delay), channel)
    }
}

/// The bin of [`DELAY_EDGES`] holding `delay`, a lower edge in its own bin and the ends held: the
/// first bin for a delay below the first edge, 42.55 Myr, which no white dwarf reaches, and for a
/// NaN delay; the last at and beyond the last edge, the Hubble time.
#[must_use]
pub fn ia_delay_bin(delay: Years) -> usize {
    DELAY_EDGES
        .partition_point(|&edge| edge <= delay.value())
        .saturating_sub(1)
        .min(IA_DELAY_BINS - 1)
}

/// The scratch yield (module documentation; plan 11, Design note 13): 1 in every delay bin and
/// for both channels, every detonatable candidate exploding, since the detonatable pool falls
/// short of the yield in every bin.
pub const IA_YIELD: IaYieldTable = IaYieldTable::new([[1.0; 2]; IA_DELAY_BINS]);

/// A mark's median as a function of its condition (P11.T8): its value at 17 evenly spaced nodes
/// from the first to the last value of the condition, linear between them and held beyond.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MedianCurve {
    first: f64,
    last: f64,
    nodes: [f64; 17],
}

impl MedianCurve {
    /// The median of `nodes` at 17 evenly spaced values of the condition from `first` to `last`.
    #[must_use]
    pub const fn new(first: f64, last: f64, nodes: [f64; 17]) -> Self {
        Self { first, last, nodes }
    }

    /// The condition's values at the first and the last node.
    #[must_use]
    pub const fn span(&self) -> (f64, f64) {
        (self.first, self.last)
    }

    /// The median at the 17 nodes.
    #[must_use]
    pub const fn nodes(&self) -> &[f64; 17] {
        &self.nodes
    }
}

/// One mark of a [`ClassSamplerTable`] (P11.T8): a residual, by its quantiles at the 17 ranks
/// k ÷ 16 and linear between them, about a median that is zero or follows a condition, the value
/// held to its limits.
///
/// A mark of one law has the same spread at every condition, which the median shifts (plan 11's
/// Risks, "P11.T8.a as built"). A conditional mark may instead hold a law at each of its median's
/// 17 nodes ([`with_node_residuals`](Self::with_node_residuals), P15.T10.b's refit at 27), mixed
/// between the two nearest nodes as the median is, so that its spread follows the condition too.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplerMark {
    name: &'static str,
    residual: [f64; 17],
    node_residuals: Option<&'static [[f64; 17]; 17]>,
    median: Option<MedianCurve>,
    limits: (f64, f64),
}

impl SamplerMark {
    /// The mark `name` of residual quantiles `residual` (non-decreasing, at the ranks k ÷ 16)
    /// about `median`, held to `limits`.
    #[must_use]
    pub const fn new(
        name: &'static str,
        residual: [f64; 17],
        median: Option<MedianCurve>,
        limits: (f64, f64),
    ) -> Self {
        Self {
            name,
            residual,
            node_residuals: None,
            median,
            limits,
        }
    }

    /// The mark with a residual law at each node of its median: `rows[k]`, the law's quantiles at
    /// the ranks k ÷ 16 (each row non-decreasing), at node k. Its own residual stays the law of
    /// all its members, which a draw of a mark with node laws does not read.
    ///
    /// # Panics
    ///
    /// If the mark has no median, whose nodes the laws are at: in a `const` table, a compile
    /// error.
    #[must_use]
    pub const fn with_node_residuals(self, rows: &'static [[f64; 17]; 17]) -> Self {
        assert!(
            self.median.is_some(),
            "a mark with node laws follows a median"
        );
        Self {
            node_residuals: Some(rows),
            ..self
        }
    }

    /// The residual laws at the median's 17 nodes, or `None` for a mark of one law.
    #[must_use]
    pub const fn node_residuals(&self) -> Option<&'static [[f64; 17]; 17]> {
        self.node_residuals
    }

    /// The mark's name, in `snake_case`, with its unit.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The residual's quantiles at the ranks k ÷ 16, non-decreasing.
    #[must_use]
    pub const fn residual(&self) -> &[f64; 17] {
        &self.residual
    }

    /// The median the residual is added to: `None` for an unconditional mark, whose residual is
    /// its value.
    #[must_use]
    pub const fn median(&self) -> Option<&MedianCurve> {
        self.median.as_ref()
    }

    /// The least and the greatest value a draw takes.
    #[must_use]
    pub const fn limits(&self) -> (f64, f64) {
        self.limits
    }
}

/// One choice among a class's kinds (P11.T8): its weights, which sum to 1, in the kinds' order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplerPick {
    name: &'static str,
    weights: &'static [f64],
}

impl SamplerPick {
    /// The choice `name` among kinds of `weights`, in the kinds' order.
    #[must_use]
    pub const fn new(name: &'static str, weights: &'static [f64]) -> Self {
        Self { name, weights }
    }

    /// The choice's name, in `snake_case`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The kinds' weights, in order.
    #[must_use]
    pub const fn weights(&self) -> &'static [f64] {
        self.weights
    }
}

/// The key of a class's mark or choice in its [`ClassSamplerTable`]: a class's own enum, so that a
/// sampler is only ever asked for its own table's marks (P11.T8).
pub trait SamplerKey: Copy {
    /// The key's place in its table's marks or choices.
    #[must_use]
    fn index(self) -> usize;
}

/// A catalogue class's sampler table (plan 11's Provides; P11.T8): its marks and its choices, in
/// the order the class draws them, keyed by the class's mark enum `M` and choice enum `P`. The
/// mark list is fixed per class: [`AwdMark`] and [`AwdPick`] for [`AWD_FAST_SAMPLER`] and
/// [`AWD_SLOW_SAMPLER`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassSamplerTable<M, P> {
    marks: &'static [SamplerMark],
    picks: &'static [SamplerPick],
    keys: PhantomData<fn() -> (M, P)>,
}

impl<M: SamplerKey, P: SamplerKey> ClassSamplerTable<M, P> {
    /// The table of `marks` and `picks`, in the orders `M` and `P` index them.
    #[must_use]
    pub const fn new(marks: &'static [SamplerMark], picks: &'static [SamplerPick]) -> Self {
        Self {
            marks,
            picks,
            keys: PhantomData,
        }
    }

    /// The marks, in the class's order.
    #[must_use]
    pub const fn marks(&self) -> &'static [SamplerMark] {
        self.marks
    }

    /// The choices, in the class's order.
    #[must_use]
    pub const fn picks(&self) -> &'static [SamplerPick] {
        self.picks
    }

    /// Mark `mark`.
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names: a table built against another list.
    #[must_use]
    pub fn mark(&self, mark: M) -> &'static SamplerMark {
        &self.marks[mark.index()]
    }

    /// Choice `pick`.
    ///
    /// # Panics
    ///
    /// If the table holds fewer choices than `P` names.
    #[must_use]
    pub fn pick(&self, pick: P) -> &'static SamplerPick {
        &self.picks[pick.index()]
    }
}

/// The marks of the accreting white dwarfs' samplers ([`AWD_FAST_SAMPLER`] and
/// [`AWD_SLOW_SAMPLER`]), in their order (P11.T8.a): what an accreting white dwarf is now, then
/// its history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AwdMark {
    /// log₁₀ of the nova recurrence period, years, unconditional: the mark the two classes split
    /// on at L (Design note 12).
    Recurrence,
    /// The white dwarf's mass, M☉, given the recurrence's log₁₀.
    WhiteDwarfMass,
    /// log₁₀ of a main-sequence donor's orbital period, hours, given log₁₀ of the transfer rate,
    /// M☉ yr⁻¹.
    MainSequencePeriod,
    /// log₁₀ of a helium donor's orbital period, hours, given log₁₀ of the transfer rate.
    HeliumPeriod,
    /// log₁₀ of a giant donor's orbital period, hours, given log₁₀ of the transfer rate.
    GiantPeriod,
    /// log₁₀ of a main-sequence donor's mass, M☉, given log₁₀ of the period, hours.
    MainSequenceDonorMass,
    /// log₁₀ of a helium donor's mass, M☉, given log₁₀ of the period, hours.
    HeliumDonorMass,
    /// log₁₀ of a giant donor's mass, M☉, given log₁₀ of the period, hours.
    GiantDonorMass,
    /// log₁₀ of the primary's initial mass, M☉, given the white dwarf's mass.
    PrimaryInitialMass,
    /// log₁₀ of the donor's initial mass, M☉, given log₁₀ of its mass now.
    DonorInitialMass,
    /// log₁₀ of the system's age at the epoch, years, given log₁₀ of the primary's initial mass.
    Age,
}

impl AwdMark {
    /// Every mark, in the samplers' order.
    pub const ALL: [Self; 11] = [
        Self::Recurrence,
        Self::WhiteDwarfMass,
        Self::MainSequencePeriod,
        Self::HeliumPeriod,
        Self::GiantPeriod,
        Self::MainSequenceDonorMass,
        Self::HeliumDonorMass,
        Self::GiantDonorMass,
        Self::PrimaryInitialMass,
        Self::DonorInitialMass,
        Self::Age,
    ];

    /// The mark's place in the samplers' marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for AwdMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of the accreting white dwarfs' samplers, in their order (P11.T8.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AwdPick {
    /// The donor's kind ([`AwdDonorKind`]).
    DonorKind,
}

impl SamplerKey for AwdPick {
    fn index(self) -> usize {
        match self {
            Self::DonorKind => 0,
        }
    }
}

/// The donor kinds of the accreting white dwarfs' samplers' one choice, in its order (P11.T8.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AwdDonorKind {
    /// A hydrogen-rich main-sequence star: a cataclysmic variable.
    MainSequence,
    /// A helium white dwarf or a helium star: an AM Canum Venaticorum star, or its progenitor's
    /// helium-star donor.
    Helium,
    /// A red giant filling its Roche lobe: a symbiotic star.
    Giant,
}

impl AwdDonorKind {
    /// Every kind, in the choice's order.
    pub const ALL: [Self; 3] = [Self::MainSequence, Self::Helium, Self::Giant];
}

/// The residual of a normal of standard deviation 1, truncated at ±2.5, at the ranks k ÷ 16.
const NORMAL_RANKS: [f64; 17] = [
    -2.5, -1.491_36, -1.128_013, -0.872_818, -0.664_751, -0.482_209, -0.314_548, -0.155_341, 0.0,
    0.155_341, 0.314_548, 0.482_209, 0.664_751, 0.872_818, 1.128_013, 1.491_36, 2.5,
];

/// [`NORMAL_RANKS`] scaled to a standard deviation of `sigma`.
const fn normal(sigma: f64) -> [f64; 17] {
    let mut out = [0.0; 17];
    let mut k = 0;
    while k < 17 {
        out[k] = NORMAL_RANKS[k] * sigma;
        k += 1;
    }
    out
}

/// The marks of [`MERGER_SAMPLER`], in its order (P11.T8.c): a stellar merger's age, then its
/// pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MergerMark {
    /// The rank, in [0, 1], of the system's age at the epoch in its density component's law
    /// restricted to the born systems, unconditional.
    AgeRank,
    /// The rank, in [0, 1], of the primary's initial mass in the galaxy's mass function over its
    /// layer's band, cut at the turn-off at the merger, unconditional.
    PrimaryRank,
    /// The mass ratio of a contact pair before the merger, at the tidal limit, unconditional:
    /// version 26's contact channel.
    ContactMassRatio,
    /// The rank, in [0, 1], of an evolved donor's initial mass in the galaxy's mass function over
    /// the masses living past their main sequence at the merger, unconditional.
    EvolvedPrimaryRank,
    /// The mass ratio of an evolved donor's companion over the donor, initial masses,
    /// unconditional: version 26's for every evolved donor, from 27 a giant-branch donor's.
    EvolvedMassRatio,
    /// The initial mass, M☉, of a dynamical main-sequence merger's donor, the primary, a low-mass
    /// (BSE's k = 0) main-sequence star, unconditional: version 27's dynamical channel.
    DynamicalPrimaryMass,
    /// The mass ratio of a dynamical main-sequence merger's companion over its donor, initial
    /// masses, unconditional: version 27's.
    DynamicalMassRatio,
    /// The mass ratio of a Hertzsprung-gap donor's companion over the donor, initial masses,
    /// unconditional: version 27's.
    GapMassRatio,
    /// The mass ratio of a core-helium-burning donor's companion over the donor, initial masses,
    /// unconditional: version 27's.
    HeliumBurningMassRatio,
    /// The mass ratio of an asymptotic-giant-branch donor's companion over the donor, initial
    /// masses, unconditional: version 27's.
    AgbMassRatio,
}

impl MergerMark {
    /// Every mark, in [`MERGER_SAMPLER`]'s order.
    pub const ALL: [Self; 10] = [
        Self::AgeRank,
        Self::PrimaryRank,
        Self::ContactMassRatio,
        Self::EvolvedPrimaryRank,
        Self::EvolvedMassRatio,
        Self::DynamicalPrimaryMass,
        Self::DynamicalMassRatio,
        Self::GapMassRatio,
        Self::HeliumBurningMassRatio,
        Self::AgbMassRatio,
    ];

    /// The mark's place in [`MERGER_SAMPLER`]'s marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for MergerMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of [`MERGER_SAMPLER`], in its order (P11.T8.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MergerPick {
    /// The layer of the primary, A–E, in [`MassBand::ALL`]'s order.
    Layer,
    /// The channel among [`MergerChannel::ALL`]: version 27's, with the dynamical main sequence.
    EngineChannel,
}

impl SamplerKey for MergerPick {
    fn index(self) -> usize {
        match self {
            Self::Layer => 0,
            Self::EngineChannel => 1,
        }
    }
}

/// The channels of [`MERGER_SAMPLER`]'s channel choice (P11.T8.c's fix and its missing channel):
/// what a stellar merger is before its merger time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MergerChannel {
    /// A contact pair of two main-sequence stars, at the tidal limit (Rasio 1995).
    Contact,
    /// An evolved donor, from the Hertzsprung gap to the asymptotic giant branch, whose common
    /// envelope merges it with its main-sequence companion.
    EvolvedDonor,
    /// A low-mass (BSE's k = 0) main-sequence donor at its Roche lobe past its critical mass ratio,
    /// which the engine merges dynamically with its main-sequence companion (version 27's).
    DynamicalMainSequence,
}

impl MergerChannel {
    /// Every channel, in [`MergerPick::EngineChannel`]'s order.
    pub const ALL: [Self; 3] = [
        Self::Contact,
        Self::EvolvedDonor,
        Self::DynamicalMainSequence,
    ];
}

/// The ranks k ÷ 16 themselves: a residual that leaves a rank as it is drawn.
const IDENTITY_RANKS: [f64; 17] = [
    0.0, 0.0625, 0.125, 0.1875, 0.25, 0.3125, 0.375, 0.4375, 0.5, 0.5625, 0.625, 0.6875, 0.75,
    0.8125, 0.875, 0.9375, 1.0,
];

/// The scratch sampler of the stellar mergers (P11.T8.c; Design notes 10 and 13): [`MergerMark`]'s
/// marks and [`MergerPick`]'s choices.
///
/// Hand-made, the figures recalled and not read at source in this build (the literature could not
/// be reached), and the same in every population; P15.T10.b refits it against the engine's own
/// mergers:
///
/// - **The age** and **the primary's mass** are ranks that leave the density component's own age
///   law and the galaxy's own mass function as they are: the scratch merger rate per system is the
///   same at every age and mass within a layer, so a merger's history is a grid system's. The
///   primary is on its main sequence at the merger, so its mass is cut at the turn-off then.
/// - **The layer**: [`CLASS_SHARES`]'s row for the stellar mergers, A 0.1, B 0.2, C 0.5, D 0.15,
///   E 0.05, restricted to the layers with stars on their main sequence at the merger.
///
/// Version 26's channels (P11.T8.c's fix; research-leans-13 §2(c)) add three marks, the contact
/// pair's and the evolved donor's, whose channel version 27's choice draws:
///
/// - **The channel**: at 26 0.15 contact and 0.85 evolved donor, scratch. Luminous red novae whose
///   progenitors are resolved had evolved donors: M31-LRN-2015's a subgiant of 3–5.5 M☉ (MacLeod
///   et al. 2017, ApJ 835, 282) and M101 OT2015-1's a yellow supergiant of about 18 M☉
///   (Blagorodnova et al. 2017, ApJ 834, 107); and the engine's own mergers, its pre-main-sequence
///   ones left out, are mostly evolved stars beside a main-sequence companion and a quarter or so
///   two main-sequence stars, of which few are in contact
///   (`galaxy/catalogue_classes/binary/merger/channels_hold.rs`'s
///   `the_channel_sampler_against_the_engine_under_every_26_hold` prints its split).
/// - **The contact pair's mass ratio**: 0.06–0.15, median 0.09, about Rasio's (1995) tidal limit,
///   below which a contact pair is unstable and coalesces on the main sequence, as the engine's
///   contact rule merges it; V1309 Scorpii's was about 0.1 (Stępień et al. 2011). The limits are
///   scratch, ours. The contact pair's primary is at least 0.1 ÷ 0.15 M☉, so that its secondary is
///   a track's at that ratio.
/// - **The evolved donor's companion**: q = M₂ ÷ M₁ of initial masses, 0.05–1, median 0.6, broad
///   up to the twins, the engine's own evolved donors' (weighted median 0.63), scratch; the scan
///   draws it below the ratio at which the engine finds the donor's transfer unstable.
/// - **The evolved donor's mass** is a rank that leaves the galaxy's mass function over the masses
///   past their main sequence and living at the merger as it is.
///
/// Version 27's (P11.T8.c's missing channel; research-leans-16 §1.1) add a choice and five marks,
/// scratch, shaped on the engine's own mergers under every 26 hold (48,000 members, an effective
/// 1,259 when measured; `merger/engine_channels_hold.rs`'s slow test prints them) and not fitted:
///
/// - **The channel** among three: 0.15 contact, as at 26, the light curves' evidence; 0.50 evolved
///   donor and 0.35 dynamical main sequence, the rest split about as the engine's evolved donors
///   beside a main-sequence star (0.40 of its mergers) and its pairs of main-sequence stars (0.30,
///   95% of them a low-mass donor detached before the merger).
/// - **The dynamical donor's mass**: 0.1–0.7 M☉, median 0.6, mostly above 0.4 (the engine's
///   weighted median 0.61, 5–95% 0.42–0.70): a star a little under BSE's 0.7 M☉ fills its lobe
///   first.
/// - **Its companion's mass ratio**: 0.15–1, median 0.73 (the engine's 0.71, 5–95% 0.49–0.95).
/// - **An evolved donor's companion by the donor's kind**: a giant-branch donor's stays
///   [`MergerMark::EvolvedMassRatio`] (the engine's median 0.59); a Hertzsprung-gap donor's
///   0.05–0.25, median 0.16, inside the ratio 1 ÷ 4 at which its transfer is unstable (the engine's
///   gap donors there lie about 0.11–0.25); a core-helium-burning donor's 0.05–0.33, flat to the
///   ratio 1 ÷ 3, the engine merging almost none; an asymptotic-giant-branch donor's 0.05–1,
///   median 0.49 (the engine's).
pub const MERGER_SAMPLER: ClassSamplerTable<MergerMark, MergerPick> = ClassSamplerTable::new(
    &MERGER_MARKS,
    &[
        SamplerPick::new("layer", &[0.1, 0.2, 0.5, 0.15, 0.05]),
        SamplerPick::new("engine_channel", &[0.15, 0.50, 0.35]),
    ],
);

/// [`MERGER_SAMPLER`]'s marks, in [`MergerMark`]'s order.
#[rustfmt::skip]
const MERGER_MARKS: [SamplerMark; 10] = [
    SamplerMark {
        name: "age_rank",
        residual: IDENTITY_RANKS,
        median: None,
        node_residuals: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "primary_rank",
        residual: IDENTITY_RANKS,
        median: None,
        node_residuals: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "contact_mass_ratio",
        residual: [
            0.060, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.090,
            0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.150,
        ],
        median: None,
        node_residuals: None,
        limits: (0.06, 0.15),
    },
    SamplerMark {
        name: "evolved_primary_rank",
        residual: IDENTITY_RANKS,
        median: None,
        node_residuals: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "evolved_mass_ratio",
        residual: [
            0.05, 0.11, 0.17, 0.25, 0.33, 0.41, 0.49, 0.55, 0.60,
            0.68, 0.76, 0.83, 0.89, 0.93, 0.96, 0.98, 1.0,
        ],
        median: None,
        node_residuals: None,
        limits: (0.05, 1.0),
    },
    SamplerMark {
        name: "dynamical_primary_mass",
        residual: [
            0.10, 0.40, 0.42, 0.44, 0.46, 0.49, 0.52, 0.56, 0.60,
            0.61, 0.62, 0.63, 0.64, 0.66, 0.68, 0.69, 0.70,
        ],
        median: None,
        node_residuals: None,
        limits: (0.1, 0.7),
    },
    SamplerMark {
        name: "dynamical_mass_ratio",
        residual: [
            0.15, 0.45, 0.52, 0.58, 0.63, 0.66, 0.69, 0.71, 0.73,
            0.76, 0.79, 0.82, 0.85, 0.88, 0.91, 0.94, 1.0,
        ],
        median: None,
        node_residuals: None,
        limits: (0.15, 1.0),
    },
    SamplerMark {
        name: "gap_mass_ratio",
        residual: [
            0.05, 0.08, 0.10, 0.11, 0.12, 0.13, 0.14, 0.15, 0.16,
            0.17, 0.18, 0.19, 0.20, 0.21, 0.22, 0.235, 0.25,
        ],
        median: None,
        node_residuals: None,
        limits: (0.05, 0.25),
    },
    SamplerMark {
        name: "helium_burning_mass_ratio",
        residual: [
            0.05, 0.0675, 0.085, 0.1025, 0.12, 0.1375, 0.155, 0.1725, 0.19,
            0.2075, 0.225, 0.2425, 0.26, 0.2775, 0.295, 0.3125, 0.33,
        ],
        median: None,
        node_residuals: None,
        limits: (0.05, 0.33),
    },
    SamplerMark {
        name: "agb_mass_ratio",
        residual: [
            0.05, 0.09, 0.12, 0.18, 0.27, 0.33, 0.39, 0.44, 0.49,
            0.54, 0.59, 0.64, 0.69, 0.75, 0.82, 0.89, 1.0,
        ],
        median: None,
        node_residuals: None,
        limits: (0.05, 1.0),
    },
];

/// The marks of [`NSM_SAMPLER`], in its order (P11.T8.d): a neutron-star merger's pair, then its
/// history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NsmMark {
    /// A neutron star's mass at the merger, M☉, unconditional.
    NeutronStarMass,
    /// log₁₀ of a black hole's mass at the merger, M☉, unconditional.
    LogBlackHoleMass,
    /// The rank, in [0, 1], of a neutron star's progenitor's initial mass in the galaxy's mass
    /// function over [`NSM_NEUTRON_STAR_PROGENITORS`], unconditional.
    NeutronStarProgenitorRank,
    /// The rank, in [0, 1], of a black hole's progenitor's initial mass in the galaxy's mass
    /// function over [`NSM_BLACK_HOLE_PROGENITORS`], unconditional.
    BlackHoleProgenitorRank,
    /// The rank, in [0, 1], of the system's age at the epoch in its density component's law
    /// restricted to the born systems, unconditional.
    AgeRank,
}

impl NsmMark {
    /// Every mark, in [`NSM_SAMPLER`]'s order.
    pub const ALL: [Self; 5] = [
        Self::NeutronStarMass,
        Self::LogBlackHoleMass,
        Self::NeutronStarProgenitorRank,
        Self::BlackHoleProgenitorRank,
        Self::AgeRank,
    ];

    /// The mark's place in [`NSM_SAMPLER`]'s marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for NsmMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of [`NSM_SAMPLER`], in its order (P11.T8.d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NsmPick {
    /// The pair's kind ([`NsmPairKind`]).
    PairKind,
}

impl SamplerKey for NsmPick {
    fn index(self) -> usize {
        match self {
            Self::PairKind => 0,
        }
    }
}

/// The pair kinds of [`NSM_SAMPLER`]'s one choice, in its order (P11.T8.d): the two kinds of
/// compact pair whose merger is a neutron-star merger (P11.T7's `merger_class`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NsmPairKind {
    /// Two neutron stars.
    DoubleNeutronStar,
    /// A black hole and a neutron star; the black hole is the primary.
    NeutronStarBlackHole,
}

impl NsmPairKind {
    /// Every kind, in the choice's order.
    pub const ALL: [Self; 2] = [Self::DoubleNeutronStar, Self::NeutronStarBlackHole];
}

/// The initial masses of a neutron star's progenitor a neutron-star merger draws, M☉: 8–20, ours,
/// provisional; about where single stars leave neutron stars rather than white dwarfs or black
/// holes (Sukhbold et al. 2016, ApJ 821, 38, recalled, not read at source).
pub const NSM_NEUTRON_STAR_PROGENITORS: (f64, f64) = (8.0, 20.0);

/// The initial masses of a black hole's progenitor a neutron-star merger draws, M☉: 25–60, ours,
/// provisional, as [`NSM_NEUTRON_STAR_PROGENITORS`].
pub const NSM_BLACK_HOLE_PROGENITORS: (f64, f64) = (25.0, 60.0);

/// A normal of mean `mean` and standard deviation `sigma`, truncated at ±2.5 σ, at the ranks
/// k ÷ 16.
const fn normal_about(mean: f64, sigma: f64) -> [f64; 17] {
    let mut out = normal(sigma);
    let mut k = 0;
    while k < 17 {
        out[k] += mean;
        k += 1;
    }
    out
}

/// The scratch sampler of the neutron-star mergers (P11.T8.d; Design notes 10 and 13):
/// [`NsmMark`]'s five marks and [`NsmPick`]'s one choice, the pair's kind ([`NsmPairKind`]).
///
/// Hand-made, the figures recalled and not read at source in this build (the literature could not
/// be reached), and the same in every population; P15.T10.b refits it against the engine's own
/// mergers:
///
/// - **A neutron star's mass**: 1.33 ± 0.09 M☉, the Galactic double neutron stars' (Özel and
///   Freire 2016, ARA&A 54, 401), held to plan 06's 1.13–2.0 M☉ (Mandel and Müller 2020).
/// - **A black hole's mass**: log-normal about 7 M☉, ±0.15 dex, on 3–20 M☉: the black holes of
///   the first neutron star–black hole mergers, GW200105's 8.9 M☉ and GW200115's 5.7 M☉ (Abbott
///   et al. 2021, ApJL 915, L5).
/// - **The progenitors' masses** and **the age** are ranks that leave the galaxy's mass function
///   over [`NSM_NEUTRON_STAR_PROGENITORS`] and [`NSM_BLACK_HOLE_PROGENITORS`] and the component's
///   age law as they are: the scratch merger rate is the same at every age, as [`CLASS_SHARES`]'s
///   row makes it.
/// - **The pair's kind**: 0.8 two neutron stars, 0.2 a neutron star and a black hole, after the
///   ratio of the two rates GWTC-3 gives, about 10–1,700 and 8–140 Gpc⁻³ yr⁻¹ (Abbott et al. 2023,
///   PRX 13, 011048).
pub const NSM_SAMPLER: ClassSamplerTable<NsmMark, NsmPick> =
    ClassSamplerTable::new(&NSM_MARKS, &[SamplerPick::new("pair_kind", &[0.8, 0.2])]);

/// [`NSM_SAMPLER`]'s marks, in [`NsmMark`]'s order.
#[rustfmt::skip]
const NSM_MARKS: [SamplerMark; 5] = [
    SamplerMark {
        name: "neutron_star_mass",
        residual: normal_about(1.33, 0.09),
        median: None,
        node_residuals: None,
        limits: (1.13, 2.0),
    },
    SamplerMark {
        name: "log_black_hole_mass",
        // log₁₀ 7, ±0.15 dex, on 3–20 M☉.
        residual: normal_about(0.845_098, 0.15),
        median: None,
        node_residuals: None,
        limits: (0.477_121, 1.301_030),
    },
    SamplerMark {
        name: "neutron_star_progenitor_rank",
        residual: IDENTITY_RANKS,
        median: None,
        node_residuals: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "black_hole_progenitor_rank",
        residual: IDENTITY_RANKS,
        median: None,
        node_residuals: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "age_rank",
        residual: IDENTITY_RANKS,
        median: None,
        node_residuals: None,
        limits: (0.0, 1.0),
    },
];

/// The kinds of the X-ray binaries' class (P11.T8.b), in its sampler's two choices' order: the four
/// low-mass kinds of the old populations, then the two high-mass kinds of the young disc, then the
/// three the carving names (P11.T8.b's fix, drawn from 27 by plan 15's fitted tables): two
/// low-mass, one high-mass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbHostKind {
    /// A neutron star fed by a low-mass donor above the irradiated disc's instability line.
    NeutronStarPersistent,
    /// A neutron star fed by a low-mass donor below the line: outbursts between quiescence.
    NeutronStarTransient,
    /// A black hole fed by a low-mass donor above the line.
    BlackHolePersistent,
    /// A black hole fed by a low-mass donor below the line.
    BlackHoleTransient,
    /// A neutron star beside a Be star, plan 06's.
    BeX,
    /// A neutron star fed by a supergiant filling its Roche lobe.
    Supergiant,
    /// A neutron star fed through its inner Lagrangian point by a helium white dwarf, an
    /// ultracompact X-ray binary, persistent or transient by its disc's instability line.
    Ultracompact,
    /// A neutron star fed by a low-mass giant's wind: a symbiotic X-ray binary (ruling 129.3).
    Symbiotic,
    /// A black hole fed by a supergiant of 20–60 M☉ filling its Roche lobe, as Cyg X-1 nearly
    /// does.
    BlackHoleHighMass,
}

impl XrbHostKind {
    /// Every kind: the six of P11.T8.b's scratch sampler and P15.T10.b's first fit, then the
    /// carving's three.
    pub const ALL: [Self; 9] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
        Self::BeX,
        Self::Supergiant,
        Self::Ultracompact,
        Self::Symbiotic,
        Self::BlackHoleHighMass,
    ];

    /// The low-mass kinds, in [`XrbPick::LowMassKind`]'s order: [`LOW_MASS_HELD`](Self::LOW_MASS_HELD)'s
    /// four, then the ultracompact and symbiotic ones.
    pub const LOW_MASS: [Self; 6] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
        Self::Ultracompact,
        Self::Symbiotic,
    ];

    /// The high-mass kinds, in [`XrbPick::HighMassKind`]'s order:
    /// [`HIGH_MASS_HELD`](Self::HIGH_MASS_HELD)'s two, then the black-hole one.
    pub const HIGH_MASS: [Self; 3] = [Self::BeX, Self::Supergiant, Self::BlackHoleHighMass];

    /// The low-mass kinds P11.T8.b's scratch sampler's choice weighed and P15.T10.b's first fit
    /// fitted, the first of [`LOW_MASS`](Self::LOW_MASS).
    pub const LOW_MASS_HELD: [Self; 4] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
    ];

    /// The high-mass kinds P11.T8.b's scratch sampler's choice weighed and P15.T10.b's first fit
    /// fitted, the first of [`HIGH_MASS`](Self::HIGH_MASS).
    pub const HIGH_MASS_HELD: [Self; 2] = [Self::BeX, Self::Supergiant];

    /// Whether the kind is a high-mass X-ray binary's, whose hosts are the young disc's.
    #[must_use]
    pub const fn is_high_mass(self) -> bool {
        match self {
            Self::BeX | Self::Supergiant | Self::BlackHoleHighMass => true,
            Self::NeutronStarPersistent
            | Self::NeutronStarTransient
            | Self::BlackHolePersistent
            | Self::BlackHoleTransient
            | Self::Ultracompact
            | Self::Symbiotic => false,
        }
    }

    /// Whether the kind is a low-mass transient's, whose outbursts recur: an ultracompact host is
    /// one or not by its own rate, so the kind is not.
    #[must_use]
    pub const fn is_transient(self) -> bool {
        match self {
            Self::NeutronStarTransient | Self::BlackHoleTransient => true,
            Self::NeutronStarPersistent
            | Self::BlackHolePersistent
            | Self::BeX
            | Self::Supergiant
            | Self::Ultracompact
            | Self::Symbiotic
            | Self::BlackHoleHighMass => false,
        }
    }

    /// What the kind's compact star is: a neutron star, or a black hole for the black-hole kinds.
    #[must_use]
    pub const fn accretor(self) -> XrayAccretor {
        match self {
            Self::BlackHolePersistent | Self::BlackHoleTransient | Self::BlackHoleHighMass => {
                XrayAccretor::BlackHole
            }
            Self::NeutronStarPersistent
            | Self::NeutronStarTransient
            | Self::BeX
            | Self::Supergiant
            | Self::Ultracompact
            | Self::Symbiotic => XrayAccretor::NeutronStar,
        }
    }

    /// The binary class P11.T5's `classify` gives a pair of the kind: an ultracompact host's when
    /// it is persistent, a transient one's being its scan marks' (`XrbScanMarks::class`), since
    /// its rate decides it.
    #[must_use]
    pub const fn class(self) -> BinaryClass {
        match self {
            Self::NeutronStarPersistent | Self::BlackHolePersistent | Self::Ultracompact => {
                BinaryClass::LowMassXrayBinary(XrbKind::Persistent)
            }
            Self::NeutronStarTransient | Self::BlackHoleTransient => {
                BinaryClass::LowMassXrayBinary(XrbKind::Transient)
            }
            Self::Symbiotic => BinaryClass::LowMassXrayBinary(XrbKind::Symbiotic),
            Self::BeX => BinaryClass::HighMassXrayBinary(HmxbKind::BeX),
            Self::Supergiant | Self::BlackHoleHighMass => {
                BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant)
            }
        }
    }
}

/// The marks of [`XRB_SAMPLER`], in its order (P11.T8.b): what an X-ray binary is now, by its
/// kind, then its history, then the marks of the three kinds the carving names (P11.T8.b's fix),
/// drawn from 27.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbMark {
    /// A neutron star accretor's mass, M☉.
    NeutronStarMass,
    /// A black hole accretor's mass, M☉.
    BlackHoleMass,
    /// log₁₀ of a low-mass X-ray binary's orbital period, hours.
    LowMassPeriod,
    /// log₁₀ of a low-mass donor's mass, M☉, given log₁₀ of the period, hours.
    LowMassDonorMass,
    /// The transfer rate over the irradiated disc's instability line, dex: below 0 a transient,
    /// at or above it persistent.
    LowMassRateExcess,
    /// log₁₀ of a neutron-star transient's outburst recurrence, years.
    NeutronStarRecurrence,
    /// log₁₀ of a black-hole transient's outburst recurrence, years.
    BlackHoleRecurrence,
    /// log₁₀ of a Be/X-ray binary's orbital period, days.
    BexPeriod,
    /// A Be/X-ray binary's orbital eccentricity.
    BexEccentricity,
    /// A Be/X-ray binary's Be star's initial mass on its own track, M☉.
    BexDonorMass,
    /// log₁₀ of a supergiant X-ray binary's orbital period, days.
    SupergiantPeriod,
    /// A supergiant X-ray binary's donor's mass, M☉.
    SupergiantDonorMass,
    /// log₁₀ of a supergiant X-ray binary's transfer rate, M☉ yr⁻¹.
    SupergiantRate,
    /// A Be/X-ray binary's systemic speed from its supernova, km s⁻¹.
    BexRecoil,
    /// A supergiant X-ray binary's systemic speed from its supernova, km s⁻¹.
    SupergiantRecoil,
    /// log₁₀ of the years since a Be/X-ray binary's supernova.
    BexSinceSupernova,
    /// log₁₀ of the years since a supergiant X-ray binary's supernova.
    SupergiantSinceSupernova,
    /// log₁₀ of a neutron star's progenitor's initial mass, M☉.
    NeutronStarProgenitor,
    /// log₁₀ of a black hole's progenitor's initial mass, M☉.
    BlackHoleProgenitor,
    /// log₁₀ of a low-mass donor's initial mass over its mass now.
    DonorInitialExcess,
    /// log₁₀ of an ultracompact X-ray binary's orbital period, minutes.
    UltracompactPeriod,
    /// log₁₀ of an ultracompact transient's outburst recurrence, years.
    UltracompactRecurrence,
    /// log₁₀ of an ultracompact donor's initial mass, M☉: the star whose core it is.
    UltracompactDonorInitialMass,
    /// log₁₀ of a symbiotic X-ray binary's orbital period, days.
    SymbioticPeriod,
    /// log₁₀ of a symbiotic X-ray binary's giant's mass, M☉.
    SymbioticDonorMass,
    /// A symbiotic X-ray binary's giant's radius over its Roche lobe's.
    SymbioticFilling,
    /// A black-hole high-mass X-ray binary's black hole's mass, M☉.
    BlackHoleHighMassAccretorMass,
    /// log₁₀ of a black-hole high-mass X-ray binary's orbital period, days.
    BlackHoleHighMassPeriod,
    /// A black-hole high-mass X-ray binary's supergiant's mass, M☉.
    BlackHoleHighMassDonorMass,
    /// log₁₀ of a black-hole high-mass X-ray binary's transfer rate, M☉ yr⁻¹.
    BlackHoleHighMassRate,
}

impl XrbMark {
    /// Every mark, in [`XRB_SAMPLER`]'s order.
    pub const ALL: [Self; 30] = [
        Self::NeutronStarMass,
        Self::BlackHoleMass,
        Self::LowMassPeriod,
        Self::LowMassDonorMass,
        Self::LowMassRateExcess,
        Self::NeutronStarRecurrence,
        Self::BlackHoleRecurrence,
        Self::BexPeriod,
        Self::BexEccentricity,
        Self::BexDonorMass,
        Self::SupergiantPeriod,
        Self::SupergiantDonorMass,
        Self::SupergiantRate,
        Self::BexRecoil,
        Self::SupergiantRecoil,
        Self::BexSinceSupernova,
        Self::SupergiantSinceSupernova,
        Self::NeutronStarProgenitor,
        Self::BlackHoleProgenitor,
        Self::DonorInitialExcess,
        Self::UltracompactPeriod,
        Self::UltracompactRecurrence,
        Self::UltracompactDonorInitialMass,
        Self::SymbioticPeriod,
        Self::SymbioticDonorMass,
        Self::SymbioticFilling,
        Self::BlackHoleHighMassAccretorMass,
        Self::BlackHoleHighMassPeriod,
        Self::BlackHoleHighMassDonorMass,
        Self::BlackHoleHighMassRate,
    ];

    /// The marks of the six kinds [`XrbHostKind::LOW_MASS_HELD`] and
    /// [`XrbHostKind::HIGH_MASS_HELD`] hold, the first twenty: those P15.T10.b's first fit fitted.
    pub const HELD: [Self; 20] = {
        let mut out = [Self::NeutronStarMass; 20];
        let mut k = 0;
        while k < 20 {
            out[k] = Self::ALL[k];
            k += 1;
        }
        out
    };

    /// The mark's place in [`XRB_SAMPLER`]'s marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for XrbMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of [`XRB_SAMPLER`], in its order (P11.T8.b): a host's kind on its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbPick {
    /// A low-mass host's kind, among [`XrbHostKind::LOW_MASS`].
    LowMassKind,
    /// A high-mass host's kind, among [`XrbHostKind::HIGH_MASS`].
    HighMassKind,
}

impl SamplerKey for XrbPick {
    fn index(self) -> usize {
        match self {
            Self::LowMassKind => 0,
            Self::HighMassKind => 1,
        }
    }
}

/// The names of [`AwdMark`]'s marks, in its order: each accreting white dwarf sampler's.
const AWD_MARK_NAMES: [&str; 11] = [
    "log_recurrence_years",
    "white_dwarf_mass",
    "log_main_sequence_period_hours",
    "log_helium_period_hours",
    "log_giant_period_hours",
    "log_main_sequence_donor_mass",
    "log_helium_donor_mass",
    "log_giant_donor_mass",
    "log_primary_initial_mass",
    "log_donor_initial_mass",
    "log_age_years",
];

/// Mark `k`'s laws at its median's 17 nodes among `rows`, the 17 laws of each of a sampler's
/// marks in its order, as the fitted blocks hold them.
///
/// # Panics
///
/// If `rows` holds fewer than 17 (`k` + 1) rows: a block built against another mark list.
const fn node_rows<const N: usize>(
    rows: &'static [[f64; 17]; N],
    k: usize,
) -> &'static [[f64; 17]; 17] {
    let (_, rest) = rows.split_at(17 * k);
    rest.first_chunk::<17>()
        .expect("a fitted block holds 17 laws of each mark")
}

/// An accreting white dwarf sampler's marks from a fitted block's arrays, in [`AwdMark`]'s order:
/// each mark's residual quantiles over all its members, its laws at its median's nodes, its
/// median's span and nodes, and its limits (P15.T10.b's refit at 27). The recurrence is
/// unconditional, so its median's row and its node laws are not read.
const fn fitted_awd_marks(
    residuals: &[[f64; 17]; 11],
    node_residuals: &'static [[f64; 17]; 187],
    spans: &[(f64, f64); 11],
    medians: &[[f64; 17]; 11],
    limits: &[(f64, f64); 11],
) -> [SamplerMark; 11] {
    let mut out = [SamplerMark::new("", [0.0; 17], None, (0.0, 0.0)); 11];
    let mut k = 0;
    while k < 11 {
        out[k] = if k == AwdMark::Recurrence.index() {
            SamplerMark::new(AWD_MARK_NAMES[k], residuals[k], None, limits[k])
        } else {
            let median = MedianCurve::new(spans[k].0, spans[k].1, medians[k]);
            SamplerMark::new(AWD_MARK_NAMES[k], residuals[k], Some(median), limits[k])
                .with_node_residuals(node_rows(node_residuals, k))
        };
        k += 1;
    }
    out
}

/// The fast accreting white dwarfs' fitted marks (P15.T10.b), from task `binary_awd`'s block.
const AWD_FAST_MARKS_27: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_FAST_RESIDUALS_27,
    &AWD_FAST_NODE_RESIDUALS_27,
    &AWD_FAST_MEDIAN_SPANS_27,
    &AWD_FAST_MEDIANS_27,
    &AWD_FAST_LIMITS_27,
);

/// The slow accreting white dwarfs' fitted marks (P15.T10.b), from task `binary_awd`'s block.
const AWD_SLOW_MARKS_27: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_SLOW_RESIDUALS_27,
    &AWD_SLOW_NODE_RESIDUALS_27,
    &AWD_SLOW_MEDIAN_SPANS_27,
    &AWD_SLOW_MEDIANS_27,
    &AWD_SLOW_LIMITS_27,
);

/// The fast accreting white dwarfs' sampler fitted against the engine at generator version 27
/// (P15.T10.b's refit): [`AwdMark`]'s marks and the donor's kind, each mark fitted to the engine's
/// fast members alone, so that its recurrence lies all within L and its every rank is the fast
/// hosts'. Each member is weighted by the hosts its donor's kind stands for in the rows, which the
/// kinds' shares are, so that the marks the kinds share follow the observed mix of kinds and not
/// the engine's. Where P11.T8.a's scratch sampler served both classes, split at L, each class has
/// its own: the engine's two sides differ in their donors and rates, and the fast hosts are an
/// eighth of the members, too few of a shared law's 17 ranks. A mark too few members define keeps
/// the scratch sampler's law (task `binary_awd`'s block says which).
pub const AWD_FAST_SAMPLER: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_FAST_MARKS_27,
    &[SamplerPick::new("donor_kind", &AWD_FAST_DONOR_KINDS_27)],
);

/// The slow accreting white dwarfs' sampler fitted against the engine at 27, as
/// [`AWD_FAST_SAMPLER`] is the fast ones', its recurrence all beyond L.
pub const AWD_SLOW_SAMPLER: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_SLOW_MARKS_27,
    &[SamplerPick::new("donor_kind", &AWD_SLOW_DONOR_KINDS_27)],
);

/// The fast accreting white dwarfs' marks refitted at 29 (P15.T10.b), from task `binary_awd`'s
/// block.
const AWD_FAST_MARKS_29: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_FAST_RESIDUALS_29,
    &AWD_FAST_NODE_RESIDUALS_29,
    &AWD_FAST_MEDIAN_SPANS_29,
    &AWD_FAST_MEDIANS_29,
    &AWD_FAST_LIMITS_29,
);

/// The slow accreting white dwarfs' marks refitted at 29 (P15.T10.b), from task `binary_awd`'s
/// block.
const AWD_SLOW_MARKS_29: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_SLOW_RESIDUALS_29,
    &AWD_SLOW_NODE_RESIDUALS_29,
    &AWD_SLOW_MEDIAN_SPANS_29,
    &AWD_SLOW_MEDIANS_29,
    &AWD_SLOW_LIMITS_29,
);

/// The fast accreting white dwarfs' sampler refitted at generator version 29, as
/// [`AWD_FAST_SAMPLER`] was fitted at 27, on the census of the pairs the generator itself draws at
/// a record's first attempt (`stellar::binary::testing::class_census_in`, train 27's finding):
/// under the record's own multiplicity, a displaced record single, and on its primary's kept kick.
/// A galaxy's classes read it from 29 (`BinaryTableRules::Published`).
pub const AWD_FAST_SAMPLER_29: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_FAST_MARKS_29,
    &[SamplerPick::new("donor_kind", &AWD_FAST_DONOR_KINDS_29)],
);

/// The slow accreting white dwarfs' sampler refitted at 29, as [`AWD_FAST_SAMPLER_29`] is the
/// fast ones'.
pub const AWD_SLOW_SAMPLER_29: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_SLOW_MARKS_29,
    &[SamplerPick::new("donor_kind", &AWD_SLOW_DONOR_KINDS_29)],
);

/// The names of [`XrbMark`]'s marks, in its order, as [`XRB_SAMPLER`] names them.
const XRB_MARK_NAMES: [&str; 30] = [
    "neutron_star_mass",
    "black_hole_mass",
    "log_low_mass_period_hours",
    "log_low_mass_donor_mass",
    "low_mass_rate_excess_dex",
    "log_neutron_star_recurrence_years",
    "log_black_hole_recurrence_years",
    "log_bex_period_days",
    "bex_eccentricity",
    "bex_donor_mass",
    "log_supergiant_period_days",
    "supergiant_donor_mass",
    "log_supergiant_rate",
    "bex_recoil_km_s",
    "supergiant_recoil_km_s",
    "log_bex_years_since_supernova",
    "log_supergiant_years_since_supernova",
    "log_neutron_star_progenitor_mass",
    "log_black_hole_progenitor_mass",
    "log_donor_initial_excess",
    "log_ultracompact_period_minutes",
    "log_ultracompact_recurrence_years",
    "log_ultracompact_donor_initial_mass",
    "log_symbiotic_period_days",
    "log_symbiotic_donor_mass",
    "symbiotic_filling",
    "black_hole_high_mass_accretor_mass",
    "log_black_hole_high_mass_period_days",
    "black_hole_high_mass_donor_mass",
    "log_black_hole_high_mass_rate",
];

/// The X-ray binaries' fitted marks (P15.T10.b's refit at 27) from task `binary_xrb`'s block, in
/// [`XrbMark`]'s order: each mark's residual quantiles over all its members, its median's span
/// and nodes, and its limits. Only the low-mass donor's mass follows a condition, the period, with
/// its laws at its median's nodes `donor_mass_nodes`; the other marks' median rows are not read.
const fn fitted_xrb_marks(
    residuals: &[[f64; 17]; 30],
    donor_mass_nodes: &'static [[f64; 17]; 17],
    spans: &[(f64, f64); 30],
    medians: &[[f64; 17]; 30],
    limits: &[(f64, f64); 30],
) -> [SamplerMark; 30] {
    let mut out = [SamplerMark::new("", [0.0; 17], None, (0.0, 0.0)); 30];
    let mut k = 0;
    while k < 30 {
        out[k] = if k == XrbMark::LowMassDonorMass.index() {
            let median = MedianCurve::new(spans[k].0, spans[k].1, medians[k]);
            SamplerMark::new(XRB_MARK_NAMES[k], residuals[k], Some(median), limits[k])
                .with_node_residuals(donor_mass_nodes)
        } else {
            SamplerMark::new(XRB_MARK_NAMES[k], residuals[k], None, limits[k])
        };
        k += 1;
    }
    out
}

/// The X-ray binaries' fitted marks.
const XRB_MARKS_27: [SamplerMark; 30] = fitted_xrb_marks(
    &XRB_RESIDUALS_27,
    &XRB_DONOR_MASS_NODE_RESIDUALS_27,
    &XRB_MEDIAN_SPANS_27,
    &XRB_MEDIANS_27,
    &XRB_LIMITS_27,
);

/// The X-ray binaries' sampler fitted against the engine at generator version 27 (P15.T10.b's
/// refit): [`XrbMark`]'s marks and the two kind choices over all nine kinds the carving counts,
/// fitted to the engine's members of each kind, each mark too few members define kept as
/// P11.T8.b's hand-made scratch sampler had it, from the observed figures of plan 11's design note
/// 13 (task `binary_xrb`'s block says which). Each kind is weighted by the galaxy's share of the
/// engine's members of it, but the black-hole transients, pinned at
/// [`BLACK_HOLE_TRANSIENT_SHARE`], so that the kinds the class draws are those its row counts
/// (P11.T8.b's fix).
pub const XRB_SAMPLER: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    &XRB_MARKS_27,
    &[
        SamplerPick::new("low_mass_kind", &XRB_LOW_MASS_KINDS_27),
        SamplerPick::new("high_mass_kind", &XRB_HIGH_MASS_KINDS_27),
    ],
);

/// The X-ray binaries' marks refitted at 29, from task `binary_xrb`'s block.
const XRB_MARKS_29: [SamplerMark; 30] = fitted_xrb_marks(
    &XRB_RESIDUALS_29,
    &XRB_DONOR_MASS_NODE_RESIDUALS_29,
    &XRB_MEDIAN_SPANS_29,
    &XRB_MEDIANS_29,
    &XRB_LIMITS_29,
);

/// The X-ray binaries' sampler refitted at generator version 29, as [`XRB_SAMPLER`] was fitted at
/// 27, on the census of the pairs the generator itself draws at a record's first attempt, as
/// [`AWD_FAST_SAMPLER_29`] is. A galaxy's class reads it from 29
/// (`BinaryTableRules::Published`).
pub const XRB_SAMPLER_29: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    &XRB_MARKS_29,
    &[
        SamplerPick::new("low_mass_kind", &XRB_LOW_MASS_KINDS_29),
        SamplerPick::new("high_mass_kind", &XRB_HIGH_MASS_KINDS_29),
    ],
);

/// The black-hole transients' share of the X-ray binaries from 27: 0.13, the 1,300 of the row's
/// 10⁴ hosts that P11.T8.b's design takes, Corral-Santana et al.'s (2016, A&A 587, A61; recalled,
/// not read at source) 1,280 ± 120, which they call a lower limit (research's ruling of
/// 2026-11-02). The census gives 0.0196, about 190 (a finding: likely the common envelopes that
/// survive at `α_CE λ` = 0.5, Podsiadlowski et al. 2003, MNRAS 341, 385, recalled; neither
/// `α_CE λ` nor the kicks are touched, which would move every class after a common envelope).
pub const BLACK_HOLE_TRANSIENT_SHARE: f64 = 0.13;

/// The heaviest white dwarf an accreting host's fitted sampler draws, M☉: 1.38, at which an
/// accreting dwarf ignites carbon or, of oxygen and neon, collapses (Nomoto and Kondo 1991, ApJ
/// 367, L19; recalled), under the Chandrasekhar mass of plan 06's remnants, 1.44 M☉, under which the
/// engine's own dwarfs stop from 27 (P11.T4's `dwarf_limit`) and at which `NovaCurve::of`
/// refuses one. The cold mass–radius relation the light curves read shrinks a dwarf to nothing at
/// 1.44 M☉, so its dwarf novae's energy grows without bound there: 5,517 years' fade at 1.36 M☉,
/// 6,488 at 1.38 and 188,000 at the engine's heaviest, 1.4399. `binary_awd` holds its fitted
/// white dwarf mass's limit at it (P15.T10.b's refit at 27).
pub const ACCRETING_WHITE_DWARF_MAX_MASS: f64 = 1.38;

/// The share rows (P11.T7, P15.T10.b; module documentation): the accreting white dwarfs' rows
/// fitted against the engine at generator version 27 from task `binary_awd`'s block, each donor
/// kind scaled to its observed figure, and the X-ray binaries' from task `binary_xrb`'s, and the
/// two merger classes' scratch until their own fits, 9 × 10⁴ stellar mergers and 10 neutron-star
/// mergers in the source horizon at Milky Way parameters.
pub const CLASS_SHARES: ClassShareTable = ClassShareTable {
    rows: [
        ClassShareRow {
            class: CarvedClass::AccretingWdFast,
            hosts_per_formed_mass: AWD_FAST_HOSTS_PER_FORMED_MASS_27,
            layer_shares: AWD_FAST_LAYER_SHARES_27,
        },
        ClassShareRow {
            class: CarvedClass::AccretingWdSlow,
            hosts_per_formed_mass: AWD_SLOW_HOSTS_PER_FORMED_MASS_27,
            layer_shares: AWD_SLOW_LAYER_SHARES_27,
        },
        ClassShareRow {
            class: CarvedClass::XrayBinary,
            hosts_per_formed_mass: XRB_HOSTS_PER_FORMED_MASS_27,
            layer_shares: XRB_LAYER_SHARES_27,
        },
        uniform(
            CarvedClass::StellarMerger,
            9.0e4,
            [0.1, 0.2, 0.5, 0.15, 0.05],
        ),
        uniform(
            CarvedClass::NeutronStarMerger,
            10.0,
            [0.0, 0.0, 0.0, 0.0, 1.0],
        ),
    ],
};

/// The share rows refitted at generator version 29, as [`CLASS_SHARES`]' were fitted at 27: the
/// accreting white dwarfs' rows from task `binary_awd`'s block and the X-ray binaries' from task
/// `binary_xrb`'s, on the census of the pairs the generator itself draws, and the merger classes'
/// scratch rows as [`CLASS_SHARES`] holds them. A galaxy reads it from 29
/// (`BinaryTableRules::Published`).
pub const CLASS_SHARES_29: ClassShareTable = ClassShareTable {
    rows: [
        ClassShareRow {
            class: CarvedClass::AccretingWdFast,
            hosts_per_formed_mass: AWD_FAST_HOSTS_PER_FORMED_MASS_29,
            layer_shares: AWD_FAST_LAYER_SHARES_29,
        },
        ClassShareRow {
            class: CarvedClass::AccretingWdSlow,
            hosts_per_formed_mass: AWD_SLOW_HOSTS_PER_FORMED_MASS_29,
            layer_shares: AWD_SLOW_LAYER_SHARES_29,
        },
        ClassShareRow {
            class: CarvedClass::XrayBinary,
            hosts_per_formed_mass: XRB_HOSTS_PER_FORMED_MASS_29,
            layer_shares: XRB_LAYER_SHARES_29,
        },
        CLASS_SHARES.rows[3],
        CLASS_SHARES.rows[4],
    ],
};

// @begin-table binary_awd
// The accreting white dwarfs' samplers and share rows refitted against plan 11's engine at
// generator version 29 on the generator's own pairs (plan 15, P15.T10.b), held until 29.
//
// @generated by hyperion-fit 0.1.0, task `binary_awd` revision 0. Do not edit.
// inputs-sha256: e71e8ac98ac7f0c000eaa965a97a406fe701ad0d655697e02aa4b58160f3c9aa
// manifest: crates/hyperion-fit/manifests/binary_awd.toml
// data: none
// sim-fingerprint: e90be681791ba21923f19dd95e44fb857e830ac8e3e63be0947a49dcc67daf33
// since-generator-version: 28
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the rows by donor kind against Pala et al. 2020, MNRAS 494, 3799, Carter et al. 2013,
//   MNRAS 429, 2143, and Lü, Yungelson and Han 2006, MNRAS 372, 1389, the total against the
//   brainstorm's 6–12 × 10⁶
// acceptance: kind scales 1.6742e-1/7.4770e-2/1.0658e-2; fast: 2485 members, own nodes
//   0/15/14/0/6/17/0/4/16/17/17, donors 0.992/0.000/0.008, layers C 0.108 D 0.873 E 0.019, rates
//   a system 7.01e-7/5.46e-5/1.11e-5/1.54e-5/1.96e-5/2.03e-5/6.31e-6, not its own: HeliumPeriod
//   Scratch, HeliumDonorMass Scratch; slow: 25335 members, own nodes
//   0/16/17/16/5/17/16/9/17/17/17, donors 0.892/0.108/0.001, layers C 0.379 D 0.610 E 0.011,
//   rates a system 4.91e-6/5.17e-4/8.48e-4/6.97e-4/6.23e-4/6.20e-4/9.99e-4; the K–S tests
//   against the other half of the census are the sim's slow
//   `fitted_tables::the_accreting_white_dwarf_samplers_against_the_engine`
//
// Read by `AWD_FAST_SAMPLER_29`, `AWD_SLOW_SAMPLER_29` and `CLASS_SHARES_29`, after the block. Each
// class's marks are fitted to its own members of the engine near the solar circle at generator
// version 29's rules (hyperion-fit's `tasks::binary_classes`): a median that follows the
// mark's condition on 17 nodes, the weighted median under a triangle two node spacings wide, the
// residual's quantiles about it, and its law at each node, from the members under the node's
// triangle where their Kish size is 50 or more. Each mark's members are weighted so that their
// donor kinds are in the class's shares of them. A mark with under 50 members of its class
// takes both classes' members, and with under 50 of both keeps `AWD_SAMPLER`'s law. The fast
// recurrence's lowest quantile keeps its members' mean of 1 ÷ P over their lowest sixteenth. A
// slow host's recurrence is held at 10^10 years, under the universe clock's reach; the
// engine's longest, beyond it, never erupt in the Galaxy's age. The rows scale each donor kind to
// its observed figure (`donor_rows::OBSERVED_DONOR_KINDS`, plan 11's design note 11), each kind's
// rates each population's members a record, its bands weighted by their shares of the systems.
// The slab's census ran 10000064 records and the populations' own 1500320.

/// The fast accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16 over all their
/// members, in [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_FAST_RESIDUALS_29: [[f64; 17]; 11] = [
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [-0.922_634_256_169_204_2, -0.361_969_198_667_693_5, -0.248_071_415_069_036_88, -0.190_083_435_588_353_33, -0.147_635_217_449_392_34, -0.108_592_056_128_848_05, -0.074_085_208_785_369_62, -0.029_534_995_548_012, 0.000_814_077_816_858_601_2, 0.027_269_216_887_740_887, 0.050_804_299_580_549_58, 0.077_191_107_624_453_61, 0.108_404_078_611_272_92, 0.151_683_476_592_504, 0.186_856_442_355_286_23, 0.217_232_239_119_247_8, 0.321_451_489_320_368_4],
    [-1.285_704_800_787_205, -0.464_831_108_285_449_05, -0.176_247_932_968_776_83, -0.115_752_450_262_393_96, -0.089_599_483_683_302_35, -0.068_143_637_879_168_19, -0.045_632_430_415_177_63, -0.022_301_656_510_556_61, 0.000_403_729_226_754_272_45, 0.021_146_610_757_352_823, 0.051_735_768_078_134_96, 0.135_460_558_470_869_82, 0.230_829_080_129_971_97, 0.361_672_732_292_111_8, 0.492_248_427_664_800_8, 0.654_642_720_846_948_3, 1.265_852_495_398_798_4],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.651_690_085_044_548_2, -0.200_558_450_186_673_12, -0.133_114_678_816_823_35, -0.095_897_676_336_585_41, -0.072_861_105_860_800_93, -0.052_201_468_115_588_78, -0.036_440_987_029_110_08, -0.019_510_184_817_789_874, -0.005_530_147_271_326_538_5, 0.007_726_255_273_099_811, 0.020_991_406_440_252_037, 0.035_903_990_975_673_51, 0.051_726_457_701_499_45, 0.073_876_521_987_175_52, 0.097_425_862_611_638_46, 0.141_540_338_379_512_96, 0.743_998_532_898_900_3],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.285_608_117_672_971_7, -0.124_116_024_703_418_44, -0.088_080_780_851_145_35, -0.067_704_567_293_263_16, -0.052_462_654_840_656_01, -0.041_789_922_812_171_94, -0.031_138_869_883_540_456, -0.019_473_063_842_764_878, -0.005_892_899_125_237_608, 0.011_506_047_694_288_925, 0.028_114_066_507_499_18, 0.047_122_455_621_675_004, 0.063_877_521_360_608_95, 0.079_988_973_914_733_3, 0.103_854_992_644_342_93, 0.171_682_850_136_248, 0.326_427_937_454_586_76],
    [-0.630_822_490_633_083_9, -0.250_548_837_428_730_85, -0.191_871_189_440_688_18, -0.158_375_204_335_877_1, -0.116_937_955_779_956_3, -0.080_362_457_883_472_21, -0.048_650_233_592_546_34, -0.021_741_930_060_313_726, 0.001_338_165_107_857_199_4, 0.021_792_775_111_543_64, 0.042_539_161_139_973_58, 0.063_050_818_104_862_68, 0.080_974_569_709_732_34, 0.099_646_534_278_381_8, 0.121_236_473_766_341_52, 0.157_602_183_187_945_2, 0.375_615_389_633_816_24],
    [-1.420_014_003_199_247_2, -0.495_190_555_364_877_27, -0.351_325_058_810_437_9, -0.272_532_179_889_879_1, -0.206_878_215_437_946_5, -0.148_294_391_045_467_6, -0.099_019_493_602_360_84, -0.051_381_416_918_242_453, 0.001_214_967_608_208_082_6, 0.051_040_229_511_373_485, 0.102_632_264_537_521_96, 0.148_556_417_047_897_68, 0.204_233_496_777_002_84, 0.261_750_286_101_321_4, 0.326_901_834_735_177_25, 0.438_445_156_228_527_53, 0.695_692_761_871_830_7],
];

/// The fast accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_FAST_NODE_RESIDUALS_29: [[f64; 17]; 187] = [
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [2.599_703_588_463_843_8, 3.728_913_414_372_182_7, 4.107_281_809_051_119, 4.360_733_551_257_122, 4.529_355_753_711_830_5, 4.651_072_773_129_552_5, 4.747_394_085_346_889_4, 4.835_500_844_832_826_5, 4.897_266_243_433_534, 4.970_655_035_768_159, 5.042_515_192_958_676_5, 5.122_174_169_624_547_5, 5.186_436_904_668_856, 5.247_636_734_677_315, 5.306_104_469_465, 5.358_214_328_201_519_5, 5.418_370_850_344_337_5],
    [-0.922_634_256_169_204_2, -0.361_969_198_667_693_5, -0.248_071_415_069_036_88, -0.190_083_435_588_353_33, -0.147_635_217_449_392_34, -0.108_592_056_128_848_05, -0.074_085_208_785_369_62, -0.029_534_995_548_012, 0.000_814_077_816_858_601_2, 0.027_269_216_887_740_887, 0.050_804_299_580_549_58, 0.077_191_107_624_453_61, 0.108_404_078_611_272_92, 0.151_683_476_592_504, 0.186_856_442_355_286_23, 0.217_232_239_119_247_8, 0.321_451_489_320_368_4],
    [-0.922_634_256_169_204_2, -0.361_969_198_667_693_5, -0.248_071_415_069_036_88, -0.190_083_435_588_353_33, -0.147_635_217_449_392_34, -0.108_592_056_128_848_05, -0.074_085_208_785_369_62, -0.029_534_995_548_012, 0.000_814_077_816_858_601_2, 0.027_269_216_887_740_887, 0.050_804_299_580_549_58, 0.077_191_107_624_453_61, 0.108_404_078_611_272_92, 0.151_683_476_592_504, 0.186_856_442_355_286_23, 0.217_232_239_119_247_8, 0.321_451_489_320_368_4],
    [-0.922_634_256_169_204_2, -0.872_819_863_587_686_3, -0.654_596_068_685_706_7, -0.589_652_095_435_074_7, -0.377_248_173_144_545_6, -0.290_150_587_319_732_5, -0.170_109_127_405_440_62, -0.078_797_295_435_892_45, -0.001_938_363_658_796_413_5, 0.038_548_023_958_466_98, 0.057_330_733_301_418_475, 0.079_830_465_399_293_81, 0.095_601_922_841_655_5, 0.126_838_728_561_761_23, 0.153_078_153_336_365_54, 0.180_057_051_335_094_3, 0.196_840_431_624_774_05],
    [-0.922_634_256_169_204_2, -0.774_438_030_517_163_8, -0.644_567_204_525_517_4, -0.576_295_651_531_618_3, -0.344_555_503_929_404_15, -0.286_933_771_263_966_24, -0.207_375_595_640_508_95, -0.135_642_188_289_756_66, -0.005_538_186_497_110_764_5, 0.038_955_839_994_074_04, 0.057_119_579_551_745_96, 0.082_921_850_997_044_08, 0.113_369_344_752_576_51, 0.148_233_815_369_360_37, 0.165_189_492_191_412_63, 0.181_866_255_423_994_68, 0.196_840_431_624_774_05],
    [-0.885_696_566_753_257, -0.709_515_828_652_25, -0.636_201_863_183_898_3, -0.365_209_550_882_034_33, -0.290_176_622_401_324_3, -0.220_027_874_171_110_36, -0.158_230_919_527_003_8, -0.080_027_742_612_097_01, -0.002_768_260_538_207_041_7, 0.038_357_113_179_521_71, 0.049_700_466_523_721_14, 0.075_753_593_220_870_65, 0.099_245_732_523_006_89, 0.144_092_927_427_051_96, 0.167_129_620_260_654_48, 0.181_820_584_917_543_63, 0.196_840_431_624_774_05],
    [-0.885_696_566_753_257, -0.690_130_046_804_938_7, -0.318_172_733_091_536_04, -0.228_062_001_084_040_87, -0.167_494_465_634_982_9, -0.111_225_534_961_160_12, -0.075_892_247_309_410_26, -0.020_968_244_227_766_622, 0.011_074_618_502_092_4, 0.043_541_840_629_605_8, 0.057_844_651_852_621_075, 0.082_068_258_962_239_45, 0.140_525_926_186_051_53, 0.164_681_370_805_330_8, 0.176_102_732_053_599_94, 0.188_574_519_424_507_54, 0.214_921_763_234_864_12],
    [-0.885_696_566_753_257, -0.315_324_846_661_398_26, -0.199_999_571_845_319_8, -0.164_526_509_906_361_22, -0.117_787_175_377_011_89, -0.089_111_102_663_991_58, -0.046_293_466_738_164_81, -0.011_503_933_452_228_953, 0.011_589_122_539_670_064, 0.044_572_598_247_364_995, 0.073_898_278_633_202_71, 0.113_090_116_903_874_12, 0.153_661_890_303_697_56, 0.167_875_408_158_075_03, 0.187_862_482_604_085_17, 0.194_922_586_187_680_6, 0.240_252_153_643_639_05],
    [-0.758_681_539_205_383_4, -0.287_305_113_318_404_1, -0.174_630_501_742_636_67, -0.142_876_573_925_200_42, -0.120_248_652_077_045_5, -0.089_133_771_684_429_68, -0.055_241_577_021_145_18, -0.020_726_441_577_336_03, 0.007_166_328_586_108_725, 0.028_024_419_030_832_148, 0.056_953_635_013_946_84, 0.100_565_830_348_863_71, 0.147_169_705_900_642_88, 0.168_604_183_127_557_32, 0.192_377_971_473_833_8, 0.211_956_838_754_969_62, 0.273_301_653_876_705_24],
    [-0.758_681_539_205_383_4, -0.266_367_359_614_726_13, -0.161_740_646_282_823, -0.140_284_994_169_494_64, -0.115_630_439_656_727_16, -0.074_824_149_844_464_77, -0.050_819_797_230_548_59, -0.022_051_436_128_834_666, 0.003_493_609_402_132_71, 0.022_105_880_906_833_25, 0.047_372_819_344_115_19, 0.089_885_794_763_209_21, 0.135_448_423_528_950_48, 0.182_218_966_008_182_7, 0.206_584_719_058_525_5, 0.232_115_654_468_627_95, 0.283_341_321_697_653_9],
    [-0.758_681_539_205_383_4, -0.279_855_086_919_899_2, -0.177_807_163_668_194_35, -0.126_645_115_232_989_16, -0.107_608_727_140_843_46, -0.076_266_690_966_318_5, -0.050_875_357_856_293_7, -0.025_911_274_101_450_87, 0.003_240_819_373_834_304_5, 0.021_939_893_885_702_5, 0.040_657_930_544_151_455, 0.076_326_333_364_672_07, 0.124_975_756_269_522_23, 0.177_357_598_617_949_25, 0.211_240_883_022_607_46, 0.238_683_627_366_577_08, 0.304_104_228_269_070_46],
    [-0.739_319_459_598_245_5, -0.305_427_213_466_875_6, -0.205_794_499_732_981_42, -0.141_274_157_009_909_1, -0.113_729_937_091_518_9, -0.090_763_788_117_581_64, -0.067_432_777_501_990_84, -0.034_450_209_233_064_43, -0.001_241_855_268_163_200_9, 0.021_546_531_173_706_387, 0.041_490_346_258_613_15, 0.074_664_609_032_015_24, 0.106_465_123_757_446_14, 0.170_344_915_644_652_76, 0.208_618_776_598_373_13, 0.235_546_775_885_180_66, 0.304_104_228_269_070_46],
    [-0.739_319_459_598_245_5, -0.329_519_547_326_147, -0.219_466_705_674_571_4, -0.161_894_638_861_468_93, -0.124_621_159_257_657_67, -0.096_397_925_382_855_15, -0.071_736_846_322_629_76, -0.030_030_132_858_692_413, 0.000_749_868_574_089_820_3, 0.028_073_561_731_678_645, 0.051_104_932_121_884_77, 0.079_813_404_730_862_54, 0.119_263_195_868_958_49, 0.175_462_111_980_509, 0.213_346_154_209_236_83, 0.234_772_264_850_269_53, 0.317_923_727_483_807_4],
    [-0.708_601_071_652_949_6, -0.342_892_244_698_397_5, -0.241_339_497_668_624_9, -0.180_109_071_683_662_3, -0.136_627_009_685_340_2, -0.107_072_877_617_424_75, -0.072_918_758_639_340_89, -0.028_363_358_719_751_07, 0.003_327_769_564_043_487_7, 0.028_771_067_486_783_947, 0.059_344_981_398_079_096, 0.091_876_008_625_851_42, 0.125_703_012_760_282_15, 0.179_495_163_310_628_7, 0.210_741_246_005_634_82, 0.232_036_831_058_582_58, 0.321_451_489_320_368_4],
    [-0.708_601_071_652_949_6, -0.368_194_901_640_107_65, -0.255_548_760_641_37, -0.202_165_423_279_346_92, -0.155_155_574_012_516_5, -0.119_225_894_391_572_26, -0.084_764_813_890_791_99, -0.034_224_931_594_386_69, -0.000_457_959_237_900_983_5, 0.025_712_863_775_393_74, 0.058_264_799_758_974_14, 0.088_364_100_584_702_11, 0.121_509_374_605_307_24, 0.161_708_235_232_538_22, 0.200_066_668_224_881_3, 0.224_202_804_255_782_1, 0.321_451_489_320_368_4],
    [-0.698_515_181_489_055_3, -0.359_535_116_736_17, -0.253_757_870_161_023_86, -0.207_587_472_919_051_44, -0.167_988_133_244_532_77, -0.127_045_957_931_127_36, -0.087_451_022_948_914_03, -0.035_678_814_553_161_56, -0.001_238_123_993_935_79, 0.027_140_730_727_124_71, 0.053_940_101_710_446_8, 0.077_692_054_558_764_05, 0.100_528_363_814_868_27, 0.142_405_311_664_682_48, 0.180_233_162_981_840_75, 0.211_231_807_903_854_93, 0.321_451_489_320_368_4],
    [-0.575_577_509_043_434_9, -0.338_664_317_459_559_6, -0.249_736_576_734_301_36, -0.204_783_950_700_393_96, -0.167_607_098_793_714_3, -0.124_311_776_279_734_97, -0.083_380_312_815_250_3, -0.033_116_326_178_181_825, 0.000_585_896_268_571_248_2, 0.027_794_494_846_722_96, 0.049_730_561_884_876_914, 0.072_790_939_750_220_59, 0.095_800_045_763_389_2, 0.134_022_108_554_597_95, 0.165_006_244_879_463_44, 0.197_284_740_517_586_52, 0.321_451_489_320_368_4],
    [-0.575_577_509_043_434_9, -0.337_640_982_890_537_8, -0.260_117_794_382_987_27, -0.207_402_104_458_960_37, -0.167_565_176_582_425_84, -0.126_938_180_144_325_92, -0.081_233_150_167_690_62, -0.032_833_080_308_847_81, -0.002_624_097_315_945_46, 0.026_142_395_104_057_373, 0.046_066_353_988_567_12, 0.068_819_275_253_952_25, 0.095_700_367_825_963_21, 0.133_930_940_874_407_18, 0.161_128_560_136_695_63, 0.194_294_353_618_752_66, 0.287_433_930_035_564_83],
    [-0.507_776_542_754_278_6, -0.458_967_333_536_830_7, -0.191_708_029_065_303_9, -0.145_000_948_999_406_52, -0.077_639_500_392_902_3, -0.040_501_876_466_281_85, 0.003_672_783_520_429_681_3, 0.019_787_920_364_980_022, 0.032_906_494_441_268_14, 0.051_005_475_524_290_93, 0.113_936_352_937_284_98, 0.136_496_228_218_906_92, 0.142_552_663_653_782_89, 0.155_425_203_166_541_13, 0.169_736_043_975_066_9, 0.191_233_571_422_748_74, 1.265_852_495_398_798_4],
    [-0.669_408_177_640_534_7, -0.348_612_209_186_689_3, -0.233_222_931_472_566_92, -0.177_053_519_285_586_88, -0.147_989_366_925_334_24, -0.125_857_818_551_909_58, -0.077_566_212_848_115_57, -0.052_440_080_030_448_22, -0.008_066_364_101_269_296, 0.007_469_117_954_058_737, 0.032_510_889_745_036_08, 0.045_260_931_424_949_497, 0.096_782_054_059_492_99, 0.140_912_524_720_468_56, 0.165_621_478_258_537_99, 0.610_006_613_219_410_9, 1.265_852_495_398_798_4],
    [-1.207_613_885_720_052_2, -0.459_002_738_387_999_4, -0.316_002_064_894_948_14, -0.255_583_960_827_257_1, -0.205_071_409_298_805_13, -0.160_782_263_291_411_93, -0.130_091_701_963_751_83, -0.080_821_843_149_059_87, -0.053_840_506_981_319_786, -0.013_649_410_684_389_678, 0.018_356_255_639_465_222, 0.049_695_992_080_051_044, 0.143_739_755_948_888_44, 0.401_081_202_776_219_56, 0.557_161_290_378_059_1, 0.663_998_020_249_042_6, 1.265_852_495_398_798_4],
    [-1.273_824_624_350_113, -0.505_310_512_993_950_2, -0.347_822_925_114_922_63, -0.280_908_115_419_771_2, -0.231_969_878_767_850_38, -0.158_422_494_110_051_8, -0.095_209_063_935_157_56, -0.053_448_222_652_958_32, -0.001_802_070_505_376_328_3, 0.036_113_157_102_450_85, 0.074_032_958_176_614_64, 0.135_848_667_695_581_02, 0.228_827_277_320_675_76, 0.378_429_967_661_712_5, 0.497_654_912_162_603_3, 0.631_056_554_826_582_2, 1.265_852_495_398_798_4],
    [-1.285_704_800_787_205, -0.739_277_874_878_581_5, -0.341_936_742_970_115_56, -0.187_105_259_027_405_13, -0.094_041_527_574_594_46, -0.042_500_854_285_256_19, -0.007_286_462_588_312_33, 0.014_744_908_361_830_249, 0.033_131_242_277_509_03, 0.048_704_702_252_990_5, 0.076_533_723_640_030_28, 0.106_157_700_207_993_41, 0.158_092_531_319_783_37, 0.200_788_225_305_040_4, 0.287_569_434_353_124_04, 0.439_924_934_815_800_1, 1.051_554_977_124_532_3],
    [-1.285_704_800_787_205, -1.210_087_381_768_754_7, -0.141_559_758_263_760_22, -0.090_304_342_912_515_35, -0.053_645_333_670_443_41, -0.028_563_612_406_606_81, -0.010_338_934_066_563_991, 0.004_516_846_830_504_496_5, 0.017_387_829_053_698_158, 0.029_442_523_911_077_774, 0.042_671_400_727_289_49, 0.065_041_978_846_200_01, 0.096_195_988_373_812_82, 0.143_521_540_938_231_97, 0.200_825_623_995_997_07, 0.321_014_677_423_294_76, 1.051_554_977_124_532_3],
    [-1.285_704_800_787_205, -1.209_193_211_832_058_5, -0.108_253_929_928_339_32, -0.074_879_005_832_370_1, -0.056_668_257_177_774_96, -0.038_198_247_681_218_926, -0.023_684_619_982_991_616, -0.011_028_721_398_656_334, 0.001_385_657_529_184_332_4, 0.011_506_737_945_269_144, 0.021_931_579_801_780_645, 0.040_552_348_602_918_81, 0.086_565_005_724_363_5, 0.177_874_077_810_163_57, 0.281_399_280_570_938_27, 0.474_221_720_914_339_06, 0.897_106_360_089_383_3],
    [-1.285_704_800_787_205, -0.333_050_247_724_717_86, -0.092_232_609_922_097_84, -0.075_406_258_302_788_46, -0.061_485_592_420_468_66, -0.046_570_239_318_543_24, -0.034_376_324_558_332_616, -0.020_984_087_137_980_87, -0.008_552_192_194_869_778, 0.004_102_875_892_299_317_5, 0.019_187_653_097_532_16, 0.051_492_297_313_843_985, 0.195_790_293_666_034_93, 0.320_605_273_483_596_13, 0.443_790_374_148_521_2, 0.607_265_519_860_806_3, 1.024_647_727_301_104_8],
    [-1.281_274_243_793_870_7, -0.122_675_940_580_583_19, -0.098_976_826_817_149_25, -0.087_584_927_755_048_39, -0.075_294_593_036_591_08, -0.063_376_536_172_386_91, -0.048_855_609_581_600_17, -0.035_044_890_041_352_61, -0.017_818_633_494_133_082, 0.004_129_735_523_317_865_4, 0.079_975_805_796_931_23, 0.268_302_328_748_085_35, 0.345_264_044_463_365_84, 0.426_220_318_620_525_95, 0.547_175_079_975_387_1, 0.701_262_016_533_898_5, 1.040_356_776_942_115_4],
    [-1.281_274_243_793_870_7, -0.139_524_917_658_815_44, -0.115_235_199_436_305_21, -0.100_309_040_278_979_08, -0.090_266_210_200_210_49, -0.079_214_737_208_917_14, -0.065_697_470_304_876_55, -0.048_863_515_311_249_49, -0.024_965_583_450_638_214, 0.137_302_791_766_447_58, 0.296_103_750_604_279_46, 0.354_282_185_281_056_7, 0.416_810_991_879_958_07, 0.519_564_180_750_360_6, 0.645_516_590_041_11, 0.747_802_164_640_413_6, 1.040_356_776_942_115_4],
    [-0.554_671_084_761_598_3, -0.165_745_950_236_715_1, -0.145_618_178_827_096_02, -0.124_795_006_778_039_34, -0.111_309_359_154_209_67, -0.095_334_407_740_210_56, -0.081_494_869_282_484_08, -0.062_434_582_769_439_47, -0.031_452_135_445_032_75, 0.269_725_965_423_133_15, 0.353_727_102_909_466_6, 0.402_008_231_596_185_4, 0.494_417_485_090_892_84, 0.612_691_270_001_088_7, 0.704_721_511_114_788_1, 0.793_581_672_550_168_1, 1.040_356_776_942_115_4],
    [-0.807_115_648_352_569_7, -0.216_996_765_071_156_1, -0.175_429_948_029_996_34, -0.157_642_575_526_595_52, -0.145_800_695_055_874_37, -0.130_065_413_420_462_57, -0.108_937_970_169_331_14, -0.075_411_997_523_555_12, -0.048_201_357_577_747_635, 0.299_456_252_934_871_1, 0.403_416_009_267_446_86, 0.504_806_527_121_947_7, 0.585_188_516_050_018_8, 0.648_058_789_742_443_6, 0.738_949_426_120_685, 0.820_391_112_539_183, 1.040_356_776_942_115_4],
    [-0.900_843_076_480_083_2, -0.466_787_840_296_266_4, -0.379_151_590_132_657_1, -0.239_138_231_428_974_64, -0.195_385_262_787_558_33, -0.175_430_159_170_610_03, -0.151_170_510_290_549_26, -0.116_279_257_457_789_29, -0.040_029_710_568_538, 0.141_742_621_254_608_04, 0.402_615_702_395_213_8, 0.490_141_990_998_482_9, 0.581_908_910_406_949_1, 0.623_388_851_082_543_2, 0.707_360_268_184_477_1, 0.820_179_412_606_034_7, 0.975_516_906_883_780_8],
    [-1.026_906_800_539_018, -0.895_228_668_618_348_2, -0.848_581_066_010_860_8, -0.471_588_316_335_973_5, -0.417_763_315_071_250_86, -0.298_068_489_274_892_55, -0.209_151_987_508_518, -0.011_320_295_671_747_13, 0.042_641_662_315_449_92, 0.069_399_013_241_804_67, 0.129_468_013_407_511_52, 0.164_659_750_620_716_72, 0.224_969_165_130_638_25, 0.384_411_836_490_972_4, 0.472_735_719_272_071_8, 0.647_856_975_560_400_4, 0.975_516_906_883_780_8],
    [-1.285_704_800_787_205, -0.464_831_108_285_449_05, -0.176_247_932_968_776_83, -0.115_752_450_262_393_96, -0.089_599_483_683_302_35, -0.068_143_637_879_168_19, -0.045_632_430_415_177_63, -0.022_301_656_510_556_61, 0.000_403_729_226_754_272_45, 0.021_146_610_757_352_823, 0.051_735_768_078_134_96, 0.135_460_558_470_869_82, 0.230_829_080_129_971_97, 0.361_672_732_292_111_8, 0.492_248_427_664_800_8, 0.654_642_720_846_948_3, 1.265_852_495_398_798_4],
    [-1.285_704_800_787_205, -0.464_831_108_285_449_05, -0.176_247_932_968_776_83, -0.115_752_450_262_393_96, -0.089_599_483_683_302_35, -0.068_143_637_879_168_19, -0.045_632_430_415_177_63, -0.022_301_656_510_556_61, 0.000_403_729_226_754_272_45, 0.021_146_610_757_352_823, 0.051_735_768_078_134_96, 0.135_460_558_470_869_82, 0.230_829_080_129_971_97, 0.361_672_732_292_111_8, 0.492_248_427_664_800_8, 0.654_642_720_846_948_3, 1.265_852_495_398_798_4],
    [-1.285_704_800_787_205, -0.464_831_108_285_449_05, -0.176_247_932_968_776_83, -0.115_752_450_262_393_96, -0.089_599_483_683_302_35, -0.068_143_637_879_168_19, -0.045_632_430_415_177_63, -0.022_301_656_510_556_61, 0.000_403_729_226_754_272_45, 0.021_146_610_757_352_823, 0.051_735_768_078_134_96, 0.135_460_558_470_869_82, 0.230_829_080_129_971_97, 0.361_672_732_292_111_8, 0.492_248_427_664_800_8, 0.654_642_720_846_948_3, 1.265_852_495_398_798_4],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.403_567_005_115_827_1, -0.270_604_865_265_198_4, -0.188_231_400_179_451_59, -0.153_275_962_073_992_2, -0.122_521_449_394_350_28, -0.100_982_986_050_162_26, -0.054_204_294_284_878_614, -0.037_725_801_134_394_395, -0.017_147_321_156_148_03, 0.005_343_457_804_629_8, 0.046_066_898_442_827_235, 0.055_165_119_522_972_39, 0.077_941_506_010_639_15, 0.115_923_192_426_466_31, 0.186_425_113_544_020_17, 0.424_334_521_336_588_8, 0.684_116_542_847_229_3],
    [-0.410_093_002_727_429_9, -0.287_104_879_238_832_93, -0.264_840_621_327_942_5, -0.184_573_039_655_299_03, -0.136_313_762_809_595_4, -0.108_920_472_898_779_17, -0.049_206_308_578_580_73, -0.016_272_930_803_370_098, 0.037_168_809_452_327_05, 0.050_552_843_692_491_39, 0.060_195_960_533_354_55, 0.086_029_422_604_199_24, 0.113_118_389_389_639_75, 0.147_538_355_629_848_32, 0.216_852_572_107_053_94, 0.362_878_733_634_791_4, 0.684_116_542_847_229_3],
    [-0.410_093_002_727_429_9, -0.292_118_781_562_709_76, -0.255_960_405_216_141_4, -0.176_636_819_712_310_02, -0.144_645_730_872_408_12, -0.121_610_302_620_018_28, -0.073_402_159_601_370_8, -0.018_075_678_188_798_544, 0.046_853_795_153_840_82, 0.060_456_321_907_896_39, 0.087_943_782_273_024_23, 0.104_302_515_646_463_48, 0.141_762_896_649_614_2, 0.196_547_228_451_509_04, 0.245_608_046_837_303_56, 0.312_515_632_909_525_55, 0.684_116_542_847_229_3],
    [-0.410_093_002_727_429_9, -0.276_070_043_456_383_9, -0.213_554_092_251_799_53, -0.170_601_474_574_048_62, -0.141_380_718_362_568_88, -0.122_310_532_648_250_73, -0.092_516_684_279_638_6, -0.073_356_905_741_067_83, -0.016_133_730_113_199_38, 0.057_683_653_333_620_515, 0.096_965_545_167_833_4, 0.113_873_146_508_975_62, 0.166_431_497_771_043_68, 0.237_357_260_870_031_73, 0.289_239_085_726_137_93, 0.346_970_483_762_080_7, 0.740_295_821_018_159_4],
    [-0.410_093_002_727_429_9, -0.246_682_259_056_278_27, -0.195_411_459_571_855_13, -0.169_483_666_926_704_08, -0.138_322_115_719_625_6, -0.122_366_107_439_517_35, -0.092_474_764_189_410_9, -0.072_463_866_326_875_75, -0.018_385_887_925_240_197, 0.036_350_416_478_309_645, 0.096_750_436_170_244_66, 0.160_261_691_078_033_6, 0.209_370_073_057_607_35, 0.276_387_918_299_912_44, 0.339_315_063_257_659_6, 0.452_175_520_924_665_56, 0.740_295_821_018_159_4],
    [-0.317_385_219_384_465_3, -0.245_757_671_172_954_58, -0.202_509_269_982_989_8, -0.172_264_142_271_543_88, -0.126_111_395_151_887_18, -0.100_958_061_600_539_69, -0.068_751_632_632_500_23, -0.040_327_658_010_436_93, 0.003_427_117_078_926_392, 0.026_572_712_754_506_85, 0.096_056_467_343_682_19, 0.165_282_633_932_805_87, 0.264_182_721_471_727_17, 0.360_580_146_997_502_5, 0.476_066_228_785_59, 0.575_940_219_173_683_3, 0.740_295_821_018_159_4],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.410_093_002_727_429_9, -0.277_375_146_048_485_4, -0.205_004_961_768_327_4, -0.159_040_734_187_429_48, -0.129_882_041_106_705_8, -0.102_199_248_876_819_54, -0.070_898_569_363_258_49, -0.034_893_094_541_540_735, 0.011_561_407_874_624_051, 0.050_126_140_464_364_09, 0.078_793_784_424_015_5, 0.110_216_662_455_542_26, 0.165_539_604_963_148_4, 0.263_847_107_354_470_2, 0.359_829_891_678_049, 0.565_698_280_076_665_2, 5.100_935_424_904_311],
    [-0.441_446_410_586_244_1, -0.127_235_414_539_374_45, -0.095_500_582_750_594_28, -0.084_296_391_153_138_5, -0.070_315_541_000_552_04, -0.043_315_731_582_466_14, -0.026_832_834_660_026_325, -0.009_716_796_220_254_56, 0.000_268_526_935_840_662_8, 0.007_870_542_805_189_23, 0.016_749_081_070_647_306, 0.028_529_745_750_775_423, 0.045_350_613_293_022_04, 0.077_989_698_100_875_48, 0.091_439_417_378_365_74, 0.119_249_164_066_949_91, 0.135_267_121_539_374_6],
    [-0.651_690_085_044_548_2, -0.294_161_744_985_620_9, -0.178_479_128_309_397_13, -0.110_683_860_524_682_88, -0.095_861_286_429_224_09, -0.084_450_836_257_457_46, -0.071_055_542_209_108_73, -0.040_366_520_124_169_62, -0.022_642_038_115_614_07, -0.000_218_411_425_733_507_86, 0.014_042_885_129_483_373, 0.026_690_768_177_122_074, 0.059_134_999_517_487_82, 0.085_838_663_772_611_6, 0.102_557_472_905_404_93, 0.126_580_400_824_801, 0.135_267_121_539_374_6],
    [-0.651_690_085_044_548_2, -0.450_934_221_141_373_27, -0.328_497_088_212_968_25, -0.226_957_390_551_752_18, -0.139_294_283_476_967_4, -0.104_185_224_240_608_97, -0.085_848_080_908_864_41, -0.070_405_615_380_252_64, -0.044_605_231_139_966_44, -0.031_171_933_473_007_896, -0.013_019_740_138_258_639, 0.005_638_327_257_588_345, 0.014_351_783_556_197_751, 0.049_711_974_575_544_01, 0.074_616_872_675_556_83, 0.119_351_810_296_726_99, 0.135_267_121_539_374_6],
    [-0.651_690_085_044_548_2, -0.315_398_491_451_928_7, -0.204_375_731_403_119_9, -0.149_705_127_467_377_69, -0.127_586_298_517_497_67, -0.106_564_769_422_746_77, -0.087_379_425_851_695_36, -0.074_564_340_823_665_84, -0.069_680_376_573_610_01, -0.059_946_828_744_545_5, -0.048_064_337_214_698_605, -0.036_984_517_889_010_6, -0.025_984_508_770_765_67, -0.010_868_968_286_354_593, 0.010_440_565_837_799_8, 0.061_878_773_695_236_984, 0.256_227_951_245_106_33],
    [-0.651_690_085_044_548_2, -0.203_338_361_548_752_76, -0.150_259_008_081_288_53, -0.127_711_449_135_071_76, -0.105_815_370_748_504_01, -0.086_080_695_315_464_15, -0.071_521_908_227_818_72, -0.062_848_900_977_655_26, -0.050_684_441_514_145_05, -0.042_595_857_828_045_56, -0.023_904_436_240_784_324, -0.005_853_636_692_416_144, 0.005_666_482_726_302_882, 0.033_940_016_733_359_786, 0.095_307_610_253_477_75, 0.188_925_876_944_617_04, 0.256_227_951_245_106_33],
    [-0.320_446_960_317_168_57, -0.183_065_794_844_178_8, -0.130_938_807_646_207_97, -0.103_160_773_679_676, -0.076_899_692_192_329_07, -0.063_035_710_705_151_57, -0.049_966_685_552_981_255, -0.036_085_951_533_988_886, -0.016_939_447_952_735_94, -0.002_849_617_641_636_964_5, 0.006_205_939_881_815_709, 0.024_275_105_189_199_41, 0.046_819_879_022_508_906, 0.069_930_122_166_213_67, 0.108_601_951_366_971_36, 0.183_830_201_208_033_23, 0.300_269_840_816_839_94],
    [-0.464_338_179_736_384_86, -0.226_108_784_199_769_54, -0.144_475_961_744_695_01, -0.117_060_400_413_246_36, -0.087_601_721_984_777_92, -0.064_598_600_437_139_84, -0.045_837_752_036_123_44, -0.029_007_628_171_977_527, -0.010_326_979_521_850_117, 0.000_506_424_258_532_343_4, 0.016_262_515_954_898_957, 0.027_107_567_128_568_87, 0.042_857_057_746_735_33, 0.065_992_287_762_517_06, 0.100_949_314_116_731_37, 0.162_336_391_286_332_54, 0.300_269_840_816_839_94],
    [-0.464_338_179_736_384_86, -0.268_443_618_185_860_9, -0.214_847_132_840_072_76, -0.146_196_642_552_881_8, -0.116_721_746_037_319_38, -0.095_314_729_476_987_38, -0.068_462_677_979_967_12, -0.042_274_472_436_463_595, -0.028_127_559_697_756_3, -0.012_446_864_211_896_1, 0.003_097_199_689_660_209_7, 0.016_308_477_262_981_04, 0.026_078_620_042_957_707, 0.039_035_397_033_367_46, 0.060_590_921_437_794_154, 0.113_634_325_968_974_42, 0.300_269_840_816_839_94],
    [-0.464_338_179_736_384_86, -0.216_228_819_369_343_74, -0.139_158_229_664_486_02, -0.100_464_590_274_839_9, -0.077_752_034_215_328_23, -0.052_933_908_900_262_276, -0.039_257_844_984_294_23, -0.026_671_574_390_794_674, -0.013_168_059_055_026_332, -0.001_891_222_748_140_224_8, 0.010_753_538_052_300_58, 0.018_241_293_697_588_368, 0.029_548_503_687_913_535, 0.040_856_213_863_477_79, 0.058_241_326_460_248_3, 0.081_855_985_002_551_69, 0.300_269_840_816_839_94],
    [-0.464_338_179_736_384_86, -0.135_368_633_355_390_46, -0.089_910_824_880_045_58, -0.068_515_006_157_228_16, -0.050_226_053_269_847_02, -0.037_066_021_666_655_4, -0.024_491_888_074_153_482, -0.012_943_205_412_878_597, -0.002_072_308_636_395_479, 0.007_708_917_818_459_634, 0.017_900_081_394_397_978, 0.029_913_713_156_163_025, 0.040_074_453_881_463_2, 0.053_310_361_583_966_55, 0.072_008_344_674_986_97, 0.087_923_288_062_999_9, 0.177_741_129_671_305_2],
    [-0.405_477_845_888_118_64, -0.112_986_207_628_977_08, -0.077_180_008_073_666_1, -0.057_244_219_908_332_58, -0.041_975_539_484_099_27, -0.029_950_485_386_524_013, -0.017_399_985_692_429_624, -0.006_673_242_924_152_938, 0.004_696_164_834_118_55, 0.015_464_226_755_997_578, 0.027_992_250_879_851_11, 0.038_929_875_131_990_45, 0.050_978_588_220_810_9, 0.065_312_365_980_345_91, 0.083_421_603_030_231_44, 0.102_661_420_257_828_22, 0.189_850_323_143_153_1],
    [-0.388_511_015_195_275_6, -0.153_740_946_728_725_68, -0.090_591_316_428_955_4, -0.069_134_460_476_819_88, -0.047_124_329_775_500_5, -0.030_138_026_483_991_994, -0.016_136_476_087_609_81, -0.002_427_375_588_632_686, 0.011_577_075_055_469_252, 0.023_895_708_315_577_233, 0.035_224_056_204_512_88, 0.048_260_690_107_509_226, 0.061_028_655_355_597_42, 0.079_831_615_867_591_81, 0.096_755_380_004_614_74, 0.129_666_396_062_622_62, 0.354_406_403_643_231_8],
    [-0.388_511_015_195_275_6, -0.253_069_661_463_461_7, -0.186_692_267_396_071_28, -0.134_074_433_130_268_62, -0.094_188_817_953_361_97, -0.066_772_494_104_026_51, -0.034_161_004_006_452_55, -0.008_002_922_994_764_189, 0.011_741_517_170_445_671, 0.029_564_312_274_552_28, 0.038_612_611_440_551_77, 0.057_066_021_677_553_17, 0.079_058_487_708_241_4, 0.102_702_961_631_862_64, 0.136_138_951_001_950_9, 0.174_762_405_671_237_16, 0.499_859_920_032_886_15],
    [-0.388_511_015_195_275_6, -0.292_540_778_718_272_55, -0.237_324_709_576_221_82, -0.188_454_257_051_760_2, -0.152_240_706_232_586_04, -0.121_270_656_718_775_4, -0.086_719_251_193_574_88, -0.050_325_740_129_164_444, -0.001_835_055_301_595_756_1, 0.032_508_565_042_546_72, 0.057_158_115_253_654_454, 0.090_030_541_397_176_88, 0.122_313_201_030_821_52, 0.153_375_835_649_664_6, 0.191_968_382_519_035_78, 0.276_179_485_915_806, 0.551_772_041_941_405_7],
    [-0.388_511_015_195_275_6, -0.246_267_634_351_596_02, -0.188_677_236_163_262_54, -0.151_970_031_192_297_7, -0.130_585_459_758_317_48, -0.102_029_709_132_578_39, -0.078_195_851_924_124_65, -0.052_446_748_484_459_13, -0.012_894_062_432_211_75, 0.029_291_596_216_711_856, 0.078_143_800_091_479_73, 0.101_000_076_722_551_28, 0.129_276_764_696_824_92, 0.181_547_074_426_385_4, 0.257_355_659_850_744_3, 0.374_685_940_869_725_3, 0.743_998_532_898_900_3],
    [-0.349_452_141_574_571_2, -0.158_638_198_412_034_86, -0.136_119_832_064_486_8, -0.119_340_846_078_667_48, -0.090_302_938_388_588_27, -0.080_718_121_939_875_75, -0.054_763_475_960_029_596, -0.042_997_877_548_105_65, -0.029_287_410_666_764_165, -0.002_452_853_134_754_575_6, 0.028_352_408_704_711_722, 0.061_929_904_887_916_803, 0.098_113_763_650_686_76, 0.117_363_807_757_140_57, 0.198_339_221_644_977_86, 0.406_066_901_192_981_56, 0.743_998_532_898_900_3],
    [-0.205_195_348_394_157_15, -0.141_862_328_778_677_72, -0.120_252_784_490_151_35, -0.093_365_643_587_085_14, -0.086_199_900_995_317_77, -0.068_659_299_719_607_95, -0.052_819_874_753_840_826, -0.043_455_606_574_596_29, -0.037_581_252_433_131_3, -0.015_866_101_513_366_97, -0.001_030_513_992_965_906_2, 0.026_335_174_928_288_057, 0.054_769_516_815_107_14, 0.098_415_866_231_574_53, 0.124_126_110_921_039_1, 0.238_858_143_112_634_4, 0.743_998_532_898_900_3],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.256_336_382_770_902_04, -0.167_781_288_670_080_77, -0.114_765_376_504_165_57, -0.092_676_602_541_343_19, -0.066_212_657_073_083_02, -0.021_096_191_970_809_773, -0.006_979_237_962_986_92, 0.006_012_740_067_079_693, 0.026_359_296_670_060_74, 0.049_202_452_617_667_626, 0.061_578_170_150_091_99, 0.082_422_024_223_951_12, 0.103_326_880_603_922_21, 0.122_763_669_139_304_52, 0.155_600_572_870_999_02, 0.170_580_288_045_734_55, 0.283_625_638_853_283_35],
    [-0.256_336_382_770_902_04, -0.155_618_681_018_137_47, -0.118_033_662_285_728_6, -0.096_734_012_694_574_74, -0.077_076_111_082_866_98, -0.058_636_276_421_102_3, -0.027_224_267_747_995_853, -0.012_418_591_798_870_58, 0.002_292_462_783_530_651, 0.017_146_611_723_835_536, 0.047_001_769_744_928_486, 0.058_705_250_085_601_554, 0.084_114_942_082_817_82, 0.104_824_509_175_56, 0.146_815_908_488_184_8, 0.163_892_436_908_742_2, 0.283_625_638_853_283_35],
    [-0.256_336_382_770_902_04, -0.130_267_341_256_457_8, -0.103_081_804_468_144_09, -0.089_849_710_901_211_17, -0.077_139_921_415_752_43, -0.066_534_706_192_255_84, -0.043_698_385_935_220_8, -0.033_437_404_367_057_72, -0.014_501_380_009_748_566, -0.001_686_784_219_577_744_6, 0.009_146_365_203_058_882, 0.024_956_808_591_138_142, 0.051_971_623_832_331_92, 0.071_058_023_626_928_69, 0.101_515_699_442_405_34, 0.148_755_467_970_942_88, 0.283_625_638_853_283_35],
    [-0.238_600_874_689_039_12, -0.101_031_944_931_422_37, -0.089_746_952_199_028_69, -0.076_935_955_459_640_9, -0.066_684_417_690_627_73, -0.057_001_730_379_711_52, -0.042_988_336_970_011_53, -0.037_733_644_217_275_02, -0.022_940_674_941_772_772, -0.010_979_125_755_353_054, 0.000_316_923_564_589_623_36, 0.011_234_424_688_693_652, 0.023_338_376_070_479_776, 0.046_182_643_907_585_26, 0.083_231_243_114_026_26, 0.108_350_025_944_708_92, 0.223_203_579_199_775_35],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.256_336_382_770_902_04, -0.132_870_231_845_976_85, -0.103_079_139_009_556_55, -0.087_991_989_046_355_8, -0.068_299_776_131_610_31, -0.043_549_984_852_338_1, -0.026_947_852_355_333_65, -0.012_892_043_546_356_627, 0.001_259_809_560_538_542, 0.016_298_403_058_867_88, 0.043_774_882_664_307_13, 0.056_103_399_306_392_03, 0.087_314_177_397_977_1, 0.107_591_142_764_301_37, 0.148_536_020_626_943_68, 0.172_923_044_904_232_44, 0.938_901_394_112_795],
    [-0.285_608_117_672_971_7, -0.124_116_024_703_418_44, -0.088_080_780_851_145_35, -0.067_704_567_293_263_16, -0.052_462_654_840_656_01, -0.041_789_922_812_171_94, -0.031_138_869_883_540_456, -0.019_473_063_842_764_878, -0.005_892_899_125_237_608, 0.011_506_047_694_288_925, 0.028_114_066_507_499_18, 0.047_122_455_621_675_004, 0.063_877_521_360_608_95, 0.079_988_973_914_733_3, 0.103_854_992_644_342_93, 0.171_682_850_136_248, 0.326_427_937_454_586_76],
    [-0.258_652_269_523_687_6, -0.107_612_502_748_275_51, -0.050_138_486_598_663_28, -0.044_087_382_720_011_496, -0.031_171_472_375_357_438, -0.026_453_664_656_944_067, -0.019_911_794_365_140_765, -0.009_075_352_021_667_92, 0.001_146_947_427_351_998, 0.010_106_713_325_690_155, 0.016_206_687_895_484_212, 0.021_030_267_720_376_333, 0.032_147_786_809_378_83, 0.039_943_320_499_193_69, 0.046_886_729_470_217_8, 0.048_847_546_580_353_896, 0.050_537_520_819_808_11],
    [-0.258_652_269_523_687_6, -0.159_292_248_857_564_57, -0.098_446_196_385_394_22, -0.060_611_391_456_426_57, -0.034_000_258_590_472_27, -0.021_958_585_904_581_26, -0.011_660_839_064_483_427, 0.001_881_147_506_692_668_2, 0.013_787_931_071_817_578, 0.016_113_289_018_029_83, 0.021_931_905_767_938_75, 0.030_299_359_872_770_18, 0.035_284_797_953_121_31, 0.039_266_297_792_095_41, 0.044_027_604_333_037_69, 0.049_579_698_178_596_08, 0.072_635_600_126_589_98],
    [-0.258_652_269_523_687_6, -0.182_223_525_221_798_1, -0.130_464_346_309_341_76, -0.099_088_434_865_873_85, -0.067_641_422_611_626_62, -0.032_948_343_708_563_98, -0.012_813_779_027_857_648, -0.002_138_449_006_429_650_4, 0.013_253_240_859_324_071, 0.016_761_325_794_923_537, 0.026_653_473_384_849_764, 0.033_076_732_048_560_73, 0.035_837_862_052_149_436, 0.040_397_964_278_378_565, 0.050_502_186_623_293_845, 0.069_259_444_694_984_57, 0.137_419_265_030_921_72],
    [-0.258_652_269_523_687_6, -0.178_695_941_744_621_4, -0.140_540_513_123_835_48, -0.111_406_641_062_111_28, -0.093_559_740_447_193_15, -0.067_809_182_844_526_25, -0.034_625_288_520_452_7, -0.009_102_685_007_120_413, 0.013_671_934_865_521_488, 0.022_021_503_219_907_01, 0.033_119_777_601_448_19, 0.036_453_180_222_333_306, 0.048_612_532_485_960_89, 0.065_747_610_480_792_43, 0.078_709_162_524_126_53, 0.097_633_294_807_361_02, 0.297_121_011_020_036_8],
    [-0.245_710_100_005_780_2, -0.158_592_272_019_605_07, -0.135_364_642_658_921_87, -0.108_706_210_751_389_78, -0.098_023_236_373_260_23, -0.078_701_529_006_417_84, -0.057_659_050_801_019_7, -0.029_825_701_101_678_8, 0.001_072_469_541_628_732_3, 0.021_468_454_048_005_372, 0.033_933_148_630_096_74, 0.048_934_144_423_029_53, 0.072_039_787_744_306_01, 0.093_301_933_714_552_2, 0.104_451_116_746_884_08, 0.132_414_394_894_916_16, 0.317_495_963_110_877_24],
    [-0.245_710_100_005_780_2, -0.147_446_440_296_964_1, -0.130_378_423_575_827_9, -0.105_352_871_269_152_83, -0.091_871_197_872_507_91, -0.074_898_605_728_107_35, -0.057_806_679_888_391_59, -0.032_377_440_132_255_145, -0.012_714_494_398_750_829, 0.015_360_733_471_056_503, 0.027_816_268_260_574_217, 0.048_520_877_932_127_525, 0.074_260_149_342_612_71, 0.098_286_369_936_147_8, 0.124_256_821_338_846_66, 0.181_198_843_914_128_13, 0.326_427_937_454_586_76],
    [-0.168_850_076_053_809_8, -0.142_797_427_326_478_27, -0.116_235_084_059_304_6, -0.099_098_973_790_752_94, -0.083_230_097_809_699_38, -0.070_805_836_909_491_06, -0.052_390_554_259_108_34, -0.032_227_956_773_472_22, -0.015_116_885_004_087_746, 0.012_108_279_776_955_498, 0.026_701_774_759_117_194, 0.051_625_216_322_075_67, 0.070_901_694_162_557_95, 0.103_068_884_456_628_26, 0.135_685_824_994_179_14, 0.218_884_862_423_187_25, 0.326_427_937_454_586_76],
    [-0.176_312_108_699_055_4, -0.123_131_608_647_593_79, -0.101_911_617_959_294_61, -0.082_948_916_536_064_94, -0.069_009_259_038_621_18, -0.054_136_266_020_952_895, -0.043_204_815_573_769_796, -0.032_501_450_194_808_254, -0.019_908_969_179_918_037, -0.002_393_267_536_750_403_4, 0.023_871_302_488_458_347, 0.052_466_966_148_584_95, 0.072_526_272_636_424_44, 0.106_709_347_719_153_84, 0.185_558_375_717_906_23, 0.235_926_392_226_352, 0.326_427_937_454_586_76],
    [-0.176_312_108_699_055_4, -0.105_203_374_625_573_54, -0.079_133_120_061_841_66, -0.063_469_671_605_920_42, -0.051_498_389_566_179_41, -0.044_816_723_293_112_8, -0.038_664_430_718_917_57, -0.031_778_752_271_740_22, -0.020_722_719_759_050_24, -0.006_221_043_796_623_452, 0.020_451_727_423_935_622, 0.050_448_467_158_258_38, 0.073_966_303_525_227_44, 0.112_602_936_078_093_95, 0.200_642_628_582_467_73, 0.235_941_324_278_647_4, 0.326_427_937_454_586_76],
    [-0.185_789_796_824_047_5, -0.094_404_493_474_057_88, -0.069_890_267_139_090_13, -0.058_949_571_380_062_97, -0.049_942_315_108_142_385, -0.044_678_235_405_989_59, -0.037_785_159_409_068_38, -0.029_982_693_123_242_69, -0.019_834_460_422_100_38, -0.002_265_327_461_801_672_6, 0.020_269_849_376_268_696, 0.040_982_369_180_799_325, 0.068_171_580_036_749_09, 0.101_753_735_697_566_11, 0.177_360_696_331_344_9, 0.222_040_012_203_359_9, 0.323_938_744_945_265_5],
    [-0.200_094_235_608_484_38, -0.103_736_215_636_222_19, -0.076_613_930_307_439_57, -0.062_980_575_572_829_3, -0.054_174_461_151_621_13, -0.047_046_957_477_185_763, -0.034_831_780_223_505_686, -0.026_019_939_484_007_444, -0.014_718_547_649_892_695, 0.002_229_907_675_797_087_7, 0.020_720_287_784_982_458, 0.038_579_285_720_140_494, 0.059_794_955_901_601_894, 0.093_985_728_459_079_58, 0.143_853_736_031_436_42, 0.187_857_587_813_367_64, 0.289_903_605_045_912_84],
    [-0.244_517_811_978_479_22, -0.125_237_140_562_268_06, -0.084_564_651_659_107_4, -0.064_572_257_381_845_4, -0.053_914_780_053_177_76, -0.039_254_789_304_457_305, -0.026_231_420_047_109_41, -0.015_308_993_564_745_958, -0.001_302_420_397_110_106, 0.013_812_621_902_281_384, 0.031_544_455_647_556_86, 0.052_984_810_972_227_306, 0.073_254_878_180_218_49, 0.093_378_703_625_615_44, 0.116_169_746_043_552_53, 0.154_110_654_132_005_14, 0.262_183_799_074_355_4],
    [-0.244_517_811_978_479_22, -0.135_790_244_522_228_76, -0.084_906_034_802_403_55, -0.059_231_090_461_094_02, -0.042_337_538_936_434_06, -0.030_965_568_364_628_57, -0.017_146_615_728_760_216, -0.001_838_174_723_371_795_8, 0.016_318_813_812_966_04, 0.039_577_505_954_999_474, 0.056_842_937_452_227_814, 0.068_245_219_366_442_94, 0.076_224_457_574_067_57, 0.084_736_419_268_794_36, 0.096_828_476_698_608_5, 0.115_797_465_610_203_68, 0.217_644_651_642_552_53],
    [-0.244_517_811_978_479_22, -0.103_263_600_089_013_36, -0.070_028_811_557_782_9, -0.050_823_522_675_165_926, -0.038_899_768_375_264_446, -0.030_347_319_010_603_42, -0.019_156_922_458_778_605, -0.004_163_754_061_134, 0.015_575_575_094_159_045, 0.039_216_300_881_507_6, 0.053_659_068_459_526_93, 0.061_133_172_043_540_75, 0.069_583_132_337_078_54, 0.075_639_387_256_222_89, 0.081_717_434_830_818_68, 0.094_155_824_997_765_36, 0.170_486_593_863_614_3],
    [-0.244_517_811_978_479_22, -0.075_664_623_586_030_99, -0.055_522_082_926_071_435, -0.041_605_725_932_202_346, -0.033_152_259_530_235_095, -0.023_916_468_880_577_63, -0.018_056_497_660_877_9, -0.010_736_461_693_267_106, -0.000_895_304_026_825_615_7, 0.010_381_034_521_794_135, 0.026_874_111_633_209_612, 0.043_894_585_538_202_885, 0.056_892_424_580_084_16, 0.068_499_518_110_135_73, 0.075_440_826_307_535_22, 0.083_087_910_720_010_33, 0.118_952_621_435_764_02],
    [-0.140_635_598_314_195_48, -0.052_213_483_925_699_86, -0.039_245_957_787_338_2, -0.029_985_610_736_705_064, -0.023_297_026_422_694_73, -0.017_779_096_229_322_548, -0.013_441_170_556_114_542, -0.004_809_555_855_588_374_5, -8.520_384_801_144_433e-6, 0.005_354_263_115_660_407, 0.011_497_171_455_406_926, 0.021_482_860_838_176_385, 0.027_947_189_370_657_537, 0.042_019_621_329_069_21, 0.067_025_053_547_788_66, 0.079_524_610_335_209_86, 0.099_393_633_018_965_64],
    [-0.486_507_549_437_199_15, -0.401_795_137_601_840_9, -0.388_511_769_233_190_75, -0.378_072_020_274_918_8, -0.357_895_968_590_013_77, -0.305_921_403_144_009_9, -0.268_078_477_117_163_8, -0.079_770_610_692_836_35, 0.009_074_783_787_398_889, 0.044_939_195_603_400_03, 0.058_250_599_074_497_19, 0.065_409_946_753_989_65, 0.071_875_650_579_175_61, 0.085_346_237_827_135_23, 0.092_917_170_488_827_47, 0.096_795_836_134_608_71, 0.124_955_247_173_465_93],
    [-0.486_507_549_437_199_15, -0.403_868_723_359_398_33, -0.388_531_673_081_903_95, -0.380_027_588_904_784_4, -0.364_913_183_665_674_3, -0.308_739_036_057_745_4, -0.267_544_182_727_051_55, -0.168_373_183_868_783_72, -0.005_224_299_469_922_072, 0.014_770_157_854_098_485, 0.056_443_141_349_205_7, 0.071_457_120_117_333_86, 0.082_907_868_056_709_54, 0.094_590_448_915_464_98, 0.099_463_887_554_018_65, 0.120_698_471_457_551_44, 0.256_340_810_794_819_46],
    [-0.486_507_549_437_199_15, -0.387_523_463_571_056_77, -0.365_236_920_687_378_7, -0.326_791_935_821_858_56, -0.261_745_415_196_081_6, -0.198_753_196_079_829_36, -0.111_849_315_460_288_5, -0.046_127_077_986_852_75, -0.011_266_349_813_376_363, 0.052_031_349_538_983_68, 0.080_757_023_540_693_92, 0.096_795_856_085_820_7, 0.106_311_328_830_429_22, 0.125_790_127_877_766_1, 0.150_439_368_456_698_67, 0.264_623_354_753_825_26, 0.364_389_131_429_970_5],
    [-0.486_507_549_437_199_15, -0.211_510_168_494_055_38, -0.143_225_319_185_594_25, -0.121_290_535_954_333_69, -0.110_704_618_451_569_4, -0.101_747_076_824_427_05, -0.080_406_286_057_455_94, -0.028_047_378_820_728_23, -0.012_561_946_780_145_38, 0.080_211_883_492_436_94, 0.118_344_945_393_175_1, 0.192_856_780_696_578_58, 0.274_357_408_789_915_1, 0.308_618_438_529_047_3, 0.342_676_961_677_966_63, 0.350_925_125_004_750_6, 0.364_389_131_429_970_5],
    [-0.333_775_472_243_198_5, -0.143_855_785_385_283_1, -0.123_363_899_613_133_1, -0.109_018_648_622_662_65, -0.100_730_107_385_770_93, -0.079_070_424_971_495_9, -0.039_281_821_392_153_16, -0.027_981_629_322_769_874, -0.009_208_494_953_804_516, 0.035_437_706_416_032_88, 0.276_190_867_107_122_4, 0.296_095_018_929_338_8, 0.307_344_234_223_676_7, 0.325_839_897_662_498_16, 0.341_287_213_413_503_54, 0.350_515_415_201_762_66, 0.365_620_536_912_681_8],
    [-0.219_759_792_192_808_4, -0.159_267_869_969_379_6, -0.143_247_815_284_738_18, -0.121_022_376_020_633_18, -0.102_112_213_956_620_93, -0.082_808_951_900_450_5, -0.037_987_269_611_813_874, -0.027_188_455_488_194_225, -0.006_633_981_445_323_533, 0.039_160_905_430_083_66, 0.253_936_858_469_104_3, 0.283_878_024_247_298_47, 0.298_254_283_616_408_77, 0.310_376_980_592_359_13, 0.325_874_052_261_697_74, 0.348_400_520_287_818_1, 0.375_615_389_633_816_24],
    [-0.619_658_636_154_852_6, -0.203_709_211_026_004_28, -0.176_791_668_102_955, -0.154_376_714_605_672_04, -0.133_369_100_663_694_56, -0.112_625_687_803_681_25, -0.088_685_675_243_274_66, -0.051_167_241_636_906_17, -0.011_206_587_155_202_426, 0.111_309_362_291_145_7, 0.186_848_025_315_861_2, 0.213_035_628_812_209_33, 0.241_453_839_202_661_36, 0.270_496_855_761_634_46, 0.298_010_448_058_079_36, 0.324_275_333_888_848_1, 0.375_615_389_633_816_24],
    [-0.630_822_490_633_083_9, -0.270_186_867_497_977_5, -0.221_112_500_031_059_37, -0.202_521_045_305_248_75, -0.177_680_888_660_266_43, -0.151_064_302_570_895_04, -0.124_096_240_518_602_59, -0.085_990_687_420_340_37, 0.002_497_500_769_033_289, 0.091_615_503_122_493_71, 0.122_357_657_017_620_43, 0.151_521_305_205_476_21, 0.182_167_853_453_508_55, 0.210_399_459_265_756_07, 0.235_513_701_505_762_04, 0.270_503_188_110_343_63, 0.375_615_389_633_816_24],
    [-0.630_822_490_633_083_9, -0.323_795_088_787_979_8, -0.275_927_919_252_738, -0.227_623_618_665_264_57, -0.202_583_497_087_785_75, -0.172_644_373_751_603_13, -0.138_017_188_349_342_3, -0.092_972_722_352_708_58, 0.003_789_888_314_714_743, 0.066_155_168_078_977_95, 0.094_192_970_397_768_51, 0.113_380_839_212_152_68, 0.133_324_640_717_059_52, 0.149_876_550_796_65, 0.178_738_626_060_207_77, 0.212_939_585_713_749_32, 0.375_615_389_633_816_24],
    [-0.630_822_490_633_083_9, -0.369_930_863_641_463_44, -0.282_935_602_229_728_24, -0.212_535_434_202_736_4, -0.171_399_178_023_457_42, -0.144_432_454_288_625_86, -0.107_783_319_456_605_92, -0.065_644_706_253_682_25, -0.003_272_877_956_389_152, 0.035_851_125_729_498_7, 0.066_763_625_199_685_23, 0.085_690_463_042_451_51, 0.107_609_014_551_109_8, 0.129_170_489_684_536_04, 0.144_570_201_569_109_8, 0.171_698_613_844_613_57, 0.365_018_583_136_106_36],
    [-0.630_822_490_633_083_9, -0.305_327_693_562_364_33, -0.188_083_987_799_711_9, -0.157_726_491_395_096_67, -0.133_750_293_682_442_04, -0.105_182_677_598_332_94, -0.075_579_309_642_254_2, -0.045_056_896_711_243_286, -0.008_686_520_153_124_398, 0.021_122_844_467_021_087, 0.049_975_224_268_779_89, 0.070_001_243_515_028_46, 0.082_415_636_171_642_77, 0.105_103_605_951_302_35, 0.126_849_191_256_979_94, 0.147_711_969_292_937_45, 0.251_837_032_943_673_4],
    [-0.584_991_342_382_244_4, -0.230_501_282_213_721_9, -0.171_485_555_375_760_32, -0.140_978_218_047_822_18, -0.111_536_839_343_976_44, -0.079_596_380_944_690_47, -0.051_894_091_293_899_27, -0.021_369_672_553_364_184, 0.006_567_205_483_753_82, 0.023_627_966_031_422_81, 0.047_322_965_132_454_06, 0.066_158_935_308_159_32, 0.080_010_846_792_176_5, 0.097_706_535_956_636_56, 0.112_509_129_964_988_44, 0.137_796_037_127_074_44, 0.251_837_032_943_673_4],
    [-0.558_219_268_780_298_3, -0.240_020_960_588_885_3, -0.182_892_931_838_658_9, -0.146_824_748_407_309_47, -0.110_427_040_612_780_3, -0.073_326_128_180_107_45, -0.045_749_744_915_104_154, -0.020_376_196_935_696_93, 5.034_388_386_232_402e-5, 0.018_695_227_660_576_7, 0.039_720_805_640_699_25, 0.058_993_232_331_363_6, 0.076_387_600_950_031_4, 0.092_228_608_701_863_15, 0.111_938_000_824_699_44, 0.136_454_146_037_209_62, 0.257_305_475_275_588_56],
    [-0.484_649_605_212_987_06, -0.231_196_939_532_519_19, -0.188_446_350_409_513_27, -0.152_227_107_058_721, -0.107_879_439_168_518_1, -0.070_584_326_003_074_97, -0.045_697_509_654_137_29, -0.022_938_308_762_272_194, -0.004_504_493_743_788_663_5, 0.014_331_821_126_999_668, 0.032_830_848_171_913_195, 0.051_001_403_913_490_416, 0.069_403_167_232_245_5, 0.084_332_014_600_363_26, 0.104_999_101_610_763_28, 0.132_231_760_597_339_3, 0.257_305_475_275_588_56],
    [-0.388_340_017_276_630_9, -0.219_348_334_383_853_48, -0.184_520_912_369_358_16, -0.147_109_017_162_642_02, -0.097_683_200_248_645_02, -0.062_647_497_200_666_88, -0.037_309_175_528_404, -0.016_949_411_093_641_58, 0.001_331_790_380_528_559_4, 0.018_206_745_988_652_63, 0.035_185_556_881_276_83, 0.050_814_327_040_535_94, 0.067_430_335_642_124_8, 0.082_884_123_690_042_43, 0.099_975_015_390_899_16, 0.123_643_891_749_493_21, 0.257_305_475_275_588_56],
    [-0.321_107_618_286_784_7, -0.202_892_163_026_043_04, -0.167_695_502_411_543_02, -0.126_463_411_998_038_26, -0.080_972_605_732_732_59, -0.049_062_185_063_377_214, -0.025_815_328_847_799_08, -0.006_856_777_348_116_576, 0.010_791_008_082_891_806, 0.023_994_193_847_832_01, 0.037_160_668_077_104_15, 0.053_237_016_203_717_69, 0.070_062_393_516_384_63, 0.084_277_676_035_704_48, 0.101_148_224_869_796_34, 0.120_973_496_752_443_26, 0.257_305_475_275_588_56],
    [-0.237_347_847_372_897_2, -0.166_058_726_514_227_7, -0.128_246_524_254_787_66, -0.094_414_417_918_567_63, -0.062_649_828_039_412_59, -0.037_706_312_828_692_18, -0.019_258_366_834_012_622, -0.001_218_962_676_240_364_1, 0.014_326_634_961_119_094, 0.024_388_671_288_785_115, 0.037_196_752_174_485_19, 0.054_384_678_168_074_66, 0.072_906_303_840_341_36, 0.087_769_445_969_948_12, 0.112_614_654_395_208_99, 0.131_322_047_153_341_95, 0.230_870_476_723_371_24],
    [-0.337_815_501_870_810_66, -0.252_458_519_191_479_6, -0.224_662_049_083_194_48, -0.199_371_743_208_949_76, -0.165_782_935_321_364_15, -0.119_628_669_106_619_05, -0.044_142_675_136_914_44, -0.016_980_203_097_807_19, 0.014_833_276_727_330_754, 0.075_573_690_548_712_4, 0.126_590_993_106_205_45, 0.166_463_657_402_344_3, 0.216_200_821_089_880_29, 0.285_052_208_770_970_3, 0.384_847_551_856_063_84, 0.458_797_185_031_959_3, 0.610_277_830_760_271_2],
    [-0.355_541_657_653_402_5, -0.273_857_521_547_217_93, -0.231_129_754_788_304_72, -0.197_783_038_855_520_12, -0.172_882_899_452_781_04, -0.133_340_318_232_213_07, -0.079_777_207_106_006_43, -0.034_028_102_161_483_07, 0.000_349_488_248_735_123_4, 0.060_206_457_150_234_406, 0.127_059_144_446_191_74, 0.168_432_673_061_560_32, 0.216_233_549_782_453_32, 0.304_697_708_124_479_94, 0.386_897_135_565_622_6, 0.482_818_206_223_645_35, 0.610_277_830_760_271_2],
    [-0.433_431_344_019_137_3, -0.304_305_534_238_632_7, -0.244_667_345_583_229_5, -0.209_207_070_143_202_42, -0.183_432_771_905_332_94, -0.150_221_837_174_798_12, -0.116_732_621_226_231_07, -0.055_761_902_381_906_27, -0.002_974_523_009_439_488_4, 0.058_851_515_909_654_72, 0.126_848_895_530_209_67, 0.179_675_844_343_170_2, 0.224_157_162_316_010_68, 0.308_836_361_615_919_7, 0.404_163_067_030_765_3, 0.526_915_909_582_825_6, 0.634_586_996_442_617_8],
    [-0.576_419_513_329_877_7, -0.318_979_643_392_530_27, -0.253_982_478_569_868_4, -0.227_169_671_368_880_4, -0.195_443_174_673_375_94, -0.157_077_581_705_847_26, -0.127_982_873_362_802_34, -0.066_033_432_468_288_29, -0.005_311_085_466_567_118, 0.052_897_055_852_819_87, 0.112_646_637_520_034_4, 0.185_113_727_089_242_7, 0.257_188_674_170_485_2, 0.331_096_613_967_909_74, 0.415_072_659_813_513, 0.539_978_535_123_483_5, 0.645_901_887_524_846_8],
    [-0.730_229_854_623_088_3, -0.361_015_505_415_868_47, -0.289_873_156_233_409_7, -0.232_831_712_394_960_03, -0.199_242_886_561_398_66, -0.153_689_058_521_306_9, -0.103_845_872_302_969_49, -0.057_446_357_129_085_954, -0.009_597_281_250_301_511, 0.051_373_339_807_343_814, 0.103_037_207_039_067_49, 0.171_343_179_201_788_43, 0.252_393_082_686_475_15, 0.330_942_822_604_318_5, 0.424_162_657_087_551_2, 0.540_437_049_826_065_4, 0.661_156_097_011_913_2],
    [-0.730_229_854_623_088_3, -0.387_991_736_648_123_4, -0.309_140_384_572_473, -0.233_661_934_532_857_86, -0.189_592_737_076_497_65, -0.139_646_147_217_035_3, -0.087_577_094_584_322_31, -0.045_929_302_551_159_484, 0.000_855_781_659_730_846_5, 0.053_350_055_885_265_42, 0.105_102_880_030_164_23, 0.155_324_587_241_803_15, 0.230_536_426_451_560_72, 0.302_218_622_363_925_27, 0.408_130_308_958_568_6, 0.486_666_848_810_088_76, 0.661_156_097_011_913_2],
    [-0.811_684_177_502_055_3, -0.397_062_095_963_119_03, -0.310_406_157_818_274_2, -0.244_212_401_930_345_96, -0.181_949_093_606_801_84, -0.125_007_190_897_432_87, -0.088_082_223_448_206_93, -0.043_015_393_652_451_59, 0.003_220_041_931_649_448_4, 0.051_316_205_394_476_02, 0.099_954_126_016_687_57, 0.155_223_818_097_716_4, 0.211_957_857_408_091_9, 0.281_451_460_149_734_6, 0.368_485_895_317_827_6, 0.475_965_893_291_622_64, 0.687_134_777_466_905_4],
    [-0.811_684_177_502_055_3, -0.404_637_008_970_012, -0.305_387_386_691_559_46, -0.240_930_876_115_713_87, -0.169_125_321_089_774_93, -0.116_236_549_999_464_13, -0.090_688_176_946_151_7, -0.052_223_531_393_746_22, -0.002_446_169_877_282_985_6, 0.037_261_819_676_924_136, 0.087_050_527_636_475_79, 0.141_041_669_984_131_87, 0.198_513_850_510_088_85, 0.259_545_900_413_204_74, 0.339_114_621_965_058_15, 0.463_495_253_433_506_5, 0.687_134_777_466_905_4],
    [-0.811_684_177_502_055_3, -0.405_289_301_563_068_75, -0.304_620_854_151_755_6, -0.229_246_467_054_432_44, -0.158_131_339_201_018_43, -0.116_842_743_506_545_53, -0.091_269_067_985_715_06, -0.052_330_827_155_339_16, -0.002_292_786_370_038_077, 0.031_529_628_006_889_77, 0.075_348_782_339_693_57, 0.131_595_074_875_257_18, 0.182_731_559_892_616_94, 0.239_185_655_899_327_42, 0.318_568_066_988_369_76, 0.448_771_757_582_095_03, 0.687_134_777_466_905_4],
    [-0.921_679_509_484_924_6, -0.405_308_393_226_844_25, -0.311_818_913_452_597_8, -0.228_888_786_803_985_97, -0.158_189_835_681_543_63, -0.117_731_544_444_272_14, -0.081_555_408_572_059_01, -0.036_687_324_767_430_21, 0.004_403_872_103_118_269, 0.034_130_428_993_020_635, 0.068_012_783_886_632_83, 0.126_679_942_039_763_38, 0.188_875_323_212_406_2, 0.240_352_423_114_806_26, 0.314_659_106_971_271_86, 0.464_738_925_517_313_1, 0.695_692_761_871_830_7],
    [-1.420_014_003_199_247_2, -0.434_907_356_793_361, -0.343_058_066_184_766_53, -0.264_004_711_799_572, -0.217_246_651_284_787_46, -0.133_166_412_544_130_9, -0.090_046_728_600_047_37, -0.041_310_322_049_138_4, -0.006_724_712_895_219_338, 0.026_325_148_149_660_457, 0.072_531_754_801_769_04, 0.118_877_744_819_317_02, 0.190_396_237_814_965_1, 0.238_782_106_128_137_6, 0.314_356_624_288_423_37, 0.486_956_334_985_521_65, 0.695_692_761_871_830_7],
    [-1.420_014_003_199_247_2, -0.522_696_429_741_334_2, -0.406_210_965_545_054_04, -0.325_920_857_086_417_3, -0.256_350_320_454_709_7, -0.180_416_759_806_966_28, -0.123_591_685_465_956_81, -0.067_010_867_265_716_67, -0.024_806_234_727_056_592, 0.015_759_836_658_289_495, 0.072_965_908_148_756_2, 0.113_947_393_807_106_38, 0.187_324_213_972_436_76, 0.239_805_379_647_116_73, 0.316_335_096_277_357_45, 0.447_965_599_552_937, 0.695_692_761_871_830_7],
    [-1.420_014_003_199_247_2, -0.593_179_613_255_586_1, -0.449_572_212_481_025_6, -0.361_511_589_285_705_37, -0.266_970_119_997_586_96, -0.181_175_744_361_993_1, -0.123_463_920_580_746_75, -0.068_233_644_882_645_97, -0.010_091_370_111_840_007, 0.037_185_522_640_047_75, 0.102_639_776_557_262_54, 0.156_005_369_278_673_53, 0.210_718_821_398_755_22, 0.268_771_621_789_029_47, 0.332_501_224_185_259_9, 0.392_435_323_790_212_95, 0.695_692_761_871_830_7],
    [-1.420_014_003_199_247_2, -0.664_661_336_780_133_5, -0.524_284_624_042_301_1, -0.397_283_225_585_566_4, -0.274_792_563_795_425_66, -0.183_181_851_202_391_44, -0.112_431_912_909_653_57, -0.043_436_442_487_668_78, 0.017_308_474_499_954_936, 0.082_149_302_242_444_38, 0.130_181_714_804_523_4, 0.164_200_042_891_332_22, 0.217_072_530_806_446_15, 0.271_002_193_062_509_56, 0.320_625_166_582_700_57, 0.374_795_753_228_539_25, 0.562_514_929_382_508_9],
    [-1.412_428_164_693_674, -0.681_433_546_649_851_3, -0.570_174_328_543_471_2, -0.410_100_810_038_906_97, -0.293_064_471_565_358_03, -0.187_242_026_189_901_02, -0.110_644_356_795_511_35, -0.043_129_391_968_866_33, 0.019_417_910_903_877_07, 0.082_369_183_276_631_09, 0.121_652_731_569_827_41, 0.149_862_640_222_071_16, 0.203_305_073_915_686_25, 0.249_292_472_367_290_19, 0.288_370_016_957_415_9, 0.336_981_532_221_801_66, 0.493_838_596_312_707_9],
    [-1.412_428_164_693_674, -0.685_387_662_884_949_7, -0.552_039_489_571_550_2, -0.392_516_216_237_973_8, -0.287_171_161_879_401_67, -0.188_195_372_910_767_57, -0.110_524_483_477_835, -0.051_362_884_753_616_7, 0.003_371_061_392_293_726, 0.067_083_809_177_636_8, 0.109_934_651_244_517_48, 0.138_531_813_904_368_4, 0.179_335_792_278_388_1, 0.231_782_400_061_547_27, 0.267_073_167_811_707_3, 0.314_309_412_358_332_7, 0.407_470_764_302_773_96],
    [-1.133_929_823_932_041, -0.666_983_894_715_870_2, -0.527_828_827_836_454_8, -0.386_325_670_546_721_3, -0.302_099_176_574_135_1, -0.208_227_151_959_734_04, -0.131_751_790_780_413_32, -0.063_690_732_210_180_87, -0.002_915_814_028_714_386, 0.043_312_421_185_026_56, 0.098_489_712_535_167_38, 0.131_282_618_886_954_28, 0.171_117_905_900_639_94, 0.203_486_685_667_430_2, 0.248_721_855_619_160_7, 0.301_328_334_794_475_96, 0.374_299_077_217_290_94],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_29: [(f64, f64); 11] = [
    (0.0, 1.0),
    (2.895_592_363_109_328_5, 5.409_654_150_014_204),
    (-10.612_489_289_506_046, -7.781_108_657_130_76),
    (-12.0, -8.5),
    (-10.052_369_373_852_592, -7.563_145_491_927_914),
    (-0.267_860_650_834_757_74, 1.723_745_120_028_638_2),
    (-1.0, 0.1),
    (1.733_193_729_704_173_7, 6.121_247_066_640_631),
    (0.546_596_810_663_573_7, 1.429_758_467_095_557),
    (-1.319_039_117_967_191, 0.176_222_130_591_232_85),
    (0.273_604_915_406_215_95, 0.895_404_297_141_867_2),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_29: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.350_347_484_700_79, 1.339_466_460_114_085_8, 1.255_296_325_384_692, 1.213_438_132_752_871_6, 1.238_804_946_844_74, 1.258_610_719_098_004_7, 1.240_592_727_597_635_6, 1.201_110_502_626_166_8, 1.175_172_797_286_702_8, 1.156_252_338_228_383_4, 1.137_116_266_340_425_7, 1.134_506_900_255_689, 1.132_175_503_000_986_1, 1.116_538_835_601_775_6, 1.108_916_917_208_656_1, 1.109_843_737_780_821_2, 1.106_412_451_913_444_8],
    [0.488_259_278_749_944_2, 0.413_015_047_063_262_9, 0.398_748_885_738_563_6, 0.539_346_964_168_529_9, 1.036_689_248_963_815_3, 1.016_346_903_875_060_3, 0.971_872_462_963_946_1, 0.940_590_098_476_505_3, 0.916_059_427_063_355_3, 0.883_464_520_438_340_3, 0.837_490_813_157_591_6, 0.783_199_703_686_033_2, 0.776_977_671_340_674_5, 1.249_514_140_719_662, 1.310_148_354_787_116, 1.277_511_049_835_092_3, 0.868_212_273_113_022_3],
    [0.0, -0.043_75, -0.087_5, -0.131_249_999_999_999_98, -0.175, -0.218_75, -0.262_499_999_999_999_96, -0.306_249_999_999_999_97, -0.35, -0.393_75, -0.437_5, -0.481_249_999_999_999_96, -0.524_999_999_999_999_9, -0.568_75, -0.612_499_999_999_999_9, -0.656_25, -0.7],
    [1.951_127_281_357_212_3, 1.958_048_269_549_322_9, 2.028_903_318_778_159_5, 2.112_607_873_611_612, 2.161_830_756_284_969_5, 2.055_045_648_317_672, 2.016_888_011_168_784, 2.040_327_730_360_165_7, 2.017_545_975_581_15, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8],
    [-1.224_030_251_234_335_3, -1.239_577_734_289_407_7, -1.074_532_594_187_800_6, -0.909_969_946_289_217_8, -0.727_599_791_058_258_2, -0.625_843_069_522_113_2, -0.543_895_277_285_234_2, -0.306_210_310_044_107_4, -0.129_855_179_339_091_96, -0.085_548_488_290_231_34, -0.062_623_169_160_451_49, -0.043_116_612_095_477_99, -0.033_189_064_082_043_44, -0.090_218_753_629_177_61, -0.243_053_101_160_709_52, -0.333_649_818_325_749_4, -0.384_774_684_136_096_06],
    [0.0, -0.137_5, -0.275, -0.412_500_000_000_000_03, -0.55, -0.687_5, -0.825_000_000_000_000_1, -0.962_500_000_000_000_1, -1.1, -1.237_5, -1.375, -1.512_500_000_000_000_2, -1.650_000_000_000_000_1, -1.787_5, -1.925_000_000_000_000_3, -2.062_5, -2.2],
    [-0.448_444_177_711_024_3, -0.497_624_858_239_683_94, -0.569_368_737_836_118_2, -0.600_080_960_889_326, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4],
    [0.313_152_836_889_990_6, 0.380_087_897_337_835_9, 0.444_415_981_545_860_5, 0.478_189_662_310_366_25, 0.479_217_042_477_630_4, 0.448_207_400_339_373_7, 0.431_929_185_204_438_85, 0.441_160_917_726_070_6, 0.470_690_806_075_743_2, 0.515_223_338_176_220_1, 0.559_846_446_940_935_8, 0.615_594_595_042_114_6, 0.683_938_399_377_051_1, 0.767_129_359_824_846_1, 0.814_026_062_427_246_1, 0.812_116_890_506_310_8, 0.813_386_187_213_137_3],
    [0.234_433_560_700_501_4, 0.214_575_729_794_121_73, 0.186_353_167_266_156_97, -0.016_092_733_926_683_214, -0.016_092_733_926_683_214, 0.017_313_120_246_973_72, 0.042_423_167_758_869_14, 0.079_554_184_630_444_56, 0.191_859_624_144_576_14, 0.189_629_327_028_996_8, 0.196_646_439_488_965_65, 0.223_441_243_405_000_18, 0.223_521_200_940_458_58, 0.222_676_845_109_867_68, 0.233_813_719_431_720_58, 0.250_080_634_538_232_37, 0.262_960_568_206_388_47],
    [9.521_958_464_693_942, 9.457_634_238_072_226, 9.401_360_453_001_942, 9.368_585_833_351_792, 9.360_642_881_220_814, 9.370_971_927_443_762, 9.375_755_666_487_617, 9.370_204_460_706_26, 9.375_755_666_487_617, 9.387_215_394_401_206, 9.382_352_380_112_408, 9.390_042_636_259_766, 9.475_120_726_393_26, 9.589_987_145_522_036, 9.666_648_664_506_543, 9.671_029_990_166_69, 9.662_441_777_489_42],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_29: [(f64, f64); 11] = [
    (2.006_192_711_318_881_5, 5.418_370_850_344_337_5),
    (0.354_130_180_715_045, 1.38),
    (-0.356_948_201_150_830_5, 1.930_396_850_291_960_2),
    (-1.045_757, 0.038_753),
    (1.667_602_297_987_058_8, 7.145_041_221_830_653),
    (-1.617_989_701_238_915_5, 0.363_524_866_293_374_1),
    (-2.301_03, -0.698_97),
    (-0.746_275_547_186_667_8, 0.336_777_716_470_267_6),
    (0.027_544_719_217_018_96, 0.911_945_639_038_323_8),
    (-0.536_091_065_693_653_5, 0.490_453_627_556_435_1),
    (8.037_121_290_432_072, 10.078_348_360_620_978),
];

/// The fast accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_DONOR_KINDS_29: [f64; 3] = [
    0.991_852_273_418_232_9,
    0.0,
    0.008_147_726_581_767_09,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16 over all their
/// members, in [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_29: [[f64; 17]; 11] = [
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [-0.726_689_747_961_991_7, -0.331_805_700_985_712_55, -0.257_822_315_411_673_5, -0.210_427_511_941_527_73, -0.164_526_941_735_467_45, -0.105_990_462_759_318_4, -0.065_484_428_137_509_76, -0.031_667_197_529_083_924, -0.003_063_323_434_772_624_8, 0.030_406_145_191_142_37, 0.066_841_712_847_803_1, 0.103_283_899_351_956_55, 0.144_509_711_991_283_54, 0.222_034_913_643_104_82, 0.314_251_851_468_533_24, 0.401_596_845_320_278, 0.730_809_473_983_055_7],
    [-1.344_170_630_435_599, -0.389_118_617_475_549_1, -0.209_324_257_926_088_2, -0.107_130_705_795_699_78, -0.052_744_866_142_989_11, -0.027_465_883_542_501_83, -0.014_827_499_920_258_318, -0.006_870_478_674_801_984, 0.001_465_137_209_028_911_2, 0.011_880_709_497_901_01, 0.021_865_059_041_152_168, 0.038_023_337_198_321_176, 0.073_578_650_411_241_87, 0.135_200_960_644_326_16, 0.209_439_410_103_292_23, 0.381_445_801_761_918, 1.484_513_063_188_424_7],
    [-0.492_342_081_368_886_3, -0.021_887_211_963_985_28, -0.018_019_272_082_046_803, -0.014_679_213_819_943_32, -0.011_132_275_694_42, -0.008_559_256_176_820_917, -0.006_557_076_882_608_303_5, -0.004_702_301_499_870_463, -0.002_902_997_474_932_328, -0.000_908_813_576_322_241_8, 0.000_967_960_794_982_449_3, 0.002_701_283_589_426_781_3, 0.005_849_014_645_358_029, 0.011_070_554_062_188_548, 0.015_863_978_688_302_107, 0.020_947_376_212_240_64, 0.395_947_944_721_579_57],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.861_638_451_637_381_4, -0.149_063_374_996_416_34, -0.110_641_133_968_348_08, -0.085_535_177_223_182_73, -0.063_908_587_956_364_55, -0.042_715_096_009_118_41, -0.023_922_956_758_398_6, -0.006_885_892_119_364_714_5, 0.008_744_094_934_660_693, 0.027_233_103_928_576_11, 0.049_119_605_236_361_1, 0.097_393_098_330_913_78, 0.166_168_711_350_334_37, 0.333_166_978_939_285_43, 0.548_449_712_865_221, 0.692_007_757_415_642_4, 0.842_331_270_871_202_7],
    [-0.101_616_309_957_007_54, -0.017_352_523_561_954_956, -0.011_318_959_071_955_235, -0.006_108_523_153_221_169, -0.002_942_704_326_433_599_3, 0.000_455_696_194_857_373_9, 0.002_986_392_469_215_458, 0.004_473_140_352_240_671, 0.005_334_512_268_899_658, 0.005_938_888_104_762_435_5, 0.006_732_007_347_878_058, 0.007_363_502_801_348_323, 0.007_918_156_817_698_656, 0.008_657_799_679_141_043, 0.009_361_603_749_455_013, 0.010_327_160_212_124_06, 2.260_984_504_176_915],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.462_065_055_008_584_36, -0.125_533_244_929_907_1, -0.081_107_941_802_541_58, -0.057_185_332_756_631_71, -0.041_613_705_284_990_1, -0.029_196_201_003_773_428, -0.018_963_842_618_428_768, -0.008_868_836_609_219_171, 0.002_172_608_974_348_435_6, 0.013_959_612_799_769_067, 0.025_628_100_151_322_636, 0.036_684_406_642_857_31, 0.047_799_622_223_253_964, 0.060_197_242_916_549_7, 0.107_458_128_055_157_67, 0.212_295_272_384_651_5, 0.339_092_924_200_334_4],
    [-0.921_032_960_375_931_6, -0.680_688_514_310_209_9, -0.551_923_554_098_366, -0.427_287_601_897_960_1, -0.290_110_244_379_489_1, -0.168_564_788_401_240_42, -0.088_948_591_435_647_89, -0.037_956_648_503_122_41, 0.000_656_943_929_975_784_1, 0.031_817_171_438_486_54, 0.057_756_189_429_787_025, 0.085_335_630_496_213_6, 0.118_192_684_263_587_98, 0.172_914_329_928_169_8, 0.320_206_825_043_763_74, 0.596_867_705_783_974_1, 0.901_543_180_877_834_7],
    [-1.851_174_186_066_328_9, -0.437_589_773_036_454_6, -0.300_910_749_982_110_4, -0.210_825_525_814_388_9, -0.147_938_820_639_691_3, -0.100_037_569_392_472_38, -0.060_543_471_394_978_42, -0.027_987_062_991_188_66, 0.001_272_591_424_344_050_6, 0.027_763_401_603_766_488, 0.050_792_737_038_568_9, 0.071_568_614_692_360_77, 0.091_285_658_605_835_93, 0.110_225_306_842_619_69, 0.128_762_057_052_116_35, 0.154_274_623_694_437_87, 0.244_066_327_776_133_46],
];

/// The slow accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_SLOW_NODE_RESIDUALS_29: [[f64; 17]; 187] = [
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_223_373_954_145, 6.142_336_241_142_26, 6.331_502_807_278_19, 6.503_564_465_284_464, 6.664_945_164_773_575, 6.785_231_227_034_387, 6.895_299_274_133_498, 6.996_877_218_146_301_5, 7.172_543_195_811_916, 7.570_316_335_740_133, 7.895_860_960_787_201, 8.174_921_245_324_018, 8.409_476_610_411_19, 8.702_701_687_093_526, 10.0, 10.0],
    [-0.721_341_969_331_403_9, -0.346_012_386_077_272_36, -0.261_947_360_446_781_8, -0.198_765_217_529_075_53, -0.145_395_755_456_236_23, -0.096_920_147_518_962_69, -0.059_666_732_826_216_88, -0.025_662_924_358_750_557, 0.004_922_179_517_305_406, 0.031_654_751_778_347_47, 0.052_125_554_119_648, 0.071_332_018_503_947_26, 0.087_897_995_644_737_66, 0.106_255_437_022_470_67, 0.132_367_581_607_394_62, 0.173_696_961_063_890_15, 0.313_373_277_475_422_3],
    [-0.721_341_969_331_403_9, -0.324_353_815_792_166_93, -0.220_500_830_368_697_08, -0.145_285_700_557_955_48, -0.093_776_903_204_431_22, -0.062_430_622_239_217_456, -0.036_999_286_876_955_91, -0.009_973_928_999_087_08, 0.013_409_687_614_789_21, 0.036_351_355_380_913_55, 0.056_825_800_332_060_57, 0.074_782_909_116_367_15, 0.091_020_867_145_666_94, 0.107_023_739_190_044_57, 0.129_959_379_031_898_9, 0.162_794_220_946_925_94, 0.370_683_375_650_116_66],
    [-0.706_012_469_211_081_6, -0.318_207_117_224_353_85, -0.198_707_039_456_713_47, -0.137_277_791_910_160_5, -0.079_913_897_949_912_42, -0.046_617_422_195_663_74, -0.023_297_333_427_379_14, -0.004_171_568_138_598_753, 0.016_486_827_517_878_81, 0.035_919_461_130_584_66, 0.056_489_248_131_258_774, 0.077_053_441_339_773_24, 0.093_776_749_364_173_75, 0.109_117_518_351_921_32, 0.129_152_496_904_028_14, 0.157_813_044_711_102_2, 0.370_683_375_650_116_66],
    [-0.706_012_469_211_081_6, -0.381_158_404_033_874_24, -0.281_308_171_647_853_27, -0.207_282_542_388_888_07, -0.158_628_021_061_964_52, -0.082_016_057_696_899_08, -0.029_097_466_888_160_332, -0.007_115_221_372_961_436, 0.012_808_292_595_132_031, 0.032_657_225_124_201_356, 0.054_688_435_983_378_69, 0.080_768_508_009_086_76, 0.101_190_176_437_009_88, 0.120_214_491_009_783_28, 0.144_563_503_546_028_87, 0.198_894_519_249_546_99, 0.699_771_710_585_092_7],
    [-0.706_012_469_211_081_6, -0.375_377_941_101_953_1, -0.301_956_378_397_688_96, -0.256_047_444_382_680_5, -0.216_558_913_158_327_1, -0.177_620_838_785_001_48, -0.120_806_011_734_614_15, -0.079_409_837_127_762_88, -0.015_321_922_239_585_978, 0.017_207_142_713_311_974, 0.051_250_267_305_417_596, 0.094_007_472_897_064_97, 0.127_186_339_631_881_53, 0.176_293_137_552_353_15, 0.262_958_087_178_119, 0.393_936_092_912_767_54, 0.722_364_493_109_526_8],
    [-0.703_144_527_093_6, -0.324_552_987_804_777_65, -0.273_518_636_851_695_8, -0.247_492_537_392_846_02, -0.223_846_572_697_167_26, -0.189_763_913_379_245_92, -0.135_062_190_777_979_15, -0.100_108_547_372_961_13, -0.082_865_649_691_221_73, -0.035_013_848_063_640_635, 0.060_604_821_129_185_04, 0.161_812_803_408_278_5, 0.267_811_113_480_486_5, 0.372_141_939_257_726_65, 0.418_135_086_559_620_34, 0.477_900_768_775_134_5, 0.722_364_493_109_526_8],
    [-0.622_348_195_087_226_2, -0.312_935_387_212_113_84, -0.269_900_439_550_702_8, -0.248_619_788_420_851_94, -0.228_928_736_127_590_27, -0.192_594_974_574_032_66, -0.124_359_409_962_007_88, -0.094_194_546_525_223_5, -0.066_503_106_395_159_6, 0.081_843_604_867_803_91, 0.213_820_286_661_809_94, 0.306_773_575_468_837_5, 0.380_147_493_312_797_35, 0.411_105_612_725_267, 0.451_878_002_475_319_2, 0.503_134_106_019_137_1, 0.722_364_493_109_526_8],
    [-0.622_348_195_087_226_2, -0.431_314_937_684_222_6, -0.348_361_209_605_808_46, -0.281_108_792_959_014_53, -0.247_845_952_168_446_54, -0.201_412_462_610_565_06, -0.109_391_650_230_499_14, -0.022_000_067_387_766_73, 0.096_636_895_301_923_13, 0.154_854_847_994_593_37, 0.203_212_350_582_122_6, 0.246_762_000_419_651_66, 0.294_600_490_209_474_9, 0.348_055_828_835_580_17, 0.401_039_751_132_801, 0.461_008_797_749_504_2, 0.722_364_493_109_526_8],
    [-0.622_348_195_087_226_2, -0.404_110_215_777_202_74, -0.322_967_029_778_754_4, -0.258_166_530_729_569_47, -0.204_289_437_946_418_4, -0.156_143_156_008_883_3, -0.083_399_314_405_008_35, -0.038_334_879_739_711_56, 0.046_404_354_919_414_914, 0.118_106_862_546_437_36, 0.178_515_210_409_883_1, 0.219_934_973_704_969_42, 0.260_208_124_604_859_76, 0.301_393_835_355_907_7, 0.341_694_666_955_587_6, 0.396_512_771_857_198_1, 0.707_668_629_480_307_5],
    [-0.622_348_195_087_226_2, -0.271_014_372_512_454_2, -0.216_949_717_128_091_25, -0.181_329_222_508_816_97, -0.137_452_387_050_919_83, -0.074_930_964_073_484_83, -0.053_057_352_881_686_844, -0.032_505_875_189_179_25, -0.002_554_168_101_778_338_3, 0.043_275_690_072_824_57, 0.095_431_042_545_847_22, 0.174_595_860_820_512_84, 0.251_979_399_082_258_47, 0.305_523_467_938_367_8, 0.354_507_681_854_660_1, 0.412_627_893_330_458_13, 0.730_809_473_983_055_7],
    [-0.451_292_706_015_649_97, -0.203_661_978_550_432_6, -0.174_348_950_914_948_98, -0.138_502_453_439_007_7, -0.064_844_578_957_023_45, -0.047_180_658_979_938_055, -0.035_398_298_392_214_35, -0.022_678_811_839_426_213, -0.006_117_658_085_248_043, 0.019_215_552_766_728_303, 0.046_489_821_120_531_5, 0.075_139_292_320_589_16, 0.109_762_591_539_314_7, 0.192_931_534_291_741_15, 0.302_233_385_312_072_3, 0.383_685_600_064_406, 0.730_809_473_983_055_7],
    [-0.288_887_185_528_436_4, -0.181_295_896_603_804_19, -0.153_663_158_202_807_74, -0.082_786_910_301_440_17, -0.045_313_475_735_651_47, -0.035_726_855_073_691_464, -0.026_025_306_056_357_972, -0.016_328_599_139_871_353, -0.004_837_893_969_721_406, 0.012_310_699_740_430_999, 0.033_294_343_736_238_866, 0.054_343_992_453_100_77, 0.075_200_669_113_531_21, 0.101_262_559_247_779_05, 0.134_200_060_803_675_4, 0.282_452_287_360_905_9, 0.730_809_473_983_055_7],
    [-0.235_674_789_682_868_7, -0.166_053_953_026_190_4, -0.139_955_870_499_699, -0.048_680_416_021_206_82, -0.037_562_098_611_531_19, -0.029_645_219_252_645_953, -0.021_853_595_671_030_62, -0.012_484_539_804_937_364, -0.000_771_308_133_425_982_7, 0.011_703_677_954_360_545, 0.027_567_790_454_253_3, 0.045_150_784_828_256_91, 0.062_548_567_403_834_24, 0.082_842_794_009_795_29, 0.108_160_428_900_108_53, 0.139_410_086_333_064_12, 0.730_809_473_983_055_7],
    [-0.232_431_709_759_508_4, -0.150_241_531_794_960_47, -0.127_670_157_607_557_57, -0.041_097_878_743_429_725, -0.031_065_152_246_541_845, -0.024_276_264_885_311_73, -0.015_877_084_089_872_633, -0.005_158_401_702_266_758, 0.002_675_702_740_896_627, 0.013_128_528_550_820_957, 0.025_442_047_530_005_923, 0.044_221_730_646_515_425, 0.060_146_611_332_576_75, 0.075_444_364_153_963_8, 0.096_318_679_233_257_32, 0.137_792_288_759_642_78, 0.654_465_944_678_596_4],
    [-0.726_689_747_961_991_7, -0.331_805_700_985_712_55, -0.257_822_315_411_673_5, -0.210_427_511_941_527_73, -0.164_526_941_735_467_45, -0.105_990_462_759_318_4, -0.065_484_428_137_509_76, -0.031_667_197_529_083_924, -0.003_063_323_434_772_624_8, 0.030_406_145_191_142_37, 0.066_841_712_847_803_1, 0.103_283_899_351_956_55, 0.144_509_711_991_283_54, 0.222_034_913_643_104_82, 0.314_251_851_468_533_24, 0.401_596_845_320_278, 0.730_809_473_983_055_7],
    [-0.726_689_747_961_991_7, -0.297_904_128_924_013_6, -0.241_579_059_303_583_4, -0.180_246_448_581_211_67, -0.128_157_395_047_482_1, -0.085_008_786_942_169_01, -0.052_054_698_152_552_634, -0.025_212_060_344_461_117, -0.000_228_407_486_563_720_06, 0.031_425_719_185_368_13, 0.078_422_948_153_981_08, 0.139_500_099_696_948_13, 0.214_447_206_070_362_8, 0.275_053_392_101_922_2, 0.321_034_632_267_149_96, 0.358_862_222_244_066_9, 0.593_211_272_508_707_3],
    [-0.726_689_747_961_991_7, -0.297_779_855_640_412_76, -0.241_200_235_135_578_4, -0.179_350_566_094_515_6, -0.127_378_413_008_845_98, -0.084_206_568_820_188_34, -0.051_514_943_618_886_665, -0.024_838_402_081_108_258, -3.173_903_861_958_708e-6, 0.031_964_070_450_771_63, 0.078_937_687_476_678_48, 0.140_507_604_057_184_63, 0.214_543_365_258_312_58, 0.275_095_880_546_341_65, 0.320_904_586_508_915_4, 0.358_568_803_916_091_25, 0.389_902_565_443_805_75],
    [-0.176_205_881_722_746_12, -0.010_821_116_562_663_522, -0.004_674_536_984_927_95, -0.001_589_101_927_207_714_8, 0.000_746_551_533_834_034_1, 0.003_325_147_486_668_750_5, 0.005_630_287_002_750_390_5, 0.009_137_556_314_524_639, 0.013_353_845_208_926_983, 0.019_221_708_057_830_158, 0.023_897_653_147_603_11, 0.026_822_229_641_227_04, 0.030_481_839_370_152_464, 0.033_343_370_483_555_13, 0.036_671_979_163_070_87, 0.043_593_398_854_482_39, 1.240_276_886_221_321_4],
    [-0.176_205_881_722_746_12, -0.017_708_831_589_961_782, -0.010_546_014_243_233_514, -0.006_893_697_959_398_699, -0.003_981_974_520_620_445, -0.001_381_185_192_171_251_2, 0.001_006_507_371_128_627_6, 0.003_806_209_402_325_95, 0.006_617_686_062_439_276, 0.011_556_754_282_999_36, 0.016_722_196_931_688_436, 0.020_559_502_603_824_133, 0.024_087_411_350_253_746, 0.026_911_649_493_622_687, 0.030_765_457_808_800_335, 0.035_398_416_419_217_656, 1.313_643_353_799_167],
    [-0.176_205_881_722_746_12, -0.023_902_851_013_591_85, -0.015_402_610_857_932_904, -0.011_740_995_699_020_536, -0.009_078_458_909_491_839, -0.006_702_425_835_772_487, -0.004_114_535_946_801_063, -0.001_397_554_644_709_884, 0.001_530_951_976_991_777, 0.005_300_873_154_997_033, 0.010_514_768_188_425_614, 0.015_299_955_628_552_67, 0.018_337_969_757_552_038, 0.021_651_044_847_513_94, 0.024_868_653_970_001_14, 0.029_713_094_885_055_37, 1.313_643_353_799_167],
    [-0.176_205_881_722_746_12, -0.027_582_399_908_789_152, -0.020_940_173_756_567_36, -0.015_586_578_714_690_895, -0.013_139_859_979_527_529, -0.010_958_103_124_479_71, -0.008_717_720_072_673_333, -0.006_158_665_430_209_666, -0.003_343_182_337_135_494, 0.000_204_940_513_969_169_36, 0.005_361_835_852_599_210_5, 0.010_915_231_086_831_71, 0.014_818_676_135_593_515, 0.017_544_492_735_364_705, 0.020_687_099_378_214_71, 0.024_658_669_872_738_598, 1.328_598_754_244_953_6],
    [-0.135_847_185_675_728_76, -0.031_707_809_039_759_04, -0.025_958_222_618_195_987, -0.019_843_964_574_169_974, -0.016_593_479_657_790_528, -0.014_236_770_953_294_823, -0.011_933_000_026_570_282, -0.009_537_738_726_060_473, -0.006_281_809_506_365_352, -0.002_966_792_311_194_012, 0.001_898_257_401_928_750_2, 0.007_743_437_165_038_981, 0.011_718_176_774_682_355, 0.014_778_823_853_352_576, 0.017_816_708_424_176_132, 0.021_917_268_664_632_9, 1.484_513_063_188_424_7],
    [-0.135_847_185_675_728_76, -0.039_249_928_776_053_94, -0.032_136_436_481_905_91, -0.027_406_876_544_080_33, -0.021_774_046_431_856_86, -0.018_197_966_360_999_49, -0.015_690_042_833_656_517, -0.012_676_575_518_288_968, -0.009_147_679_471_305_677, -0.005_190_180_459_636_269, -0.000_845_238_612_719_736_3, 0.005_064_284_050_691_248, 0.009_218_118_846_418_858, 0.013_416_776_386_807_973, 0.017_630_032_305_637_797, 0.215_854_330_350_808_34, 1.484_513_063_188_424_7],
    [-0.523_300_202_142_344_1, -0.083_318_067_099_859_44, -0.052_841_283_399_649_96, -0.041_045_918_382_992_735, -0.034_150_141_105_153_825, -0.028_201_934_740_587_086, -0.022_513_554_753_409_642, -0.017_612_698_686_038_864, -0.011_932_893_782_592_938, -0.005_733_573_331_943_484, 0.001_476_414_773_733_57, 0.008_334_967_154_745_089, 0.018_863_115_856_846_642, 0.442_769_708_448_808_53, 0.653_949_390_397_882_1, 0.736_175_787_151_369_2, 1.484_513_063_188_424_7],
    [-0.542_504_629_903_040_2, -0.439_707_340_759_012_47, -0.260_205_445_991_476_55, -0.135_995_474_899_500_23, -0.066_051_005_248_241_66, -0.049_248_548_804_040_16, -0.034_577_619_506_058_89, -0.022_855_797_234_040_56, -0.005_752_616_581_989_622, 0.043_331_015_301_625_06, 0.154_291_838_625_065_62, 0.243_890_418_246_313_66, 0.370_915_615_837_636_94, 0.526_004_177_873_578, 0.641_674_132_697_296_1, 0.719_119_023_044_914_3, 1.484_513_063_188_424_7],
    [-0.542_504_629_903_040_2, -0.498_377_248_265_638_3, -0.472_021_199_180_921_9, -0.366_095_429_343_205_27, -0.189_610_124_831_271_8, -0.084_048_348_635_828_1, -0.043_422_351_114_363_905, 0.003_252_599_030_836_444_6, 0.052_647_104_641_248_575, 0.093_411_534_296_648_54, 0.130_444_735_873_110_87, 0.164_969_526_355_280_25, 0.201_850_969_226_876_06, 0.244_367_145_745_135_07, 0.357_814_374_820_563_44, 0.566_274_990_587_426_4, 1.133_539_618_283_955_2],
    [-0.542_504_629_903_040_2, -0.488_497_402_969_985_6, -0.442_752_590_289_422_43, -0.309_341_043_275_941_3, -0.135_836_981_610_438_06, -0.078_901_822_240_463_84, -0.029_626_506_611_220_207, 0.014_974_444_800_331_117, 0.049_235_036_554_613_43, 0.082_189_423_738_421_61, 0.108_380_824_846_730_92, 0.138_476_263_325_655_38, 0.164_200_012_095_310_68, 0.192_739_126_648_538_98, 0.222_374_622_299_690_46, 0.262_648_467_785_847_3, 1.316_567_244_743_094_7],
    [-0.542_504_629_903_040_2, -0.391_137_596_512_704_74, -0.282_777_237_307_911_43, -0.182_450_916_458_007_6, -0.120_896_234_195_740_53, -0.088_399_234_220_672_27, -0.056_231_680_826_218_676, -0.024_918_109_036_445_51, 0.006_106_277_232_243_475, 0.039_222_751_220_205_6, 0.076_046_085_037_662_8, 0.111_855_367_300_034_76, 0.146_201_679_355_900_23, 0.184_603_403_358_324_7, 0.220_197_832_559_545_36, 0.254_898_339_050_051_3, 1.403_211_724_118_181_6],
    [-0.534_151_104_666_489_9, -0.343_262_923_295_641_9, -0.237_532_296_960_805_86, -0.177_962_553_809_785_73, -0.128_134_802_253_724_53, -0.095_295_415_791_472_92, -0.067_339_298_976_749_58, -0.040_507_641_942_945_02, -0.014_664_679_259_678_63, 0.008_526_977_825_159_47, 0.033_620_088_527_296_63, 0.060_308_914_854_525_65, 0.096_825_681_173_651_34, 0.146_586_337_445_092_1, 0.201_312_180_649_744_3, 0.249_173_354_330_318_4, 1.441_678_612_452_206_7],
    [-0.587_188_186_948_529_1, -0.369_180_563_986_983_35, -0.267_308_947_666_092_3, -0.192_611_321_026_230_44, -0.134_873_114_211_484_43, -0.089_693_390_488_310_85, -0.056_115_976_142_503_27, -0.025_283_246_981_259_368, -0.001_278_455_167_066_253_6, 0.019_871_005_962_736_176, 0.038_498_846_432_115_16, 0.056_184_541_434_463_144, 0.072_411_025_315_240_49, 0.104_229_898_440_540_9, 0.169_432_860_713_693_35, 0.241_760_919_371_603_93, 1.471_113_880_964_431_5],
    [-1.234_068_639_115_436, -0.456_979_362_478_263_93, -0.381_120_621_897_406_53, -0.262_546_506_379_743_25, -0.170_533_285_406_002_38, -0.081_838_653_943_195_02, -0.028_005_994_578_931_15, 0.003_127_822_804_226_761_7, 0.024_485_432_830_059_863, 0.042_389_053_755_063_08, 0.056_618_361_064_643_85, 0.067_573_314_676_938_38, 0.082_200_728_149_384_96, 0.123_470_754_272_527_21, 0.226_264_890_035_735_5, 0.755_057_258_570_678_6, 1.471_113_880_964_431_5],
    [-1.335_662_381_787_965_4, -0.753_727_460_121_532_1, -0.533_987_320_444_309_7, -0.463_536_203_438_951_13, -0.385_463_634_294_593_6, -0.202_910_925_044_987_56, -0.017_420_267_005_127_426, 0.024_140_732_127_957_896, 0.048_895_015_942_294_284, 0.064_285_717_146_152_5, 0.082_511_670_361_715_01, 0.142_507_260_893_002_03, 0.391_520_922_524_361_44, 0.647_652_065_822_189_6, 0.747_081_020_200_700_7, 0.961_797_124_386_366_1, 1.471_113_880_964_431_5],
    [-1.344_170_630_435_599, -1.198_176_747_542_614_2, -0.915_408_308_012_290_7, -0.650_193_425_686_342_1, -0.507_866_960_157_908, -0.164_992_058_499_092_9, -0.021_998_534_902_371_063, 0.013_478_767_102_017_305, 0.044_053_865_176_786_076, 0.078_656_070_070_669_69, 0.139_597_488_642_038_41, 0.239_502_654_993_219_88, 0.353_348_543_684_849_2, 0.573_757_639_590_629_5, 0.701_213_608_158_583_5, 0.832_454_312_374_279_6, 1.471_113_880_964_431_5],
    [-1.344_170_630_435_599, -1.258_317_001_622_127_2, -0.834_018_193_781_874_3, -0.200_379_749_501_699_8, -0.082_007_561_145_418_67, -0.045_642_223_343_874_43, -0.019_911_383_505_400_72, -0.002_201_846_891_647_15, 0.015_468_263_478_088_032, 0.040_064_566_572_148_214, 0.059_769_459_319_345_36, 0.083_368_739_882_865_96, 0.131_322_465_516_883_17, 0.187_044_761_679_101_6, 0.335_703_011_632_386, 0.709_971_736_927_248_5, 1.302_200_607_868_516_9],
    [-0.151_279_974_080_070_38, -0.005_035_031_800_934_056, -0.000_667_740_342_282_680_2, 0.001_891_437_704_996_988_7, 0.004_112_219_177_936_953, 0.006_729_891_346_819_095, 0.009_076_606_177_910_371, 0.011_145_028_585_084_068, 0.013_395_281_679_087_875, 0.015_008_631_208_838_424, 0.016_993_679_845_624_46, 0.019_202_882_037_677_102, 0.020_990_563_141_161_475, 0.023_679_429_112_455_803, 0.026_094_352_019_618_593, 0.029_766_987_455_667_23, 0.070_877_186_986_796_01],
    [-0.151_279_974_080_070_38, -0.012_228_123_303_025_992, -0.007_190_269_235_318_344, -0.004_302_345_598_602_647_4, -0.002_070_543_786_699_036, -0.000_159_028_950_638_913_78, 0.001_631_550_395_551_504_6, 0.003_274_305_402_943_947_7, 0.005_128_714_230_487_296, 0.007_369_220_407_924_332_4, 0.009_624_334_314_976_566, 0.012_229_718_739_486_882, 0.014_553_172_601_606_532, 0.017_279_847_502_514_375, 0.020_577_873_331_696_632, 0.025_031_143_887_518_7, 0.047_456_694_460_145_44],
    [-0.146_310_872_236_597_78, -0.018_240_532_613_026_845, -0.013_222_974_357_671_34, -0.009_922_158_017_318_014, -0.007_910_140_540_021_064, -0.006_135_475_661_883_265, -0.004_556_206_677_504_138, -0.003_029_878_321_946_960_6, -0.001_489_082_347_025_213_3, 0.000_154_039_728_720_300_9, 0.001_799_429_350_282_929_8, 0.003_319_708_820_020_405_5, 0.005_609_099_108_143_553, 0.009_188_323_825_150_457, 0.013_626_342_417_894_228, 0.017_874_282_060_148_75, 0.041_619_596_273_475_196],
    [-0.144_210_161_414_672_85, -0.021_743_392_761_853_126, -0.018_193_934_302_390_16, -0.015_318_623_278_097_415, -0.012_472_081_326_255_794, -0.010_324_585_465_214_265, -0.008_672_185_508_089_486, -0.007_440_582_929_687_849, -0.006_212_161_025_338_026, -0.004_852_960_417_824_854, -0.003_476_961_609_278_652, -0.001_953_722_112_038_843_2, -0.000_138_131_007_894_593_15, 0.001_445_369_406_696_125, 0.003_150_124_978_483_291_6, 0.012_417_216_069_254_378, 0.026_465_771_848_605_824],
    [-0.144_210_161_414_672_85, -0.022_689_007_290_194_935, -0.020_875_152_926_317_198, -0.018_066_551_554_676_593, -0.015_740_328_654_568_578, -0.013_136_903_270_936_253, -0.011_247_557_799_354_754, -0.009_423_188_832_961_397, -0.008_035_154_155_356_956, -0.006_683_778_225_771_851, -0.005_536_890_144_488_876, -0.004_087_926_547_465_037, -0.002_080_781_394_369_733_8, 0.000_424_674_086_913_721_06, 0.001_980_434_330_066_085_2, 0.012_130_171_729_692_205, 0.018_344_908_661_594_897],
    [-0.144_210_161_414_672_85, -0.022_964_375_083_483_23, -0.021_486_932_974_234_8, -0.019_633_002_548_963_257, -0.017_260_896_674_314_64, -0.014_972_226_369_470_586, -0.012_354_430_395_339_427, -0.010_398_771_024_677_06, -0.008_478_185_556_144_718, -0.006_963_766_240_786_711, -0.005_729_714_020_569_978, -0.004_316_983_252_333_909, -0.002_267_565_188_538_519, 0.000_178_501_922_264_089, 0.001_949_951_815_193_948_5, 0.012_205_436_057_493_453, 0.020_838_584_983_396_94],
    [-0.079_124_541_330_797_4, -0.023_486_715_560_023_838, -0.021_679_721_638_751_308, -0.019_998_146_969_752_138, -0.017_882_897_088_557_125, -0.016_014_197_307_229_982, -0.013_132_097_037_953_287, -0.010_575_488_179_348_579, -0.008_827_758_162_431_783, -0.006_873_197_844_659_977_5, -0.005_710_269_465_194_895, -0.003_855_150_184_735_789_5, -0.002_284_309_575_974_601_8, -0.000_152_723_413_610_552_9, 0.001_840_617_998_727_026, 0.012_978_565_435_797_064, 0.022_400_283_881_667_083],
    [-0.083_160_766_531_016_63, -0.023_015_478_988_329_37, -0.020_591_691_856_094_087, -0.018_953_440_223_717_276, -0.017_100_980_844_249_148, -0.015_498_798_963_881_266, -0.012_532_046_675_443_16, -0.010_009_439_572_254_265, -0.008_177_606_085_573_821, -0.006_160_445_614_607_132, -0.004_160_717_762_130_625, -0.002_828_013_644_906_209, -0.001_126_960_357_256_856_7, 0.000_679_042_575_176_142_1, 0.002_849_580_098_157_990_4, 0.009_474_086_057_508_348, 0.023_650_059_263_285_866],
    [-0.083_160_766_531_016_63, -0.021_377_498_109_820_474, -0.019_122_484_022_092_778, -0.017_125_428_364_632_708, -0.015_887_856_025_799_493, -0.013_730_463_511_167_837, -0.011_251_352_387_891_763, -0.008_902_365_281_368_408, -0.006_787_578_414_085_772, -0.004_160_174_684_740_392, -0.002_859_502_481_361_317, -0.001_440_453_817_593_996_2, 0.000_155_989_359_712_157_32, 0.001_958_256_006_870_665_3, 0.004_981_449_765_854_498_5, 0.007_331_021_541_764_523, 0.023_650_059_263_285_866],
    [-0.264_308_312_123_055_8, -0.021_986_338_880_280_065, -0.019_027_216_638_404_485, -0.016_894_357_390_561_648, -0.015_749_343_740_647_962, -0.013_401_900_819_601_804, -0.011_261_553_124_787_45, -0.008_919_797_895_466_633, -0.006_342_778_924_513_639, -0.003_982_094_053_065_91, -0.002_863_635_211_570_119_3, -0.001_459_101_779_944_84, 0.000_161_182_559_914_411_04, 0.002_013_525_322_040_191_8, 0.004_438_322_471_959_464, 0.006_899_932_134_871_991, 0.023_650_059_263_285_866],
    [-0.264_308_312_123_055_8, -0.022_625_316_980_402_207, -0.019_820_229_013_811_794, -0.017_036_212_268_653_93, -0.015_903_477_564_109_86, -0.014_268_560_102_851_793, -0.011_721_071_225_457_852, -0.009_570_906_646_355_598, -0.006_765_264_368_350_042, -0.004_157_910_335_257_89, -0.002_871_817_043_752_153_4, -0.001_711_216_681_870_080_6, -5.444_820_041_210_896e-5, 0.001_232_426_014_994_648_5, 0.003_708_741_278_334_548, 0.005_740_898_069_408_732, 0.023_650_059_263_285_866],
    [-0.264_308_312_123_055_8, -0.022_369_774_314_437_728, -0.019_458_834_844_182_44, -0.017_115_428_577_261_154, -0.016_395_695_452_155_058, -0.014_997_587_841_901_177, -0.012_057_215_035_496_394, -0.009_338_226_391_786_058, -0.005_973_691_707_311_521, -0.004_011_504_078_912_036, -0.002_466_744_918_132_635, -0.001_202_715_798_176_296_1, -0.000_193_112_969_677_370_03, 0.002_159_666_463_188_814, 0.004_343_852_290_017_854, 0.007_313_315_331_822_808, 0.023_114_652_156_030_05],
    [-0.264_308_312_123_055_8, -0.023_479_164_901_139_823, -0.020_128_287_613_891_94, -0.018_145_626_888_845_52, -0.016_880_612_809_862_344, -0.016_146_273_142_793_07, -0.012_513_378_217_780_226, -0.010_779_155_408_534_899, -0.008_194_235_734_199_104, -0.005_320_077_759_631_971, -0.003_860_138_385_406_675_3, -0.002_447_597_057_613_968_4, -0.000_945_965_381_790_727_5, 0.001_864_586_224_489_513_2, 0.004_171_018_795_612_117, 0.013_028_625_601_002_305, 0.180_513_096_975_630_56],
    [-0.068_707_359_040_450_25, -0.025_182_112_254_465_813, -0.023_569_231_780_184_826, -0.020_242_202_755_061_233, -0.018_510_079_925_065_648, -0.016_753_109_133_704_55, -0.012_801_023_624_482_585, -0.012_034_634_072_337_368, -0.010_779_575_369_633_379, -0.009_118_727_074_538_709, -0.007_609_272_226_087_815, -0.004_197_536_984_301_659, -0.003_442_455_372_286_831_7, -0.000_798_406_773_777_445_5, 0.003_697_490_960_807_892_8, 0.015_392_280_753_668_006, 0.180_513_096_975_630_56],
    [-0.068_707_359_040_450_25, -0.025_319_918_746_563_416, -0.023_698_988_545_433_158, -0.020_384_688_783_770_7, -0.019_260_783_077_235_86, -0.015_810_470_058_823_06, -0.012_494_577_511_714_055, -0.012_176_370_944_910_891, -0.010_924_809_021_622_437, -0.009_495_418_222_749_132, -0.007_738_788_333_467_404, -0.004_421_516_924_569_035, -0.002_675_137_842_604_298_6, 0.001_335_341_834_118_388_8, 0.012_209_093_815_413_612, 0.017_097_452_723_099_472, 0.180_513_096_975_630_56],
    [-0.051_073_114_129_384_256, -0.025_146_587_955_003_078, -0.020_736_143_224_211_908, -0.018_746_616_576_348_32, -0.015_744_295_941_707_266, -0.014_879_590_913_497_337, -0.012_870_436_631_515_8, -0.012_135_265_339_451_804, -0.008_583_367_023_380_83, -0.005_171_196_046_819_199, -0.003_900_714_494_672_406_3, 1.314_947_729_149_453e-5, 0.001_006_095_150_537_161, 0.002_353_410_705_713_429_8, 0.007_657_932_879_221_543, 0.019_714_376_533_422_326, 0.395_947_944_721_579_57],
    [-0.492_342_081_368_886_3, -0.021_887_211_963_985_28, -0.018_019_272_082_046_803, -0.014_679_213_819_943_32, -0.011_132_275_694_42, -0.008_559_256_176_820_917, -0.006_557_076_882_608_303_5, -0.004_702_301_499_870_463, -0.002_902_997_474_932_328, -0.000_908_813_576_322_241_8, 0.000_967_960_794_982_449_3, 0.002_701_283_589_426_781_3, 0.005_849_014_645_358_029, 0.011_070_554_062_188_548, 0.015_863_978_688_302_107, 0.020_947_376_212_240_64, 0.395_947_944_721_579_57],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.179_638_195_845_888_58, -0.144_823_583_593_578_4, -0.110_561_541_451_383_47, -0.081_102_260_099_692_61, -0.064_781_116_191_659_17, -0.038_110_767_325_377_47, -0.024_491_406_863_615_908, 0.003_674_931_161_790_791, 0.034_419_186_660_321_45, 0.077_837_969_490_595_67, 0.113_635_821_462_315_68, 0.149_110_749_372_825_6, 0.199_890_416_904_581_2, 0.278_789_644_974_853_8, 0.337_298_499_461_006_76, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_613_1, -0.234_036_302_028_132_8, -0.189_937_372_030_544_45, -0.154_746_844_159_206_42, -0.135_800_438_175_854_6, -0.101_591_731_985_079_38, -0.071_738_651_717_408_51, -0.038_023_755_233_999_71, -0.011_972_473_648_274_37, 0.039_512_428_135_258_876, 0.104_127_330_330_901_24, 0.146_699_104_041_263_24, 0.199_040_036_298_274_33, 0.259_011_911_984_733_33, 0.286_350_782_977_096_2, 0.339_711_718_826_923_75, 0.549_121_467_614_262_8],
    [-0.640_470_106_597_607_5, -0.188_806_079_606_911_68, -0.154_429_544_623_429_8, -0.135_594_323_432_660_62, -0.111_233_204_693_630_02, -0.090_381_730_007_631_43, -0.070_155_200_097_887_71, -0.044_075_764_363_516_34, -0.028_960_422_412_097_064, 0.004_301_136_471_860_253, 0.048_408_613_393_469_05, 0.104_464_382_570_607_76, 0.150_539_519_116_775_04, 0.199_580_661_816_781_27, 0.283_158_761_774_631_1, 0.339_268_356_874_965_04, 0.549_121_467_614_262_8],
    [-0.302_242_031_195_399_94, -0.163_918_680_346_099_27, -0.130_193_914_989_280_3, -0.109_705_478_147_357_1, -0.088_876_720_166_712_39, -0.070_095_273_569_264_07, -0.045_159_275_266_010_49, -0.033_294_325_067_910_18, -0.012_059_643_309_978_26, 0.028_991_546_170_754_355, 0.058_352_128_650_192_13, 0.104_747_021_478_349_33, 0.141_499_287_148_465_2, 0.180_448_642_619_404_88, 0.268_201_479_170_173_4, 0.327_369_529_311_245_6, 0.549_121_467_614_262_8],
    [-0.248_199_228_013_739_28, -0.143_842_421_318_612_83, -0.112_592_503_236_541_27, -0.082_452_519_316_158_88, -0.069_015_998_446_252_74, -0.044_003_905_715_482_22, -0.033_231_380_649_178_1, -0.015_769_881_917_658_032, 0.025_642_814_892_889_203, 0.042_238_884_318_500_494, 0.085_984_077_447_391_66, 0.112_176_695_097_360_54, 0.139_994_630_561_318_14, 0.176_811_210_702_974_43, 0.240_379_897_175_707_3, 0.314_765_199_864_344_65, 0.549_121_467_614_262_8],
    [-0.197_187_486_681_730_03, -0.134_147_666_927_103_1, -0.098_583_855_040_779_3, -0.070_337_107_872_225_63, -0.044_005_390_082_603_076, -0.036_854_424_554_486_14, -0.027_482_027_031_775_436, -0.000_900_911_612_207_117_4, 0.025_745_609_781_048_2, 0.038_302_587_725_490_36, 0.090_044_492_712_179_97, 0.109_006_222_652_942_33, 0.118_793_608_321_927_83, 0.142_641_357_274_298_5, 0.204_591_685_452_294_6, 0.297_245_230_614_012_2, 0.349_053_675_077_441_1],
    [-0.082_854_320_532_313_56, -0.020_369_139_469_193_26, -0.007_753_676_770_148_087, 0.004_905_224_929_569_034, 0.015_363_950_507_623_517, 0.024_197_734_197_505_563, 0.031_225_051_461_397_185, 0.037_501_940_062_101_696, 0.045_195_083_701_427_5, 0.063_669_454_265_834_4, 0.116_621_236_267_549_64, 0.152_758_833_484_609_67, 0.187_979_343_784_961_4, 0.240_341_254_954_028_97, 0.312_660_272_269_451, 0.374_388_654_043_501_3, 0.515_387_177_145_576_2],
    [-0.087_041_074_464_599_97, -0.020_240_557_828_108_404, -0.008_603_138_322_544_019, 0.000_798_715_001_747_956_4, 0.009_383_387_427_261_067, 0.017_002_959_696_714_563, 0.023_818_949_388_941_246, 0.029_681_117_304_998_67, 0.035_325_381_235_698_51, 0.041_690_157_697_957_746, 0.049_565_093_755_604_26, 0.105_207_898_544_139_31, 0.283_290_882_686_966_1, 0.375_246_555_435_488_57, 0.454_622_388_697_834_83, 0.538_467_719_771_779_2, 0.739_914_449_536_719_1],
    [-0.184_358_364_695_180_52, -0.078_980_480_910_003_3, -0.052_731_653_676_179_47, -0.035_201_800_725_481_534, -0.022_401_574_047_973_546, -0.011_872_163_688_505_232, -0.002_705_473_897_219_772_8, 0.005_783_072_829_960_511, 0.014_763_606_336_600_028, 0.024_006_992_324_847_9, 0.033_086_266_692_944_78, 0.044_879_427_979_334_29, 0.342_058_434_106_896_7, 0.507_246_781_008_978_8, 0.584_851_656_192_566_1, 0.669_880_190_349_952_5, 0.842_331_270_871_202_7],
    [-0.861_638_451_637_381_4, -0.137_125_056_643_474_8, -0.108_696_196_959_583_21, -0.087_754_786_824_366_5, -0.070_873_563_744_604_78, -0.054_027_400_577_759_68, -0.039_606_252_393_410_38, -0.025_997_990_989_611_905, -0.013_150_073_035_970_856, -0.000_640_209_663_056_517_6, 0.013_329_376_950_341_535, 0.035_009_257_668_213_81, 0.485_344_698_731_606, 0.607_038_594_184_176_6, 0.678_664_941_248_332_4, 0.740_657_796_335_006_2, 0.842_331_270_871_202_7],
    [-0.861_638_451_637_381_4, -0.245_949_378_874_638_3, -0.140_276_176_488_139_7, -0.118_431_154_184_943_43, -0.102_695_632_112_488_1, -0.085_797_609_242_217_92, -0.068_311_888_914_960_32, -0.048_546_435_071_442_84, -0.026_412_099_013_813_17, 8.570_593_420_276_883e-5, 0.132_436_718_030_817_4, 0.250_977_617_930_179_53, 0.478_119_994_823_539_7, 0.629_492_516_665_378_5, 0.704_743_795_420_310_7, 0.762_293_795_998_695_1, 0.842_331_270_871_202_7],
    [-0.861_638_451_637_381_4, -0.271_587_868_273_248_8, -0.144_813_291_319_910_36, -0.117_993_169_561_382_42, -0.097_496_076_876_200_61, -0.072_884_642_484_604_6, -0.040_751_294_948_808_185, 0.018_905_463_631_623_73, 0.114_600_779_318_975_26, 0.148_956_609_276_795_05, 0.169_424_033_639_249_1, 0.192_530_315_553_117_73, 0.284_669_077_622_895_8, 0.414_998_493_892_943_96, 0.579_473_270_552_762_8, 0.736_887_922_338_815_5, 0.842_331_270_871_202_7],
    [-0.861_638_451_637_381_4, -0.174_017_036_433_084_97, -0.118_296_921_380_599_47, -0.085_916_215_891_132_31, -0.059_914_462_622_873_05, -0.032_521_276_016_247_894, -0.005_073_973_269_945_002, 0.022_881_892_241_460_98, 0.055_577_271_672_194_96, 0.088_582_377_967_424_69, 0.123_303_280_485_368_69, 0.145_106_939_648_541_27, 0.162_941_725_834_347_27, 0.178_217_565_512_039_5, 0.200_677_845_971_887_23, 0.335_227_557_335_082_3, 0.826_777_647_408_653_9],
    [-0.302_339_660_410_117_06, -0.158_617_933_670_310_8, -0.118_673_310_048_771_23, -0.091_696_682_456_399_75, -0.070_943_498_147_649_14, -0.051_601_871_547_215_704, -0.032_285_544_408_038_07, -0.013_865_473_189_451_432, 0.004_274_297_830_255_64, 0.022_482_572_317_684_77, 0.042_047_752_661_333_97, 0.059_995_181_215_226_58, 0.077_377_357_488_070_1, 0.099_072_788_113_531_55, 0.120_932_335_546_225_15, 0.158_679_664_180_891_1, 0.485_701_196_259_941_35],
    [-0.285_059_480_347_857_1, -0.135_305_085_863_267_17, -0.100_797_154_913_063, -0.076_678_056_029_352_22, -0.056_963_748_529_219_246, -0.040_137_651_972_023_11, -0.023_742_767_759_981_347, -0.009_286_270_029_765_552, 0.005_092_174_468_293_473, 0.019_376_913_266_254_344, 0.033_095_339_933_019_91, 0.049_215_650_803_542_25, 0.061_418_720_210_837_635, 0.074_091_352_144_383_43, 0.091_573_183_902_877_3, 0.109_877_961_612_121_75, 0.516_761_670_694_796_7],
    [-0.251_751_329_416_819_66, -0.104_326_993_143_607_41, -0.076_033_169_333_484_06, -0.057_663_507_625_487_69, -0.043_213_728_514_390_2, -0.029_944_087_812_543_183, -0.016_406_269_018_903_65, -0.004_915_375_165_754_447, 0.006_834_865_077_691_61, 0.018_228_545_400_627_96, 0.029_167_349_091_324_9, 0.043_684_532_446_010_8, 0.054_710_556_891_197_46, 0.066_654_170_873_840_42, 0.081_830_811_477_821_73, 0.102_097_819_370_596_95, 0.554_614_559_151_012_1],
    [-0.399_464_488_123_331_96, -0.084_555_599_979_933_78, -0.063_398_900_410_181_79, -0.049_042_820_486_291_494, -0.035_717_350_908_347_73, -0.024_832_559_831_900_945, -0.014_082_939_706_414_08, -0.005_087_381_717_022_169, 0.005_149_767_545_679_799_5, 0.015_341_919_374_125_202, 0.022_967_452_972_999_198, 0.035_383_028_374_316_72, 0.046_989_575_724_108_61, 0.062_285_670_720_779_285, 0.087_619_854_700_065_67, 0.468_630_968_204_087_9, 0.554_614_559_151_012_1],
    [-0.510_571_719_270_341_8, -0.104_388_561_636_479_46, -0.069_902_082_440_929_37, -0.051_280_166_759_464_35, -0.035_631_729_526_210_65, -0.024_798_662_013_720_24, -0.012_644_358_752_531_988, -0.001_380_402_524_384_633_1, 0.009_538_998_771_916_293, 0.020_421_762_954_704_648, 0.034_899_172_350_753_61, 0.047_949_380_084_090_956, 0.069_711_133_203_922_93, 0.218_058_204_903_232_3, 0.382_552_202_047_788_56, 0.504_463_382_286_980_5, 0.554_614_559_151_012_1],
    [-0.553_660_161_080_307_8, -0.125_127_849_468_294_67, -0.089_601_593_792_345_96, -0.068_967_936_170_290_76, -0.050_755_346_227_750_56, -0.033_722_003_045_415_06, -0.017_824_057_853_326_983, -0.000_568_376_852_128_524_1, 0.018_047_811_528_185_177, 0.030_853_709_436_189_35, 0.044_254_245_932_794_097, 0.055_134_642_893_852_99, 0.067_705_196_795_920_99, 0.079_092_212_908_551_09, 0.176_384_609_188_908_41, 0.318_366_873_353_963, 0.554_614_559_151_012_1],
    [-0.614_908_624_354_833_3, -0.123_480_986_413_761_25, -0.091_041_791_767_417_07, -0.071_769_623_286_857_42, -0.053_715_198_280_610_9, -0.037_816_432_251_487_26, -0.023_297_021_756_203_223, -0.009_072_965_112_536_243, 0.012_231_344_544_879_381, 0.027_465_381_133_997_963, 0.038_774_655_179_780_95, 0.047_946_120_402_777_556, 0.058_224_002_053_123_24, 0.067_874_407_355_187_57, 0.076_824_380_375_463_59, 0.090_587_803_286_547_57, 0.517_873_981_079_024_5],
    [-0.614_908_624_354_833_3, -0.504_415_370_731_925_9, -0.096_628_185_997_250_68, -0.069_679_575_967_238_34, -0.048_691_805_340_874_375, -0.033_770_409_971_407_23, -0.019_796_773_296_674_824, -0.008_886_032_821_075_735, 0.003_254_915_894_329_72, 0.026_405_548_973_321_147, 0.041_349_059_184_099_24, 0.051_482_938_394_152_24, 0.064_622_427_840_389_7, 0.072_860_212_143_295_95, 0.081_809_524_899_632_39, 0.099_759_987_056_666_82, 0.547_124_725_225_549_6],
    [-0.614_908_624_354_833_3, -0.545_541_063_391_928_5, -0.495_762_330_904_597_04, -0.109_231_594_799_874_6, -0.051_086_722_163_044_98, -0.037_604_411_899_302, -0.018_061_152_339_900_58, -0.006_256_348_212_373_652, 0.004_635_972_250_019_981, 0.033_523_248_331_281, 0.061_865_380_675_442_1, 0.072_650_917_336_315_32, 0.085_892_971_282_681, 0.103_094_052_820_431_24, 0.121_290_367_660_044_97, 0.317_269_495_673_125_96, 0.690_581_667_801_033_2],
    [-0.614_908_624_354_833_3, -0.496_022_915_207_693_7, -0.139_548_535_140_995_3, -0.085_856_208_382_176_2, -0.062_758_186_066_086_94, -0.045_439_172_717_245_256, -0.039_813_106_271_955_38, -0.034_579_290_819_745_014, -0.018_454_675_277_000_124, -0.013_102_554_039_345_701, 0.002_844_212_339_569_25, 0.040_774_493_142_065_854, 0.112_712_694_235_859_86, 0.210_793_253_182_134_27, 0.458_200_829_335_657, 0.574_655_966_694_303_2, 0.690_581_667_801_033_2],
    [-0.101_616_309_957_007_54, -0.017_352_523_561_954_956, -0.011_318_959_071_955_235, -0.006_108_523_153_221_169, -0.002_942_704_326_433_599_3, 0.000_455_696_194_857_373_9, 0.002_986_392_469_215_458, 0.004_473_140_352_240_671, 0.005_334_512_268_899_658, 0.005_938_888_104_762_435_5, 0.006_732_007_347_878_058, 0.007_363_502_801_348_323, 0.007_918_156_817_698_656, 0.008_657_799_679_141_043, 0.009_361_603_749_455_013, 0.010_327_160_212_124_06, 2.260_984_504_176_915],
    [-0.001_467_309_005_945_383, -0.000_225_210_210_818_778_71, 0.000_475_900_948_889_269_26, 0.001_370_640_957_462_010_7, 0.001_921_795_805_461_513_3, 0.002_522_076_972_483_67, 0.003_014_729_474_202_753_7, 0.004_275_330_239_286_62, 0.004_881_216_155_439_775_5, 0.005_269_160_005_722_966_5, 0.005_346_934_771_407_206, 0.005_977_247_647_028_33, 0.007_554_885_601_599_844_5, 0.008_390_111_619_683_982, 0.009_664_681_339_432_138, 0.011_310_766_273_891_692, 1.669_418_147_147_517_6],
    [-0.001_467_309_005_945_383, 0.001_760_866_686_978_615_5, 0.003_012_792_467_291_293_6, 0.004_383_385_529_392_743, 0.005_008_488_010_337_366_6, 0.005_346_823_046_262_598_5, 0.005_829_524_690_465_940_5, 0.006_384_207_226_758_562, 0.006_993_343_275_374_75, 0.008_208_017_867_010_477, 0.008_446_232_827_556_69, 0.008_792_393_774_639_18, 0.009_176_887_491_225_397, 0.009_771_933_910_899_405, 0.010_554_487_222_486_426, 0.011_448_935_236_986_889, 1.669_418_147_147_517_6],
    [-0.000_247_712_394_012_777_9, 0.002_708_963_313_646_762, 0.003_832_603_188_131_386, 0.004_742_141_236_693_369, 0.005_346_788_525_161_484_5, 0.005_842_166_054_900_833, 0.006_411_225_654_999_943, 0.006_984_325_559_683_206, 0.007_674_300_021_320_122, 0.008_242_885_800_316_414, 0.008_671_889_893_976_355, 0.008_836_612_375_622_525, 0.009_272_948_041_086_35, 0.009_674_924_711_348_874, 0.010_450_013_294_365_868, 0.011_501_628_579_084_267, 1.669_418_147_147_517_6],
    [0.001_466_698_469_328_342_5, 0.002_340_815_271_743_846, 0.002_940_497_139_419_479_7, 0.003_747_470_118_005_444, 0.004_548_909_443_252_449, 0.005_245_081_980_830_323, 0.005_751_195_856_261_377, 0.006_032_811_698_527_75, 0.006_704_741_295_852_159, 0.007_214_510_992_996_534, 0.007_685_158_628_608_291, 0.008_225_447_979_991_04, 0.008_747_809_163_157_708, 0.009_104_455_351_808_339, 0.009_441_035_274_080_365, 0.010_363_047_526_614_19, 1.669_418_147_147_517_6],
    [0.001_466_698_469_328_342_5, 0.002_650_879_627_248_535_7, 0.003_631_796_042_513_990_2, 0.004_659_821_903_445_607, 0.005_340_027_856_789_974, 0.005_786_728_806_432_957, 0.006_075_201_422_879_347, 0.006_644_589_809_374_958, 0.007_189_385_271_251_980_5, 0.007_711_061_268_571_713_4, 0.008_239_808_295_438_168, 0.008_665_968_393_465_931, 0.009_063_209_850_213_815, 0.009_512_674_994_082_163, 0.010_374_691_944_546_433, 0.011_080_514_039_785_268, 1.649_392_263_170_850_2],
    [0.001_466_698_469_328_342_5, 0.004_719_657_928_468_03, 0.005_604_799_006_988_677, 0.006_038_102_508_824_331, 0.006_301_604_694_270_504_5, 0.006_993_539_549_072_798, 0.007_538_874_153_951_335, 0.008_099_399_789_383_788, 0.008_504_452_856_012_85, 0.008_891_844_822_080_837, 0.009_274_571_424_187_198, 0.009_958_357_406_976_499, 0.010_393_603_875_995_186, 0.010_953_739_344_705_529, 0.011_373_320_235_688_3, 0.011_806_504_005_139_164, 1.825_022_937_289_413_3],
    [0.002_951_425_010_812_691_6, 0.004_744_617_809_991_214, 0.005_476_573_119_765_308, 0.006_044_221_450_195_136, 0.006_239_193_264_360_711, 0.006_703_644_394_607_867, 0.007_146_164_560_246_197, 0.007_774_111_551_393_963, 0.008_383_436_311_141_48, 0.008_657_696_400_414_683, 0.009_009_935_326_865_602, 0.009_419_583_124_266_91, 0.010_027_591_765_922_102, 0.010_534_079_335_581_334, 0.011_136_433_501_737_833, 0.011_715_626_568_669_013, 1.825_022_937_289_413_3],
    [0.003_318_078_950_958_147_6, 0.004_273_032_976_489_315, 0.005_055_336_099_336_105, 0.005_697_938_297_633_724, 0.006_109_813_089_282_84, 0.006_418_404_348_728_047, 0.006_771_489_920_952_539, 0.007_124_008_991_919_915, 0.007_649_743_640_446_334, 0.008_261_086_314_555_072, 0.008_626_732_546_897_548, 0.008_971_862_945_544_628, 0.009_191_917_179_749_499, 0.009_606_472_187_464_35, 0.010_101_348_208_090_543, 0.010_955_912_500_855_222, 1.825_022_937_289_413_3],
    [0.003_318_078_950_958_147_6, 0.004_775_873_660_605_288, 0.005_790_557_786_028_05, 0.006_250_615_044_470_575, 0.006_763_150_507_116_802, 0.007_109_989_511_510_03, 0.007_516_079_784_232_554, 0.007_972_641_584_070_949, 0.008_452_988_948_870_17, 0.008_915_620_238_354_557, 0.009_160_612_584_093_35, 0.009_393_715_130_189_256, 0.009_820_114_749_101_489, 0.010_243_784_083_143_968, 0.010_831_515_574_730_556, 0.011_733_747_409_564_203, 1.825_022_937_289_413_3],
    [0.003_318_078_950_958_147_6, 0.005_592_167_173_193_752, 0.006_313_926_261_999_841, 0.006_870_915_227_990_804, 0.007_302_537_240_974_805, 0.007_705_227_344_703_612, 0.008_033_087_262_480_452, 0.008_522_900_101_737_825, 0.008_937_427_800_966_846, 0.009_190_259_897_739_453, 0.009_525_083_033_183_931, 0.009_953_189_939_658_812, 0.010_263_878_369_096_027, 0.010_781_964_910_613_517, 0.011_382_596_990_069_337, 0.012_023_918_214_209_71, 2.260_984_504_176_915],
    [0.004_119_262_021_643_921, 0.005_309_770_646_720_309, 0.005_688_598_346_973_954, 0.006_303_184_858_796_747, 0.006_837_791_930_403_839, 0.007_257_747_415_268_961, 0.007_524_223_530_480_503, 0.007_827_018_614_258_116, 0.008_141_774_690_715_036, 0.008_570_659_246_512_8, 0.008_941_184_163_410_325, 0.009_235_455_946_977_647, 0.009_657_207_460_978_935, 0.010_057_422_018_380_792, 0.010_465_431_856_420_952, 0.011_325_425_666_214_71, 2.260_984_504_176_915],
    [0.001_541_794_051_847_400_5, 0.005_072_476_251_624_673, 0.005_295_321_712_647_820_5, 0.005_534_968_687_474_334, 0.005_890_112_894_241_403_4, 0.006_443_586_497_686_927, 0.006_888_467_698_291_313, 0.007_229_392_324_895_217, 0.007_444_266_882_079_394, 0.007_692_357_395_712_863, 0.007_949_688_364_291_734, 0.008_310_453_668_580_794, 0.008_700_485_629_953_774, 0.009_147_319_738_028_119, 0.009_588_959_299_955_473, 0.010_043_414_133_121_131, 2.260_984_504_176_915],
    [-0.009_332_715_441_370_71, -0.001_557_456_541_425_597_7, 0.001_547_480_498_609_619_5, 0.002_915_000_037_777_196_4, 0.003_898_877_915_166_994, 0.004_585_518_286_972_32, 0.005_187_690_304_910_605_5, 0.005_485_609_816_857_149, 0.005_827_874_924_774_234, 0.006_368_931_095_906_528, 0.006_889_625_165_687_743, 0.007_305_754_423_071_406, 0.007_631_314_335_634_054_6, 0.008_009_813_031_991_62, 0.008_557_475_709_177_972, 0.009_327_006_424_901_056, 2.260_984_504_176_915],
    [-0.026_679_538_226_928_48, -0.012_820_355_302_448_815, -0.008_134_223_976_765_139, -0.005_526_715_172_083_747, -0.003_863_184_194_629_252_4, -0.002_238_786_808_859_99, -0.000_750_814_866_431_974_9, 0.000_846_678_991_695_974_5, 0.002_164_444_337_626_908_4, 0.002_942_786_947_288_321_8, 0.003_744_415_323_096_328, 0.004_420_025_668_570_759, 0.005_143_894_069_438_834, 0.005_796_690_333_897_359, 0.006_796_092_856_430_113_6, 0.007_844_083_554_218_142, 0.023_248_716_094_254_274],
    [-0.054_451_946_832_261_73, -0.020_359_989_719_349_592, -0.017_189_330_047_645_123, -0.014_549_656_500_299_267, -0.012_460_129_269_784_912, -0.010_375_212_618_719_255, -0.008_398_314_777_627_232, -0.006_889_613_574_374_153, -0.005_496_263_710_520_583, -0.004_413_293_241_320_636, -0.003_109_028_747_992_619_5, -0.001_775_477_378_606_786_5, -0.000_274_073_362_744_238_46, 0.001_772_432_161_641_441_3, 0.003_199_550_292_559_895_4, 0.004_762_084_309_299_737, 0.011_103_405_115_104_437],
    [-0.089_366_611_182_049_03, -0.025_879_594_437_157_753, -0.023_443_069_738_224_02, -0.020_918_211_809_193_29, -0.019_109_559_607_788_772, -0.017_700_885_702_954_2, -0.016_296_897_286_485_652, -0.014_780_263_981_705_967, -0.013_245_585_859_145_14, -0.011_814_287_506_500_763, -0.009_924_466_058_086_73, -0.008_201_121_572_263_584, -0.006_498_541_723_174_551, -0.005_164_326_196_923_58, -0.003_705_845_776_035_228_4, -0.001_736_667_437_824_828_4, 0.005_572_868_656_008_367],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_895_05, -0.087_048_558_793_213_47, -0.074_368_617_811_434_46, -0.048_362_272_907_490_136, -0.038_521_905_458_098_06, -0.031_437_658_801_262_904, -0.013_998_251_413_628_877, -0.008_152_181_858_794_894, 0.002_973_560_866_905_903, 0.006_726_506_231_464_101, 0.014_964_465_688_434_675, 0.021_272_254_177_966_923, 0.034_500_263_022_034_634, 0.048_766_812_068_680_98, 0.060_020_925_288_595_64, 0.080_342_509_064_801_84, 0.111_004_919_151_146_8],
    [-0.124_586_499_325_895_05, -0.085_322_636_023_008_12, -0.064_880_345_857_038_9, -0.051_120_913_300_807_216, -0.042_761_145_392_182_11, -0.033_814_849_358_779_736, -0.017_090_634_498_119_622, -0.011_755_298_617_766_882, 0.002_538_892_909_834_084, 0.013_856_964_298_819_018, 0.019_385_549_481_935_337, 0.027_996_371_734_340_064, 0.036_010_412_078_698_255, 0.052_729_625_202_691_63, 0.059_414_352_520_648_306, 0.079_680_711_175_887_17, 0.111_004_919_151_146_8],
    [-0.124_586_499_325_895_05, -0.082_747_271_408_374_69, -0.065_024_139_836_817_86, -0.057_609_731_614_704_73, -0.047_948_132_888_920_37, -0.039_347_733_124_643_916, -0.020_401_668_100_781_464, -0.010_497_570_067_549_268, 0.002_155_179_264_224_022, 0.015_492_820_043_365_805, 0.022_606_114_063_730_465, 0.031_586_837_244_242_62, 0.040_088_897_168_333_255, 0.051_457_728_413_026_89, 0.057_599_071_755_409_674, 0.078_284_361_022_205_79, 0.117_947_229_490_633_63],
    [-0.123_707_386_673_730_93, -0.084_356_279_717_965_82, -0.067_892_877_906_181_29, -0.059_995_619_282_329_625, -0.046_495_007_747_687_486, -0.039_374_698_524_121_94, -0.014_301_542_489_828_897, -0.006_535_474_030_698_877, 0.001_972_059_205_326_978, 0.015_845_961_573_909_363, 0.028_694_933_172_101_39, 0.040_050_810_880_800_834, 0.049_174_074_518_170_24, 0.053_600_570_014_004_42, 0.064_267_405_557_693_99, 0.087_377_300_513_668_49, 0.117_947_229_490_633_63],
    [-0.123_707_386_673_730_93, -0.086_275_803_661_361_52, -0.076_650_650_399_719_23, -0.062_038_008_781_773_05, -0.047_119_624_562_038_87, -0.039_372_902_918_041_54, -0.019_984_288_763_466_22, -0.003_140_381_819_062_984, 0.001_693_479_948_959_945_5, 0.013_861_967_971_856_274, 0.027_244_256_790_871_366, 0.048_000_728_383_138_014, 0.052_294_386_149_545_17, 0.064_700_460_903_938_52, 0.080_657_446_063_395_69, 0.103_533_540_700_826_72, 0.117_947_229_490_633_63],
    [-0.117_000_137_386_872_54, -0.082_875_937_424_470_47, -0.069_967_772_971_316_58, -0.056_522_236_432_067_094, -0.045_169_195_866_172_53, -0.033_551_378_828_752_476, -0.024_186_336_415_726_664, -0.011_017_056_823_427_556, 0.001_506_609_640_685_073_5, 0.007_550_618_240_099_02, 0.016_782_896_836_011_5, 0.028_731_141_037_363_49, 0.049_730_253_486_819_39, 0.064_754_659_209_094_05, 0.090_596_287_718_940_77, 0.111_381_979_445_150_45, 0.125_442_659_204_805_35],
    [-0.101_427_562_605_892_7, -0.069_830_983_906_861_81, -0.051_170_536_800_909_626, -0.041_516_582_845_884_935, -0.035_286_861_033_576_57, -0.025_550_188_841_155_31, -0.018_696_999_461_683_43, -0.010_465_989_271_570_69, -0.001_863_752_795_762_585_8, 0.004_507_636_339_510_342, 0.009_248_515_765_904_049, 0.019_763_213_496_086_6, 0.029_459_597_641_529_004, 0.041_788_053_011_367_66, 0.067_212_039_256_514_64, 0.110_303_740_241_100_35, 0.125_442_659_204_805_35],
    [-0.082_305_206_996_403_76, -0.048_235_079_461_758_13, -0.039_541_473_405_433_8, -0.033_036_382_383_033_09, -0.028_478_510_250_236_427, -0.018_769_241_269_789_964, -0.011_866_595_460_261_245, -0.007_827_199_247_496_702, -0.001_547_016_222_730_06, 0.002_314_810_727_793_573_3, 0.006_449_937_011_172_943, 0.009_021_891_348_520_002, 0.016_607_334_250_973_96, 0.026_956_477_279_831_248, 0.035_485_785_430_558_664, 0.070_693_851_662_308_99, 0.125_442_659_204_805_35],
    [-0.051_623_519_897_588_29, -0.038_033_215_298_189_17, -0.032_247_195_636_244_04, -0.026_612_997_827_594_784, -0.016_888_902_074_373_362, -0.012_163_170_359_360_653, -0.008_430_708_566_231_13, -0.004_103_340_083_393_476, 0.000_121_974_275_689_750_58, 0.003_644_563_262_791_477_4, 0.006_640_519_532_524_13, 0.008_916_426_043_170_111, 0.014_223_459_945_265_64, 0.026_091_474_960_896_383, 0.029_751_053_568_851_826, 0.041_600_800_294_900_26, 0.125_442_659_204_805_35],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_895_05, -0.080_164_689_380_887_97, -0.060_497_840_028_598_48, -0.046_600_434_333_704_7, -0.037_279_054_989_130_29, -0.025_043_402_186_129_982, -0.012_893_345_477_725_459, -0.006_025_138_013_616_811, 0.001_695_618_951_146_278_6, 0.006_582_721_264_923_233, 0.014_354_715_808_000_07, 0.025_582_155_080_346_124, 0.035_559_769_651_648_02, 0.049_710_396_993_590_29, 0.060_035_979_866_168_966, 0.091_436_848_511_729_66, 0.562_299_322_024_696_8],
    [-0.185_061_868_468_332_17, -0.095_341_433_304_852_15, -0.077_461_070_548_297_91, -0.064_421_545_686_930_68, -0.052_063_131_407_733_665, -0.042_924_109_735_569_12, -0.034_516_049_967_722_54, -0.027_400_780_362_465_02, -0.019_610_783_128_679_657, -0.011_783_648_971_153_617, -0.004_583_341_553_605_466_5, 0.004_189_385_222_286_523, 0.012_923_284_636_588_454, 0.024_340_371_548_091_35, 0.036_771_948_825_332_31, 0.052_852_904_459_375_42, 0.108_601_251_936_825_13],
    [-0.264_731_425_640_652_7, -0.081_228_899_773_668_47, -0.062_003_497_192_310_81, -0.048_778_759_769_894_89, -0.039_076_002_878_466_11, -0.030_596_843_379_522_547, -0.023_437_436_798_296_993, -0.015_725_277_719_883_41, -0.008_562_400_205_498_314, -0.002_324_463_566_677_118_6, 0.006_454_770_969_256_653, 0.014_045_821_244_293_339, 0.023_743_938_991_417_225, 0.033_360_000_364_574_26, 0.045_171_037_759_542_36, 0.057_294_159_288_819_826, 0.108_601_251_936_825_13],
    [-0.414_247_789_217_793_9, -0.069_284_805_005_160_85, -0.054_933_166_719_811_03, -0.045_040_993_251_083_794, -0.036_342_091_434_570_706, -0.029_247_003_392_088_328, -0.022_986_499_380_194_04, -0.016_969_927_900_579_11, -0.010_884_600_767_396_04, -0.005_189_809_524_645_583, 0.001_193_840_890_477_486_7, 0.007_688_342_562_105_2, 0.015_047_640_253_898_594, 0.022_699_228_601_413_945, 0.032_190_046_876_265_23, 0.046_318_023_886_626_11, 0.108_601_251_936_825_13],
    [-0.462_065_055_008_584_36, -0.064_058_074_095_920_1, -0.049_864_125_978_864_714, -0.039_511_377_697_101_81, -0.030_392_769_931_557_68, -0.022_951_007_999_699_773, -0.016_095_293_810_048_14, -0.009_767_879_348_731_422, -0.003_795_923_738_683_615_2, 0.002_412_027_524_268_351, 0.008_742_661_981_270_454, 0.015_452_799_290_996_862, 0.022_075_270_685_316_44, 0.029_683_707_491_131_17, 0.038_855_825_328_998_97, 0.050_449_367_350_295_39, 0.183_793_229_944_294_64],
    [-0.462_065_055_008_584_36, -0.056_487_606_826_067_54, -0.036_712_768_217_735_08, -0.023_450_948_153_525_754, -0.013_473_546_087_253_005, -0.005_252_131_791_135_897, 0.002_822_563_635_349_145_4, 0.009_970_475_844_905_298, 0.016_423_413_650_094_146, 0.022_511_167_328_236_003, 0.028_118_573_724_036_48, 0.033_737_408_457_965_7, 0.039_963_726_368_970_39, 0.045_925_274_111_334_916, 0.052_491_667_119_246_355, 0.058_560_165_383_946_7, 0.183_793_229_944_294_64],
    [-0.462_065_055_008_584_36, -0.083_165_163_964_718_3, -0.021_143_839_098_413_875, -0.004_209_627_807_917_705, 0.007_718_539_139_833_826, 0.015_014_382_321_945_251, 0.021_333_226_274_384_05, 0.026_440_919_096_668_597, 0.031_153_622_532_191_318, 0.035_637_566_443_850_15, 0.039_833_349_350_821_44, 0.043_752_162_422_134_98, 0.047_929_156_874_412_99, 0.051_887_731_921_777_56, 0.056_251_607_145_591_57, 0.060_144_857_455_725_23, 0.183_793_229_944_294_64],
    [-0.462_065_055_008_584_36, -0.338_063_911_027_094_77, -0.301_090_612_159_887_7, -0.116_136_784_120_384_25, -0.021_719_962_767_215_12, 8.497_027_668_356_089e-5, 0.011_969_240_576_376_938, 0.019_794_309_474_970_49, 0.025_765_740_568_997_702, 0.031_250_738_076_926_333, 0.035_898_036_829_951_79, 0.040_175_809_057_706_73, 0.044_092_171_816_050_17, 0.048_135_433_380_608_356, 0.052_321_985_942_390_41, 0.057_591_486_131_850_944, 0.183_793_229_944_294_64],
    [-0.427_584_866_113_864_04, -0.323_800_067_212_009_1, -0.291_456_959_222_561_04, -0.257_756_889_100_007_1, -0.192_697_514_148_298_1, -0.073_561_879_026_922_13, -0.010_564_881_090_047_364, 0.010_361_956_841_287_677, 0.021_347_279_337_401_014, 0.028_594_182_367_544_126, 0.034_865_733_535_430_05, 0.040_175_341_885_366_76, 0.045_177_150_338_539_58, 0.048_999_869_655_149_526, 0.054_032_735_675_640_12, 0.071_491_847_954_864_56, 0.266_283_565_521_134_5],
    [-0.427_584_866_113_864_04, -0.279_743_063_081_005_77, -0.235_857_427_058_388_33, -0.185_701_927_439_428_93, -0.120_832_616_033_343_03, -0.065_335_254_239_434_93, -0.002_489_459_413_341_834_6, 0.021_124_130_481_459_984, 0.033_452_327_963_061_74, 0.042_045_722_925_956_42, 0.048_578_443_279_906_96, 0.054_993_388_702_915_54, 0.072_127_457_458_489_13, 0.098_706_976_545_678_24, 0.130_618_449_436_851_89, 0.171_983_476_470_636_17, 0.327_062_053_883_389_2],
    [-0.413_217_587_584_000_7, -0.222_338_572_104_993, -0.129_662_833_279_837_4, -0.095_787_721_258_644_28, -0.074_081_498_100_527_29, -0.056_650_399_939_286_61, -0.039_834_842_387_040_84, -0.021_632_175_760_111_47, 0.005_783_348_876_603_803, 0.040_650_540_764_631_62, 0.064_858_139_057_715_06, 0.102_639_718_651_755_35, 0.139_856_890_272_140_38, 0.171_874_807_726_394_6, 0.203_678_511_629_346_05, 0.239_353_382_292_762_68, 0.339_092_924_200_334_4],
    [-0.413_217_587_584_000_7, -0.139_395_033_012_288_48, -0.098_490_096_875_212_08, -0.077_722_280_844_500_4, -0.062_603_418_474_796_06, -0.049_663_054_154_455_83, -0.039_141_544_663_476_864, -0.028_765_464_559_575_43, -0.019_517_337_352_440_132, -0.008_448_402_758_993_815, 0.011_759_579_154_173_033, 0.053_809_841_744_277_345, 0.119_983_240_997_662_74, 0.195_943_436_429_725_84, 0.234_268_026_298_406_93, 0.253_263_684_047_577_76, 0.339_092_924_200_334_4],
    [-0.355_365_004_930_881_5, -0.119_940_526_025_959_85, -0.086_164_853_876_859_18, -0.064_039_366_002_516_18, -0.049_090_784_456_334_385, -0.038_629_021_280_011_43, -0.029_377_079_602_657_157, -0.021_841_120_576_098_483, -0.014_553_223_841_303_16, -0.005_475_376_255_199_037, 0.011_385_863_219_442_332, 0.037_138_633_543_274_194, 0.073_107_633_296_490_38, 0.151_923_412_517_986_33, 0.233_928_958_503_801_38, 0.253_068_071_799_920_9, 0.339_092_924_200_334_4],
    [-0.337_444_965_102_302_26, -0.102_775_231_843_136_93, -0.070_584_282_254_958_91, -0.049_413_553_160_254_024, -0.036_466_068_272_655_72, -0.027_328_746_651_451_963, -0.019_659_892_263_381_613, -0.011_554_605_899_944_222, -0.001_946_299_472_529_255_2, 0.014_230_449_522_590_76, 0.033_215_160_671_587_314, 0.052_135_698_594_461_606, 0.077_893_027_929_968_95, 0.118_801_320_330_171_11, 0.215_919_780_903_051_75, 0.243_792_572_048_823_85, 0.339_092_924_200_334_4],
    [-0.316_865_281_526_402_53, -0.100_128_952_922_553_89, -0.067_750_905_255_153_79, -0.046_223_660_071_200_406, -0.030_914_259_093_614_553, -0.020_343_239_190_914_94, -0.008_957_915_973_552_956, 0.005_054_695_692_347_566, 0.022_168_552_952_751_414, 0.036_087_709_097_456_42, 0.050_069_785_810_192_655, 0.064_794_898_076_347_42, 0.085_882_041_106_937_16, 0.129_583_930_034_550_24, 0.195_244_314_112_802_5, 0.221_579_329_732_199_07, 0.326_556_747_662_560_04],
    [-0.316_865_281_526_402_53, -0.114_727_485_158_139_28, -0.089_117_256_333_313_29, -0.059_104_189_868_001_776, -0.034_699_121_656_260_545, -0.015_955_621_839_551_255, 0.002_185_314_103_137_592_5, 0.019_480_871_774_331_592, 0.031_169_127_146_029_45, 0.043_249_064_459_729_13, 0.055_556_172_889_844_36, 0.067_718_697_226_328_38, 0.091_616_374_520_644_54, 0.152_191_898_707_794_32, 0.178_166_721_060_676_6, 0.200_927_587_208_358_02, 0.286_801_353_253_997_7],
    [-0.245_928_352_305_683_16, -0.144_127_829_081_375_97, -0.123_688_802_865_654_31, -0.102_293_430_678_069_04, -0.073_418_935_286_243_8, -0.033_806_018_780_297_53, -0.002_678_604_828_786_151_5, 0.014_059_639_021_656_094, 0.028_717_155_684_656_13, 0.038_940_662_738_639_935, 0.050_020_156_044_166_65, 0.062_379_191_465_375_21, 0.083_418_499_991_787_3, 0.143_191_680_745_387_4, 0.163_603_346_718_487_56, 0.177_838_495_018_578_98, 0.252_143_280_021_500_3],
    [-0.245_928_352_305_683_16, -0.152_851_485_200_141_88, -0.143_894_189_497_340_4, -0.133_347_352_784_519_92, -0.122_326_812_715_849_9, -0.100_968_825_644_617_7, -0.072_920_560_691_917_84, -0.024_751_239_772_180_324, 0.008_189_953_374_264_647, 0.026_710_129_610_006_086, 0.039_802_366_093_410_924, 0.052_701_873_794_652_48, 0.064_676_181_709_470_47, 0.085_335_157_205_300_73, 0.132_674_922_180_324_92, 0.164_024_873_733_539_05, 0.208_054_196_286_607_04],
    [-0.640_045_289_841_312, -0.155_399_638_488_060_18, -0.120_455_742_364_567_98, -0.085_748_743_302_857_28, -0.061_113_103_349_701_75, -0.039_108_438_882_857_983, -0.018_514_710_391_285_757, 0.001_194_614_672_187_607_8, 0.016_803_707_641_210_395, 0.027_826_063_870_943_732, 0.037_665_912_285_516_03, 0.046_960_525_333_837_91, 0.057_299_865_271_569_04, 0.069_542_987_412_769_93, 0.085_906_883_212_766_64, 0.111_691_215_930_650_2, 0.512_771_116_283_206_9],
    [-0.863_535_728_077_828, -0.155_572_781_612_132_48, -0.119_130_532_308_237_59, -0.083_756_210_422_486_5, -0.056_090_403_914_392_434, -0.032_970_650_860_810_02, -0.009_112_472_840_032_118, 0.012_995_187_122_582_439, 0.030_030_918_372_275_135, 0.042_918_294_808_925_43, 0.055_285_626_123_635_43, 0.069_160_094_472_976_1, 0.086_332_243_656_694_87, 0.112_663_148_042_600_58, 0.163_535_112_644_522_45, 0.292_764_555_095_313_1, 0.720_383_106_435_218_6],
    [-0.863_535_728_077_828, -0.570_613_968_215_865_4, -0.330_342_093_336_799_1, -0.154_778_565_648_892_86, -0.092_888_029_905_129_11, -0.051_681_509_593_454_99, -0.018_465_995_139_681_25, 0.015_148_549_781_664_253, 0.046_299_172_687_247_74, 0.073_947_690_438_605_12, 0.108_899_789_860_053_04, 0.159_151_782_418_806_12, 0.220_752_026_867_100_46, 0.327_110_867_381_244_7, 0.492_271_685_680_896_6, 0.592_585_296_025_849_2, 0.792_559_711_537_951_2],
    [-0.863_535_728_077_828, -0.611_823_309_929_528_5, -0.493_764_589_382_150_4, -0.349_525_679_451_083_1, -0.228_257_899_184_517_5, -0.132_226_300_013_094_5, -0.068_962_943_540_901_86, -0.029_524_626_472_338_285, 0.009_517_198_269_045_759, 0.052_643_204_255_757_24, 0.098_809_082_170_517_62, 0.160_943_626_076_730_27, 0.239_673_960_141_327_22, 0.408_471_612_642_245_5, 0.572_560_500_659_794_2, 0.644_821_108_695_507_6, 0.827_320_705_764_097_2],
    [-0.863_535_728_077_828, -0.592_723_752_897_799_2, -0.494_326_467_811_144_87, -0.382_174_282_767_233_5, -0.293_888_262_780_058_2, -0.207_293_922_224_919_7, -0.128_938_756_504_924_67, -0.069_166_683_019_024_52, -0.025_928_532_647_791_953, 0.018_054_020_898_797_228, 0.063_233_329_635_776_2, 0.125_643_373_070_020_02, 0.204_773_723_011_199_84, 0.344_853_031_587_168_85, 0.595_306_305_504_201_7, 0.677_748_439_882_696_1, 0.875_746_433_378_275_7],
    [-0.745_069_607_129_417_6, -0.550_570_385_476_836_6, -0.462_631_583_422_244_64, -0.373_115_640_048_448_4, -0.302_068_210_504_412, -0.234_929_833_352_087_2, -0.163_641_615_181_720_35, -0.099_829_506_903_922_32, -0.046_598_203_685_583_13, 0.001_932_806_650_571_454_8, 0.050_313_916_942_358_36, 0.109_897_691_494_476_81, 0.180_695_622_976_741_52, 0.298_075_897_223_343_9, 0.609_099_273_856_846_5, 0.709_676_610_600_470_4, 0.875_746_433_378_275_7],
    [-0.656_018_232_828_953_1, -0.506_898_660_011_358_4, -0.420_031_096_274_974_6, -0.349_731_830_140_379, -0.282_477_180_363_348_46, -0.223_689_535_483_046_2, -0.150_226_771_204_400_2, -0.092_064_527_312_610_1, -0.042_674_219_260_099_35, 0.013_371_797_909_682_081, 0.059_781_511_367_815_01, 0.112_955_761_954_923_54, 0.166_911_853_266_452_16, 0.246_466_610_743_435_04, 0.407_982_865_128_404_87, 0.738_251_028_066_361_2, 0.901_543_180_877_834_7],
    [-0.613_956_036_010_537_4, -0.520_169_353_283_263_1, -0.437_552_048_948_749_9, -0.363_501_834_365_636_5, -0.276_838_028_005_622_5, -0.223_720_020_281_944_57, -0.167_904_359_655_205_86, -0.110_237_307_181_043_81, -0.062_841_343_348_469_89, -0.009_508_166_452_260_736, 0.048_798_053_485_580_146, 0.112_530_459_621_255_14, 0.193_077_069_295_455_46, 0.309_579_810_255_424_3, 0.635_877_396_726_517_9, 0.707_903_060_485_985_8, 0.901_543_180_877_834_7],
    [-0.902_759_087_348_114_5, -0.560_892_608_180_223, -0.480_423_241_379_786_7, -0.415_387_382_338_272_27, -0.338_511_017_568_713_9, -0.261_419_427_681_330_75, -0.198_200_094_163_776_74, -0.132_278_851_981_423_83, -0.074_999_684_516_827_73, -0.007_194_908_335_905_692, 0.088_513_277_533_598_79, 0.217_424_199_161_258_85, 0.377_010_533_886_974_9, 0.553_164_440_066_339_9, 0.630_999_042_605_022_7, 0.682_458_547_371_505_1, 0.901_543_180_877_834_7],
    [-0.921_032_960_375_931_6, -0.710_396_541_113_688_3, -0.593_262_892_391_561_6, -0.519_390_761_037_609_2, -0.441_027_011_604_761_4, -0.363_689_888_354_323_64, -0.273_948_988_458_219, -0.176_855_358_648_610_04, -0.079_436_337_794_012_49, 0.027_790_592_654_851_56, 0.101_495_230_894_087_74, 0.161_740_974_623_303_56, 0.262_809_060_089_121_3, 0.390_564_023_329_679_93, 0.540_665_196_625_377_8, 0.635_785_376_733_740_1, 0.901_543_180_877_834_7],
    [-0.921_032_960_375_931_6, -0.803_454_674_661_655_5, -0.708_206_307_031_159_6, -0.631_617_910_139_886_9, -0.546_613_958_305_781_2, -0.450_477_342_694_471_1, -0.333_776_059_972_861, -0.173_207_110_669_524_2, -0.013_402_145_355_087_75, 0.037_217_796_722_279_02, 0.069_074_687_438_559_93, 0.095_828_071_853_458_3, 0.122_297_172_659_399_43, 0.156_031_796_942_502_9, 0.238_412_116_638_560_66, 0.418_730_219_353_731_17, 0.783_814_148_462_811],
    [-0.921_032_960_375_931_6, -0.803_270_253_774_586_8, -0.717_390_587_058_848_5, -0.645_954_516_858_512_8, -0.541_136_092_642_979_9, -0.410_203_906_631_339_1, -0.165_833_685_615_549_93, -0.025_338_555_311_779_115, 0.009_188_995_800_130_757, 0.032_980_223_606_605_43, 0.052_131_738_802_522_594, 0.069_334_626_784_240_86, 0.086_434_884_548_777_46, 0.103_955_954_834_611_29, 0.125_474_612_921_885_55, 0.159_257_117_530_966_27, 0.698_499_808_231_315],
    [-0.921_032_960_375_931_6, -0.760_209_243_333_197_1, -0.686_951_732_078_789_8, -0.566_604_086_475_761_1, -0.370_571_848_395_693_96, -0.124_629_552_828_296_76, -0.038_014_508_193_178_86, -0.008_004_743_113_947_324, 0.013_195_239_800_347_048, 0.030_773_273_164_105_875, 0.044_842_016_161_247_04, 0.059_841_681_589_851_224, 0.073_290_647_865_647_43, 0.087_544_807_534_736_35, 0.103_726_840_120_977_45, 0.125_484_929_507_068_52, 0.246_890_236_984_564_14],
    [-0.866_723_738_298_79, -0.720_904_970_109_705_8, -0.608_982_381_508_590_4, -0.377_587_944_750_784_7, -0.161_476_265_262_765_48, -0.063_006_349_228_397_5, -0.026_633_835_276_032_036, -0.004_869_601_288_187_781, 0.012_656_817_221_180_871, 0.029_424_293_457_123_277, 0.041_959_474_840_044_81, 0.055_470_292_058_668_04, 0.068_667_346_422_324_36, 0.082_578_762_967_563_75, 0.096_690_999_910_142_13, 0.118_303_785_448_950_08, 0.258_872_350_590_696_74],
    [-0.746_101_706_575_289_6, -0.211_105_646_555_174_41, -0.144_539_332_531_776_92, -0.116_672_709_825_400_72, -0.097_213_984_499_948_34, -0.074_538_774_634_134_55, -0.054_821_748_695_619_78, -0.043_723_798_411_972_03, -0.029_522_709_485_196_39, -0.008_464_500_912_745_898, 0.005_202_701_389_419_839, 0.025_234_495_273_906_053, 0.048_805_056_220_843_27, 0.073_166_032_049_897_8, 0.103_387_555_538_205_67, 0.137_040_416_357_046_4, 0.258_872_350_590_696_74],
    [-0.573_064_249_609_944, -0.134_552_167_262_001_75, -0.111_577_272_124_861_72, -0.092_522_652_811_610_4, -0.069_220_222_808_615_05, -0.051_765_000_628_952_736, -0.038_320_983_551_284_18, -0.024_188_553_795_579_607, -0.006_846_457_885_407_579, 0.005_413_172_668_307_84, 0.020_013_727_512_380_53, 0.037_100_925_052_825_855, 0.055_411_556_840_750_83, 0.073_785_407_424_053_6, 0.099_217_527_638_855_11, 0.127_249_547_977_094_53, 0.265_559_831_599_769_15],
    [-0.272_737_160_029_733_3, -0.117_107_992_226_197_36, -0.099_354_736_696_568_32, -0.073_459_814_006_307_73, -0.054_505_837_368_745_74, -0.036_716_279_241_997_01, -0.024_265_509_527_020_85, -0.008_564_767_803_389_994, 0.003_231_050_152_572_824_7, 0.016_580_450_753_404_787, 0.028_109_831_858_553_962, 0.043_568_131_240_544_435, 0.058_582_457_317_446_796, 0.074_204_597_616_508_12, 0.098_031_682_453_505_42, 0.125_811_863_356_341_6, 0.471_819_048_506_015_2],
    [-0.404_774_177_088_979_67, -0.132_946_991_530_273_37, -0.090_232_631_186_165_38, -0.059_967_889_123_476_26, -0.039_890_725_483_740_78, -0.025_104_806_459_902_256, -0.012_250_098_915_275_218, -0.000_696_252_872_038_624_5, 0.012_369_794_163_842_924, 0.021_748_810_609_858_744, 0.031_528_076_225_811_295, 0.040_472_069_593_743_49, 0.053_573_399_900_725_65, 0.070_437_028_876_73, 0.093_725_060_703_130_98, 0.112_218_349_174_820_86, 0.157_492_463_512_626_9],
    [-0.489_465_223_298_859_44, -0.188_661_788_285_946_32, -0.132_642_444_528_331_74, -0.097_152_988_237_512_5, -0.071_475_720_965_307, -0.048_726_881_230_950_814, -0.031_359_198_212_644_136, -0.013_937_974_592_222_898, 0.000_488_275_465_472_255_2, 0.014_773_887_951_300_73, 0.026_981_837_706_039_593, 0.040_089_614_920_480_646, 0.052_664_689_738_376_09, 0.066_339_884_578_328_18, 0.082_455_897_771_882_23, 0.113_830_103_240_241_23, 0.173_711_046_433_837_17],
    [-0.678_931_284_007_967_8, -0.248_104_154_092_029_57, -0.171_308_472_850_363_96, -0.127_221_939_103_250_58, -0.094_154_537_974_880_99, -0.069_123_008_559_006_04, -0.045_595_458_989_256_72, -0.023_469_089_771_292_542, -0.003_127_375_595_535_274_7, 0.014_422_656_215_342_004, 0.031_573_228_220_215_28, 0.046_565_915_955_807_32, 0.060_608_355_801_974_524, 0.074_817_295_958_120_33, 0.086_770_544_957_306_04, 0.123_337_481_756_638_18, 0.184_604_377_696_540_92],
    [-0.717_653_413_340_672_2, -0.311_701_166_834_856_27, -0.211_331_358_200_916_34, -0.154_355_730_684_276_6, -0.112_421_445_075_144_29, -0.080_030_501_775_312_18, -0.052_570_215_094_990_64, -0.027_824_099_045_539_997, -0.000_852_818_835_709_776_1, 0.020_409_136_200_025_716, 0.039_006_956_093_660_165, 0.055_871_511_266_599_2, 0.071_406_191_081_906_64, 0.083_587_766_901_631_6, 0.099_003_050_769_785_69, 0.137_088_404_779_028_2, 0.195_957_723_721_793_54],
    [-0.816_299_347_080_256_9, -0.348_691_466_790_211_07, -0.250_867_050_215_136_8, -0.177_296_096_092_636_16, -0.123_949_341_559_295_36, -0.085_603_688_919_758_64, -0.052_824_982_230_695_75, -0.027_164_454_850_801_72, 0.002_957_717_346_484_519, 0.025_508_580_628_754_142, 0.044_420_801_769_789_11, 0.062_895_555_247_293_08, 0.079_677_645_644_367_4, 0.093_117_782_517_526_95, 0.107_890_852_213_623_56, 0.145_999_640_171_979_17, 0.210_816_039_103_013_6],
    [-0.940_576_521_312_193_5, -0.388_935_250_762_131_04, -0.276_459_731_409_220_2, -0.193_400_559_301_936_92, -0.138_269_084_398_034_04, -0.094_534_527_193_365_53, -0.056_304_468_932_422_53, -0.027_883_743_402_735_868, 0.001_578_490_338_528_357_1, 0.026_935_302_896_532_832, 0.047_526_711_092_918_235, 0.067_225_338_610_442_64, 0.084_750_314_539_058_57, 0.099_324_374_135_317_83, 0.113_999_300_671_474_79, 0.145_759_217_514_621_06, 0.221_688_894_261_403_04],
    [-1.062_817_565_887_517_2, -0.436_227_523_547_182_8, -0.305_263_414_484_514_4, -0.213_782_957_133_288_54, -0.152_240_472_283_206_56, -0.105_221_299_204_696_86, -0.064_718_733_667_102_01, -0.029_715_769_351_313_43, 0.000_440_311_476_194_154_24, 0.028_592_184_437_346_88, 0.052_005_619_139_276_274, 0.072_498_985_392_269_89, 0.091_478_657_479_771_28, 0.107_739_525_523_369_11, 0.123_462_509_005_544_62, 0.149_484_930_190_010_3, 0.233_887_187_374_532_96],
    [-1.113_664_398_911_437_2, -0.475_793_058_300_127_64, -0.333_852_421_711_506_64, -0.242_529_396_227_099_38, -0.169_171_503_703_232_1, -0.117_773_230_547_223_91, -0.071_009_798_982_398_01, -0.033_420_024_959_152_556, 6.508_240_786_313_21e-5, 0.031_445_223_838_326_326, 0.056_443_860_428_793_69, 0.078_334_207_378_455_91, 0.098_589_440_477_857_05, 0.117_414_178_257_080_59, 0.133_370_346_898_015_44, 0.154_778_340_554_831_8, 0.242_459_566_290_593_3],
    [-1.199_419_919_031_022_6, -0.497_832_842_734_332_3, -0.354_120_450_116_298_7, -0.263_990_148_166_958_15, -0.187_720_037_257_097_65, -0.127_517_015_825_988_15, -0.077_854_770_121_746_37, -0.037_865_833_696_785_825, -0.002_604_852_394_633_595_7, 0.031_036_689_462_938_936, 0.057_762_996_196_884_714, 0.081_642_983_192_062_67, 0.104_195_586_159_017_39, 0.122_676_849_416_677_63, 0.139_217_524_730_634_65, 0.157_646_749_286_794_8, 0.244_066_327_776_133_46],
    [-1.307_809_752_822_638_7, -0.506_357_379_514_275_3, -0.357_079_026_728_596_65, -0.266_234_003_302_894_7, -0.192_984_609_883_332_3, -0.130_743_390_887_193_7, -0.080_576_815_758_138_4, -0.039_652_603_049_891_29, -0.003_518_692_422_278_058_4, 0.030_422_339_313_598_87, 0.059_344_532_370_087, 0.084_814_109_152_258_2, 0.108_283_009_600_742_32, 0.126_635_133_857_885_66, 0.143_503_636_924_137_3, 0.160_723_788_384_666_3, 0.244_066_327_776_133_46],
    [-1.436_712_236_537_596_6, -0.510_091_880_760_887_2, -0.349_975_526_263_040_6, -0.259_474_429_076_651_64, -0.185_231_762_962_321_27, -0.126_231_907_725_27, -0.077_894_920_271_829_45, -0.037_326_509_549_052_92, -0.002_285_344_854_734_067, 0.029_975_386_947_902_1, 0.060_167_340_338_126_08, 0.085_282_602_820_223_64, 0.108_729_098_287_568_3, 0.127_837_561_030_263_9, 0.145_119_840_800_238_07, 0.162_137_670_770_969_2, 0.244_066_327_776_133_46],
    [-1.519_160_898_975_426_8, -0.493_318_989_618_010_63, -0.339_435_591_711_103_4, -0.246_620_608_505_163_76, -0.173_588_685_094_388_2, -0.119_718_434_397_362_61, -0.074_959_330_091_913_41, -0.035_278_495_387_141_32, -0.000_857_918_898_679_115_9, 0.029_795_311_963_647_767, 0.056_496_034_275_057_45, 0.081_356_326_806_548_97, 0.104_097_998_175_723_04, 0.123_545_423_485_565_33, 0.141_423_133_332_071_93, 0.161_223_388_602_113_72, 0.244_066_327_776_133_46],
    [-1.519_160_898_975_426_8, -0.465_514_482_800_499_7, -0.321_479_578_762_940_03, -0.227_033_123_994_158_8, -0.160_146_916_597_262_46, -0.111_169_997_139_049_26, -0.068_250_063_725_975_5, -0.031_951_256_912_473_09, 0.001_257_600_521_944_958, 0.032_104_091_768_995_5, 0.055_636_810_549_884_57, 0.078_339_798_113_790_05, 0.099_405_000_724_262_05, 0.118_662_136_998_079_19, 0.134_279_980_378_902_8, 0.159_979_322_508_392_78, 0.243_623_215_849_494_2],
    [-1.851_174_186_066_328_9, -0.441_728_842_141_481_95, -0.305_550_803_566_032, -0.218_001_298_232_840_98, -0.151_883_471_531_848_92, -0.104_983_778_409_684_65, -0.063_583_783_437_737_03, -0.027_784_683_848_418_933, 0.003_335_438_614_427_932, 0.033_392_175_701_137_22, 0.056_055_394_115_432_16, 0.078_359_385_542_718_57, 0.098_499_982_369_255_06, 0.114_989_696_274_907_93, 0.128_577_277_673_184_14, 0.159_533_704_375_950_7, 0.233_354_794_020_021_57],
    [-1.851_174_186_066_328_9, -0.443_543_471_986_195_6, -0.306_088_152_383_194_3, -0.214_133_843_692_641_1, -0.148_147_321_290_230_6, -0.101_220_961_209_588_33, -0.062_147_574_040_818_69, -0.027_095_205_278_581_815, 0.001_580_902_643_127_136_5, 0.030_596_800_266_920_802, 0.053_866_937_696_431_326, 0.075_231_426_503_529_28, 0.095_304_159_148_835_75, 0.111_147_390_760_690_41, 0.124_756_427_624_288_5, 0.156_266_286_822_950_8, 0.225_646_316_904_535_74],
    [-1.851_174_186_066_328_9, -0.447_754_211_678_401_2, -0.304_970_488_729_509_2, -0.208_029_908_074_105_65, -0.147_736_422_768_577_62, -0.099_524_416_570_637_62, -0.058_986_993_132_893_07, -0.026_914_372_379_578_583, -0.000_466_711_466_767_660_7, 0.026_146_722_165_293_436, 0.049_132_388_050_704_266, 0.069_104_956_366_073_68, 0.088_501_391_455_132_13, 0.103_882_913_006_088_23, 0.117_905_927_956_864_93, 0.147_753_278_727_228_68, 0.214_813_546_281_259_7],
    [-1.851_174_186_066_328_9, -0.419_943_223_729_874_9, -0.264_704_571_842_282_35, -0.185_693_153_699_614_3, -0.128_747_427_225_074_82, -0.079_406_060_627_570_12, -0.045_713_684_822_997_76, -0.019_933_133_345_082_853, 0.005_462_798_587_945_807, 0.029_178_937_726_101_625, 0.050_829_904_744_526_77, 0.065_971_802_162_738_06, 0.085_124_521_190_100_68, 0.098_503_564_211_382_37, 0.112_207_024_965_445_78, 0.130_521_801_881_335_25, 0.209_644_599_768_379_8],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_29: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.500_119_986_938_366, 10.0),
    (-12.165_500_939_540_205, -9.619_712_842_621_347),
    (-12.796_177_267_575_76, -10.125_587_608_492_339),
    (-12.884_547_191_599_681, -9.361_087_481_593_842),
    (-0.181_540_247_888_528_63, 1.408_097_724_855_432_3),
    (-0.291_488_901_491_261_94, 0.281_164_958_820_116_94),
    (1.647_537_087_812_366_7, 2.701_447_054_508_368_4),
    (0.354_477_009_926_082_9, 1.392_532_853_548_757_6),
    (-2.165_020_391_639_158, 0.055_437_148_568_989_27),
    (0.062_699_781_801_213_58, 0.846_450_041_134_491_2),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_29: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.118_509_092_103_519_5, 1.114_761_017_663_422, 1.090_145_923_599_326_5, 1.051_306_494_417_931, 1.010_355_871_746_886, 0.633_014_841_554_321_3, 0.625_681_037_160_974_2, 0.986_382_153_392_992, 0.806_267_350_655_852_4, 0.621_119_454_697_435_5, 0.582_045_901_467_825_4, 0.572_777_510_505_312_9, 0.572_513_203_113_305_9, 0.576_031_877_305_889_2, 0.575_274_732_062_552_6, 1.049_541_809_950_715_2, 1.050_018_180_477_788_8],
    [0.225_776_365_579_176_6, 0.206_375_601_154_299_81, 0.186_064_863_777_886_75, 0.161_373_659_945_874_94, 0.135_589_868_032_679_28, 0.108_413_830_535_260_66, 0.089_872_722_031_314_76, 0.102_939_543_019_540_64, 0.513_449_461_737_117_4, 0.467_786_535_294_108_8, 0.330_484_864_732_826_6, 0.263_089_392_510_404_2, 0.271_492_913_691_989_14, 0.308_424_114_338_266_16, 0.366_452_978_806_329_8, 1.040_804_983_193_066, 1.065_714_253_890_298_8],
    [0.252_448_913_484_234_87, 0.235_521_139_482_789_22, 0.209_905_036_234_318_42, 0.176_832_592_062_150_1, 0.139_614_641_316_313_7, 0.104_023_775_158_282_61, 0.069_594_414_962_660_21, 0.030_351_389_546_930_972, -0.008_467_524_809_223_371, -0.039_831_529_360_759_73, -0.078_615_658_567_338_74, -0.113_460_615_678_848_52, -0.148_107_741_417_022_74, -0.175_763_620_338_672_2, -0.211_482_541_674_388_15, -0.256_547_343_739_451_87, -0.291_497_488_660_539_1],
    [2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.196_341_216_618_717, 1.946_833_865_057_706_2, 1.931_889_774_746_453_8, 1.975_295_353_626_568_7, 2.039_038_539_925_477, 2.092_289_835_713_160_5],
    [-1.290_031_723_366_974_7, -1.419_777_788_001_388_2, -1.537_267_805_765_534_2, -1.618_322_423_041_915, -1.632_849_136_145_768_9, -0.810_051_113_615_869_8, -0.685_560_808_574_564_3, -0.639_990_370_036_259_2, -0.608_174_079_605_855_9, -0.576_120_526_201_756_7, -0.548_871_121_653_822_1, -0.501_112_274_053_087_9, -0.003_302_503_925_102_202_8, 0.013_625_966_377_394_21, 0.028_411_977_997_693_332, 0.036_376_343_345_723_24, -0.484_360_341_243_027_57],
    [-1.611_852_982_398_538_4, -1.649_091_439_165_757_4, -1.689_593_477_648_851_3, -1.727_882_958_316_754_8, -1.758_834_142_543_077_4, -1.796_676_045_548_839_5, -1.835_610_723_641_158_3, -1.871_927_494_917_804, -1.905_246_869_869_773, -1.941_894_563_996_794_2, -1.981_585_254_284_951, -2.015_310_333_335_729_6, -2.051_133_864_279_816, -2.087_649_672_333_279, -2.119_731_848_705_025, -2.144_868_881_926_853_5, -2.163_082_998_219_638_7],
    [-0.596_108_799_753_416_4, -0.596_108_799_753_416_4, -0.597_581_038_254_729_6, -0.606_649_677_122_446_9, -0.618_808_558_486_542_7, -0.625_080_083_250_130_5, -0.645_016_245_032_328_9, -0.672_109_836_374_504, -0.700_775_818_160_529_3, -0.704_106_937_062_799_6, -0.703_176_204_975_496_5, -0.696_781_197_424_017_6, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1],
    [0.138_529_516_364_454_37, 0.176_938_128_056_034_45, 0.283_400_665_278_053_3, 0.330_268_468_628_932_94, 0.379_557_328_290_293_7, 0.446_619_346_387_358_6, 0.482_892_801_943_497_15, 0.526_639_338_170_773_4, 0.578_945_047_362_877_6, 0.505_614_621_278_085_9, 0.476_229_186_228_091_85, 0.501_677_031_206_850_7, 0.553_675_171_059_727_7, 0.614_232_538_214_275, 0.677_008_845_866_704_4, 0.720_904_784_012_547, 0.726_322_024_771_350_1],
    [0.288_245_518_442_738_77, 0.241_551_434_438_825_49, -0.138_697_580_673_391_58, -0.345_493_757_612_862_6, -0.430_924_450_323_499_85, -0.499_737_632_590_354_7, -0.551_793_343_032_854_9, -0.539_809_721_622_865_6, -0.449_821_170_457_011_2, -0.336_057_609_181_810_43, 0.146_053_078_598_571_56, 0.228_361_633_495_709_37, 0.246_626_720_103_407_42, 0.252_952_111_394_112_5, 0.108_874_587_885_642_46, 0.138_045_520_933_083_23, 0.148_936_690_320_286_8],
    [9.957_873_996_022_524, 9.934_757_156_622_99, 9.913_740_039_501_961, 9.902_398_736_805_894, 9.892_154_106_675_022, 9.879_552_313_196_047, 9.866_655_987_872_262, 9.855_209_330_364_083, 9.842_969_655_149_242, 9.834_662_796_913_012, 9.835_140_623_376_008, 9.840_664_071_539_013, 9.850_903_067_972_387, 9.862_513_488_087_654, 9.868_612_159_090_903, 9.870_292_120_444_882, 9.879_535_066_677_853],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_29: [(f64, f64); 11] = [
    (5.419_185_498_579_54, 10.0),
    (0.320_048_993_757_405_53, 1.38),
    (-0.278_456_376_545_300_03, 1.971_095_891_507_528_3),
    (-0.783_839_570_029_425_4, 0.359_480_989_581_144_33),
    (1.506_939_585_167_018_6, 6.442_488_747_737_999_5),
    (-1.908_825_021_689_898, 0.209_409_169_898_635),
    (-2.264_699_308_176_646, 0.243_432_298_064_600_95),
    (-0.769_398_248_555_516_1, -0.125_262_481_778_891_35),
    (-0.046_532_352_103_877_81, 0.909_956_226_855_928_4),
    (-1.096_848_077_448_504_3, 0.620_755_738_826_302),
    (8.017_791_846_134_891, 10.079_132_695_443_889),
];

/// The slow accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_DONOR_KINDS_29: [f64; 3] = [
    0.891_543_473_320_574,
    0.107_644_680_560_873_58,
    0.000_811_846_118_552_335_3,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_29: [f64; 7] = [
    1.070_856_747_113_518_4e-7,
    7.627_409_745_087_261_4e-6,
    1.446_858_847_466_834_3e-6,
    1.754_200_077_983_565e-6,
    2.697_742_482_791_967_5e-6,
    2.704_518_401_900_469_6e-6,
    4.624_367_645_404_432e-7,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_29: [f64; 5] = [
    0.0,
    0.0,
    0.108_115_617_631_121_76,
    0.873_313_311_938_809_6,
    0.018_571_070_430_068_778,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_29: [f64; 7] = [
    3.347_466_321_155_178e-7,
    6.932_180_585_923_057e-5,
    0.000_109_976_709_155_302_63,
    9.344_774_662_138_364e-5,
    8.291_920_177_050_407e-5,
    8.388_570_026_843_173e-5,
    0.000_123_076_318_033_680_84,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_29: [f64; 5] = [
    0.0,
    0.0,
    0.379_249_260_084_120_1,
    0.609_713_611_661_880_2,
    0.011_037_128_253_999_703,
];
// @end-table binary_awd

// @begin-table binary_xrb
// The X-ray binaries' sampler and share row refitted against plan 11's engine at generator
// version 29 on the generator's own pairs (plan 15, P15.T10.b), held until 29.
//
// @generated by hyperion-fit 0.1.0, task `binary_xrb` revision 0. Do not edit.
// inputs-sha256: 10bd856ea7f7acebe8112f6d5d4f45bbb904ec87ba352c6678af3ed66490b7f5
// manifest: crates/hyperion-fit/manifests/binary_xrb.toml
// data: none
// sim-fingerprint: 5a071611475533bb2b16ef5be3305f0f52fb6d60273009b86acf833166d133fe
// since-generator-version: 28
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the total against the brainstorm's 10⁴, the black-hole transients pinned at
//   Corral-Santana et al.'s (2016, A&A 587, A61) observed 1,300
// acceptance: slab members by kind 385/9707/4/91/7/0/991/4/0; young members by kind
//   544/399/107/168/874/4/123/0/4; census kinds
//   0.0365/0.8952/0.0007/0.0067/0.0008/0.0000/0.0598/0.0002/0.0000; weights
//   0.0320/0.7841/0.0006/0.1300/0.0524/0.0002 and 0.0007/0.0000/0.0000; black-hole transients
//   1300 of 1e4 (the census's 67); high-mass hosts 7; donor mass's own nodes 14; layers C 0.000
//   D 0.185 E 0.815; rates a system 1.56e-4/2.46e-4/1.65e-4/1.31e-4/1.95e-4/1.35e-4/3.08e-5;
//   scratch marks: NeutronStarRecurrence, BlackHoleRecurrence, SupergiantPeriod,
//   SupergiantDonorMass, SupergiantRate, SupergiantRecoil, SupergiantSinceSupernova,
//   UltracompactRecurrence, SymbioticPeriod, SymbioticDonorMass, SymbioticFilling,
//   BlackHoleHighMassAccretorMass, BlackHoleHighMassPeriod, BlackHoleHighMassDonorMass,
//   BlackHoleHighMassRate; the K–S tests against the other half of the census are the sim's slow
//   `fitted_tables::the_x_ray_binary_sampler_against_the_engine`
//
// Read by `XRB_SAMPLER_29` and `CLASS_SHARES_29`, before the block. The marks are fitted to the
// engine's members of the nine kinds `XrbHostKind` holds at generator version 29's rules
// (hyperion-fit's `tasks::binary_classes::xrb`), each to the kinds the class draws it for: the
// low-mass and history marks to the solar circle's slab, the high-mass marks to the young thin
// disc's own census of 3000000 records, each unconditional but the low-mass donor's mass, which
// follows the period with a law at each node, from the members under the node's triangle where
// their Kish size is 50 or more. A mark with under 50 members keeps `XRB_SAMPLER`'s law. The
// kinds' weights are the galaxy's shares of its hosts of each kind, the black-hole transients'
// pinned at `BLACK_HOLE_TRANSIENT_SHARE` and the rest keeping their ratios; each mark's members
// are weighted so that their kinds are in those shares. The row counts every member of the class and is
// scaled to 1e4 hosts at Milky Way parameters (plan 11's design note 11).

/// The X-ray binaries' marks' residual quantiles at the ranks k ÷ 16 over all their members, in
/// [`XrbMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const XRB_RESIDUALS_29: [[f64; 17]; 30] = [
    [1.147_638_965_107_692_8, 1.265_013_003_929_796_4, 1.279_446_188_640_969, 1.290_990_055_397_372_3, 1.301_174_163_747_494_5, 1.310_192_356_004_936_4, 1.319_259_966_249_130_2, 1.329_285_372_308_207_9, 1.340_123_700_187_141_3, 1.354_453_596_036_227, 1.372_978_457_331_046_6, 1.392_589_372_790_518_8, 1.489_281_422_791_451_3, 1.554_418_430_130_143, 1.607_771_348_544_110_5, 1.677_581_022_857_299, 1.975_628_261_510_142_4],
    [2.106_510_635_585_874, 2.474_966_656_616_427_7, 2.746_226_297_877_986_3, 2.903_705_752_060_261, 3.026_388_171_891_752_2, 3.074_675_433_332_101_4, 3.205_816_439_497_368_3, 3.297_787_924_962_521_3, 3.440_549_839_558_064_3, 3.533_988_517_837_649, 3.727_694_190_172_553_3, 3.907_616_711_229_456_6, 4.207_298_260_913_865, 4.699_325_451_073_171, 5.346_134_533_537_876, 6.038_213_753_070_96, 8.787_813_035_339_518],
    [-0.372_612_994_664_013_3, 0.041_838_153_670_280_72, 0.134_785_723_508_261_1, 0.182_204_380_297_424_1, 0.214_070_123_321_179_47, 0.239_209_185_282_579_4, 0.317_310_289_179_891_24, 0.384_528_750_346_127_3, 0.440_993_508_898_495_3, 0.488_435_777_055_584_6, 0.541_824_805_218_044_4, 0.637_159_851_612_859_8, 1.007_181_668_721_185_4, 1.208_749_808_251_908, 1.443_125_234_163_384, 1.646_403_014_166_133_4, 3.355_474_783_371_506],
    [-0.830_383_227_408_568, -0.458_494_008_535_731_2, -0.294_154_846_825_211_2, -0.167_900_781_211_954_92, -0.075_314_696_645_297_37, -0.037_700_963_803_324_904, -0.007_758_621_229_510_415, 0.011_597_231_506_256_007, 0.028_039_425_894_425_343, 0.047_284_708_367_659_654, 0.068_756_018_450_345_93, 0.088_952_759_162_523_54, 0.115_600_146_614_418_88, 0.142_840_581_384_793_97, 0.191_943_159_437_616_58, 0.328_408_164_490_533_3, 0.718_986_689_212_949],
    [-5.596_358_743_396_394, -2.266_809_300_841_913, -2.086_603_343_561_619_4, -1.880_038_463_807_428, -1.695_710_463_277_186, -1.550_645_501_280_554_2, -1.435_426_097_983_837, -1.331_654_598_043_950_5, -1.233_716_215_108_083_2, -1.134_693_214_184_477_7, -1.037_368_135_501_051_6, -0.946_247_612_818_210_3, -0.856_470_010_541_668_3, -0.776_137_847_322_946_2, -0.666_979_226_665_886_4, -0.370_976_938_664_910_7, 1.454_811_353_166_347_1],
    [1.75, 2.254_32, 2.435_993_5, 2.563_591, 2.667_624_5, 2.758_895_5, 2.842_726, 2.922_329_5, 3.0, 3.077_670_5, 3.157_274, 3.241_104_5, 3.332_375_5, 3.436_409, 3.564_006_5, 3.745_68, 4.25],
    [1.25, 1.754_319_999_999_999_9, 1.935_993_5, 2.063_591, 2.167_624_5, 2.258_895_5, 2.342_726, 2.422_329_5, 2.5, 2.577_670_5, 2.657_274, 2.741_104_5, 2.832_375_5, 2.936_409, 3.064_006_5, 3.245_68, 3.75],
    [0.355_095_299_505_206_4, 1.224_394_065_855_718_6, 1.410_449_544_102_363_7, 1.567_470_255_427_445_3, 1.660_466_111_021_045, 1.737_957_577_467_870_8, 1.813_809_494_487_485_9, 1.884_784_410_737_844_2, 1.938_184_737_447_966_4, 2.000_122_520_345_962, 2.046_159_900_111_420_3, 2.112_634_385_209_328, 2.187_278_893_443_442, 2.259_201_596_826_994, 2.361_221_699_025_148, 2.506_893_303_003_255_6, 2.984_609_187_822_059_5],
    [0.005_594_880_237_021_038, 0.023_810_962_892_669_988, 0.031_753_814_145_039_71, 0.039_870_863_175_128_335, 0.050_041_568_158_941_445, 0.057_906_087_244_822_61, 0.065_906_273_296_543_76, 0.074_815_904_234_129_43, 0.085_506_695_161_258_83, 0.096_560_833_304_166_93, 0.107_211_626_139_984_3, 0.117_081_179_537_633_8, 0.130_724_745_464_533_55, 0.150_619_237_584_367_18, 0.169_331_447_841_785_4, 0.205_317_852_982_541_96, 0.700_909_782_619_88],
    [8.006_095_233_936_879, 8.340_719_109_060_089, 8.751_177_350_879_157, 9.084_776_660_770_812, 9.383_352_895_825_58, 9.629_300_607_535_445, 9.831_498_352_717_357, 10.099_450_946_255_649, 10.390_837_530_986_408, 10.689_787_692_223_14, 10.997_640_113_684_815, 11.377_610_996_367_025, 11.682_340_427_694_53, 12.115_550_991_323_447, 12.865_645_810_314_149, 13.701_891_708_249_22, 21.068_498_767_664_43],
    [-0.150_000_000_000_000_02, 0.152_592, 0.261_596_1, 0.338_154_599_999_999_97, 0.400_574_699_999_999_95, 0.455_337_299_999_999_94, 0.505_635_6, 0.553_397_7, 0.6, 0.646_602_3, 0.694_364_4, 0.744_662_7, 0.799_425_3, 0.861_845_4, 0.938_403_9, 1.047_408, 1.35],
    [10.0, 14.034_559_999_999_999, 15.487_948, 16.508_728, 17.340_996, 18.071_164, 18.741_808, 19.378_636, 20.0, 20.621_364, 21.258_192, 21.928_836, 22.659_004, 23.491_272, 24.512_052, 25.965_44, 30.0],
    [-10.25, -9.745_68, -9.564_006_5, -9.436_409, -9.332_375_5, -9.241_104_5, -9.157_274, -9.077_670_5, -9.0, -8.922_329_5, -8.842_726, -8.758_895_5, -8.667_624_5, -8.563_591, -8.435_993_5, -8.254_32, -7.75],
    [0.494_476_138_824_081_7, 1.365_631_850_258_909_5, 1.799_404_737_798_460_3, 2.213_322_023_539_68, 2.612_946_838_160_016, 3.012_025_638_885_759_4, 3.520_694_675_921_299, 3.923_715_779_371_759_6, 4.355_252_294_595_691, 4.944_802_704_127_86, 5.444_970_985_056_782_5, 6.080_732_467_150_769, 6.673_462_238_939_827, 7.482_218_748_842_096, 8.495_384_723_673_308, 11.075_712_796_165_256, 26.700_840_877_730_062],
    [0.0, 21.541_471_172_88, 27.883_959_356_160_005, 32.722_835_660_352_004, 36.900_838_535_952_005, 40.732_034_499_504, 44.381_465_974_608_005, 47.957_508_026_832, 51.545_915_694_144_01, 55.228_255_120_512, 59.095_911_217_440_005, 63.268_150_174_896, 67.923_921_544_560_02, 73.375_080_106_944_01, 80.282_901_424_464, 90.636_775_035_600_01, 120.071_697_528_624_01],
    [4.174_421_988_060_552, 6.164_169_652_569_123, 6.409_412_463_613_848, 6.565_853_053_528_664, 6.655_441_249_789_554, 6.747_463_088_053_393, 6.828_646_772_791_342, 6.884_380_301_983_972, 6.946_116_736_259_897, 6.991_067_371_115_394, 7.040_811_568_462_426, 7.079_192_516_918_915, 7.125_047_296_144_938, 7.180_946_882_939_013, 7.228_172_608_041_603_6, 7.307_502_867_033_945, 7.477_784_022_709_876],
    [5.7, 5.901_728, 5.974_397_4, 6.025_436_4, 6.067_049_8, 6.103_558_2, 6.137_090_4, 6.168_931_8, 6.2, 6.231_068_2, 6.262_909_6, 6.296_441_8, 6.332_950_2, 6.374_563_6, 6.425_602_6, 6.498_272, 6.7],
    [0.575_937_324_311_977_3, 0.821_720_124_855_051_6, 0.877_787_068_700_429, 0.902_493_191_300_152_9, 0.922_210_116_231_958_3, 0.933_448_434_895_555_6, 0.947_796_317_019_488_4, 0.963_954_540_039_400_1, 0.979_225_915_346_859_5, 0.993_822_028_785_616_3, 1.007_790_980_671_389, 1.020_374_041_377_174_2, 1.031_365_209_900_598_8, 1.043_216_118_654_124_2, 1.057_235_329_383_993_1, 1.075_819_098_321_011_4, 1.446_931_504_220_512_6],
    [1.037_076_384_580_339, 1.049_625_743_761_265_7, 1.064_693_595_026_591_2, 1.070_691_571_976_975_6, 1.076_709_699_107_96, 1.081_272_549_707_585_5, 1.087_278_521_183_535, 1.092_738_865_947_327_8, 1.099_630_909_887_002_3, 1.108_115_572_143_026, 1.113_018_387_078_685, 1.127_434_256_232_003_7, 1.195_076_781_005_984_8, 1.249_117_640_092_114, 1.327_716_246_946_173, 1.364_903_011_273_039_1, 1.629_310_277_259_491_8],
    [-0.001_813_727_417_228_904_4, 0.286_901_493_795_972_46, 0.447_306_507_171_988_7, 0.660_526_636_907_114_4, 0.952_598_163_500_868_5, 1.113_899_882_121_896_2, 1.214_145_366_561_435_7, 1.296_445_840_030_431_7, 1.376_223_558_228_855_8, 1.455_620_922_044_350_6, 1.551_269_459_362_049, 1.676_145_161_335_071_3, 2.087_051_067_003_583_3, 2.269_474_261_533_620_3, 2.370_701_740_990_936_6, 2.461_559_446_856_482_8, 2.802_935_133_072_123_7],
    [1.282_993_821_330_733_3, 1.578_040_850_414_360_2, 1.662_454_457_720_069, 1.712_893_885_288_879_3, 1.743_213_967_075_660_1, 1.764_697_829_180_391_4, 1.786_204_569_249_513_7, 1.805_153_552_787_215_3, 1.818_157_822_413_308_6, 1.829_977_946_668_91, 1.842_056_365_669_074, 1.854_282_458_284_942_6, 1.862_651_124_890_286_4, 1.871_968_753_840_678_8, 1.880_034_008_424_106, 1.888_256_664_715_448_6, 1.909_120_072_658_062_2],
    [0.0, 0.403_455_999_999_999_9, 0.548_794_8, 0.650_872_799_999_999_9, 0.734_099_6, 0.807_116_4, 0.874_180_8, 0.937_863_6, 1.0, 1.062_136_4, 1.125_819_2, 1.192_883_6, 1.265_900_4, 1.349_127_2, 1.451_205_2, 1.596_544_000_000_000_2, 2.0],
    [0.092_834_707_932_504_24, 0.166_307_838_359_314_18, 0.197_990_896_975_957_31, 0.226_233_832_619_249_95, 0.246_702_769_661_313_77, 0.265_055_010_863_703_87, 0.279_454_174_086_709_53, 0.296_367_271_807_226_5, 0.309_462_742_213_112_3, 0.321_210_539_700_778_3, 0.333_857_928_870_481_6, 0.346_476_587_928_926_66, 0.364_789_995_218_139_4, 0.389_178_621_052_604_2, 0.418_068_991_235_720_56, 0.463_641_339_529_154_9, 0.836_266_432_148_279],
    [2.375, 2.627_16, 2.717_996_75, 2.781_795_5, 2.833_812_25, 2.879_447_75, 2.921_363, 2.961_164_75, 3.0, 3.038_835_25, 3.078_637, 3.120_552_25, 3.166_187_75, 3.218_204_5, 3.282_003_25, 3.372_84, 3.625],
    [0.001_090_999_999_999_980_9, 0.071_695_799_999_999_99, 0.097_130_089_999_999_99, 0.114_993_739_999_999_98, 0.129_558_43, 0.142_336_37, 0.154_072_639_999_999_98, 0.165_217_13, 0.176_091, 0.186_964_87, 0.198_109_36, 0.209_845_63, 0.222_623_57, 0.237_188_26, 0.255_051_91, 0.280_486_2, 0.351_091_000_000_000_04],
    [0.150_000_000_000_000_02, 0.271_036_8, 0.314_638_44, 0.345_261_840_000_000_04, 0.370_229_88, 0.392_134_92, 0.412_254_24, 0.431_359_08, 0.45, 0.468_640_92, 0.487_745_760_000_000_03, 0.507_865_08, 0.529_770_12, 0.554_738_16, 0.585_361_56, 0.628_963_200_000_000_1, 0.75],
    [15.7, 17.919_007_999_999_998, 18.718_371_4, 19.279_800_4, 19.737_547_799_999_998, 20.139_140_2, 20.507_994_399_999_998, 20.858_249_8, 21.2, 21.541_750_2, 21.892_005_6, 22.260_859_8, 22.662_452_2, 23.120_199_6, 23.681_628_6, 24.480_992, 26.7],
    [0.498_187_999_999_999_96, 0.599_051_999_999_999_9, 0.635_386_7, 0.660_906_2, 0.681_712_9, 0.699_967_1, 0.716_733_2, 0.732_653_9, 0.748_188, 0.763_722_1, 0.779_642_8, 0.796_408_9, 0.814_663_1, 0.835_469_799_999_999_9, 0.860_989_3, 0.897_324, 0.998_188],
    [22.1, 29.563_935_999_999_998, 32.252_703_8, 34.141_146_8, 35.680_842_6, 37.031_653_4, 38.272_344_8, 39.450_476_6, 40.6, 41.749_523_4, 42.927_655_200_000_004, 44.168_346_6, 45.519_157_400_000_005, 47.058_853_2, 48.947_296_200_000_004, 51.636_064_000_000_005, 59.1],
    [-9.5, -9.096_544, -8.951_205_2, -8.849_127_2, -8.765_900_4, -8.692_883_6, -8.625_819_2, -8.562_136_4, -8.5, -8.437_863_6, -8.374_180_8, -8.307_116_4, -8.234_099_6, -8.150_872_8, -8.048_794_8, -7.903_456, -7.5],
];

/// The low-mass donor's mass's residual laws at its median's 17 nodes, each row its node's
/// quantiles at the ranks k ÷ 16, log₁₀ M☉.
#[rustfmt::skip]
pub const XRB_DONOR_MASS_NODE_RESIDUALS_29: [[f64; 17]; 17] = [
    [-0.064_792_103_912_876_04, 0.015_980_462_516_465_996, 0.039_670_278_676_604_734, 0.060_280_218_507_412_12, 0.077_124_922_157_692_54, 0.089_458_216_361_295_08, 0.097_692_733_380_526_45, 0.105_163_425_878_723_45, 0.112_578_625_789_694_88, 0.121_490_281_994_378_83, 0.132_740_688_852_775_5, 0.203_575_694_780_408_05, 0.319_453_360_193_542, 0.404_062_902_362_760_6, 0.494_913_144_365_317_97, 0.603_351_745_513_811, 0.703_999_982_865_565_1],
    [-0.674_550_466_144_921_1, -0.478_593_529_598_137_45, -0.374_361_334_403_291_9, -0.272_213_038_986_355_57, -0.163_868_188_136_622_7, -0.050_709_001_146_929_07, -0.016_626_672_933_505_244, 0.003_039_412_693_768_653, 0.024_725_455_824_502_56, 0.050_731_126_103_499_01, 0.073_352_595_342_143_51, 0.096_677_118_680_874_64, 0.115_936_009_664_290_49, 0.172_457_997_651_931_4, 0.428_601_116_523_660_4, 0.599_261_034_359_443_9, 0.718_986_689_212_949],
    [-0.674_550_466_144_921_1, -0.526_300_351_049_997_1, -0.447_570_629_965_127_4, -0.376_450_524_641_041_84, -0.295_356_356_733_636_4, -0.204_248_514_774_367_28, -0.116_479_733_275_323_43, -0.038_073_564_104_368, -0.015_291_441_926_203_526, 0.003_239_453_847_052_453_6, 0.027_021_931_892_607_584, 0.057_316_118_287_749_9, 0.089_658_709_810_994_1, 0.128_878_592_879_097_9, 0.247_066_317_502_737_66, 0.461_712_429_334_307_3, 0.718_986_689_212_949],
    [-0.674_550_466_144_921_1, -0.518_587_656_016_324_5, -0.422_471_248_849_229_26, -0.311_678_609_276_394_6, -0.182_330_839_681_810_55, -0.061_498_989_853_513_14, -0.026_583_192_962_883_376, -0.005_301_880_572_659_516, 0.018_657_558_439_445_46, 0.042_064_557_797_652_48, 0.068_496_537_519_146_94, 0.089_559_653_420_429_8, 0.115_347_076_911_977_29, 0.145_228_688_036_124_3, 0.182_526_103_426_029_96, 0.286_251_499_444_555_9, 0.718_986_689_212_949],
    [-0.674_550_466_144_921_1, -0.352_284_249_931_007_27, -0.129_831_975_156_405_35, -0.050_235_473_203_443_52, -0.022_653_032_872_648_63, -0.000_405_152_213_909_403_63, 0.022_895_763_937_350_842, 0.042_610_845_848_419_58, 0.062_031_754_008_739_406, 0.076_881_721_959_090_6, 0.094_541_273_657_622_2, 0.111_005_180_562_115_25, 0.128_930_396_601_409_84, 0.147_942_490_095_758_44, 0.170_467_217_294_379_58, 0.202_943_208_368_433_02, 0.718_986_689_212_949],
    [-0.494_522_119_504_122_95, -0.169_341_346_079_330_52, -0.074_342_493_894_794_68, -0.035_057_287_842_005_784, -0.009_609_435_890_660_453, 0.014_571_314_624_712_322, 0.035_767_504_183_516_92, 0.053_958_720_912_262_64, 0.070_868_181_222_458_57, 0.084_924_836_236_757_02, 0.102_446_553_290_441_26, 0.118_252_481_608_495_68, 0.133_959_082_624_651_37, 0.151_508_118_263_021_13, 0.175_884_380_012_095_3, 0.215_420_200_190_539_6, 0.471_666_476_484_926_66],
    [-0.591_440_540_786_834_1, -0.188_527_527_002_317_2, -0.145_230_166_559_560_25, -0.085_948_940_815_806_41, -0.035_283_250_920_621_435, -0.002_095_229_518_112_86, 0.025_519_066_628_725_82, 0.044_573_521_167_908_58, 0.063_081_547_577_213_82, 0.082_675_383_358_845_96, 0.105_052_897_101_998_86, 0.127_431_794_637_790_95, 0.150_811_611_638_322_82, 0.184_698_809_071_675_66, 0.249_810_949_786_020_64, 0.329_606_834_258_738_27, 0.471_666_476_484_926_66],
    [-0.689_450_624_135_336_8, -0.185_506_322_931_199_8, -0.116_071_424_403_361_6, -0.078_886_440_446_148_34, -0.058_130_605_217_515_466, -0.037_976_289_910_568_756, -0.007_184_396_499_155_289, 0.011_998_176_899_779_781, 0.022_924_359_816_135_6, 0.032_140_883_484_692_98, 0.042_967_553_966_441_45, 0.054_153_460_482_567_36, 0.073_328_513_590_833_45, 0.100_436_193_218_045_95, 0.165_642_650_112_176_58, 0.244_941_042_067_161_36, 0.471_666_476_484_926_66],
    [-0.740_974_200_051_840_2, -0.216_402_778_940_670_84, -0.114_145_225_582_197_72, -0.079_181_606_281_494_43, -0.061_732_296_912_667_184, -0.044_437_382_014_352_63, -0.008_649_276_876_198_264, 0.008_243_395_945_589_345, 0.014_931_726_715_274_436, 0.021_615_584_999_604_136, 0.029_672_721_008_789_957, 0.036_663_717_758_937_1, 0.045_248_987_016_008_005, 0.057_983_778_803_774_644, 0.078_149_496_352_364_05, 0.124_422_638_013_213_02, 0.430_266_868_289_765_07],
    [-0.830_383_227_408_568, -0.344_393_903_294_005_27, -0.157_598_878_150_039_87, -0.083_317_532_882_867_15, -0.057_923_924_011_774_326, -0.004_306_126_297_812_865, 0.006_526_958_323_623_241, 0.011_893_870_044_166_564, 0.015_044_994_475_865_284, 0.020_349_529_155_558_98, 0.030_139_360_596_487_544, 0.037_503_635_736_809_6, 0.047_301_260_634_588_21, 0.064_509_226_088_473_43, 0.085_172_594_807_792_73, 0.136_235_890_240_311_2, 0.328_624_496_086_787_9],
    [-0.830_383_227_408_568, -0.475_652_002_451_132_3, -0.229_323_417_850_482_7, -0.132_906_578_158_961_98, -0.048_570_973_537_685_1, 0.002_085_019_204_922_441, 0.009_692_126_813_709_753, 0.012_652_377_328_104_585, 0.018_375_020_167_158_144, 0.030_138_327_184_818_046, 0.038_155_952_28, 0.052_381_603_821_279_57, 0.072_526_315_655_973_99, 0.081_516_086_495_381_42, 0.131_989_319_010_051_6, 0.200_412_456_202_645_75, 0.550_235_605_794_934],
    [-0.830_383_227_408_568, -0.584_624_931_120_800_6, -0.426_256_310_910_537_8, -0.244_416_570_019_590_75, -0.157_168_895_055_709_63, -0.115_429_469_917_191_45, 0.000_243_954_526_527_951_23, 0.010_254_896_682_196_648, 0.018_707_497_522_786_41, 0.037_295_813_005_986_195, 0.052_646_962_245_920_12, 0.071_430_578_442_765_42, 0.078_967_449_746_661_76, 0.120_393_131_561_386_55, 0.182_265_474_432_956_9, 0.302_253_207_804_988_8, 0.550_235_605_794_934],
    [-0.830_383_227_408_568, -0.568_908_400_169_423_7, -0.464_713_349_139_342_3, -0.374_157_272_936_692_9, -0.258_087_403_024_698_46, -0.205_664_484_866_422_55, -0.147_260_188_764_675_9, -0.117_182_260_673_436_47, -0.078_905_605_697_017_89, -0.002_687_600_777_818_223_5, 0.036_609_418_267_575_07, 0.068_985_089_957_620_3, 0.102_174_998_504_949_15, 0.175_874_287_444_202_94, 0.274_687_412_173_241_9, 0.362_478_586_764_637_73, 0.550_235_605_794_934],
    [-0.712_608_842_045_877_6, -0.465_440_567_060_130_4, -0.350_554_927_922_377_63, -0.258_332_183_669_572_17, -0.217_899_825_413_490_43, -0.188_757_233_436_834_57, -0.139_936_087_143_629_68, -0.098_436_721_887_533_23, -0.082_634_262_263_739_95, -0.046_626_076_171_091_45, -0.030_646_510_446_492_036, 0.021_974_377_477_112_284, 0.092_241_095_771_127_1, 0.200_476_947_220_451_84, 0.336_345_388_309_248_1, 0.496_930_305_126_962_8, 0.550_235_605_794_934],
    [-0.830_383_227_408_568, -0.458_494_008_535_731_2, -0.294_154_846_825_211_2, -0.167_900_781_211_954_92, -0.075_314_696_645_297_37, -0.037_700_963_803_324_904, -0.007_758_621_229_510_415, 0.011_597_231_506_256_007, 0.028_039_425_894_425_343, 0.047_284_708_367_659_654, 0.068_756_018_450_345_93, 0.088_952_759_162_523_54, 0.115_600_146_614_418_88, 0.142_840_581_384_793_97, 0.191_943_159_437_616_58, 0.328_408_164_490_533_3, 0.718_986_689_212_949],
    [-0.830_383_227_408_568, -0.458_494_008_535_731_2, -0.294_154_846_825_211_2, -0.167_900_781_211_954_92, -0.075_314_696_645_297_37, -0.037_700_963_803_324_904, -0.007_758_621_229_510_415, 0.011_597_231_506_256_007, 0.028_039_425_894_425_343, 0.047_284_708_367_659_654, 0.068_756_018_450_345_93, 0.088_952_759_162_523_54, 0.115_600_146_614_418_88, 0.142_840_581_384_793_97, 0.191_943_159_437_616_58, 0.328_408_164_490_533_3, 0.718_986_689_212_949],
    [-0.830_383_227_408_568, -0.458_494_008_535_731_2, -0.294_154_846_825_211_2, -0.167_900_781_211_954_92, -0.075_314_696_645_297_37, -0.037_700_963_803_324_904, -0.007_758_621_229_510_415, 0.011_597_231_506_256_007, 0.028_039_425_894_425_343, 0.047_284_708_367_659_654, 0.068_756_018_450_345_93, 0.088_952_759_162_523_54, 0.115_600_146_614_418_88, 0.142_840_581_384_793_97, 0.191_943_159_437_616_58, 0.328_408_164_490_533_3, 0.718_986_689_212_949],
];

/// The X-ray binaries' marks' medians' spans of their conditions, first and last node, in
/// [`XrbMark`]'s order; only the low-mass donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIAN_SPANS_29: [(f64, f64); 30] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (-0.181_258_134_893_243_55, 2.468_921_556_899_302),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
];

/// The X-ray binaries' marks' medians at their 17 nodes, in [`XrbMark`]'s order; only the low-mass
/// donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIANS_29: [[f64; 17]; 30] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-1.447_002_974_944_649_1, -1.627_952_985_904_699_3, -1.669_697_470_105_776, -0.898_986_128_153_270_2, -0.714_804_504_460_929_4, -0.621_235_092_429_963_2, -0.502_062_893_465_277_2, -0.021_002_059_584_344_052, 0.052_063_294_817_594_45, 0.128_243_958_472_409_3, 0.192_978_984_007_761_23, 0.204_698_555_453_161_68, -0.026_599_865_459_155_922, -0.339_803_968_201_348_3, -0.469_873_826_278_093_4, -0.472_552_001_834_108_5, -0.515_189_039_357_603_7],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
];

/// The X-ray binaries' marks' least and greatest values, in [`XrbMark`]'s order.
#[rustfmt::skip]
#[allow(clippy::approx_constant, reason = "fitted values that may land near a constant are data")]
pub const XRB_LIMITS_29: [(f64, f64); 30] = [
    (1.147_638_965_107_692_8, 1.975_628_261_510_142_4),
    (2.106_510_635_585_874, 8.787_813_035_339_518),
    (-0.372_612_994_664_013_3, 3.355_474_783_371_506),
    (-1.847_982_958_708_832_8, 0.596_484_119_817_057_1),
    (-5.596_358_743_396_394, 1.454_811_353_166_347_1),
    (1.75, 4.25),
    (1.25, 3.75),
    (0.355_095_299_505_206_4, 2.984_609_187_822_059_5),
    (0.005_594_880_237_021_038, 0.700_909_782_619_88),
    (8.006_095_233_936_879, 21.068_498_767_664_43),
    (0.1, 1.2),
    (10.0, 35.0),
    (-10.25, -7.75),
    (0.494_476_138_824_081_7, 26.700_840_877_730_062),
    (0.0, 120.1),
    (4.174_421_988_060_552, 7.477_784_022_709_876),
    (5.5, 6.7),
    (0.575_937_324_311_977_3, 1.446_931_504_220_512_6),
    (1.037_076_384_580_339, 1.629_310_277_259_491_8),
    (-0.001_813_727_417_228_904_4, 2.802_935_133_072_123_7),
    (1.282_993_821_330_733_3, 1.909_120_072_658_062_2),
    (0.0, 2.0),
    (0.092_834_707_932_504_24, 0.836_266_432_148_279),
    (2.176_091, 3.698_97),
    (0.0, 0.477_121),
    (0.15, 0.75),
    (15.0, 30.0),
    (0.301_029_995_663_981_2, 1.0),
    (20.0, 60.0),
    (-9.5, -7.5),
];

/// The low-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::LOW_MASS`]' order: the black-hole transients' pinned at
/// [`BLACK_HOLE_TRANSIENT_SHARE`].
#[rustfmt::skip]
pub const XRB_LOW_MASS_KINDS_29: [f64; 6] = [
    0.031_963_502_649_812_74,
    0.784_113_627_570_619_4,
    0.000_615_101_181_527_035_3,
    0.13,
    0.052_371_072_539_426_85,
    0.000_214_802_132_302_956_2,
];

/// The high-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.
#[rustfmt::skip]
pub const XRB_HIGH_MASS_KINDS_29: [f64; 3] = [
    0.000_715_916_363_116_250_1,
    2.988_781_597_444_539_5e-6,
    2.988_781_597_444_539_5e-6,
];

/// The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const XRB_HOSTS_PER_FORMED_MASS_29: [f64; 7] = [
    8.220_474_860_023_064e-8,
    1.294_599_929_581_590_8e-7,
    8.690_724_698_949_116e-8,
    6.907_475_694_698_502e-8,
    1.030_594_091_909_633_2e-7,
    7.092_667_680_688_02e-8,
    1.625_673_950_249_354_5e-8,
];

/// The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const XRB_LAYER_SHARES_29: [f64; 5] = [
    0.0,
    0.0,
    0.0,
    0.185_381_316_639_707_1,
    0.814_618_683_360_292_9,
];
// @end-table binary_xrb

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;

    /// P11.T6: the scratch yield is 1 in every delay bin and for both channels, and a delay takes
    /// the bin of `DELAY_EDGES` that holds it, the ends held.
    #[test]
    fn the_scratch_ia_yield_is_one_everywhere() {
        for bin in 0..IA_DELAY_BINS {
            for channel in [IaPoolChannel::Merger, IaPoolChannel::Accretion] {
                assert_eq!(IA_YIELD.eta_in(bin, channel), 1.0);
            }
        }
        assert_eq!(IA_DELAY_BINS, 24);
        for (bin, w) in DELAY_EDGES.windows(2).enumerate() {
            assert_eq!(ia_delay_bin(Years::new(w[0])), bin);
            assert_eq!(ia_delay_bin(Years::new(f64::midpoint(w[0], w[1]))), bin);
        }
        assert_eq!(ia_delay_bin(Years::new(1.0e6)), 0);
        assert_eq!(ia_delay_bin(Years::new(1.0e11)), IA_DELAY_BINS - 1);
        assert_eq!(ia_delay_bin(Years::new(f64::NAN)), 0);
    }

    /// P11.T6: a yield outside [0, 1] is refused.
    #[test]
    #[should_panic(expected = "a yield is a share, in [0, 1]")]
    fn a_yield_that_is_not_a_share_is_refused() {
        let mut eta = [[0.5; 2]; IA_DELAY_BINS];
        eta[3][1] = 1.5;
        let _refused = IaYieldTable::new(eta);
    }

    /// The Milky Way fixture's formed mass: its components' born systems, in their fixed order,
    /// times the mass formed per system.
    fn milky_way_formed_mass() -> f64 {
        let galaxy = Galaxy::from_params(Seed::new(0x0b17_0007), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds");
        let systems = galaxy
            .fields()
            .components()
            .iter()
            .fold(0.0, |sum, c| sum + c.count());
        systems * galaxy.mean_formed_mass().value()
    }

    /// The scratch rates, the census galaxy's and the merger classes' own, are scaled to the
    /// fixture's formed mass, so that it holds the brainstorm's counts, and each class's layer
    /// shares sum to 1.
    #[test]
    fn the_scratch_rates_give_the_milky_way_counts() {
        let formed = milky_way_formed_mass();
        assert!(
            (formed / MILKY_WAY_FORMED_MASS - 1.0).abs() < 1e-9,
            "the fixture forms {formed:e} M_sun, the table is scaled to {MILKY_WAY_FORMED_MASS:e}"
        );
        let expected = [4.5e6, 4.5e6, 1.0e4, 9.0e4, 10.0];
        for (row, count) in SCRATCH_CLASS_SHARES.rows().iter().zip(expected) {
            let hosts = row.hosts_per_formed_mass(Population::YoungThinDisc) * formed;
            assert!(
                (hosts / count - 1.0).abs() < 1e-9,
                "{:?}: {hosts}",
                row.class()
            );
            let layers = MassBand::ALL
                .iter()
                .fold(0.0, |sum, &band| sum + row.layer_share(band));
            assert!((layers - 1.0).abs() < 1e-12, "{:?}: {layers}", row.class());
            assert!(POPULATIONS.iter().all(|&p| {
                row.hosts_per_formed_mass(p)
                    .total_cmp(&row.hosts_per_formed_mass(Population::Halo))
                    .is_eq()
            }));
        }
        assert_eq!(
            SCRATCH_CLASS_SHARES.row(CarvedClass::XrayBinary).class(),
            CarvedClass::XrayBinary
        );
    }

    /// The stellar mergers' table follows its mark list, each residual is non-decreasing inside
    /// its limits, and its layer weights are the share row's.
    #[test]
    fn the_merger_table_follows_its_mark_list_and_the_share_row() {
        let names = [
            "age_rank",
            "primary_rank",
            "contact_mass_ratio",
            "evolved_primary_rank",
            "evolved_mass_ratio",
            "dynamical_primary_mass",
            "dynamical_mass_ratio",
            "gap_mass_ratio",
            "helium_burning_mass_ratio",
            "agb_mass_ratio",
        ];
        assert_eq!(MERGER_SAMPLER.marks().len(), MergerMark::ALL.len());
        for (mark, name) in MergerMark::ALL.into_iter().zip(names) {
            let law = MERGER_SAMPLER.mark(mark);
            assert_eq!(law.name(), name);
            assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{name}");
            let (lo, hi) = law.limits();
            assert!(
                law.residual().iter().all(|r| (lo..=hi).contains(r)),
                "{name}"
            );
        }
        let channels = MERGER_SAMPLER.pick(MergerPick::EngineChannel).weights();
        assert_eq!(channels.len(), MergerChannel::ALL.len());
        assert!((channels.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let row = CLASS_SHARES.row(CarvedClass::StellarMerger);
        let weights = MERGER_SAMPLER.pick(MergerPick::Layer).weights();
        assert_eq!(weights.len(), MassBand::ALL.len());
        for (band, &weight) in MassBand::ALL.iter().zip(weights) {
            assert!((weight - row.layer_share(*band)).abs() < 1e-15, "{band:?}");
        }
    }

    /// The neutron-star mergers' table follows its mark list, each residual is non-decreasing, its
    /// masses are inside plan 06's neutron stars' and the black holes', and its kinds' weights sum
    /// to 1.
    #[test]
    fn the_nsm_table_follows_its_mark_list() {
        use crate::stellar::remnant::collapse::{MAX_NEUTRON_STAR_MASS, MIN_NEUTRON_STAR_MASS};
        let names = [
            "neutron_star_mass",
            "log_black_hole_mass",
            "neutron_star_progenitor_rank",
            "black_hole_progenitor_rank",
            "age_rank",
        ];
        assert_eq!(NSM_SAMPLER.marks().len(), NsmMark::ALL.len());
        for (mark, name) in NsmMark::ALL.into_iter().zip(names) {
            let law = NSM_SAMPLER.mark(mark);
            assert_eq!(law.name(), name);
            assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{name}");
            let (lo, hi) = law.limits();
            assert!(lo < hi, "{name}");
        }
        let (lo, hi) = NSM_SAMPLER.mark(NsmMark::NeutronStarMass).limits();
        assert_eq!(
            (lo, hi),
            (MIN_NEUTRON_STAR_MASS.value(), MAX_NEUTRON_STAR_MASS.value())
        );
        let (lo, _) = NSM_SAMPLER.mark(NsmMark::LogBlackHoleMass).limits();
        assert!(crate::math::exp10(lo) > MAX_NEUTRON_STAR_MASS.value());
        let kinds = NSM_SAMPLER.pick(NsmPick::PairKind).weights();
        assert_eq!(kinds.len(), NsmPairKind::ALL.len());
        assert!((kinds.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
}
