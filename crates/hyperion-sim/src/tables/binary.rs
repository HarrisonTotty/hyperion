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

use std::marker::PhantomData;

use crate::galaxy::POPULATIONS;
use crate::galaxy::Population;
use crate::galaxy::imf::MassBand;
pub use crate::stellar::binary::IaPoolChannel;
use crate::stellar::binary::light_curves::XrayAccretor;
use crate::stellar::binary::{BinaryClass, CarvedClass, HmxbKind, XrbKind};
use crate::tables::type_ia_delay::DELAY_EDGES;
use crate::units::Years;

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

// @begin-table binary_awd
// The accreting white dwarfs' samplers and share rows fitted against plan 11's engine at
// generator version 27 (plan 15, P15.T10.b), held until 27 (`catalogue_classes::binary`'s hold).
//
// @generated by hyperion-fit 0.1.0, task `binary_awd` revision 0. Do not edit.
// inputs-sha256: 77b2d86b5c0e936218648dcaba146e41f9af9bc698b21286d75ee37dec5f5dd7
// manifest: crates/hyperion-fit/manifests/binary_awd.toml
// data: none
// sim-fingerprint: e90be681791ba21923f19dd95e44fb857e830ac8e3e63be0947a49dcc67daf33
// since-generator-version: 26
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the rows by donor kind against Pala et al. 2020, MNRAS 494, 3799, Carter et al. 2013,
//   MNRAS 429, 2143, and Lü, Yungelson and Han 2006, MNRAS 372, 1389, the total against the
//   brainstorm's 6–12 × 10⁶
// acceptance: kind scales 2.8979e-1/7.5593e-2/2.3673e-2; fast: 613 members, own nodes
//   0/8/11/0/2/13/0/3/14/11/10, donors 0.981/0.000/0.019, layers C 0.102 D 0.857 E 0.041, rates
//   a system 7.01e-7/1.32e-5/7.66e-6/1.26e-5/1.26e-5/1.19e-5/6.31e-6, not its own: HeliumPeriod
//   Scratch, HeliumDonorMass Scratch; slow: 17500 members, own nodes
//   0/16/17/16/0/14/16/0/17/17/17, donors 0.904/0.096/0.001, layers C 0.532 D 0.453 E 0.015,
//   rates a system 4.91e-6/3.50e-4/7.04e-4/5.26e-4/4.51e-4/4.50e-4/9.00e-4; the K–S tests
//   against the other half of the census are the sim's slow
//   `tables_hold::the_accreting_white_dwarf_samplers_against_the_engine`
//
// Read by `AWD_FAST_SAMPLER_27`, `AWD_SLOW_SAMPLER_27` and `CLASS_SHARES_27`, after the block. Each
// class's marks are fitted to its own members of the engine near the solar circle at generator
// version 27's rules (hyperion-fit's `tasks::binary_classes`): a median that follows the
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
pub const AWD_FAST_RESIDUALS_27: [[f64; 17]; 11] = [
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-1.164_111_055_929_217_4, -0.775_421_644_888_874_2, -0.436_759_822_323_536_33, -0.215_642_025_666_629_57, -0.123_845_614_372_794_05, -0.071_528_823_308_661_57, -0.042_667_059_043_325_885, -0.018_894_770_618_637_216, 0.002_398_376_668_550_865, 0.021_928_796_584_484_073, 0.054_244_514_376_542_91, 0.131_663_386_372_419_84, 0.256_057_486_699_505_1, 0.530_494_327_463_588_4, 0.680_073_109_868_769_6, 0.808_174_624_514_457_1, 1.098_102_559_889_582_6],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.387_681_756_305_064_1, -0.139_915_273_169_329_67, -0.101_379_119_849_872_93, -0.078_482_519_293_108_18, -0.064_720_569_630_611_06, -0.047_493_955_024_984_23, -0.033_495_593_845_659_734, -0.021_182_887_867_983_224, -0.011_111_126_186_946_18, -0.000_810_711_369_996_291_3, 0.013_057_217_074_962_279, 0.031_034_067_602_048_975, 0.042_569_872_839_103_49, 0.059_961_516_964_100_13, 0.081_365_022_559_710_98, 0.138_213_198_170_812_82, 0.743_438_566_985_661_8],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.291_285_565_089_977_7, -0.066_594_467_714_896_51, -0.038_340_954_816_483_42, -0.026_658_485_283_843_986, -0.018_711_950_678_449_222, -0.012_456_919_072_635_811, -0.008_587_362_964_881_532, -0.003_977_674_427_465_77, -0.000_383_237_214_408_548_56, 0.002_119_992_269_261_531_7, 0.004_767_224_201_472_132, 0.009_205_860_117_660_062, 0.014_218_524_178_244_424, 0.021_276_532_453_210_855, 0.032_090_346_981_830_69, 0.040_086_871_139_112_13, 0.170_080_975_778_405_86],
    [-0.470_827_178_148_350_6, -0.131_718_232_647_130_62, -0.081_380_898_486_239_75, -0.061_929_224_006_789_28, -0.045_785_066_241_376_945, -0.027_603_557_016_741_91, -0.016_284_704_227_553_13, -0.005_731_674_279_781_777, 0.003_605_384_124_594_845, 0.010_751_067_173_949_864, 0.021_305_074_310_112_623, 0.031_870_451_912_861_67, 0.040_392_488_758_355_64, 0.053_023_696_800_779_87, 0.071_033_470_629_369_11, 0.094_562_909_405_339_15, 0.287_858_679_756_317_24],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
];

