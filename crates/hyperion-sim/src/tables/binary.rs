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
// sim-fingerprint: 7e786303287be2da402a5458cd09202bd8cc004d62d5e86c3c1f8b2ca2dec9ac
// since-generator-version: 24
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the rows by donor kind against Pala et al. 2020, MNRAS 494, 3799, Carter et al. 2013,
//   MNRAS 429, 2143, and Lü, Yungelson and Han 2006, MNRAS 372, 1389, the total against the
//   brainstorm's 6–12 × 10⁶
// acceptance: kind scales 2.9035e-1/5.7242e-2/2.3102e-2; fast: 660 members, own nodes
//   0/9/11/0/4/13/0/4/13/11/11, donors 0.967/0.000/0.033, layers C 0.113 D 0.866 E 0.020, rates
//   a system 7.01e-7/1.41e-5/1.11e-5/1.68e-5/1.54e-5/1.33e-5/7.71e-6, not its own: HeliumPeriod
//   Scratch, HeliumDonorMass Scratch; slow: 19196 members, own nodes
//   0/16/17/16/4/15/17/4/17/17/17, donors 0.911/0.088/0.000, layers C 0.511 D 0.472 E 0.017,
//   rates a system 8.41e-6/3.85e-4/7.50e-4/5.41e-4/4.75e-4/4.80e-4/9.24e-4, not its own:
//   GiantPeriod Pooled, GiantDonorMass Pooled; the K–S tests against the other half of the
//   census are the sim's slow
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
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-1.200_204_083_082_625, -0.509_020_252_723_337_6, -0.353_320_278_944_552_7, -0.175_054_521_436_731_72, -0.115_757_368_278_181_06, -0.078_275_586_866_856_89, -0.048_260_209_251_860_35, -0.030_254_932_681_778_53, -0.000_546_551_132_533_934_9, 0.020_151_078_914_535_07, 0.051_730_193_208_939_21, 0.118_124_514_946_108_32, 0.408_009_864_140_245_2, 0.679_604_822_152_086_2, 0.768_243_956_020_020_9, 0.871_452_665_534_376_5, 1.009_927_306_338_269_4],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.377_219_589_614_803_84, -0.127_436_620_777_885_67, -0.093_002_400_858_919_6, -0.078_174_396_946_966_7, -0.060_341_919_118_998_805, -0.045_065_351_466_540_98, -0.031_278_040_114_043_87, -0.022_474_165_651_258_083, -0.010_559_829_520_106_58, 0.003_509_105_627_785_603, 0.015_992_836_192_781_74, 0.030_096_971_964_667_29, 0.045_147_164_512_387_98, 0.059_122_469_101_189_44, 0.078_356_318_693_310_61, 0.127_471_461_527_905, 0.452_805_398_203_227_1],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.486_129_220_719_366_54, -0.310_229_391_153_095_1, -0.136_127_964_927_725_72, -0.038_045_401_734_295_256, -0.022_679_666_794_898_57, -0.013_217_839_438_041_94, -0.008_250_652_435_656_58, -0.003_851_616_865_085_881_4, 0.001_753_531_606_691_046_2, 0.005_202_050_473_556_658, 0.009_188_574_618_628_643, 0.013_164_987_152_264_22, 0.017_023_416_516_428_46, 0.023_847_964_523_316_074, 0.031_831_535_775_517_034, 0.045_757_829_083_603_65, 0.181_877_261_513_583_52],
    [-0.470_012_071_605_791_95, -0.133_637_241_446_674_88, -0.082_172_917_913_668_44, -0.063_608_075_944_134_22, -0.045_986_983_058_839_29, -0.032_275_345_712_794_37, -0.018_904_207_337_672_75, -0.009_586_799_652_626_39, 0.000_265_137_378_016_270_9, 0.008_117_252_046_851_556, 0.016_924_498_916_329_783, 0.026_603_785_394_445_214, 0.037_222_050_273_461_99, 0.049_592_663_862_333_92, 0.065_205_824_805_541_1, 0.081_883_703_021_274_66, 0.246_315_491_199_618_54],
    [-1.537_315_209_469_474_2, -0.717_565_434_678_378_9, -0.508_859_330_493_889_4, -0.356_409_873_637_091_15, -0.239_623_381_078_315_8, -0.173_191_326_075_652_05, -0.094_210_052_879_314_36, -0.042_867_403_893_676_71, 0.001_068_889_917_049_821_4, 0.033_392_398_040_667_86, 0.063_800_223_319_321_89, 0.099_426_163_977_055_42, 0.123_561_865_764_174_34, 0.155_776_465_669_281_86, 0.180_964_302_688_689_04, 0.213_875_095_376_486_5, 0.340_666_480_571_501_4],
];