/// The fast accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_FAST_NODE_RESIDUALS_27: [[f64; 17]; 187] = [
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [2.845_060_162_136_332, 3.365_789_955_544_262, 3.708_939_563_368_781, 4.064_919_053_486_51, 4.401_948_507_620_873, 4.577_672_998_440_025, 4.698_299_529_841_7, 4.818_170_573_618_532_5, 4.889_638_079_311_962, 4.948_088_391_654_839, 5.024_828_749_817_566_6, 5.111_302_614_455_006, 5.191_775_409_290_485, 5.254_022_885_800_976_5, 5.305_695_647_657_43, 5.352_213_224_346_634, 5.417_670_438_063_333],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.793_133_727_643_607_7, -0.586_267_462_866_500_1, -0.493_935_801_744_538_5, -0.440_998_669_448_139_87, -0.334_256_581_112_431_7, -0.191_421_810_424_863_56, -0.110_391_020_507_512_84, -0.051_733_986_057_785_43, 2.271_999_370_181_222e-6, 0.026_515_702_606_448_53, 0.051_615_054_120_952_64, 0.070_400_189_451_351_79, 0.096_765_397_745_114_2, 0.117_629_313_679_156_41, 0.146_451_882_167_792_17, 0.196_464_502_651_194_75, 0.641_239_456_182_135_6],
    [-0.691_338_188_466_854_6, -0.539_152_795_594_996_3, -0.385_306_978_475_205_23, -0.310_763_642_200_895_36, -0.183_601_978_598_604_08, -0.161_266_013_041_594_35, -0.133_156_994_378_867_7, -0.100_955_260_137_517_42, -0.007_957_629_533_338_482, 0.009_745_514_764_227_614, 0.022_733_028_055_965_17, 0.102_353_206_438_030_4, 0.134_085_065_679_606_46, 0.173_370_070_323_063_98, 0.195_863_537_762_109_8, 0.242_474_055_116_712_74, 0.284_694_033_092_070_86],
    [-0.775_334_045_696_903_3, -0.570_257_773_909_375_7, -0.466_040_396_044_741_35, -0.368_184_564_085_667_63, -0.312_605_271_159_014_7, -0.175_183_601_525_276_73, -0.139_544_741_408_328_76, -0.119_103_651_173_258_32, -0.035_352_780_456_228_93, 0.017_829_025_295_582_304, 0.084_039_369_992_191_37, 0.129_267_867_854_459, 0.162_292_345_887_654_7, 0.173_100_431_554_587_82, 0.192_464_592_062_874_06, 0.229_400_308_976_590_28, 0.284_694_033_092_070_86],
    [-0.793_133_727_643_607_7, -0.586_271_398_548_565_5, -0.496_848_477_553_856_1, -0.410_413_098_552_719_74, -0.355_289_102_025_732_9, -0.197_336_564_305_009_7, -0.133_103_499_925_275_1, -0.068_637_249_834_069_98, 0.012_319_199_924_943_543, 0.057_171_633_931_109_446, 0.105_204_171_051_029_8, 0.121_933_672_493_641_23, 0.137_407_917_098_436_97, 0.157_826_818_005_148_1, 0.168_793_458_491_391_63, 0.187_046_840_095_883_68, 0.284_694_033_092_070_86],
    [-0.793_133_727_643_607_7, -0.589_720_444_633_229_4, -0.509_786_790_710_780_3, -0.474_346_617_303_657_7, -0.402_237_017_698_072_85, -0.227_787_072_926_816_6, -0.124_102_421_946_800_43, -0.047_333_216_454_733_845, 0.013_335_086_986_583_113, 0.058_834_774_489_946_38, 0.087_268_232_933_184_9, 0.102_616_743_500_155_61, 0.117_315_594_924_361_29, 0.122_011_229_941_202_86, 0.131_472_658_545_958_5, 0.158_430_101_363_062_76, 0.239_611_392_667_783_64],
    [-0.793_133_727_643_607_7, -0.581_362_032_256_562_6, -0.519_675_467_313_355_5, -0.482_574_184_998_016_1, -0.437_983_131_169_293_05, -0.254_535_455_825_242_7, -0.137_767_768_994_602_25, -0.071_120_779_587_820_97, 0.002_322_467_751_756_673_3, 0.050_035_438_936_223_41, 0.074_339_298_293_742_83, 0.087_566_214_946_247_86, 0.099_650_163_904_700_26, 0.109_077_963_779_504_49, 0.118_906_187_702_476_38, 0.128_106_587_638_651_18, 0.174_720_973_116_268_44],
    [-0.793_133_727_643_607_7, -0.584_789_579_335_712, -0.507_059_105_857_221_8, -0.468_942_774_841_078_6, -0.400_546_113_444_431_5, -0.249_270_767_340_993_77, -0.130_402_714_133_372_23, -0.062_764_825_823_965_3, -0.002_046_118_051_786_528_6, 0.029_408_434_048_392_978, 0.051_482_738_291_273_62, 0.067_569_150_316_301_44, 0.079_261_436_282_293_5, 0.092_232_666_342_391_08, 0.102_551_397_125_804_95, 0.118_381_056_600_753_94, 0.154_339_527_727_776_6],
    [-0.680_650_064_592_156, -0.587_779_779_278_119_1, -0.497_884_386_754_801_6, -0.462_497_080_982_892_33, -0.363_653_104_743_703_2, -0.207_923_173_178_937_05, -0.117_517_145_799_228_27, -0.048_572_074_666_582_254, -0.000_666_911_854_276_738_5, 0.019_917_192_654_370_666, 0.036_255_721_417_645_04, 0.049_230_717_166_429_17, 0.055_570_368_879_111_376, 0.069_156_551_869_659_72, 0.085_578_634_108_216_96, 0.100_986_082_255_669_99, 0.154_339_527_727_776_6],
    [-0.680_650_064_592_156, -0.591_770_456_583_535_3, -0.486_503_173_458_63, -0.442_775_293_411_015_24, -0.361_708_661_894_447_8, -0.200_424_714_028_982_24, -0.106_728_518_308_727_55, -0.023_294_994_046_321_328, 0.000_971_180_286_746_683_6, 0.017_111_241_864_725_752, 0.029_905_381_303_499_02, 0.038_989_783_428_117_45, 0.049_849_684_283_323_537, 0.055_434_009_790_885_405, 0.069_067_135_895_978_33, 0.091_936_207_870_741_29, 0.154_339_527_727_776_6],
    [-0.467_850_058_574_357_8, -0.376_257_331_496_975_6, -0.202_536_497_250_228_43, -0.148_585_486_968_531_36, -0.129_348_589_259_748_9, -0.085_815_200_627_848_18, -0.055_378_004_796_330_58, -0.025_281_216_706_726_933, 0.005_390_998_786_540_702_5, 0.017_594_713_219_392_117, 0.052_957_273_546_578_656, 0.108_174_453_531_991_29, 0.136_855_911_659_811_67, 0.171_416_826_044_896_4, 0.230_661_315_621_096_37, 0.636_844_334_990_973_9, 0.897_986_263_097_887_9],
    [-0.631_207_300_438_516_4, -0.337_243_110_379_811_5, -0.208_905_097_789_040_15, -0.149_715_660_195_641_48, -0.129_336_038_368_068_8, -0.088_288_490_397_137_89, -0.055_775_651_008_939_83, -0.033_089_281_712_270_845, -0.008_692_803_467_333_312, 0.005_827_258_745_633_577, 0.018_047_080_033_552_92, 0.036_602_973_731_071_48, 0.063_804_371_844_546_65, 0.113_860_297_949_858_27, 0.167_738_846_582_510_7, 0.620_990_148_854_284_6, 0.897_986_263_097_887_9],
    [-1.073_557_309_996_426_4, -0.416_352_516_155_193_3, -0.276_199_861_095_627_8, -0.169_452_357_998_217_37, -0.124_965_358_507_173_66, -0.084_649_671_733_254_73, -0.043_784_144_503_402_345, -0.014_781_317_110_601_728, -0.001_362_888_713_242_869_7, 0.008_522_407_464_130_657, 0.025_042_856_826_393_265, 0.041_109_976_744_888_11, 0.063_621_755_044_670_78, 0.114_887_048_318_509_18, 0.259_408_841_497_488_33, 0.669_322_211_037_761_2, 0.897_986_263_097_887_9],
    [-1.125_489_294_519_578_2, -0.766_552_251_036_202_6, -0.463_847_551_010_435_84, -0.303_098_989_978_702_7, -0.169_516_575_967_494_84, -0.070_679_239_606_291_85, -0.016_780_778_273_120_193, 0.002_567_239_972_356_340_6, 0.020_440_077_676_495_805, 0.039_863_007_283_407_4, 0.068_631_767_098_451_29, 0.142_162_065_079_666_42, 0.366_407_871_574_437_8, 0.524_044_138_477_068_1, 0.609_372_614_855_604_2, 0.715_581_093_701_890_5, 0.888_924_331_145_922_7],
    [-1.164_111_055_929_217_4, -1.103_644_704_577_899_4, -1.073_037_431_628_383_5, -0.903_938_917_592_802_1, -0.620_414_103_824_394_2, -0.445_911_936_033_481_7, -0.225_577_751_758_479_94, 0.018_318_084_146_490_256, 0.055_586_412_841_558_76, 0.105_995_853_798_942_62, 0.169_846_758_870_349_53, 0.272_561_236_799_492_7, 0.416_482_094_672_552_4, 0.514_042_775_566_105_6, 0.587_171_498_008_571_5, 0.694_558_402_077_180_7, 1.007_533_406_445_877_6],
    [-1.164_111_055_929_217_4, -1.116_442_530_429_3, -1.104_092_684_924_436_1, -1.088_171_706_417_229, -0.473_472_485_127_151, -0.452_019_659_450_412_84, -0.132_573_326_231_212_9, -0.039_110_701_891_034_29, 0.001_007_576_172_648_111, 0.018_185_106_707_701_85, 0.044_790_470_522_468_8, 0.074_477_245_867_433_36, 0.126_941_752_270_195_55, 0.251_115_343_680_427_04, 0.486_824_497_619_772_87, 0.625_222_914_365_039_9, 1.098_102_559_889_582_6],
    [-1.164_111_055_929_217_4, -1.106_335_564_685_268_2, -0.465_757_652_399_276_4, -0.436_792_043_781_116_36, -0.063_792_582_103_098_89, -0.045_550_221_829_720_23, -0.027_397_874_570_617_03, -0.017_688_559_145_256_535, -9.966_408_352_353_484e-6, 0.009_127_041_325_763_646, 0.029_259_028_978_632_397, 0.044_713_301_855_860_914, 0.078_439_315_322_781_34, 0.424_722_952_828_032_13, 0.724_112_855_319_728_7, 0.877_620_362_446_521_4, 1.098_102_559_889_582_6],
    [-1.164_111_055_929_217_4, -0.437_172_991_172_334_16, -0.103_311_304_241_579_93, -0.064_382_486_203_028_46, -0.051_346_493_781_424_31, -0.039_338_472_943_222_6, -0.026_525_081_467_742_177, -0.018_459_782_297_393_5, -0.000_585_285_797_462_079_5, 0.013_757_016_314_874_271, 0.037_106_148_505_216_96, 0.080_967_087_374_470_24, 0.600_160_436_782_866_7, 0.774_906_306_322_693_9, 0.814_747_883_723_708_8, 0.963_043_766_883_552_3, 1.098_102_559_889_582_6],
    [-0.490_017_853_067_563_35, -0.151_144_364_528_951_47, -0.115_857_229_389_693_31, -0.080_009_998_119_802_11, -0.060_497_864_838_228_09, -0.045_492_694_967_271_5, -0.034_208_856_080_519_73, -0.022_187_738_323_173_5, -0.003_195_961_231_816_310_7, 0.023_787_270_720_803_624, 0.395_800_521_733_267_74, 0.639_466_227_787_732_5, 0.730_980_679_336_784_6, 0.794_674_309_824_683_3, 0.824_831_070_679_124, 0.928_410_056_730_874_8, 1.098_102_559_889_582_6],
    [-0.668_793_007_425_062_9, -0.279_689_587_026_526_8, -0.160_031_154_717_690_94, -0.121_862_001_240_217_91, -0.094_399_741_268_224_44, -0.071_729_688_597_156_48, -0.045_282_616_717_829_68, -0.032_743_565_033_620_31, -0.004_297_506_618_080_107, 0.036_377_074_771_890_19, 0.520_720_120_119_424, 0.612_524_022_674_775_9, 0.725_108_006_638_801_6, 0.785_167_858_033_755_7, 0.809_854_829_118_425, 0.855_026_229_648_091_6, 1.033_581_574_026_011],
    [-0.963_321_542_395_673_1, -0.615_245_525_591_540_1, -0.478_764_707_069_718_46, -0.319_268_770_656_545_13, -0.232_808_399_556_691_66, -0.160_173_352_713_239_24, -0.111_178_904_685_562_64, -0.066_669_924_026_503_24, -0.027_835_405_914_559_363, 0.061_521_532_092_173_39, 0.395_284_449_988_595_1, 0.534_893_377_768_618_3, 0.586_066_220_591_063_4, 0.683_224_998_643_683_4, 0.784_450_059_523_392_4, 0.835_756_899_269_885_6, 1.033_581_574_026_011],
    [-1.164_111_055_929_217_4, -0.775_421_644_888_874_2, -0.436_759_822_323_536_33, -0.215_642_025_666_629_57, -0.123_845_614_372_794_05, -0.071_528_823_308_661_57, -0.042_667_059_043_325_885, -0.018_894_770_618_637_216, 0.002_398_376_668_550_865, 0.021_928_796_584_484_073, 0.054_244_514_376_542_91, 0.131_663_386_372_419_84, 0.256_057_486_699_505_1, 0.530_494_327_463_588_4, 0.680_073_109_868_769_6, 0.808_174_624_514_457_1, 1.098_102_559_889_582_6],
    [-1.164_111_055_929_217_4, -0.775_421_644_888_874_2, -0.436_759_822_323_536_33, -0.215_642_025_666_629_57, -0.123_845_614_372_794_05, -0.071_528_823_308_661_57, -0.042_667_059_043_325_885, -0.018_894_770_618_637_216, 0.002_398_376_668_550_865, 0.021_928_796_584_484_073, 0.054_244_514_376_542_91, 0.131_663_386_372_419_84, 0.256_057_486_699_505_1, 0.530_494_327_463_588_4, 0.680_073_109_868_769_6, 0.808_174_624_514_457_1, 1.098_102_559_889_582_6],
    [-1.164_111_055_929_217_4, -0.775_421_644_888_874_2, -0.436_759_822_323_536_33, -0.215_642_025_666_629_57, -0.123_845_614_372_794_05, -0.071_528_823_308_661_57, -0.042_667_059_043_325_885, -0.018_894_770_618_637_216, 0.002_398_376_668_550_865, 0.021_928_796_584_484_073, 0.054_244_514_376_542_91, 0.131_663_386_372_419_84, 0.256_057_486_699_505_1, 0.530_494_327_463_588_4, 0.680_073_109_868_769_6, 0.808_174_624_514_457_1, 1.098_102_559_889_582_6],
    [-1.164_111_055_929_217_4, -0.775_421_644_888_874_2, -0.436_759_822_323_536_33, -0.215_642_025_666_629_57, -0.123_845_614_372_794_05, -0.071_528_823_308_661_57, -0.042_667_059_043_325_885, -0.018_894_770_618_637_216, 0.002_398_376_668_550_865, 0.021_928_796_584_484_073, 0.054_244_514_376_542_91, 0.131_663_386_372_419_84, 0.256_057_486_699_505_1, 0.530_494_327_463_588_4, 0.680_073_109_868_769_6, 0.808_174_624_514_457_1, 1.098_102_559_889_582_6],
    [-1.164_111_055_929_217_4, -0.775_421_644_888_874_2, -0.436_759_822_323_536_33, -0.215_642_025_666_629_57, -0.123_845_614_372_794_05, -0.071_528_823_308_661_57, -0.042_667_059_043_325_885, -0.018_894_770_618_637_216, 0.002_398_376_668_550_865, 0.021_928_796_584_484_073, 0.054_244_514_376_542_91, 0.131_663_386_372_419_84, 0.256_057_486_699_505_1, 0.530_494_327_463_588_4, 0.680_073_109_868_769_6, 0.808_174_624_514_457_1, 1.098_102_559_889_582_6],
    [-1.164_111_055_929_217_4, -0.775_421_644_888_874_2, -0.436_759_822_323_536_33, -0.215_642_025_666_629_57, -0.123_845_614_372_794_05, -0.071_528_823_308_661_57, -0.042_667_059_043_325_885, -0.018_894_770_618_637_216, 0.002_398_376_668_550_865, 0.021_928_796_584_484_073, 0.054_244_514_376_542_91, 0.131_663_386_372_419_84, 0.256_057_486_699_505_1, 0.530_494_327_463_588_4, 0.680_073_109_868_769_6, 0.808_174_624_514_457_1, 1.098_102_559_889_582_6],
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
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.206_931_987_494_571_53, -0.179_062_657_118_742_67, -0.164_130_549_452_386_78, -0.137_200_591_358_101_8, -0.105_023_945_196_083_2, -0.084_501_554_327_197_8, -0.052_948_593_313_216_6, 0.037_780_991_961_992_68, 0.057_195_804_117_812_07, 0.085_100_099_349_803_1, 0.117_079_769_444_516_72, 0.191_143_531_798_942, 0.237_256_990_490_719_02, 0.267_024_637_614_026_1, 0.333_862_663_461_850_06, 0.685_932_686_538_597_3],
    [-0.343_037_845_593_652_9, -0.202_329_269_208_252_2, -0.177_572_388_411_030_4, -0.164_136_043_578_923_09, -0.141_728_405_983_945_32, -0.109_221_726_516_133_46, -0.095_806_532_273_351_8, -0.061_536_732_494_680_95, -0.020_489_759_450_352_21, 0.037_808_723_992_275_53, 0.076_973_174_772_974_16, 0.159_995_346_177_725_57, 0.188_312_222_522_850_65, 0.233_854_365_251_854_9, 0.282_120_674_790_104_9, 0.366_443_448_436_706_7, 0.685_932_686_538_597_3],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.456_921_525_171_712_25, -0.214_714_956_709_550_87, -0.179_136_080_321_559_9, -0.164_154_172_686_118_5, -0.133_160_858_523_206_58, -0.098_206_504_589_135_4, -0.060_589_754_752_088_16, -0.018_278_522_911_436_508, 0.037_841_773_953_782_18, 0.084_159_805_578_251_66, 0.114_100_379_070_142_09, 0.190_995_869_598_238_83, 0.239_260_193_742_774_88, 0.285_799_782_465_860_35, 0.473_474_698_801_954_6, 3.507_236_842_748_771, 5.158_465_709_971_066_5],
    [-0.387_681_756_305_064_1, -0.139_915_273_169_329_67, -0.101_379_119_849_872_93, -0.078_482_519_293_108_18, -0.064_720_569_630_611_06, -0.047_493_955_024_984_23, -0.033_495_593_845_659_734, -0.021_182_887_867_983_224, -0.011_111_126_186_946_18, -0.000_810_711_369_996_291_3, 0.013_057_217_074_962_279, 0.031_034_067_602_048_975, 0.042_569_872_839_103_49, 0.059_961_516_964_100_13, 0.081_365_022_559_710_98, 0.138_213_198_170_812_82, 0.743_438_566_985_661_8],
    [-0.387_681_756_305_064_1, -0.139_915_273_169_329_67, -0.101_379_119_849_872_93, -0.078_482_519_293_108_18, -0.064_720_569_630_611_06, -0.047_493_955_024_984_23, -0.033_495_593_845_659_734, -0.021_182_887_867_983_224, -0.011_111_126_186_946_18, -0.000_810_711_369_996_291_3, 0.013_057_217_074_962_279, 0.031_034_067_602_048_975, 0.042_569_872_839_103_49, 0.059_961_516_964_100_13, 0.081_365_022_559_710_98, 0.138_213_198_170_812_82, 0.743_438_566_985_661_8],
    [-0.387_681_756_305_064_1, -0.139_915_273_169_329_67, -0.101_379_119_849_872_93, -0.078_482_519_293_108_18, -0.064_720_569_630_611_06, -0.047_493_955_024_984_23, -0.033_495_593_845_659_734, -0.021_182_887_867_983_224, -0.011_111_126_186_946_18, -0.000_810_711_369_996_291_3, 0.013_057_217_074_962_279, 0.031_034_067_602_048_975, 0.042_569_872_839_103_49, 0.059_961_516_964_100_13, 0.081_365_022_559_710_98, 0.138_213_198_170_812_82, 0.743_438_566_985_661_8],
    [-0.387_681_756_305_064_1, -0.161_039_444_730_097_04, -0.136_640_635_966_700_62, -0.130_670_637_923_475_89, -0.114_504_919_327_108_46, -0.106_469_833_057_065_78, -0.096_360_134_127_640_43, -0.091_821_172_236_854_24, -0.079_596_917_956_852_3, -0.075_757_370_938_871_82, -0.064_589_075_122_579_28, -0.053_691_749_396_810_11, -0.041_984_822_418_211_366, -0.025_910_031_868_989_526, -0.005_885_332_127_350_777_6, 0.139_737_329_396_053_93, 0.218_964_018_907_221_06],
    [-0.208_859_662_760_340_85, -0.148_226_026_550_808_13, -0.128_527_357_267_007_12, -0.108_506_647_352_006_31, -0.098_837_901_789_325_15, -0.089_903_653_255_023_56, -0.078_445_732_699_465_45, -0.067_030_278_092_802_54, -0.052_211_354_131_071_13, -0.036_962_335_823_543_9, -0.025_811_612_421_085_726, -0.005_779_592_466_470_675, 0.022_007_526_661_968_986, 0.064_949_061_391_616, 0.105_482_064_004_210_73, 0.185_182_262_993_439_23, 0.218_964_018_907_221_06],
    [-0.312_006_880_624_988_1, -0.130_663_499_184_614_15, -0.103_776_839_319_769_11, -0.083_037_624_329_719_31, -0.074_358_546_120_998_32, -0.051_524_834_442_928_57, -0.035_008_023_516_907_856, -0.025_806_205_464_118_208, -0.014_450_510_659_021_39, -0.001_316_545_267_273_011, 0.018_583_946_391_155_83, 0.041_132_442_468_436_795, 0.065_378_676_578_529_89, 0.079_462_811_308_966_03, 0.137_931_683_711_308, 0.196_190_748_542_394_85, 0.313_176_844_094_089_5],
    [-0.339_321_303_192_868_46, -0.111_468_666_595_098_84, -0.081_560_784_781_280_34, -0.067_165_645_834_398_1, -0.048_318_901_362_899_23, -0.034_852_966_928_382_13, -0.025_138_850_221_469_118, -0.014_309_745_841_989_184, -0.003_152_879_903_526_649, 0.013_464_223_220_656_674, 0.027_911_385_889_713_77, 0.047_474_744_899_728_1, 0.070_217_043_280_626_72, 0.096_174_494_744_138_28, 0.169_341_208_887_906_08, 0.204_426_621_293_780_17, 0.313_176_844_094_089_5],
    [-0.339_321_303_192_868_46, -0.108_626_130_869_291_38, -0.073_131_698_972_400_24, -0.063_928_042_666_641_27, -0.049_179_632_594_629_27, -0.037_860_830_239_990_92, -0.028_670_000_464_304_53, -0.020_794_133_195_898_217, -0.013_548_052_662_244_401, -0.004_618_272_202_907_786, 0.006_018_637_702_169_427, 0.019_082_267_575_312_475, 0.035_675_812_798_167_09, 0.051_071_275_712_779_94, 0.072_127_071_551_623_91, 0.160_965_801_557_709_2, 0.313_176_844_094_089_5],
    [-0.339_321_303_192_868_46, -0.077_939_369_537_382_15, -0.065_227_662_014_700_66, -0.046_206_252_053_309_01, -0.035_782_508_067_228_054, -0.025_874_801_105_090_665, -0.018_549_876_530_939_95, -0.011_901_698_627_833_026, -0.004_081_840_696_682_916, 0.004_795_586_969_320_785, 0.014_514_495_936_381_593, 0.024_556_768_124_447_993, 0.037_132_904_244_362_566, 0.046_782_461_728_199_49, 0.056_232_476_839_880_92, 0.071_797_837_641_195_66, 0.313_176_844_094_089_5],
    [-0.339_321_303_192_868_46, -0.071_135_491_955_040_11, -0.051_582_006_750_307_996, -0.035_570_198_017_402_66, -0.024_969_008_835_151_275, -0.016_212_395_099_483_235, -0.008_749_784_630_033_583, -0.001_195_393_131_733_997_3, 0.006_945_759_695_481_406, 0.017_598_658_863_290_5, 0.024_439_260_216_265_31, 0.036_572_097_312_558_29, 0.043_286_635_962_608_476, 0.052_266_873_905_788_43, 0.061_104_991_781_412_6, 0.076_158_282_972_122_49, 0.120_713_576_184_449_07],
    [-0.141_767_305_947_180_1, -0.076_331_406_677_469_1, -0.051_734_741_610_373_88, -0.033_245_114_298_777_56, -0.019_654_842_046_064_86, -0.012_769_871_823_522_782, -0.006_382_442_820_882_885, -0.000_279_712_994_037_234_25, 0.007_266_494_988_681_606, 0.020_251_692_875_597_02, 0.024_352_745_087_136_64, 0.037_355_320_473_630_44, 0.051_081_188_278_453_954, 0.055_775_403_981_579_42, 0.075_982_699_534_419_12, 0.094_447_695_214_863_14, 0.120_713_576_184_449_07],
    [-0.322_981_392_049_982_16, -0.083_363_228_274_394_91, -0.069_721_303_437_414_12, -0.037_326_195_561_766_64, -0.025_202_785_657_743_063, -0.017_407_977_031_518_116, -0.009_674_891_914_243_03, -0.003_550_487_215_017_322, 0.001_283_489_890_401_269_3, 0.013_416_381_425_774_813, 0.027_857_373_326_773_8, 0.038_895_790_453_449_15, 0.045_156_049_421_224_136, 0.057_488_860_119_524_53, 0.077_873_414_354_879_99, 0.104_023_998_094_843_79, 0.181_245_055_787_031_82],
    [-0.322_981_392_049_982_16, -0.141_693_991_259_531_97, -0.079_798_399_746_246_88, -0.039_554_538_506_237_86, -0.026_266_956_124_433_243, -0.019_061_935_271_667_423, -0.008_845_209_407_223_577, 1.049_253_383_597_691e-5, 0.017_415_469_367_209_72, 0.036_431_007_438_457_26, 0.040_865_388_565_450_41, 0.045_021_351_664_654_9, 0.057_070_483_187_563_13, 0.067_156_464_577_044, 0.090_504_667_386_909_28, 0.121_924_035_846_285_2, 0.206_694_438_683_914_68],
    [-0.322_981_392_049_982_16, -0.178_937_129_339_857_9, -0.115_808_319_159_424_15, -0.074_031_747_889_146_54, -0.060_003_827_491_624_674, -0.034_258_067_551_234_97, -0.024_820_515_865_939_075, -0.008_871_938_194_162_524, 0.004_457_214_547_838_544, 0.033_635_100_278_194_58, 0.040_125_716_559_486_9, 0.046_002_660_527_329_01, 0.058_705_950_052_318_094, 0.079_460_078_396_149_98, 0.122_152_072_023_350_9, 0.177_813_908_816_394_9, 0.620_225_339_579_939_1],
    [-0.322_981_392_049_982_16, -0.195_773_571_019_198_34, -0.135_426_666_442_704_65, -0.111_738_208_460_086_89, -0.077_794_327_277_848_42, -0.066_287_792_297_859_5, -0.053_378_046_964_132_75, -0.034_181_375_912_488_48, -0.017_912_177_836_799_27, 0.002_046_629_090_536_252_4, 0.021_883_816_378_890_083, 0.037_203_323_402_023_97, 0.049_725_679_485_547_85, 0.086_286_624_600_294_49, 0.122_472_002_805_368_53, 0.182_929_727_107_246_61, 0.743_438_566_985_661_8],
    [-0.274_093_379_165_158_75, -0.141_575_330_084_217_88, -0.129_789_514_398_534_37, -0.110_917_181_882_468_46, -0.077_917_375_976_372_7, -0.066_807_261_204_518_12, -0.057_512_141_130_510_425, -0.045_244_904_923_772_17, -0.035_272_550_465_108_86, -0.014_289_667_775_564_091, 0.002_635_518_204_258_708, 0.033_310_856_534_096_6, 0.038_595_115_182_671_69, 0.054_749_499_588_855_1, 0.106_394_871_779_461_9, 0.495_182_141_955_095_13, 0.743_438_566_985_661_8],
    [-0.387_681_756_305_064_1, -0.139_915_273_169_329_67, -0.101_379_119_849_872_93, -0.078_482_519_293_108_18, -0.064_720_569_630_611_06, -0.047_493_955_024_984_23, -0.033_495_593_845_659_734, -0.021_182_887_867_983_224, -0.011_111_126_186_946_18, -0.000_810_711_369_996_291_3, 0.013_057_217_074_962_279, 0.031_034_067_602_048_975, 0.042_569_872_839_103_49, 0.059_961_516_964_100_13, 0.081_365_022_559_710_98, 0.138_213_198_170_812_82, 0.743_438_566_985_661_8],
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
    [-0.245_024_436_997_118_68, -0.074_510_071_808_971_5, -0.057_050_679_293_295_15, -0.046_111_488_908_410_02, -0.011_874_080_486_172_806, -0.007_588_922_079_999_173, 0.006_201_797_772_298_864, 0.014_926_186_104_129_11, 0.031_550_743_115_582_335, 0.047_000_250_499_192_334, 0.056_392_461_588_152_43, 0.079_365_927_828_730_81, 0.091_082_085_129_279_19, 0.095_815_836_101_639_24, 0.104_278_502_539_084_58, 0.116_885_075_089_163_13, 0.229_055_643_763_353_86],
    [-0.245_024_436_997_118_68, -0.139_087_267_427_332_63, -0.088_455_116_827_691_51, -0.067_522_349_674_700_04, -0.056_953_871_618_432_06, -0.045_202_797_467_009_906, -0.029_174_980_630_176_536, -0.007_677_905_504_575_319, 0.003_365_586_847_277_713_5, 0.014_260_620_436_539_227, 0.031_207_412_936_331_363, 0.046_205_205_134_669_7, 0.055_859_712_664_257_676, 0.088_455_960_931_116, 0.096_225_116_148_738_35, 0.106_615_745_852_700_49, 0.229_055_643_763_353_86],
    [-0.245_024_436_997_118_68, -0.150_533_659_551_161_66, -0.132_310_984_874_753_32, -0.097_898_512_746_135_38, -0.079_170_093_456_998_66, -0.068_646_738_632_608_9, -0.056_995_285_805_526_3, -0.045_298_500_142_833_83, -0.034_178_208_168_921_066, -0.012_127_026_932_088_279, -0.001_565_429_548_337_174_4, 0.005_727_936_595_668_942, 0.027_998_480_797_296_47, 0.037_790_802_718_586_46, 0.058_728_830_075_454_85, 0.095_516_582_726_549_48, 0.229_055_643_763_353_86],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.245_024_436_997_118_68, -0.134_575_884_993_054_42, -0.089_590_613_402_747_1, -0.068_791_452_551_711, -0.056_739_394_753_713_05, -0.041_114_057_987_643_6, -0.024_507_790_598_671_003, -0.007_628_540_102_675_239_5, 0.004_543_764_215_071_713, 0.015_092_682_861_274_815, 0.031_300_271_670_574_45, 0.048_282_576_478_749_95, 0.077_128_567_921_781_19, 0.095_017_443_882_453_85, 0.107_992_860_811_097_67, 0.382_133_093_191_222_3, 0.951_083_694_016_714],
    [-0.291_285_565_089_977_7, -0.066_594_467_714_896_51, -0.038_340_954_816_483_42, -0.026_658_485_283_843_986, -0.018_711_950_678_449_222, -0.012_456_919_072_635_811, -0.008_587_362_964_881_532, -0.003_977_674_427_465_77, -0.000_383_237_214_408_548_56, 0.002_119_992_269_261_531_7, 0.004_767_224_201_472_132, 0.009_205_860_117_660_062, 0.014_218_524_178_244_424, 0.021_276_532_453_210_855, 0.032_090_346_981_830_69, 0.040_086_871_139_112_13, 0.170_080_975_778_405_86],
    [-0.291_285_565_089_977_7, -0.066_594_467_714_896_51, -0.038_340_954_816_483_42, -0.026_658_485_283_843_986, -0.018_711_950_678_449_222, -0.012_456_919_072_635_811, -0.008_587_362_964_881_532, -0.003_977_674_427_465_77, -0.000_383_237_214_408_548_56, 0.002_119_992_269_261_531_7, 0.004_767_224_201_472_132, 0.009_205_860_117_660_062, 0.014_218_524_178_244_424, 0.021_276_532_453_210_855, 0.032_090_346_981_830_69, 0.040_086_871_139_112_13, 0.170_080_975_778_405_86],
    [-0.291_285_565_089_977_7, -0.066_594_467_714_896_51, -0.038_340_954_816_483_42, -0.026_658_485_283_843_986, -0.018_711_950_678_449_222, -0.012_456_919_072_635_811, -0.008_587_362_964_881_532, -0.003_977_674_427_465_77, -0.000_383_237_214_408_548_56, 0.002_119_992_269_261_531_7, 0.004_767_224_201_472_132, 0.009_205_860_117_660_062, 0.014_218_524_178_244_424, 0.021_276_532_453_210_855, 0.032_090_346_981_830_69, 0.040_086_871_139_112_13, 0.170_080_975_778_405_86],
    [-0.259_812_324_017_429_5, -0.081_739_940_700_150_12, -0.057_388_317_069_750_18, -0.052_226_890_911_547_134, -0.044_326_450_712_358_27, -0.039_838_415_891_178_23, -0.036_857_112_131_467_15, -0.030_265_068_512_421_42, -0.018_854_871_460_136_573, -0.011_475_768_014_304_609, -0.004_752_533_479_111_167, 0.009_613_095_733_228_829, 0.012_356_898_075_465_61, 0.030_534_289_759_830_666, 0.033_117_855_065_641_466, 0.039_032_555_560_986_865, 0.045_738_883_894_875_626],
    [-0.259_812_324_017_429_5, -0.137_506_632_181_278_95, -0.066_594_139_519_491_65, -0.050_855_772_595_133_15, -0.038_431_570_507_305_28, -0.029_326_218_613_948_792, -0.018_277_161_290_688_16, -0.005_214_508_306_733_604, 0.005_791_238_619_307_437, 0.010_942_606_517_640_317, 0.012_737_547_142_251_003, 0.027_525_545_502_587_44, 0.031_478_138_024_222_57, 0.033_524_612_100_845_48, 0.036_438_782_755_454_69, 0.039_752_414_840_389_33, 0.064_579_888_831_616_8],
    [-0.217_620_667_512_133_77, -0.135_037_847_116_754_1, -0.095_162_011_282_008_16, -0.056_232_392_648_704_31, -0.025_342_869_857_481_99, -0.017_401_099_969_447_515, -0.003_882_564_264_473_667_6, 0.004_535_411_469_113_613, 0.010_041_396_694_836_73, 0.016_111_589_335_768_114, 0.019_635_420_666_782_536, 0.026_787_002_427_483_827, 0.031_539_036_279_229, 0.034_163_687_677_618_91, 0.038_174_705_919_804_396, 0.043_292_468_082_227_37, 0.064_579_888_831_616_8],
    [-0.195_138_346_870_441_02, -0.130_854_852_650_747_67, -0.103_399_443_496_090_23, -0.072_918_674_038_749_58, -0.041_620_257_696_768_456, -0.014_455_573_891_380_283, -0.004_605_417_180_624_760_5, 0.001_086_101_283_201_638_3, 0.007_557_433_703_186_955, 0.017_472_002_753_146_042, 0.019_346_832_516_314_253, 0.024_140_993_739_279_224, 0.028_567_855_126_831_167, 0.036_241_097_031_476_474, 0.042_104_987_433_072_39, 0.048_894_675_828_754_38, 0.170_080_975_778_405_86],
    [-0.291_285_565_089_977_7, -0.136_402_431_038_705_42, -0.122_654_192_950_769_01, -0.095_893_321_431_159_93, -0.067_584_058_610_054_22, -0.038_497_324_256_768_765, -0.007_352_575_755_617_208, 0.000_888_574_690_987_197_2, 0.007_368_594_203_904_672, 0.014_302_069_820_020_639, 0.018_267_973_980_015_523, 0.025_499_012_834_575_823, 0.033_983_903_994_276_64, 0.041_951_601_963_198_075, 0.048_764_575_042_614_66, 0.053_273_322_260_543_43, 0.170_080_975_778_405_86],
    [-0.291_285_565_089_977_7, -0.257_333_552_666_631_27, -0.129_234_148_677_797_73, -0.104_493_123_987_912, -0.069_517_064_033_878_61, -0.036_697_298_176_884_99, -0.013_322_483_258_366_63, -0.001_884_293_306_919_567_4, 0.001_554_858_939_292_722, 0.007_098_567_360_457_965, 0.008_777_982_434_529_468, 0.015_080_974_095_446_745, 0.023_841_693_251_973_106, 0.040_858_346_320_613_09, 0.049_008_028_452_098_09, 0.112_505_415_400_026_48, 0.170_080_975_778_405_86],
    [-0.291_285_565_089_977_7, -0.283_058_535_824_922_96, -0.074_414_271_629_484_04, -0.040_362_071_759_051_48, -0.021_923_292_786_309_907, -0.015_972_977_487_536_295, -0.011_980_609_020_445_485, -0.006_776_029_070_701_613, -0.003_049_970_383_531_396_7, 0.001_449_977_972_353_160_5, 0.001_641_440_923_763_924_2, 0.004_783_463_501_732_749, 0.007_609_120_978_267_976, 0.015_116_701_103_723_51, 0.052_129_965_134_653_34, 0.083_142_177_464_151_6, 0.170_080_975_778_405_86],
    [-0.291_285_565_089_977_7, -0.067_880_243_800_467_81, -0.049_824_092_757_253_28, -0.028_671_224_651_095_305, -0.019_931_606_389_775_4, -0.014_966_895_781_326, -0.010_289_276_260_846_142, -0.006_108_232_515_398_675, -0.004_001_639_790_797_483_5, -0.001_557_952_888_698_583, 0.001_295_229_518_063_877_1, 0.001_516_839_422_231_917_5, 0.002_059_693_148_671_785, 0.007_480_477_313_168_401, 0.059_217_273_060_839_03, 0.078_610_819_660_427_5, 0.111_010_642_075_070_61],
    [-0.290_097_147_750_438_8, -0.054_065_469_090_592_93, -0.035_135_205_955_995_36, -0.025_106_382_077_270_763, -0.019_618_364_048_571_337, -0.014_002_780_696_124_676, -0.007_292_327_604_443_281, -0.005_514_065_125_020_009, -0.003_144_139_628_433_926, -0.000_381_966_838_683_031_14, 0.001_191_292_773_426_201_6, 0.001_756_690_197_253_073_7, 0.002_581_236_182_505_191_3, 0.005_015_236_147_522_853_5, 0.051_058_117_723_521_91, 0.060_295_952_510_632_564, 0.090_545_589_872_768_95],
    [-0.290_097_147_750_438_8, -0.040_690_250_075_656_555, -0.033_465_423_285_716_6, -0.025_360_518_539_873_533, -0.022_265_515_888_310_6, -0.015_877_118_546_975_703, -0.011_833_598_442_855_614, -0.008_636_604_195_119_534, -0.004_314_051_895_778_149, -0.001_591_317_269_983_069_6, 0.000_506_842_382_151_722, 0.001_654_847_651_048_827_1, 0.002_605_988_778_711_854, 0.004_484_860_453_446_517, 0.006_726_158_797_224_882, 0.053_239_549_634_121_11, 0.078_472_267_945_387_75],
    [-0.284_216_342_480_747_95, -0.037_751_100_562_800_91, -0.030_932_784_555_226_904, -0.025_631_854_856_967_76, -0.021_970_107_074_781_378, -0.016_172_177_719_674_07, -0.014_184_816_402_236_096, -0.010_929_897_384_327_534, -0.009_249_965_246_880_716, -0.005_087_845_848_074_272, -0.002_008_140_815_979_063_7, 0.000_480_885_441_383_818_7, 0.002_479_708_695_257_843_7, 0.004_741_672_251_863_94, 0.008_776_499_698_477_365, 0.024_696_228_894_473_152, 0.061_127_996_586_011_31],
    [-0.055_797_817_416_510_576, -0.035_003_302_510_062_735, -0.026_402_934_176_941_14, -0.019_489_954_772_523_407, -0.014_962_007_015_323_917, -0.011_915_879_312_162_292, -0.009_344_766_240_251_531, -0.007_956_201_256_673_5, -0.003_233_311_280_288_533_7, -0.001_238_906_758_949_646_7, 0.002_034_525_048_830_395_6, 0.004_064_665_237_217_469, 0.005_452_107_042_173_989_4, 0.010_214_972_978_443_496, 0.017_662_650_823_592_65, 0.025_002_126_412_684_827, 0.040_064_249_576_116_43],
    [-0.055_797_817_416_510_576, -0.028_426_794_427_456_24, -0.019_167_501_390_119_492, -0.012_498_647_440_889_694, -0.009_251_765_294_082_729, -0.007_079_182_356_062_515, -0.003_276_369_095_579_804_4, 9.655_178_465_593_393e-6, 0.002_210_389_014_843_178_4, 0.004_347_531_927_927_7, 0.005_705_028_205_010_581, 0.009_713_763_519_626_705, 0.012_107_116_000_679_284, 0.017_258_657_626_961_13, 0.022_723_423_199_047_424, 0.032_039_567_214_284_33, 0.039_700_193_797_693_184],
    [-0.055_797_817_416_510_576, -0.023_461_617_151_388_087, -0.012_235_518_802_519_971, -0.008_340_683_587_908_924, -0.003_838_747_630_216_324, -6.364_994_649_835_315e-5, 0.002_555_332_501_166_372, 0.004_665_719_909_067_409, 0.008_229_371_331_477_229, 0.010_631_364_599_803_09, 0.012_547_954_427_061_745, 0.015_684_617_877_310_434, 0.019_105_310_617_429_466, 0.021_552_771_742_019_524, 0.028_908_804_457_506_24, 0.034_927_681_697_657_524, 0.038_938_236_430_021_145],
    [-0.470_827_178_148_350_6, -0.131_718_232_647_130_62, -0.081_380_898_486_239_75, -0.061_929_224_006_789_28, -0.045_785_066_241_376_945, -0.027_603_557_016_741_91, -0.016_284_704_227_553_13, -0.005_731_674_279_781_777, 0.003_605_384_124_594_845, 0.010_751_067_173_949_864, 0.021_305_074_310_112_623, 0.031_870_451_912_861_67, 0.040_392_488_758_355_64, 0.053_023_696_800_779_87, 0.071_033_470_629_369_11, 0.094_562_909_405_339_15, 0.287_858_679_756_317_24],
    [-0.470_827_178_148_350_6, -0.131_718_232_647_130_62, -0.081_380_898_486_239_75, -0.061_929_224_006_789_28, -0.045_785_066_241_376_945, -0.027_603_557_016_741_91, -0.016_284_704_227_553_13, -0.005_731_674_279_781_777, 0.003_605_384_124_594_845, 0.010_751_067_173_949_864, 0.021_305_074_310_112_623, 0.031_870_451_912_861_67, 0.040_392_488_758_355_64, 0.053_023_696_800_779_87, 0.071_033_470_629_369_11, 0.094_562_909_405_339_15, 0.287_858_679_756_317_24],
    [-0.470_827_178_148_350_6, -0.131_718_232_647_130_62, -0.081_380_898_486_239_75, -0.061_929_224_006_789_28, -0.045_785_066_241_376_945, -0.027_603_557_016_741_91, -0.016_284_704_227_553_13, -0.005_731_674_279_781_777, 0.003_605_384_124_594_845, 0.010_751_067_173_949_864, 0.021_305_074_310_112_623, 0.031_870_451_912_861_67, 0.040_392_488_758_355_64, 0.053_023_696_800_779_87, 0.071_033_470_629_369_11, 0.094_562_909_405_339_15, 0.287_858_679_756_317_24],
    [-0.470_827_178_148_350_6, -0.131_718_232_647_130_62, -0.081_380_898_486_239_75, -0.061_929_224_006_789_28, -0.045_785_066_241_376_945, -0.027_603_557_016_741_91, -0.016_284_704_227_553_13, -0.005_731_674_279_781_777, 0.003_605_384_124_594_845, 0.010_751_067_173_949_864, 0.021_305_074_310_112_623, 0.031_870_451_912_861_67, 0.040_392_488_758_355_64, 0.053_023_696_800_779_87, 0.071_033_470_629_369_11, 0.094_562_909_405_339_15, 0.287_858_679_756_317_24],
    [-0.470_827_178_148_350_6, -0.131_718_232_647_130_62, -0.081_380_898_486_239_75, -0.061_929_224_006_789_28, -0.045_785_066_241_376_945, -0.027_603_557_016_741_91, -0.016_284_704_227_553_13, -0.005_731_674_279_781_777, 0.003_605_384_124_594_845, 0.010_751_067_173_949_864, 0.021_305_074_310_112_623, 0.031_870_451_912_861_67, 0.040_392_488_758_355_64, 0.053_023_696_800_779_87, 0.071_033_470_629_369_11, 0.094_562_909_405_339_15, 0.287_858_679_756_317_24],
    [-0.470_827_178_148_350_6, -0.131_718_232_647_130_62, -0.081_380_898_486_239_75, -0.061_929_224_006_789_28, -0.045_785_066_241_376_945, -0.027_603_557_016_741_91, -0.016_284_704_227_553_13, -0.005_731_674_279_781_777, 0.003_605_384_124_594_845, 0.010_751_067_173_949_864, 0.021_305_074_310_112_623, 0.031_870_451_912_861_67, 0.040_392_488_758_355_64, 0.053_023_696_800_779_87, 0.071_033_470_629_369_11, 0.094_562_909_405_339_15, 0.287_858_679_756_317_24],
    [-0.470_827_178_148_350_6, -0.079_504_783_072_400_69, -0.074_573_038_803_066_26, -0.060_326_130_172_330_29, -0.051_416_490_141_196_98, -0.037_569_755_401_852_93, -0.021_636_251_687_655_06, -0.008_496_038_186_445_499, 0.000_184_855_484_813_048_31, 0.008_224_814_328_774_133, 0.020_009_644_672_573_632, 0.030_913_809_611_004_657, 0.046_878_384_991_065_634, 0.065_867_553_130_655_62, 0.090_521_678_454_628_77, 0.110_670_929_036_058_14, 0.196_744_929_605_098_62],
    [-0.470_827_178_148_350_6, -0.079_561_012_485_814_99, -0.073_243_431_901_422, -0.057_843_349_402_844_31, -0.051_472_814_347_068_865, -0.043_181_884_298_548_606, -0.021_292_272_925_791_663, -0.008_801_491_031_785_446, -0.000_176_883_129_551_719_93, 0.005_667_994_002_560_923, 0.014_988_357_095_947_771, 0.021_945_462_670_233_173, 0.043_658_011_394_191_336, 0.073_558_257_697_291_4, 0.095_691_231_096_234_09, 0.125_004_990_364_917_6, 0.196_744_929_605_098_62],
    [-0.470_827_178_148_350_6, -0.215_239_480_373_636_08, -0.080_407_549_960_386_64, -0.074_037_969_544_156_4, -0.056_633_523_745_925_764, -0.049_874_830_057_372_5, -0.030_088_183_551_282_89, -0.014_633_401_331_514_978, -0.005_406_007_319_419_953, 0.006_309_742_055_498_549_5, 0.017_303_941_296_059_777, 0.028_889_341_142_091_046, 0.048_621_407_144_227_975, 0.074_081_053_123_334_73, 0.089_060_996_117_487_65, 0.116_678_043_107_674_67, 0.196_744_929_605_098_62],
    [-0.470_827_178_148_350_6, -0.268_939_547_641_480_37, -0.218_259_646_066_312_26, -0.103_907_131_947_428_9, -0.077_627_712_985_573_11, -0.059_464_888_961_992_95, -0.046_053_465_126_745_64, -0.014_633_182_233_400_715, 0.000_959_970_196_797_147_5, 0.012_300_740_310_164_723, 0.028_807_902_245_369_343, 0.040_683_645_567_803_57, 0.058_422_020_180_810_864, 0.074_251_402_158_661_18, 0.089_959_679_197_093_05, 0.113_368_745_397_122, 0.196_744_929_605_098_62],
    [-0.465_909_236_902_975_53, -0.302_051_145_999_406_13, -0.247_771_429_141_375_76, -0.161_745_900_519_838_45, -0.102_809_480_613_693_55, -0.063_357_962_205_616_76, -0.030_193_417_484_274_822, -0.005_707_285_571_415_832_5, 0.011_580_115_082_708_913, 0.024_399_993_074_698_28, 0.034_787_820_653_605_264, 0.044_353_242_994_912_93, 0.058_306_388_948_827_29, 0.071_888_715_672_290_05, 0.081_593_972_731_329_18, 0.104_550_128_577_927_96, 0.180_255_205_835_654_05],
    [-0.407_346_286_778_211_16, -0.297_543_371_862_755_53, -0.237_348_625_574_479_36, -0.153_417_286_925_218_28, -0.108_796_209_760_765_1, -0.071_246_862_056_884_31, -0.020_109_139_300_719_687, 0.000_914_613_455_170_747_7, 0.013_444_906_699_842_323, 0.024_983_836_077_536_823, 0.032_127_841_556_361_066, 0.038_659_434_087_053_62, 0.044_440_701_002_433_78, 0.058_219_969_032_561_91, 0.071_176_797_782_353_83, 0.085_294_597_366_475_68, 0.153_391_502_481_718],
    [-0.387_522_972_513_074_2, -0.168_399_899_410_339_1, -0.130_318_570_955_218_62, -0.103_909_983_765_896_32, -0.082_221_382_854_536_91, -0.068_183_417_978_083_93, -0.043_555_204_951_819_06, -0.021_440_211_102_394_652, -0.005_234_638_505_611_557, 0.003_871_504_740_015_378, 0.019_921_778_463_398_02, 0.027_519_605_771_402_098, 0.033_243_580_627_093_85, 0.040_653_615_886_315_275, 0.047_987_978_648_108_415, 0.069_638_347_103_021_96, 0.153_890_366_398_135_3],
    [-0.307_881_872_406_567_56, -0.124_903_339_325_375_03, -0.091_676_139_112_662_12, -0.075_625_172_308_037_67, -0.056_305_013_476_277_886, -0.039_949_968_390_068_29, -0.026_520_016_219_866_38, -0.016_255_426_503_099_45, -0.005_391_986_586_597_779, 0.002_153_840_270_839_788, 0.010_297_961_885_139_456, 0.017_691_091_500_307_787, 0.027_692_839_843_209_884, 0.036_930_021_160_812_836, 0.043_620_367_290_435_974, 0.056_688_257_926_038_59, 0.153_890_366_398_135_3],
    [-0.174_820_804_524_706_39, -0.089_201_450_833_329_53, -0.057_973_518_834_390_066, -0.043_530_118_053_851_166, -0.031_270_307_721_343_59, -0.020_272_816_001_553_867, -0.013_678_149_692_212_25, -0.005_408_379_789_269_278, 0.002_286_009_282_110_205_7, 0.008_042_808_046_317_242, 0.012_346_102_836_935_553, 0.021_506_204_842_660_58, 0.030_771_149_277_328_835, 0.038_369_861_321_744_3, 0.045_141_693_000_474_41, 0.056_438_750_741_443_78, 0.161_919_857_303_427_84],
    [-0.132_169_208_379_102_18, -0.050_590_467_287_014_72, -0.032_245_329_057_072_66, -0.018_173_226_690_780_395, -0.013_118_225_848_539_647, -0.004_737_433_947_458_018, 0.002_576_981_393_826_112_2, 0.007_700_368_441_020_647, 0.012_283_111_786_926_707, 0.019_702_148_645_084_11, 0.029_590_193_440_943_238, 0.034_586_502_726_482_574, 0.039_599_482_079_450_105, 0.042_391_397_910_793_216, 0.053_588_930_898_218_395, 0.069_982_234_130_023_04, 0.172_804_662_890_106_26],
    [-0.051_560_461_538_786_61, -0.016_272_566_225_561_735, 0.000_209_675_895_412_925_33, 0.005_894_282_525_619_592, 0.014_824_443_424_047_115, 0.021_806_209_452_857_085, 0.029_953_789_781_614_1, 0.034_609_402_267_836_61, 0.037_444_366_038_822_92, 0.039_801_079_242_660_55, 0.043_764_503_707_303_046, 0.052_404_933_196_314_854, 0.066_183_917_754_827_18, 0.076_543_475_331_668_94, 0.080_468_307_755_398_32, 0.093_326_617_931_081_46, 0.210_476_976_043_761_1],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
    [-1.017_007_289_386_898, -0.784_407_278_168_238_9, -0.586_964_505_850_160_5, -0.505_279_039_345_910_6, -0.366_942_889_011_930_95, -0.199_296_280_784_577_56, -0.157_926_384_998_208_46, -0.032_003_617_568_577_18, 0.010_873_753_570_405_462, 0.052_084_903_998_548_28, 0.084_120_457_578_590_52, 0.130_428_236_796_780_78, 0.153_924_641_987_338_38, 0.181_181_352_063_179_92, 0.212_547_608_327_566_14, 0.230_037_013_230_076_44, 0.344_541_006_473_043_86],
    [-1.017_007_289_386_898, -0.825_966_686_435_532_1, -0.693_575_155_612_970_3, -0.492_409_806_186_612_45, -0.311_862_750_000_424_96, -0.193_669_036_435_590_7, -0.132_766_219_405_946, -0.033_987_793_229_822_51, 0.033_121_415_446_210_02, 0.079_340_992_646_431_16, 0.106_537_369_964_253_7, 0.141_439_918_952_420_42, 0.157_431_380_210_059_87, 0.185_732_425_619_508_98, 0.224_091_666_239_896_43, 0.249_394_476_071_053_67, 0.375_907_771_816_091_7],
    [-1.049_581_186_459_059_6, -0.809_762_661_099_030_3, -0.705_526_602_733_234_4, -0.493_212_568_084_550_44, -0.364_495_809_735_301_16, -0.235_591_913_086_295_12, -0.189_287_021_574_630_37, -0.112_444_390_511_884_28, -0.034_157_590_819_512_28, 0.078_511_358_350_073_97, 0.106_410_904_172_812_71, 0.142_549_853_762_477_09, 0.157_424_436_836_591_88, 0.181_524_911_652_335_6, 0.238_489_337_055_139_08, 0.282_596_752_703_592_97, 0.375_907_771_816_091_7],
    [-1.049_581_186_459_059_6, -0.769_005_723_351_054_5, -0.614_328_809_415_631_1, -0.494_776_284_006_401_8, -0.417_268_818_534_918_3, -0.292_180_483_426_000_8, -0.193_579_556_212_564_37, -0.113_983_623_968_218_83, -0.049_378_873_357_479_085, 0.083_907_891_955_398_68, 0.108_630_787_696_653_95, 0.145_302_472_855_685_5, 0.171_749_172_240_029_9, 0.204_730_807_778_461_8, 0.241_555_605_673_807_35, 0.303_082_348_050_123_63, 0.375_907_771_816_091_7],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
    [-1.540_286_842_326_198_2, -0.669_255_957_000_627, -0.480_451_270_556_581_93, -0.332_741_918_846_319, -0.227_475_836_729_693_18, -0.151_007_911_087_373_16, -0.095_386_693_741_309_1, -0.045_029_934_725_491_97, 0.000_330_746_385_637_303_9, 0.036_988_309_172_969_316, 0.071_034_009_827_970_85, 0.101_310_211_546_776_73, 0.131_894_225_692_659_46, 0.157_954_917_891_016_93, 0.187_804_874_842_523_98, 0.227_885_202_396_681_84, 0.375_907_771_816_091_7],
    [-1.140_524_638_885_287, -0.752_927_600_071_990_2, -0.503_556_192_180_691_3, -0.409_402_254_628_596_3, -0.238_585_489_344_220_75, -0.174_311_581_220_940_43, -0.089_555_128_455_010_35, -0.018_316_361_468_230_55, 0.018_522_642_489_686_558, 0.053_609_708_884_305_114, 0.067_223_659_867_843_19, 0.082_764_129_016_246_48, 0.101_558_501_400_984_37, 0.113_683_656_682_968_52, 0.150_558_149_083_571_82, 0.172_259_763_955_011_22, 0.214_700_644_753_937_9],
    [-1.540_286_842_326_198_2, -0.757_652_236_187_902_1, -0.525_163_979_346_277_9, -0.317_116_537_442_594_94, -0.198_129_342_555_523_56, -0.122_184_565_191_488_29, -0.077_027_771_926_018_55, -0.020_153_471_662_186_284, 0.004_261_236_874_497_687, 0.048_184_055_107_197_38, 0.066_647_667_024_128_86, 0.080_112_561_570_328_69, 0.101_221_658_836_857_81, 0.132_249_329_391_269_57, 0.159_084_516_712_528_7, 0.178_181_413_592_895_4, 0.287_205_327_720_242_2],
    [-1.540_286_842_326_198_2, -0.720_563_396_602_297_4, -0.437_766_625_183_190_9, -0.240_872_819_772_253_08, -0.154_439_338_525_534_57, -0.096_408_668_595_496_03, -0.052_007_529_921_435_564, -0.016_055_428_595_872_445, 0.005_935_240_098_430_38, 0.042_109_907_830_330_896, 0.072_788_571_409_110_88, 0.096_804_396_066_658_79, 0.122_205_994_629_587_28, 0.154_471_561_224_663_68, 0.178_138_067_148_758_38, 0.196_018_499_739_176_15, 0.287_205_327_720_242_2],
    [-1.540_286_842_326_198_2, -0.582_550_865_490_933, -0.384_060_184_975_659, -0.251_265_837_225_397_75, -0.165_261_818_325_999_45, -0.111_806_207_052_386_36, -0.064_133_946_600_068_49, -0.021_580_301_116_757_26, 0.006_926_720_295_568_612, 0.037_024_012_134_588_15, 0.070_833_466_039_496_93, 0.100_142_895_045_917_79, 0.134_742_158_899_747_9, 0.163_285_448_573_985_8, 0.189_012_853_805_126_64, 0.213_191_077_478_225_27, 0.287_205_327_720_242_2],
    [-1.540_286_842_326_198_2, -0.575_321_715_551_517_8, -0.394_472_281_447_896_03, -0.275_621_335_248_143, -0.193_799_669_769_452_95, -0.136_769_174_234_311_56, -0.085_681_653_671_392_26, -0.045_993_823_322_393_01, -0.004_647_955_997_916_199, 0.030_051_999_047_213_975, 0.061_603_742_132_288_41, 0.095_252_671_585_015_81, 0.125_632_086_852_262_65, 0.153_694_805_202, 0.187_855_918_102_673_32, 0.227_700_199_903_365_2, 0.287_205_327_720_242_2],
    [-1.233_447_822_673_913, -0.585_059_077_997_152_4, -0.424_798_941_816_244_34, -0.287_383_910_282_580_1, -0.223_751_480_346_800_42, -0.162_916_450_235_863_04, -0.117_630_838_923_557_05, -0.076_630_277_641_728_78, -0.021_908_577_602_664_018, 0.022_501_487_647_427_793, 0.051_525_915_812_809_575, 0.083_792_519_713_016_46, 0.114_693_772_151_887_92, 0.142_196_249_276_144_1, 0.177_948_284_966_839_65, 0.228_073_247_381_024_02, 0.272_313_820_761_411_3],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_27: [(f64, f64); 11] = [
    (0.0, 1.0),
    (2.911_927_013_371_116_6, 5.402_890_976_862_883),
    (-10.439_976_391_499_203, -7.440_464_694_468_909),
    (-12.0, -8.5),
    (-9.955_942_377_347_57, -7.350_440_549_816_761),
    (-0.274_201_077_592_644_3, 1.817_844_483_274_783),
    (-1.0, 0.1),
    (1.731_290_787_190_484_1, 6.872_822_726_207_732),
    (0.388_434_716_439_335_75, 1.367_497_311_254_042),
    (-1.400_532_894_803_024_5, 0.159_532_975_905_228_64),
    (0.179_465_534_110_166_72, 0.906_585_411_832_142_6),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_27: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.728_760_543_817_864_5, 0.728_760_543_817_864_5, 0.874_025_750_676_169_5, 0.966_679_881_676_364_4, 1.019_845_765_024_508_5, 1.113_982_155_001_448_1, 1.138_587_425_499_78, 1.079_739_706_127_723, 1.083_125_942_709_871_2, 1.083_190_156_396_994_6, 1.097_339_197_889_934, 1.184_595_156_014_553_6, 1.222_339_270_875_62, 1.213_585_059_957_901, 1.208_602_477_805_944, 1.215_534_163_028_289_9, 1.219_145_495_413_593],
    [0.338_723_370_629_424_26, 0.321_138_572_144_510_47, 0.335_179_770_682_943_1, 0.377_098_798_083_426_2, 0.887_756_149_938_139_4, 0.837_771_515_595_080_5, 0.837_771_515_595_080_5, 0.831_881_677_352_501_8, 0.823_823_501_786_154_7, 0.817_628_925_205_675_4, 0.792_794_489_002_250_4, 1.265_953_364_068_304_7, 1.310_148_354_787_116, 1.277_511_049_835_092_3, 0.748_672_252_335_117_6, 0.748_672_252_335_117_6, 0.795_684_549_502_532_6],
    [0.0, -0.043_75, -0.087_5, -0.131_249_999_999_999_98, -0.175, -0.218_75, -0.262_499_999_999_999_96, -0.306_249_999_999_999_97, -0.35, -0.393_75, -0.437_5, -0.481_249_999_999_999_96, -0.524_999_999_999_999_9, -0.568_75, -0.612_499_999_999_999_9, -0.656_25, -0.7],
    [2.264_018_743_313_535, 2.264_018_743_313_535, 2.264_018_743_313_535, 2.200_825_467_922_642, 2.097_251_936_553_146, 2.016_888_011_168_784, 1.991_276_420_981_236_4, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5, 1.986_575_511_859_586_5],
    [-1.222_530_261_152_061_6, -1.224_030_251_234_335_3, -1.044_926_587_101_387_3, -0.862_099_178_190_120_7, -0.673_132_016_407_244_8, -0.577_574_616_338_212_9, -0.512_622_863_568_628, -0.134_317_249_977_353_64, -0.083_526_647_407_097_17, -0.056_478_632_566_960_58, -0.026_748_183_556_663_34, -0.005_933_772_277_289_671, 0.010_885_784_484_074_491, -0.028_293_511_486_494_723, -0.199_715_830_125_514_78, -0.368_817_795_718_794_2, -0.424_231_443_110_968_94],
    [0.0, -0.137_5, -0.275, -0.412_500_000_000_000_03, -0.55, -0.687_5, -0.825_000_000_000_000_1, -0.962_500_000_000_000_1, -1.1, -1.237_5, -1.375, -1.512_500_000_000_000_2, -1.650_000_000_000_000_1, -1.787_5, -1.925_000_000_000_000_3, -2.062_5, -2.2],
    [-0.395_468_357_708_967_4, -0.444_998_358_963_543_34, -0.535_110_717_039_772_9, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3, -0.614_305_977_546_446_3],
    [0.179_623_256_088_098_9, 0.234_323_516_329_352_67, 0.303_224_438_071_970_47, 0.365_588_366_816_305_3, 0.421_056_986_388_074_3, 0.480_089_674_715_282_85, 0.500_350_700_010_247_3, 0.515_031_517_796_785_8, 0.609_883_073_427_974_9, 0.675_062_727_031_004_9, 0.725_496_234_972_103_5, 0.763_581_942_664_629_3, 0.787_989_710_298_530_4, 0.824_185_173_349_591_7, 0.851_509_766_408_007_2, 0.864_544_837_458_762_2, 0.873_845_895_434_084],
    [-0.081_787_423_602_021_44, -0.155_410_975_739_351_95, -0.155_410_975_739_351_95, -0.124_959_045_807_360_07, -0.103_879_619_222_985_01, -0.067_967_773_664_604_74, -0.065_453_474_642_711_96, -0.065_453_474_642_711_96, -0.065_126_189_815_652_66, -0.053_645_777_993_428_984, -0.009_102_918_706_985_996, 0.013_093_735_767_136_834, 0.007_130_864_878_018_162, 0.013_965_758_726_303_894, 0.023_393_791_018_860_043, 0.039_835_340_451_457_77, 0.079_040_061_247_257_93],
    [9.871_334_428_950_114, 9.775_010_643_975_232, 9.767_324_009_113_468, 9.754_394_299_740_573, 9.754_394_299_740_573, 9.768_794_233_097_442, 9.754_394_299_740_573, 9.640_342_150_781_256, 9.583_072_291_390_502, 9.732_929_397_717_832, 9.898_802_286_780_102, 9.869_382_734_324_74, 9.804_783_401_482_545, 9.799_224_073_284_25, 9.780_041_995_153_912, 9.759_513_722_211_704, 9.732_724_739_182_972],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_27: [(f64, f64); 11] = [
    (2.535_617_635_715_720_7, 5.417_670_438_063_333),
    (0.354_130_180_715_045, 1.370_524_574_250_195_5),
    (-0.326_339_540_334_136_9, 1.930_396_850_291_960_2),
    (-1.045_757, 0.038_753),
    (1.721_345_212_712_888_3, 7.145_041_221_830_653),
    (-1.472_492_710_800_363_6, 0.363_524_866_293_374_1),
    (-2.301_03, -0.698_97),
    (-0.705_408_242_771_589_6, 0.336_777_716_470_267_6),
    (0.027_544_719_217_018_96, 0.911_945_639_038_323_8),
    (-0.536_091_065_693_653_5, 0.366_898_741_003_575_17),
    (8.241_614_998_553_404, 10.078_348_360_620_978),
];