/// The fast accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_FAST_NODE_RESIDUALS_27: [[f64; 17]; 187] = [
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [2.946_395_681_233_526_4, 3.437_246_025_614_596_2, 3.752_082_282_273_199, 4.096_997_988_609_574, 4.394_704_642_353_509, 4.559_015_602_423_710_5, 4.673_845_145_677_006, 4.780_024_171_536_083, 4.874_287_460_298_066, 4.941_673_561_454_537, 5.026_084_593_617_513, 5.109_010_444_449_519_5, 5.190_295_740_726_5, 5.241_983_404_873_452, 5.296_298_695_173_606, 5.348_931_742_553_959, 5.414_295_786_640_511],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.856_499_905_176_875_5, -0.526_603_574_345_214_3, -0.451_406_335_171_867_9, -0.344_620_909_941_967_7, -0.211_867_956_788_723_75, -0.146_418_293_150_631_43, -0.091_347_912_955_638_17, -0.037_526_699_040_651_37, 0.001_059_240_293_260_342_4, 0.024_439_486_525_796_1, 0.045_958_544_918_400_02, 0.066_757_152_866_901_44, 0.089_902_117_570_784_24, 0.109_097_699_604_456_74, 0.132_568_997_931_128_97, 0.197_006_365_246_079_73, 0.417_050_522_851_581_3],
    [-0.554_978_698_827_897_5, -0.240_449_392_364_707_28, -0.178_862_698_714_295_75, -0.151_373_969_173_887_82, -0.105_752_113_764_396_47, -0.078_202_628_631_509_35, -0.037_146_767_970_537_32, -0.027_694_033_850_521_806, -0.002_038_905_433_969_671, 0.029_176_080_735_291_425, 0.042_583_895_912_370_52, 0.057_084_849_330_747_68, 0.075_725_633_453_266_62, 0.106_592_499_739_385_87, 0.133_710_857_635_427_2, 0.213_871_325_648_460_56, 0.248_785_713_102_306_57],
    [-0.591_069_806_043_728_4, -0.317_897_039_193_806_96, -0.216_204_378_628_982_95, -0.178_551_620_588_765_67, -0.122_376_181_915_492_72, -0.068_309_797_855_520_5, -0.034_716_527_990_813_26, -0.026_184_956_156_077_65, -0.001_786_185_018_869_368, 0.034_246_377_873_520_355, 0.053_107_658_018_654_726, 0.067_569_443_388_130_62, 0.106_694_980_747_248_08, 0.150_660_146_919_235_52, 0.209_924_170_500_206_15, 0.241_729_376_754_526_27, 0.248_785_713_102_306_57],
    [-0.591_069_806_043_728_4, -0.497_023_701_949_702_97, -0.342_241_580_742_373_1, -0.262_409_348_280_699_7, -0.168_602_395_714_410_55, -0.114_135_698_316_542_62, -0.048_512_093_810_534_68, -0.032_534_060_642_169_68, -0.005_660_400_267_106_003_4, 0.028_538_609_108_764_35, 0.059_140_158_479_230_43, 0.112_419_756_063_470_93, 0.155_918_351_685_929_66, 0.196_715_383_034_094_86, 0.207_704_731_142_049_65, 0.232_336_370_460_530_77, 0.248_785_713_102_306_57],
    [-0.605_351_607_518_697_8, -0.524_828_219_529_950_3, -0.440_245_827_360_440_7, -0.348_146_877_695_958_6, -0.200_985_847_775_493_75, -0.139_864_857_646_963_44, -0.091_313_162_209_122_58, -0.037_120_486_177_091_294, -0.002_216_050_263_990_371_5, 0.029_853_583_788_576_863, 0.097_371_206_639_548_8, 0.121_783_986_519_019_9, 0.153_958_155_297_900_27, 0.170_027_004_903_098_1, 0.197_623_482_001_253_3, 0.210_583_365_164_305_34, 0.248_785_713_102_306_57],
    [-0.605_351_607_518_697_8, -0.533_411_546_866_079_9, -0.467_959_286_080_685_43, -0.374_064_950_493_272_04, -0.226_057_201_827_669, -0.170_980_666_548_914_45, -0.091_198_467_608_028_04, -0.039_469_665_543_549_78, 0.005_885_365_877_787_864, 0.063_323_232_448_368_65, 0.092_308_464_346_024_2, 0.110_555_817_480_701_13, 0.123_711_658_704_808_85, 0.136_768_710_016_402_62, 0.155_870_848_024_651_95, 0.179_018_840_294_917_82, 0.237_994_374_038_575_93],
    [-0.605_351_607_518_697_8, -0.526_801_405_860_185_6, -0.466_495_967_644_124, -0.375_837_814_595_053_67, -0.239_387_103_393_502_3, -0.188_594_866_168_163_82, -0.107_247_525_290_509_03, -0.067_444_600_859_066_81, 0.006_152_747_635_328_994, 0.057_056_207_931_995_41, 0.080_620_209_857_611_6, 0.094_893_632_817_136_03, 0.106_532_161_877_573_8, 0.118_015_340_773_038_3, 0.127_400_207_347_951_52, 0.146_874_892_692_437_48, 0.211_906_652_905_625_05],
    [-0.639_603_426_170_044_3, -0.534_221_677_560_549_3, -0.467_091_378_474_379_76, -0.383_080_912_078_935_74, -0.259_257_513_264_485_5, -0.197_498_246_906_990_08, -0.121_815_827_347_864_85, -0.076_134_004_546_267_19, -0.009_226_611_502_859_762, 0.023_822_303_029_803_506, 0.049_554_857_057_686_925, 0.066_044_217_561_313_03, 0.080_098_817_613_270_66, 0.093_721_066_547_797_83, 0.105_341_335_180_237_96, 0.125_030_964_022_522_5, 0.160_223_085_424_894_72],
    [-0.639_603_426_170_044_3, -0.526_620_983_333_097, -0.467_699_548_910_422_87, -0.409_441_126_808_382_8, -0.258_532_774_001_516_14, -0.182_779_815_549_022_67, -0.099_474_707_828_985_33, -0.047_328_930_611_691_21, -0.000_524_765_710_216_458_6, 0.018_524_493_588_376_694, 0.034_668_793_695_621_515, 0.044_874_709_810_976_03, 0.054_580_763_619_997_25, 0.070_353_034_682_279_4, 0.087_324_093_041_010_49, 0.100_292_193_230_800_99, 0.160_223_085_424_894_72],
    [-0.639_603_426_170_044_3, -0.505_131_830_903_766_5, -0.466_363_399_043_848_9, -0.398_105_826_494_120_7, -0.234_193_881_916_336_03, -0.140_788_285_358_382_6, -0.087_577_546_783_547_82, -0.016_086_105_767_757_24, 0.001_233_719_770_394_498_7, 0.016_434_652_320_733_007, 0.027_395_207_440_415_414, 0.036_876_025_270_580_99, 0.046_302_339_699_804_59, 0.054_293_410_648_460_87, 0.071_225_344_006_265_37, 0.093_724_350_849_315_03, 0.160_223_085_424_894_72],
    [-0.442_409_404_353_608_76, -0.370_142_835_134_870_9, -0.199_994_231_419_918_96, -0.142_273_328_763_366_68, -0.135_432_430_683_466_36, -0.081_682_094_133_477_24, -0.060_789_425_209_695_93, -0.028_399_066_752_908_5, 0.008_124_345_051_098_667, 0.027_091_486_631_794_95, 0.059_927_165_190_695_41, 0.110_752_252_279_156_08, 0.135_121_930_722_242_96, 0.173_297_770_367_432_5, 0.233_157_598_103_170_94, 0.620_514_926_764_589_9, 0.904_901_948_862_725_7],
    [-0.532_471_506_546_331_5, -0.355_983_170_869_768_94, -0.224_674_667_831_379, -0.160_495_221_826_297_33, -0.136_967_402_102_793, -0.097_947_914_671_929_8, -0.069_709_372_678_098_32, -0.039_880_301_575_566_85, -0.009_094_214_338_061_055, 0.009_043_418_777_793_567, 0.022_123_943_246_025_79, 0.040_489_900_787_324_01, 0.065_456_669_443_322_04, 0.106_323_479_233_763_3, 0.156_701_649_459_500_38, 0.576_324_522_204_182_7, 0.904_901_948_862_725_7],
    [-1.069_407_866_984_669_4, -0.464_912_340_914_167_17, -0.308_537_768_870_067_5, -0.213_673_187_887_474_22, -0.139_291_197_110_658_82, -0.096_555_983_212_895_31, -0.053_457_255_725_291_995, -0.027_781_230_902_904_793, -0.004_161_172_271_631_275, 0.010_137_483_267_657_567, 0.023_689_171_574_532_975, 0.040_485_492_183_743_46, 0.062_082_531_741_298_526, 0.095_176_574_822_837_63, 0.183_884_070_684_808_58, 0.629_109_974_935_514_6, 0.904_901_948_862_725_7],
    [-1.200_204_083_082_625, -0.624_811_516_756_413, -0.503_403_672_533_659_6, -0.378_466_309_788_136_4, -0.239_224_640_161_673_72, -0.098_394_609_885_834_23, -0.029_491_026_076_059_034, 4.749_550_745_950_301e-5, 0.014_864_928_368_372_988, 0.035_221_977_835_669_695, 0.069_283_884_203_791_61, 0.095_200_114_226_602_1, 0.282_461_251_004_620_44, 0.449_090_167_955_672_2, 0.571_556_678_971_316_2, 0.700_921_840_296_911_1, 0.879_453_413_718_774_7],
    [-1.200_204_083_082_625, -1.149_253_335_994_500_5, -1.128_472_862_461_118_4, -0.749_360_953_639_907_4, -0.512_535_908_108_764_9, -0.474_479_185_644_683, -0.024_777_283_105_493_57, 0.014_512_122_438_473_937, 0.073_357_396_324_218_8, 0.108_125_388_207_340_68, 0.187_510_722_142_705_17, 0.298_182_122_207_347_47, 0.401_546_729_785_441_8, 0.462_716_248_493_236_2, 0.568_114_258_441_026, 0.722_881_339_706_689_7, 1.001_175_235_093_050_3],
    [-1.200_204_083_082_625, -1.162_766_689_810_94, -1.148_152_961_161_247, -1.132_810_800_886_480_1, -0.510_068_235_942_887_9, -0.477_691_502_280_994_83, -0.067_894_744_092_038_79, -0.020_986_051_788_774_263, 0.000_784_309_796_400_911_8, 0.030_490_961_947_711_59, 0.081_139_127_333_284_66, 0.155_630_222_645_907_2, 0.305_440_630_671_219_85, 0.459_689_256_798_634_3, 0.734_704_275_322_037_2, 0.893_736_534_065_884_6, 1.001_175_235_093_050_3],
    [-1.200_204_083_082_625, -1.146_506_198_035_589_7, -0.501_194_118_842_197_1, -0.101_093_324_407_829_34, -0.076_559_248_799_664_35, -0.055_958_307_189_281_555, -0.037_284_550_260_933_08, -0.021_298_916_704_584_246, 0.001_011_621_930_143_164_6, 0.023_843_580_108_551_293, 0.076_176_951_581_402_75, 0.406_477_562_418_477_1, 0.713_209_300_688_102_1, 0.805_781_207_220_896_6, 0.879_728_926_374_255_5, 0.917_265_163_693_846, 1.001_175_235_093_050_3],
    [-1.177_904_153_322_845_8, -0.157_555_025_501_008_94, -0.097_559_955_885_966_89, -0.083_908_771_926_241_55, -0.061_768_148_051_025_51, -0.048_259_350_782_33, -0.037_446_423_842_029_54, -0.026_631_716_162_332_064, 0.006_570_958_739_337_732_5, 0.031_032_821_565_023_93, 0.523_942_182_773_111_4, 0.697_347_798_512_230_7, 0.761_442_368_410_037_4, 0.802_545_051_922_303_3, 0.878_059_600_195_756_5, 0.904_106_795_542_286_8, 1.001_175_235_093_050_3],
    [-0.521_712_189_817_649, -0.145_185_579_956_740_56, -0.122_473_612_267_913_2, -0.090_833_174_074_011_59, -0.065_790_781_121_981_79, -0.053_899_011_571_213_81, -0.042_225_909_217_483_57, -0.031_794_427_514_065_124, 0.002_999_276_777_664_678_7, 0.028_332_916_989_582_52, 0.640_866_527_938_154_6, 0.702_403_576_259_732_5, 0.767_726_681_116_112_5, 0.792_892_507_165_089_9, 0.846_915_500_331_353_3, 0.896_422_586_891_925, 0.982_058_048_939_208_5],
    [-0.485_046_943_217_669_34, -0.188_727_627_700_862_6, -0.139_059_885_925_943_73, -0.127_030_440_693_391_42, -0.094_104_241_985_357_94, -0.063_133_683_230_246_94, -0.049_994_958_587_246_79, -0.039_496_768_759_627_605, -0.019_992_155_851_808_743, 0.009_708_673_338_414_292, 0.569_953_070_334_302_4, 0.697_708_049_865_230_4, 0.763_922_669_308_970_1, 0.785_707_214_334_350_9, 0.834_186_551_976_966_9, 0.882_696_470_615_718_4, 0.982_058_048_939_208_5],
    [-0.475_080_185_835_159_76, -0.357_400_997_516_318_43, -0.178_954_783_385_040_64, -0.149_717_738_509_257_35, -0.127_049_970_493_556_5, -0.091_820_781_494_183_88, -0.066_698_915_942_790_5, -0.053_431_099_342_393_346, -0.039_327_166_141_786_84, -0.029_308_444_054_353_55, 0.019_040_408_885_588_854, 0.637_364_509_997_207_5, 0.706_971_723_214_497_1, 0.772_029_660_078_12, 0.821_624_201_718_285_1, 0.875_921_473_191_078_8, 0.977_849_308_994_872_3],
    [-1.200_204_083_082_625, -0.509_020_252_723_337_6, -0.353_320_278_944_552_7, -0.175_054_521_436_731_72, -0.115_757_368_278_181_06, -0.078_275_586_866_856_89, -0.048_260_209_251_860_35, -0.030_254_932_681_778_53, -0.000_546_551_132_533_934_9, 0.020_151_078_914_535_07, 0.051_730_193_208_939_21, 0.118_124_514_946_108_32, 0.408_009_864_140_245_2, 0.679_604_822_152_086_2, 0.768_243_956_020_020_9, 0.871_452_665_534_376_5, 1.009_927_306_338_269_4],
    [-1.200_204_083_082_625, -0.509_020_252_723_337_6, -0.353_320_278_944_552_7, -0.175_054_521_436_731_72, -0.115_757_368_278_181_06, -0.078_275_586_866_856_89, -0.048_260_209_251_860_35, -0.030_254_932_681_778_53, -0.000_546_551_132_533_934_9, 0.020_151_078_914_535_07, 0.051_730_193_208_939_21, 0.118_124_514_946_108_32, 0.408_009_864_140_245_2, 0.679_604_822_152_086_2, 0.768_243_956_020_020_9, 0.871_452_665_534_376_5, 1.009_927_306_338_269_4],
    [-1.200_204_083_082_625, -0.509_020_252_723_337_6, -0.353_320_278_944_552_7, -0.175_054_521_436_731_72, -0.115_757_368_278_181_06, -0.078_275_586_866_856_89, -0.048_260_209_251_860_35, -0.030_254_932_681_778_53, -0.000_546_551_132_533_934_9, 0.020_151_078_914_535_07, 0.051_730_193_208_939_21, 0.118_124_514_946_108_32, 0.408_009_864_140_245_2, 0.679_604_822_152_086_2, 0.768_243_956_020_020_9, 0.871_452_665_534_376_5, 1.009_927_306_338_269_4],
    [-1.200_204_083_082_625, -0.509_020_252_723_337_6, -0.353_320_278_944_552_7, -0.175_054_521_436_731_72, -0.115_757_368_278_181_06, -0.078_275_586_866_856_89, -0.048_260_209_251_860_35, -0.030_254_932_681_778_53, -0.000_546_551_132_533_934_9, 0.020_151_078_914_535_07, 0.051_730_193_208_939_21, 0.118_124_514_946_108_32, 0.408_009_864_140_245_2, 0.679_604_822_152_086_2, 0.768_243_956_020_020_9, 0.871_452_665_534_376_5, 1.009_927_306_338_269_4],
    [-1.200_204_083_082_625, -0.509_020_252_723_337_6, -0.353_320_278_944_552_7, -0.175_054_521_436_731_72, -0.115_757_368_278_181_06, -0.078_275_586_866_856_89, -0.048_260_209_251_860_35, -0.030_254_932_681_778_53, -0.000_546_551_132_533_934_9, 0.020_151_078_914_535_07, 0.051_730_193_208_939_21, 0.118_124_514_946_108_32, 0.408_009_864_140_245_2, 0.679_604_822_152_086_2, 0.768_243_956_020_020_9, 0.871_452_665_534_376_5, 1.009_927_306_338_269_4],
    [-1.200_204_083_082_625, -0.509_020_252_723_337_6, -0.353_320_278_944_552_7, -0.175_054_521_436_731_72, -0.115_757_368_278_181_06, -0.078_275_586_866_856_89, -0.048_260_209_251_860_35, -0.030_254_932_681_778_53, -0.000_546_551_132_533_934_9, 0.020_151_078_914_535_07, 0.051_730_193_208_939_21, 0.118_124_514_946_108_32, 0.408_009_864_140_245_2, 0.679_604_822_152_086_2, 0.768_243_956_020_020_9, 0.871_452_665_534_376_5, 1.009_927_306_338_269_4],
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
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.247_787_832_759_588_32, -0.183_189_394_723_961_62, -0.152_254_504_057_816_5, -0.116_949_671_919_049_63, -0.075_830_144_793_945_66, -0.066_407_853_945_976_98, -0.029_962_489_075_394_946, 0.007_555_300_538_689_674, 0.060_225_495_752_430_28, 0.094_162_771_642_797_31, 0.136_356_354_932_108_15, 0.166_987_951_542_834_36, 0.214_754_844_051_166_68, 0.296_354_046_737_708_7, 0.353_695_024_966_633_46, 0.640_182_590_281_388_9],
    [-0.376_037_984_235_564_1, -0.250_242_654_439_034_5, -0.210_609_149_181_552_32, -0.176_505_885_531_642_04, -0.147_604_730_076_721_65, -0.093_131_504_250_455_87, -0.068_833_960_510_797_56, -0.013_989_540_080_805_203, 0.019_949_074_917_386_712, 0.056_526_577_925_499_71, 0.076_751_651_081_285_83, 0.124_185_778_422_674_86, 0.165_905_234_721_615_64, 0.200_960_705_289_459_56, 0.232_714_097_483_204_4, 0.322_262_316_331_395_4, 0.640_182_590_281_388_9],
    [-0.376_037_984_235_564_1, -0.247_358_064_049_742_12, -0.210_756_662_362_772_43, -0.183_179_745_028_427_38, -0.160_743_911_406_682, -0.136_145_881_044_711_43, -0.093_257_372_373_006_06, -0.046_886_074_217_786_08, -0.000_704_856_533_501_001_6, 0.030_044_688_844_434_2, 0.056_293_593_909_305_085, 0.120_366_606_098_478_77, 0.166_021_678_985_566_38, 0.202_497_496_805_644_1, 0.242_260_963_315_786_64, 0.333_306_991_983_677_9, 0.704_927_738_997_239_3],
    [-0.376_037_984_235_564_1, -0.231_283_301_974_136_3, -0.202_957_419_360_057_3, -0.176_534_760_875_125_9, -0.150_894_752_156_535_22, -0.135_996_914_279_804_07, -0.093_142_850_791_606_03, -0.052_655_070_448_531_74, -0.017_223_735_400_189_344, 0.014_839_250_457_010_483, 0.053_228_702_337_075_11, 0.134_969_255_550_217_94, 0.191_374_314_074_685_05, 0.233_315_976_120_377_24, 0.304_922_437_957_862_67, 0.478_879_007_982_019_2, 0.704_927_738_997_239_3],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.376_037_984_235_564_1, -0.254_697_298_481_879_3, -0.203_405_435_456_970_43, -0.173_480_114_781_843_55, -0.138_214_085_420_024_83, -0.093_032_071_478_422_56, -0.067_296_537_717_713_28, -0.021_986_650_167_529_233, 0.015_730_095_918_011_11, 0.056_758_641_055_747, 0.101_029_562_755_668_05, 0.157_203_170_812_180_3, 0.199_888_708_697_027_27, 0.243_317_288_485_996_7, 0.337_203_757_388_980_84, 0.559_201_101_034_823_9, 5.051_132_513_006_893],
    [-0.377_219_589_614_803_84, -0.127_436_620_777_885_67, -0.093_002_400_858_919_6, -0.078_174_396_946_966_7, -0.060_341_919_118_998_805, -0.045_065_351_466_540_98, -0.031_278_040_114_043_87, -0.022_474_165_651_258_083, -0.010_559_829_520_106_58, 0.003_509_105_627_785_603, 0.015_992_836_192_781_74, 0.030_096_971_964_667_29, 0.045_147_164_512_387_98, 0.059_122_469_101_189_44, 0.078_356_318_693_310_61, 0.127_471_461_527_905, 0.452_805_398_203_227_1],
    [-0.377_219_589_614_803_84, -0.127_436_620_777_885_67, -0.093_002_400_858_919_6, -0.078_174_396_946_966_7, -0.060_341_919_118_998_805, -0.045_065_351_466_540_98, -0.031_278_040_114_043_87, -0.022_474_165_651_258_083, -0.010_559_829_520_106_58, 0.003_509_105_627_785_603, 0.015_992_836_192_781_74, 0.030_096_971_964_667_29, 0.045_147_164_512_387_98, 0.059_122_469_101_189_44, 0.078_356_318_693_310_61, 0.127_471_461_527_905, 0.452_805_398_203_227_1],
    [-0.377_219_589_614_803_84, -0.283_164_918_115_448_1, -0.194_517_566_614_142_2, -0.148_018_328_315_537_63, -0.120_764_182_618_980_96, -0.099_985_752_958_170_14, -0.074_684_089_325_634_61, -0.052_785_105_409_085_58, -0.043_692_215_557_508_03, -0.033_567_355_404_394_913, -0.021_052_374_504_272_883, -0.018_672_714_548_578_145, 0.005_163_551_623_349_666, 0.017_349_343_469_910_24, 0.034_324_910_764_216_905, 0.076_528_883_877_499_07, 0.088_150_962_952_828_76],
    [-0.377_219_589_614_803_84, -0.151_647_066_148_529_85, -0.124_221_181_585_486_9, -0.116_303_230_565_152_57, -0.105_733_349_300_532_55, -0.097_883_608_404_912_4, -0.082_477_706_336_136_17, -0.078_624_914_778_961_32, -0.071_606_744_813_993_34, -0.055_754_247_921_834_435, -0.048_815_874_369_475_46, -0.042_345_749_118_671_915, -0.027_883_319_952_601_57, -0.019_674_105_897_522_654, 0.010_977_252_306_331_356, 0.134_551_744_688_738_62, 0.214_347_090_829_642_2],
    [-0.214_914_729_489_091_85, -0.137_730_427_505_329_9, -0.117_214_257_013_125_66, -0.106_119_363_184_816_85, -0.096_696_285_823_744_82, -0.082_386_097_647_427_13, -0.074_705_361_852_659_64, -0.061_992_627_111_535_56, -0.049_397_141_537_676_9, -0.039_835_227_697_577_066, -0.026_515_003_762_015_44, -0.010_398_170_094_909_502, 0.019_681_613_481_749_242, 0.057_441_089_180_221_31, 0.112_136_017_118_630_7, 0.185_822_908_007_764_9, 0.452_805_398_203_227_1],
    [-0.298_149_951_804_769_2, -0.119_509_648_309_758_55, -0.105_480_967_519_073_61, -0.082_416_538_729_085_5, -0.068_624_898_111_186_86, -0.054_296_832_808_940_355, -0.039_703_212_540_256_695, -0.027_809_448_293_528_164, -0.013_016_356_724_230_389, -0.002_076_710_164_925_094_3, 0.018_142_930_458_106_582, 0.034_528_201_395_950_05, 0.057_567_829_036_907_4, 0.080_253_971_978_098_07, 0.142_014_346_021_681_73, 0.194_265_249_639_606_9, 0.452_805_398_203_227_1],
    [-0.347_367_255_777_046_2, -0.113_698_691_412_336_17, -0.078_599_909_949_676_6, -0.059_760_479_112_127_99, -0.048_819_709_742_995_21, -0.035_904_737_866_786_93, -0.026_208_464_780_940_237, -0.014_070_516_911_800_997, -0.009_812_119_208_985_936, 0.008_388_650_157_851_063, 0.030_843_772_886_475_322, 0.042_057_780_065_706_325, 0.066_186_458_828_723_19, 0.106_371_660_986_760_18, 0.157_189_968_126_947_72, 0.197_880_346_973_198_92, 0.452_805_398_203_227_1],
    [-0.347_367_255_777_046_2, -0.089_614_365_401_388_73, -0.068_374_826_160_601_12, -0.055_746_650_188_146_71, -0.042_009_535_547_360_42, -0.035_036_141_031_408_445, -0.027_150_429_564_373_846, -0.021_684_003_959_591_934, -0.012_931_943_320_674_912, -0.006_912_885_300_516_243, 0.007_324_493_746_194_573, 0.024_322_203_101_749_15, 0.035_753_401_748_495_575, 0.048_423_229_897_172_65, 0.070_352_662_822_725_69, 0.127_563_617_855_275_42, 0.452_805_398_203_227_1],
    [-0.347_367_255_777_046_2, -0.077_400_321_032_630_99, -0.055_877_533_622_540_276, -0.041_448_585_628_081_92, -0.030_443_699_362_830_243, -0.026_771_994_161_828_253, -0.019_806_699_851_748_575, -0.012_377_119_793_218_835, -0.004_899_867_268_674_676, 0.007_591_103_666_087_691, 0.022_504_609_117_767_735, 0.030_516_740_112_035_92, 0.037_322_428_558_944_52, 0.047_500_427_185_859_08, 0.057_974_422_865_920_17, 0.070_570_660_785_943_12, 0.219_290_903_866_434_74],
    [-0.347_367_255_777_046_2, -0.072_429_284_200_685_45, -0.051_526_778_338_535_65, -0.035_233_325_217_550_945, -0.027_143_143_618_624_55, -0.018_577_811_470_639_05, -0.008_271_571_709_691_549, -9.809_498_786_851_048e-5, 0.009_207_210_964_595_472, 0.017_495_698_400_092_668, 0.028_679_585_882_280_92, 0.034_749_402_370_567_346, 0.045_949_580_886_256_97, 0.052_879_355_884_641_65, 0.060_620_294_031_295_34, 0.078_245_325_530_997, 0.122_181_172_580_212_1],
    [-0.281_951_496_723_554_24, -0.078_743_964_236_181_38, -0.066_647_776_373_008_8, -0.038_287_436_769_747_85, -0.030_035_120_385_830_897, -0.012_794_632_153_038_873, -0.007_232_355_647_853_999, 0.003_508_245_680_913_7, 0.008_643_814_803_937_578, 0.017_369_014_122_518_248, 0.025_355_503_154_731_52, 0.034_094_223_977_400_59, 0.047_596_634_024_627_216, 0.057_677_191_878_845_09, 0.070_650_511_411_743_03, 0.086_346_911_153_080_03, 0.161_519_989_802_607_87],
    [-0.281_951_496_723_554_24, -0.106_847_916_406_193_64, -0.068_991_098_270_892_4, -0.039_669_857_173_303_92, -0.022_385_253_492_945_997, -0.010_161_086_040_276_192, -0.006_470_636_302_026_315, -0.001_037_416_715_676_883, 0.004_562_672_174_857_158, 0.013_468_647_458_032_288, 0.024_304_581_874_133_87, 0.043_132_683_834_347_39, 0.053_803_624_216_052_844, 0.066_310_503_619_486_65, 0.102_070_645_421_619_42, 0.112_443_161_969_982_03, 0.161_519_989_802_607_87],
    [-0.377_219_589_614_803_84, -0.127_436_620_777_885_67, -0.093_002_400_858_919_6, -0.078_174_396_946_966_7, -0.060_341_919_118_998_805, -0.045_065_351_466_540_98, -0.031_278_040_114_043_87, -0.022_474_165_651_258_083, -0.010_559_829_520_106_58, 0.003_509_105_627_785_603, 0.015_992_836_192_781_74, 0.030_096_971_964_667_29, 0.045_147_164_512_387_98, 0.059_122_469_101_189_44, 0.078_356_318_693_310_61, 0.127_471_461_527_905, 0.452_805_398_203_227_1],
    [-0.281_951_496_723_554_24, -0.081_552_223_720_834_15, -0.065_565_970_173_541, -0.041_145_310_242_143_71, -0.023_967_316_045_581_333, -0.004_577_503_261_209_778, 0.013_440_073_643_888_124, 0.023_087_376_261_777_554, 0.045_052_478_711_835_54, 0.053_118_752_537_235_726, 0.057_598_395_610_762_236, 0.069_836_318_349_182_5, 0.084_993_140_646_112_27, 0.102_709_842_925_789_97, 0.128_871_832_368_691_62, 0.195_119_550_396_104_4, 0.221_370_460_606_621_8],
    [-0.173_851_041_007_400_56, -0.087_536_048_170_887_21, -0.076_903_160_210_642_83, -0.066_132_854_706_375_05, -0.051_630_571_016_742_51, -0.035_343_403_015_289_5, -0.023_301_742_318_929_848, -0.000_438_331_026_167_373_96, 0.015_282_943_269_229_97, 0.024_154_408_602_324_314, 0.042_872_047_039_572_925, 0.051_679_656_658_165_7, 0.057_576_351_042_472_02, 0.073_398_262_495_175_38, 0.111_234_867_093_565_78, 0.130_936_756_441_380_45, 0.221_370_460_606_621_8],
    [-0.134_915_113_060_993_07, -0.100_047_679_409_356_44, -0.087_486_725_403_174_52, -0.080_164_891_125_502_76, -0.071_008_354_827_268_71, -0.059_618_465_799_203_935, -0.044_593_410_621_326_02, -0.029_227_860_746_826_47, -0.022_182_625_138_831_93, 0.003_455_525_527_086_265_8, 0.016_779_705_711_265_344, 0.024_843_984_312_539_924, 0.042_863_295_512_534_594, 0.056_488_160_629_796_454, 0.065_452_316_810_271_28, 0.110_506_620_745_693_05, 0.221_370_460_606_621_8],
    [-0.377_219_589_614_803_84, -0.127_436_620_777_885_67, -0.093_002_400_858_919_6, -0.078_174_396_946_966_7, -0.060_341_919_118_998_805, -0.045_065_351_466_540_98, -0.031_278_040_114_043_87, -0.022_474_165_651_258_083, -0.010_559_829_520_106_58, 0.003_509_105_627_785_603, 0.015_992_836_192_781_74, 0.030_096_971_964_667_29, 0.045_147_164_512_387_98, 0.059_122_469_101_189_44, 0.078_356_318_693_310_61, 0.127_471_461_527_905, 0.452_805_398_203_227_1],
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
    [-0.185_768_431_174_098_36, -0.073_670_495_971_910_78, -0.044_701_484_790_990_88, -0.028_515_113_016_192_02, -0.020_753_425_216_897_246, -0.005_049_925_283_657_331, 0.013_818_907_077_992_954, 0.023_909_249_346_221_323, 0.031_699_665_189_715_36, 0.054_068_490_331_809_22, 0.064_277_918_904_357_36, 0.077_208_768_153_251_12, 0.095_904_717_928_904_83, 0.112_642_979_542_283_03, 0.121_993_630_891_237_49, 0.144_281_105_371_373_26, 0.218_884_038_911_751_32],
    [-0.185_768_431_174_098_36, -0.099_019_234_732_477_19, -0.076_777_742_458_443_77, -0.055_491_577_263_445_7, -0.039_402_611_634_359_49, -0.027_194_408_547_987_776, -0.019_564_132_548_273_97, -0.005_338_195_369_859_007, 0.007_765_028_378_307_326, 0.020_125_584_632_895_84, 0.031_637_900_993_202, 0.055_072_618_337_628_95, 0.068_290_263_062_920_55, 0.086_046_774_294_045_7, 0.116_914_630_173_305_49, 0.133_954_902_571_980_73, 0.218_884_038_911_751_32],
    [-0.185_768_431_174_098_36, -0.110_629_993_964_842_06, -0.094_510_219_667_431_99, -0.085_950_410_310_526_82, -0.069_387_698_212_382_5, -0.053_588_688_650_773_826, -0.039_336_072_127_929_31, -0.032_890_281_442_348_145, -0.022_613_792_882_472_967, -0.008_132_417_952_170_675, 0.000_779_130_354_552_718_3, 0.019_410_507_142_955_894, 0.029_390_762_532_087_555, 0.055_343_842_233_390_39, 0.081_861_961_379_129_74, 0.121_481_946_345_719_11, 0.218_884_038_911_751_32],
    [-0.177_755_772_833_497_53, -0.111_443_930_973_687_53, -0.098_668_741_995_079_65, -0.089_455_773_835_972_27, -0.081_570_466_320_511_32, -0.068_947_104_045_919_28, -0.058_664_882_022_418_3, -0.043_471_289_016_015_33, -0.035_910_881_411_553_81, -0.033_385_956_681_571_07, -0.022_975_807_292_831_513, -0.008_555_575_595_264_885, 0.000_109_261_297_484_355_98, 0.014_777_276_676_292_982, 0.028_077_227_047_149_766, 0.069_841_192_001_089_51, 0.188_602_713_172_526_03],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.185_768_431_174_098_36, -0.105_783_259_498_793_14, -0.086_394_548_238_384_62, -0.067_088_419_949_570_56, -0.044_765_445_357_838_98, -0.034_114_578_416_629_79, -0.026_102_429_152_048_436, -0.011_608_688_469_219_13, -0.000_651_932_903_731_381_3, 0.016_292_686_118_205_214, 0.027_854_046_603_891_642, 0.045_648_017_357_338_56, 0.065_433_319_688_225_17, 0.090_606_118_275_440_77, 0.118_390_943_894_450_47, 0.150_048_769_273_900_26, 0.962_914_860_156_158_1],
    [-0.486_129_220_719_366_54, -0.310_229_391_153_095_1, -0.136_127_964_927_725_72, -0.038_045_401_734_295_256, -0.022_679_666_794_898_57, -0.013_217_839_438_041_94, -0.008_250_652_435_656_58, -0.003_851_616_865_085_881_4, 0.001_753_531_606_691_046_2, 0.005_202_050_473_556_658, 0.009_188_574_618_628_643, 0.013_164_987_152_264_22, 0.017_023_416_516_428_46, 0.023_847_964_523_316_074, 0.031_831_535_775_517_034, 0.045_757_829_083_603_65, 0.181_877_261_513_583_52],
    [-0.486_129_220_719_366_54, -0.310_229_391_153_095_1, -0.136_127_964_927_725_72, -0.038_045_401_734_295_256, -0.022_679_666_794_898_57, -0.013_217_839_438_041_94, -0.008_250_652_435_656_58, -0.003_851_616_865_085_881_4, 0.001_753_531_606_691_046_2, 0.005_202_050_473_556_658, 0.009_188_574_618_628_643, 0.013_164_987_152_264_22, 0.017_023_416_516_428_46, 0.023_847_964_523_316_074, 0.031_831_535_775_517_034, 0.045_757_829_083_603_65, 0.181_877_261_513_583_52],
    [-0.486_129_220_719_366_54, -0.310_229_391_153_095_1, -0.136_127_964_927_725_72, -0.038_045_401_734_295_256, -0.022_679_666_794_898_57, -0.013_217_839_438_041_94, -0.008_250_652_435_656_58, -0.003_851_616_865_085_881_4, 0.001_753_531_606_691_046_2, 0.005_202_050_473_556_658, 0.009_188_574_618_628_643, 0.013_164_987_152_264_22, 0.017_023_416_516_428_46, 0.023_847_964_523_316_074, 0.031_831_535_775_517_034, 0.045_757_829_083_603_65, 0.181_877_261_513_583_52],
    [-0.486_129_220_719_366_54, -0.310_229_391_153_095_1, -0.136_127_964_927_725_72, -0.038_045_401_734_295_256, -0.022_679_666_794_898_57, -0.013_217_839_438_041_94, -0.008_250_652_435_656_58, -0.003_851_616_865_085_881_4, 0.001_753_531_606_691_046_2, 0.005_202_050_473_556_658, 0.009_188_574_618_628_643, 0.013_164_987_152_264_22, 0.017_023_416_516_428_46, 0.023_847_964_523_316_074, 0.031_831_535_775_517_034, 0.045_757_829_083_603_65, 0.181_877_261_513_583_52],
    [-0.259_604_988_586_030_7, -0.148_986_914_102_832_74, -0.091_378_204_928_500_13, -0.057_488_107_371_342_11, -0.040_984_567_003_243_95, -0.025_633_516_987_667_884, -0.014_811_517_844_095_656, -0.010_931_927_974_744_54, -0.003_751_426_232_136_827_5, 0.004_710_917_439_322_385, 0.013_828_069_150_942_878, 0.023_315_324_561_794_24, 0.032_314_349_676_432_54, 0.038_888_194_472_249_58, 0.043_636_193_474_385_306, 0.049_468_216_808_221_874, 0.073_904_197_680_680_02],
    [-0.183_920_558_696_843_93, -0.133_080_558_824_448_22, -0.094_508_798_992_418_12, -0.065_701_205_549_695_2, -0.045_246_751_288_734_4, -0.012_498_139_353_311_934, -0.009_755_053_983_122_647, -0.000_518_689_889_139_111_6, 0.005_014_252_191_688_861, 0.011_983_304_832_008_566, 0.014_477_844_978_811_032, 0.024_578_553_384_205_757, 0.027_997_267_174_982_17, 0.034_747_384_979_178_29, 0.044_008_139_297_782_13, 0.053_321_404_913_555_56, 0.073_904_197_680_680_02],
    [-0.183_920_558_696_843_93, -0.121_644_544_283_388_75, -0.094_240_609_672_851_06, -0.069_623_203_570_410_99, -0.048_605_253_420_478_39, -0.011_731_989_750_052_931, -0.001_459_950_743_467_303_4, 0.003_976_862_430_051_359, 0.006_026_798_845_072_48, 0.011_645_754_845_464_137, 0.014_784_007_915_775_197, 0.024_888_109_088_940_017, 0.027_873_454_947_582_206, 0.034_031_824_828_551_986, 0.047_330_506_746_237_296, 0.054_201_428_018_691_39, 0.181_877_261_513_583_52],
    [-0.378_511_307_631_174_85, -0.126_229_941_883_989_06, -0.106_286_917_887_736_67, -0.086_402_907_294_879_46, -0.058_774_385_725_038_23, -0.033_446_852_038_075_26, -0.000_892_523_296_730_148_1, 0.004_633_918_698_707_356, 0.009_512_623_362_829_837, 0.016_432_876_207_888_84, 0.025_839_123_164_374_73, 0.031_672_346_472_930_71, 0.045_261_811_633_415_645, 0.048_823_758_963_764_514, 0.054_285_794_141_567_31, 0.064_060_526_211_475_86, 0.181_877_261_513_583_52],
    [-0.452_569_631_391_724_7, -0.339_141_419_306_143_67, -0.271_364_755_167_119_83, -0.120_780_625_454_484_41, -0.090_809_338_661_330_44, -0.055_953_906_390_421_45, -0.014_110_542_959_975_507, 0.004_200_435_170_009_015, 0.014_776_204_547_152_23, 0.022_926_666_210_699_196, 0.025_583_797_507_929_98, 0.031_556_202_434_487_3, 0.038_852_613_867_127_01, 0.050_064_948_254_378_32, 0.059_740_113_474_516_354, 0.125_683_511_289_398_2, 0.181_877_261_513_583_52],
    [-0.480_499_462_526_394_36, -0.389_659_143_827_661_26, -0.340_314_034_910_896_9, -0.291_127_884_052_736_2, -0.272_439_475_330_369_05, -0.114_523_558_797_145_08, -0.019_073_261_789_135_775, -0.000_386_691_682_932_620_17, 0.005_939_975_243_993_327, 0.015_233_007_651_782_63, 0.022_680_806_117_795_306, 0.025_271_979_677_949_13, 0.029_078_560_956_549_298, 0.033_117_515_842_822_52, 0.053_024_017_470_306_14, 0.112_705_072_230_782_97, 0.181_877_261_513_583_52],
    [-0.486_129_220_719_366_54, -0.395_214_353_643_201_23, -0.347_238_456_624_705_4, -0.296_343_182_816_282_14, -0.281_078_362_073_694_9, -0.251_984_383_259_786_9, -0.019_369_024_332_425_1, -0.003_724_394_499_878_651_3, 0.006_735_692_607_132_353, 0.014_700_040_534_413_247, 0.017_411_650_940_177_51, 0.023_872_892_243_345_694, 0.026_855_518_918_980_758, 0.032_045_620_054_880_69, 0.053_895_619_575_759_626, 0.107_523_251_848_647_76, 0.128_841_939_031_905_88],
    [-0.486_129_220_719_366_54, -0.392_322_029_288_521_1, -0.332_656_278_172_772_8, -0.292_219_730_611_030_5, -0.269_106_840_716_922_87, -0.033_259_089_323_220_44, -0.015_652_440_443_956_764, -0.004_343_583_117_750_033_5, 0.005_700_521_726_950_899, 0.011_520_880_894_822_578, 0.015_125_348_686_900_972, 0.016_862_353_099_869_76, 0.018_400_795_884_592_543, 0.026_842_962_102_412_434, 0.040_247_121_563_737_8, 0.075_560_148_455_324_15, 0.114_050_976_393_392_68],
    [-0.486_129_220_719_366_54, -0.342_167_152_017_771_4, -0.316_325_589_889_641_1, -0.284_163_892_102_234_4, -0.038_779_729_837_431_935, -0.023_475_724_941_810_664, -0.014_155_724_163_194_91, -0.009_282_629_579_374_962, -0.003_956_647_578_655_184, 0.003_501_510_749_923_061, 0.007_430_026_013_051_026, 0.012_202_064_153_466_016, 0.014_491_328_765_224_53, 0.017_340_388_166_398_41, 0.020_882_635_073_946_984, 0.059_783_858_838_660_775, 0.112_012_087_750_703_7],
    [-0.486_129_220_719_366_54, -0.321_213_464_275_198_7, -0.279_075_189_871_583_64, -0.032_197_461_531_775_79, -0.025_353_799_473_396_97, -0.017_708_930_629_344_647, -0.013_050_008_670_996_862, -0.009_894_659_857_046_52, -0.006_707_016_975_807_96, -0.003_838_747_849_613_803, 0.000_214_659_916_039_655_58, 0.004_174_431_804_487_748, 0.006_658_375_645_332_015_5, 0.012_087_745_675_579_341, 0.016_967_866_691_796_472, 0.032_200_374_137_704_005, 0.076_712_042_634_088_82],
    [-0.372_593_927_331_447_8, -0.064_830_051_876_192_29, -0.030_526_883_127_152_71, -0.023_183_200_526_960_06, -0.014_311_209_454_564_914, -0.010_547_567_015_649_13, -0.008_090_832_989_502_705, -0.006_325_244_467_679_023, -0.002_517_472_015_031_327, 0.000_324_654_259_810_709_63, 0.003_282_561_757_222_648, 0.005_827_221_399_535_164, 0.007_043_075_941_642_924, 0.011_634_654_186_062_599, 0.019_156_723_960_437_876, 0.029_168_994_327_584_855, 0.048_014_475_493_765_14],
    [-0.372_593_927_331_447_8, -0.032_696_967_523_726_67, -0.022_426_223_254_247_656, -0.013_253_548_801_903_936, -0.009_192_265_986_969_478, -0.006_776_439_930_095_541, -0.002_704_294_619_880_482_8, 0.000_366_738_149_610_192, 0.003_279_393_397_351_511, 0.005_234_153_894_328_979, 0.006_720_595_072_249, 0.010_597_071_000_972_895, 0.013_025_679_438_290_803, 0.017_070_438_372_765_09, 0.021_990_113_550_190_947, 0.031_681_655_526_939_606, 0.043_470_704_738_081_456],
    [-0.350_593_428_756_825_14, -0.023_708_315_450_591_64, -0.013_434_817_172_175_357, -0.007_546_716_785_377_664, -0.004_135_308_782_077_347, 0.000_256_296_529_432_326_96, 0.003_258_564_697_826_114_4, 0.005_327_249_727_481_948, 0.007_710_317_073_743_184, 0.011_105_101_226_900_664, 0.013_128_360_490_380_334, 0.015_629_713_145_918_208, 0.018_713_618_055_203_326, 0.021_114_799_928_580_844, 0.026_106_655_824_229_334, 0.035_467_501_336_261_385, 0.039_913_032_486_546_32],
    [-0.470_012_071_605_791_95, -0.133_637_241_446_674_88, -0.082_172_917_913_668_44, -0.063_608_075_944_134_22, -0.045_986_983_058_839_29, -0.032_275_345_712_794_37, -0.018_904_207_337_672_75, -0.009_586_799_652_626_39, 0.000_265_137_378_016_270_9, 0.008_117_252_046_851_556, 0.016_924_498_916_329_783, 0.026_603_785_394_445_214, 0.037_222_050_273_461_99, 0.049_592_663_862_333_92, 0.065_205_824_805_541_1, 0.081_883_703_021_274_66, 0.246_315_491_199_618_54],
    [-0.470_012_071_605_791_95, -0.133_637_241_446_674_88, -0.082_172_917_913_668_44, -0.063_608_075_944_134_22, -0.045_986_983_058_839_29, -0.032_275_345_712_794_37, -0.018_904_207_337_672_75, -0.009_586_799_652_626_39, 0.000_265_137_378_016_270_9, 0.008_117_252_046_851_556, 0.016_924_498_916_329_783, 0.026_603_785_394_445_214, 0.037_222_050_273_461_99, 0.049_592_663_862_333_92, 0.065_205_824_805_541_1, 0.081_883_703_021_274_66, 0.246_315_491_199_618_54],
    [-0.470_012_071_605_791_95, -0.133_637_241_446_674_88, -0.082_172_917_913_668_44, -0.063_608_075_944_134_22, -0.045_986_983_058_839_29, -0.032_275_345_712_794_37, -0.018_904_207_337_672_75, -0.009_586_799_652_626_39, 0.000_265_137_378_016_270_9, 0.008_117_252_046_851_556, 0.016_924_498_916_329_783, 0.026_603_785_394_445_214, 0.037_222_050_273_461_99, 0.049_592_663_862_333_92, 0.065_205_824_805_541_1, 0.081_883_703_021_274_66, 0.246_315_491_199_618_54],
    [-0.470_012_071_605_791_95, -0.133_637_241_446_674_88, -0.082_172_917_913_668_44, -0.063_608_075_944_134_22, -0.045_986_983_058_839_29, -0.032_275_345_712_794_37, -0.018_904_207_337_672_75, -0.009_586_799_652_626_39, 0.000_265_137_378_016_270_9, 0.008_117_252_046_851_556, 0.016_924_498_916_329_783, 0.026_603_785_394_445_214, 0.037_222_050_273_461_99, 0.049_592_663_862_333_92, 0.065_205_824_805_541_1, 0.081_883_703_021_274_66, 0.246_315_491_199_618_54],
    [-0.470_012_071_605_791_95, -0.133_637_241_446_674_88, -0.082_172_917_913_668_44, -0.063_608_075_944_134_22, -0.045_986_983_058_839_29, -0.032_275_345_712_794_37, -0.018_904_207_337_672_75, -0.009_586_799_652_626_39, 0.000_265_137_378_016_270_9, 0.008_117_252_046_851_556, 0.016_924_498_916_329_783, 0.026_603_785_394_445_214, 0.037_222_050_273_461_99, 0.049_592_663_862_333_92, 0.065_205_824_805_541_1, 0.081_883_703_021_274_66, 0.246_315_491_199_618_54],
    [-0.470_012_071_605_791_95, -0.133_637_241_446_674_88, -0.082_172_917_913_668_44, -0.063_608_075_944_134_22, -0.045_986_983_058_839_29, -0.032_275_345_712_794_37, -0.018_904_207_337_672_75, -0.009_586_799_652_626_39, 0.000_265_137_378_016_270_9, 0.008_117_252_046_851_556, 0.016_924_498_916_329_783, 0.026_603_785_394_445_214, 0.037_222_050_273_461_99, 0.049_592_663_862_333_92, 0.065_205_824_805_541_1, 0.081_883_703_021_274_66, 0.246_315_491_199_618_54],
    [-0.470_012_071_605_791_95, -0.073_801_430_041_643_79, -0.065_084_188_726_550_49, -0.060_267_566_390_159_386, -0.049_499_794_320_245_02, -0.037_675_207_171_866_97, -0.018_452_064_537_554_134, -0.011_502_394_421_612_45, 0.000_204_105_924_315_831_07, 0.008_492_079_779_994_537, 0.020_268_734_126_832_045, 0.026_382_138_658_476_023, 0.049_412_756_634_198_945, 0.060_724_590_187_802_435, 0.074_416_254_492_488_01, 0.083_586_118_865_974_89, 0.139_573_154_965_516_84],
    [-0.470_012_071_605_791_95, -0.076_885_711_110_535_17, -0.066_236_416_604_226_94, -0.053_827_782_419_123_06, -0.050_864_234_740_773_99, -0.043_090_678_207_051_76, -0.023_911_773_046_936_348, -0.011_528_457_768_095_893, -0.002_341_612_403_466_008, 0.003_614_205_720_447_459_6, 0.015_077_031_240_386_344, 0.022_818_153_919_548_147, 0.027_643_980_321_891_443, 0.065_433_900_586_052_68, 0.081_899_593_823_100_93, 0.103_704_463_440_675_99, 0.194_562_496_618_760_24],
    [-0.470_012_071_605_791_95, -0.095_744_663_573_966_52, -0.078_606_487_705_341_25, -0.063_109_613_395_289_41, -0.052_027_582_670_557_22, -0.046_756_218_582_265_68, -0.028_524_865_386_302_656, -0.014_937_654_136_498_914, -0.001_938_258_347_468_288_7, 0.003_558_003_496_922_667_6, 0.016_958_093_871_958_804, 0.026_905_871_738_222_904, 0.047_206_492_991_676_625, 0.070_469_263_862_145_25, 0.086_206_531_312_891_73, 0.106_778_544_075_634_5, 0.194_562_496_618_760_24],
    [-0.470_012_071_605_791_95, -0.260_987_887_670_151_2, -0.124_404_448_200_557_92, -0.083_858_028_903_712_41, -0.070_004_442_976_571_53, -0.052_063_661_042_322_73, -0.032_268_872_622_289_946, -0.016_247_393_294_859_847, -0.001_900_247_203_383_852_2, 0.007_950_352_650_544_532, 0.022_137_334_844_130_33, 0.034_395_553_983_235_995, 0.047_486_074_751_959_88, 0.064_675_946_754_772_71, 0.074_157_504_953_035_11, 0.102_110_474_204_661_47, 0.194_562_496_618_760_24],
    [-0.469_686_534_977_257_93, -0.300_937_894_332_338_25, -0.236_985_681_975_250_72, -0.118_454_600_378_585_1, -0.079_361_883_675_772_2, -0.060_829_273_013_626_665, -0.028_534_238_434_866_015, -0.012_175_758_828_900_214, -0.000_624_290_872_374_765_7, 0.008_632_833_545_603_678, 0.025_991_213_049_575_745, 0.034_106_924_186_684_665, 0.039_222_941_556_132_08, 0.051_941_713_460_552_645, 0.066_481_802_430_165_73, 0.081_008_562_632_506_7, 0.194_562_496_618_760_24],
    [-0.469_686_534_977_257_93, -0.300_059_290_713_343_25, -0.209_352_862_385_554_84, -0.115_188_850_513_665_55, -0.058_128_566_191_047_185, -0.026_298_943_488_928_566, -0.010_076_767_915_613_523, -0.000_632_492_085_220_064_1, 0.007_152_233_902_782_629, 0.016_158_671_477_418_313, 0.025_900_387_360_641_812, 0.033_623_993_905_862_14, 0.036_623_363_259_281_49, 0.047_626_128_957_610_86, 0.058_612_129_025_626, 0.079_011_484_467_462_56, 0.148_851_019_631_605_3],
    [-0.402_473_769_461_389_3, -0.210_683_739_254_651_7, -0.136_850_906_932_037_33, -0.092_386_651_548_770_46, -0.059_057_701_265_620_93, -0.032_002_884_699_463_36, -0.014_189_382_686_127_084, -0.000_295_563_523_275_295_7, 0.009_827_463_299_823_995, 0.015_025_875_077_935_052, 0.022_718_042_081_271_882, 0.029_563_067_923_396_54, 0.034_954_060_817_236_53, 0.051_673_360_915_631_34, 0.063_111_516_970_362_96, 0.079_147_907_750_052_51, 0.132_315_640_926_888_83],
    [-0.380_576_478_587_078_77, -0.139_744_852_319_115_04, -0.106_801_747_736_285_55, -0.084_472_299_061_606_51, -0.070_624_397_976_152_78, -0.056_456_733_728_736_5, -0.040_044_652_947_341_68, -0.026_535_833_121_774_172, -0.012_329_622_121_751_241, 6.751_174_184_362_307e-5, 0.010_969_517_756_859_36, 0.017_407_161_988_625_984, 0.027_252_264_556_137_145, 0.041_505_188_956_542_75, 0.052_036_830_010_841_06, 0.069_241_596_656_044_78, 0.156_699_371_423_060_23],
    [-0.291_546_531_187_350_9, -0.102_746_598_189_849_63, -0.082_170_116_186_753_48, -0.068_194_074_956_339_21, -0.052_993_040_067_893_47, -0.039_830_863_437_008_49, -0.027_609_471_907_171_838, -0.014_528_861_488_328_408, -0.007_694_651_242_312_566, 0.000_469_202_891_665_522_56, 0.007_835_837_000_690_102, 0.014_628_999_330_267_392, 0.025_368_177_988_250_622, 0.037_171_662_907_457_784, 0.049_455_127_287_613_89, 0.059_694_677_467_160_79, 0.156_699_371_423_060_23],
    [-0.161_294_994_379_195_28, -0.079_505_623_638_131_31, -0.054_764_996_585_275_04, -0.043_392_510_173_252_37, -0.026_631_332_811_386_188, -0.016_306_179_929_128_493, -0.010_093_448_847_444_484, -0.001_976_889_793_477_861, 0.002_899_243_517_152_633_3, 0.008_569_776_142_104_496, 0.011_784_318_198_826_369, 0.022_662_187_998_433_867, 0.030_932_809_151_720_718, 0.040_025_463_375_807_55, 0.051_238_220_077_390_86, 0.065_356_056_944_472_93, 0.194_098_713_466_114_15],
    [-0.095_840_852_546_018_66, -0.045_247_632_826_233_25, -0.026_117_170_215_799_31, -0.014_311_063_506_680_533, -0.007_071_247_426_357_166, -0.001_129_718_214_620_899_7, 0.003_040_174_140_879_285_8, 0.008_774_170_958_353_613, 0.010_966_286_737_138_417, 0.019_760_963_611_480_782, 0.027_625_623_150_227_68, 0.036_867_213_804_156_11, 0.042_973_230_219_699_12, 0.050_760_353_142_918_414, 0.065_329_063_369_178_54, 0.080_382_165_341_668_93, 0.194_098_713_466_114_15],
    [-1.537_315_209_469_474_2, -0.717_565_434_678_378_9, -0.508_859_330_493_889_4, -0.356_409_873_637_091_15, -0.239_623_381_078_315_8, -0.173_191_326_075_652_05, -0.094_210_052_879_314_36, -0.042_867_403_893_676_71, 0.001_068_889_917_049_821_4, 0.033_392_398_040_667_86, 0.063_800_223_319_321_89, 0.099_426_163_977_055_42, 0.123_561_865_764_174_34, 0.155_776_465_669_281_86, 0.180_964_302_688_689_04, 0.213_875_095_376_486_5, 0.340_666_480_571_501_4],
    [-1.537_315_209_469_474_2, -0.717_565_434_678_378_9, -0.508_859_330_493_889_4, -0.356_409_873_637_091_15, -0.239_623_381_078_315_8, -0.173_191_326_075_652_05, -0.094_210_052_879_314_36, -0.042_867_403_893_676_71, 0.001_068_889_917_049_821_4, 0.033_392_398_040_667_86, 0.063_800_223_319_321_89, 0.099_426_163_977_055_42, 0.123_561_865_764_174_34, 0.155_776_465_669_281_86, 0.180_964_302_688_689_04, 0.213_875_095_376_486_5, 0.340_666_480_571_501_4],
    [-1.537_315_209_469_474_2, -0.717_565_434_678_378_9, -0.508_859_330_493_889_4, -0.356_409_873_637_091_15, -0.239_623_381_078_315_8, -0.173_191_326_075_652_05, -0.094_210_052_879_314_36, -0.042_867_403_893_676_71, 0.001_068_889_917_049_821_4, 0.033_392_398_040_667_86, 0.063_800_223_319_321_89, 0.099_426_163_977_055_42, 0.123_561_865_764_174_34, 0.155_776_465_669_281_86, 0.180_964_302_688_689_04, 0.213_875_095_376_486_5, 0.340_666_480_571_501_4],
    [-1.537_315_209_469_474_2, -0.717_565_434_678_378_9, -0.508_859_330_493_889_4, -0.356_409_873_637_091_15, -0.239_623_381_078_315_8, -0.173_191_326_075_652_05, -0.094_210_052_879_314_36, -0.042_867_403_893_676_71, 0.001_068_889_917_049_821_4, 0.033_392_398_040_667_86, 0.063_800_223_319_321_89, 0.099_426_163_977_055_42, 0.123_561_865_764_174_34, 0.155_776_465_669_281_86, 0.180_964_302_688_689_04, 0.213_875_095_376_486_5, 0.340_666_480_571_501_4],
    [-0.984_217_723_219_933, -0.849_541_900_219_723_1, -0.719_317_081_608_463_7, -0.611_056_169_328_395, -0.425_276_052_290_739_17, -0.243_900_562_941_731_12, -0.149_229_805_740_284_8, -0.034_815_558_039_852_42, 0.001_096_510_985_587_859_3, 0.056_964_994_551_504_6, 0.098_626_954_081_186_03, 0.134_327_817_450_124_54, 0.160_543_330_842_352_62, 0.170_656_080_738_212_37, 0.180_044_496_065_190_68, 0.202_470_389_205_298_7, 0.252_784_027_973_545_4],
    [-1.063_421_824_915_902_6, -0.929_650_877_571_330_9, -0.813_712_835_381_677_7, -0.626_887_195_687_803_7, -0.411_621_970_667_603_64, -0.224_134_454_866_235_68, -0.138_622_198_138_747_02, -0.020_210_358_970_521_307, 0.045_520_963_050_547_13, 0.078_654_672_956_783_66, 0.099_063_627_190_490_76, 0.132_865_671_638_054_3, 0.152_711_407_200_023_46, 0.165_557_854_029_671_66, 0.181_957_428_644_042_66, 0.207_363_725_043_366_95, 0.313_867_588_129_584_26],
    [-1.102_274_394_107_238_8, -0.930_494_627_657_442_1, -0.813_345_338_310_477_8, -0.595_846_637_248_629_3, -0.407_382_276_473_407_97, -0.275_856_836_435_469_04, -0.211_046_631_962_491_74, -0.082_748_205_203_713_59, 0.024_912_792_172_663_81, 0.056_685_493_287_848_07, 0.088_667_854_369_632_84, 0.109_575_708_593_795_01, 0.136_900_827_847_483_14, 0.163_254_359_993_502_32, 0.203_267_239_903_236_09, 0.229_802_558_338_159_44, 0.313_867_588_129_584_26],
    [-1.102_274_394_107_238_8, -0.908_626_737_311_878_4, -0.680_484_535_497_017_5, -0.526_242_570_147_846_5, -0.401_487_190_175_815_36, -0.281_476_805_387_926_9, -0.212_415_956_947_041_7, -0.081_235_506_929_174_24, 0.004_519_377_400_394_189, 0.048_059_669_487_169_034, 0.075_729_843_775_664_14, 0.102_060_791_661_079_66, 0.131_715_475_199_700_3, 0.175_524_884_726_718_43, 0.209_769_104_439_793, 0.245_438_380_549_160_03, 0.340_666_480_571_501_4],
    [-1.102_274_394_107_238_8, -0.807_883_070_397_312_2, -0.630_966_284_234_975_7, -0.570_774_949_017_226_7, -0.452_023_331_245_921_76, -0.302_084_758_543_652, -0.160_758_255_433_503_53, -0.072_241_552_782_949_44, -0.015_676_232_207_460_797, 0.029_163_577_867_567_392, 0.075_101_381_582_093_85, 0.108_160_952_783_213_26, 0.131_706_094_552_478_42, 0.175_611_670_653_840_87, 0.199_107_635_374_576_65, 0.245_151_793_875_168_17, 0.340_666_480_571_501_4],
    [-1.537_315_209_469_474_2, -0.717_565_434_678_378_9, -0.508_859_330_493_889_4, -0.356_409_873_637_091_15, -0.239_623_381_078_315_8, -0.173_191_326_075_652_05, -0.094_210_052_879_314_36, -0.042_867_403_893_676_71, 0.001_068_889_917_049_821_4, 0.033_392_398_040_667_86, 0.063_800_223_319_321_89, 0.099_426_163_977_055_42, 0.123_561_865_764_174_34, 0.155_776_465_669_281_86, 0.180_964_302_688_689_04, 0.213_875_095_376_486_5, 0.340_666_480_571_501_4],
    [-1.537_315_209_469_474_2, -0.717_565_434_678_378_9, -0.508_859_330_493_889_4, -0.356_409_873_637_091_15, -0.239_623_381_078_315_8, -0.173_191_326_075_652_05, -0.094_210_052_879_314_36, -0.042_867_403_893_676_71, 0.001_068_889_917_049_821_4, 0.033_392_398_040_667_86, 0.063_800_223_319_321_89, 0.099_426_163_977_055_42, 0.123_561_865_764_174_34, 0.155_776_465_669_281_86, 0.180_964_302_688_689_04, 0.213_875_095_376_486_5, 0.340_666_480_571_501_4],
    [-1.135_733_085_969_429, -0.749_193_336_837_961_1, -0.560_650_060_772_606_6, -0.403_956_698_196_443_75, -0.226_896_708_449_271_8, -0.178_395_609_886_484_7, -0.102_858_297_006_272_15, -0.025_263_174_261_814_39, 0.010_239_410_263_276_794, 0.044_463_675_896_802_966, 0.060_879_522_819_974_04, 0.078_180_480_491_864_52, 0.100_583_491_518_536_31, 0.122_150_379_771_635_77, 0.164_480_423_832_298_52, 0.185_534_243_638_673_05, 0.251_936_021_015_989_55],
    [-1.537_315_209_469_474_2, -0.739_066_137_400_640_3, -0.553_341_000_382_825_7, -0.301_894_310_543_383, -0.195_850_512_173_889_24, -0.122_399_703_601_788_56, -0.076_026_219_735_144_83, -0.023_302_205_728_046_047, 0.003_221_785_581_553_627_8, 0.027_180_259_223_594_423, 0.060_804_903_511_785_82, 0.082_063_147_611_744_09, 0.110_355_062_838_084_4, 0.157_746_789_334_211_88, 0.180_949_695_257_258_7, 0.196_472_733_789_922_5, 0.290_199_207_202_435_97],
    [-1.537_315_209_469_474_2, -0.643_544_907_665_174, -0.403_133_208_224_714_4, -0.227_178_963_751_083_96, -0.157_717_748_978_787_5, -0.092_723_526_552_907_29, -0.050_853_949_914_497_29, -0.013_066_515_310_597_129, 0.009_221_157_660_835_767, 0.034_124_105_874_656_63, 0.064_898_636_425_136_55, 0.099_558_494_606_783_63, 0.127_202_503_521_801_1, 0.168_688_154_195_142_4, 0.186_642_980_158_432_14, 0.209_027_443_447_131_53, 0.290_199_207_202_435_97],
    [-1.537_315_209_469_474_2, -0.562_747_745_685_152_8, -0.344_170_563_235_106_1, -0.240_128_053_647_259_92, -0.172_568_018_039_845_12, -0.107_596_446_448_253_35, -0.067_816_512_690_944_76, -0.020_528_784_219_571_435, 0.003_211_391_546_546_099_7, 0.031_036_953_190_688_15, 0.064_729_396_631_977_62, 0.100_524_492_074_095_6, 0.137_142_271_805_699_27, 0.164_254_056_066_824_92, 0.191_883_707_760_470_78, 0.216_470_843_345_003_33, 0.322_647_735_277_223_9],
    [-1.537_315_209_469_474_2, -0.542_076_419_468_526_4, -0.385_411_975_130_877_44, -0.266_676_807_938_038_83, -0.197_369_673_040_472_4, -0.134_062_075_480_101_85, -0.083_704_533_275_121_73, -0.045_543_682_377_375_54, -0.006_057_829_117_467_916, 0.025_399_601_909_612_754, 0.054_613_288_745_458_24, 0.094_666_814_284_425_35, 0.126_430_610_338_678, 0.154_423_656_000_934_63, 0.185_702_110_084_115_75, 0.230_827_766_549_808_3, 0.322_647_735_277_223_9],
    [-1.231_173_262_468_594_5, -0.580_866_186_448_838_7, -0.424_297_943_469_151_84, -0.286_030_905_394_755_45, -0.227_929_579_885_705_53, -0.161_740_346_837_113_3, -0.116_245_576_837_939_79, -0.079_543_476_408_510_44, -0.021_499_084_892_289_723, 0.021_722_527_754_280_29, 0.043_232_509_199_493_535, 0.077_189_335_131_054_64, 0.116_295_103_570_570_64, 0.142_111_042_879_783_68, 0.178_005_134_977_332_72, 0.229_374_854_100_736_94, 0.322_647_735_277_223_9],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_27: [(f64, f64); 11] = [
    (0.0, 1.0),
    (3.004_773_398_704_022, 5.394_491_872_528_683),
    (-10.433_170_566_604_165, -7.536_514_136_198_833),
    (-12.0, -8.5),
    (-9.771_313_606_110_292, -7.531_282_176_211_772),
    (-0.271_013_079_216_346_1, 1.821_534_455_744_667_3),
    (-1.0, 0.1),
    (1.729_393_283_774_961_7, 6.126_590_067_601_561),
    (0.390_454_062_867_078_3, 1.367_347_855_504_556_2),
    (-1.376_082_599_277_379_3, 0.087_371_246_031_908_96),
    (0.218_707_404_892_395_24, 0.904_946_941_757_851_2),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_27: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.075_444_092_183_405, 0.938_161_972_846_105_5, 1.010_965_454_899_470_7, 1.065_671_980_869_937, 1.253_099_953_982_539_6, 1.253_099_953_982_539_6, 1.191_435_609_748_869_6, 1.151_756_156_816_133_2, 1.141_153_306_169_537_4, 1.120_107_877_198_277_3, 1.120_107_877_198_277_3, 1.143_252_791_520_965_8, 1.192_484_415_831_9, 1.209_765_506_713_302, 1.198_639_898_981_921, 1.215_867_875_882_919_3, 1.220_207_543_992_035_2],
    [0.335_486_258_516_837_6, 0.317_667_843_834_935_57, 0.333_002_314_562_899_4, 0.360_066_306_086_078_54, 0.911_749_230_290_118_2, 0.889_773_889_800_006_7, 0.881_338_187_487_550_9, 0.866_249_838_474_697_6, 0.847_003_956_298_164_4, 0.804_297_942_623_143_8, 0.741_870_512_853_412_2, 0.568_394_162_748_914_6, 0.521_772_088_701_866_4, 0.691_522_394_290_225_8, 0.691_522_394_290_225_8, 0.691_522_394_290_225_8, 0.691_522_394_290_225_8],
    [0.0, -0.043_75, -0.087_5, -0.131_249_999_999_999_98, -0.175, -0.218_75, -0.262_499_999_999_999_96, -0.306_249_999_999_999_97, -0.35, -0.393_75, -0.437_5, -0.481_249_999_999_999_96, -0.524_999_999_999_999_9, -0.568_75, -0.612_499_999_999_999_9, -0.656_25, -0.7],
    [2.108_443_218_857_722_5, 2.108_443_218_857_722_5, 2.123_853_333_857_856_5, 2.123_853_333_857_856_5, 2.090_499_293_009_260_3, 1.984_709_799_000_117_5, 1.980_482_555_445_835_4, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761, 2.093_908_708_823_761],
    [-1.186_658_335_976_619_4, -1.184_314_851_543_181_2, -1.040_046_775_686_769_7, -0.878_900_162_960_245_8, -0.666_918_499_296_823_8, -0.566_773_307_274_758_2, -0.505_286_583_783_794_1, -0.138_152_204_829_692_6, -0.084_689_804_780_238_31, -0.052_310_155_301_442_98, -0.029_425_512_412_182_355, -0.011_246_866_927_212_686, -0.014_028_963_311_531_015, -0.117_962_458_697_647_99, -0.271_124_589_543_316_14, -0.338_141_296_048_558_1, -0.385_315_390_564_432_1],
    [0.0, -0.137_5, -0.275, -0.412_500_000_000_000_03, -0.55, -0.687_5, -0.825_000_000_000_000_1, -0.962_500_000_000_000_1, -1.1, -1.237_5, -1.375, -1.512_500_000_000_000_2, -1.650_000_000_000_000_1, -1.787_5, -1.925_000_000_000_000_3, -2.062_5, -2.2],
    [-0.408_674_720_632_721_26, -0.466_759_177_206_795_05, -0.525_042_028_094_276_9, -0.582_475_230_846_916, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5, -0.626_137_143_685_890_5],
    [0.344_334_947_094_895_26, 0.344_334_947_094_895_26, 0.344_334_947_094_895_26, 0.369_769_328_744_103_5, 0.412_321_058_935_477_44, 0.470_126_920_162_657_3, 0.494_156_520_327_397_9, 0.512_490_521_188_48, 0.592_843_057_721_120_2, 0.658_267_233_362_702_8, 0.687_415_885_918_128_9, 0.746_806_638_153_960_5, 0.776_025_008_349_026_3, 0.817_221_657_667_462, 0.848_927_379_834_031_2, 0.863_356_019_838_611_8, 0.872_989_252_212_290_5],
    [-0.081_787_423_602_021_44, -0.123_707_855_603_700_7, -0.146_605_514_285_375_88, -0.124_959_045_807_360_07, -0.104_920_460_035_195_65, -0.087_708_481_746_845_19, -0.070_109_068_601_830_42, -0.070_109_068_601_830_42, -0.065_126_189_815_652_66, -0.053_645_777_993_428_984, -0.015_185_458_028_706_212, 0.023_839_047_609_678_088, 0.031_162_935_600_644_05, 0.004_656_285_388_353_322, 0.014_389_063_707_366_883, 0.030_340_552_215_730_014, 0.043_201_546_091_400_47],
    [9.920_085_444_149_995, 9.936_317_606_749_97, 9.808_188_535_340_232, 9.763_430_137_739_913, 9.804_763_326_225_714, 9.799_540_121_782_263, 9.720_892_050_975_975, 9.676_332_814_040_67, 9.656_442_334_804_668, 9.837_793_539_534_538, 9.893_361_966_095_068, 9.854_765_543_646_772, 9.798_724_168_241_446, 9.793_914_309_537_29, 9.775_558_756_799_837, 9.758_336_254_635_477, 9.730_353_680_663_13],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_27: [(f64, f64); 11] = [
    (2.535_617_635_715_720_7, 5.414_295_786_640_511),
    (0.364_869_764_595_566_3, 1.370_524_473_319_045_4),
    (-0.309_621_764_997_752_35, 1.888_197_883_129_475_7),
    (-1.045_757, 0.038_753),
    (1.720_296_526_975_577_4, 7.145_041_221_830_653),
    (-1.457_367_118_186_709_8, 0.149_916_150_548_279_06),
    (-2.301_03, -0.698_97),
    (-0.680_781_083_004_675_7, 0.336_777_716_470_267_6),
    (0.089_730_049_049_489_39, 0.912_367_887_644_425_7),
    (-0.536_092_810_130_140_8, 0.289_517_037_291_019),
    (8.241_614_998_553_404, 10.078_348_360_620_978),
];