/// The fast accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_DONOR_KINDS_27: [f64; 3] = [
    0.980_904_498_232_842_6,
    0.0,
    0.019_095_501_767_157_345,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16 over all their
/// members, in [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_27: [[f64; 17]; 11] = [
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [-0.729_532_620_194_650_3, -0.206_690_742_484_923_58, -0.173_042_240_815_100_2, -0.142_881_235_221_654_1, -0.073_992_286_998_388_85, -0.039_458_387_993_266_87, -0.021_556_170_674_067_805, -0.008_964_333_584_765_42, 0.007_094_649_833_911_942, 0.028_275_370_284_516_866, 0.052_018_750_253_496_49, 0.078_668_957_602_014_49, 0.110_545_205_243_562_05, 0.177_751_792_221_344_85, 0.316_784_825_345_736_15, 0.445_263_055_879_100_76, 0.814_895_666_842_071_5],
    [-0.534_898_263_095_038, -0.226_719_069_399_036_94, -0.109_423_275_025_768_67, -0.043_022_815_861_665_79, -0.024_534_392_102_111_814, -0.015_679_493_346_419_75, -0.010_181_947_623_565_368, -0.006_191_139_981_705_319, -0.001_767_755_751_419_185_7, 0.002_982_843_963_828_824_7, 0.009_885_867_484_461_656, 0.019_363_302_299_285_255, 0.028_161_785_261_169_123, 0.045_574_913_095_788_82, 0.076_774_061_270_250_72, 0.106_825_182_080_580_49, 2.153_023_501_587_720_4],
    [-0.490_888_733_893_869_1, -0.021_537_941_775_717_325, -0.017_689_289_912_642_618, -0.014_398_090_265_244_242, -0.010_999_419_195_043_492, -0.008_392_084_282_923_373, -0.006_401_568_745_904_121, -0.004_540_818_810_977_147, -0.002_743_730_869_208_880_4, -0.000_772_270_420_410_371_4, 0.001_096_124_036_146_601_3, 0.002_780_452_186_920_403, 0.005_684_119_546_213_358, 0.010_547_445_591_465_945, 0.015_585_463_591_815_32, 0.020_790_904_208_113_89, 0.397_401_292_196_596_7],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-1.173_874_980_003_347, -0.346_133_034_963_822_53, -0.074_890_200_389_494_65, -0.050_761_360_474_842, -0.036_837_419_860_487_34, -0.024_791_271_597_626_598, -0.014_434_699_185_801_453, -0.005_214_292_228_810_75, 0.003_342_846_203_660_511_6, 0.017_068_299_305_687_38, 0.045_939_192_434_348_65, 0.069_601_833_324_414_29, 0.104_700_237_609_753_5, 0.374_331_060_253_482_4, 0.574_997_949_050_993_9, 0.714_737_095_116_894_3, 0.863_362_668_478_674_3],
    [-0.101_628_726_896_669_35, -0.017_422_748_226_931_985, -0.011_262_311_182_724_105, -0.006_128_685_688_076_772, -0.002_865_554_424_463_768, 0.000_472_398_212_179_861_96, 0.003_028_642_751_744_502, 0.004_498_860_245_205_296, 0.005_245_696_871_359_235, 0.005_938_314_578_067_467, 0.006_621_210_861_221_538, 0.007_247_682_767_408_94, 0.007_824_439_447_365_826, 0.008_555_968_022_394_217, 0.009_238_569_185_135_088, 0.010_296_405_345_278_774, 2.260_931_126_371_733_3],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.565_580_984_052_321_2, -0.184_179_758_977_524_14, -0.072_853_834_054_344_7, -0.047_224_377_590_130_62, -0.030_884_975_494_956_397, -0.018_973_757_901_599_088, -0.008_933_725_120_551_166, -0.000_877_875_462_674_446_1, 0.006_534_611_620_824_800_5, 0.012_936_190_811_234_798, 0.018_669_948_422_597_837, 0.023_571_606_661_438_458, 0.028_650_055_145_827_09, 0.034_721_882_215_767_86, 0.044_675_096_661_218_994, 0.056_313_144_061_620_87, 0.218_850_858_559_235_92],
    [-0.763_936_359_946_104_4, -0.456_088_758_000_637_36, -0.316_914_064_305_417_4, -0.223_172_177_510_602_56, -0.161_660_649_584_976_97, -0.111_418_343_523_844_11, -0.067_006_602_950_714_12, -0.026_440_713_125_521_164, 0.008_181_581_095_118_56, 0.038_542_807_257_451_295, 0.069_457_704_846_511_02, 0.105_154_531_317_939_3, 0.145_153_909_099_452_06, 0.199_046_826_202_895_96, 0.261_666_818_409_800_6, 0.359_155_985_613_504_1, 0.982_132_001_672_724_9],
    [-1.855_448_171_412_975_4, -0.404_579_896_461_517_93, -0.273_425_794_689_547_25, -0.185_337_806_702_439_5, -0.129_753_479_549_216_32, -0.087_187_396_621_023_65, -0.053_634_522_764_414_765, -0.024_511_215_254_099_97, 0.001_535_749_154_479_902_3, 0.024_136_565_623_336_766, 0.043_866_327_360_907_606, 0.063_005_276_249_712_8, 0.080_399_707_353_446_97, 0.096_017_471_863_869_3, 0.111_576_531_360_755_88, 0.138_981_588_211_822_66, 0.224_440_489_999_244_26],
];