/// The fast accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_DONOR_KINDS_27: [f64; 3] = [
    0.966_566_906_096_843_8,
    0.0,
    0.033_433_093_903_156_1,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16 over all their
/// members, in [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_27: [[f64; 17]; 11] = [
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [-1.020_248_343_926_629_2, -0.218_562_085_565_686_68, -0.177_952_069_415_985_63, -0.148_762_695_518_211_25, -0.083_427_824_623_513_43, -0.040_794_476_877_842_19, -0.021_877_554_677_633_8, -0.009_033_436_319_396_295, 0.006_860_422_952_459_17, 0.028_741_672_921_119_205, 0.052_418_500_916_481_925, 0.078_567_471_566_038_7, 0.109_659_349_812_498_57, 0.182_192_853_037_335_82, 0.303_063_456_634_886_54, 0.458_731_667_118_366_44, 0.807_208_216_636_010_7],
    [-0.542_061_527_132_926, -0.223_594_668_934_990_83, -0.108_464_515_258_761_45, -0.044_544_444_336_828_595, -0.024_700_579_913_401_85, -0.015_669_783_954_712_302, -0.010_009_607_492_073_592, -0.006_257_555_702_607_824, -0.001_879_701_625_544_200_3, 0.002_881_677_857_745_894_3, 0.009_762_751_110_429_613, 0.019_307_432_366_063_54, 0.028_029_367_878_610_024, 0.044_615_542_172_715_93, 0.074_401_959_625_187_95, 0.103_135_503_426_441_89, 1.990_745_028_556_484_7],
    [-0.802_680_931_121_322_3, -0.024_861_435_825_751_663, -0.020_360_816_269_047_13, -0.017_623_526_615_428_268, -0.014_780_549_038_476_474, -0.011_918_967_302_842_852, -0.008_635_333_102_176_73, -0.003_249_459_472_484_808, -7.722_735_934_558_103e-5, 0.001_433_622_671_962_355_2, 0.003_024_055_378_104_088_3, 0.004_987_769_898_339_081_5, 0.008_400_923_630_665_458, 0.012_688_091_947_320_042, 0.017_057_530_017_508_815, 0.024_952_998_404_536_384, 0.342_040_355_308_288_86],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-1.112_210_342_801_534_4, -0.113_027_958_009_852_91, -0.078_891_765_061_705_49, -0.055_312_296_833_111_43, -0.038_415_792_379_875_72, -0.024_934_218_888_151_12, -0.014_150_861_819_681_573, -0.004_614_488_978_421_392, 0.005_607_615_910_737_631, 0.020_825_774_669_923_514, 0.053_954_636_671_302_04, 0.080_811_076_783_456_48, 0.253_231_508_757_327_14, 0.499_354_230_472_868_9, 0.657_123_429_562_477_2, 0.770_702_726_682_86, 0.873_530_080_868_285],
    [-0.054_443_870_739_584_455, -0.018_413_425_058_788_093, -0.011_586_344_763_082_228, -0.007_857_447_922_886_528, -0.004_392_912_619_016_887, -0.001_220_918_589_339_392_7, 0.001_346_081_421_484_181_8, 0.003_517_924_120_458_464_3, 0.005_303_189_211_596_808, 0.006_519_346_380_092_918, 0.007_309_431_866_294_265, 0.007_905_345_782_983_624, 0.008_583_161_961_302_148, 0.009_279_700_428_357_35, 0.010_074_274_103_950_332, 0.011_073_585_356_234_235, 2.259_659_106_902_606_4],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.589_270_183_908_706, -0.100_304_532_499_896_07, -0.061_166_487_088_819_75, -0.043_686_642_158_489_164, -0.030_174_583_006_902_542, -0.019_270_981_784_713_277, -0.010_445_835_641_341_281, -0.002_415_118_812_782_201_3, 0.004_677_950_561_033_277, 0.011_189_252_095_418_833, 0.016_894_662_353_858_555, 0.022_357_708_656_237_825, 0.028_418_462_047_254_935, 0.037_904_692_263_244_9, 0.049_298_642_952_920_496, 0.063_092_165_916_294_9, 0.213_248_457_482_100_3],
    [-0.749_301_412_843_868_9, -0.457_772_228_484_503_1, -0.319_068_338_336_063_4, -0.224_958_588_957_715_8, -0.162_693_747_906_618_67, -0.112_215_743_929_559_73, -0.067_775_063_723_002_92, -0.025_398_730_784_236_514, 0.006_678_852_858_972_7, 0.035_307_229_041_880_09, 0.068_595_368_653_006_67, 0.106_057_087_517_818_02, 0.146_856_531_505_616_5, 0.201_116_078_763_545_31, 0.262_373_447_599_796_8, 0.360_757_570_160_904_4, 1.081_370_965_798_622_6],
    [-1.854_356_837_626_653_5, -0.408_436_894_133_672_3, -0.277_255_403_512_871_8, -0.187_932_151_289_755_13, -0.131_040_173_074_517_76, -0.087_727_588_267_616_34, -0.054_212_138_463_516_69, -0.024_968_721_162_731_647, 0.001_433_899_342_554_302_4, 0.024_643_651_404_417_53, 0.044_626_655_842_212_835, 0.063_784_543_200_924_93, 0.080_880_358_705_426_47, 0.096_631_805_378_136_8, 0.112_451_453_823_060_56, 0.139_191_762_295_784_64, 0.224_447_456_701_952_32],
];