/// The slow accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_SLOW_NODE_RESIDUALS_27: [[f64; 17]; 187] = [
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [5.419_747_950_083_047, 6.155_500_615_369_866, 6.535_647_011_745_884, 6.735_072_182_909_636, 6.841_677_364_757_329, 6.924_241_092_726_48, 7.013_390_886_332_059, 7.211_289_920_296_163, 7.589_810_799_085_348, 7.856_728_679_661_605, 8.047_580_582_858_606, 8.213_780_271_298_507, 8.359_964_630_806_95, 8.499_737_656_676_285, 8.696_405_245_582_29, 10.0, 10.0],
    [-0.713_712_216_935_975_2, -0.408_661_879_200_613_3, -0.248_407_355_194_106_64, -0.095_152_431_005_581_16, -0.016_137_883_113_174_246, 0.012_854_127_558_768_665, 0.032_530_031_696_302_62, 0.045_336_122_336_874_986, 0.054_800_490_763_082_8, 0.065_473_637_169_420_28, 0.073_712_952_698_098_88, 0.080_007_345_283_539_07, 0.090_488_378_464_924_38, 0.102_278_736_391_182_36, 0.119_686_894_660_222_17, 0.171_097_699_090_519_92, 0.443_515_604_612_393_14],
    [-0.713_712_216_935_975_2, -0.356_585_870_164_715_8, -0.199_674_160_446_369_04, -0.047_551_908_637_546_714, -0.002_237_029_862_897_522_3, 0.015_125_783_399_762_619, 0.031_383_336_148_664_11, 0.043_030_391_818_382, 0.053_216_419_540_640_32, 0.064_090_362_264_301_23, 0.072_836_065_108_113_94, 0.080_403_102_019_435, 0.092_278_419_159_486_97, 0.107_571_421_301_317_62, 0.132_245_890_317_760_7, 0.210_235_299_493_944_04, 0.674_598_988_906_288_8],
    [-0.552_735_787_153_843_2, -0.316_918_942_486_954_9, -0.157_855_553_092_23, -0.043_460_297_548_006_246, 0.000_369_474_302_262_333_9, 0.020_476_072_027_786_788, 0.037_780_882_731_164_99, 0.051_477_225_141_028_49, 0.065_350_520_696_899_35, 0.076_984_479_304_125_66, 0.089_168_606_339_917_12, 0.106_359_992_170_344_57, 0.125_016_041_999_117_5, 0.152_757_469_158_248_4, 0.190_369_058_082_956_43, 0.309_573_547_918_612_7, 0.771_468_054_148_814_3],
    [-0.551_014_871_444_087, -0.192_251_106_943_366_25, -0.085_055_044_036_179_11, -0.032_524_589_681_551_884, -0.015_626_891_953_594_92, -0.001_971_540_463_245_888, 0.013_878_705_041_945_947, 0.030_603_489_370_818_197, 0.047_961_117_021_789_61, 0.066_228_288_229_470_34, 0.084_199_327_196_749_26, 0.105_072_064_621_133_75, 0.122_405_645_984_202_34, 0.152_659_938_480_746_3, 0.189_579_534_576_759_05, 0.351_322_193_534_674_53, 0.811_207_877_061_456_4],
    [-0.551_014_871_444_087, -0.178_878_827_762_241_54, -0.154_409_483_194_157_04, -0.114_684_082_389_897_79, -0.034_442_967_629_181_864, -0.019_995_112_933_356_603, -0.012_489_123_735_885_564, -0.004_840_452_773_835_08, 0.005_569_175_010_468_105_5, 0.018_415_918_323_883_094, 0.033_594_040_335_022_72, 0.052_751_199_024_990_29, 0.076_448_443_782_395_22, 0.105_100_612_058_404, 0.140_896_166_681_803_66, 0.337_636_172_029_129_6, 0.814_895_666_842_071_5],
    [-0.323_418_986_819_410_1, -0.186_919_589_597_408_87, -0.169_593_959_005_133_22, -0.155_098_131_872_425_52, -0.139_825_742_795_661_54, -0.105_673_565_797_781_25, -0.024_017_865_358_725_517, -0.015_463_013_447_223_884, -0.009_056_847_944_243_421, -0.000_649_484_649_812_203_9, 0.009_367_008_043_886_524, 0.025_108_614_883_385_84, 0.044_358_956_035_968_7, 0.074_579_327_257_384_25, 0.125_449_705_492_279_68, 0.512_736_631_473_307_7, 0.814_895_666_842_071_5],
    [-0.295_484_231_862_876_5, -0.196_646_826_637_361_5, -0.178_633_170_546_580_26, -0.165_571_186_002_245_24, -0.152_460_309_662_731_94, -0.133_266_879_847_818_63, -0.093_796_359_859_326_43, -0.022_299_679_874_416_705, -0.011_919_018_345_794_56, -0.000_969_540_811_518_755_2, 0.015_074_857_494_074_756, 0.043_835_375_100_343_806, 0.100_280_331_644_096_36, 0.342_874_140_455_926_85, 0.506_012_462_210_711_1, 0.622_893_322_920_116_7, 0.814_895_666_842_071_5],
    [-0.295_484_231_862_876_5, -0.229_283_970_012_444_77, -0.194_748_021_311_424_24, -0.163_130_732_798_143_1, -0.096_847_286_798_234_07, -0.070_620_431_100_360_36, -0.047_492_167_141_229_8, -0.015_969_225_545_511_99, 0.024_308_853_193_876_233, 0.094_385_589_235_882_62, 0.246_443_924_263_679_34, 0.340_963_992_142_616_1, 0.416_550_129_670_020_7, 0.468_094_115_133_718_46, 0.533_006_663_442_944_2, 0.602_656_765_859_103_7, 0.814_895_666_842_071_5],
    [-0.295_484_231_862_876_5, -0.225_080_291_661_953_98, -0.198_435_744_266_422_96, -0.143_251_526_497_627_6, -0.075_823_038_194_204_16, -0.057_836_877_866_888_875, -0.041_016_722_602_022_5, -0.020_189_248_171_832_184, 0.015_296_573_379_672_202, 0.067_645_263_444_237_46, 0.157_240_167_118_649_4, 0.273_031_035_060_961_3, 0.350_700_671_739_657_74, 0.411_159_001_034_437_14, 0.458_338_222_373_259_5, 0.523_401_119_193_668_2, 0.730_709_122_256_984_5],
    [-0.295_484_231_862_876_5, -0.208_613_140_206_920_48, -0.181_237_866_402_703_26, -0.136_869_724_732_186_67, -0.062_013_804_449_403_846, -0.048_026_848_675_725_41, -0.035_732_376_380_678_58, -0.020_358_449_415_569_576, 0.001_008_958_135_575_300_2, 0.035_662_022_059_017_154, 0.071_947_001_835_554_19, 0.113_225_457_082_179_33, 0.233_290_586_831_020_5, 0.314_037_752_493_754_4, 0.388_418_476_676_158_7, 0.454_247_816_537_289_63, 0.732_153_983_447_158_2],
    [-0.272_423_985_249_528_5, -0.191_839_294_227_115_07, -0.164_333_313_886_850_57, -0.125_655_116_066_993_12, -0.049_315_156_362_423_666, -0.038_172_390_874_657_146, -0.028_712_534_617_693_415, -0.016_907_050_745_858_812, -0.002_794_365_353_165_522_5, 0.020_829_153_618_907_668, 0.045_015_299_765_855_964, 0.070_767_904_508_288_1, 0.100_504_330_891_596_02, 0.137_923_794_833_378_88, 0.272_180_571_151_092_57, 0.368_370_783_973_486_16, 0.732_153_983_447_158_2],
    [-0.255_061_002_940_379_14, -0.176_746_523_354_662_68, -0.149_798_442_911_099_5, -0.061_312_620_530_375_186, -0.041_297_091_226_840_64, -0.032_727_855_701_554_49, -0.023_848_442_312_570_04, -0.014_142_321_572_358_299, -0.003_174_062_096_968_652, 0.012_979_899_748_803_866, 0.032_825_475_675_233_85, 0.052_503_101_840_110_19, 0.073_340_580_749_196_9, 0.097_223_445_787_728_62, 0.126_872_610_655_840_13, 0.242_662_082_719_602_9, 0.732_153_983_447_158_2],
    [-0.230_034_572_396_974_34, -0.162_621_411_424_143_44, -0.136_628_856_998_077_95, -0.046_582_365_963_581_01, -0.035_874_979_407_434_414, -0.028_555_699_679_154_68, -0.020_642_889_665_771_474, -0.011_462_457_346_105_87, -0.000_218_922_534_932_528_2, 0.011_644_025_926_792_728, 0.026_153_404_655_310_446, 0.043_648_439_963_047_4, 0.060_850_070_758_977_005, 0.079_079_486_907_219_79, 0.103_059_350_218_143_65, 0.133_209_214_679_820_44, 0.732_153_983_447_158_2],
    [-0.205_158_306_972_278_44, -0.148_457_713_640_387_26, -0.126_657_278_950_782_83, -0.039_429_236_225_117_834, -0.030_353_674_852_258_625, -0.023_307_005_923_460_525, -0.015_034_435_979_432_73, -0.003_589_001_864_518_578, 0.002_293_099_847_577_647_7, 0.013_013_564_533_217_099, 0.024_912_963_542_082_47, 0.038_204_140_538_898_79, 0.055_739_912_405_929_42, 0.070_943_045_965_900_37, 0.089_178_340_187_178_43, 0.125_829_670_771_358_15, 0.598_813_272_214_879],
    [-0.729_532_620_194_650_3, -0.206_690_742_484_923_58, -0.173_042_240_815_100_2, -0.142_881_235_221_654_1, -0.073_992_286_998_388_85, -0.039_458_387_993_266_87, -0.021_556_170_674_067_805, -0.008_964_333_584_765_42, 0.007_094_649_833_911_942, 0.028_275_370_284_516_866, 0.052_018_750_253_496_49, 0.078_668_957_602_014_49, 0.110_545_205_243_562_05, 0.177_751_792_221_344_85, 0.316_784_825_345_736_15, 0.445_263_055_879_100_76, 0.814_895_666_842_071_5],
    [-0.729_532_620_194_650_3, -0.303_509_421_595_196_56, -0.258_952_959_402_329_54, -0.193_322_038_588_229_44, -0.137_417_998_479_472_42, -0.091_189_068_365_624_75, -0.054_731_014_613_038_88, -0.026_156_439_023_273_137, -0.000_695_182_921_019_109_2, 0.027_316_470_386_129_63, 0.071_618_455_991_478_69, 0.127_336_429_215_270_33, 0.203_918_010_192_898_3, 0.272_653_038_996_789_33, 0.322_208_654_417_467_5, 0.364_136_106_887_544_8, 0.598_813_272_214_879],
    [-0.729_532_620_194_650_3, -0.301_910_629_707_549_8, -0.256_984_554_187_652_3, -0.192_158_734_233_415_78, -0.136_528_203_198_926_78, -0.090_009_232_943_918_78, -0.053_870_433_993_536_214, -0.024_990_623_413_285_583, 6.886_167_844_169_039e-5, 0.028_186_074_813_376_535, 0.073_242_608_749_221_26, 0.130_593_231_473_831, 0.206_141_640_336_491_43, 0.273_436_890_901_868_7, 0.322_928_662_250_005_44, 0.364_134_427_830_059_84, 0.397_405_583_874_993_5],
    [-0.175_529_599_382_815_62, -0.008_978_246_377_683_807, -0.003_297_435_192_311_846, -0.000_918_714_121_074_623_9, 0.001_339_036_517_059_959_6, 0.003_266_195_831_237_079_6, 0.005_575_379_230_122_746_5, 0.008_293_635_989_835_076, 0.011_740_207_708_984_63, 0.020_053_084_001_134_88, 0.024_823_568_158_293_466, 0.028_363_942_780_073_145, 0.031_086_543_163_999_694, 0.033_959_638_744_718_85, 0.037_580_535_339_508_975, 0.043_143_965_325_244_74, 1.240_837_665_648_873_3],
    [-0.175_529_599_382_815_62, -0.015_378_236_838_458_277, -0.007_898_554_995_170_275, -0.004_661_430_150_865_351, -0.002_480_353_295_314_396_4, -0.000_367_883_411_200_027_4, 0.001_915_406_933_393_239_8, 0.003_989_510_896_225_641, 0.006_398_444_192_183_013, 0.009_728_562_518_341_704, 0.016_565_933_530_229_757, 0.021_585_316_774_038_3, 0.025_386_658_692_204_998, 0.028_830_185_735_242_885, 0.032_092_656_039_049_684, 0.036_595_984_557_447_26, 1.240_837_665_648_873_3],
    [-0.175_529_599_382_815_62, -0.020_521_072_024_758_735, -0.012_486_290_135_668_989, -0.008_515_329_498_251_683, -0.006_152_121_764_829_729, -0.004_098_991_217_299_018, -0.002_229_805_984_445_618_4, -0.000_202_853_403_587_764_57, 0.002_172_136_272_789_418_5, 0.004_822_349_326_678_008_5, 0.007_809_051_476_108_244, 0.013_824_784_900_950_965, 0.018_854_374_192_117_84, 0.022_916_120_476_689_85, 0.027_221_960_385_904_045, 0.032_054_499_596_089_91, 1.240_837_665_648_873_3],
    [-0.175_529_599_382_815_62, -0.023_377_219_996_998_46, -0.017_586_622_933_827_675, -0.012_014_726_628_676_505, -0.009_513_213_514_288_249, -0.007_568_386_586_337_75, -0.005_849_205_195_160_505, -0.004_073_971_230_094_557, -0.002_058_897_071_548_512, 0.000_274_585_251_011_542_43, 0.002_960_684_742_148_875_7, 0.006_362_583_527_748_29, 0.012_252_904_458_090_82, 0.018_145_449_279_052_206, 0.022_389_470_693_015_73, 0.027_551_674_793_656_12, 1.240_837_665_648_873_3],
    [-0.131_078_342_429_008_8, -0.026_404_599_020_608_113, -0.021_576_594_758_696_003, -0.015_815_695_641_871_665, -0.012_085_762_540_465_712, -0.010_271_360_145_989_05, -0.008_584_595_076_233_714, -0.006_872_732_302_140_11, -0.005_101_544_895_217_968, -0.002_833_648_875_216_33, -0.000_131_385_446_444_582_04, 0.002_581_066_480_830_188_5, 0.007_005_989_545_722_052, 0.013_697_310_365_278_832, 0.019_383_352_404_353_925, 0.025_108_251_430_629_343, 1.493_690_812_731_555_8],
    [-0.131_078_342_429_008_8, -0.027_725_836_496_744_996, -0.023_589_157_753_710_704, -0.019_212_689_729_912_236, -0.014_098_367_153_834_238, -0.011_784_844_426_746_233, -0.010_351_649_333_258_88, -0.008_772_423_066_040_569, -0.006_841_962_000_194_808, -0.004_636_068_829_326_398_5, -0.001_217_269_940_752_938, 0.001_537_989_518_315_807_2, 0.005_245_728_976_360_804_6, 0.011_800_278_447_446_756, 0.018_365_639_419_236_86, 0.024_201_914_480_256_766, 1.493_690_812_731_555_8],
    [-0.131_078_342_429_008_8, -0.028_262_460_778_457_947, -0.024_332_664_944_913_963, -0.020_578_421_937_983_298, -0.015_326_723_479_519_262, -0.012_620_312_457_072_043, -0.011_114_848_253_487_6, -0.009_579_552_528_772_912, -0.007_680_696_761_450_82, -0.005_120_612_892_358_83, -0.001_226_841_736_285_652_2, 0.001_763_897_182_365_375_6, 0.005_573_767_152_837_445, 0.012_075_766_531_774_166, 0.018_644_649_933_084_418, 0.024_825_497_315_139_392, 1.493_690_812_731_555_8],
    [-0.047_695_302_891_500_105, -0.028_944_169_976_021_8, -0.024_497_141_589_574_212, -0.020_914_358_204_628_78, -0.016_188_049_556_635_463, -0.013_367_605_475_732_274, -0.011_310_854_236_021_206, -0.009_600_568_422_651_987, -0.007_746_444_885_156_913, -0.005_021_206_417_987_389, -0.001_051_477_275_313_348_6, 0.002_431_370_125_588_523_5, 0.006_908_746_542_848_107, 0.013_349_690_171_626_542, 0.019_253_484_781_227_78, 0.027_301_086_517_961_478, 1.493_690_812_731_555_8],
    [-0.068_352_610_862_149_15, -0.031_157_034_519_492_723, -0.026_383_950_077_307_39, -0.022_220_340_454_605_542, -0.017_610_683_736_008_784, -0.014_686_163_047_857_76, -0.011_995_878_309_557_37, -0.009_828_907_409_021_922, -0.007_928_638_579_420_545, -0.005_163_818_274_677_958, -0.001_541_266_100_568_917_7, 0.002_899_277_682_719_793, 0.007_373_558_086_661_594, 0.013_789_229_984_114_552, 0.019_305_448_260_994_923, 0.084_321_155_185_522_18, 1.323_727_169_988_600_8],
    [-0.224_011_260_771_807_22, -0.054_254_076_895_631_3, -0.037_754_457_819_006_645, -0.029_779_556_005_169_038, -0.025_516_330_359_653_654, -0.021_521_098_797_495_582, -0.017_374_437_828_330_418, -0.014_325_886_097_485_451, -0.010_990_678_670_803_168, -0.008_126_886_799_422_223, -0.004_467_033_324_106_711, 0.000_337_461_157_669_446_9, 0.005_083_308_595_656_515, 0.013_154_335_866_190_688, 0.022_172_825_627_853_342, 0.242_732_755_633_920_9, 1.323_727_169_988_600_8],
    [-0.327_063_749_453_214_04, -0.185_075_894_311_738_9, -0.134_431_932_171_707_57, -0.096_391_972_313_097_76, -0.068_925_954_770_477_17, -0.052_862_596_025_838_8, -0.039_301_931_325_144_98, -0.029_576_105_696_213_963, -0.021_860_119_273_815_763, -0.015_428_625_251_804_468, -0.008_463_137_964_139_139, 0.000_192_749_288_386_076_18, 0.012_549_677_493_744_634, 0.043_628_447_482_780_05, 0.114_030_046_360_942_19, 0.285_904_591_505_640_35, 1.323_727_169_988_600_8],
    [-0.382_944_902_039_636_7, -0.237_838_429_519_791_28, -0.186_120_785_385_123_72, -0.145_023_232_790_681_8, -0.112_012_472_119_507_49, -0.088_242_591_756_408_13, -0.063_890_265_959_345_08, -0.043_721_208_420_934_33, -0.025_686_913_522_801_97, -0.007_729_796_068_341_194, 0.008_590_352_431_206_172, 0.028_164_743_350_597_373, 0.045_560_699_322_423_43, 0.063_205_032_735_028_85, 0.082_259_879_495_915_07, 0.140_950_257_317_954_4, 1.323_727_169_988_600_8],
    [-0.511_103_231_652_525_2, -0.270_296_620_728_358_3, -0.203_921_337_763_243_03, -0.148_046_551_313_167_13, -0.110_815_448_064_960_3, -0.078_775_951_104_861_62, -0.052_081_159_716_287_12, -0.027_554_564_017_606_795, -0.005_925_715_826_247_059, 0.013_105_100_169_321_905, 0.030_589_963_838_580_335, 0.046_083_809_274_577_454, 0.060_933_823_757_329_41, 0.074_058_167_350_908_71, 0.089_000_366_859_921_52, 0.109_763_516_430_452_22, 1.270_719_204_667_160_7],
    [-0.534_898_263_095_038, -0.329_598_301_276_109_23, -0.236_233_954_680_135_06, -0.158_674_836_937_886_62, -0.110_794_230_364_895_52, -0.067_653_484_960_602_26, -0.034_708_816_906_348_444, -0.007_142_733_546_272_972, 0.017_155_462_292_817_857, 0.036_514_308_935_884_976, 0.051_640_365_931_249_335, 0.065_098_365_946_071_59, 0.076_093_429_569_709_56, 0.087_404_409_899_467_95, 0.099_077_945_211_408_72, 0.114_416_003_966_556_32, 1.270_719_204_667_160_7],
    [-0.534_898_263_095_038, -0.414_149_369_382_668_7, -0.339_901_311_762_649_3, -0.223_412_432_012_103, -0.144_452_566_819_369_5, -0.071_712_477_713_735_46, -0.021_964_897_689_856_044, 0.014_132_586_508_280_563, 0.038_199_917_370_686_84, 0.054_564_370_150_631_28, 0.067_880_817_579_850_15, 0.078_619_112_535_205_47, 0.087_357_826_496_472_11, 0.095_375_555_193_247_32, 0.104_892_255_270_190_37, 0.121_124_331_557_171_8, 1.686_707_535_708_021_5],
    [-0.534_898_263_095_038, -0.464_403_768_881_537_17, -0.428_863_019_019_459_6, -0.391_966_403_384_838_3, -0.303_286_578_177_591_2, -0.203_898_127_073_965_46, -0.097_229_953_541_095_19, -0.021_573_212_676_581_566, 0.029_254_672_106_074_048, 0.055_108_292_145_630_79, 0.072_281_594_037_185_8, 0.083_435_804_626_866_24, 0.090_733_895_113_356_15, 0.099_362_622_097_348_93, 0.111_438_016_373_515_91, 0.163_139_771_841_269_15, 1.686_707_535_708_021_5],
    [-0.534_898_263_095_038, -0.473_260_849_087_843_94, -0.435_297_659_686_532_06, -0.402_670_993_797_522_97, -0.325_668_158_625_594_2, -0.225_052_586_525_899_35, -0.176_894_099_249_929_88, -0.100_217_606_528_702_68, -0.069_675_135_550_824_61, -0.053_691_224_261_778_37, -0.028_385_014_310_607_505, 0.057_393_115_202_208_404, 0.095_160_596_945_888_77, 0.353_400_134_117_027_3, 0.568_326_854_984_038_9, 1.031_039_673_649_256_4, 2.140_697_515_148_535_2],
    [-0.151_126_818_029_507, -0.004_874_540_919_806_533, -0.000_449_887_964_135_521_6, 0.002_085_038_295_379_906_7, 0.004_339_128_504_801_58, 0.006_919_616_646_844_757, 0.009_295_891_639_953_325, 0.011_296_666_640_501_293, 0.013_501_270_814_545_612, 0.015_116_738_831_985_173, 0.017_006_033_399_745_418, 0.019_159_007_907_079_608, 0.020_972_151_842_551_016, 0.023_413_171_974_749_483, 0.026_079_837_114_076_083, 0.029_656_097_950_514_108, 0.071_030_343_037_359_39],
    [-0.151_126_818_029_507, -0.012_125_083_903_703_432, -0.007_019_643_369_616_892_5, -0.004_150_537_511_703_745, -0.001_977_530_414_868_829, -5.641_502_144_783_859_6e-5, 0.001_773_022_143_093_799_4, 0.003_398_682_745_373_511, 0.005_102_638_547_367_705, 0.007_441_197_857_013_408, 0.009_577_218_464_719_513, 0.012_170_681_027_856_886, 0.014_516_211_931_078_648, 0.017_068_826_939_179_275, 0.020_470_252_680_416_644, 0.024_949_760_501_182_35, 0.047_609_850_510_708_82],
    [-0.146_056_608_314_588_67, -0.018_072_559_255_209_41, -0.013_127_850_454_812_486, -0.009_794_248_472_948_124, -0.007_793_495_555_266_6, -0.005_996_752_791_745_248, -0.004_433_230_131_072_839, -0.002_912_973_988_146_569, -0.001_367_688_678_246_201_8, 0.000_222_344_897_839_254_17, 0.001_776_331_889_381_832_6, 0.003_266_875_224_910_125_5, 0.005_534_938_073_648_784, 0.008_896_778_851_629_37, 0.013_160_665_735_503_195, 0.017_544_660_361_677, 0.041_790_917_665_297_63],
    [-0.138_144_852_801_877_77, -0.021_461_506_067_207_932, -0.017_987_824_828_035_2, -0.015_216_102_216_840_945, -0.012_415_953_571_708_233, -0.010_275_229_713_997_464, -0.008_565_268_542_898_588, -0.007_323_920_143_061_896, -0.006_127_640_724_034_066, -0.004_784_895_931_073_354, -0.003_462_855_693_548_068_7, -0.002_027_421_737_703_256_7, -0.000_205_971_814_627_111_42, 0.001_351_173_828_496_902_8, 0.002_958_247_886_125_244_6, 0.009_951_262_151_057_731, 0.026_721_053_809_331_54],
    [-0.138_144_852_801_877_77, -0.022_520_256_367_499_696, -0.020_602_693_438_809_286, -0.017_930_656_290_026_766, -0.015_574_730_762_779_997, -0.013_080_909_705_025_567, -0.011_177_480_616_289_53, -0.009_314_571_644_596_66, -0.007_915_640_317_134_38, -0.006_671_856_828_999_713, -0.005_449_152_327_750_082, -0.004_134_249_802_383_765, -0.002_122_635_242_847_874, 0.000_165_988_560_794_249_74, 0.001_937_590_187_414_165, 0.003_744_619_496_762_757, 0.018_584_311_162_186_423],
    [-0.078_706_706_605_907_77, -0.022_676_413_549_914_936, -0.021_179_565_009_917_065, -0.019_337_794_580_068_398, -0.017_098_852_435_476_687, -0.014_762_170_949_078_389, -0.012_183_937_273_275_885, -0.010_212_461_968_843_1, -0.008_292_964_327_348_174, -0.006_948_675_034_823_625, -0.005_561_043_282_397_818, -0.004_205_609_239_953_538, -0.002_282_802_354_060_821, 0.000_104_068_532_249_797_64, 0.001_939_084_069_450_844, 0.003_693_631_227_658_211_5, 0.021_081_154_004_173_11],
    [-0.078_706_706_605_907_77, -0.022_910_233_756_986_77, -0.021_289_752_904_080_656, -0.019_445_300_691_302_59, -0.017_417_754_875_980_865, -0.015_577_051_383_151_045, -0.012_850_375_391_359_558, -0.010_392_288_360_044_933, -0.008_459_073_209_564_528, -0.006_710_530_148_566_175, -0.005_394_865_191_042_573, -0.003_624_112_530_327_99, -0.001_949_571_663_701_939_7, 9.466_066_368_582_306e-5, 0.001_917_094_224_928_622_2, 0.006_314_954_833_116_43, 0.022_170_345_408_985_707],
    [-0.082_973_675_523_385_54, -0.022_518_311_590_441_78, -0.020_090_339_349_904_583, -0.018_301_803_732_234_875, -0.016_508_622_879_953_933, -0.014_613_539_806_494_3, -0.012_194_795_919_595_075, -0.009_478_433_641_188_931, -0.007_543_035_754_645_938, -0.005_703_379_909_767_35, -0.003_677_621_939_721_603_8, -0.002_294_411_548_239_878_5, -0.000_715_433_236_880_253_3, 0.001_085_492_022_472_876_6, 0.003_129_404_177_058_359_4, 0.007_260_063_102_813_044, 0.022_170_345_408_985_707],
    [-0.082_973_675_523_385_54, -0.021_276_349_134_948_8, -0.018_606_054_248_696_647, -0.016_631_106_011_821_235, -0.015_411_001_141_478_337, -0.013_499_479_974_432_045, -0.010_984_410_534_187_783, -0.008_574_060_675_368_837, -0.006_357_854_395_984_017, -0.003_785_878_747_086_676_7, -0.002_412_703_389_103_236_6, -0.001_333_183_913_504_527, 0.000_386_307_251_176_303_6, 0.001_923_026_776_594_158_5, 0.004_778_966_183_809_513, 0.007_255_356_219_623_333, 0.022_170_345_408_985_707],
    [-0.264_212_025_067_850_74, -0.021_632_331_185_417_36, -0.018_784_603_408_915_18, -0.016_549_447_372_826_348, -0.015_400_803_332_639_46, -0.013_285_981_680_720_357, -0.010_999_425_952_616_63, -0.008_872_011_529_018_049, -0.006_379_344_196_968_411, -0.003_657_934_118_397_153, -0.002_451_021_894_196_735_6, -0.001_366_126_159_653_712, 0.000_402_374_297_363_660_5, 0.001_757_567_747_880_585_8, 0.004_156_369_191_018_386, 0.006_416_835_883_351_273, 0.022_731_342_275_363_164],
    [-0.264_212_025_067_850_74, -0.022_390_243_115_957_383, -0.019_445_190_381_975_71, -0.016_845_520_777_101_9, -0.015_615_548_343_017_666, -0.013_938_513_789_259_584, -0.011_334_557_004_754_013, -0.009_554_166_810_221_284, -0.006_494_696_558_210_767, -0.004_124_382_207_921_945_5, -0.002_487_612_674_600_904, -0.001_400_577_771_797_490_8, 0.000_264_474_733_257_481_16, 0.001_543_062_081_602_163_4, 0.003_783_122_874_508_694_5, 0.005_951_503_649_634_718, 0.022_922_567_766_286_442],
    [-0.264_212_025_067_850_74, -0.022_475_877_334_523_15, -0.019_243_602_620_357_174, -0.017_160_288_995_902_655, -0.016_414_872_122_851_688, -0.015_251_573_859_259_007, -0.012_016_484_620_839_261, -0.009_330_957_759_747_585, -0.005_605_108_162_045_983, -0.003_950_590_412_776_711, -0.002_400_317_976_325_481, -0.001_280_262_313_884_804_3, 0.000_147_126_764_101_723_1, 0.002_194_311_903_651_856, 0.004_616_492_077_822_271, 0.007_247_189_997_241_481_5, 0.022_922_567_766_286_442],
    [-0.264_212_025_067_850_74, -0.023_183_343_756_253_066, -0.020_156_071_829_332_295, -0.018_222_785_431_806_447, -0.017_041_476_261_237_528, -0.016_248_337_407_228_77, -0.012_546_066_782_638_044, -0.010_077_726_486_593_941, -0.008_226_122_748_935_817, -0.005_510_478_799_671_22, -0.003_859_677_355_647_043, -0.002_386_851_286_206_658_4, -0.000_884_589_504_284_309_7, 0.001_901_394_595_781_881_8, 0.003_893_453_522_854_126_6, 0.013_454_547_717_001_83, 0.179_885_669_747_058_22],
    [-0.068_069_488_095_887_03, -0.025_961_480_256_544_083, -0.022_944_387_315_462_48, -0.020_161_339_926_098_313, -0.018_273_823_127_220_694, -0.016_944_304_410_024_57, -0.012_948_691_644_318_92, -0.011_328_484_599_265_36, -0.009_945_396_056_382_99, -0.008_619_893_173_852_19, -0.007_518_117_468_205_466, -0.004_043_998_511_247_876_5, -0.002_970_482_804_685_469_3, -0.000_634_377_883_179_852_7, 0.003_368_542_461_530_879_5, 0.016_310_313_874_378_78, 0.179_885_669_747_058_22],
    [-0.068_069_488_095_887_03, -0.026_003_553_997_248_18, -0.022_971_958_889_555_492, -0.020_162_500_131_024_268, -0.019_134_330_814_699_344, -0.015_604_683_701_641_328, -0.012_846_953_917_143_389, -0.011_665_857_364_661_128, -0.010_392_718_736_186_949, -0.009_605_445_093_080_346, -0.008_088_834_278_406_853, -0.003_557_838_984_642_923_4, -0.002_417_294_047_414_682, 0.002_789_155_475_375_469, 0.012_131_663_481_961_702, 0.016_386_056_795_239_443, 0.179_885_669_747_058_22],
    [-0.049_619_766_654_367_07, -0.025_590_017_498_866_104, -0.020_211_496_951_662_817, -0.018_438_766_830_236_347, -0.014_919_479_320_430_48, -0.013_473_208_674_610_155, -0.012_449_698_643_644_21, -0.011_658_108_638_385_687, -0.008_546_645_151_362_619, -0.005_202_153_184_998_289, -0.002_792_694_259_639_782_4, 0.001_203_988_288_050_464_8, 0.002_417_460_099_481_016_5, 0.003_658_715_125_234_064_8, 0.009_241_071_796_341_415, 0.020_806_043_831_286_96, 0.397_401_292_196_596_7],
    [-0.490_888_733_893_869_1, -0.021_537_941_775_717_325, -0.017_689_289_912_642_618, -0.014_398_090_265_244_242, -0.010_999_419_195_043_492, -0.008_392_084_282_923_373, -0.006_401_568_745_904_121, -0.004_540_818_810_977_147, -0.002_743_730_869_208_880_4, -0.000_772_270_420_410_371_4, 0.001_096_124_036_146_601_3, 0.002_780_452_186_920_403, 0.005_684_119_546_213_358, 0.010_547_445_591_465_945, 0.015_585_463_591_815_32, 0.020_790_904_208_113_89, 0.397_401_292_196_596_7],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.221_789_351_073_594, -0.131_457_941_307_833_92, -0.100_710_465_404_236_28, -0.074_421_161_256_992_52, -0.057_944_700_592_219_79, -0.037_616_151_536_315_985, -0.016_106_712_202_795_304, -0.008_203_994_680_762_945, 0.002_354_147_555_268_859_5, 0.008_199_722_063_402_592, 0.024_582_759_674_248_274, 0.066_029_253_200_924_45, 0.083_915_487_342_026_88, 0.121_195_393_210_549_66, 0.184_423_013_216_916_07, 0.261_640_987_008_293_14, 4.337_726_522_736_64],
    [-0.098_297_146_044_785_46, -0.054_286_061_288_417_28, -0.043_132_692_570_166_185, -0.033_203_814_877_888_906, -0.024_885_183_828_249_02, -0.018_023_352_435_260_032, -0.011_059_985_510_496_34, -0.001_303_938_876_790_355_7, 0.010_913_591_446_349_074, 0.039_816_050_913_849_14, 0.061_780_034_938_509_136, 0.077_790_408_078_888_28, 0.109_881_526_140_314_28, 0.135_917_916_296_939_53, 0.174_663_488_189_794_05, 0.208_521_243_183_514_14, 0.328_620_380_344_145_87],
    [-0.098_297_146_044_785_46, -0.051_792_178_645_940_32, -0.039_793_288_620_888_44, -0.027_515_594_944_100_275, -0.020_111_048_335_150_806, -0.013_408_908_782_162_908, -0.007_976_607_321_228_178, -0.000_371_508_043_868_887, 0.005_244_299_856_960_661, 0.015_304_334_127_942_755, 0.056_093_673_697_043_98, 0.128_868_480_262_171_07, 0.178_209_558_896_649_33, 0.211_184_893_006_697_88, 0.279_057_235_717_298_96, 0.326_542_834_321_967_4, 0.445_808_891_816_117_16],
    [-0.098_297_146_044_785_46, -0.040_829_666_625_148_35, -0.025_329_489_487_727_935, -0.016_836_555_779_866_35, -0.010_697_388_640_254_454, -0.005_425_137_738_505_465, -0.000_363_669_528_234_405_3, 0.003_185_045_175_456_804, 0.007_745_690_123_024_778, 0.012_810_048_328_621_032, 0.021_787_353_518_411_2, 0.062_230_384_384_163_236, 0.266_749_936_242_470_06, 0.325_776_697_640_118_74, 0.375_468_742_922_171_1, 0.429_194_269_683_275_1, 0.623_516_833_322_955_9],
    [-0.098_297_146_044_785_46, -0.031_122_314_355_495_357, -0.019_891_525_083_925_73, -0.013_134_184_934_903_767, -0.008_086_802_714_963_688, -0.003_769_264_753_387_630_5, -1.846_259_217_585_170_7e-5, 0.003_188_646_544_922_283, 0.007_223_698_712_604_022, 0.011_754_971_539_844_948, 0.017_066_033_518_908_62, 0.035_332_056_932_027_93, 0.366_470_873_618_368_83, 0.436_996_974_415_840_1, 0.502_341_329_297_916_5, 0.573_145_086_624_517_6, 0.738_736_795_701_422_6],
    [-0.126_802_209_845_274_8, -0.043_948_215_201_059_33, -0.031_976_878_834_939_62, -0.024_089_691_446_730_722, -0.017_353_023_892_412_98, -0.012_155_205_878_314_606, -0.007_552_356_249_600_902, -0.003_546_725_736_868_072, 0.000_587_914_168_957_136_1, 0.005_025_888_905_320_065, 0.010_973_479_595_921_782, 0.022_242_465_937_348_63, 0.453_150_189_246_899_3, 0.554_533_846_659_922_1, 0.611_266_482_052_907_3, 0.683_618_214_148_452_8, 0.863_362_668_478_674_3],
    [-0.993_673_854_309_323_4, -0.157_089_383_322_348_92, -0.063_813_210_377_551_75, -0.050_442_262_432_074_34, -0.040_990_388_547_556_21, -0.032_879_376_368_712_5, -0.025_822_159_032_895_21, -0.019_260_351_802_695_874, -0.012_429_425_855_287_227, -0.006_290_098_276_501_086, -1.822_016_828_329_147e-5, 0.009_549_991_431_376_064, 0.479_172_080_437_713_7, 0.625_107_040_767_982_8, 0.695_365_199_375_157_7, 0.757_433_322_583_867_5, 0.863_362_668_478_674_3],
    [-1.173_874_980_003_347, -0.507_832_396_772_852_1, -0.284_544_161_691_234, -0.133_912_962_213_430_6, -0.070_230_358_960_716_75, -0.057_316_546_984_382_49, -0.047_745_044_739_101_7, -0.038_800_728_262_303_893, -0.028_971_225_579_673_02, -0.017_982_303_667_202_448, -0.003_654_695_701_379_142, 0.072_089_463_514_710_81, 0.287_507_454_658_735_45, 0.602_006_982_282_334, 0.714_790_011_461_237_8, 0.776_690_202_316_979_8, 0.863_362_668_478_674_3],
    [-1.173_874_980_003_347, -0.706_372_583_112_826_2, -0.494_411_836_294_47, -0.301_060_535_969_627_9, -0.131_142_800_496_837_4, -0.063_210_676_741_934_87, -0.045_494_315_328_073_49, -0.021_780_198_080_327_995, 0.040_800_211_868_798_62, 0.059_510_850_051_918_59, 0.073_264_645_190_419_71, 0.089_098_636_312_221_12, 0.120_632_813_295_662_5, 0.281_002_225_384_362_9, 0.501_323_487_243_526_6, 0.740_437_767_145_072_1, 0.863_362_668_478_674_3],
    [-1.173_874_980_003_347, -0.666_796_022_955_708, -0.279_012_842_880_746_74, 0.020_575_999_219_727_418, 0.032_011_978_914_849_76, 0.040_445_125_540_674_96, 0.046_944_578_543_670_63, 0.053_301_617_743_757_99, 0.059_619_297_821_830_86, 0.064_519_838_082_967_52, 0.069_941_337_545_675_37, 0.075_549_310_703_027_13, 0.082_900_958_328_178_12, 0.092_040_346_614_380_13, 0.113_328_018_984_743_28, 0.282_307_185_981_077_1, 0.859_507_713_575_422_9],
    [-1.173_874_980_003_347, 0.003_367_477_628_928_83, 0.024_290_461_777_853_916, 0.032_552_175_369_294_83, 0.039_683_368_722_812_39, 0.045_625_674_257_491_665, 0.050_275_649_584_678_256, 0.054_391_105_325_042_98, 0.059_369_855_677_091_55, 0.062_786_686_314_007_84, 0.066_955_648_321_755_55, 0.071_173_104_568_827_17, 0.075_506_080_904_739_36, 0.079_795_607_989_517_9, 0.086_942_487_107_124_66, 0.095_860_949_057_465_99, 0.147_722_626_276_744_15],
    [-0.261_315_248_858_902, -0.045_363_842_945_235_98, 0.003_037_168_704_575_603, 0.020_260_766_415_367_785, 0.033_124_387_010_114_43, 0.042_577_655_452_035_54, 0.049_884_163_993_513_635, 0.055_910_377_816_744_89, 0.061_466_311_351_637_87, 0.065_334_740_457_314_33, 0.071_412_622_061_495_33, 0.076_000_456_371_683_28, 0.080_426_724_410_347_84, 0.086_698_853_211_869_13, 0.093_808_945_232_784_06, 0.102_701_313_523_490_66, 0.259_163_791_904_607_75],
    [-0.233_652_612_542_829_32, -0.103_267_419_725_846_26, -0.077_267_931_780_598_86, -0.047_331_821_813_838_81, -0.025_582_380_719_044_538, -0.016_022_956_929_574_516, -0.004_084_264_583_029_562, 0.007_801_832_499_196_691, 0.013_275_713_425_830_999, 0.020_408_561_205_541_8, 0.028_043_790_057_451_994, 0.039_104_219_047_276_87, 0.053_581_118_549_266_48, 0.083_721_224_607_452_02, 0.100_129_255_797_917_7, 0.110_920_480_750_249_59, 0.259_163_791_904_607_75],
    [-1.173_874_980_003_347, -0.346_133_034_963_822_53, -0.074_890_200_389_494_65, -0.050_761_360_474_842, -0.036_837_419_860_487_34, -0.024_791_271_597_626_598, -0.014_434_699_185_801_453, -0.005_214_292_228_810_75, 0.003_342_846_203_660_511_6, 0.017_068_299_305_687_38, 0.045_939_192_434_348_65, 0.069_601_833_324_414_29, 0.104_700_237_609_753_5, 0.374_331_060_253_482_4, 0.574_997_949_050_993_9, 0.714_737_095_116_894_3, 0.863_362_668_478_674_3],
    [-1.173_874_980_003_347, -0.346_133_034_963_822_53, -0.074_890_200_389_494_65, -0.050_761_360_474_842, -0.036_837_419_860_487_34, -0.024_791_271_597_626_598, -0.014_434_699_185_801_453, -0.005_214_292_228_810_75, 0.003_342_846_203_660_511_6, 0.017_068_299_305_687_38, 0.045_939_192_434_348_65, 0.069_601_833_324_414_29, 0.104_700_237_609_753_5, 0.374_331_060_253_482_4, 0.574_997_949_050_993_9, 0.714_737_095_116_894_3, 0.863_362_668_478_674_3],
    [-1.173_874_980_003_347, -0.346_133_034_963_822_53, -0.074_890_200_389_494_65, -0.050_761_360_474_842, -0.036_837_419_860_487_34, -0.024_791_271_597_626_598, -0.014_434_699_185_801_453, -0.005_214_292_228_810_75, 0.003_342_846_203_660_511_6, 0.017_068_299_305_687_38, 0.045_939_192_434_348_65, 0.069_601_833_324_414_29, 0.104_700_237_609_753_5, 0.374_331_060_253_482_4, 0.574_997_949_050_993_9, 0.714_737_095_116_894_3, 0.863_362_668_478_674_3],
    [-0.495_551_908_738_725_57, -0.431_353_049_614_145_83, -0.061_960_906_790_392_19, -0.037_544_452_343_786_38, -0.026_754_457_260_449_414, -0.018_905_315_645_507_386, -0.012_806_780_506_358_445, 0.001_621_943_592_125_108, 0.005_141_832_342_958_8, 0.010_345_318_536_572_714, 0.019_589_588_389_405_575, 0.021_770_699_403_745_72, 0.027_251_105_716_303_55, 0.031_335_907_347_269_945, 0.039_323_677_917_067_07, 0.045_684_318_833_256_39, 0.102_243_187_408_123_96],
    [-0.495_551_908_738_725_57, -0.057_395_370_157_817_83, -0.034_253_985_136_130_545, -0.021_459_632_140_259_14, -0.013_339_127_844_855_14, 0.001_198_630_600_036_221_2, 0.003_470_388_502_679_924, 0.008_618_255_884_508_623, 0.020_230_149_237_496_022, 0.024_308_916_914_936_658, 0.029_508_742_481_720_016, 0.037_494_298_480_197_51, 0.040_191_800_493_447_06, 0.042_503_129_477_488_51, 0.046_520_514_258_306_66, 0.065_027_429_365_882_56, 0.087_514_120_031_430_05],
    [-0.101_628_726_896_669_35, -0.017_422_748_226_931_985, -0.011_262_311_182_724_105, -0.006_128_685_688_076_772, -0.002_865_554_424_463_768, 0.000_472_398_212_179_861_96, 0.003_028_642_751_744_502, 0.004_498_860_245_205_296, 0.005_245_696_871_359_235, 0.005_938_314_578_067_467, 0.006_621_210_861_221_538, 0.007_247_682_767_408_94, 0.007_824_439_447_365_826, 0.008_555_968_022_394_217, 0.009_238_569_185_135_088, 0.010_296_405_345_278_774, 2.260_931_126_371_733_3],
    [-0.001_285_789_151_731_053_8, -6.015_981_581_312_001e-5, 0.000_620_227_087_068_010_7, 0.001_482_547_074_495_271_4, 0.002_100_114_868_477_896_5, 0.002_613_840_351_047_361, 0.003_241_491_703_393_963_7, 0.004_559_184_277_894_717, 0.005_166_721_257_708_086, 0.005_453_659_205_800_93, 0.005_662_846_412_917_051, 0.006_209_089_551_599_917, 0.007_826_872_934_178_09, 0.008_555_484_329_633_678, 0.009_932_883_636_140_25, 0.011_468_210_663_097_967, 1.669_564_175_627_756_8],
    [-0.001_285_789_151_731_053_8, 0.001_912_076_838_296_226_7, 0.003_169_982_463_354_288_5, 0.004_625_928_513_061_358, 0.005_292_752_557_999_692, 0.005_519_424_378_353_629, 0.005_998_577_061_653_501, 0.006_528_816_459_119_915_5, 0.007_595_375_037_625_097, 0.008_364_953_635_853_089, 0.008_612_929_475_047_01, 0.008_989_873_417_710_58, 0.009_351_630_837_812_269, 0.009_919_401_597_338_362, 0.010_715_575_906_883_594, 0.011_614_957_496_911_09, 1.726_949_594_052_013_2],
    [-6.239_479_011_549_42e-5, 0.002_813_073_576_165_942, 0.004_035_164_463_130_327, 0.004_879_316_045_720_738, 0.005_494_910_188_099_522, 0.005_974_071_122_468_562, 0.006_609_983_280_718_911_5, 0.007_159_977_020_739_455, 0.007_801_273_019_555_137, 0.008_381_895_322_718_66, 0.008_818_810_435_786_768, 0.009_036_672_202_713_397, 0.009_388_686_843_805_819, 0.009_840_726_653_260_457, 0.010_640_353_423_208_905, 0.011_812_179_843_185_779, 1.726_949_594_052_013_2],
    [0.001_581_290_397_109_302_9, 0.002_448_382_080_963_459, 0.003_056_358_354_246_188_7, 0.003_890_477_039_938_073_5, 0.004_672_357_317_946_923, 0.005_361_794_858_944_71, 0.005_877_290_641_618_18, 0.006_167_992_584_160_1, 0.006_945_517_602_589_214, 0.007_364_164_470_645_739, 0.007_803_394_948_569_948, 0.008_393_110_951_869_53, 0.008_884_135_508_643_69, 0.009_340_561_320_789, 0.009_589_586_569_838_377, 0.010_618_944_237_748_784, 1.726_949_594_052_013_2],
    [0.001_581_290_397_109_302_9, 0.002_758_151_908_648_801, 0.003_741_391_795_344_658_3, 0.004_762_400_479_452_483, 0.005_438_526_705_074_133, 0.005_891_635_846_041_365_5, 0.006_171_354_421_777_938, 0.006_751_521_475_103_489_4, 0.007_282_136_546_242_036, 0.007_814_765_872_792_721, 0.008_353_151_325_344_003, 0.008_813_006_919_945_578, 0.009_180_175_338_480_61, 0.009_713_744_279_963_56, 0.010_523_672_667_199_661, 0.011_198_774_891_837_649, 1.726_949_594_052_013_2],
    [0.001_581_290_397_109_302_9, 0.004_812_727_164_434_823_5, 0.005_699_074_702_961_904, 0.006_075_679_161_654_153, 0.006_399_143_363_192_531, 0.007_102_315_845_638_734, 0.007_606_649_941_406_555, 0.008_157_715_276_887_573, 0.008_566_602_593_402_792, 0.008_978_775_220_623_634, 0.009_377_787_766_711_7, 0.010_043_477_869_173_539, 0.010_446_255_215_919_51, 0.011_014_227_835_443_792, 0.011_401_931_025_649_173, 0.011_898_943_101_570_825, 1.825_043_923_824_374_7],
    [0.003_052_075_429_130_774, 0.004_816_941_264_610_622, 0.005_507_764_147_914_633, 0.006_071_555_453_739_117, 0.006_256_493_991_647_224, 0.006_777_734_925_854_417, 0.007_238_901_143_937_342, 0.007_825_552_618_407_882, 0.008_411_551_194_695_884, 0.008_697_777_494_657_628, 0.008_998_591_981_810_319, 0.009_455_374_414_707_8, 0.010_052_059_210_222_097, 0.010_601_935_174_651_45, 0.011_168_448_131_757_934, 0.011_767_841_504_490_08, 1.825_043_923_824_374_7],
    [0.003_440_453_370_304_963_3, 0.004_270_014_772_384_882, 0.005_148_256_030_852_541, 0.005_663_724_336_464_94, 0.006_041_198_096_338_527, 0.006_344_413_528_372_666, 0.006_680_609_032_255_394, 0.007_076_857_353_272_06, 0.007_603_477_729_282_599_6, 0.008_135_965_155_974_463, 0.008_573_987_244_848_242, 0.008_876_994_896_799_55, 0.009_032_703_861_902_785, 0.009_552_368_314_205_988, 0.010_047_308_195_885_826, 0.010_978_025_873_872_596, 1.825_043_923_824_374_7],
    [0.003_440_453_370_304_963_3, 0.004_535_819_866_101_958, 0.005_637_284_281_637, 0.006_137_392_588_261_222, 0.006_595_038_826_616_38, 0.006_980_745_843_050_246, 0.007_413_095_516_524_843_5, 0.007_821_577_321_739_7, 0.008_368_067_843_131_256, 0.008_788_173_783_655_102, 0.008_962_117_918_458_348, 0.009_209_511_719_212_247, 0.009_654_551_894_463_174, 0.010_144_335_118_282_728, 0.010_728_614_489_340_687, 0.011_639_177_438_036_613, 1.825_043_923_824_374_7],
    [0.003_440_453_370_304_963_3, 0.005_430_869_997_632_252_5, 0.006_220_427_080_325_059, 0.006_756_254_434_041_869, 0.007_210_721_723_914_435, 0.007_592_409_829_436_202, 0.007_955_980_245_605_872, 0.008_438_195_204_527_257, 0.008_845_098_293_027_676, 0.009_096_447_793_168_712, 0.009_427_440_916_034_452, 0.009_815_276_735_071_34, 0.010_190_098_214_028_706, 0.010_689_719_806_027_696, 0.011_325_676_232_933_898, 0.011_942_797_615_941_213, 2.260_931_126_371_733_3],
    [0.003_746_654_679_867_456, 0.005_069_121_560_317_138, 0.005_551_829_884_459_37, 0.006_162_469_050_863_326, 0.006_647_466_332_467_628, 0.007_007_046_626_076_163, 0.007_315_410_673_897_584, 0.007_604_766_613_247_495, 0.007_952_941_740_913_414, 0.008_380_989_859_261_804, 0.008_746_386_164_117_983, 0.009_096_712_914_432_465, 0.009_463_348_566_267_906, 0.009_841_157_620_229_746, 0.010_423_527_135_387_37, 0.011_264_828_270_016_333, 2.260_931_126_371_733_3],
    [0.001_540_453_699_476_085_3, 0.004_660_056_816_240_306, 0.005_063_935_726_533_122, 0.005_410_254_624_594_241_5, 0.005_781_202_951_886_986, 0.006_313_164_709_692_172, 0.006_698_698_698_794_905, 0.006_959_517_844_186_455, 0.007_225_938_008_812_771, 0.007_481_546_432_930_557, 0.007_750_038_013_176_453, 0.008_123_639_373_705_851, 0.008_495_062_100_762_965, 0.008_896_974_490_521_965, 0.009_332_103_013_106_4, 0.009_812_866_577_671_928, 2.260_931_126_371_733_3],
    [-0.009_428_127_528_875_23, -0.001_612_393_024_195_904_8, 0.001_489_732_319_538_468_3, 0.002_924_830_365_941_612_3, 0.003_929_779_149_524_007, 0.004_606_233_808_919_347, 0.005_051_994_168_022_991, 0.005_435_493_772_506_458, 0.005_819_613_846_301_783, 0.006_349_305_603_068_877, 0.006_805_124_175_501_297, 0.007_174_410_373_227_39, 0.007_536_063_763_565_471, 0.007_932_040_448_062_038, 0.008_441_550_457_970_301, 0.009_218_323_759_346_93, 2.260_931_126_371_733_3],
    [-0.026_745_127_934_060_253, -0.012_860_198_771_177_946, -0.008_194_037_318_572_957, -0.005_570_606_781_041_666, -0.003_891_167_539_469_314_8, -0.002_255_294_836_947_596_4, -0.000_759_224_091_764_755_8, 0.000_832_765_636_902_417_3, 0.002_165_892_074_567_324, 0.002_954_008_959_197_963_4, 0.003_779_793_432_544_159_4, 0.004_479_824_985_531_372, 0.005_160_524_547_562_939, 0.005_831_424_861_236_524, 0.006_836_272_439_940_776, 0.007_866_886_703_681_974, 0.022_940_273_791_630_44],
    [-0.054_464_363_771_923_54, -0.020_313_077_584_609_648, -0.017_272_061_254_963_04, -0.014_553_555_576_212_383, -0.012_368_057_164_188_498, -0.010_357_487_454_580_441, -0.008_458_255_590_588_099, -0.006_919_217_444_896_143_4, -0.005_538_149_456_624_761_5, -0.004_445_310_092_523_065, -0.003_105_022_813_486_370_5, -0.001_785_508_599_623_964_3, -0.000_264_559_850_568_770_9, 0.001_789_365_438_262_528_7, 0.003_224_482_196_891_661_3, 0.004_802_796_069_214_638, 0.011_156_139_818_893_518],
    [-0.089_379_028_121_710_84, -0.025_979_858_043_584_013, -0.023_519_120_876_347_794, -0.020_942_596_744_829_775, -0.019_172_109_291_285_072, -0.017_769_848_255_771_586, -0.016_401_383_669_010_66, -0.014_830_754_171_206_823, -0.013_306_825_527_275_19, -0.011_876_205_635_596_902, -0.009_907_939_744_072_662, -0.008_271_217_241_135_758, -0.006_572_423_256_609_675, -0.005_198_977_871_092_95, -0.003_754_923_274_054_502_5, -0.001_748_561_091_160_555_7, 0.005_584_283_979_161_775],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.073_156_063_960_494_23, -0.064_614_676_915_156_09, -0.049_785_512_170_611_584, -0.042_685_249_750_725_83, -0.032_613_829_906_289_665, -0.024_727_461_463_383_966, -0.015_003_480_233_953_943, -0.009_309_953_924_275_088, 0.000_735_709_300_975_218, 0.022_789_843_163_287_485, 0.036_983_316_251_699_3, 0.051_159_524_820_588_14, 0.066_077_420_051_526_25, 0.079_402_003_313_017_64, 0.083_155_065_505_331_7, 0.088_307_895_258_990_12, 0.548_653_607_009_765_4],
    [-0.186_416_827_525_857_36, -0.097_874_936_897_985_14, -0.078_743_563_383_610_24, -0.066_114_425_095_697_77, -0.054_955_155_012_836_85, -0.044_439_847_063_165_845, -0.036_011_924_086_528_96, -0.028_779_056_657_116_37, -0.021_135_864_625_251_823, -0.013_057_469_769_124_47, -0.005_662_482_741_542_974, 0.003_468_167_412_788_958_3, 0.012_242_422_161_157_056, 0.022_981_508_259_799_995, 0.035_844_846_794_702_25, 0.052_562_946_591_966_81, 0.111_518_528_289_357_3],
    [-0.264_136_702_072_749_53, -0.082_800_484_032_771_7, -0.063_638_532_941_287_56, -0.049_464_788_356_336_366, -0.038_768_494_835_337_94, -0.030_562_827_678_137_065, -0.022_987_072_024_621_164, -0.015_531_280_511_399_367, -0.007_826_292_641_235_46, -0.001_330_583_076_250_656_6, 0.007_617_377_081_030_678, 0.014_668_840_057_609_71, 0.023_924_327_636_748_913, 0.034_140_260_287_662_24, 0.045_635_287_157_044_29, 0.058_519_904_008_220_5, 0.111_518_528_289_357_3],
    [-0.264_136_702_072_749_53, -0.070_232_678_808_483_62, -0.055_450_100_789_415_38, -0.045_202_991_198_934_96, -0.036_524_994_429_478_125, -0.029_645_730_484_249_885, -0.023_791_274_747_660_216, -0.017_470_295_392_375_19, -0.011_528_072_789_524_123, -0.006_031_630_385_449_761, 0.000_568_093_604_045_130_2, 0.007_083_504_247_729_051, 0.014_154_591_037_197_267, 0.022_019_561_721_808_266, 0.032_200_070_857_492_99, 0.046_490_481_688_121_18, 0.218_850_858_559_235_92],
    [-0.464_740_940_279_211_53, -0.064_643_683_625_597_12, -0.051_150_721_475_265_51, -0.040_797_607_260_017_43, -0.031_871_494_532_878_575, -0.024_823_679_796_688_546, -0.018_150_123_842_864_404, -0.012_463_500_897_574_538, -0.006_337_108_881_966, -9.480_728_336_485_32e-5, 0.005_913_420_850_553_516_5, 0.012_805_506_107_281_001, 0.019_409_585_270_834_556, 0.026_798_953_039_144_594, 0.036_263_441_566_757_9, 0.048_385_485_627_878_054, 0.218_850_858_559_235_92],
    [-0.464_740_940_279_211_53, -0.056_442_768_518_627_555, -0.038_140_792_402_650_63, -0.025_118_004_456_565_38, -0.015_443_240_045_958_047, -0.007_217_439_415_906_681_5, 0.000_400_009_613_064_860_87, 0.007_228_224_784_568_93, 0.013_958_858_511_689_409, 0.019_864_446_107_505_804, 0.025_554_783_981_893_033, 0.031_282_285_494_910_17, 0.037_701_852_433_689_544, 0.043_658_369_522_913_05, 0.050_468_863_282_727_79, 0.056_914_097_877_698_126, 0.218_850_858_559_235_92],
    [-0.464_740_940_279_211_53, -0.041_204_844_408_541_72, -0.014_380_277_684_794_635, -0.000_966_481_893_563_539_8, 0.008_210_387_217_097_83, 0.014_507_726_914_389_535, 0.020_561_589_600_181_28, 0.025_250_710_255_032_883, 0.029_723_088_232_795_188, 0.034_026_596_813_824_195, 0.038_315_345_615_700_3, 0.042_214_475_648_553_7, 0.046_300_746_198_284_49, 0.050_631_438_064_141_24, 0.054_725_367_205_271_78, 0.058_807_392_636_768_59, 0.218_850_858_559_235_92],
    [-0.464_740_940_279_211_53, -0.329_533_343_125_502_6, -0.043_326_634_952_323_986, -0.014_837_321_008_714_43, 0.000_531_520_646_013_809_6, 0.009_209_745_228_062_26, 0.015_439_682_115_094_127, 0.021_237_850_503_221_432, 0.025_106_395_801_228_795, 0.029_702_378_867_252_59, 0.033_855_341_239_920_5, 0.037_797_605_313_500_16, 0.041_626_554_855_722_76, 0.046_663_266_323_557_14, 0.051_709_001_208_250_75, 0.056_400_666_110_925_986, 0.182_201_263_253_828_9],
    [-0.474_539_630_086_027_34, -0.334_437_670_024_795_7, -0.300_047_804_322_729_3, -0.063_743_819_265_320_9, -0.020_826_830_948_356_103, -0.007_869_839_627_982_803, -0.000_256_708_261_073_452_5, 0.005_403_743_567_232_587, 0.010_097_285_205_932_562, 0.014_435_446_973_471_081, 0.018_761_615_553_996_098, 0.021_720_169_813_123_72, 0.024_243_651_266_394_203, 0.027_749_487_302_686_855, 0.033_937_596_369_925_11, 0.044_049_407_801_490_72, 0.182_201_263_253_828_9],
    [-0.551_495_093_999_045_1, -0.309_477_204_134_318_34, -0.258_463_585_177_066_63, -0.083_639_145_566_666_6, -0.018_360_042_619_998_313, -0.006_680_620_740_142_822, -0.000_488_540_988_755_682_9, 0.004_952_325_959_111_485, 0.008_430_844_479_681_924, 0.012_720_131_889_667_344, 0.015_875_426_714_406_86, 0.019_178_440_363_049_977, 0.021_592_371_984_572_638, 0.024_021_345_713_584_785, 0.026_610_420_308_173_92, 0.030_572_003_264_214_637, 0.156_880_099_784_486_14],
    [-0.565_580_984_052_321_2, -0.296_497_635_637_589_5, -0.253_634_118_465_175_86, -0.220_863_734_317_749_28, -0.020_767_388_934_373_05, -0.006_386_514_601_865_168, 0.002_126_860_125_620_086_8, 0.007_143_618_907_675_746, 0.011_155_591_687_962_266, 0.015_226_272_710_639_891, 0.018_813_803_691_298_68, 0.021_807_946_069_712_19, 0.024_477_882_244_648_527, 0.027_469_965_498_082_748, 0.030_132_223_350_620_872, 0.033_904_653_081_705_684, 0.139_357_960_296_624_28],
    [-0.565_580_984_052_321_2, -0.451_406_572_889_821, -0.266_487_163_874_559_84, -0.230_558_538_327_197_9, -0.100_963_731_392_746_93, -0.009_153_122_138_119_552, 0.000_846_379_860_956_721_8, 0.007_733_010_037_636_122, 0.012_831_508_578_515_815, 0.016_672_564_873_502_247, 0.020_360_533_376_993_832, 0.023_563_112_872_225_19, 0.026_423_121_211_552_276, 0.029_085_540_880_164_712, 0.031_649_490_056_442_48, 0.080_643_985_415_341_76, 0.139_357_960_296_624_28],
    [-0.565_580_984_052_321_2, -0.465_575_076_841_697_13, -0.264_705_072_833_021_75, -0.206_993_536_707_705_37, -0.080_627_233_430_009_8, -0.008_835_046_556_878_771, 0.002_398_261_979_162_254_4, 0.008_680_857_997_255_386, 0.013_312_103_185_438_29, 0.017_128_706_251_341_23, 0.020_587_216_658_362_776, 0.023_742_932_951_475_525, 0.026_657_922_783_449_11, 0.029_150_797_592_949_446, 0.031_773_752_535_245_184, 0.081_784_195_588_983_57, 0.134_373_225_505_978_97],
    [-0.565_580_984_052_321_2, -0.289_385_242_924_690_64, -0.195_414_455_879_092_73, -0.148_855_721_146_131_08, -0.025_275_026_291_928_526, -0.004_756_458_050_182_634, 0.004_786_652_752_062_821_5, 0.009_424_614_974_368, 0.013_975_109_708_720_068, 0.017_910_245_187_856_732, 0.020_686_157_204_002_027, 0.023_551_607_697_741_98, 0.026_718_358_020_133_042, 0.029_082_946_383_820_33, 0.031_697_466_098_909_71, 0.069_708_913_459_416_06, 0.117_075_443_093_616_06],
    [-0.553_955_220_301_997_5, -0.195_658_954_512_146_6, -0.151_527_493_057_655_73, -0.110_061_194_977_149_36, -0.021_537_182_117_594_36, -0.003_009_362_140_165_97, 0.005_599_812_260_198_971, 0.010_531_052_577_441_19, 0.014_913_692_102_894_326, 0.018_946_256_646_904_573, 0.021_626_759_739_774_59, 0.024_619_941_884_435_476, 0.027_605_668_232_436_64, 0.030_087_334_454_237_34, 0.035_116_212_212_541_75, 0.053_197_643_930_596_95, 0.096_094_463_894_777_83],
    [-0.518_686_781_467_079_5, -0.199_669_341_188_538_55, -0.142_140_898_461_332_54, -0.112_087_701_825_505_65, -0.071_825_319_587_947_02, -0.017_336_856_966_569_215, 0.000_958_330_199_629_399_8, 0.010_205_979_925_647_573, 0.016_836_471_024_464_94, 0.021_557_848_851_461_64, 0.025_111_727_868_949_7, 0.029_285_612_775_456_608, 0.033_131_841_468_304_62, 0.039_220_301_789_786_66, 0.050_955_323_612_997_69, 0.065_805_598_904_895_68, 0.147_764_758_908_932_7],
    [-0.380_898_084_931_481_6, -0.211_938_849_604_364_3, -0.171_774_566_096_614_35, -0.140_955_442_820_454_64, -0.112_361_968_342_098_93, -0.080_847_807_660_044_4, -0.051_613_380_159_924_89, -0.011_170_068_933_247_393, 0.014_533_108_365_547_925, 0.025_088_978_065_020_754, 0.033_250_872_075_970_564, 0.042_081_177_374_459_974, 0.051_608_296_567_386_906, 0.061_906_222_854_593_25, 0.079_032_303_696_829_77, 0.111_047_958_831_667_58, 0.147_764_758_908_932_7],
    [-0.341_956_615_431_480_54, -0.191_940_989_921_554_45, -0.170_671_643_560_420_6, -0.158_916_326_618_859_1, -0.145_213_998_069_858_13, -0.127_875_172_480_738_04, -0.101_542_283_241_940_74, -0.072_940_836_463_575_81, -0.040_760_563_550_761_55, 0.004_837_489_653_339_895, 0.031_337_090_959_666_97, 0.047_592_164_271_375_08, 0.058_237_824_749_662_72, 0.074_614_854_126_315_1, 0.100_352_837_245_447_61, 0.125_262_716_350_720_1, 0.147_764_758_908_932_7],
    [-0.467_936_761_512_614_2, -0.154_926_973_357_145_42, -0.117_852_615_036_460_76, -0.085_218_123_104_596_45, -0.059_926_849_476_798_02, -0.036_465_034_217_281_92, -0.016_144_619_429_973_955, 0.002_709_762_237_224_506, 0.017_380_439_331_283_564, 0.028_501_767_365_195_33, 0.038_669_698_944_979_83, 0.047_929_124_147_784_8, 0.058_178_083_355_382_7, 0.071_386_385_000_446_72, 0.088_142_711_694_018_81, 0.116_889_256_121_772_41, 0.602_789_442_347_525_4],
    [-0.763_936_359_946_104_4, -0.147_534_731_280_818_76, -0.107_430_072_503_685_6, -0.074_172_015_267_824_77, -0.046_769_791_947_305_28, -0.022_041_179_537_775_72, 0.002_639_306_326_441_91, 0.022_368_290_848_907_506, 0.037_977_328_658_952_716, 0.050_109_067_574_186_21, 0.062_534_843_114_970_18, 0.078_676_845_267_687_92, 0.100_964_894_837_764_2, 0.131_549_852_117_599_9, 0.196_890_173_135_391_5, 0.300_857_249_534_192_96, 0.749_714_049_496_058_6],
    [-0.763_936_359_946_104_4, -0.540_066_712_522_999_3, -0.347_793_330_258_447_2, -0.147_390_859_010_389_23, -0.065_195_959_528_045_59, -0.017_417_407_120_707_81, 0.019_029_839_644_322_04, 0.046_541_478_311_322_756, 0.070_355_028_538_911_03, 0.096_416_685_638_552_3, 0.128_993_319_820_196_74, 0.160_982_791_284_602_86, 0.197_680_422_744_941_5, 0.240_695_782_383_085_95, 0.294_969_272_159_305_8, 0.378_867_673_287_316_4, 0.819_474_796_010_708_9],
    [-0.763_936_359_946_104_4, -0.567_242_894_261_848_5, -0.470_023_291_186_129_45, -0.351_197_501_488_981_43, -0.237_926_431_692_131_32, -0.131_455_225_291_561_67, -0.049_232_953_161_178_86, -0.001_836_581_127_114_144_2, 0.032_181_039_531_427_33, 0.064_487_067_971_355_52, 0.094_468_168_924_542_28, 0.131_256_049_047_369_84, 0.170_474_050_054_655_7, 0.213_980_128_597_029_1, 0.270_715_367_641_426_7, 0.348_610_524_087_984, 0.903_596_207_481_432],
    [-0.763_936_359_946_104_4, -0.547_888_389_027_102_7, -0.464_390_018_013_353_44, -0.370_810_559_175_668_74, -0.282_074_578_726_038_36, -0.205_978_066_024_286_75, -0.131_130_371_979_638_48, -0.063_491_558_022_128_27, -0.009_364_510_347_485_982, 0.027_253_503_951_964_086, 0.062_756_646_154_142_21, 0.098_410_640_523_489_22, 0.138_626_408_226_549_56, 0.190_924_608_464_521_56, 0.251_714_949_386_715_9, 0.343_420_546_925_403_9, 0.903_596_207_481_432],
    [-0.695_039_990_743_615, -0.512_302_651_196_318_5, -0.434_776_148_213_703_75, -0.350_252_085_893_269_5, -0.279_692_550_076_510_8, -0.219_505_611_190_224_63, -0.155_863_604_375_033_85, -0.093_174_272_217_479_95, -0.036_201_134_240_057_484, 0.010_070_323_143_763_639, 0.050_598_071_712_334_63, 0.092_486_517_449_727_34, 0.131_295_213_671_162_87, 0.188_790_903_457_359_16, 0.252_466_858_962_933_8, 0.354_916_153_761_175_9, 0.903_596_207_481_432],
    [-0.614_079_204_137_425, -0.460_811_666_326_535_4, -0.387_944_913_025_185_2, -0.317_357_809_722_975_67, -0.255_808_629_659_294_85, -0.202_521_880_111_999_06, -0.142_579_833_310_205_4, -0.078_831_750_336_823_53, -0.027_790_988_458_206_87, 0.015_639_261_781_709_844, 0.069_651_768_975_563_16, 0.112_709_704_432_104_36, 0.154_800_127_468_785_6, 0.208_026_936_288_084_47, 0.262_768_218_600_742_9, 0.365_823_486_285_210_4, 0.982_132_001_672_724_9],
    [-0.537_381_323_076_629_8, -0.442_780_728_466_293_7, -0.359_161_786_471_610_34, -0.301_463_397_346_854_46, -0.233_152_319_800_255_94, -0.177_900_257_334_024_87, -0.128_239_831_820_649_77, -0.068_646_493_626_804_82, -0.016_624_474_588_179_9, 0.024_664_355_069_435_318, 0.072_191_694_110_769_16, 0.119_767_402_305_522_88, 0.158_804_989_352_218_12, 0.208_015_179_995_705_7, 0.262_418_685_366_717_14, 0.348_810_561_737_768_7, 0.982_132_001_672_724_9],
    [-0.509_437_796_581_204_1, -0.406_985_897_775_493_5, -0.336_111_109_460_665_4, -0.275_980_975_023_536_4, -0.206_069_157_727_078_3, -0.152_856_115_107_280_48, -0.099_089_253_627_696_94, -0.056_049_861_557_941_56, -0.011_129_668_470_822_852, 0.027_750_703_585_165_5, 0.069_543_621_422_572_93, 0.117_207_651_048_079_09, 0.151_675_790_331_321_48, 0.209_460_462_011_328_56, 0.266_421_597_262_982_4, 0.358_886_981_964_902, 0.982_132_001_672_724_9],
    [-0.504_231_010_839_034_9, -0.351_334_742_710_552_9, -0.287_857_532_558_641_2, -0.226_123_166_722_447_67, -0.178_300_131_141_559_57, -0.130_968_044_042_666_93, -0.091_952_148_770_377_52, -0.053_153_991_266_066_57, -0.011_654_955_770_286_808, 0.028_490_083_845_813_2, 0.072_059_317_345_513_06, 0.113_850_581_486_835_62, 0.148_470_780_802_631_2, 0.209_185_776_604_942_86, 0.272_249_596_057_639_4, 0.378_805_196_301_549_63, 0.982_132_001_672_724_9],
    [-0.500_113_702_426_740_8, -0.287_817_070_096_370_03, -0.230_411_760_292_185_66, -0.188_190_472_650_157_3, -0.152_118_482_685_849_32, -0.120_394_515_197_412_15, -0.088_589_789_836_358_4, -0.051_902_631_571_356_82, -0.013_104_240_248_390_277, 0.024_264_369_150_776_698, 0.067_313_852_883_020_54, 0.102_168_428_109_917_5, 0.144_030_775_643_088_64, 0.201_733_105_633_933_57, 0.269_987_133_481_052_2, 0.371_530_756_996_931_1, 0.913_630_271_389_224_5],
    [-0.424_403_489_005_277_36, -0.232_884_570_596_925_94, -0.190_130_019_022_954_82, -0.157_469_591_400_162_78, -0.130_981_891_764_900_85, -0.101_671_372_908_413_78, -0.071_073_538_002_177_4, -0.036_102_909_570_987_1, -0.002_257_011_616_265_251_7, 0.029_810_565_693_347_51, 0.067_186_889_908_091_41, 0.104_569_634_393_498_98, 0.148_986_838_397_414_36, 0.206_408_149_526_925_88, 0.273_433_777_114_385_5, 0.375_191_832_940_888, 0.657_765_793_906_678],
    [-0.317_896_257_974_544_8, -0.189_932_088_518_624_95, -0.153_382_316_265_622_35, -0.127_245_580_734_655_14, -0.095_211_939_110_362_37, -0.066_809_554_552_078_07, -0.037_254_152_946_980_26, -0.012_055_291_799_358_12, 0.012_203_823_876_359_194, 0.043_735_205_556_763_88, 0.085_007_314_991_394_63, 0.126_317_347_897_970_72, 0.171_487_979_890_059_62, 0.226_648_490_450_911_7, 0.307_177_305_498_517_2, 0.407_069_039_351_999_7, 0.613_606_130_772_431_6],
    [-0.247_270_136_361_988_47, -0.147_132_478_494_068_16, -0.110_906_822_500_068_23, -0.083_557_056_596_712_88, -0.058_740_559_551_189_786, -0.043_407_660_220_462_156, -0.025_133_381_926_736_694, -0.001_622_680_549_169_482_7, 0.023_918_903_342_489_205, 0.064_486_576_422_861_83, 0.107_680_305_400_528_38, 0.152_311_205_208_581_5, 0.205_319_629_306_689_65, 0.256_867_052_766_936_83, 0.356_240_287_757_626_06, 0.429_343_725_146_679_73, 0.596_857_182_815_223_3],
    [-0.157_578_370_024_876_62, -0.094_002_108_232_649_51, -0.072_361_642_435_714_52, -0.056_678_203_181_114_224, -0.047_502_240_884_629_704, -0.039_997_486_491_313_31, -0.030_448_652_765_184_374, -0.015_161_701_397_996_93, 0.017_579_733_186_557_884, 0.068_658_012_974_474_3, 0.115_813_050_213_596_03, 0.166_318_440_196_938_2, 0.232_397_293_634_438_42, 0.330_075_277_334_189_1, 0.400_950_847_948_437_05, 0.446_452_734_079_087_03, 0.582_122_837_482_117_2],
    [-0.142_392_046_938_837_67, -0.131_928_885_872_536_54, -0.088_641_165_828_714_01, -0.083_056_722_990_290_09, -0.070_580_438_343_134_4, -0.062_113_429_020_013_135, -0.056_458_858_181_736_876, -0.046_221_336_030_517_264, -0.038_082_403_381_407_59, -0.030_446_681_142_215_51, -0.005_483_095_381_186_655_5, -0.000_294_395_265_276_014_3, 0.009_453_355_373_906_53, 0.020_430_083_648_976_85, 0.035_563_357_336_581_6, 0.052_958_083_393_601_973, 0.582_122_837_482_117_2],
    [-0.142_392_046_938_837_67, -0.088_646_589_996_929_19, -0.072_423_899_326_236_61, -0.062_169_511_016_707_45, -0.054_486_338_790_065_196, -0.041_677_847_359_584_3, -0.031_698_009_147_463_85, -0.006_177_364_629_302_702, 0.000_972_583_808_214_407_7, 0.008_461_987_040_043_455, 0.016_426_740_462_669_717, 0.021_473_350_223_778_317, 0.027_319_245_807_045_707, 0.037_226_197_750_997_89, 0.049_966_572_665_699_45, 0.069_249_664_001_989_22, 0.582_122_837_482_117_2],
    [-0.353_491_341_522_897_1, -0.116_545_527_179_052_1, -0.075_089_343_251_959_98, -0.050_995_926_305_149_95, -0.033_000_506_385_314_98, -0.019_984_505_906_364_927, -0.007_309_247_279_512_316_6, 0.005_119_931_031_263_407, 0.016_283_164_097_796_503, 0.023_729_965_491_893_05, 0.033_888_762_081_494_79, 0.045_602_497_706_099_226, 0.057_623_369_515_953_024, 0.077_434_996_721_029_27, 0.094_545_800_247_129_87, 0.108_620_964_854_479_53, 0.156_458_095_865_122_71],
    [-0.489_982_732_209_865_55, -0.172_705_419_313_773_45, -0.119_848_906_731_160_05, -0.087_982_267_360_015_56, -0.063_207_860_183_720_06, -0.042_200_279_513_330_954, -0.025_982_596_137_135_428, -0.010_556_604_177_921_79, 0.003_231_761_870_614_522, 0.016_303_309_505_937_398, 0.026_571_394_694_929_192, 0.039_908_971_065_367_595, 0.052_849_166_944_659_27, 0.066_849_059_531_488_92, 0.083_325_937_435_614_34, 0.112_026_572_547_924_27, 0.172_289_742_782_234_97],
    [-0.681_913_008_075_943_2, -0.228_098_483_962_843_07, -0.162_663_091_006_260_3, -0.118_414_545_670_463_74, -0.089_025_469_544_194_75, -0.065_587_424_245_204_64, -0.043_273_832_687_214_32, -0.022_393_159_717_421_094, -0.003_431_123_499_688_771_6, 0.013_468_549_279_199_903, 0.028_655_238_695_076_492, 0.044_428_533_977_549_75, 0.058_581_956_472_421_75, 0.072_657_344_369_331_12, 0.084_864_642_709_771_68, 0.120_961_757_422_499_66, 0.180_978_534_697_995_74],
    [-0.724_805_657_383_187_8, -0.296_580_269_277_889_8, -0.205_024_415_185_818_27, -0.146_830_368_537_642_47, -0.107_961_277_799_137_3, -0.076_961_362_729_437_09, -0.051_817_424_262_467_02, -0.027_681_734_825_515_093, -0.001_629_315_603_006_553_6, 0.018_339_390_599_555_427, 0.036_471_608_921_362_97, 0.053_384_750_284_604_4, 0.068_818_994_948_957_85, 0.080_656_205_612_976_48, 0.094_877_927_956_486_35, 0.133_055_534_951_995_93, 0.189_059_374_100_198_24],
    [-0.776_318_120_959_85, -0.343_009_697_372_337_83, -0.241_515_376_492_112_74, -0.168_673_662_741_483_07, -0.120_543_524_341_143_43, -0.083_560_385_773_213_16, -0.054_208_595_011_939_725, -0.027_787_482_855_413_33, 0.001_551_160_743_276_189, 0.022_828_763_436_291_8, 0.041_393_880_450_437_82, 0.059_724_804_932_430_67, 0.075_813_607_701_894_53, 0.087_659_415_862_062_19, 0.101_312_200_864_982_9, 0.140_301_371_709_958_43, 0.190_734_026_958_161],
    [-0.960_740_018_384_369_1, -0.380_926_039_289_254_6, -0.269_261_772_801_820_1, -0.192_347_436_266_337_38, -0.135_889_805_646_926_9, -0.092_446_534_928_460_99, -0.058_116_371_241_409_28, -0.030_062_401_508_176_465, -0.000_228_385_594_849_960_74, 0.022_076_151_526_171_085, 0.042_883_315_872_737_02, 0.061_228_344_747_932_71, 0.077_752_129_679_349_93, 0.090_947_545_205_074_84, 0.103_663_609_774_510_28, 0.139_013_167_931_182_37, 0.198_752_281_221_876_71],
    [-1.086_132_635_147_636_8, -0.421_908_227_957_657_6, -0.287_271_741_507_058, -0.201_998_919_863_814_44, -0.145_051_727_903_311_86, -0.098_624_217_118_821_94, -0.060_014_406_120_864_16, -0.027_905_757_634_983_21, -7.362_101_323_784_671e-5, 0.024_014_007_888_704_4, 0.045_286_147_444_323_91, 0.063_947_443_681_887_87, 0.080_162_620_949_847_07, 0.095_215_829_925_675_3, 0.108_044_287_477_532_85, 0.140_829_316_146_996_12, 0.210_541_846_347_025_44],
    [-1.133_990_468_092_893_2, -0.465_855_234_697_480_25, -0.316_153_984_470_885_9, -0.220_834_815_687_532_58, -0.153_175_764_793_130_7, -0.104_590_562_992_964_94, -0.063_297_054_370_573_24, -0.027_787_428_376_638_387, 0.001_211_297_432_690_051, 0.026_684_851_215_976_454, 0.048_177_164_630_632_5, 0.068_959_469_850_669_09, 0.085_913_064_743_849_52, 0.101_078_413_110_258, 0.114_956_273_891_755_63, 0.145_029_654_698_691_5, 0.221_707_094_138_979_6],
    [-1.219_057_655_272_573_4, -0.520_438_247_002_101_4, -0.356_429_027_681_234_2, -0.255_362_052_600_175_3, -0.174_224_219_068_031_3, -0.117_242_352_879_770_93, -0.071_397_867_522_811_91, -0.034_717_177_918_737_18, -0.002_690_850_988_507_803_7, 0.025_807_371_043_016_24, 0.048_196_774_804_583_495, 0.070_832_548_926_682_07, 0.089_973_806_398_492_67, 0.105_767_057_694_738_57, 0.120_035_718_267_965_62, 0.143_068_032_639_578_18, 0.224_440_489_999_244_26],
    [-1.325_997_880_567_969_7, -0.543_280_825_547_628_8, -0.365_260_658_477_204_6, -0.262_841_345_422_880_35, -0.183_872_116_917_532_89, -0.125_810_010_745_087_9, -0.076_308_325_920_383_18, -0.038_976_218_977_885_37, -0.004_480_315_858_111_978, 0.025_049_415_406_885_142, 0.049_418_993_317_704_09, 0.072_572_486_199_171_16, 0.094_407_834_865_221_11, 0.110_083_487_728_829_53, 0.125_786_561_698_356_25, 0.143_084_700_821_113_4, 0.224_440_489_999_244_26],
    [-1.469_629_468_851_389_1, -0.513_061_854_997_340_9, -0.343_974_494_471_714_4, -0.245_056_281_785_674_23, -0.176_061_746_686_063_5, -0.117_216_222_168_227_95, -0.070_302_004_635_964_41, -0.032_518_820_058_417_414, -0.000_949_271_489_318_141, 0.027_529_229_959_744_116, 0.052_153_160_189_402_886, 0.078_836_587_639_472_31, 0.097_930_578_212_888_31, 0.114_090_153_688_459_7, 0.128_977_573_180_303_8, 0.151_736_372_285_421_44, 0.224_440_489_999_244_26],
    [-1.533_820_028_121_610_3, -0.481_533_173_801_997_1, -0.332_314_600_923_870_3, -0.235_131_666_978_921_35, -0.169_821_148_560_496_96, -0.114_450_411_021_710_69, -0.070_570_141_309_463_64, -0.032_438_941_620_295_71, -0.000_634_990_218_393_886_5, 0.027_753_387_491_955_425, 0.050_424_727_562_791_08, 0.074_032_898_816_111_8, 0.093_160_294_640_047_63, 0.109_908_219_531_465_8, 0.125_370_414_035_394_34, 0.154_753_126_296_196_3, 0.224_440_489_999_244_26],
    [-1.533_820_028_121_610_3, -0.475_876_098_180_273_54, -0.326_230_408_155_273_65, -0.224_735_401_864_447, -0.159_481_081_832_297_62, -0.109_151_774_615_245_7, -0.069_462_269_048_048_16, -0.030_877_365_962_521_774, -0.000_244_712_563_080_556_53, 0.028_486_097_063_151_41, 0.050_544_239_896_948_89, 0.072_773_139_734_325_23, 0.091_727_484_260_983_15, 0.106_605_067_384_208_5, 0.121_426_766_873_832_66, 0.151_883_040_475_296_27, 0.222_537_338_351_733_28],
    [-1.855_448_171_412_975_4, -0.459_379_823_661_358_1, -0.313_046_947_016_557_9, -0.210_846_828_796_266_98, -0.152_234_035_461_501_85, -0.104_332_709_996_161_5, -0.063_271_719_173_249_89, -0.027_100_621_360_793_47, 0.001_434_451_494_184_425_8, 0.030_463_591_269_046_374, 0.052_361_623_862_232_906, 0.075_400_141_164_096, 0.092_389_914_843_452_92, 0.106_756_374_110_827_5, 0.120_599_771_669_934_32, 0.153_461_597_574_678_2, 0.216_368_535_777_323_96],
    [-1.855_448_171_412_975_4, -0.440_763_676_423_524_04, -0.306_545_616_982_484_8, -0.205_537_866_383_002, -0.149_261_154_218_296_86, -0.101_893_507_884_534_76, -0.060_374_280_689_245_906, -0.029_098_396_400_627_57, -0.001_477_542_072_978_122, 0.026_427_240_393_902_686, 0.049_985_861_621_123_26, 0.072_929_379_340_891_25, 0.090_357_557_652_631_43, 0.105_170_681_897_150_65, 0.118_128_685_129_422_4, 0.150_635_805_682_986_9, 0.209_412_737_967_795_7],
    [-1.855_448_171_412_975_4, -0.416_873_867_110_043_15, -0.292_028_427_633_304, -0.199_953_283_969_762_09, -0.143_589_162_826_023_74, -0.091_467_771_525_614_51, -0.053_406_440_729_990_71, -0.027_103_334_247_674_92, -0.001_490_685_098_420_100_3, 0.023_761_720_509_145_382, 0.045_967_466_728_461_054, 0.066_642_665_112_035_57, 0.083_420_973_257_091_07, 0.098_380_398_180_444_25, 0.112_126_014_697_254_14, 0.143_324_674_540_401_38, 0.205_081_670_973_891_4],
    [-1.855_448_171_412_975_4, -0.364_280_615_753_798_83, -0.232_221_661_134_356_3, -0.165_620_320_955_343_99, -0.109_876_854_343_805_56, -0.069_123_371_361_082_64, -0.038_905_849_089_484_95, -0.014_847_451_424_063_218, 0.005_854_643_351_915_091, 0.030_983_557_481_146_017, 0.045_889_727_245_642_73, 0.062_892_624_951_050_54, 0.079_308_851_012_906_02, 0.091_067_070_600_449_1, 0.102_962_635_977_589_56, 0.124_197_299_269_155_99, 0.205_081_670_973_891_4],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_27: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.616_332_831_069_388, 10.0),
    (-12.197_642_630_642_052, -9.824_274_739_469_189),
    (-12.796_000_305_546_626, -10.121_214_237_184_518),
    (-12.178_263_269_200_539, -6.664_053_171_969_436),
    (-0.204_487_200_827_631_17, 0.859_183_668_994_316_4),
    (-0.291_671_890_357_536_55, 0.281_268_978_159_679_16),
    (1.887_844_047_391_827_9, 6.414_618_865_436_563),
    (0.346_609_450_463_532_3, 1.381_681_812_055_976_2),
    (-2.162_702_275_634_650_6, -0.092_699_163_716_663_7),
    (0.047_798_029_729_289_856, 0.860_180_814_827_200_1),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_27: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.110_879_339_708_090_7, 1.035_174_459_204_201_4, 0.922_350_275_436_115_3, 0.669_850_195_322_314, 0.567_146_288_971_933_4, 0.544_551_864_213_071_8, 0.543_312_812_190_289_3, 0.628_542_537_396_211_5, 0.622_384_666_012_798_1, 0.592_517_680_167_146, 0.577_013_294_125_883_2, 0.572_029_655_890_508_6, 0.572_293_696_811_974_1, 0.575_274_732_062_552_6, 0.566_736_707_430_777_3, 1.041_309_841_154_949_7, 1.042_515_162_046_601],
    [0.230_751_853_454_302_24, 0.211_705_030_785_315_77, 0.192_101_955_929_598_67, 0.167_895_063_615_659_76, 0.143_796_743_555_262_6, 0.117_875_137_174_424_63, 0.090_409_883_852_252_11, 0.062_605_790_176_680_2, 0.035_915_150_432_969_15, 0.008_502_267_904_279_735, 0.005_085_464_364_472_52, 0.148_858_521_265_506_08, 0.191_036_151_123_454_18, 0.233_716_885_811_472_2, 0.281_760_938_034_163_97, 0.298_259_504_830_534_76, -0.181_927_610_080_192_26],
    [0.252_295_757_433_671_5, 0.235_198_821_430_374_57, 0.209_519_887_828_647_83, 0.176_559_895_424_955_57, 0.138_981_813_492_693_5, 0.103_478_350_703_422_72, 0.068_806_907_809_495_78, 0.028_362_176_344_898_998, -0.008_863_875_966_242_385, -0.040_848_715_445_038_81, -0.079_696_909_531_448_92, -0.114_015_433_014_527_3, -0.148_358_197_533_550_12, -0.177_573_473_604_510_2, -0.211_482_541_674_388_15, -0.259_257_924_054_939_8, -0.292_950_836_135_556_3],
    [2.283_717_040_098_810_3, 2.283_717_040_098_810_3, 2.283_717_040_098_810_3, 2.283_717_040_098_810_3, 2.283_717_040_098_810_3, 2.283_717_040_098_810_3, 2.283_717_040_098_810_3, 2.132_522_364_545_242, 2.108_365_507_469_739_6, 2.104_762_225_001_359_3, 2.104_762_225_001_359_3, 2.104_762_225_001_359_3, 2.104_762_225_001_359_3, 2.104_762_225_001_359_3, 2.104_762_225_001_359_3, 2.104_762_225_001_359_3, 2.104_762_225_001_359_3],
    [-1.221_435_514_801_442_2, -1.304_601_841_385_279_1, -1.400_754_891_683_839, -1.477_343_850_248_163_3, -1.557_905_567_943_956_4, -1.632_130_431_375_603_3, -1.674_133_706_782_044_3, -0.859_605_132_826_637_2, -0.685_007_708_833_444_3, -0.604_356_363_625_525_8, -0.560_681_447_801_396, -0.539_586_858_168_444_8, -0.535_906_795_103_247_5, -0.517_825_398_112_881_4, -0.124_910_361_061_909_11, -0.052_653_728_444_044_12, -0.014_658_870_556_788_753],
    [-1.611_852_982_398_538_4, -1.649_091_439_165_757_4, -1.689_593_477_648_851_3, -1.727_882_958_316_754_8, -1.758_834_142_543_077_4, -1.796_676_045_548_839_5, -1.835_610_723_641_158_3, -1.871_886_057_496_214_6, -1.905_246_869_869_773, -1.941_451_188_620_964_7, -1.981_522_942_026_141, -2.015_310_333_335_729_6, -2.050_523_729_110_041, -2.087_816_365_506_925_3, -2.119_791_277_088_348_5, -2.144_833_035_963_794_4, -2.163_070_581_279_977],
    [-0.593_724_111_591_899_3, -0.657_542_314_540_818, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7, -0.673_916_088_788_656_7],
    [0.134_439_018_698_883_2, 0.169_205_352_911_228_32, 0.271_624_559_718_026, 0.324_368_984_121_710_06, 0.372_613_939_989_419_7, 0.440_350_665_051_099_2, 0.483_371_215_276_857_34, 0.540_385_974_663_611_5, 0.603_730_716_605_646, 0.643_830_469_216_046_4, 0.691_768_678_977_872, 0.731_002_513_320_713_1, 0.763_701_937_310_655_7, 0.798_115_015_271_722_2, 0.814_788_099_569_435_3, 0.807_220_911_793_885_3, 0.740_432_775_122_006_9],
    [0.288_865_043_145_540_4, 0.240_170_719_530_362_5, -0.228_215_175_337_520_86, -0.395_247_709_016_861_35, -0.475_612_045_163_401_1, -0.556_011_857_399_485_9, -0.584_921_420_126_476, -0.600_016_387_716_512_6, -0.590_133_242_373_806_8, -0.573_808_068_397_428, -0.545_573_479_937_256_8, -0.491_344_456_642_631, -0.446_919_882_730_006_4, -0.427_703_099_372_830_3, -0.398_006_863_104_973_1, -0.034_425_331_437_608_38, 0.015_699_006_971_809_426],
    [9.966_560_564_454_614, 9.939_140_581_380_284, 9.916_476_544_337_057, 9.905_643_986_662_529, 9.896_920_055_626_259, 9.888_768_472_634_059, 9.883_136_301_723_415, 9.878_227_749_973_925, 9.866_510_607_425_077, 9.854_332_976_196_229, 9.854_762_951_908_02, 9.858_754_949_094_402, 9.869_024_042_404_02, 9.874_268_343_520_466, 9.873_229_386_973_511, 9.876_470_373_936_971, 9.890_553_315_044_812],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_27: [(f64, f64); 11] = [
    (5.419_747_950_083_047, 10.0),
    (0.312_982_541_851_950_85, 1.38),
    (-0.278_456_376_545_300_03, 1.971_095_891_507_528_3),
    (-0.783_839_570_029_425_4, 0.359_480_989_581_144_33),
    (1.886_327_497_983_178_3, 6.442_488_747_737_999_5),
    (-1.913_579_116_770_117_7, 0.209_409_169_898_635),
    (-2.264_699_308_176_646, 0.243_432_298_064_600_95),
    (-0.735_015_719_585_119, -0.125_262_481_778_891_35),
    (-0.051_977_808_826_974_16, 0.909_956_226_855_928_4),
    (-1.096_848_077_448_504_3, 0.620_755_738_826_302),
    (8.017_791_846_134_891, 10.079_132_695_443_889),
];

/// The slow accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_DONOR_KINDS_27: [f64; 3] = [
    0.903_855_704_250_816_2,
    0.095_546_419_942_860_2,
    0.000_597_875_806_323_54,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    1.853_577_153_912_22e-7,
    3.007_194_930_479_294_2e-6,
    1.612_573_022_325_941_2e-6,
    2.485_229_912_059_853_5e-6,
    2.826_178_319_181_957_5e-6,
    2.470_207_313_663_718e-6,
    8.171_561_968_582_12e-7,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.102_187_680_184_835_11,
    0.856_578_757_759_75,
    0.041_233_562_055_414_855,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    3.384_313_014_720_399e-7,
    7.247_913_645_160_988e-5,
    0.000_141_528_741_412_866_08,
    0.000_110_560_300_386_005_01,
    9.373_204_903_620_278e-5,
    9.548_212_998_151_102e-5,
    0.000_169_885_081_646_956_7,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.531_583_358_523_258_9,
    0.453_261_644_880_31,
    0.015_154_996_596_431_073,
];
// @end-table binary_awd

// @begin-table binary_xrb
// The X-ray binaries' sampler and share row fitted against plan 11's engine at generator version 27
// (plan 15, P15.T10.b), held until 27 (`catalogue_classes::binary`'s hold).
//
// @generated by hyperion-fit 0.1.0, task `binary_xrb` revision 0. Do not edit.
// inputs-sha256: 519618bd9567af6ddf2a07f1639885fbe012655854b1f95f345428a97ae7815e
// manifest: crates/hyperion-fit/manifests/binary_xrb.toml
// data: none
// sim-fingerprint: 72325b0f2658c5ace59c3c7457ed5f212fc813bf9f288a45647bcaceee8c700b
// since-generator-version: 24
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the total against the brainstorm's 10⁴, the black-hole transients pinned at
//   Corral-Santana et al.'s (2016, A&A 587, A61) observed 1,300
// acceptance: slab members by kind 8/916/2/36/7/0/795/2/0; young members by kind
//   14/64/109/55/1145/8/98/2/2; census kinds
//   0.0055/0.5849/0.0017/0.0196/0.0077/0.0001/0.3795/0.0010/0.0000; weights
//   0.0049/0.5190/0.0016/0.1300/0.3368/0.0009 and 0.0068/0.0000/0.0000; black-hole transients
//   1300 of 1e4 (the census's 196); high-mass hosts 69; donor mass's own nodes 10; layers C
//   0.011 D 0.524 E 0.465; rates a system
//   1.05e-4/3.93e-5/1.24e-5/6.31e-6/9.81e-6/1.82e-5/1.89e-5; scratch marks: BlackHoleMass,
//   NeutronStarRecurrence, BlackHoleRecurrence, SupergiantPeriod, SupergiantDonorMass,
//   SupergiantRate, SupergiantRecoil, SupergiantSinceSupernova, BlackHoleProgenitor,
//   UltracompactRecurrence, SymbioticPeriod, SymbioticDonorMass, SymbioticFilling,
//   BlackHoleHighMassAccretorMass, BlackHoleHighMassPeriod, BlackHoleHighMassDonorMass,
//   BlackHoleHighMassRate; the K–S tests against the other half of the census are the sim's slow
//   `tables_hold::the_x_ray_binary_sampler_against_the_engine`
//
// Read by `XRB_SAMPLER_27` and `CLASS_SHARES_27`, before the block. The marks are fitted to the
// engine's members of the nine kinds `XrbHostKind` holds at generator version 27's rules
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
pub const XRB_RESIDUALS_27: [[f64; 17]; 30] = [
    [1.150_547_897_432_618_9, 1.233_689_476_048_369, 1.260_679_668_773_270_2, 1.264_012_155_553_618, 1.267_929_836_169_681_4, 1.272_371_653_223_313_4, 1.275_725_520_704_896_8, 1.279_196_750_682_613, 1.282_663_526_500_34, 1.294_039_778_393_424, 1.304_881_084_458_426_7, 1.324_649_009_125_507_9, 1.357_017_721_214_630_4, 1.392_406_897_107_029_6, 1.511_531_865_681_117_9, 1.637_562_701_339_422_7, 1.980_991_864_166_536_3],
    [4.8, 6.010_368, 6.446_384_399_999_999_5, 6.752_618_399_999_999, 7.002_298_8, 7.221_349_2, 7.422_542_4, 7.613_590_8, 7.8, 7.986_409_2, 8.177_457_6, 8.378_650_799_999_999, 8.597_701_2, 8.847_381_6, 9.153_615_6, 9.589_632, 10.8],
    [-0.192_836_227_429_209_08, 0.408_664_299_109_845_8, 0.576_361_479_540_026, 0.800_117_036_003_243, 1.061_335_107_815_208_5, 1.116_759_636_462_952, 1.155_344_247_501_625_1, 1.190_656_642_129_617_2, 1.225_538_030_099_732_7, 1.261_516_498_555_422_1, 1.300_378_820_947_778_2, 1.367_660_881_413_159_4, 1.459_323_184_109_660_3, 1.614_150_400_048_358_2, 1.837_406_037_631_848_6, 2.199_549_258_335_501, 3.347_845_383_723_693_2],
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
    [-4.998_088_391_436_855, -2.588_858_898_247_783, -2.042_211_801_776_907_5, -1.806_702_794_833_526, -1.653_724_027_764_585, -1.587_952_005_746_800_4, -1.513_466_366_639_525_2, -1.434_127_234_104_644, -1.379_879_518_634_649, -1.318_201_468_424_789_8, -1.247_010_903_600_314_3, -1.161_726_899_229_109_2, -1.058_024_188_787_426_9, -0.911_567_593_362_503_1, -0.780_546_940_487_370_2, -0.450_298_197_517_384_1, 0.558_308_072_658_647_6],
    [1.75, 2.254_32, 2.435_993_5, 2.563_591, 2.667_624_5, 2.758_895_5, 2.842_726, 2.922_329_5, 3.0, 3.077_670_5, 3.157_274, 3.241_104_5, 3.332_375_5, 3.436_409, 3.564_006_5, 3.745_68, 4.25],
    [1.25, 1.754_319_999_999_999_9, 1.935_993_5, 2.063_591, 2.167_624_5, 2.258_895_5, 2.342_726, 2.422_329_5, 2.5, 2.577_670_5, 2.657_274, 2.741_104_5, 2.832_375_5, 2.936_409, 3.064_006_5, 3.245_68, 3.75],
    [0.224_583_558_522_893_8, 0.729_629_508_174_697_2, 0.888_235_645_564_952_7, 1.563_872_952_552_953_3, 1.679_111_595_832_241_6, 1.760_732_443_097_544_2, 1.841_540_708_792_467_1, 1.913_141_534_502_278_4, 1.959_502_962_199_623_4, 2.005_600_682_870_861_5, 2.060_777_146_711_930_5, 2.128_201_465_200_201_7, 2.197_017_598_562_989, 2.256_961_232_138_178_5, 2.354_576_078_380_209, 2.472_599_178_896_401_5, 2.984_609_187_822_059_5],
    [0.0, 0.027_792_898_575_942_356, 0.039_548_128_976_030_86, 0.049_601_136_415_902_64, 0.060_071_932_682_441_78, 0.067_925_153_710_729_94, 0.076_706_096_214_699_25, 0.085_670_531_332_290_36, 0.095_969_477_632_126_57, 0.109_899_325_920_823_08, 0.123_117_899_128_496_2, 0.138_502_820_788_984_96, 0.160_377_146_257_275_64, 0.196_626_676_302_368_35, 0.272_593_756_784_327_35, 0.364_453_494_028_595_67, 0.986_854_386_182_416_4],
    [8.003_154_553_454_676, 8.315_866_906_979_13, 8.664_421_525_491_99, 8.997_703_424_635_551, 9.341_847_154_508_278, 9.662_167_796_964_091, 9.955_961_840_624_15, 10.350_574_101_844_92, 10.699_218_594_498_571, 11.035_224_084_815_225, 11.453_157_954_347_29, 11.923_573_119_342_391, 12.522_680_446_584_904, 13.237_533_382_169_955, 13.971_221_750_143_751, 15.460_285_634_939_91, 20.678_578_766_257_51],
    [-0.150_000_000_000_000_02, 0.152_592, 0.261_596_1, 0.338_154_599_999_999_97, 0.400_574_699_999_999_95, 0.455_337_299_999_999_94, 0.505_635_6, 0.553_397_7, 0.6, 0.646_602_3, 0.694_364_4, 0.744_662_7, 0.799_425_3, 0.861_845_4, 0.938_403_9, 1.047_408, 1.35],
    [10.0, 14.034_559_999_999_999, 15.487_948, 16.508_728, 17.340_996, 18.071_164, 18.741_808, 19.378_636, 20.0, 20.621_364, 21.258_192, 21.928_836, 22.659_004, 23.491_272, 24.512_052, 25.965_44, 30.0],
    [-10.25, -9.745_68, -9.564_006_5, -9.436_409, -9.332_375_5, -9.241_104_5, -9.157_274, -9.077_670_5, -9.0, -8.922_329_5, -8.842_726, -8.758_895_5, -8.667_624_5, -8.563_591, -8.435_993_5, -8.254_32, -7.75],
    [0.138_508_588_504_407_45, 1.070_579_915_414_710_6, 1.688_727_194_440_198_5, 2.261_110_625_097_366_7, 2.753_529_734_259_164, 3.246_103_058_182_427, 3.881_118_341_719_044, 4.478_718_671_272_78, 4.944_535_794_925_734, 5.400_602_352_438_715, 6.039_929_899_684_254, 6.809_832_047_234_167, 7.778_098_463_581_235, 9.857_494_055_256_625, 21.311_766_910_937_614, 80.322_087_994_354_48, 272.816_835_604_796_55],
    [0.0, 21.541_471_172_88, 27.883_959_356_160_005, 32.722_835_660_352_004, 36.900_838_535_952_005, 40.732_034_499_504, 44.381_465_974_608_005, 47.957_508_026_832, 51.545_915_694_144_01, 55.228_255_120_512, 59.095_911_217_440_005, 63.268_150_174_896, 67.923_921_544_560_02, 73.375_080_106_944_01, 80.282_901_424_464, 90.636_775_035_600_01, 120.071_697_528_624_01],
    [4.174_421_988_060_552, 6.145_042_821_616_532, 6.331_225_807_422_328, 6.430_162_381_090_373, 6.530_961_068_491_383, 6.608_272_603_125_759_5, 6.677_628_339_644_947, 6.762_024_034_417_307_5, 6.840_340_260_816_6, 6.904_765_846_845_629, 6.962_341_956_800_831, 7.015_626_106_804_297, 7.071_354_460_595_864, 7.126_002_332_412_687, 7.192_983_150_295_578_6, 7.264_437_714_403_596, 7.457_852_875_750_245],
    [5.7, 5.901_728, 5.974_397_4, 6.025_436_4, 6.067_049_8, 6.103_558_2, 6.137_090_4, 6.168_931_8, 6.2, 6.231_068_2, 6.262_909_6, 6.296_441_8, 6.332_950_2, 6.374_563_6, 6.425_602_6, 6.498_272, 6.7],
    [0.318_354_258_786_814_9, 0.446_824_849_146_446_05, 0.480_457_730_287_660_9, 0.509_239_182_918_239_3, 0.541_602_358_979_749_3, 0.637_519_717_540_944_2, 0.858_858_500_000_789_8, 0.876_838_747_419_622_8, 0.892_348_491_763_602_3, 0.908_293_837_136_315_2, 0.923_592_311_947_105_7, 0.940_514_079_731_405_8, 0.962_952_597_880_429_4, 1.003_056_109_292_671_3, 1.047_228_564_425_111_1, 1.084_196_856_424_229_7, 1.318_919_933_041_615],
    [1.23, 1.330_864, 1.367_198_699_999_999_9, 1.392_718_2, 1.413_524_9, 1.431_779_1, 1.448_545_2, 1.464_465_9, 1.48, 1.495_534_1, 1.511_454_799_999_999_9, 1.528_220_9, 1.546_475_099_999_999_9, 1.567_281_8, 1.592_801_3, 1.629_136, 1.73],
    [-0.044_357_294_101_395_356, 0.125_484_115_215_383_7, 0.201_989_108_630_857_98, 0.272_684_544_590_224_34, 0.314_315_982_731_986_8, 0.346_928_654_467_234_64, 0.384_814_273_163_058_2, 0.416_013_774_888_552_73, 0.448_789_951_441_476_54, 0.489_829_953_390_018_8, 0.577_399_024_603_239_1, 0.704_151_122_167_127_5, 0.857_866_665_466_730_3, 0.993_662_024_693_341_2, 1.095_388_599_396_599_5, 1.250_860_085_958_368_3, 2.368_771_197_224_294_4],
    [1.052_269_568_369_403_9, 1.573_606_438_721_037_1, 1.644_949_996_517_274, 1.693_549_084_135_141, 1.730_387_944_747_658_4, 1.756_178_333_496_167_6, 1.780_404_107_053_317_4, 1.801_478_332_547_461, 1.815_511_297_806_854, 1.830_375_081_250_665_2, 1.844_850_210_386_514_8, 1.855_152_272_606_465, 1.864_353_608_390_758_6, 1.873_259_023_314_376_9, 1.880_701_753_172_957_7, 1.886_908_629_849_493_7, 1.903_377_954_052_515_8],
    [0.0, 0.403_455_999_999_999_9, 0.548_794_8, 0.650_872_799_999_999_9, 0.734_099_6, 0.807_116_4, 0.874_180_8, 0.937_863_6, 1.0, 1.062_136_4, 1.125_819_2, 1.192_883_6, 1.265_900_4, 1.349_127_2, 1.451_205_2, 1.596_544_000_000_000_2, 2.0],
    [0.109_469_220_667_324_5, 0.304_750_653_289_642_35, 0.319_555_589_126_330_3, 0.328_668_790_391_881_1, 0.337_464_898_184_178_46, 0.344_493_768_252_702_34, 0.353_372_533_765_763_9, 0.361_347_480_587_553_3, 0.369_042_469_172_356_34, 0.378_980_270_187_808_17, 0.389_435_367_280_606_6, 0.400_155_835_314_746_13, 0.410_844_844_202_860_4, 0.419_239_614_850_400_9, 0.431_092_493_803_340_86, 0.453_942_388_915_748_06, 0.814_900_717_507_095_9],
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
pub const XRB_DONOR_MASS_NODE_RESIDUALS_27: [[f64; 17]; 17] = [
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
    [-0.960_003_698_578_555_7, -0.915_222_757_342_026_7, -0.893_520_523_759_760_5, -0.803_199_596_427_399_6, -0.642_304_178_172_823_8, -0.369_842_218_040_168_54, -0.171_575_781_293_670_82, -0.100_264_962_822_091_64, -0.070_860_988_937_857_17, -0.054_866_528_147_288_6, -0.035_108_746_444_442_61, -0.000_908_419_178_870_276_5, 0.000_830_651_066_094_570_2, 0.030_185_003_237_880_124, 0.074_900_227_401_863_35, 0.134_431_703_331_048_3, 0.582_388_300_880_676],
    [-0.960_003_698_578_555_7, -0.803_179_762_637_988_1, -0.218_104_045_885_934_4, -0.150_478_184_456_428_52, -0.096_663_486_001_878_07, -0.071_124_260_522_999_49, -0.055_008_821_119_233_47, -0.041_967_407_163_660_18, -0.019_637_539_854_387_615, -0.003_691_658_176_027_492_5, 0.007_045_965_153_572_557, 0.020_478_187_146_492_296, 0.046_350_044_694_813_34, 0.067_488_940_451_344_35, 0.077_038_953_125_366_04, 0.100_581_719_073_333_41, 0.582_388_300_880_676],
    [-0.960_003_698_578_555_7, -0.169_620_967_444_439_08, -0.130_540_897_030_592_57, -0.094_631_713_965_611_86, -0.057_625_009_300_530_11, -0.042_099_309_656_393_94, -0.025_388_788_611_236_14, -0.007_247_993_918_622_943, 0.004_713_489_307_484_912, 0.015_879_567_675_961_55, 0.025_024_660_642_704_03, 0.046_340_568_296_583_985, 0.058_683_915_001_327_226, 0.070_196_209_224_703_17, 0.087_339_365_596_349_87, 0.113_061_094_370_026_7, 0.360_415_037_228_669_2],
    [-0.512_640_947_740_742_4, -0.187_112_403_215_786_82, -0.131_495_974_090_043_96, -0.114_574_154_822_952_37, -0.075_564_185_023_821_17, -0.048_273_663_587_982_844, -0.035_505_422_657_417_275, -0.024_756_122_407_588_08, -0.012_902_787_724_283_273, -0.002_339_264_210_376_861_5, 0.007_961_262_340_391_923, 0.019_689_181_323_592_462, 0.035_573_096_062_227_19, 0.060_801_437_927_035_615, 0.088_097_749_946_612_51, 0.181_732_790_048_793_34, 0.360_415_037_228_669_2],
    [-0.628_859_576_027_716_1, -0.150_051_636_188_833_27, -0.116_128_932_243_034_61, -0.085_285_553_785_598_34, -0.064_792_590_220_019_44, -0.045_279_924_118_453_18, -0.031_244_576_688_832_09, -0.021_627_435_594_366_008, -0.010_292_745_165_548_17, -0.002_262_541_905_826_760_2, 0.004_594_110_062_958_631, 0.009_177_278_973_392_112, 0.018_281_638_971_508_772, 0.030_891_470_878_848_065, 0.046_703_777_293_255_61, 0.087_654_916_321_130_37, 0.360_415_037_228_669_2],
    [-0.668_255_477_578_996_8, -0.159_260_295_689_803_77, -0.104_691_804_566_944_75, -0.077_401_375_839_270_38, -0.052_966_456_234_184_764, -0.031_020_935_769_862_33, -0.019_038_717_449_308_01, -0.005_944_017_863_191_109, 0.001_672_535_905_279_075, 0.006_885_629_377_526_84, 0.015_206_617_863_406_01, 0.024_933_479_681_296_84, 0.032_769_069_173_062_486, 0.044_269_030_770_960_67, 0.065_288_827_683_633_28, 0.098_381_033_357_267_93, 0.360_415_037_228_669_2],
    [-0.950_325_202_910_714_4, -0.256_955_522_268_857_76, -0.142_659_813_000_619_5, -0.086_249_621_959_769_4, -0.053_897_602_092_703_11, -0.023_353_928_282_272_05, -0.004_525_799_506_138_059, 0.006_628_252_543_875_981, 0.015_289_961_564_347_887, 0.027_932_616_397_727_33, 0.034_786_258_339_901_94, 0.044_255_079_288_591_03, 0.053_777_541_811_182_03, 0.082_968_675_123_010_76, 0.115_130_769_094_312_88, 0.206_028_987_220_763_15, 0.539_710_865_576_478_4],
    [-0.950_325_202_910_714_4, -0.419_450_195_609_452_7, -0.320_451_906_748_274_06, -0.242_776_384_168_860_77, -0.137_694_617_576_175_74, -0.056_078_254_994_704_79, 0.001_789_195_881_064_747_8, 0.028_744_015_903_849_295, 0.036_772_000_053_934_335, 0.047_577_654_431_350_95, 0.053_785_240_967_590_01, 0.085_793_607_021_838_51, 0.122_792_880_479_165_87, 0.187_474_400_600_416_73, 0.258_322_994_228_635_14, 0.372_888_621_841_627_1, 0.589_245_293_535_213_9],
    [-0.950_325_202_910_714_4, -0.449_239_538_444_981_55, -0.342_583_849_220_133_9, -0.300_853_297_210_287_5, -0.243_643_257_177_393_82, -0.181_290_769_256_300_7, -0.119_916_918_526_441_89, -0.056_227_625_562_524_97, 0.034_891_059_084_678_974, 0.048_080_320_250_108_874, 0.093_576_727_315_291_28, 0.170_206_469_263_997_3, 0.231_060_058_159_328_92, 0.323_102_749_012_139_03, 0.431_098_819_072_487_87, 0.549_540_970_520_930_9, 0.589_245_293_535_213_9],
    [-0.950_325_202_910_714_4, -0.373_741_238_807_940_97, -0.260_065_557_974_057_84, -0.217_899_428_969_452_83, -0.181_368_035_193_601_76, -0.152_592_263_631_398_4, -0.110_031_621_804_920_85, -0.099_014_413_153_423_21, -0.055_770_907_637_971_504, 0.000_307_035_905_415_505_36, 0.086_260_684_211_171_39, 0.093_863_459_036_110_07, 0.173_350_964_373_386_15, 0.297_452_384_307_337_2, 0.430_759_637_322_074_7, 0.519_642_473_681_970_5, 1.020_207_382_647_117_6],
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
    [-0.960_003_698_578_555_7, -0.326_891_663_952_325_4, -0.184_978_724_063_756_6, -0.120_354_551_843_080_85, -0.085_348_151_265_160_49, -0.054_499_694_301_475_665, -0.030_927_890_591_036_077, -0.010_702_590_135_506_264, 0.000_839_591_468_572_018_1, 0.010_767_356_822_826_674, 0.026_073_424_460_494_807, 0.037_459_062_070_092_6, 0.054_583_618_902_270_74, 0.088_515_270_568_986_7, 0.141_507_133_557_776_14, 0.258_440_411_647_115_56, 1.020_207_382_647_117_6],
];

/// The X-ray binaries' marks' medians' spans of their conditions, first and last node, in
/// [`XrbMark`]'s order; only the low-mass donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIAN_SPANS_27: [(f64, f64); 30] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.051_311_812_409_660_346, 3.037_055_499_238_799),
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
pub const XRB_MEDIANS_27: [[f64; 17]; 30] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-1.658_792_468_229_203_5, -0.894_074_510_405_889_3, -0.681_007_282_379_932_2, -0.593_788_257_749_047_9, -0.493_523_429_058_234_4, 0.073_259_059_196_606_65, 0.103_789_912_861_125_11, 0.123_977_580_262_783_5, 0.165_746_848_076_331_4, -0.121_326_820_507_497_52, -0.285_734_322_074_559_3, -0.359_386_402_098_793_47, -0.425_269_380_164_140_5, -0.470_892_012_158_291_63, -0.470_892_012_158_291_63, -0.470_892_012_158_291_63, -0.470_892_012_158_291_63],
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
pub const XRB_LIMITS_27: [(f64, f64); 30] = [
    (1.150_547_897_432_618_9, 1.980_991_864_166_536_3),
    (5.0, 15.0),
    (-0.192_836_227_429_209_08, 3.347_845_383_723_693_2),
    (-1.842_462_294_429_793_5, 0.641_483_906_500_898_3),
    (-4.998_088_391_436_855, 0.558_308_072_658_647_6),
    (1.75, 4.25),
    (1.25, 3.75),
    (0.224_583_558_522_893_8, 2.984_609_187_822_059_5),
    (0.0, 0.986_854_386_182_416_4),
    (8.003_154_553_454_676, 20.678_578_766_257_51),
    (0.1, 1.2),
    (10.0, 35.0),
    (-10.25, -7.75),
    (0.138_508_588_504_407_45, 272.816_835_604_796_55),
    (0.0, 120.1),
    (4.174_421_988_060_552, 7.457_852_875_750_245),
    (5.5, 6.7),
    (0.318_354_258_786_814_9, 1.318_919_933_041_615),
    (1.322_219, 1.778_151),
    (-0.044_357_294_101_395_356, 2.368_771_197_224_294_4),
    (1.052_269_568_369_403_9, 1.903_377_954_052_515_8),
    (0.0, 2.0),
    (0.109_469_220_667_324_5, 0.814_900_717_507_095_9),
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
pub const XRB_LOW_MASS_KINDS_27: [f64; 6] = [
    0.004_884_328_443_558_895,
    0.518_999_982_005_366_7,
    0.001_550_709_669_218_423_9,
    0.13,
    0.336_758_320_381_700_56,
    0.000_911_995_788_979_455_4,
];

/// The high-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.
#[rustfmt::skip]
pub const XRB_HIGH_MASS_KINDS_27: [f64; 3] = [
    0.006_834_970_825_172_426,
    4.775_430_880_291_354e-5,
    1.193_857_720_072_838_5e-5,
];

/// The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const XRB_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    4.390_305_012_512_863_5e-7,
    1.644_110_076_128_027_4e-7,
    5.202_877_444_750_964_4e-8,
    2.636_904_700_372_173e-8,
    4.102_536_620_434_315_4e-8,
    7.617_793_176_394_038e-8,
    7.911_330_478_986_362e-8,
];

/// The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const XRB_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.010_748_170_505_599_569,
    0.524_382_305_587_311_6,
    0.464_869_523_907_088_9,
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