/// The slow accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_SLOW_NODE_RESIDUALS_27: [[f64; 17]; 187] = [
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [5.419_694_594_756_431, 6.168_486_847_407_617, 6.538_591_370_988_098, 6.736_072_088_239_109, 6.842_387_116_903_24, 6.923_916_028_895_338, 7.010_877_099_310_455, 7.188_945_486_414_878, 7.567_595_403_806_953, 7.846_704_931_598_557, 8.036_169_403_362_438, 8.203_324_604_239_56, 8.351_540_113_096_04, 8.491_416_071_711_887, 8.681_528_528_146_444, 10.0, 10.0],
    [-0.551_967_905_036_875_5, -0.316_025_058_127_260_17, -0.118_371_894_877_895_79, -0.026_977_094_958_560_596, 0.003_112_839_030_057_163, 0.021_747_212_301_153_375, 0.033_910_764_871_861_61, 0.044_504_841_000_409_585, 0.053_416_845_658_879_28, 0.062_450_694_428_961_7, 0.070_372_948_629_340_07, 0.078_819_958_780_943_72, 0.088_654_447_115_201_21, 0.101_657_250_179_742_9, 0.124_220_288_723_266_43, 0.178_381_881_379_095_03, 0.421_400_388_879_051_6],
    [-0.551_967_905_036_875_5, -0.321_385_557_876_394_33, -0.126_124_870_762_644_56, -0.026_681_748_748_506_505, 0.002_058_541_320_072_438_7, 0.017_554_767_315_734_052, 0.030_191_649_227_323_848, 0.042_262_738_249_628_694, 0.051_307_354_321_215_2, 0.062_163_855_586_574_134, 0.070_429_218_480_706_24, 0.080_484_977_269_985_45, 0.092_473_008_925_592_6, 0.107_428_272_841_315_15, 0.140_278_290_440_516_04, 0.219_421_099_279_349_27, 0.681_807_719_953_177_8],
    [-0.551_967_905_036_875_5, -0.303_584_384_906_821_9, -0.131_234_955_191_939_7, -0.043_997_698_542_459_87, -0.002_517_688_134_399_433_6, 0.018_707_102_161_511_88, 0.034_175_770_363_271_81, 0.049_063_250_509_854_564, 0.063_300_025_848_754_23, 0.074_852_428_631_408_23, 0.089_374_948_027_786_59, 0.104_490_696_912_830_57, 0.124_102_339_635_409_44, 0.154_780_700_141_734_65, 0.192_801_787_956_537_38, 0.307_305_083_818_103_56, 0.782_151_977_586_855_3],
    [-0.551_967_905_036_875_5, -0.184_487_953_025_008, -0.087_758_502_893_822_34, -0.034_437_183_958_951_505, -0.017_402_142_669_417_614, -0.004_937_772_494_439_13, 0.010_209_207_569_075_24, 0.026_617_530_106_435_8, 0.043_756_293_783_130_555, 0.062_664_570_316_152_11, 0.080_413_456_932_805_87, 0.098_507_775_481_093_6, 0.118_755_714_448_644_34, 0.149_950_139_229_104_75, 0.187_803_463_047_005_26, 0.357_406_610_249_305_65, 0.803_994_613_875_567],
    [-0.533_916_572_143_990_5, -0.178_802_534_010_499_3, -0.154_564_489_820_822_88, -0.116_590_563_279_521_76, -0.037_837_025_564_266_54, -0.020_661_890_257_934_804, -0.013_296_173_987_426_193, -0.005_709_365_190_421_281, 0.004_398_896_055_895_677, 0.016_606_629_799_194_816, 0.031_769_322_252_785_274, 0.049_847_098_716_927_49, 0.072_929_758_779_545_48, 0.099_611_353_376_698_17, 0.136_476_951_123_087_36, 0.343_751_659_588_074_7, 0.807_208_216_636_010_7],
    [-0.312_862_880_022_963_7, -0.187_339_110_102_654_64, -0.170_333_009_722_153_73, -0.155_644_015_040_616_37, -0.141_477_464_828_579_12, -0.108_814_879_665_400_36, -0.025_430_790_024_038_946, -0.016_049_363_631_901_334, -0.009_688_496_659_602_532, -0.001_242_851_524_280_288_8, 0.008_804_073_801_921_376, 0.024_177_562_199_753_45, 0.043_258_393_845_930_4, 0.073_909_842_793_715_76, 0.124_497_497_732_570_54, 0.511_506_491_656_657_7, 0.807_208_216_636_010_7],
    [-0.290_469_843_363_280_9, -0.197_116_477_189_787_23, -0.178_763_487_023_710_75, -0.165_401_744_231_703_7, -0.151_983_631_424_679_47, -0.132_799_327_083_070_66, -0.094_680_459_937_950_82, -0.022_628_017_074_757_62, -0.011_786_461_215_669_229, -0.000_595_499_946_633_645, 0.016_787_853_314_515_985, 0.045_799_906_698_297_095, 0.120_546_981_508_399_93, 0.360_957_993_257_596_87, 0.507_375_879_779_906_2, 0.619_448_465_244_639_6, 0.807_208_216_636_010_7],
    [-0.290_469_843_363_280_9, -0.227_756_262_626_177_74, -0.194_632_546_266_433_53, -0.161_900_127_562_129_8, -0.094_324_090_804_067_95, -0.070_103_686_861_425_97, -0.044_760_261_062_418_43, -0.014_369_741_320_630_132, 0.028_670_291_279_998_63, 0.103_743_229_131_185_12, 0.255_012_121_947_837_46, 0.344_076_374_177_575_6, 0.415_868_392_161_102_6, 0.465_221_106_894_412_15, 0.528_672_710_896_192_5, 0.598_333_358_224_146_3, 0.807_208_216_636_010_7],
    [-0.290_469_843_363_280_9, -0.225_702_460_787_910_18, -0.198_124_435_291_336_4, -0.139_317_837_786_124_9, -0.074_651_572_701_440_7, -0.057_487_053_360_800_19, -0.040_803_574_957_929_536, -0.020_357_212_181_811_885, 0.017_419_022_688_150_782, 0.066_395_818_618_786_31, 0.156_939_722_953_718_25, 0.272_145_814_659_178_1, 0.348_914_635_875_044_6, 0.408_584_114_965_797_8, 0.456_243_263_482_909_4, 0.520_165_805_020_679_1, 0.741_468_057_042_220_2],
    [-0.290_469_843_363_280_9, -0.207_972_700_825_153_54, -0.180_746_452_339_188_73, -0.136_317_293_900_063_25, -0.061_453_430_869_376_23, -0.047_583_307_359_613_96, -0.035_303_199_040_161_1, -0.020_010_496_370_079_84, 0.001_523_825_429_588_033_3, 0.034_403_041_793_707_03, 0.070_428_952_465_813_07, 0.111_363_833_109_022_97, 0.229_416_903_676_558_8, 0.312_773_706_261_480_1, 0.386_136_612_068_264, 0.451_437_655_784_764_64, 0.731_826_227_898_121_5],
    [-0.277_076_516_201_363_75, -0.190_350_534_951_656_3, -0.163_430_424_612_777_01, -0.124_865_446_858_884_48, -0.048_856_424_288_054_8, -0.037_503_038_925_718_955, -0.028_049_726_423_808_104, -0.016_327_560_075_162_152, -0.002_583_851_418_786_383_5, 0.018_517_042_400_525_03, 0.043_888_442_928_417_05, 0.069_434_135_284_168_49, 0.099_454_101_816_888_92, 0.134_932_192_599_694_08, 0.265_907_721_354_767_2, 0.366_261_154_335_816_1, 0.731_826_227_898_121_5],
    [-0.257_845_336_095_254_8, -0.175_968_560_492_705_73, -0.148_837_739_302_466_77, -0.060_673_182_177_989_546, -0.040_496_330_332_540_89, -0.032_083_871_178_039_07, -0.023_329_435_683_550_673, -0.014_067_595_506_581_715, -0.002_988_061_161_873_061_6, 0.012_220_840_244_519_708, 0.031_775_358_637_100_494, 0.050_304_754_321_682_42, 0.072_154_261_464_060_07, 0.095_862_421_846_828_68, 0.125_597_116_173_897_9, 0.233_671_500_501_337_73, 0.731_826_227_898_121_5],
    [-0.229_900_309_072_353_6, -0.161_072_527_088_166_66, -0.134_991_000_473_550_56, -0.045_541_464_949_118_995, -0.035_363_562_611_627_06, -0.028_258_080_655_908_57, -0.020_096_939_707_056_26, -0.011_218_667_859_107_169, -0.000_205_188_123_527_590_01, 0.011_268_070_483_587_831, 0.025_636_154_480_657_04, 0.041_255_207_669_371_78, 0.059_830_522_650_144_64, 0.080_138_738_433_786_02, 0.103_508_472_047_089_05, 0.132_813_532_416_757_36, 0.731_826_227_898_121_5],
    [-0.844_869_744_498_786_3, -0.147_462_821_404_760_05, -0.124_143_513_361_175_85, -0.038_831_967_936_265_18, -0.030_185_414_422_425_77, -0.023_093_680_945_989_226, -0.014_614_843_755_908_215, -0.003_674_778_980_487_323_3, 0.003_084_803_172_134_861, 0.013_210_618_985_586_132, 0.024_693_168_311_266_46, 0.040_139_390_364_419_16, 0.056_817_256_624_924_8, 0.073_846_455_093_395_02, 0.094_928_998_126_749_3, 0.125_880_619_539_659_6, 0.604_276_430_182_037_1],
    [-1.020_248_343_926_629_2, -0.218_562_085_565_686_68, -0.177_952_069_415_985_63, -0.148_762_695_518_211_25, -0.083_427_824_623_513_43, -0.040_794_476_877_842_19, -0.021_877_554_677_633_8, -0.009_033_436_319_396_295, 0.006_860_422_952_459_17, 0.028_741_672_921_119_205, 0.052_418_500_916_481_925, 0.078_567_471_566_038_7, 0.109_659_349_812_498_57, 0.182_192_853_037_335_82, 0.303_063_456_634_886_54, 0.458_731_667_118_366_44, 0.807_208_216_636_010_7],
    [-1.020_248_343_926_629_2, -0.717_707_976_091_018, -0.377_466_660_288_552_1, -0.320_895_135_021_080_64, -0.284_506_255_896_964_5, -0.235_462_686_979_519_86, -0.161_809_096_098_628_98, -0.069_974_926_706_218_62, -0.001_403_279_616_196_054_8, 0.049_950_270_428_158_465, 0.088_420_403_939_866_61, 0.153_631_612_997_390_75, 0.204_568_699_785_020_74, 0.208_461_878_766_242_86, 0.242_257_287_716_276_5, 0.386_173_327_536_873_4, 0.789_130_397_165_968_7],
    [-1.020_248_343_926_629_2, -0.708_269_687_136_758_6, -0.375_766_161_153_078_7, -0.319_857_436_775_660_75, -0.283_582_131_511_180_34, -0.235_219_730_917_981_62, -0.161_563_829_896_432_36, -0.067_394_921_661_419_5, -3.242_225_118_733_37e-5, 0.051_300_486_542_167_94, 0.089_596_218_375_810_39, 0.155_711_616_831_559_65, 0.204_709_403_049_506_33, 0.208_482_767_474_903_44, 0.242_783_102_866_148_76, 0.386_956_047_205_672_17, 0.789_130_397_165_968_7],
    [-0.173_242_500_216_681_28, -0.009_718_284_884_161_842, -0.004_062_932_737_528_759, -0.001_615_369_614_311_281_7, 0.000_792_416_024_999_770_9, 0.002_560_867_912_889_885_3, 0.005_006_055_822_513_043_5, 0.007_708_820_172_756_738, 0.010_859_660_927_575_85, 0.018_956_045_163_679_19, 0.024_040_986_859_717_707, 0.027_735_666_797_071_36, 0.030_516_105_602_354_31, 0.033_139_346_265_438_35, 0.036_639_366_606_644_56, 0.042_483_874_090_596_85, 0.736_673_978_346_308_5],
    [-0.173_242_500_216_681_28, -0.015_660_044_984_260_32, -0.008_154_759_152_870_275, -0.004_953_902_338_542_099, -0.002_813_034_715_493_531_5, -0.000_815_684_073_517_087_1, 0.001_337_568_046_570_176_7, 0.003_523_541_095_002_714_7, 0.006_052_809_235_201_544, 0.009_397_593_806_260_134, 0.015_985_589_359_040_618, 0.021_183_514_966_450_552, 0.025_022_418_365_331_648, 0.028_767_260_415_527_383, 0.031_732_463_550_150_934, 0.036_285_364_779_065_99, 1.265_709_782_257_220_5],
    [-0.173_242_500_216_681_28, -0.020_334_072_622_581_745, -0.012_609_500_740_595_674, -0.008_664_786_720_031_622, -0.006_201_541_404_618_215, -0.004_368_287_183_802_691, -0.002_461_548_832_073_372, -0.000_583_954_329_345_735_2, 0.001_709_248_544_434_227_8, 0.004_520_381_346_052_593, 0.007_594_330_873_324_583_5, 0.013_762_787_652_577_364, 0.018_907_208_231_201_744, 0.022_588_848_681_554_57, 0.027_087_927_943_445_53, 0.031_924_640_715_456_07, 1.265_709_782_257_220_5],
    [-0.173_242_500_216_681_28, -0.023_404_695_671_998_946, -0.017_554_571_145_590_583, -0.012_007_721_483_226_16, -0.009_315_588_578_027_409, -0.007_544_229_208_468_416_5, -0.005_820_952_361_600_62, -0.004_180_507_499_574_305_5, -0.002_107_920_508_164_294_6, 0.000_153_238_818_929_794_68, 0.002_792_642_439_492_213, 0.006_392_912_336_833_559, 0.012_610_390_229_468_75, 0.018_215_189_663_418_26, 0.022_299_299_991_102_24, 0.027_403_360_819_823_405, 1.265_709_782_257_220_5],
    [-0.128_329_177_345_499_48, -0.025_933_558_149_059_57, -0.021_356_094_235_035_41, -0.015_730_354_595_980_8, -0.011_834_595_510_405_296, -0.009_812_955_469_593_048, -0.008_443_748_344_304_629, -0.006_764_031_041_347_971, -0.005_108_290_151_906_246, -0.002_666_645_459_135_084, -0.000_241_386_532_012_655_9, 0.002_732_773_705_326_899, 0.007_026_716_300_033_31, 0.014_139_220_721_192_076, 0.019_444_936_420_647_787, 0.024_995_488_823_748_444, 1.350_838_674_495_015_8],
    [-0.128_329_177_345_499_48, -0.027_590_059_428_512_028, -0.023_374_435_228_790_864, -0.019_121_177_632_483_08, -0.013_704_654_451_191_978, -0.011_545_794_801_349_079, -0.009_948_479_742_239_01, -0.008_584_069_807_471_917, -0.006_725_804_322_956_443, -0.004_625_090_819_006_776, -0.001_255_229_598_349_605_6, 0.001_649_193_840_403_113_4, 0.005_064_563_744_781_818, 0.011_663_944_111_919_226, 0.018_451_048_660_245_32, 0.024_011_975_872_794_127, 1.509_749_508_669_463_2],
    [-0.128_329_177_345_499_48, -0.028_260_704_982_667_38, -0.024_327_611_230_564_223, -0.020_500_419_425_487_33, -0.015_393_932_413_768_241, -0.012_547_541_904_407_886, -0.010_975_828_694_621_576, -0.009_428_289_202_164_859, -0.007_684_663_933_250_773, -0.005_118_772_549_263_635, -0.001_260_887_196_490_775, 0.001_816_878_499_445_360_8, 0.005_238_583_200_525_343, 0.012_238_287_842_239_378, 0.018_693_898_981_515_14, 0.024_766_295_283_070_8, 1.509_749_508_669_463_2],
    [-0.047_901_909_651_339_78, -0.028_920_663_318_383_347, -0.024_553_004_278_210_284, -0.020_936_454_118_540_313, -0.016_001_051_387_963_608, -0.013_272_068_335_257_086, -0.011_286_507_002_544_301, -0.009_516_866_327_054_78, -0.007_659_377_194_444_805, -0.004_835_874_461_995_419, -0.000_897_942_746_575_747, 0.002_696_994_866_590_528_4, 0.007_259_221_023_465_439, 0.014_078_427_882_229_702, 0.019_664_429_996_165_335, 0.026_746_919_082_057_12, 1.509_749_508_669_463_2],
    [-0.066_523_914_955_651_2, -0.030_168_950_602_811_7, -0.025_450_625_120_914_065, -0.021_519_007_718_185_382, -0.016_992_946_885_174_66, -0.014_129_074_523_768_324, -0.011_868_369_802_778_911, -0.009_589_669_927_631_909, -0.007_628_671_992_453_293, -0.004_888_975_170_527_306, -0.001_332_073_549_817_209_1, 0.003_441_109_714_827_412, 0.007_637_465_116_720_503_5, 0.014_233_114_788_800_074, 0.019_673_134_296_380_103, 0.032_040_492_244_466_96, 1.509_749_508_669_463_2],
    [-0.211_650_753_217_691_97, -0.047_269_447_992_288_46, -0.034_228_724_649_398_08, -0.027_587_080_876_829_765, -0.023_131_615_194_716_82, -0.018_517_918_300_041_056, -0.015_435_115_217_657_811, -0.012_755_890_120_390_545, -0.010_330_625_070_684_35, -0.007_325_634_436_465_469, -0.004_127_143_515_523_61, 0.000_646_503_908_702_174_8, 0.005_925_488_583_485_725_5, 0.013_168_774_516_545_096, 0.019_469_730_083_781_302, 0.156_679_140_597_428_68, 1.039_235_158_321_366_6],
    [-0.319_684_023_704_244_54, -0.173_563_364_884_229_02, -0.119_768_495_322_012_07, -0.089_102_715_490_393_1, -0.065_036_674_921_260_51, -0.044_060_428_242_091_325, -0.033_387_958_115_925_2, -0.025_344_138_268_161_88, -0.018_356_419_723_067_21, -0.013_353_772_380_856_164, -0.008_002_178_540_160_58, -0.000_743_070_425_799_774_9, 0.009_101_396_605_416_15, 0.029_829_537_667_038_348, 0.102_789_225_313_692_96, 0.248_042_384_728_124_65, 1.039_235_158_321_366_6],
    [-0.375_340_618_542_303_9, -0.228_388_286_352_070_37, -0.183_401_574_789_147_95, -0.143_882_190_480_022_96, -0.109_627_767_273_742_95, -0.085_525_302_575_808_88, -0.063_727_375_274_821_77, -0.043_068_905_952_627_044, -0.025_854_440_658_561_094, -0.010_237_974_765_250_932, 0.006_029_514_314_153_664, 0.025_331_293_469_230_1, 0.044_550_711_796_187_886, 0.062_932_061_557_296_44, 0.084_322_111_421_763_06, 0.173_709_890_659_140_47, 0.895_557_387_800_694_4],
    [-0.476_380_177_607_661_44, -0.263_877_127_498_794_44, -0.196_399_842_126_307_26, -0.146_422_706_088_432_4, -0.109_178_358_759_804_75, -0.077_401_139_243_189_55, -0.052_896_600_468_094_38, -0.029_099_990_911_670_73, -0.008_097_184_830_454_112, 0.010_099_620_864_156_148, 0.028_293_382_786_002_547, 0.043_500_172_877_405_52, 0.058_099_790_591_328_69, 0.071_418_166_685_947_25, 0.086_811_985_628_964_96, 0.109_072_460_340_455_15, 1.162_432_147_950_362],
    [-0.542_061_527_132_926, -0.315_784_735_382_065_9, -0.223_340_448_309_758_8, -0.152_181_323_292_515_82, -0.106_846_394_135_014_29, -0.066_494_488_055_431_76, -0.035_657_427_998_560_46, -0.010_214_942_650_757_24, 0.012_133_969_170_435_594, 0.032_262_016_942_267_49, 0.047_195_331_695_267_53, 0.061_292_511_850_895_12, 0.072_231_499_406_967_46, 0.084_429_156_170_314_43, 0.096_286_554_404_562_14, 0.111_902_628_048_635_2, 1.162_432_147_950_362],
    [-0.542_061_527_132_926, -0.397_986_669_351_035_2, -0.310_917_744_981_747, -0.195_719_014_030_290_07, -0.129_294_503_586_337_65, -0.064_045_692_296_156_75, -0.022_152_921_463_343_852, 0.011_466_710_046_638_821, 0.035_123_168_828_196_15, 0.051_101_909_872_240_496, 0.064_887_667_566_200_39, 0.075_527_992_330_754_98, 0.084_865_610_977_734_17, 0.092_804_653_908_145_28, 0.102_439_590_674_024, 0.117_158_778_333_214_34, 1.186_067_560_668_010_2],
    [-0.542_061_527_132_926, -0.449_147_908_215_307_03, -0.401_417_438_838_762_96, -0.346_472_750_909_557_6, -0.233_783_344_378_436_18, -0.152_576_359_738_455_82, -0.061_517_513_715_362_37, 0.000_493_109_123_453_469_5, 0.035_553_781_947_888_61, 0.056_785_882_657_418_255, 0.070_794_583_400_811_61, 0.082_113_903_932_986_15, 0.088_918_210_070_914_62, 0.096_331_647_949_214_24, 0.106_730_169_818_319_68, 0.130_829_922_933_352_9, 1.276_876_578_892_430_1],
    [-0.542_061_527_132_926, -0.464_712_741_938_212_5, -0.411_250_883_832_093_57, -0.355_089_865_076_859_8, -0.290_187_523_577_502_6, -0.219_658_807_944_503_43, -0.137_612_728_590_697_45, -0.089_855_740_419_628_59, -0.074_930_575_773_204_3, -0.062_220_954_235_549_44, -0.031_143_567_190_900_653, 0.060_056_959_930_164_34, 0.084_660_770_731_682_43, 0.100_318_321_641_114_2, 0.410_587_678_087_940_67, 0.713_731_593_241_243_8, 1.276_876_578_892_430_1],
    [-0.131_360_627_978_668_65, -0.010_631_246_173_403_469, -0.004_571_893_397_991_238, -0.000_717_044_199_662_864_2, 0.002_855_435_395_336_056, 0.007_133_591_673_281_293_5, 0.010_697_068_667_507_989, 0.013_302_078_957_941_416, 0.015_328_318_580_950_549, 0.017_444_402_862_780_62, 0.019_550_338_778_399_193, 0.022_573_380_215_838_8, 0.026_000_182_232_054_116, 0.029_641_920_549_344_74, 0.032_962_242_496_541_214, 0.038_410_258_133_852_29, 0.059_766_809_547_088_63],
    [-0.131_360_627_978_668_65, -0.017_512_936_564_424_205, -0.012_798_347_269_754_888, -0.009_142_603_034_641_92, -0.005_608_192_134_837_792, -0.001_955_421_707_890_931, 0.001_089_206_941_778_47, 0.003_853_776_116_423_982, 0.006_874_494_241_233_812_5, 0.009_700_818_932_876_278, 0.012_464_448_265_985_351, 0.014_564_417_597_476_655, 0.016_730_567_206_188_377, 0.019_186_359_025_700_383, 0.023_196_114_125_632_803, 0.030_070_322_912_720_642, 0.059_766_809_547_088_63],
    [-0.149_598_198_222_642_45, -0.022_259_074_263_020_36, -0.018_363_441_483_272_173, -0.015_700_907_566_516_074, -0.012_944_573_009_864_102, -0.010_382_817_334_428_942, -0.007_732_180_029_608_394, -0.003_773_062_377_935_523_4, -0.000_218_156_411_707_648_4, 0.001_878_746_931_260_111_3, 0.003_927_076_026_478_519, 0.006_084_733_243_435_187, 0.008_927_606_972_223_646, 0.011_953_263_374_336_434, 0.015_091_514_026_307_562, 0.019_249_284_777_050_48, 0.049_099_863_394_249_155],
    [-0.149_598_198_222_642_45, -0.025_858_758_066_163_286, -0.021_959_156_483_025_19, -0.019_759_879_809_132_276, -0.017_869_195_739_478_042, -0.015_843_637_807_365_74, -0.013_364_657_051_81, -0.011_298_943_951_610_173, -0.008_515_281_501_629_307, -0.002_999_821_734_200_321_4, -0.000_176_430_533_166_792_4, 0.000_999_074_377_554_955_4, 0.002_354_243_018_911_047_4, 0.003_859_116_576_697_671, 0.006_515_541_386_892_387, 0.010_937_069_346_452_537, 0.030_658_999_261_034_603],
    [-0.162_313_430_049_853_69, -0.027_606_271_793_536_592, -0.023_224_163_843_739_466, -0.021_373_207_439_866_3, -0.019_885_951_257_347_875, -0.018_293_682_194_921_796, -0.016_215_407_502_252_158, -0.013_612_314_695_645_536, -0.011_652_852_734_841_722, -0.007_053_759_789_671_128, -0.000_801_150_808_121_586_7, 0.000_146_664_974_126_848_62, 0.000_943_265_481_911_840_1, 0.001_748_015_574_243_065_5, 0.002_908_142_171_316_483, 0.005_624_919_353_991_62, 0.022_464_792_873_566_647],
    [-0.191_342_081_320_500_37, -0.027_697_710_396_932_14, -0.023_297_912_103_834_924, -0.021_318_049_140_921_017, -0.019_800_197_317_471_34, -0.018_052_251_242_275, -0.016_086_296_305_293_99, -0.013_650_360_355_277_552, -0.011_645_042_992_095_56, -0.006_285_679_304_208_818, -0.000_508_208_307_224_089_2, 0.000_565_113_765_136_943_3, 0.001_315_877_082_332_063, 0.002_195_870_324_373_336_3, 0.003_181_440_222_118_801, 0.005_227_274_796_657_13, 0.068_758_364_953_288_92],
    [-0.191_342_081_320_500_37, -0.026_725_696_849_825_384, -0.022_707_455_949_721_286, -0.020_656_637_098_573_027, -0.018_671_121_450_019_28, -0.017_338_119_026_893_885, -0.015_389_245_328_324_587, -0.012_687_545_782_054_338, -0.010_889_543_946_928_721, -0.002_095_319_532_570_993_4, 0.000_566_783_164_648_786_7, 0.001_314_544_321_921_259_6, 0.002_031_926_623_904_899, 0.003_058_363_953_019_032, 0.004_227_226_010_178_148, 0.006_082_284_046_502_353, 0.068_758_364_953_288_92],
    [-0.191_342_081_320_500_37, -0.025_516_353_775_936_675, -0.021_703_561_431_949_642, -0.019_435_934_237_615_346, -0.017_789_705_539_091_31, -0.016_297_383_717_903_093, -0.013_965_140_693_581_69, -0.011_652_297_005_779_794, -0.004_147_511_137_744_063, 0.000_275_179_123_731_987_67, 0.001_210_867_486_923_055, 0.001_851_797_274_215_533, 0.002_808_813_185_630_991_8, 0.003_821_259_784_016_968, 0.004_686_899_175_907_355, 0.007_710_865_703_937_275_6, 0.068_758_364_953_288_92],
    [-0.191_342_081_320_500_37, -0.024_031_799_147_614_767, -0.021_120_942_703_226_95, -0.018_232_511_359_249_148, -0.016_664_528_181_371_93, -0.014_756_780_954_624_807, -0.012_374_285_185_330_633, -0.009_652_454_401_681_054, -0.001_624_302_878_054_446_2, 0.000_736_716_274_912_432_7, 0.001_777_163_128_574_894_7, 0.002_686_079_116_371_305, 0.003_708_253_173_268_447_7, 0.004_403_243_471_032_729, 0.005_447_579_824_341_756_5, 0.007_912_543_286_069_725, 0.068_758_364_953_288_92],
    [-0.071_746_105_048_881_92, -0.025_239_745_150_304_003, -0.021_129_442_647_771_07, -0.018_437_912_987_195_246, -0.016_463_785_491_638_645, -0.014_971_094_456_258_717, -0.012_966_475_797_451_5, -0.010_131_858_851_304_3, -0.005_217_764_314_391_317, -0.000_260_843_162_159_184_6, 0.001_694_007_849_348_300_7, 0.002_693_192_823_778_037, 0.003_910_909_078_706_411, 0.004_642_625_299_587_254, 0.005_847_691_688_588_491, 0.007_920_698_502_926_567, 0.018_663_015_274_756_535],
    [-0.263_164_644_935_030_06, -0.026_918_775_120_949_888, -0.022_083_641_244_290_228, -0.019_508_860_177_218_72, -0.017_669_999_447_215_416, -0.016_053_283_385_745_062, -0.014_514_954_265_620_461, -0.011_815_542_718_308_538, -0.008_235_207_336_356_057, -0.001_860_149_250_983_027, -0.000_214_285_730_052_003_13, 0.001_773_798_105_005_550_3, 0.003_184_645_603_540_105, 0.004_486_993_318_002_415, 0.005_939_556_234_722_594, 0.008_134_940_830_921_054, 0.018_663_015_274_756_535],
    [-0.263_164_644_935_030_06, -0.026_556_808_544_100_613, -0.021_359_136_790_306_727, -0.018_848_601_318_747_13, -0.016_740_646_746_369_717, -0.014_967_895_537_679_63, -0.014_194_887_775_752_254, -0.010_885_160_166_643_324, -0.007_714_900_082_511_67, -0.002_557_441_886_514_677, -4.372_826_297_614_322e-5, 0.002_199_969_714_869_107, 0.004_210_925_639_688_474, 0.005_199_603_783_989_235, 0.006_707_373_928_643_749_6, 0.008_474_658_553_299_739, 0.017_239_536_404_376_36],
    [-0.263_164_644_935_030_06, -0.024_728_458_113_919_79, -0.018_911_977_650_534_758, -0.017_193_023_831_779_12, -0.015_468_495_889_959_301, -0.014_200_040_775_825_963, -0.011_666_313_464_696_718, -0.009_642_326_528_393_365, -0.007_534_864_908_644_261, -0.000_201_844_875_493_654_56, 0.002_866_183_621_846_460_6, 0.004_037_384_153_679_714, 0.004_912_935_705_549_434, 0.006_059_850_750_396_251, 0.007_407_154_880_138_734_5, 0.008_686_853_750_509_359, 0.017_239_536_404_376_36],
    [-0.263_164_644_935_030_06, -0.024_678_141_223_554_842, -0.020_065_741_195_733_77, -0.017_699_440_535_945_78, -0.016_213_672_348_280_028, -0.014_216_979_283_722_66, -0.011_957_805_133_107_308, -0.009_583_762_150_695_3, -0.003_012_146_136_161_744_5, -0.000_447_495_273_380_428_66, 0.002_067_671_744_651_995_5, 0.003_589_532_929_271_399_3, 0.004_286_691_640_935_028, 0.005_631_577_104_157_261, 0.006_483_620_905_080_21, 0.007_746_904_189_652_883, 0.017_239_536_404_376_36],
    [-0.074_170_749_900_377_57, -0.027_547_092_137_335_624, -0.020_889_673_020_087_374, -0.020_048_132_656_019_28, -0.017_517_777_534_877_57, -0.015_502_920_751_177_962, -0.013_947_813_863_751_475, -0.006_286_810_013_361_345, -0.003_579_427_295_152_987, -0.002_366_521_134_137_444, -0.001_069_750_010_249_860_4, 0.001_087_870_060_196_469_2, 0.003_136_883_959_182_564, 0.004_336_112_010_578_335, 0.005_746_989_318_790_236_5, 0.006_822_116_251_750_069, 0.183_857_355_473_760_73],
    [-0.052_303_781_509_954_045, -0.031_271_650_353_479_1, -0.029_155_386_089_974_665, -0.021_661_952_295_482_654, -0.020_049_097_302_446_712, -0.017_415_867_720_120_61, -0.014_229_916_162_574_5, -0.006_598_154_288_217_761, -0.005_093_637_062_199_702, -0.003_074_086_795_014_249, -0.002_340_817_776_119_454_5, -0.001_547_799_906_403_006, 0.000_772_352_979_204_354_3, 0.002_387_073_284_616_024_3, 0.004_631_701_534_471_139_4, 0.006_204_369_324_342_036, 0.183_857_355_473_760_73],
    [-0.802_680_931_121_322_3, -0.024_861_435_825_751_663, -0.020_360_816_269_047_13, -0.017_623_526_615_428_268, -0.014_780_549_038_476_474, -0.011_918_967_302_842_852, -0.008_635_333_102_176_73, -0.003_249_459_472_484_808, -7.722_735_934_558_103e-5, 0.001_433_622_671_962_355_2, 0.003_024_055_378_104_088_3, 0.004_987_769_898_339_081_5, 0.008_400_923_630_665_458, 0.012_688_091_947_320_042, 0.017_057_530_017_508_815, 0.024_952_998_404_536_384, 0.342_040_355_308_288_86],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.251_551_141_692_031_2, -0.190_710_379_918_616_47, -0.155_424_255_472_705_3, -0.118_696_879_740_254_92, -0.095_286_793_416_210_34, -0.056_609_724_283_677_804, -0.018_971_386_151_326_593, 0.010_293_117_294_575_22, 0.064_353_581_565_396_03, 0.102_241_399_996_834_33, 0.151_136_934_580_700_27, 0.189_792_320_985_297_6, 0.239_898_118_329_242_82, 0.277_816_231_403_006_04, 0.366_931_695_180_958_2, 0.657_620_356_686_202_9],
    [-0.402_674_010_420_801_3, -0.252_371_724_365_990_7, -0.219_024_119_441_240_95, -0.173_663_503_002_265_93, -0.143_231_685_379_441_84, -0.102_360_977_432_451_87, -0.064_271_441_892_118_95, -0.041_933_453_905_311_38, 0.002_448_524_469_929_633, 0.034_296_728_144_816_96, 0.081_616_969_808_342_79, 0.119_457_290_352_989_83, 0.180_378_305_089_518_46, 0.207_392_700_167_158_58, 0.256_042_650_348_801_2, 0.366_966_422_073_642_8, 0.657_620_356_686_202_9],
    [-0.365_124_942_203_826_5, -0.271_814_370_773_179_8, -0.236_395_141_160_842_2, -0.205_053_355_577_146_05, -0.163_007_395_287_824_26, -0.133_982_453_607_380_84, -0.102_417_047_440_854_23, -0.061_060_132_122_538_56, -0.015_376_536_199_320_18, 0.022_674_967_373_873_194, 0.064_288_789_436_448, 0.102_555_689_143_095_42, 0.178_751_599_348_714_15, 0.217_556_546_622_273_83, 0.262_380_727_083_964_76, 0.411_133_442_276_808_1, 5.054_541_928_821_393_5],
    [-0.365_124_942_203_826_5, -0.304_837_128_073_606_13, -0.242_475_699_113_016_28, -0.223_657_939_166_710_4, -0.182_744_858_436_278_94, -0.143_172_100_112_923_58, -0.115_912_235_401_111_13, -0.100_812_870_641_565_3, -0.000_606_345_513_540_992_6, 0.026_531_013_703_800_704, 0.102_149_836_940_219_2, 0.160_743_055_241_022_9, 0.221_306_979_964_717_82, 0.288_896_128_413_186_9, 0.520_903_920_516_261_7, 2.587_725_289_827_530_7, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.402_674_010_420_801_3, -0.263_028_270_691_484_2, -0.220_984_658_494_098_04, -0.174_859_538_394_063_1, -0.143_141_743_376_704_5, -0.101_779_900_044_291_16, -0.061_153_964_804_308_99, -0.017_843_274_064_859_388, 0.010_675_216_857_443_645, 0.064_095_159_120_232_6, 0.102_559_453_770_784_46, 0.172_117_270_411_107_3, 0.204_020_991_330_139_07, 0.256_326_420_982_246_9, 0.340_238_429_347_171_97, 0.577_247_081_401_240_4, 5.054_541_928_821_393_5],
    [-0.103_603_203_038_347_36, -0.053_806_483_857_144_156, -0.041_464_956_814_634_86, -0.030_653_571_668_302_44, -0.023_530_616_521_530_82, -0.016_637_641_242_253_733, -0.009_946_205_687_447_564, 0.001_040_757_957_635_719, 0.012_626_310_837_483_654, 0.036_783_197_702_979_31, 0.058_812_794_590_020_624, 0.076_975_096_249_518_79, 0.102_167_337_113_050_41, 0.136_928_901_808_695_27, 0.178_519_710_513_177_25, 0.221_004_596_853_263, 0.347_855_278_835_883_1],
    [-0.103_603_203_038_347_36, -0.050_194_977_888_593_58, -0.033_811_609_436_850_3, -0.023_848_732_815_182_017, -0.016_497_713_169_802_4, -0.009_840_470_531_595_097, -0.002_921_466_835_093_045, 0.003_892_009_639_256_755, 0.010_129_014_753_971_16, 0.018_887_289_751_012_12, 0.049_895_763_451_571_48, 0.129_826_040_888_901_93, 0.185_645_681_326_173_37, 0.234_141_513_441_449_76, 0.291_782_939_991_778_23, 0.352_447_076_422_838_4, 0.463_595_237_608_356_97],
    [-0.103_603_203_038_347_36, -0.035_441_553_296_818_39, -0.021_033_609_724_285_567, -0.013_091_866_640_495_407, -0.006_847_667_129_729_412, -0.001_893_394_074_501_940_7, 0.002_925_335_098_623_73, 0.006_449_500_129_855_405, 0.010_814_929_337_143_172, 0.015_533_631_332_966_107, 0.023_722_979_570_193_127, 0.059_211_815_265_178_41, 0.282_344_512_872_325_1, 0.347_888_697_208_901_4, 0.400_334_519_752_826, 0.463_588_710_738_339_3, 0.657_374_311_609_898_1],
    [-0.102_155_661_524_550_47, -0.031_568_306_242_223_995, -0.020_952_320_897_309_706, -0.013_407_374_678_198_025, -0.008_440_390_376_507_637, -0.004_219_159_857_715_706, -5.602_171_784_452_656e-5, 0.003_701_800_379_687_361_7, 0.007_477_450_865_427_605_5, 0.012_076_608_563_080_607, 0.017_680_307_755_079_214, 0.031_584_448_370_221_37, 0.375_434_458_887_869_64, 0.462_651_713_991_872_7, 0.539_074_745_766_148_7, 0.605_202_461_749_912_9, 0.790_804_604_887_722_9],
    [-0.161_352_971_699_500_58, -0.062_960_777_932_977_43, -0.045_332_426_668_668_976, -0.034_133_892_093_683_886, -0.025_632_397_681_650_626, -0.019_372_219_451_126_6, -0.013_044_041_493_630_283, -0.008_066_840_156_697_353, -0.002_962_168_258_779_254_4, 0.002_370_355_254_511_343, 0.008_508_321_084_028_308, 0.018_459_004_581_848_34, 0.460_054_182_536_371_3, 0.575_250_082_120_253_5, 0.647_826_847_199_159_3, 0.722_801_576_344_988_4, 0.873_530_080_868_285],
    [-1.112_210_342_801_534_4, -0.108_677_998_305_991_17, -0.083_817_683_621_174_93, -0.068_743_901_011_074_72, -0.055_193_988_368_694_67, -0.044_807_565_077_367_17, -0.035_232_003_918_565_33, -0.026_971_435_849_941_09, -0.019_910_162_887_693_938, -0.011_560_333_737_029_94, -0.002_932_657_223_299_799_6, 0.011_667_130_614_846_553, 0.569_324_765_568_397_3, 0.680_282_170_933_536_2, 0.745_659_824_924_272_2, 0.792_845_574_578_223_2, 0.873_530_080_868_285],
    [-1.112_210_342_801_534_4, -0.282_006_550_296_609_26, -0.119_778_909_954_386_95, -0.097_575_079_883_463_76, -0.083_040_452_184_873_41, -0.070_552_146_776_682_21, -0.056_137_085_572_088_22, -0.042_401_271_598_220_66, -0.026_699_714_256_576_097, -0.008_741_657_653_975_625, 0.063_813_701_583_259_2, 0.273_200_397_677_551_67, 0.565_838_324_467_682_3, 0.717_473_057_053_866, 0.779_017_632_627_789_3, 0.815_804_043_565_907, 0.873_530_080_868_285],
    [-1.112_210_342_801_534_4, -0.376_749_014_132_691_96, -0.140_240_969_883_524_72, -0.096_886_177_747_787_57, -0.075_653_670_507_307_9, -0.036_183_305_241_773_27, 0.044_291_079_647_449_015, 0.054_720_003_528_524_25, 0.065_621_854_318_310_11, 0.076_395_325_862_551_28, 0.088_276_385_370_197_26, 0.148_048_950_916_921_22, 0.295_605_428_424_318_7, 0.452_289_902_867_681_4, 0.660_809_892_931_858_5, 0.801_303_656_295_102_5, 0.873_530_080_868_285],
    [-1.112_210_342_801_534_4, -0.051_901_703_069_112_654, 0.032_876_067_081_427_034, 0.043_933_463_275_859_07, 0.048_309_331_983_862_64, 0.053_740_328_796_482_48, 0.059_048_067_125_997_694, 0.064_003_239_731_446_34, 0.069_010_848_801_842_43, 0.073_548_493_751_826_82, 0.078_525_831_439_062_18, 0.083_467_568_860_374_94, 0.089_831_692_780_575_66, 0.101_543_672_962_088_54, 0.182_883_762_653_951_1, 0.365_773_637_307_468_6, 0.862_518_645_319_953_8],
    [-0.261_034_208_503_044_7, -0.009_819_325_472_960_192, 0.025_761_224_211_109_768, 0.041_148_500_793_147_25, 0.046_099_707_979_016_11, 0.051_396_621_067_257_384, 0.056_834_856_242_383_25, 0.061_443_567_530_562_83, 0.065_608_790_597_087_46, 0.070_117_412_505_117_95, 0.074_066_408_051_969_51, 0.078_738_891_215_590_94, 0.082_679_753_144_479_06, 0.087_419_249_819_821_4, 0.094_361_583_034_253_02, 0.103_851_900_017_873_82, 0.239_943_247_973_236_68],
    [-0.261_034_208_503_044_7, -0.097_299_972_063_496_85, -0.040_795_600_530_314_25, -0.015_444_615_515_340_257, 0.003_950_064_710_032_841, 0.013_461_050_921_280_951, 0.027_466_197_230_088_22, 0.041_002_440_501_939_54, 0.054_035_333_420_460_095, 0.066_109_968_089_883_72, 0.074_039_497_723_134_33, 0.082_674_773_082_199_52, 0.089_484_706_942_256_3, 0.095_776_824_806_258_96, 0.103_723_053_165_900_15, 0.111_584_549_341_156_57, 0.261_036_617_552_412_2],
    [-0.371_706_890_714_201_9, -0.116_709_026_135_555_17, -0.099_762_039_230_977_68, -0.074_194_293_779_495_13, -0.050_186_805_652_684_616, -0.037_186_963_300_837_92, -0.026_483_746_889_932_17, -0.019_756_875_866_960_226, -0.006_793_636_857_411_692_5, -0.000_928_717_797_092_597_6, 0.007_110_882_731_055_121, 0.013_305_454_418_377_143, 0.024_636_198_282_771_853, 0.030_974_081_772_847_224, 0.043_444_864_280_224_475, 0.200_674_521_647_087_54, 0.261_036_617_552_412_2],
    [-1.112_210_342_801_534_4, -0.113_027_958_009_852_91, -0.078_891_765_061_705_49, -0.055_312_296_833_111_43, -0.038_415_792_379_875_72, -0.024_934_218_888_151_12, -0.014_150_861_819_681_573, -0.004_614_488_978_421_392, 0.005_607_615_910_737_631, 0.020_825_774_669_923_514, 0.053_954_636_671_302_04, 0.080_811_076_783_456_48, 0.253_231_508_757_327_14, 0.499_354_230_472_868_9, 0.657_123_429_562_477_2, 0.770_702_726_682_86, 0.873_530_080_868_285],
    [-1.112_210_342_801_534_4, -0.113_027_958_009_852_91, -0.078_891_765_061_705_49, -0.055_312_296_833_111_43, -0.038_415_792_379_875_72, -0.024_934_218_888_151_12, -0.014_150_861_819_681_573, -0.004_614_488_978_421_392, 0.005_607_615_910_737_631, 0.020_825_774_669_923_514, 0.053_954_636_671_302_04, 0.080_811_076_783_456_48, 0.253_231_508_757_327_14, 0.499_354_230_472_868_9, 0.657_123_429_562_477_2, 0.770_702_726_682_86, 0.873_530_080_868_285],
    [-0.484_937_202_551_798_7, -0.430_349_635_603_060_83, -0.306_693_946_477_050_1, -0.055_838_527_152_060_63, -0.036_756_901_824_833_585, -0.029_747_969_397_544_48, -0.021_624_818_801_998, -0.013_928_377_600_024_354, -0.005_172_436_147_692_807, -0.000_518_498_542_027_316_2, 0.010_078_883_091_194_311, 0.018_986_022_020_211_924, 0.025_811_953_201_323_155, 0.028_664_512_591_287_578, 0.032_686_369_352_254_115, 0.042_117_803_358_387_72, 0.210_944_616_090_067_84],
    [-0.484_937_202_551_798_7, -0.077_264_508_548_511_1, -0.054_464_535_548_462_03, -0.034_336_090_566_832_005, -0.027_139_036_069_046_774, -0.016_753_784_229_660_958, -0.002_508_032_424_521_778_7, 0.002_641_068_279_900_699, 0.009_942_612_390_288_465, 0.017_215_773_955_881_707, 0.023_716_749_266_337_895, 0.026_162_815_806_801_912, 0.028_795_952_734_228_51, 0.031_875_877_777_244_696, 0.033_451_930_443_799_426, 0.038_497_067_215_739_03, 0.067_039_837_340_232],
    [-0.484_937_202_551_798_7, -0.070_500_870_819_809_98, -0.053_696_897_561_517_68, -0.033_694_177_931_074_515, -0.022_611_379_910_735_252, -0.004_287_163_238_439_737, 0.003_489_727_529_387_614, 0.007_050_278_860_880_419, 0.014_325_315_575_839_84, 0.021_773_654_776_826_627, 0.024_035_235_396_684_642, 0.025_261_506_459_100_032, 0.030_367_695_316_468_896, 0.032_795_419_831_911_35, 0.034_645_316_666_143_87, 0.043_199_514_383_335_844, 0.067_039_837_340_232],
    [0.000_619_447_502_229_642, 0.004_477_482_852_453_985_5, 0.005_357_393_769_516_645, 0.005_942_976_818_477_302, 0.006_869_033_897_905_243, 0.007_330_493_883_061_276_4, 0.008_049_940_145_812_593, 0.008_967_475_745_523_376, 0.009_434_580_120_885_026, 0.010_747_531_235_974_937, 0.013_051_761_419_459_584, 0.019_842_908_141_561_373, 0.025_672_538_262_575_75, 0.032_308_466_915_374_76, 0.048_854_577_948_018_4, 1.586_441_808_819_178, 1.651_054_557_826_031_2],
    [0.000_619_447_502_229_642, 0.004_675_154_695_259_165, 0.005_491_032_851_745_14, 0.005_996_157_532_804_807_4, 0.006_645_251_030_449_291, 0.007_457_418_245_295_479, 0.007_929_829_920_594_79, 0.008_344_245_246_074_584, 0.008_789_781_728_129_416, 0.009_100_857_306_710_612, 0.009_641_412_285_035_138, 0.010_077_860_031_732_928, 0.010_825_737_936_265_584, 0.011_486_320_340_725_163, 0.014_450_651_437_012_846, 1.149_838_923_116_541, 1.651_054_557_826_031_2],
    [-0.001_091_474_096_666_100_7, 0.004_437_843_557_955_998, 0.005_524_832_239_632_986, 0.005_979_640_977_226_068, 0.006_473_927_891_152_987, 0.007_429_184_815_822_102, 0.007_833_565_591_690_806, 0.008_302_640_265_226_952, 0.008_713_612_906_664_057, 0.008_974_322_575_882_763, 0.009_589_194_207_291_894, 0.009_928_536_252_048_698, 0.010_331_573_748_770_452, 0.010_898_617_222_896_92, 0.011_636_814_758_514_602, 0.013_039_508_770_304_579, 1.651_054_557_826_031_2],
    [-0.001_091_474_096_666_100_7, 0.001_383_531_516_111_785_6, 0.002_673_834_232_249_876_4, 0.003_242_967_742_067_72, 0.004_277_613_929_705_713, 0.005_138_223_103_188_728, 0.005_816_666_073_833_074, 0.006_359_555_511_465_518, 0.007_152_869_433_071_749_5, 0.007_695_703_651_018_178, 0.008_299_987_496_229_037, 0.008_998_720_762_698_987, 0.009_911_618_587_947_256, 0.010_284_513_120_636_989, 0.011_276_121_200_730_179, 0.012_468_638_942_485_689, 1.848_323_127_284_081_9],
    [-0.001_091_474_096_666_100_7, 0.000_808_531_931_885_820_8, 0.001_781_011_103_393_179, 0.002_674_921_624_453_176, 0.003_131_707_631_592_013, 0.003_998_593_028_551_311, 0.004_367_708_389_799_872, 0.005_080_242_951_888_06, 0.005_711_116_616_426_06, 0.006_119_467_446_940_941, 0.006_850_043_576_362_682_5, 0.007_488_626_235_501_244, 0.008_319_794_307_156_172, 0.009_149_735_465_419_471, 0.010_186_357_619_022_63, 0.011_387_045_126_215_209, 1.848_323_127_284_081_9],
    [-0.001_091_474_096_666_100_7, 0.002_197_300_014_341_509_4, 0.003_205_336_001_892_569, 0.004_375_664_991_917_212, 0.005_249_629_078_620_348_5, 0.005_812_083_366_199_728_5, 0.006_427_798_276_254_403, 0.006_877_046_048_051_248, 0.007_318_035_198_027_564_5, 0.007_786_748_346_566_417_5, 0.008_215_252_758_759_289, 0.008_518_057_371_267_313, 0.009_085_858_452_282_877, 0.009_638_813_457_299_271, 0.010_320_936_586_107_575, 0.011_279_340_644_358_666, 1.848_323_127_284_081_9],
    [-0.000_855_997_546_402_864_6, 0.004_234_734_836_392_89, 0.004_868_116_374_108_943, 0.005_500_826_848_061_112, 0.006_080_307_511_183_189, 0.006_463_715_984_947_474, 0.006_883_717_950_880_62, 0.007_290_574_907_319_177_4, 0.007_551_449_500_960_926, 0.007_956_934_709_618_802, 0.008_217_615_082_975_863, 0.008_507_583_421_515_662, 0.008_872_066_575_817_667, 0.009_546_424_863_174_512, 0.010_309_038_488_276_03, 0.011_252_700_689_511_59, 1.848_323_127_284_081_9],
    [0.002_189_804_418_237_795, 0.004_161_178_732_229_892, 0.004_815_148_447_351_195, 0.005_337_946_488_091_555_6, 0.005_907_231_833_472_713, 0.006_338_543_738_115_658, 0.006_665_629_740_388_224, 0.006_963_398_820_675_502, 0.007_282_196_942_621_675, 0.007_708_345_829_335_975, 0.007_981_068_204_884_346, 0.008_258_497_635_714_634, 0.008_660_661_611_300_486, 0.009_258_494_902_890_324, 0.009_909_541_321_777_762, 0.010_485_374_782_335_207, 1.826_984_377_792_202_4],
    [0.002_189_804_418_237_795, 0.004_885_995_994_237_128, 0.005_721_854_085_047_826, 0.006_300_781_636_036_532, 0.006_669_745_249_302_644, 0.006_904_851_267_215_271, 0.007_006_393_832_343_756, 0.007_434_542_457_404_695, 0.007_836_098_673_287_11, 0.008_010_173_667_566_537, 0.008_347_025_113_319_384, 0.008_909_298_550_311_338, 0.009_378_112_403_170_892, 0.009_817_576_233_391_585, 0.010_226_104_140_742_747, 0.010_633_270_302_374_006, 2.259_659_106_902_606_4],
    [0.002_741_374_277_628_106, 0.006_063_170_518_267_519, 0.006_507_067_189_502_145, 0.006_770_654_463_509_621, 0.006_950_559_492_841_404, 0.007_324_135_722_214_749, 0.007_600_836_040_611_11, 0.007_846_868_027_442_27, 0.008_018_196_772_468_872, 0.008_424_885_951_722_767, 0.008_970_482_194_560_222, 0.009_406_512_752_827_537, 0.009_760_011_979_273_559, 0.010_012_768_478_507_367, 0.010_367_024_362_318_391, 0.010_815_294_231_625_548, 2.259_659_106_902_606_4],
    [0.004_590_033_850_425_446, 0.006_375_807_664_794_864, 0.006_689_754_704_638_739, 0.007_082_895_812_005_336, 0.007_449_425_898_250_987, 0.007_673_218_946_298_06, 0.007_879_908_619_203_395, 0.008_155_532_561_308_016, 0.008_470_199_441_575_213, 0.008_884_513_622_156_984, 0.009_279_505_898_245_275, 0.009_664_210_265_403_195, 0.009_980_996_511_771_291, 0.010_327_065_027_033_044, 0.010_713_402_863_261_915, 0.011_223_342_560_066_263, 2.259_659_106_902_606_4],
    [0.004_590_033_850_425_446, 0.006_756_327_659_666_663, 0.007_364_677_903_166_425, 0.007_679_220_943_283_501, 0.007_930_710_165_026_701, 0.008_249_793_599_642_553, 0.008_565_409_859_652_729, 0.008_844_662_133_504_388, 0.009_115_683_174_979_653, 0.009_483_279_733_917_995, 0.009_829_233_522_065_883, 0.010_187_384_353_147_283, 0.010_578_376_436_665_532, 0.010_894_364_789_909_497, 0.011_287_818_340_052_875, 0.011_836_670_896_399_38, 2.259_659_106_902_606_4],
    [-0.003_051_698_106_173_006_4, 0.003_593_501_016_983_644, 0.005_333_978_969_120_34, 0.006_452_027_167_665_402, 0.007_121_882_586_562_391, 0.007_609_008_308_632_92, 0.007_965_897_628_000_733, 0.008_371_666_555_696_426, 0.008_705_204_283_126_123, 0.009_024_451_790_684_526, 0.009_388_917_217_118_51, 0.009_692_266_769_470_4, 0.010_191_803_983_326_798, 0.010_735_118_204_506_925, 0.011_226_488_420_449_312, 0.011_805_996_825_536_734, 0.118_114_012_872_766_02],
    [-0.014_151_151_470_029_433, -0.004_994_286_639_264_205, -0.002_048_676_391_579_052, -0.000_157_579_563_013_608_12, 0.001_422_961_866_783_961, 0.002_681_006_719_753_893_7, 0.003_767_791_513_820_028_6, 0.004_755_542_159_413_597_5, 0.005_738_538_102_918_543, 0.006_477_543_506_853_294, 0.007_173_852_336_681_194, 0.007_767_659_570_031_082, 0.008_472_454_634_466_747, 0.009_068_468_033_762_996, 0.009_681_539_689_299_135, 0.010_790_206_782_705_786, 0.118_114_012_872_766_02],
    [-0.031_649_785_022_389_576, -0.013_905_337_936_609_584, -0.010_783_812_396_046_01, -0.008_347_408_482_621_71, -0.006_331_172_218_818_595_5, -0.004_647_625_681_114_219_6, -0.003_053_395_161_898_348, -0.001_807_711_947_358_650_3, -0.000_694_297_696_869_176_7, 0.000_376_479_365_594_323_3, 0.001_610_084_044_238_615_7, 0.002_681_448_873_105_96, 0.003_872_853_575_036_952, 0.005_148_791_866_688_645, 0.006_601_062_423_958_468, 0.008_137_413_042_877_413, 0.042_325_585_076_061_32],
    [-0.054_443_870_739_584_455, -0.024_641_067_261_267_066, -0.019_326_891_614_533_145, -0.015_677_830_767_624_19, -0.013_739_630_385_239_196, -0.012_141_719_343_899_744, -0.010_967_738_139_112_386, -0.009_714_691_937_027_845, -0.008_362_507_584_185_793, -0.006_955_440_343_075_958, -0.005_545_424_246_247_614_5, -0.004_082_436_148_863_214, -0.002_446_245_300_925_185, -0.000_834_573_041_518_004, 0.000_876_547_880_047_143_2, 0.002_968_297_826_677_656_4, 0.010_932_521_632_989_19],
    [-0.054_443_870_739_584_455, -0.033_034_695_917_091_17, -0.028_145_591_108_055_18, -0.026_266_761_927_995_333, -0.024_325_539_695_009_36, -0.021_826_236_005_743_89, -0.019_499_858_227_095_976, -0.017_106_397_013_150_72, -0.015_161_727_037_915_419, -0.013_738_064_286_135_776, -0.012_341_555_944_634_085, -0.011_217_482_005_775_197, -0.009_990_283_827_598_303, -0.008_668_999_765_047_342, -0.006_996_665_726_907_556, -0.005_003_641_858_478_122, 0.001_107_588_630_237_632_5],
    [-0.278_601_014_240_892_9, -0.082_377_696_024_618_09, -0.059_693_032_379_265_53, -0.033_530_200_108_115_09, -0.013_249_990_072_524_83, -0.007_866_945_043_535_814, 0.010_771_586_153_322_887, 0.030_896_161_679_774_59, 0.040_725_425_316_985_98, 0.054_088_815_089_179_33, 0.072_607_387_536_579_23, 0.086_744_807_859_950_96, 0.102_604_004_035_095_04, 0.130_133_030_336_807_95, 0.134_134_221_133_606_18, 0.154_955_744_781_050_73, 0.240_125_920_997_913_83],
    [-0.278_601_014_240_892_9, -0.099_444_699_353_236_44, -0.081_159_319_013_769_8, -0.063_735_650_941_299_43, -0.050_907_665_386_820_67, -0.031_282_080_992_494_59, -0.014_402_894_171_877_42, -0.008_216_578_990_883_835, 0.007_134_501_674_222_561, 0.022_368_657_113_274_303, 0.038_709_334_101_354_04, 0.054_385_379_254_339_526, 0.073_436_776_884_714_45, 0.091_945_126_614_271_86, 0.130_312_632_481_390_66, 0.142_792_938_267_747_3, 0.240_125_920_997_913_83],
    [-0.278_601_014_240_892_9, -0.124_014_196_277_033_72, -0.096_958_172_468_603_64, -0.081_124_443_522_659_4, -0.071_202_482_111_152_46, -0.058_471_744_147_641_336, -0.047_022_157_492_084_69, -0.030_398_410_409_888_228, -0.018_922_460_863_222_617, -0.010_624_993_265_885_766, 0.004_461_398_956_561_191_5, 0.016_527_213_763_281_035, 0.035_836_454_739_708_68, 0.057_909_584_856_957_18, 0.083_877_019_384_763_57, 0.130_690_678_723_216_16, 0.240_125_920_997_913_83],
    [-0.278_601_014_240_892_9, -0.126_231_805_027_428_34, -0.098_125_238_690_654_34, -0.087_135_669_626_445_62, -0.072_354_706_637_357_9, -0.066_100_299_376_758, -0.054_748_473_858_281_36, -0.050_082_368_504_711_39, -0.041_883_336_958_849_78, -0.024_727_885_332_430_186, -0.017_396_727_796_989_624, -0.010_207_701_271_260_881, 0.005_914_725_969_951_617, 0.025_264_450_634_906_715, 0.039_895_932_856_439_58, 0.066_534_819_707_883_55, 0.201_684_480_884_082_8],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.278_601_014_240_892_9, -0.099_556_548_348_831_03, -0.083_138_097_052_997_38, -0.069_319_567_967_758_5, -0.053_699_757_545_777_33, -0.036_592_948_465_028_606, -0.019_970_869_848_671_437, -0.010_946_762_731_904_178, 0.003_323_266_205_248_724_8, 0.013_435_079_353_582_067, 0.034_601_554_537_983_484, 0.051_296_336_613_317_55, 0.074_778_901_965_413_91, 0.102_834_775_821_648_85, 0.133_544_215_098_991_11, 0.184_276_216_686_896_84, 0.968_224_652_875_115_9],
    [-0.192_334_865_206_847_92, -0.105_812_865_595_952_71, -0.086_387_989_564_133_49, -0.073_735_417_017_678_38, -0.061_813_688_259_842_23, -0.053_229_709_152_290_826, -0.045_394_405_809_864_25, -0.038_067_212_897_744_6, -0.030_198_309_390_253_4, -0.023_651_388_236_484_487, -0.016_088_958_266_062_74, -0.007_474_104_710_504_977, 0.001_056_058_910_391_000_3, 0.012_002_112_564_770_52, 0.023_615_290_868_005_036, 0.037_985_109_569_722_8, 0.084_540_501_320_536_92],
    [-0.269_538_492_166_832_3, -0.088_180_045_703_795_84, -0.069_573_036_185_934_27, -0.058_108_279_746_075_07, -0.048_827_662_891_949_22, -0.041_116_011_820_341_02, -0.034_412_379_059_646_274, -0.027_066_218_632_346_307, -0.021_164_376_132_473_704, -0.014_685_374_227_937_18, -0.007_925_486_974_694_708, -0.001_040_836_384_858_021_2, 0.007_074_371_042_037_558, 0.015_943_614_895_082_517, 0.026_136_159_311_398_36, 0.039_553_330_322_740_17, 0.138_929_833_226_795_75],
    [-0.269_538_492_166_832_3, -0.072_638_801_620_280_16, -0.057_617_385_071_620_77, -0.047_335_191_229_596_736, -0.038_518_505_633_144_94, -0.031_141_606_466_874_593, -0.024_188_388_082_345_354, -0.017_880_419_605_071_956, -0.011_269_851_065_184_135, -0.005_104_103_568_668_083, 0.001_622_163_710_211_852_5, 0.009_222_410_483_605_914, 0.016_739_398_068_759_397, 0.024_775_010_836_687_913, 0.034_852_231_797_490_53, 0.047_643_651_820_346_81, 0.184_666_992_624_873_63],
    [-0.347_235_750_090_346_13, -0.062_788_664_322_731_43, -0.046_588_947_118_130_89, -0.034_307_213_086_304_46, -0.024_099_446_596_401_522, -0.015_674_756_628_825_337, -0.007_651_855_450_915_298, -0.000_449_427_763_034_762_9, 0.007_423_046_208_896_139, 0.014_591_369_619_982_37, 0.021_271_550_396_611_376, 0.028_147_689_509_901_777, 0.035_573_563_605_713_14, 0.043_096_165_729_340_46, 0.051_125_120_342_573_83, 0.059_977_782_800_575_294, 0.195_549_304_437_103_4],
    [-0.347_235_750_090_346_13, -0.047_031_639_004_226_744, -0.026_004_010_332_981_4, -0.012_109_558_650_855_941, -0.002_123_072_852_706_664, 0.007_596_223_010_978_354, 0.015_315_322_600_832_201, 0.021_900_205_236_389_705, 0.028_162_667_162_401_24, 0.033_723_029_759_207_35, 0.038_875_401_007_783_65, 0.044_364_425_172_223_484, 0.049_780_938_134_596_95, 0.054_637_716_003_069_46, 0.060_334_554_645_731_68, 0.065_868_592_046_028_7, 0.195_549_304_437_103_4],
    [-0.416_989_077_365_675_6, -0.028_417_606_467_156_23, -0.011_122_966_766_577_586, -0.000_624_735_389_672_889_4, 0.007_825_429_689_846_163, 0.013_830_973_675_406_37, 0.019_867_593_380_761_484, 0.026_753_863_020_867_446, 0.032_653_889_814_477_42, 0.037_303_318_067_045_97, 0.042_171_870_068_004_3, 0.046_654_486_176_606_974, 0.052_061_311_331_474, 0.056_918_597_034_153_09, 0.061_843_227_105_138_33, 0.066_712_717_364_215_1, 0.195_549_304_437_103_4],
    [-0.538_192_935_609_709_5, -0.049_389_930_465_112_84, -0.024_315_419_195_439_65, -0.015_244_112_884_907_663, -0.009_222_146_566_296_914, -0.004_289_427_228_148_434, 7.002_243_813_189_036e-5, 0.003_522_676_070_839_441, 0.007_263_664_676_960_817, 0.010_217_481_878_513_205, 0.013_116_734_078_232_12, 0.015_619_394_183_004_136, 0.018_867_843_822_488_586, 0.023_503_444_015_932_046, 0.036_156_603_372_429_576, 0.051_657_701_469_456_42, 0.195_549_304_437_103_4],
    [-0.574_661_492_905_762_3, -0.235_021_218_201_215_8, -0.032_650_016_134_162_82, -0.018_003_759_713_622_888, -0.010_979_026_707_448_594, -0.005_468_725_621_512_861, -0.001_346_539_564_921_972_8, 0.002_246_942_280_685_206_5, 0.005_873_997_700_859_105, 0.008_395_572_114_613_562, 0.011_217_554_078_608_685, 0.013_728_378_420_860_672, 0.016_087_564_827_175_79, 0.018_543_619_051_897_04, 0.021_071_859_711_836_276, 0.024_435_192_517_937_75, 0.149_856_051_580_514_4],
    [-0.574_661_492_905_762_3, -0.287_988_700_451_717_1, -0.217_874_662_594_885_28, -0.026_626_722_896_040_546, -0.011_778_988_864_872_438, -0.003_692_870_423_039_600_3, 0.000_935_664_531_704_943_3, 0.005_541_006_391_999_19, 0.008_959_172_781_075_348, 0.012_163_418_656_089_11, 0.015_222_615_738_751_331, 0.017_714_095_129_769_913, 0.020_139_609_124_189_096, 0.022_615_675_525_418_52, 0.025_115_689_234_059_258, 0.075_598_753_229_590_18, 0.132_060_261_896_487_12],
    [-0.574_661_492_905_762_3, -0.307_619_360_958_781_84, -0.230_846_329_925_856_74, -0.038_983_366_476_899_046, -0.011_904_454_993_306_102, -0.001_733_806_987_216_578_7, 0.004_184_293_132_074_579, 0.008_638_630_356_107_239, 0.012_211_258_258_492_135, 0.015_579_119_678_502_937, 0.018_325_038_121_510_425, 0.020_958_935_436_525_713, 0.023_574_016_466_159_194, 0.025_994_095_400_052_174, 0.030_548_671_593_126_922, 0.079_077_187_287_715_61, 0.132_060_261_896_487_12],
    [-0.589_270_183_908_706, -0.259_100_133_461_805_27, -0.174_034_472_361_221_85, -0.033_686_744_633_628_536, -0.008_046_435_143_257_619, 0.001_356_464_358_720_058_8, 0.007_395_340_342_944_473, 0.011_416_437_120_412_62, 0.015_219_411_403_652_577, 0.018_260_990_145_874_383, 0.021_418_679_565_173_772, 0.024_233_888_583_656_823, 0.026_508_332_367_232_525, 0.029_347_443_230_447_537, 0.039_118_821_050_620_096, 0.070_309_674_406_454_23, 0.132_060_261_896_487_12],
    [-0.589_270_183_908_706, -0.214_093_054_915_683_9, -0.141_787_918_130_149_54, -0.066_492_662_373_398_03, -0.011_019_318_823_587_306, 0.002_868_246_973_742_483_4, 0.009_232_777_964_783_872, 0.014_622_210_591_640_596, 0.018_545_978_729_117_174, 0.022_534_338_154_393_967, 0.025_253_207_200_064_787, 0.028_079_894_872_554_505, 0.031_157_500_584_307_447, 0.035_080_994_843_488_5, 0.046_307_444_450_218_61, 0.065_614_254_824_095, 0.162_447_527_353_247_3],
    [-0.589_270_183_908_706, -0.366_211_884_145_956_73, -0.226_304_557_110_826_73, -0.138_966_112_382_800_4, -0.093_522_260_088_698_34, -0.037_252_899_665_389_88, -0.000_990_950_322_876_330_3, 0.013_409_762_555_404_783, 0.020_460_725_589_227_558, 0.026_121_439_983_550_248, 0.031_077_294_188_241_56, 0.036_328_403_057_913_176, 0.044_947_190_893_612_12, 0.055_892_778_152_104_715, 0.070_488_857_022_850_42, 0.110_605_761_896_984_07, 0.192_236_477_069_595_8],
    [-0.589_270_183_908_706, -0.379_837_003_242_067_15, -0.332_439_569_845_826_2, -0.282_023_754_698_398_6, -0.213_480_927_811_544_96, -0.131_862_788_251_799_3, -0.088_530_166_146_852_05, -0.053_540_093_046_498_37, -0.017_482_455_152_128_94, 0.017_638_459_802_835_446, 0.035_038_109_957_693_93, 0.047_265_066_955_599_76, 0.066_012_016_320_540_06, 0.090_896_430_477_662_29, 0.117_795_867_373_719_1, 0.138_209_399_777_373_54, 0.192_236_477_069_595_8],
    [-0.574_789_484_127_671_5, -0.332_507_062_951_562_85, -0.278_974_914_339_516_93, -0.221_874_834_264_631_14, -0.103_297_516_530_326_32, -0.059_718_733_682_945_93, -0.044_401_796_026_953_32, -0.032_759_482_421_302_83, -0.021_713_872_761_971_46, -0.007_995_218_742_601_848, 0.006_352_433_612_313_398, 0.023_006_047_579_356_717, 0.046_141_112_258_991_58, 0.074_509_717_494_959_8, 0.102_498_133_702_981_2, 0.131_108_989_008_290_85, 0.192_236_477_069_595_8],
    [-0.457_444_176_580_472_54, -0.092_885_233_018_659_36, -0.052_720_523_201_658_83, -0.041_348_069_969_208_22, -0.032_738_575_388_021_22, -0.025_532_332_185_479_685, -0.018_606_145_409_245_694, -0.010_825_880_001_196_664, -0.003_065_598_398_635_169_3, 0.004_823_873_590_894_413, 0.013_718_512_468_841_908, 0.023_897_135_464_754_63, 0.037_935_470_941_634_24, 0.053_897_869_193_695_01, 0.076_053_566_271_379_96, 0.102_114_182_108_512_45, 0.192_236_477_069_595_8],
    [-0.168_130_434_276_997_98, -0.044_613_750_704_334_64, -0.032_807_529_271_019_53, -0.024_673_602_927_008_976, -0.017_556_093_223_462_337, -0.010_527_416_280_321_545, -0.003_636_628_663_334_699, 0.002_912_056_168_759_079_7, 0.010_755_811_107_217_407, 0.017_650_615_562_756_936, 0.026_351_707_110_562_984, 0.037_038_722_337_348_58, 0.049_436_873_116_640_01, 0.067_192_133_900_889_92, 0.083_997_566_969_303_23, 0.110_332_117_549_299_96, 0.170_960_001_902_380_14],
    [-0.453_871_864_829_795_6, -0.162_448_791_906_850_63, -0.117_635_131_268_765_93, -0.089_636_559_665_163_62, -0.063_774_357_898_235_45, -0.038_341_875_621_511_08, -0.015_734_945_187_938_786, -0.003_305_092_404_901_247_4, 0.006_264_433_965_150_65, 0.012_873_896_038_776_23, 0.021_518_221_728_295_65, 0.033_050_011_811_844_41, 0.047_929_961_247_320_19, 0.069_876_786_304_378_03, 0.096_138_702_569_837_31, 0.145_683_659_235_688_05, 0.644_760_927_269_944_8],
    [-0.749_301_412_843_868_9, -0.162_619_914_165_816_2, -0.109_614_359_907_736_82, -0.074_667_982_426_314_5, -0.047_054_314_864_716_11, -0.015_579_929_374_906_641, -0.001_346_734_258_824_847_4, 0.009_259_053_361_709_782, 0.018_080_969_970_018_807, 0.029_897_816_974_008_992, 0.045_971_503_686_210_245, 0.068_455_763_257_028_08, 0.093_677_737_787_813_45, 0.132_454_126_361_323_34, 0.197_010_635_641_245_85, 0.312_413_839_276_996_45, 0.842_352_026_925_371],
    [-0.749_301_412_843_868_9, -0.539_158_324_803_184_2, -0.362_182_609_877_246_93, -0.155_865_233_861_737_53, -0.056_275_929_459_889_84, -0.009_303_312_869_820_17, 0.018_355_892_684_293_766, 0.043_972_472_244_159_89, 0.071_294_686_454_268_55, 0.097_625_593_935_729_03, 0.132_362_221_885_510_78, 0.165_988_684_489_548_8, 0.203_403_233_870_547_84, 0.239_499_099_401_101_1, 0.297_263_564_859_102_94, 0.390_574_904_471_847_6, 1.081_370_965_798_622_6],
    [-0.749_301_412_843_868_9, -0.565_779_934_755_074_6, -0.470_593_536_023_426_74, -0.354_170_022_091_866_43, -0.237_838_082_124_020_66, -0.130_597_701_850_294_46, -0.046_313_874_643_823_66, 0.001_429_813_936_386_344_4, 0.035_184_972_192_069_64, 0.067_706_153_507_608_25, 0.097_459_200_975_186_82, 0.135_247_599_170_439_27, 0.174_506_420_677_683_18, 0.215_022_402_483_561_94, 0.273_529_506_164_586_43, 0.348_113_325_615_798_66, 1.081_370_965_798_622_6],
    [-0.749_301_412_843_868_9, -0.548_890_614_796_212_5, -0.465_819_747_816_033_1, -0.371_364_609_635_183_6, -0.281_194_530_841_124_83, -0.205_356_503_108_035_63, -0.127_629_847_545_272_85, -0.059_127_346_116_958_9, -0.006_214_989_243_816_767, 0.029_891_750_100_076_036, 0.065_383_202_071_528_34, 0.101_212_284_419_265_48, 0.142_066_961_724_984_3, 0.192_089_782_306_471_8, 0.253_105_317_528_074_86, 0.344_140_008_954_835_76, 1.081_370_965_798_622_6],
    [-0.692_180_703_804_929_6, -0.515_549_452_614_649_5, -0.436_894_397_297_807_4, -0.351_878_996_009_537_54, -0.279_847_237_896_477_6, -0.217_809_052_168_216_17, -0.153_366_170_721_878_08, -0.090_028_085_313_612_05, -0.036_153_521_213_193_32, 0.011_326_125_908_845_51, 0.050_290_492_856_604_41, 0.092_215_678_234_672_1, 0.132_070_477_452_620_02, 0.189_007_332_808_777_63, 0.252_926_001_084_336_8, 0.356_239_369_340_891_4, 1.081_370_965_798_622_6],
    [-0.617_125_235_758_684_3, -0.463_208_561_736_907_3, -0.387_118_795_466_282_86, -0.317_166_017_032_020_1, -0.256_822_315_527_822_03, -0.203_975_677_100_498_08, -0.144_325_350_323_929_85, -0.080_255_235_190_952_68, -0.030_388_155_768_951_035, 0.014_035_428_518_352_25, 0.066_545_312_406_669_86, 0.110_720_813_231_751_55, 0.149_408_789_372_922_43, 0.204_996_272_765_235_47, 0.263_883_091_965_399_16, 0.369_083_749_575_353_9, 1.045_338_107_544_225_2],
    [-0.540_797_011_096_184_5, -0.438_716_899_559_915, -0.355_496_285_363_168, -0.299_096_138_234_714_65, -0.232_922_361_219_644_38, -0.176_981_123_175_387_14, -0.125_284_500_968_635_91, -0.067_480_013_852_258_06, -0.015_676_632_737_787_966, 0.024_455_038_170_700_657, 0.071_229_961_148_321, 0.118_636_470_001_961_09, 0.156_370_021_519_022_74, 0.205_583_955_525_516_78, 0.263_880_943_169_702_95, 0.351_855_511_658_859, 1.007_375_906_404_493],
    [-0.507_802_677_621_412_3, -0.400_721_549_821_838_83, -0.334_183_027_166_784_3, -0.270_377_011_565_844_7, -0.203_654_896_184_516_5, -0.151_764_749_145_339_22, -0.099_290_403_789_237_36, -0.057_685_864_179_482_405, -0.012_584_687_401_589_072, 0.025_963_215_824_674_875, 0.068_390_908_541_440_92, 0.115_314_577_957_824_99, 0.149_359_846_009_984_6, 0.206_661_138_174_211_42, 0.268_191_619_367_991_46, 0.358_407_981_925_429_5, 1.007_375_906_404_493],
    [-0.506_609_858_043_081_9, -0.343_073_759_371_670_9, -0.278_349_278_268_649_64, -0.219_137_165_538_952_8, -0.174_138_034_779_864_4, -0.131_496_001_384_754_65, -0.092_903_722_048_414_09, -0.054_513_542_752_586_82, -0.013_038_940_397_886_415, 0.026_826_221_686_785_112, 0.071_268_955_456_731_29, 0.110_747_875_257_736_16, 0.147_924_097_868_660_34, 0.209_666_496_381_824_86, 0.279_166_181_478_344_3, 0.387_564_927_222_033_7, 0.976_898_651_744_744_7],
    [-0.501_769_234_642_144_2, -0.276_371_552_939_682_35, -0.222_485_190_897_979_05, -0.183_227_020_436_527_17, -0.147_762_381_106_262_16, -0.115_912_903_428_786_73, -0.086_890_955_582_973_44, -0.049_923_257_095_413_1, -0.011_794_400_715_418_036, 0.025_060_907_947_302_962, 0.066_586_398_008_042_04, 0.104_063_440_676_669_73, 0.145_386_489_329_468_15, 0.205_809_078_473_869_17, 0.277_822_864_115_843_34, 0.383_969_596_733_496_2, 0.942_376_623_888_753],
    [-0.397_063_018_657_122_8, -0.223_574_736_860_892_66, -0.182_385_865_093_337_4, -0.149_980_516_388_575_76, -0.124_906_529_003_993, -0.094_529_077_712_827_63, -0.062_902_668_950_406_37, -0.029_958_952_268_534_2, 0.001_911_779_222_362_659, 0.032_767_149_787_070_09, 0.068_056_400_500_369_48, 0.108_410_077_012_336_46, 0.153_358_523_601_074_46, 0.209_760_300_089_692_26, 0.277_891_380_278_223_24, 0.379_118_453_341_836_25, 0.693_367_862_007_536],
    [-0.305_173_427_487_956_84, -0.180_223_462_657_045_34, -0.142_402_934_069_035_4, -0.112_543_321_292_841_96, -0.084_459_732_796_088_29, -0.056_288_138_093_193_99, -0.032_850_309_001_509_634, -0.007_909_033_314_055_808, 0.016_396_779_266_489_758, 0.048_476_100_779_495_58, 0.090_229_395_383_652_86, 0.130_261_105_839_449_6, 0.173_649_686_029_939_05, 0.226_915_363_698_301_56, 0.309_092_540_664_535_7, 0.399_114_770_387_994_87, 0.914_413_558_292_821_9],
    [-0.230_556_217_590_667_74, -0.133_455_717_422_784_2, -0.100_018_455_276_423_21, -0.074_788_731_279_549_78, -0.053_965_745_259_320_28, -0.038_712_407_309_296_2, -0.021_912_422_979_505_17, 0.002_552_524_215_935_781_4, 0.030_990_914_151_349_025, 0.066_332_037_508_552_1, 0.108_345_608_925_616_26, 0.150_149_310_210_790_06, 0.201_183_374_548_902_34, 0.252_247_563_458_079_47, 0.349_319_816_124_152_7, 0.420_631_953_963_162_66, 0.914_413_558_292_821_9],
    [-0.162_144_440_543_557_7, -0.092_200_429_211_148_38, -0.072_793_755_009_783_9, -0.058_772_387_108_692_32, -0.050_309_384_834_781_94, -0.044_589_331_323_580_48, -0.034_819_561_672_975_397, -0.022_420_970_598_088_33, 0.000_643_792_356_343_031_2, 0.038_802_848_912_152_424, 0.087_212_125_886_191_52, 0.148_117_849_208_110_5, 0.206_002_838_794_459_02, 0.289_022_380_225_204_05, 0.388_356_819_104_466_9, 0.436_420_726_971_062_27, 0.914_413_558_292_821_9],
    [-0.162_144_440_543_557_7, -0.141_298_189_917_279_16, -0.100_647_711_722_343_71, -0.087_165_649_136_780_07, -0.075_128_897_694_942_65, -0.069_504_238_912_872_77, -0.053_665_396_507_979_42, -0.021_305_947_060_717_92, -0.012_301_303_704_613_338, -0.007_027_722_806_108_566, -0.000_286_809_659_491_944, 0.004_657_736_437_325_051, 0.016_554_141_886_477_837, 0.029_304_303_703_571_15, 0.037_326_332_169_024_326, 0.058_841_012_688_203_215, 0.914_413_558_292_821_9],
    [-0.162_144_440_543_557_7, -0.096_309_461_380_209_27, -0.079_293_631_581_377_73, -0.070_034_570_403_588_87, -0.053_457_562_729_722_15, -0.020_931_584_002_913_735, -0.010_992_728_478_411_552, -0.005_648_416_099_863_654, 0.001_297_822_270_926_705_7, 0.008_809_269_479_500_647, 0.017_280_721_446_085_057, 0.026_374_853_125_653_86, 0.030_846_779_075_741_6, 0.039_376_818_410_751_4, 0.050_715_160_538_470_395, 0.064_979_745_320_494_93, 0.668_550_211_856_005_2],
    [-0.353_302_774_176_651, -0.118_315_545_413_820_24, -0.075_766_813_685_705_37, -0.051_371_581_903_624_51, -0.032_930_778_917_042_15, -0.020_152_981_814_028_47, -0.007_705_521_776_838_409_5, 0.004_812_415_476_216_739, 0.016_392_809_697_973_66, 0.024_116_540_982_856_524, 0.034_036_870_143_821_424, 0.045_603_840_505_776_455, 0.058_025_113_050_334_476, 0.077_915_705_683_826_4, 0.094_487_766_897_620_96, 0.108_533_461_748_610_07, 0.156_656_853_698_034_84],
    [-0.489_889_988_259_710_3, -0.172_824_267_277_765_81, -0.122_616_290_568_658_86, -0.090_057_725_445_788_56, -0.065_245_754_561_207_42, -0.043_889_359_722_094_665, -0.026_734_837_883_646_174, -0.012_212_399_194_885_986, 0.001_954_661_633_521_027_6, 0.016_110_504_730_805_823, 0.026_485_694_979_765_405, 0.039_445_189_827_524_214, 0.052_946_967_214_750_82, 0.066_900_830_111_439_94, 0.082_815_776_705_200_02, 0.111_337_148_942_398_18, 0.172_017_828_109_845_9],
    [-0.683_659_544_315_421_6, -0.232_732_226_028_512_2, -0.163_855_720_854_119_5, -0.118_988_175_395_121_52, -0.089_987_888_996_803_21, -0.066_487_086_436_618_38, -0.045_057_985_066_888_15, -0.023_436_507_986_079_13, -0.004_058_534_263_396_639_5, 0.012_171_791_865_654_077, 0.028_137_513_322_477_905, 0.044_314_228_759_024_76, 0.058_841_126_716_970_484, 0.072_378_200_121_860_01, 0.084_402_290_831_549_46, 0.121_131_833_718_073_83, 0.179_179_245_391_880_57],
    [-0.725_682_281_903_099, -0.295_754_488_606_699_23, -0.205_818_034_051_907_1, -0.146_590_778_110_224_06, -0.107_595_670_051_971_56, -0.076_840_037_674_331_94, -0.053_076_748_833_220_89, -0.028_338_700_668_499_747, -0.001_596_781_756_305_148_7, 0.018_075_717_372_073_582, 0.036_190_272_415_194_55, 0.054_071_796_937_655_36, 0.068_850_982_384_038_03, 0.080_417_316_662_746_97, 0.094_200_154_762_323_81, 0.133_169_897_034_690_34, 0.188_374_144_742_624_6],
    [-0.776_847_671_709_758_1, -0.339_047_586_638_618_84, -0.236_743_585_666_752_35, -0.165_381_569_111_692_72, -0.120_543_137_816_463_75, -0.083_403_645_143_039_1, -0.053_702_951_948_212_56, -0.027_984_355_931_495_58, 0.002_388_493_792_285_763_3, 0.023_160_441_403_633_682, 0.042_062_953_593_693_615, 0.060_234_174_265_110_41, 0.075_469_775_162_041_88, 0.087_692_341_796_694_98, 0.101_182_118_190_036_3, 0.140_135_859_285_639, 0.190_112_979_408_759_49],
    [-0.960_877_314_725_559_4, -0.373_944_916_893_920_7, -0.264_430_265_102_041_54, -0.187_187_962_024_122_36, -0.130_861_502_183_761, -0.090_275_425_755_697_72, -0.056_914_001_380_925_51, -0.028_412_631_256_457_016, 4.482_247_486_314_736_5e-5, 0.022_419_504_941_483_09, 0.043_045_251_490_915_51, 0.061_171_838_526_107_275, 0.077_214_909_279_623_63, 0.090_666_306_174_567_63, 0.103_104_680_771_835_8, 0.138_258_004_281_270_39, 0.198_578_927_062_982_3],
    [-1.085_861_719_601_041_4, -0.416_004_741_320_373_35, -0.283_445_417_765_247_45, -0.200_423_181_448_662_8, -0.142_024_018_320_348_83, -0.096_632_823_177_252_91, -0.058_787_191_528_532_165, -0.027_942_487_974_685_484, 0.000_173_386_872_212_853_93, 0.024_000_582_826_133_706, 0.045_121_702_481_119_394, 0.063_389_050_591_604_99, 0.079_581_573_158_954_2, 0.094_586_866_169_478_78, 0.107_422_056_303_286_5, 0.138_507_958_994_293_83, 0.210_826_643_768_069_03],
    [-1.134_429_206_077_859_6, -0.461_660_612_918_397_8, -0.314_067_312_363_398_13, -0.220_708_731_587_330_4, -0.152_310_760_617_999_75, -0.101_752_692_468_570_73, -0.061_744_484_731_248_93, -0.027_764_773_678_312_59, 0.001_121_391_115_887_581_4, 0.026_631_496_270_187_76, 0.047_534_719_877_167_016, 0.068_297_670_823_638_62, 0.085_208_233_656_004_69, 0.100_486_924_162_787_18, 0.114_396_714_435_536_23, 0.140_756_459_800_246_6, 0.221_373_849_157_103_56],
    [-1.219_129_026_985_575_2, -0.517_301_386_629_929, -0.353_995_208_283_993_17, -0.255_539_506_294_858_3, -0.171_526_796_849_873_32, -0.114_027_786_117_936_2, -0.070_212_950_855_755_82, -0.034_603_373_940_077_82, -0.002_833_435_612_731_813, 0.025_866_254_334_585_354, 0.047_904_664_312_447_86, 0.070_822_212_942_908_98, 0.090_224_924_790_603_44, 0.105_564_287_240_805_48, 0.119_792_875_434_414_11, 0.140_537_904_589_878_97, 0.224_447_456_701_952_32],
    [-1.328_951_888_573_513_4, -0.539_981_118_731_980_6, -0.364_972_225_888_943_96, -0.264_766_487_391_815_1, -0.182_416_749_168_521_13, -0.123_669_559_619_808_26, -0.074_919_001_989_341_89, -0.037_721_227_621_868_99, -0.004_019_818_869_300_411, 0.024_760_470_957_933_055, 0.049_943_128_697_900_095, 0.074_010_721_173_117_26, 0.095_432_224_026_547_55, 0.110_437_432_837_863_32, 0.126_150_880_113_636_48, 0.143_121_103_897_703_88, 0.224_447_456_701_952_32],
    [-1.469_734_429_799_487_4, -0.512_994_503_635_161_1, -0.345_793_056_190_679_5, -0.245_381_042_759_585_3, -0.173_150_113_823_453_8, -0.116_361_967_603_863_69, -0.066_755_257_325_825_64, -0.031_012_393_892_965_88, -0.001_101_124_161_404_998_3, 0.027_459_814_036_090_876, 0.053_201_691_793_830_37, 0.081_037_698_271_621_58, 0.098_555_550_878_057_66, 0.114_687_955_067_842_07, 0.129_576_636_776_246_6, 0.152_726_954_145_753_66, 0.224_447_456_701_952_32],
    [-1.719_718_939_751_588_8, -0.511_238_020_393_141_5, -0.344_534_641_110_551_15, -0.239_545_225_504_113_67, -0.168_928_431_362_155_07, -0.115_692_729_845_725_6, -0.069_407_921_852_619_33, -0.032_408_815_782_708_21, -0.001_752_377_217_793_782_2, 0.028_746_688_956_538_39, 0.052_460_381_083_965_46, 0.077_510_159_808_009_85, 0.095_496_712_871_309_13, 0.111_960_730_939_499_68, 0.127_692_490_778_500_23, 0.157_503_285_746_399_85, 0.224_447_456_701_952_32],
    [-1.854_356_837_626_653_5, -0.521_509_484_029_734_4, -0.348_648_656_589_219, -0.238_237_798_761_231_4, -0.164_295_691_623_388_17, -0.112_693_977_216_447_79, -0.070_888_346_343_997_02, -0.031_207_796_826_963_91, -0.000_828_269_055_511_836, 0.029_591_948_641_761_897, 0.052_528_310_880_438_11, 0.075_634_100_173_978_41, 0.094_712_846_225_441_64, 0.110_229_990_165_946_66, 0.125_363_085_364_523_76, 0.159_097_464_866_308_96, 0.224_067_056_680_803_84],
    [-1.854_356_837_626_653_5, -0.491_917_687_162_996_7, -0.334_308_632_083_932_97, -0.225_773_230_183_225_37, -0.155_686_241_553_646_94, -0.108_808_645_241_101_13, -0.068_347_791_709_464_99, -0.029_192_212_840_793_912, 0.001_245_407_303_931_380_3, 0.030_011_878_903_301_22, 0.054_121_424_866_446_305, 0.077_073_478_813_787_33, 0.095_370_050_662_189_19, 0.109_366_154_524_009_57, 0.123_627_678_186_978_92, 0.158_726_769_979_309_32, 0.222_575_225_470_334_81],
    [-1.854_356_837_626_653_5, -0.464_391_044_603_562_7, -0.313_703_831_948_145_44, -0.215_267_854_306_339_7, -0.153_083_162_006_520_45, -0.104_623_062_162_973_78, -0.064_123_346_390_079_54, -0.029_285_674_255_913_19, -0.000_517_593_480_107_509_6, 0.027_153_018_367_513_514, 0.051_862_585_369_313_007, 0.073_344_284_863_613_34, 0.091_090_989_755_136_95, 0.106_843_299_452_869_1, 0.119_454_953_209_511_52, 0.152_662_534_308_609_6, 0.216_079_936_064_547_34],
    [-1.854_356_837_626_653_5, -0.436_149_391_069_512_8, -0.293_921_902_911_333, -0.203_313_709_706_259_8, -0.147_737_253_801_970_14, -0.094_512_099_080_435_34, -0.055_731_544_263_867_006, -0.026_576_105_423_593_63, -0.000_451_895_850_512_977_55, 0.026_130_378_382_000_32, 0.047_938_607_846_453_49, 0.068_270_177_114_644_73, 0.082_895_181_873_826_5, 0.098_879_552_926_884, 0.112_777_067_926_569_91, 0.142_658_289_091_194_82, 0.207_491_053_439_413_88],
    [-1.532_337_020_787_173, -0.362_888_141_221_171_13, -0.244_182_300_141_591_7, -0.168_807_689_458_639_2, -0.113_926_968_878_089_22, -0.072_156_841_573_199_3, -0.041_280_414_675_334_25, -0.014_121_112_311_904_495, 0.006_287_375_708_486_889, 0.031_064_437_301_143_624, 0.045_959_416_114_736_71, 0.063_389_109_647_713, 0.077_587_552_427_521_56, 0.089_483_929_784_845_35, 0.101_406_911_339_069_03, 0.122_851_867_431_515_09, 0.205_798_968_200_156_2],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_27: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.636_574_480_072_413, 10.0),
    (-12.186_017_939_738_319, -9.862_044_621_323_495),
    (-12.843_348_703_460_642, -10.283_738_169_187_547),
    (-12.819_657_778_664_542, -6.713_369_259_291_587_5),
    (-0.204_776_735_693_378_04, 0.931_452_301_651_068_8),
    (-0.238_484_750_398_751_14, 0.312_245_524_690_239_7),
    (1.732_803_032_711_984, 6.418_404_095_033_965),
    (0.346_600_488_638_014_74, 1.611_673_965_209_735_2),
    (-2.186_322_183_828_693_7, -0.040_137_452_641_955_22),
    (0.048_467_055_280_656_4, 0.862_400_317_428_598_8),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_27: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.114_485_116_116_303_9, 1.032_241_860_957_166_4, 0.912_921_009_223_059_7, 0.658_516_799_427_092_1, 0.565_793_831_502_227_5, 0.544_030_959_751_890_6, 0.543_654_455_362_870_6, 0.633_081_573_876_899_3, 0.622_502_534_311_945, 0.591_928_434_276_728_4, 0.576_068_790_808_718_5, 0.571_137_278_652_728, 0.571_972_082_324_220_7, 0.575_274_720_294_333, 0.544_910_327_096_778_2, 1.330_104_167_211_692_7, 1.333_230_412_493_017],
    [0.229_865_197_774_523_16, 0.210_895_122_829_479_5, 0.191_054_199_000_683_66, 0.167_256_414_534_006_75, 0.143_210_030_879_783_5, 0.117_975_054_605_656_76, 0.091_375_054_618_679_27, 0.064_537_708_320_776_34, 0.037_599_895_560_310_09, 0.009_086_313_197_601_245, -0.001_544_650_911_268_551, 0.137_858_678_432_215_53, 0.185_658_390_037_057_42, 0.226_155_065_346_374_56, 0.276_316_997_232_443_63, 0.302_036_403_259_354_4, -0.164_874_674_398_830_16],
    [0.274_477_062_835_317_3, 0.257_343_817_310_399_74, 0.233_978_318_894_311_3, 0.204_702_396_699_970_26, 0.170_345_875_604_730_1, 0.135_260_974_493_685_4, 0.100_569_057_325_043_13, 0.067_554_241_484_370_81, 0.031_111_963_065_333_686, -0.005_200_615_962_811_936, -0.032_147_509_381_515_93, -0.074_116_912_699_361_58, -0.107_014_583_041_881_55, -0.138_064_290_671_479_6, -0.170_742_789_781_779_04, -0.196_471_292_096_418_45, -0.240_989_368_252_761_93],
    [2.268_517_925_175_288, 2.268_517_925_175_288, 2.268_517_925_175_288, 2.268_517_925_175_288, 2.268_517_925_175_288, 2.268_517_925_175_288, 2.268_517_925_175_288, 2.268_517_925_175_288, 2.111_610_465_651_022_4, 2.101_200_681_256_35, 2.082_235_362_686_176, 2.090_499_293_009_260_3, 2.090_499_293_009_260_3, 2.090_499_293_009_260_3, 2.090_499_293_009_260_3, 2.090_499_293_009_260_3, 2.090_499_293_009_260_3],
    [-1.220_581_778_254_945_7, -1.314_049_977_687_990_8, -1.415_541_030_302_812_4, -1.494_991_294_264_280_2, -1.581_473_206_448_296_4, -1.646_884_850_694_387_2, -1.666_547_277_184_852, -0.772_654_929_308_157_6, -0.639_853_529_389_689_4, -0.578_753_864_828_334_8, -0.539_073_458_800_865_1, -0.541_697_639_208_327_4, -0.521_775_619_565_030_4, -0.138_744_412_282_991_4, -0.061_021_201_504_118_584, -0.005_868_423_636_523_798, 0.028_400_167_113_249_088],
    [-1.669_418_764_527_514_5, -1.713_793_815_176_364_3, -1.741_859_950_263_924_5, -1.783_281_902_142_912_5, -1.806_053_579_735_176_9, -1.847_320_336_287_909, -1.883_801_640_919_432_5, -1.913_997_208_308_619_9, -1.952_208_706_690_695_2, -1.986_710_588_111_076_5, -2.020_864_244_516_385_4, -2.056_572_653_260_219_3, -2.092_620_661_999_525_6, -2.124_254_971_770_141_8, -2.151_353_397_140_736_6, -2.174_536_060_088_051, -2.191_896_491_834_132],
    [-0.432_682_147_709_932_96, -0.481_609_578_104_169_4, -0.548_029_371_798_639_5, -0.614_659_864_354_391_5, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3, -0.631_446_936_404_848_3],
    [0.140_357_056_379_873_75, 0.200_222_894_408_731_14, 0.304_873_754_725_604_13, 0.356_658_785_102_541_1, 0.418_819_387_208_096_25, 0.486_292_792_616_050_63, 0.575_545_463_150_330_8, 0.636_070_282_153_172_7, 0.688_590_692_118_613_2, 0.734_827_083_342_590_4, 0.771_923_506_978_558_9, 0.805_825_606_741_503_4, 0.818_643_365_261_997_4, 0.732_956_826_955_688_9, 0.639_964_887_011_971_9, 0.655_472_169_152_643_7, 0.672_669_019_814_724_2],
    [0.306_371_507_231_263_5, 0.298_959_388_099_905_24, -0.233_718_509_364_349_35, -0.392_369_745_214_548_8, -0.472_551_536_327_260_8, -0.552_345_698_002_694_7, -0.584_889_583_963_979_6, -0.597_601_490_653_589_9, -0.586_558_691_859_627_3, -0.569_546_109_445_358_8, -0.536_217_965_020_821_3, -0.479_203_315_191_887_3, -0.440_038_138_141_672_4, -0.417_426_610_958_028, -0.366_374_282_975_361_9, 0.008_908_552_280_323_475, 0.034_205_859_364_661_39],
    [9.966_126_603_382_406, 9.938_548_925_521_08, 9.915_912_905_951_185, 9.905_745_349_694_827, 9.898_663_391_807_997, 9.889_303_891_227_847, 9.883_500_233_283_018, 9.878_042_277_869_636, 9.865_857_252_295_799, 9.854_540_621_641_542, 9.854_540_621_641_542, 9.855_524_330_337_825, 9.862_513_488_087_654, 9.870_388_505_819_19, 9.872_215_452_319_395, 9.877_449_478_142_415, 9.894_312_447_969_522],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_27: [(f64, f64); 11] = [
    (5.419_694_594_756_431, 10.0),
    (0.312_982_068_566_387_77, 2.122_360_809_658_985_7),
    (-0.283_133_262_180_449_04, 1.825_870_354_157_654_6),
    (-1.043_670_299_374_084_3, 0.334_243_872_382_405_94),
    (1.720_296_526_975_577_4, 7.145_041_221_830_653),
    (-1.909_140_242_470_061_4, 0.478_406_778_667_533_84),
    (-2.246_340_362_573_716_4, 0.243_429_318_416_641_12),
    (-0.771_064_759_797_293_6, 0.336_777_716_470_267_6),
    (-0.051_977_808_826_974_16, 0.909_955_207_761_149),
    (-1.096_848_077_381_047_8, 0.699_947_149_381_942_1),
    (8.017_791_846_134_891, 10.079_054_057_276_323),
];

/// The slow accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_DONOR_KINDS_27: [f64; 3] = [
    0.911_486_187_243_549_1,
    0.088_362_195_026_554_68,
    0.000_151_617_729_896_170_52,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    1.857_213_569_845_463_3e-7,
    3.032_537_917_963_637_6e-6,
    2.102_767_402_624_474_8e-6,
    2.405_998_254_830_147_5e-6,
    3.060_689_520_606_453_4e-6,
    2.844_895_934_984_777_4e-6,
    1.017_275_123_269_505e-6,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.113_406_582_497_867_77,
    0.866_455_144_773_028_7,
    0.020_138_272_729_103_675,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    4.393_215_172_041_945_4e-7,
    7.297_064_632_496_492e-5,
    0.000_140_106_201_786_780_13,
    0.000_110_992_046_251_691_38,
    9.440_088_088_636_791e-5,
    9.518_598_432_069_217e-5,
    0.000_165_707_203_329_893_25,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.511_149_040_418_476_6,
    0.471_647_878_122_391_8,
    0.017_203_081_459_131_558,
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
