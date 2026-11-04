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
// since-generator-version: 27
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the rows by donor kind against Pala et al. 2020, MNRAS 494, 3799, Carter et al. 2013,
//   MNRAS 429, 2143, and Lü, Yungelson and Han 2006, MNRAS 372, 1389, the total against the
//   brainstorm's 6–12 × 10⁶
// acceptance: kind scales 1.6635e-1/7.4559e-2/1.0634e-2; fast: 2501 members, own nodes
//   0/15/14/0/6/17/0/4/16/17/17, donors 0.992/0.000/0.008, layers C 0.108 D 0.868 E 0.024, rates
//   a system 7.01e-7/5.50e-5/1.11e-5/1.54e-5/1.96e-5/2.03e-5/6.31e-6, not its own: HeliumPeriod
//   Scratch, HeliumDonorMass Scratch; slow: 25488 members, own nodes
//   0/16/17/16/5/17/16/9/17/17/17, donors 0.891/0.108/0.001, layers C 0.378 D 0.608 E 0.014,
//   rates a system 4.91e-6/5.20e-4/8.53e-4/6.95e-4/6.23e-4/6.26e-4/1.01e-3; the K–S tests
//   against the other half of the census are the sim's slow
//   `fitted_tables::the_accreting_white_dwarf_samplers_against_the_engine`
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
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [-0.923_330_085_215_136_3, -0.362_867_169_207_415_5, -0.248_401_063_296_856_68, -0.189_831_140_538_87, -0.148_027_045_364_524_66, -0.108_685_491_271_142_96, -0.073_844_195_238_353_45, -0.029_753_588_351_842_82, 0.001_212_599_723_378_336_4, 0.027_481_413_535_579, 0.051_151_576_825_203_66, 0.077_173_280_074_073_98, 0.109_343_347_278_879_98, 0.151_391_365_839_337_54, 0.186_151_990_577_196_06, 0.216_268_972_552_910_17, 0.320_361_121_893_641_1],
    [-1.285_301_100_270_818_6, -0.464_253_430_828_213_5, -0.178_157_008_499_203_6, -0.116_553_268_292_537_7, -0.089_218_709_976_712_15, -0.068_346_027_146_397_63, -0.045_411_314_794_198_65, -0.022_389_147_125_601_996, 0.000_930_028_235_976_871_7, 0.021_065_230_555_770_476, 0.051_704_157_584_994_234, 0.131_738_538_850_878_22, 0.227_512_906_566_634_28, 0.362_097_578_934_113_76, 0.493_003_448_258_641_3, 0.656_131_765_531_592_4, 1.268_059_362_837_156_1],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.651_556_897_545_603_1, -0.200_103_085_176_663_28, -0.132_676_733_400_193_18, -0.095_658_353_383_396_14, -0.072_654_963_489_268_28, -0.051_672_387_461_478_03, -0.036_164_223_820_753_91, -0.019_399_228_640_302_555, -0.003_963_605_206_468_792, 0.009_106_650_729_080_951, 0.021_914_626_101_453_904, 0.036_413_835_436_790_84, 0.052_500_245_133_443_17, 0.074_200_901_911_139_23, 0.098_399_468_424_746_12, 0.140_903_271_456_548_93, 0.742_969_326_089_614_2],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.285_608_117_672_971_7, -0.125_047_797_964_321_94, -0.088_463_501_068_343_97, -0.068_229_008_409_247_49, -0.053_024_956_252_053_064, -0.042_287_764_318_317_614, -0.031_338_483_659_560_9, -0.019_951_990_462_582_198, -0.005_776_398_482_576_008, 0.011_404_871_651_942_61, 0.027_814_890_235_815_813, 0.047_604_100_938_750_64, 0.064_251_348_595_350_03, 0.079_650_843_200_979_91, 0.103_116_215_108_142_5, 0.171_173_475_041_212_94, 0.325_676_844_884_152_6],
    [-0.711_377_142_356_211_9, -0.250_652_296_971_277_3, -0.192_832_456_381_794_44, -0.158_946_455_043_429_83, -0.117_236_022_258_645_16, -0.081_639_952_505_222_5, -0.049_345_359_527_928_456, -0.021_876_671_324_708_025, 0.001_396_935_191_005_895_8, 0.022_639_730_511_125_014, 0.042_316_813_948_872_37, 0.063_433_086_203_496_29, 0.081_520_289_481_382_47, 0.100_472_374_793_559_76, 0.123_056_653_087_124_68, 0.159_540_612_386_147_28, 0.381_565_767_985_836_7],
    [-1.420_661_206_756_072_7, -0.493_473_409_879_718_5, -0.348_759_157_683_216, -0.271_956_263_902_143_3, -0.206_571_411_207_827_68, -0.148_302_036_938_386_45, -0.098_563_579_388_337_89, -0.049_645_064_010_132_765, 0.000_917_369_713_423_477_3, 0.051_524_927_250_755_47, 0.102_542_662_610_998_56, 0.148_920_699_942_645_17, 0.203_471_486_436_158_23, 0.262_497_797_330_791_8, 0.326_151_125_891_410_7, 0.434_235_919_183_808_5, 0.700_463_748_651_957_4],
];

/// The fast accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_FAST_NODE_RESIDUALS_27: [[f64; 17]; 187] = [
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [2.600_392_314_909_610_6, 3.734_806_437_371_2, 4.106_755_158_829_078, 4.357_839_987_648_332, 4.531_005_144_866_936, 4.651_567_499_439_796, 4.747_632_445_636_984, 4.834_841_404_781_351_5, 4.896_035_687_647_471, 4.969_450_853_395_998, 5.040_537_519_685_821, 5.121_582_814_046_503, 5.185_856_752_655_713, 5.247_400_793_710_946, 5.306_126_554_428_198, 5.358_228_232_356_788_5, 5.418_370_850_344_337_5],
    [-0.923_330_085_215_136_3, -0.362_867_169_207_415_5, -0.248_401_063_296_856_68, -0.189_831_140_538_87, -0.148_027_045_364_524_66, -0.108_685_491_271_142_96, -0.073_844_195_238_353_45, -0.029_753_588_351_842_82, 0.001_212_599_723_378_336_4, 0.027_481_413_535_579, 0.051_151_576_825_203_66, 0.077_173_280_074_073_98, 0.109_343_347_278_879_98, 0.151_391_365_839_337_54, 0.186_151_990_577_196_06, 0.216_268_972_552_910_17, 0.320_361_121_893_641_1],
    [-0.923_330_085_215_136_3, -0.362_867_169_207_415_5, -0.248_401_063_296_856_68, -0.189_831_140_538_87, -0.148_027_045_364_524_66, -0.108_685_491_271_142_96, -0.073_844_195_238_353_45, -0.029_753_588_351_842_82, 0.001_212_599_723_378_336_4, 0.027_481_413_535_579, 0.051_151_576_825_203_66, 0.077_173_280_074_073_98, 0.109_343_347_278_879_98, 0.151_391_365_839_337_54, 0.186_151_990_577_196_06, 0.216_268_972_552_910_17, 0.320_361_121_893_641_1],
    [-0.923_330_085_215_136_3, -0.872_443_351_884_781_7, -0.653_942_227_473_304_2, -0.589_784_476_341_348_5, -0.377_206_644_116_477_86, -0.290_070_892_136_527_3, -0.170_577_314_164_926_08, -0.079_207_362_884_207_59, -0.001_783_635_678_089_729, 0.038_293_974_565_227_554, 0.056_902_430_677_338_085, 0.079_829_966_649_839_74, 0.095_640_351_738_246_16, 0.126_999_748_549_135_23, 0.152_565_025_030_911_74, 0.180_689_303_547_470_37, 0.196_512_943_336_197_62],
    [-0.923_330_085_215_136_3, -0.773_915_992_114_013, -0.644_433_393_367_386, -0.576_476_319_967_412_6, -0.344_806_160_184_984_67, -0.286_743_784_553_539_6, -0.207_577_112_459_444_1, -0.135_555_130_606_430_82, -0.005_361_723_661_326_713_5, 0.038_697_512_861_453_606, 0.056_677_627_648_885_5, 0.082_915_617_166_213_85, 0.113_078_268_463_703_61, 0.148_271_570_476_766_95, 0.165_383_726_069_872_28, 0.181_818_838_531_383_7, 0.196_512_943_336_197_62],
    [-0.885_561_259_743_636_9, -0.709_021_238_162_823_8, -0.636_494_477_853_616_2, -0.364_902_134_189_566_63, -0.290_270_920_840_587_64, -0.219_803_830_744_707, -0.156_571_251_699_929_75, -0.080_190_853_189_010_05, -0.002_650_794_602_459_651_8, 0.038_640_237_466_207_296, 0.051_357_994_624_472_04, 0.076_047_770_969_338_82, 0.099_408_844_028_063_21, 0.144_260_934_266_776_66, 0.167_237_620_012_567, 0.181_571_570_012_641_82, 0.196_784_211_290_293_55],
    [-0.885_561_259_743_636_9, -0.688_797_636_306_229_7, -0.314_761_864_569_667_9, -0.226_095_870_408_184_77, -0.165_578_856_471_856_55, -0.106_789_810_510_958_7, -0.075_632_952_488_113_55, -0.015_433_689_472_337_146, 0.012_556_663_696_733_122, 0.043_646_204_222_009_76, 0.058_839_809_236_823_56, 0.083_015_813_274_426_52, 0.138_669_840_293_429_55, 0.165_026_948_834_362_7, 0.176_058_135_740_882_75, 0.191_751_340_169_335_04, 0.215_873_127_516_046_81],
    [-0.885_561_259_743_636_9, -0.311_816_321_461_973, -0.197_575_961_419_694_44, -0.157_034_721_181_160_9, -0.117_881_690_255_012_01, -0.084_917_539_646_267_78, -0.043_474_965_437_874_7, -0.007_299_273_087_250_289, 0.012_353_504_475_663_277, 0.045_287_272_111_010_95, 0.074_053_843_239_994_48, 0.104_669_105_593_922_01, 0.152_544_904_134_183_37, 0.169_104_986_218_257_76, 0.187_095_637_432_250_22, 0.197_792_553_432_822_04, 0.238_474_075_347_693_27],
    [-0.760_616_194_701_247_5, -0.284_985_134_242_873_3, -0.174_644_177_024_917_27, -0.144_905_421_926_174_48, -0.119_242_254_695_120_48, -0.087_463_749_316_460_82, -0.051_125_576_692_467_36, -0.017_163_918_961_155_14, 0.007_917_950_062_578_37, 0.027_312_451_786_627_29, 0.055_340_866_428_994_07, 0.096_217_194_589_829_04, 0.141_462_189_468_374_63, 0.169_997_190_543_443_28, 0.194_586_023_554_654_32, 0.211_253_976_164_622_74, 0.272_899_165_367_826_55],
    [-0.760_616_194_701_247_5, -0.262_984_261_136_803_9, -0.159_593_593_203_317_26, -0.140_381_049_739_833_5, -0.116_429_323_049_761_24, -0.074_914_545_401_177_13, -0.049_895_467_083_625_91, -0.021_476_360_640_036_524, 0.006_295_715_825_155_333, 0.025_044_280_255_594_58, 0.048_320_067_556_522_09, 0.092_163_098_998_512_55, 0.138_442_365_351_793_1, 0.180_328_508_278_354_87, 0.204_754_088_085_819_35, 0.228_358_391_281_133_7, 0.283_344_069_481_175_74],
    [-0.760_616_194_701_247_5, -0.279_269_467_349_900_2, -0.177_871_249_378_656_28, -0.126_556_736_790_746_55, -0.107_388_657_664_707_41, -0.076_467_695_397_900_6, -0.051_542_301_266_835_7, -0.026_068_077_650_660_92, 0.002_768_746_901_215_406, 0.023_144_298_369_147_88, 0.041_456_753_233_806_27, 0.077_780_438_874_821_47, 0.126_541_626_920_488_62, 0.177_182_037_587_837_32, 0.210_688_303_496_068_43, 0.238_366_451_899_229_95, 0.303_930_937_375_330_06],
    [-0.739_239_493_695_204_9, -0.305_414_588_319_917_5, -0.205_843_093_760_428_7, -0.141_742_164_375_356_58, -0.113_623_941_211_269_3, -0.090_740_502_137_957_4, -0.067_512_486_385_485_75, -0.034_881_869_619_369_676, -0.001_875_729_502_816_421_1, 0.021_912_315_767_068_324, 0.041_836_272_995_374_4, 0.074_886_489_924_649_14, 0.112_075_291_247_409_82, 0.170_965_714_759_042_8, 0.208_736_909_048_539_17, 0.235_462_300_246_776_64, 0.303_930_937_375_330_06],
    [-0.739_239_493_695_204_9, -0.329_936_466_119_879_2, -0.219_574_998_065_474_42, -0.161_902_462_547_856_2, -0.123_997_384_691_044_46, -0.096_626_306_542_734_03, -0.070_713_917_986_580_04, -0.030_211_318_001_675_32, 0.000_947_733_622_837_748_3, 0.028_180_078_874_348_695, 0.051_019_329_687_144_184, 0.080_765_020_950_449_6, 0.120_144_638_253_823_75, 0.175_384_607_392_643_05, 0.212_439_590_478_488_5, 0.234_709_900_707_468_28, 0.316_438_153_143_036_9],
    [-0.708_944_396_163_000_2, -0.344_891_022_236_044_4, -0.242_540_578_995_648_13, -0.181_025_582_133_397_84, -0.136_228_799_679_876_1, -0.107_343_595_271_585_02, -0.073_404_764_362_762_8, -0.027_705_513_535_008_355, 0.003_606_109_836_443_945_5, 0.028_549_390_045_309_205, 0.059_047_262_890_692_27, 0.091_593_909_515_774_45, 0.125_359_432_172_687_47, 0.179_060_656_525_560_72, 0.208_782_990_855_399_11, 0.230_780_340_074_972_06, 0.320_361_121_893_641_1],
    [-0.708_944_396_163_000_2, -0.369_659_426_608_306, -0.256_872_949_912_312_13, -0.203_206_574_101_515, -0.156_141_566_468_736_1, -0.119_403_422_165_759_39, -0.084_190_238_123_581_49, -0.033_289_384_838_382_5, 0.000_179_393_783_454_024_66, 0.026_233_850_237_995_272, 0.058_232_553_280_362_23, 0.087_187_786_075_538_91, 0.121_340_204_825_499_8, 0.161_645_931_758_467_98, 0.198_075_665_338_432_52, 0.222_460_097_513_930_48, 0.320_361_121_893_641_1],
    [-0.699_772_604_827_577_9, -0.360_548_259_040_182_1, -0.254_419_082_120_638_8, -0.208_390_049_587_623_97, -0.168_350_237_620_346_5, -0.126_876_339_481_807_76, -0.087_647_151_727_609_98, -0.036_202_403_437_779_36, -0.001_660_707_786_077_411, 0.026_952_340_961_712_427, 0.053_467_517_574_877_15, 0.077_043_039_129_408_96, 0.099_838_782_127_420_4, 0.142_017_855_246_986_3, 0.179_413_349_317_063_76, 0.209_915_411_453_683_53, 0.320_361_121_893_641_1],
    [-0.576_313_393_154_596_8, -0.339_395_956_752_155_84, -0.249_829_200_858_140_14, -0.204_955_997_751_747_55, -0.168_341_001_830_104_97, -0.124_215_830_905_344_38, -0.081_984_188_422_601_59, -0.032_520_834_765_279_4, 0.000_246_466_033_319_005_74, 0.027_875_657_886_926_575, 0.050_583_666_642_797_92, 0.072_373_667_342_437_3, 0.095_126_693_943_584_24, 0.133_146_340_046_041_8, 0.164_086_155_673_619_3, 0.196_105_372_929_321_3, 0.320_361_121_893_641_1],
    [-0.576_313_393_154_596_8, -0.338_765_413_102_440_85, -0.260_794_034_325_873_5, -0.208_430_318_176_396_32, -0.168_442_131_540_160_52, -0.126_463_765_327_534_2, -0.080_374_027_751_848_02, -0.030_651_640_223_475_496, -0.001_566_422_866_833_702_5, 0.025_546_958_674_234_02, 0.045_377_408_142_599_33, 0.069_410_371_049_129_74, 0.095_778_283_895_623_11, 0.133_520_556_633_253_32, 0.160_055_708_491_588_3, 0.192_881_000_017_377_12, 0.286_456_618_571_283_57],
    [-0.507_776_542_754_278_6, -0.458_941_385_117_456_25, -0.191_723_291_934_709_72, -0.144_455_654_358_585_7, -0.075_479_246_949_306_42, -0.040_506_407_962_634_51, 0.004_091_104_820_625_795_5, 0.018_655_503_656_443_69, 0.032_933_110_259_982_63, 0.051_043_294_415_166, 0.114_844_359_547_115_28, 0.136_489_119_640_522_52, 0.142_634_317_765_133_08, 0.155_552_923_985_577_77, 0.170_248_598_948_899_02, 0.191_188_267_524_982_52, 1.268_059_362_837_156_1],
    [-0.660_487_017_067_465_9, -0.345_449_860_996_153_9, -0.231_812_435_510_969_03, -0.178_706_465_173_593_73, -0.148_182_771_987_192_1, -0.125_559_983_340_518_32, -0.073_382_847_429_745_23, -0.050_229_421_405_579_164, -0.006_366_649_497_756_313, 0.009_903_118_993_644_848, 0.030_993_386_251_790_25, 0.047_727_607_652_591_854, 0.096_299_084_147_246_74, 0.140_030_134_263_01, 0.163_936_208_403_009_88, 0.618_988_724_725_434_4, 1.268_059_362_837_156_1],
    [-1.206_912_284_702_842, -0.451_538_943_027_664_6, -0.308_115_412_516_962_4, -0.253_752_147_099_974_8, -0.206_810_503_490_874_7, -0.158_953_979_507_352_63, -0.127_279_612_100_682_4, -0.079_190_691_104_226_33, -0.052_078_072_858_446_02, -0.010_172_102_868_259_18, 0.019_613_256_338_167_923, 0.050_206_956_647_155_81, 0.141_745_916_674_459_4, 0.404_610_999_898_844_36, 0.561_798_785_478_627_4, 0.668_881_827_421_713_6, 1.268_059_362_837_156_1],
    [-1.273_417_855_720_798, -0.503_405_677_226_489_5, -0.346_505_087_490_962_7, -0.280_660_111_062_489_45, -0.232_607_294_002_358_16, -0.155_904_976_146_632_5, -0.095_449_641_769_168_98, -0.052_097_931_528_170_535, -0.004_990_670_075_809_944, 0.036_163_909_152_222_944, 0.072_825_224_930_535_89, 0.136_618_579_482_032_68, 0.230_125_264_670_178_7, 0.387_005_765_761_886_3, 0.499_986_750_317_046_9, 0.637_376_726_510_355_6, 1.268_059_362_837_156_1],
    [-1.285_301_100_270_818_6, -0.828_388_326_108_320_1, -0.349_327_481_825_817_2, -0.206_029_270_406_873_74, -0.094_136_660_983_528_55, -0.045_696_786_593_415_56, -0.008_468_083_510_078_467, 0.014_624_462_494_166_098, 0.033_129_165_286_817_69, 0.049_026_360_289_761_116, 0.076_104_547_261_071_58, 0.104_889_570_455_556_39, 0.156_911_312_325_697_16, 0.201_846_090_580_682_9, 0.292_114_887_263_486_83, 0.454_656_808_288_192_77, 1.058_256_822_100_844_4],
    [-1.285_301_100_270_818_6, -1.210_446_104_886_686_4, -0.242_847_660_618_321_1, -0.090_198_326_270_900_78, -0.053_471_810_293_884_13, -0.028_945_162_058_091_61, -0.010_376_005_044_107_099, 0.004_682_585_415_274_611, 0.017_268_976_486_701_282, 0.030_364_035_668_271_762, 0.043_135_425_537_922_92, 0.063_822_366_593_230_25, 0.097_234_203_495_055_9, 0.144_318_477_161_683_3, 0.201_519_915_026_394_08, 0.320_908_412_654_808_46, 1.058_105_974_427_741_2],
    [-1.285_301_100_270_818_6, -1.211_133_633_044_168_4, -0.107_933_323_667_342_5, -0.074_726_046_836_267_43, -0.056_587_048_365_837_66, -0.037_938_133_304_363_006, -0.023_158_712_033_965_103, -0.010_710_693_199_678_403, 0.001_589_931_494_938_266, 0.011_615_373_659_323_568, 0.022_191_204_393_320_868, 0.040_395_936_920_851_67, 0.084_941_041_193_175_86, 0.166_809_653_444_634_97, 0.274_166_417_375_876_6, 0.460_925_053_508_643_05, 0.897_449_107_237_503_9],
    [-1.285_301_100_270_818_6, -0.336_933_103_518_648_75, -0.091_860_650_602_349_96, -0.074_958_103_242_734_9, -0.060_774_708_942_965_196, -0.045_903_607_151_846_11, -0.033_997_356_166_622_354, -0.020_645_638_648_757_028, -0.009_783_860_696_447_676, 0.004_188_987_893_190_605, 0.019_079_912_982_905_272, 0.050_654_569_542_538_12, 0.184_200_961_322_794_98, 0.317_386_522_330_008, 0.438_911_951_278_100_6, 0.606_511_249_274_355_6, 1.026_038_361_488_906_8],
    [-1.280_417_235_759_364_8, -0.121_897_821_495_803_01, -0.098_096_732_336_619_82, -0.086_275_473_831_976_8, -0.074_888_026_740_191_04, -0.063_024_962_587_716_81, -0.048_309_278_292_640_51, -0.035_616_396_437_067_83, -0.017_941_151_265_203_188, 0.003_826_510_689_395_389_7, 0.069_846_513_440_491_81, 0.250_349_078_209_014_4, 0.336_498_056_225_373, 0.425_913_250_911_345_7, 0.548_244_223_060_340_7, 0.700_735_067_598_046_4, 1.041_121_430_680_52],
    [-1.280_417_235_759_364_8, -0.138_981_782_577_960_07, -0.114_892_050_526_991_1, -0.099_070_536_820_985_07, -0.089_220_855_384_074_63, -0.078_247_980_168_775_36, -0.065_552_722_558_143_2, -0.048_112_556_477_471_105, -0.024_651_351_973_472_763, 0.085_860_047_915_464_46, 0.293_818_680_460_326_3, 0.355_477_036_903_575, 0.412_586_115_168_429_5, 0.520_658_904_452_103_2, 0.647_230_071_589_990_5, 0.751_633_562_346_419_3, 1.041_121_430_680_52],
    [-0.553_080_028_433_078_6, -0.164_404_041_867_577_23, -0.144_755_610_088_213_28, -0.124_366_941_723_298_98, -0.110_061_447_942_976_56, -0.094_625_216_970_547_75, -0.080_574_060_798_739_2, -0.062_815_484_471_342_55, -0.033_209_942_678_639_755, 0.269_055_654_186_863_17, 0.355_059_018_838_017_4, 0.402_358_088_496_476_14, 0.495_849_855_351_142_8, 0.614_231_091_596_524_7, 0.709_697_939_219_467_6, 0.797_665_929_404_358_4, 1.041_121_430_680_52],
    [-0.792_589_116_239_240_9, -0.216_700_383_103_294_84, -0.173_990_491_345_055_45, -0.156_634_120_782_088_16, -0.144_678_605_689_876_67, -0.129_532_315_642_704_07, -0.108_658_763_462_953_96, -0.074_620_322_225_607_76, -0.047_094_625_821_249_014, 0.300_497_013_360_910_9, 0.403_981_433_313_226_83, 0.493_049_723_125_259_63, 0.587_652_612_061_937_5, 0.648_347_970_842_271_8, 0.741_949_029_722_910_4, 0.821_989_876_691_362_7, 1.041_121_430_680_52],
    [-0.895_141_952_257_573, -0.464_088_127_776_849_05, -0.377_838_811_277_903_8, -0.235_558_178_533_575_9, -0.194_469_344_472_532_78, -0.174_173_854_685_382_52, -0.148_876_378_410_450_73, -0.114_922_565_976_929_38, -0.040_734_808_112_349_7, 0.152_250_047_576_948_27, 0.404_303_036_800_915_84, 0.492_586_376_517_210_2, 0.582_251_566_628_372_1, 0.626_478_941_629_582_2, 0.712_888_489_330_378, 0.821_111_261_888_221, 0.975_620_706_325_839_7],
    [-1.026_218_840_301_411_6, -0.882_489_580_779_273_8, -0.822_866_334_733_483, -0.467_094_919_088_198_83, -0.413_439_651_091_753_8, -0.297_876_952_741_136_1, -0.208_479_634_075_830_23, -0.001_651_389_891_707_992_7, 0.059_876_182_435_409_31, 0.091_588_104_482_697_9, 0.144_365_318_826_398_33, 0.181_496_187_391_554_4, 0.235_026_079_582_146_02, 0.391_257_000_230_732_07, 0.522_252_170_762_856_5, 0.648_189_335_587_863_7, 0.975_620_706_325_839_7],
    [-1.285_301_100_270_818_6, -0.464_253_430_828_213_5, -0.178_157_008_499_203_6, -0.116_553_268_292_537_7, -0.089_218_709_976_712_15, -0.068_346_027_146_397_63, -0.045_411_314_794_198_65, -0.022_389_147_125_601_996, 0.000_930_028_235_976_871_7, 0.021_065_230_555_770_476, 0.051_704_157_584_994_234, 0.131_738_538_850_878_22, 0.227_512_906_566_634_28, 0.362_097_578_934_113_76, 0.493_003_448_258_641_3, 0.656_131_765_531_592_4, 1.268_059_362_837_156_1],
    [-1.285_301_100_270_818_6, -0.464_253_430_828_213_5, -0.178_157_008_499_203_6, -0.116_553_268_292_537_7, -0.089_218_709_976_712_15, -0.068_346_027_146_397_63, -0.045_411_314_794_198_65, -0.022_389_147_125_601_996, 0.000_930_028_235_976_871_7, 0.021_065_230_555_770_476, 0.051_704_157_584_994_234, 0.131_738_538_850_878_22, 0.227_512_906_566_634_28, 0.362_097_578_934_113_76, 0.493_003_448_258_641_3, 0.656_131_765_531_592_4, 1.268_059_362_837_156_1],
    [-1.285_301_100_270_818_6, -0.464_253_430_828_213_5, -0.178_157_008_499_203_6, -0.116_553_268_292_537_7, -0.089_218_709_976_712_15, -0.068_346_027_146_397_63, -0.045_411_314_794_198_65, -0.022_389_147_125_601_996, 0.000_930_028_235_976_871_7, 0.021_065_230_555_770_476, 0.051_704_157_584_994_234, 0.131_738_538_850_878_22, 0.227_512_906_566_634_28, 0.362_097_578_934_113_76, 0.493_003_448_258_641_3, 0.656_131_765_531_592_4, 1.268_059_362_837_156_1],
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
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.401_351_221_045_355_37, -0.268_977_572_506_143_24, -0.188_178_535_292_709_36, -0.151_112_799_809_979_5, -0.121_632_582_092_610_64, -0.100_747_425_646_683_99, -0.053_963_653_928_016_615, -0.037_490_796_183_126_875, -0.016_652_664_214_968_877, 0.005_594_519_932_985_053, 0.046_264_008_300_917_16, 0.057_498_395_027_399_486, 0.079_324_409_240_794_92, 0.116_077_638_816_913_55, 0.188_129_981_096_713_17, 0.424_531_879_679_137_6, 0.684_304_256_947_579_2],
    [-0.405_969_552_841_021, -0.286_054_368_156_453_9, -0.262_560_605_328_114_15, -0.184_495_354_497_623, -0.136_006_366_799_277_36, -0.114_055_916_201_113_22, -0.048_052_896_227_049_696, -0.015_109_997_449_284_561, 0.039_333_772_803_804_234, 0.052_087_447_257_770_82, 0.060_853_193_240_891_67, 0.086_820_914_105_496_82, 0.117_794_001_322_381_09, 0.148_131_480_946_397_94, 0.219_202_149_090_109_1, 0.369_491_265_066_736_46, 0.684_304_256_947_579_2],
    [-0.405_969_552_841_021, -0.287_108_645_448_461_5, -0.250_333_912_066_290_8, -0.167_445_689_231_621_4, -0.139_679_127_246_603_7, -0.115_050_455_902_671_6, -0.066_474_765_881_626_53, -0.016_111_648_597_920_504, 0.050_816_467_560_594_08, 0.061_063_440_629_407_36, 0.091_325_689_271_241_4, 0.109_916_790_738_130_45, 0.147_087_418_578_170_54, 0.200_981_468_415_793_3, 0.249_343_182_757_470_5, 0.317_813_055_783_726_6, 0.684_304_256_947_579_2],
    [-0.405_969_552_841_021, -0.272_239_394_317_379_7, -0.205_695_394_268_188_59, -0.165_979_675_874_431_9, -0.139_809_681_555_293_5, -0.119_149_908_766_974, -0.090_691_741_799_558_41, -0.065_563_628_679_421_23, -0.011_915_276_938_381_72, 0.058_911_099_750_742_144, 0.103_638_612_107_837_4, 0.117_424_789_730_460_46, 0.167_794_201_148_181_4, 0.232_477_122_996_046_23, 0.294_200_958_154_927_97, 0.345_535_356_883_885_26, 0.740_263_769_181_754_8],
    [-0.405_969_552_841_021, -0.244_455_943_002_419, -0.195_331_769_927_401_95, -0.167_920_610_303_079_48, -0.139_869_425_809_045_52, -0.121_983_493_445_082_65, -0.092_144_095_218_069_18, -0.070_899_518_603_301_7, -0.027_937_116_086_356_406, 0.032_017_562_337_896_76, 0.096_264_778_601_272_33, 0.147_849_546_821_262_06, 0.207_953_998_975_020_48, 0.266_517_217_478_614_8, 0.335_636_412_960_702_57, 0.427_215_321_050_114_06, 0.740_263_769_181_754_8],
    [-0.317_418_018_108_297_73, -0.249_537_773_362_714_85, -0.202_881_003_246_481_8, -0.168_643_247_237_069_78, -0.127_703_646_724_247_72, -0.111_268_372_019_081_49, -0.070_024_899_709_903_08, -0.046_073_952_611_966_64, -0.001_763_431_848_811_556_8, 0.026_053_930_051_883_36, 0.096_653_920_444_793_6, 0.147_629_959_395_788_8, 0.248_319_309_641_735_18, 0.338_537_115_784_277_9, 0.443_083_188_984_097_26, 0.566_320_150_456_216_9, 0.740_263_769_181_754_8],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.405_969_552_841_021, -0.273_198_594_120_140_97, -0.202_818_618_295_901_25, -0.156_113_045_778_643_04, -0.125_781_510_759_221_12, -0.105_705_319_164_207_72, -0.069_207_598_680_048_22, -0.035_517_404_300_040_877, 0.007_709_568_357_016_739, 0.052_311_815_166_897_71, 0.078_510_514_033_223_14, 0.110_351_445_512_671_02, 0.165_952_598_958_886_03, 0.259_973_974_888_670_13, 0.352_055_845_891_804_3, 0.550_341_723_080_892_7, 5.100_935_424_904_311],
    [-0.439_558_182_059_522, -0.128_989_207_769_728_16, -0.098_703_370_238_642_67, -0.086_797_882_487_722_69, -0.071_925_329_240_402_18, -0.044_450_876_974_324_906, -0.027_543_100_494_509_36, -0.009_671_083_401_751_83, -0.000_259_244_853_518_826_4, 0.012_707_714_861_469_382, 0.017_116_491_486_053_046, 0.031_726_502_726_058_75, 0.061_142_997_674_009_77, 0.082_625_144_570_735_4, 0.094_516_963_917_318_02, 0.114_895_203_153_178_22, 0.130_551_099_574_068_63],
    [-0.651_556_897_545_603_1, -0.293_872_193_782_172_3, -0.181_757_323_124_511_34, -0.114_754_133_089_454_05, -0.099_355_498_474_457_03, -0.087_718_328_673_903_38, -0.071_157_447_623_215_44, -0.036_214_951_559_475_34, -0.013_307_185_054_227_422, 7.805_704_117_693_88e-5, 0.014_411_060_129_623_107, 0.033_071_793_530_348_88, 0.066_473_828_823_129_45, 0.085_826_238_066_109_42, 0.110_213_309_589_032_28, 0.121_578_254_631_869_12, 0.130_551_099_574_068_63],
    [-0.651_556_897_545_603_1, -0.446_314_146_476_824_94, -0.322_159_531_830_632_3, -0.224_536_239_650_497_93, -0.140_157_792_823_928_8, -0.104_553_106_978_333_74, -0.087_991_540_990_428_48, -0.071_137_634_822_234_54, -0.044_576_440_348_422_335, -0.031_737_333_895_526_11, -0.013_285_715_648_050_494, 0.007_555_179_018_362_852_4, 0.015_899_530_295_437_82, 0.054_336_425_759_883_33, 0.083_926_048_815_027_28, 0.114_735_304_511_772_36, 0.130_551_099_574_068_63],
    [-0.651_556_897_545_603_1, -0.300_601_854_592_438_7, -0.202_590_319_384_270_02, -0.150_243_430_016_569_15, -0.127_565_887_999_947_53, -0.106_245_480_448_652_55, -0.090_489_868_063_912_04, -0.077_761_911_283_451_53, -0.070_469_318_965_975_9, -0.061_701_735_716_206_34, -0.050_404_416_536_075_61, -0.037_772_475_214_480_18, -0.027_693_132_932_910_244, -0.011_165_975_250_602_879, 0.010_808_692_341_851_932, 0.063_318_506_665_062_5, 0.254_499_343_787_979_9],
    [-0.651_556_897_545_603_1, -0.203_017_405_849_693_33, -0.151_495_959_841_045_2, -0.127_002_243_074_985_3, -0.104_765_772_291_955_63, -0.086_783_312_031_594_53, -0.072_360_761_318_209_05, -0.062_845_880_192_913_54, -0.051_384_161_638_873_3, -0.041_724_372_914_415_234, -0.019_685_108_676_112_395, -0.004_329_675_747_113_734_4, 0.008_111_745_798_705_557, 0.041_076_311_919_520_186, 0.096_118_978_849_279_63, 0.187_574_897_895_214_34, 0.254_499_343_787_979_9],
    [-0.319_392_644_947_382_2, -0.182_697_321_924_407_9, -0.130_755_498_271_801_5, -0.101_603_290_507_794_64, -0.076_621_499_437_551_41, -0.061_120_765_186_805_04, -0.048_594_997_417_170_84, -0.030_332_477_486_744_103, -0.013_909_830_363_754_995, -0.001_046_120_964_409_549_6, 0.009_093_251_501_021_141, 0.026_351_592_192_572_222, 0.049_027_257_347_585_285, 0.071_526_718_907_953_87, 0.107_887_832_096_040_11, 0.178_376_067_751_564_17, 0.303_358_163_154_571_26],
    [-0.462_076_052_963_160_34, -0.233_868_999_433_826_94, -0.143_291_049_022_893_14, -0.117_397_253_140_661_28, -0.090_436_831_683_984_96, -0.063_577_054_771_717_23, -0.044_600_372_294_647_125, -0.027_460_192_965_379_53, -0.011_051_608_452_323_435, 0.001_895_333_047_011_998_4, 0.017_418_471_188_712_162, 0.028_787_495_973_694_163, 0.042_742_818_284_984_135, 0.067_047_316_579_015_26, 0.100_966_175_047_099, 0.164_555_814_688_795_85, 0.303_358_163_154_571_26],
    [-0.462_076_052_963_160_34, -0.268_668_469_884_454_14, -0.214_308_637_319_504_86, -0.142_483_843_379_472_54, -0.116_340_449_218_307_97, -0.095_028_483_037_432_72, -0.067_092_587_875_656_99, -0.042_751_219_759_979_82, -0.026_863_586_760_912_75, -0.013_575_955_626_300_16, 0.002_956_635_805_482_696_6, 0.015_908_526_798_471_496, 0.023_465_746_188_327_41, 0.039_891_374_329_102_57, 0.060_250_028_233_064_12, 0.112_133_721_301_828_53, 0.303_358_163_154_571_26],
    [-0.462_076_052_963_160_34, -0.211_176_851_633_649_85, -0.134_679_619_923_318_42, -0.099_592_319_638_062, -0.077_070_436_755_815_51, -0.052_410_447_063_704_45, -0.038_564_988_288_546_27, -0.025_163_119_139_683_33, -0.012_167_612_767_469_952, -0.000_589_548_881_147_964, 0.011_454_662_505_317_856, 0.020_001_058_905_748_61, 0.029_614_793_192_082_822, 0.042_346_828_293_224_864, 0.058_658_940_968_019_98, 0.082_467_621_021_740_06, 0.303_358_163_154_571_26],
    [-0.462_076_052_963_160_34, -0.134_671_910_654_032_75, -0.088_733_788_221_150_26, -0.067_400_771_340_208_6, -0.049_394_295_144_290_966, -0.036_430_246_514_223_37, -0.023_566_690_595_854_544, -0.012_012_149_128_850_038, -0.001_268_256_899_877_985, 0.009_083_866_311_501_39, 0.019_791_526_660_956_61, 0.030_301_122_513_903_98, 0.041_240_733_454_269_96, 0.054_562_363_997_527_78, 0.073_275_154_402_555_2, 0.088_961_642_049_956_84, 0.177_311_864_341_226_92],
    [-0.388_860_488_139_333_05, -0.112_880_587_817_138_71, -0.076_806_130_910_884_19, -0.056_535_403_184_898_08, -0.041_286_535_726_931_646, -0.029_408_426_249_461_44, -0.016_555_838_249_220_394, -0.006_182_164_148_906_85, 0.005_385_735_661_318_182, 0.015_902_776_083_464_753, 0.028_551_536_910_405_49, 0.039_575_847_960_547_77, 0.051_529_836_234_915_09, 0.066_163_342_867_209_62, 0.084_257_885_877_088_92, 0.103_577_377_702_076_1, 0.189_783_755_790_409_18],
    [-0.388_860_488_139_333_05, -0.156_818_638_365_660_62, -0.094_144_289_202_533_32, -0.070_913_433_563_126_49, -0.048_301_460_900_080_47, -0.030_735_113_273_127_14, -0.015_712_619_836_021_63, -0.002_570_836_752_153_644_4, 0.011_456_470_039_533_167, 0.024_383_094_860_580_164, 0.035_265_407_411_267_505, 0.048_562_447_711_681_12, 0.061_907_262_358_575_47, 0.081_305_614_150_103_82, 0.099_354_139_378_776_8, 0.132_953_562_767_410_08, 0.357_071_520_727_089_14],
    [-0.388_860_488_139_333_05, -0.260_024_821_080_973_13, -0.190_262_983_473_512_1, -0.139_075_656_766_433_5, -0.098_588_087_607_371_88, -0.069_620_846_027_45, -0.037_024_634_466_305_95, -0.008_370_595_895_541_812, 0.011_379_974_511_236_858, 0.028_619_801_001_332_417, 0.039_927_987_067_119_73, 0.057_454_660_305_375_78, 0.081_437_925_798_761_54, 0.106_936_241_070_055_3, 0.138_710_446_542_963_74, 0.177_047_996_183_835_24, 0.499_549_118_671_697_93],
    [-0.388_860_488_139_333_05, -0.295_605_856_216_200_95, -0.235_623_062_591_320_35, -0.189_215_367_676_592_14, -0.150_496_415_698_495_62, -0.120_061_942_575_613_21, -0.087_264_730_142_554_36, -0.049_575_158_276_895_716, -0.001_002_419_201_339_097, 0.032_398_117_773_151_42, 0.059_672_774_022_599_35, 0.091_952_706_414_012_64, 0.125_548_942_747_899_52, 0.155_608_820_920_514_2, 0.196_520_020_045_587_77, 0.278_588_586_483_518_14, 0.608_286_362_338_787_4],
    [-0.386_427_769_599_863_8, -0.240_540_558_295_249_78, -0.183_962_638_091_283_85, -0.144_415_377_252_658_46, -0.124_482_803_995_705_55, -0.093_448_610_150_255_92, -0.074_702_711_704_861_9, -0.047_812_927_824_474_86, -0.010_361_406_448_125_136, 0.029_963_960_837_445_25, 0.076_254_640_485_229_29, 0.104_450_539_493_751_62, 0.127_283_271_919_811_54, 0.182_925_373_910_634_2, 0.264_944_533_840_373_9, 0.376_289_898_844_196_06, 0.742_969_326_089_614_2],
    [-0.324_698_867_531_646_1, -0.152_068_585_265_849_52, -0.129_792_528_036_108_4, -0.113_776_018_352_082_13, -0.086_328_450_234_372_84, -0.075_071_301_385_654_06, -0.053_502_025_398_281_17, -0.038_699_100_269_235_77, -0.027_303_324_943_550_33, 1.752_044_099_191_240_2e-5, 0.029_994_648_001_299_785, 0.059_701_778_602_553_28, 0.102_995_262_971_181_18, 0.119_279_578_278_863_01, 0.183_037_369_662_541_83, 0.394_360_811_160_542_3, 0.742_969_326_089_614_2],
    [-0.203_044_253_524_741_7, -0.138_771_192_181_228_4, -0.115_951_593_589_517_47, -0.092_062_744_817_880_02, -0.081_878_526_284_996_95, -0.062_714_118_571_623_12, -0.049_146_074_079_627_16, -0.040_311_334_276_628_78, -0.034_081_452_805_818_85, -0.013_011_252_028_266_8, 0.003_000_208_663_743_168, 0.021_073_294_332_390_857, 0.054_707_477_628_720_295, 0.103_346_948_275_718_5, 0.123_602_114_453_728_04, 0.234_636_679_922_662_63, 0.742_969_326_089_614_2],
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
    [-0.257_980_566_308_521_5, -0.168_692_742_951_053_55, -0.114_695_291_533_587_04, -0.093_560_068_036_568_81, -0.066_712_681_616_793_68, -0.021_567_439_558_403_016, -0.007_466_948_073_301_736, 0.004_815_135_421_423_534, 0.025_233_107_106_413_154, 0.049_526_461_039_963_79, 0.060_377_785_997_886_74, 0.085_994_946_803_318_86, 0.106_621_810_737_678_79, 0.126_469_134_408_228_63, 0.153_567_798_007_514_27, 0.169_748_347_845_128_66, 0.283_076_019_402_019_4],
    [-0.257_980_566_308_521_5, -0.153_859_113_199_844_05, -0.119_736_778_454_578_2, -0.098_125_188_407_511_61, -0.077_122_525_560_470_58, -0.057_348_716_199_954_144, -0.026_088_379_930_286_884, -0.011_528_152_422_252_57, 0.002_696_843_805_391_879_5, 0.017_821_502_976_037_39, 0.046_894_699_753_485_064, 0.059_738_739_467_823_04, 0.086_556_279_297_990_38, 0.106_920_731_229_340_33, 0.143_942_856_116_258_1, 0.161_889_646_918_059_8, 0.283_076_019_402_019_4],
    [-0.257_980_566_308_521_5, -0.132_034_177_857_728_78, -0.104_333_291_509_322_95, -0.091_147_033_393_102_93, -0.078_173_636_678_546_07, -0.067_688_690_656_374_95, -0.044_852_189_073_246_8, -0.034_655_071_568_091_39, -0.015_514_747_251_072_453, -0.001_870_226_593_934_418_6, 0.009_648_439_810_327_482, 0.025_035_181_607_719_652, 0.052_130_828_170_767_95, 0.080_519_400_916_895_38, 0.105_140_246_913_460_38, 0.147_381_919_011_439_88, 0.283_076_019_402_019_4],
    [-0.240_414_296_216_423_96, -0.102_335_618_227_917_1, -0.091_102_021_546_787_98, -0.078_208_764_762_507_4, -0.068_382_143_173_741_01, -0.058_993_654_628_455_69, -0.044_783_438_411_067_69, -0.039_485_888_315_660_855, -0.026_399_395_214_995_753, -0.014_495_987_481_902_402, -0.000_269_033_019_831_159_73, 0.013_231_072_271_543_525, 0.023_284_609_120_515_39, 0.045_520_003_975_662_054, 0.086_779_079_464_259_14, 0.108_594_808_474_010_65, 0.221_291_535_037_944_08],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.257_980_566_308_521_5, -0.134_263_169_705_758_2, -0.104_174_394_317_829_27, -0.089_271_087_726_028_6, -0.069_775_847_054_851_38, -0.044_790_220_643_627_44, -0.028_165_418_257_031_24, -0.013_233_150_674_061_877, -8.941_120_507_574_28e-5, 0.015_479_235_049_053_713, 0.044_748_498_989_171_28, 0.057_436_413_895_600_884, 0.086_918_368_215_931_17, 0.108_307_442_469_310_41, 0.147_383_490_203_082_22, 0.172_008_914_604_361_52, 0.936_858_677_359_593_7],
    [-0.285_608_117_672_971_7, -0.125_047_797_964_321_94, -0.088_463_501_068_343_97, -0.068_229_008_409_247_49, -0.053_024_956_252_053_064, -0.042_287_764_318_317_614, -0.031_338_483_659_560_9, -0.019_951_990_462_582_198, -0.005_776_398_482_576_008, 0.011_404_871_651_942_61, 0.027_814_890_235_815_813, 0.047_604_100_938_750_64, 0.064_251_348_595_350_03, 0.079_650_843_200_979_91, 0.103_116_215_108_142_5, 0.171_173_475_041_212_94, 0.325_676_844_884_152_6],
    [-0.258_717_064_288_283_67, -0.110_269_832_561_935_62, -0.050_957_820_688_275_26, -0.043_509_354_128_266_07, -0.030_330_520_266_320_604, -0.026_382_972_679_887_484, -0.019_603_652_640_804_943, -0.008_760_606_955_215_898, 0.001_567_046_896_678_082_4, 0.010_959_553_949_389_562, 0.016_450_514_809_509_768, 0.021_591_611_218_058_89, 0.031_249_555_465_188_52, 0.039_274_679_819_949_455, 0.047_310_877_694_242_38, 0.049_529_186_956_755_58, 0.051_071_449_765_774_99],
    [-0.258_717_064_288_283_67, -0.160_532_721_113_444, -0.099_872_077_428_451_32, -0.060_933_012_027_355_495, -0.034_553_469_082_460_25, -0.021_981_924_417_330_594, -0.011_336_500_278_246_755, 0.001_664_134_813_731_837, 0.013_172_249_672_520_7, 0.016_016_450_010_327_973, 0.022_103_528_009_330_55, 0.029_639_527_498_389_457, 0.034_075_801_966_356_1, 0.038_503_515_331_768_275, 0.043_218_773_380_567_45, 0.049_708_177_884_604_916, 0.072_406_360_016_635_14],
    [-0.258_717_064_288_283_67, -0.182_516_302_029_624_27, -0.129_248_287_808_503_64, -0.100_317_213_834_210_52, -0.068_618_890_967_319_84, -0.032_747_245_847_232_22, -0.012_405_490_279_002_113, -0.001_919_353_587_412_057_3, 0.013_128_963_839_402_274, 0.017_017_491_311_903_37, 0.026_726_671_583_848_068, 0.032_605_659_855_588_67, 0.035_331_650_765_582_99, 0.039_582_445_924_993_9, 0.050_069_226_492_841_554, 0.067_385_166_319_696_35, 0.137_100_374_347_262_47],
    [-0.258_717_064_288_283_67, -0.179_748_630_125_156_4, -0.140_646_431_727_261_02, -0.111_436_531_496_622_62, -0.093_152_539_918_050_71, -0.069_043_170_169_125_73, -0.034_991_672_747_843_35, -0.009_200_641_411_352_392, 0.013_263_220_358_590_464, 0.023_556_198_192_100_29, 0.032_283_996_164_238_52, 0.036_816_580_078_317_66, 0.048_165_443_276_557_515, 0.064_026_525_213_632_43, 0.077_461_311_385_149_33, 0.096_883_037_699_748_32, 0.296_992_653_527_643_3],
    [-0.247_925_711_831_986_8, -0.158_654_867_132_774_46, -0.135_446_411_915_035_7, -0.108_619_130_478_191_8, -0.098_199_277_705_808_44, -0.079_079_185_735_451_13, -0.058_204_772_907_605_77, -0.030_001_048_350_917_315, 0.000_690_418_198_575_236_5, 0.020_956_525_574_143_535, 0.033_601_151_549_074_774, 0.048_450_921_636_660_364, 0.071_289_834_063_494_7, 0.092_300_393_849_091_61, 0.104_093_454_454_368_45, 0.132_355_648_398_687_02, 0.316_827_462_168_331_9],
    [-0.247_925_711_831_986_8, -0.147_927_401_243_673_18, -0.130_474_170_613_132_4, -0.106_407_009_490_539_69, -0.092_835_167_952_782_59, -0.075_074_541_645_288_54, -0.058_351_350_448_533_38, -0.032_774_004_829_167_06, -0.013_291_866_331_678_098, 0.014_936_917_583_841_957, 0.027_132_867_511_838_472, 0.047_715_777_090_930_85, 0.071_578_883_771_567_68, 0.097_437_693_283_484_13, 0.124_032_903_673_992_8, 0.181_548_729_180_915_08, 0.325_676_844_884_152_6],
    [-0.169_525_801_768_174_83, -0.143_622_963_756_606_5, -0.116_824_533_661_758_3, -0.099_191_418_442_919_22, -0.083_490_585_947_898_14, -0.071_385_890_664_422_72, -0.052_983_411_135_746_7, -0.032_596_909_115_143_21, -0.015_776_717_318_196_982, 0.011_704_994_160_737_883, 0.026_416_581_594_949_11, 0.050_366_149_028_143_04, 0.069_591_784_040_164, 0.102_157_902_423_785_5, 0.134_630_007_136_157_16, 0.218_612_297_220_228_46, 0.325_676_844_884_152_6],
    [-0.177_109_833_885_906_34, -0.124_020_403_210_556_52, -0.102_395_383_294_502_83, -0.083_114_804_272_658_34, -0.069_982_755_972_210_97, -0.054_932_658_912_801_43, -0.044_090_228_260_958_333, -0.033_092_555_315_331_93, -0.020_673_663_268_744_334, -0.002_813_681_557_990_723, 0.023_490_472_738_539_186, 0.051_297_596_344_149_717, 0.070_905_566_453_172_54, 0.108_031_831_793_621_47, 0.185_391_398_737_568_07, 0.235_111_076_670_934_87, 0.325_676_844_884_152_6],
    [-0.177_109_833_885_906_34, -0.105_811_718_798_415_08, -0.080_002_642_070_099_16, -0.064_247_368_778_726_07, -0.052_313_431_531_602_145, -0.045_719_096_991_288_14, -0.039_651_602_563_726_67, -0.032_721_413_406_375_53, -0.021_400_648_679_968_076, -0.006_362_306_249_009_091, 0.020_477_836_362_008_246, 0.050_422_691_945_052_285, 0.075_040_563_942_454_22, 0.111_710_111_164_193_1, 0.203_318_940_733_323_68, 0.235_094_784_249_341_77, 0.325_676_844_884_152_6],
    [-0.186_789_024_958_168_51, -0.095_277_138_929_890_91, -0.070_499_129_749_991_17, -0.059_951_984_841_386_925, -0.050_839_433_526_327_39, -0.045_586_300_877_444_974, -0.038_287_078_088_219_26, -0.030_646_705_145_173_703, -0.020_471_864_163_320_305, -0.002_848_763_000_023_590_8, 0.019_320_723_802_198_472, 0.040_129_191_267_235_374, 0.067_630_195_182_794_23, 0.101_663_173_708_791_45, 0.177_462_281_190_203_02, 0.222_029_103_972_383_3, 0.323_686_963_251_929_9],
    [-0.200_604_479_473_945_22, -0.104_311_899_930_583_2, -0.077_288_099_709_173_05, -0.063_268_723_513_021_86, -0.055_053_634_716_287_94, -0.047_401_623_500_859_46, -0.035_396_098_313_022_53, -0.026_724_919_351_941_715, -0.014_966_074_614_422_687, 0.002_039_322_338_349_745_5, 0.019_687_761_976_061_237, 0.037_614_664_483_787_544, 0.059_050_090_192_025_84, 0.093_072_296_048_695_62, 0.143_271_566_900_406_04, 0.186_924_648_715_335_48, 0.288_964_876_253_956_6],
    [-0.245_385_966_311_010_95, -0.129_048_285_660_495_73, -0.085_389_846_186_097_5, -0.064_887_060_856_381_64, -0.052_664_523_858_117_006, -0.039_503_543_414_700_8, -0.026_570_037_229_613_01, -0.015_079_300_618_848_799, -0.000_995_830_287_286_960_3, 0.014_082_029_836_712_178, 0.031_318_093_280_456_6, 0.052_653_822_225_102_516, 0.074_558_025_575_620_9, 0.093_092_373_161_239_82, 0.117_175_379_107_545_54, 0.154_084_511_827_347_4, 0.261_507_935_018_235_77],
    [-0.245_385_966_311_010_95, -0.136_800_176_110_585_62, -0.084_687_167_529_504_84, -0.059_082_461_927_366_83, -0.041_038_864_941_543_62, -0.031_379_068_425_538_87, -0.017_360_107_351_846_094, -0.001_765_537_334_791_625_6, 0.016_748_687_762_329, 0.042_355_943_857_926_23, 0.056_709_557_616_744_906, 0.068_065_408_571_399_77, 0.075_941_094_657_804_92, 0.085_061_522_482_676_52, 0.096_876_235_277_966_65, 0.117_242_028_495_010_34, 0.216_906_836_076_064_3],
    [-0.245_385_966_311_010_95, -0.103_238_921_200_147_42, -0.070_604_301_239_192_63, -0.050_718_449_350_385_35, -0.039_538_665_782_507_97, -0.030_850_984_712_314_757, -0.019_393_943_068_208_026, -0.004_151_686_377_504_059, 0.015_940_505_350_615_725, 0.042_653_419_126_541_06, 0.053_646_314_468_126_56, 0.060_951_704_171_655_96, 0.068_686_453_584_599_77, 0.075_369_319_031_516_03, 0.081_466_706_206_347_55, 0.093_818_882_238_836_86, 0.170_215_148_110_828_16],
    [-0.245_385_966_311_010_95, -0.076_163_580_231_391_55, -0.056_010_154_674_411_804, -0.042_356_363_766_217_94, -0.033_536_821_773_641_534, -0.024_211_523_205_497_14, -0.018_096_584_206_403_938, -0.010_602_710_742_501_23, -0.001_207_040_871_442_949_6, 0.011_355_042_920_893_26, 0.027_325_368_000_955_03, 0.043_916_193_128_262_31, 0.056_778_760_326_025_184, 0.067_853_505_022_163_49, 0.075_182_191_136_818_68, 0.083_276_750_446_309_53, 0.118_289_611_623_056_73],
    [-0.141_095_582_478_527_6, -0.052_699_840_504_915_26, -0.039_633_722_103_589_196, -0.030_759_588_000_750_845, -0.023_381_158_036_326_64, -0.017_965_411_825_720_083, -0.013_564_304_841_397_293, -0.005_121_257_840_856_954, 0.000_192_909_350_340_995_56, 0.005_098_590_499_688_54, 0.011_445_195_590_999_581, 0.021_906_658_281_499_403, 0.027_970_403_227_601_876, 0.042_758_789_552_417_42, 0.067_442_305_005_743_14, 0.079_975_798_709_534_32, 0.099_161_741_533_476_9],
    [-0.459_397_448_946_083_14, -0.387_423_159_635_551_2, -0.377_303_955_022_758_1, -0.357_801_874_402_068_95, -0.336_358_474_343_993_6, -0.300_367_644_311_612_5, -0.264_498_471_678_715_7, -0.062_998_251_175_274_5, 0.023_578_706_718_975_84, 0.036_131_667_848_069_71, 0.060_295_836_128_331_43, 0.063_731_018_047_464_2, 0.087_056_614_215_913_67, 0.091_832_940_800_251_95, 0.097_470_261_404_654_15, 0.103_164_664_703_556_79, 0.141_480_045_115_474_6],
    [-0.459_397_448_946_083_14, -0.385_749_959_821_606_9, -0.370_045_137_980_675_84, -0.356_397_712_913_321_73, -0.341_930_419_540_719_4, -0.305_588_440_428_801_2, -0.254_960_936_777_526_96, -0.198_299_701_391_105_95, 0.002_323_869_718_469_164, 0.030_462_641_716_458_995, 0.061_352_488_202_129_52, 0.086_133_720_075_020_64, 0.097_365_980_017_620_8, 0.102_935_616_453_022_47, 0.119_483_156_460_835_96, 0.140_745_945_078_689_8, 0.259_039_626_911_117_54],
    [-0.459_397_448_946_083_14, -0.377_541_885_901_313_8, -0.349_573_922_348_118_9, -0.322_558_998_562_192_17, -0.253_739_645_711_789_64, -0.202_871_506_001_999_46, -0.126_736_731_659_838_43, -0.085_311_022_182_060_53, -0.003_961_090_400_731_605, 0.042_787_944_420_216_61, 0.097_436_064_733_008_32, 0.107_339_539_297_389_33, 0.120_388_481_686_659_94, 0.141_222_758_284_817_95, 0.159_103_956_919_839_64, 0.259_843_196_916_632_77, 0.358_861_150_734_431_3],
    [-0.459_397_448_946_083_14, -0.252_035_653_664_484_8, -0.166_582_963_832_309_3, -0.134_490_529_950_639_37, -0.113_216_729_973_393_69, -0.108_222_921_352_180_77, -0.083_433_326_374_457_91, -0.038_175_963_558_304_21, -0.014_939_729_291_807_348, 0.098_132_209_464_300_04, 0.139_251_203_156_928_65, 0.203_358_988_144_398_68, 0.268_412_208_046_211_3, 0.315_169_003_958_825_74, 0.334_601_606_176_639_3, 0.346_357_866_175_512_5, 0.358_861_150_734_431_3],
    [-0.381_750_023_005_531_35, -0.152_841_537_624_803_3, -0.133_140_866_122_157_84, -0.111_782_312_130_004_3, -0.107_432_593_792_163_24, -0.079_386_155_591_155_36, -0.045_180_950_562_788_535, -0.035_987_564_474_588_09, -0.012_442_641_891_940_522, 0.126_314_613_479_182_82, 0.274_368_930_188_686_6, 0.289_585_876_622_310_94, 0.300_961_663_193_900_94, 0.318_911_060_366_275_1, 0.330_406_451_053_699_5, 0.344_295_545_824_649_05, 0.367_785_987_507_208_4],
    [-0.210_275_309_913_286_47, -0.165_778_953_982_262_06, -0.149_956_333_372_727_9, -0.118_675_998_489_243_72, -0.106_691_217_881_545_72, -0.081_929_686_628_421_6, -0.047_686_940_308_154_305, -0.035_955_228_031_199_306, -0.006_557_493_963_456_62, 0.052_533_540_726_403_69, 0.258_557_727_930_586_57, 0.280_397_047_837_831_76, 0.294_158_207_112_078_13, 0.311_000_399_178_299_6, 0.321_920_102_916_385_7, 0.340_852_067_799_697_96, 0.381_565_767_985_836_7],
    [-0.609_893_041_055_413_3, -0.203_715_021_292_112_2, -0.174_661_833_612_909_7, -0.156_697_027_170_047_05, -0.128_997_188_935_603_4, -0.113_956_690_475_627_4, -0.092_364_684_212_930_16, -0.045_476_140_025_226_146, -0.005_831_993_170_520_274, 0.113_614_039_089_885_81, 0.194_320_930_739_271_15, 0.221_116_642_107_361_4, 0.244_311_033_641_741_48, 0.276_789_097_958_825_06, 0.295_355_959_895_918_3, 0.321_557_139_712_318_5, 0.381_565_767_985_836_7],
    [-0.711_377_142_356_211_9, -0.269_862_047_681_468_4, -0.220_799_814_164_349_16, -0.200_259_451_447_372_3, -0.174_026_348_245_407_64, -0.145_755_032_957_453_54, -0.119_679_249_293_374_62, -0.086_398_514_038_891_72, -0.005_415_088_084_390_664, 0.089_549_065_666_285_75, 0.128_633_072_755_318_43, 0.157_863_588_044_297_1, 0.188_043_560_419_668_5, 0.217_158_267_823_331_7, 0.242_381_069_417_157_04, 0.278_954_468_954_424_4, 0.381_565_767_985_836_7],
    [-0.711_377_142_356_211_9, -0.343_526_677_759_068_37, -0.276_937_272_958_147_96, -0.232_543_629_201_725_4, -0.201_983_476_213_817_82, -0.171_771_139_476_053_44, -0.137_568_476_343_009_92, -0.097_224_974_158_715_53, 0.000_628_168_465_224_724_8, 0.062_170_842_713_535_03, 0.095_477_852_960_292_29, 0.113_954_939_004_032_68, 0.135_526_632_741_174_44, 0.152_566_534_771_029_3, 0.182_289_527_635_618_5, 0.220_993_465_870_268_66, 0.381_565_767_985_836_7],
    [-0.711_377_142_356_211_9, -0.385_533_880_453_674, -0.285_735_040_094_141_7, -0.212_052_256_691_485_2, -0.171_080_228_497_427_52, -0.144_368_976_055_526_34, -0.106_292_994_473_82, -0.067_293_545_701_875_43, -0.013_022_903_015_880_905, 0.034_895_948_183_302_876, 0.068_102_603_096_616_92, 0.087_906_703_979_928_16, 0.109_817_970_501_494_68, 0.130_872_654_652_638_53, 0.147_090_908_297_704_83, 0.173_696_184_512_794_67, 0.374_400_454_803_746_6],
    [-0.711_377_142_356_211_9, -0.306_517_782_782_610_27, -0.188_094_940_824_796_72, -0.158_030_835_657_059_67, -0.133_216_366_766_239_05, -0.104_298_664_873_343_7, -0.074_996_587_964_083_82, -0.044_726_715_335_605_78, -0.008_644_526_651_376_343, 0.021_698_538_425_995_62, 0.050_928_238_361_259_16, 0.069_552_409_440_343_3, 0.082_793_680_120_990_49, 0.105_120_988_810_565_82, 0.128_341_480_899_237_44, 0.147_691_030_810_695_23, 0.253_641_299_772_071_6],
    [-0.584_811_502_771_087_5, -0.229_571_374_732_154_7, -0.171_354_132_085_495_58, -0.140_555_360_632_348_3, -0.110_871_060_661_408_24, -0.079_751_337_951_599_74, -0.050_954_395_698_541_544, -0.020_392_211_085_331_72, 0.006_537_820_689_992_847, 0.024_332_438_467_472_874, 0.048_116_172_769_264_65, 0.066_343_965_730_599_11, 0.079_379_948_669_130_52, 0.097_572_704_831_704_19, 0.112_511_726_006_503_46, 0.137_849_524_921_965_62, 0.251_671_349_259_211],
    [-0.558_291_313_933_344_4, -0.241_767_643_242_711_74, -0.182_754_398_186_135_23, -0.146_753_714_896_237_76, -0.110_157_868_608_326_21, -0.073_154_834_334_763_79, -0.045_503_378_206_162_5, -0.020_109_949_416_309_194, -0.000_147_591_813_738_726_88, 0.018_609_762_798_912_376, 0.039_883_379_334_015_58, 0.059_066_446_283_153_6, 0.076_049_354_829_180_35, 0.091_824_212_939_315_06, 0.111_880_538_322_112_26, 0.136_322_835_954_533_2, 0.257_365_217_731_037_5],
    [-0.484_949_100_432_005_45, -0.231_552_777_094_135_16, -0.188_519_351_203_105_95, -0.152_106_420_473_489_02, -0.107_524_899_672_497_83, -0.070_704_236_073_044_96, -0.045_643_557_687_016_79, -0.022_631_205_922_830_806, -0.004_476_668_289_028_516, 0.014_357_306_796_008_136, 0.032_848_570_576_577_15, 0.051_327_840_403_966_23, 0.069_516_664_642_282_9, 0.084_298_722_612_708_85, 0.104_890_982_014_722_7, 0.132_237_780_399_798_2, 0.257_365_217_731_037_5],
    [-0.388_258_686_613_260_4, -0.219_739_379_620_233_24, -0.184_673_188_970_163_66, -0.147_020_880_161_171_74, -0.097_614_016_094_659_83, -0.062_786_802_633_348_04, -0.037_302_535_698_055_87, -0.016_736_902_275_856_45, 0.001_397_302_922_293_501_1, 0.018_337_390_046_310_9, 0.035_395_635_090_138_05, 0.051_257_131_699_353_98, 0.067_511_168_614_489_14, 0.082_495_990_669_612_6, 0.099_918_240_933_797_56, 0.123_429_884_988_606_8, 0.257_365_217_731_037_5],
    [-0.321_035_383_661_449_57, -0.202_403_576_703_541_8, -0.167_034_234_842_885_65, -0.125_504_299_652_469_31, -0.083_173_854_180_029_64, -0.050_503_436_581_396_93, -0.026_240_114_529_685_33, -0.007_014_660_502_571_936, 0.011_049_253_261_088_859, 0.024_527_438_790_178_914, 0.037_829_419_351_844_84, 0.053_689_799_289_892_8, 0.070_013_934_442_010_22, 0.084_609_110_685_118_98, 0.101_330_634_167_675_43, 0.121_011_165_079_541_43, 0.257_365_217_731_037_5],
    [-0.237_241_025_972_254, -0.165_002_337_710_678_27, -0.127_417_870_725_530_04, -0.093_615_395_168_371_52, -0.066_294_942_175_334_73, -0.040_068_878_314_331_6, -0.019_878_854_198_047_746, -0.001_553_513_608_405_008, 0.014_909_676_056_749, 0.024_585_489_943_551_718, 0.038_074_383_426_804_23, 0.054_446_492_821_153_714, 0.072_657_101_633_015, 0.088_073_261_961_192_68, 0.112_173_275_462_374_49, 0.131_791_939_570_524_76, 0.231_205_473_640_758_44],
    [-0.338_240_002_043_587_4, -0.253_012_329_360_178_2, -0.225_186_044_276_044_6, -0.199_809_743_536_690_5, -0.166_375_673_952_512_02, -0.120_295_854_949_659_53, -0.045_143_912_291_356_31, -0.017_785_742_173_677_02, 0.014_126_551_571_873_099, 0.075_534_199_051_376_69, 0.126_027_092_079_219_9, 0.165_914_674_181_184_66, 0.216_048_842_847_520_96, 0.283_470_199_526_634_5, 0.384_101_323_627_396_9, 0.458_161_901_020_532_3, 0.609_845_165_222_182_6],
    [-0.355_771_096_311_835_7, -0.274_437_457_233_729_6, -0.231_624_063_258_723_78, -0.198_300_755_857_005_6, -0.173_219_625_819_540_56, -0.133_844_987_264_496_48, -0.080_300_963_456_721_95, -0.034_829_685_150_894_09, -8.183_036_848_623_113e-5, 0.059_667_060_041_405_24, 0.126_566_896_548_703_26, 0.168_111_560_665_342_36, 0.216_066_965_516_912_76, 0.304_459_536_014_624_3, 0.386_902_984_702_547_97, 0.482_776_309_402_974_45, 0.609_845_165_222_182_6],
    [-0.433_482_316_297_761, -0.305_523_013_519_164_83, -0.244_938_863_155_239_88, -0.209_386_471_771_167_76, -0.183_919_078_144_346, -0.150_512_442_032_085_89, -0.117_253_130_593_988_36, -0.056_126_089_628_732_785, -0.003_150_932_010_482_056_6, 0.058_433_746_131_675_09, 0.126_424_953_451_750_64, 0.179_337_262_492_321_9, 0.223_879_108_012_010_08, 0.308_588_658_186_828, 0.404_195_418_979_628_96, 0.527_275_981_683_554, 0.634_537_180_026_571_3],
    [-0.576_907_147_631_969_2, -0.319_481_223_135_485_55, -0.254_220_466_003_650_34, -0.227_319_909_488_033_3, -0.195_642_844_599_774_3, -0.157_749_226_504_717_9, -0.127_162_703_783_995_52, -0.066_076_420_630_728_6, -0.005_552_948_947_843_419, 0.052_800_034_367_190_91, 0.112_589_060_285_934_81, 0.184_973_341_681_816_34, 0.257_145_433_823_834_87, 0.330_503_558_747_385_4, 0.415_075_345_947_122_03, 0.539_868_285_676_964_1, 0.645_850_063_238_732_7],
    [-0.730_728_181_034_804_1, -0.361_782_920_326_405_36, -0.290_029_590_931_184_6, -0.233_593_754_249_988_8, -0.199_067_096_519_967_3, -0.153_881_360_748_398_3, -0.104_240_647_465_754_62, -0.057_690_630_732_274_06, -0.009_578_485_497_945_59, 0.051_447_347_295_641_446, 0.103_851_421_805_641_62, 0.175_128_178_432_242_88, 0.253_028_887_786_661, 0.330_072_865_136_895_9, 0.423_735_647_782_752_4, 0.540_195_261_120_983_9, 0.660_182_243_003_983_9],
    [-0.730_728_181_034_804_1, -0.387_914_398_687_520_8, -0.309_044_054_088_064_5, -0.234_131_315_556_433_74, -0.189_907_246_888_204_76, -0.139_260_950_039_259_3, -0.088_401_816_162_513_18, -0.045_432_011_586_340_81, 0.001_034_234_185_836_213_4, 0.053_186_327_877_383_546, 0.104_729_973_808_426_36, 0.155_244_248_310_560_13, 0.230_237_191_949_195_26, 0.300_430_409_287_793, 0.408_096_091_594_683_1, 0.486_126_969_715_182, 0.660_182_243_003_983_9],
    [-0.811_421_992_181_742_3, -0.397_685_705_981_707_66, -0.310_203_721_041_387_3, -0.241_979_681_514_548_93, -0.184_966_889_358_446_35, -0.126_603_091_680_961_37, -0.088_569_800_419_729_24, -0.043_250_109_747_637_97, 0.002_967_077_621_305_635, 0.051_102_434_729_635_57, 0.100_139_627_507_229_67, 0.155_283_513_524_786_86, 0.212_582_854_052_579_1, 0.280_612_642_791_784_16, 0.368_122_279_762_352_4, 0.474_060_636_963_961_7, 0.688_771_792_113_358_9],
    [-0.811_421_992_181_742_3, -0.404_747_834_224_153_2, -0.304_291_093_605_077_67, -0.240_027_242_079_954_82, -0.169_339_360_626_434_23, -0.116_073_004_021_585_8, -0.090_706_085_684_788_38, -0.052_377_403_524_020_524, -0.002_432_670_447_126_359, 0.038_256_063_403_388_06, 0.088_164_981_207_031_16, 0.140_515_894_404_012_33, 0.197_295_537_721_754_37, 0.257_639_512_500_871_7, 0.338_038_736_642_554_46, 0.456_832_717_715_159_6, 0.688_771_792_113_358_9],
    [-0.811_421_992_181_742_3, -0.403_596_130_747_464_1, -0.302_962_816_711_018_7, -0.228_177_978_004_097_53, -0.155_772_338_692_221_9, -0.114_844_520_990_983_19, -0.090_004_723_309_071_18, -0.052_271_397_271_496_54, -0.001_229_512_253_528_938_4, 0.032_347_426_357_138_755, 0.075_158_083_134_547_3, 0.131_423_530_522_315_9, 0.183_481_788_313_215_9, 0.238_806_447_264_116_57, 0.319_388_914_928_287_36, 0.435_092_365_482_008_6, 0.688_771_792_113_358_9],
    [-0.923_556_992_039_820_6, -0.402_373_993_126_357_7, -0.310_222_840_820_892_2, -0.225_957_259_946_715_37, -0.155_506_454_788_076_31, -0.114_983_111_648_774_16, -0.077_312_850_535_963_61, -0.033_401_483_916_501_06, 0.005_580_924_330_100_325, 0.035_640_214_818_989_73, 0.069_390_467_674_619_55, 0.125_129_622_859_709_1, 0.187_071_054_553_816_04, 0.241_552_880_946_786_26, 0.315_947_440_521_646_1, 0.458_580_088_478_983_24, 0.700_463_748_651_957_4],
    [-1.420_661_206_756_072_7, -0.430_141_654_799_322_76, -0.338_340_194_344_254_6, -0.258_623_621_964_756_86, -0.211_959_496_733_931_63, -0.129_328_997_168_656_26, -0.082_982_044_903_556_4, -0.035_855_477_846_025_385, -0.005_332_881_858_947_498_5, 0.029_287_058_007_792_25, 0.072_255_540_403_183_74, 0.119_283_513_492_669_34, 0.192_843_156_309_293_33, 0.241_600_971_043_560_35, 0.316_623_012_191_338_5, 0.489_266_206_761_334, 0.700_463_748_651_957_4],
    [-1.420_661_206_756_072_7, -0.528_125_682_878_591_5, -0.406_406_045_829_241, -0.324_982_220_210_467_8, -0.252_844_098_552_541_4, -0.178_709_984_371_552_92, -0.118_109_752_942_836_28, -0.063_424_194_468_739_9, -0.022_148_138_376_276_664, 0.020_251_988_274_840_18, 0.074_708_878_222_202_43, 0.119_368_231_555_459_53, 0.189_749_796_831_837_3, 0.245_668_460_855_863_58, 0.316_581_553_210_334_65, 0.444_750_067_896_722_6, 0.700_463_748_651_957_4],
    [-1.420_661_206_756_072_7, -0.602_005_137_655_002_7, -0.448_922_228_433_511_15, -0.357_921_434_302_463_04, -0.264_207_687_809_929_4, -0.183_896_027_992_733_44, -0.117_861_027_164_315_35, -0.067_238_950_590_984_73, -0.009_276_236_114_000_415, 0.038_426_733_281_567_93, 0.099_175_383_679_826_1, 0.154_820_729_695_486_52, 0.204_821_819_142_657_32, 0.265_720_057_983_899_14, 0.330_012_502_839_509_1, 0.388_163_428_786_615_5, 0.700_463_748_651_957_4],
    [-1.420_661_206_756_072_7, -0.669_796_124_644_972_5, -0.529_601_832_731_406_7, -0.404_628_506_369_601_35, -0.282_843_219_439_153_3, -0.186_447_485_806_464_2, -0.114_317_429_420_783_46, -0.040_037_507_202_466_995, 0.014_492_829_710_711_906, 0.075_340_974_255_572_24, 0.124_368_057_371_025_56, 0.166_495_994_261_098_65, 0.213_266_985_985_763_19, 0.265_744_176_874_116_17, 0.316_288_879_909_002, 0.369_206_465_635_847_2, 0.561_295_041_249_112],
    [-1.414_790_819_443_656, -0.691_182_209_024_419_8, -0.559_459_805_583_879_1, -0.413_030_306_845_160_9, -0.292_492_961_730_861_5, -0.189_439_307_002_087_2, -0.112_287_610_928_422_47, -0.040_324_165_319_107_62, 0.018_730_285_173_139_637, 0.075_235_501_906_287_87, 0.120_476_695_628_691_3, 0.149_433_336_898_088_66, 0.201_247_383_019_259_3, 0.248_442_340_846_908_78, 0.283_714_990_411_525_04, 0.333_245_341_617_472_43, 0.485_220_377_377_380_9],
    [-1.414_790_819_443_656, -0.693_003_775_664_324_4, -0.549_888_653_061_762_2, -0.392_209_735_221_706_93, -0.289_194_907_623_078_7, -0.187_178_511_093_371_4, -0.110_577_692_761_107, -0.052_204_510_391_769_58, 0.003_927_476_409_287_568, 0.064_596_719_168_765_32, 0.111_273_716_093_677_38, 0.139_487_520_981_079_06, 0.179_205_840_071_927_16, 0.231_625_603_359_948_72, 0.267_652_538_105_142, 0.310_659_678_508_502_1, 0.405_542_403_371_409_7],
    [-1.131_859_282_164_395_4, -0.673_389_044_954_647_2, -0.515_249_177_559_204_9, -0.392_315_015_607_990_2, -0.302_264_006_555_862_3, -0.206_439_136_748_044_4, -0.131_641_529_450_884_48, -0.066_514_661_905_716_48, -0.002_789_936_868_050_371_4, 0.042_345_053_462_386_17, 0.098_567_534_890_916_19, 0.132_336_163_286_296_74, 0.172_466_247_334_802_37, 0.205_293_914_307_525_03, 0.249_124_675_727_208_8, 0.300_544_982_058_423_56, 0.374_751_157_502_593_9],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_27: [(f64, f64); 11] = [
    (0.0, 1.0),
    (2.897_033_345_325_824, 5.409_641_297_827_761),
    (-10.612_308_208_024_835, -7.785_361_733_659_064),
    (-12.0, -8.5),
    (-10.051_689_937_405_882, -7.564_536_382_481_232),
    (-0.267_813_086_206_949_77, 1.735_421_131_171_683_8),
    (-1.0, 0.1),
    (1.733_341_836_166_564_7, 6.116_011_264_579_155),
    (0.547_372_775_430_907_5, 1.429_542_341_227_538_5),
    (-1.318_591_262_072_919_9, 0.175_961_329_560_217_65),
    (0.273_942_300_133_430_5, 0.895_308_866_987_209_6),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_27: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.350_347_484_700_79, 1.339_466_460_114_085_8, 1.255_296_325_384_692, 1.213_438_132_752_871_6, 1.238_804_946_844_74, 1.258_610_719_098_004_7, 1.234_279_021_193_822_2, 1.203_902_847_812_870_8, 1.176_567_283_286_163_3, 1.156_252_338_228_383_4, 1.136_921_197_124_732_6, 1.134_834_558_572_767_5, 1.132_526_641_077_571_4, 1.118_216_682_925_154, 1.109_876_773_035_399_1, 1.110_535_958_267_054_4, 1.108_284_118_818_984_5],
    [0.488_259_278_749_944_2, 0.413_015_047_063_262_9, 0.396_418_900_247_196_4, 0.529_720_896_260_026_7, 1.035_045_700_035_564_6, 1.016_133_030_599_924_5, 0.971_996_157_886_820_9, 0.940_495_923_922_419_3, 0.915_233_618_868_144_1, 0.882_111_658_594_334_8, 0.837_459_312_124_722_5, 0.783_199_703_686_033_2, 0.776_977_671_340_674_5, 1.217_394_178_371_170_1, 1.310_148_354_787_116, 1.277_511_049_835_092_3, 0.868_212_273_113_022_3],
    [0.0, -0.043_75, -0.087_5, -0.131_249_999_999_999_98, -0.175, -0.218_75, -0.262_499_999_999_999_96, -0.306_249_999_999_999_97, -0.35, -0.393_75, -0.437_5, -0.481_249_999_999_999_96, -0.524_999_999_999_999_9, -0.568_75, -0.612_499_999_999_999_9, -0.656_25, -0.7],
    [1.951_127_281_357_212_3, 1.958_048_269_549_322_9, 2.028_903_318_778_159_5, 2.112_607_873_611_612, 2.157_924_771_280_962_6, 2.045_744_675_589_932, 2.016_888_011_168_784, 2.040_327_730_360_165_7, 2.040_327_730_360_165_7, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8, 2.044_105_796_926_342_8],
    [-1.222_530_261_152_061_6, -1.234_475_835_551_472_4, -1.074_532_594_187_800_6, -0.906_124_878_431_611_6, -0.722_024_305_752_882_8, -0.624_481_153_262_451_6, -0.542_042_417_765_411_5, -0.292_802_456_025_545_7, -0.127_002_964_960_330_92, -0.084_699_912_383_928_99, -0.062_376_436_370_306_08, -0.040_791_725_327_668_71, -0.033_467_145_154_763_54, -0.098_458_074_732_631_14, -0.255_577_589_934_351_65, -0.343_986_612_055_977_5, -0.387_064_856_364_729_26],
    [0.0, -0.137_5, -0.275, -0.412_500_000_000_000_03, -0.55, -0.687_5, -0.825_000_000_000_000_1, -0.962_500_000_000_000_1, -1.1, -1.237_5, -1.375, -1.512_500_000_000_000_2, -1.650_000_000_000_000_1, -1.787_5, -1.925_000_000_000_000_3, -2.062_5, -2.2],
    [-0.448_444_177_711_024_3, -0.495_648_401_776_355_66, -0.568_127_157_173_622_7, -0.598_963_075_272_664_6, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326, -0.600_080_960_889_326],
    [0.313_152_836_889_990_6, 0.380_087_897_337_835_9, 0.446_357_177_374_539_67, 0.478_422_163_207_749, 0.481_146_737_863_983_43, 0.448_207_400_339_373_7, 0.431_929_185_204_438_85, 0.442_435_282_679_372_6, 0.471_153_704_076_061_2, 0.516_629_027_839_418_1, 0.560_537_460_030_413_9, 0.616_718_762_285_603_2, 0.684_251_490_177_415_4, 0.767_697_867_476_660_9, 0.815_076_512_399_952_3, 0.812_116_890_506_310_8, 0.812_302_044_035_869_7],
    [0.234_433_560_700_501_4, 0.186_353_167_266_156_97, 0.174_463_673_818_276_4, -0.017_187_767_481_784_88, -0.008_019_869_652_068_707, 0.029_579_208_106_954_52, 0.037_118_116_397_991_396, 0.069_718_231_280_658_06, 0.188_854_702_186_503_77, 0.187_199_839_882_621_65, 0.196_646_439_488_965_65, 0.223_831_185_198_768_5, 0.223_441_243_405_000_18, 0.222_589_310_841_832_28, 0.233_734_225_447_120_33, 0.248_906_344_503_733_72, 0.262_960_568_206_388_47],
    [9.521_958_464_693_942, 9.457_634_238_072_226, 9.401_360_453_001_942, 9.368_585_833_351_792, 9.360_642_881_220_814, 9.372_137_390_762_969, 9.375_755_666_487_617, 9.370_204_460_706_26, 9.373_898_426_221_76, 9.384_282_283_175_65, 9.377_331_045_474_447, 9.392_001_128_301_144, 9.475_466_812_247_449, 9.602_988_457_402_132, 9.666_845_176_031_17, 9.669_864_216_401_301, 9.659_978_109_621_475],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_27: [(f64, f64); 11] = [
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
pub const AWD_FAST_DONOR_KINDS_27: [f64; 3] = [
    0.991_841_305_305_274_9,
    0.0,
    0.008_158_694_694_725_053,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16 over all their
/// members, in [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_27: [[f64; 17]; 11] = [
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [-0.726_738_182_612_015_5, -0.331_773_621_348_832_37, -0.258_498_938_937_827_27, -0.210_810_793_694_697_32, -0.164_841_757_156_878_9, -0.106_336_871_459_298_18, -0.065_588_384_792_973_44, -0.031_838_656_414_959_51, -0.002_697_519_131_610_646_6, 0.030_716_123_853_268_683, 0.067_359_427_414_167_88, 0.103_535_057_557_165_13, 0.144_709_114_393_150_8, 0.222_263_018_143_755_18, 0.314_431_995_967_514_2, 0.401_216_361_377_139_56, 0.730_280_085_006_360_9],
    [-1.343_097_014_740_407_8, -0.389_130_830_440_948_1, -0.209_281_802_628_611_62, -0.107_472_394_223_106_66, -0.052_509_894_444_759_495, -0.027_268_849_794_172_58, -0.014_748_160_530_222_197, -0.006_840_079_300_771_604, 0.001_456_795_077_866_659_8, 0.011_941_023_843_031_04, 0.021_999_695_700_016_75, 0.038_104_010_006_734_29, 0.073_607_722_047_678_11, 0.134_619_046_725_924_53, 0.208_809_644_271_916_62, 0.381_254_338_782_496_3, 1.484_473_992_154_255],
    [-0.492_342_081_368_886_3, -0.021_835_691_425_734_96, -0.017_953_208_039_489_56, -0.014_643_052_031_393_109, -0.011_061_717_988_363_282, -0.008_517_171_827_344_018, -0.006_525_843_616_451_379, -0.004_683_597_599_473_965_5, -0.002_857_045_936_395_703, -0.000_830_458_477_713_455_4, 0.001_007_599_059_707_691_3, 0.002_768_688_692_257_747_7, 0.005_930_288_766_903_061, 0.011_150_510_550_934_349, 0.015_914_759_361_132_836, 0.021_029_147_984_204_83, 0.395_947_944_721_579_57],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-1.027_022_028_902_709_7, -0.149_332_375_211_798_85, -0.110_656_611_843_587_44, -0.085_428_837_104_381_46, -0.063_683_712_029_831_88, -0.042_509_242_543_903_876, -0.023_868_486_323_472_913, -0.006_958_608_732_992_671, 0.008_783_436_018_192_866, 0.027_203_611_102_789_85, 0.049_001_375_625_692_11, 0.096_401_063_017_869_38, 0.166_729_948_330_241_43, 0.335_475_691_519_383_8, 0.549_569_924_701_316_5, 0.693_002_388_664_704_3, 0.842_569_165_567_405_9],
    [-0.101_516_406_530_789_64, -0.017_310_644_019_847_003, -0.011_338_015_267_668_991, -0.006_125_296_433_708_744, -0.002_975_817_121_301_008_3, 0.000_376_467_084_149_508_1, 0.002_942_457_103_185_955, 0.004_478_590_681_884_457, 0.005_316_617_877_003_080_4, 0.005_943_527_820_627_752, 0.006_709_876_635_443_933, 0.007_369_929_840_312_012, 0.007_928_043_548_854_976, 0.008_645_317_671_290_344, 0.009_332_227_449_572_702, 0.010_306_360_538_891_95, 2.260_971_381_516_218_7],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.462_164_890_989_437_64, -0.125_713_836_289_330_2, -0.081_291_744_904_171_72, -0.057_361_274_798_199_36, -0.041_709_816_282_509_936, -0.029_267_400_112_543_453, -0.018_997_184_093_180_155, -0.008_864_393_716_770_875, 0.002_266_115_104_026_601, 0.014_057_370_552_579_888, 0.025_605_425_215_006_285, 0.036_634_926_186_292_33, 0.047_802_862_748_987_45, 0.060_161_441_113_290_276, 0.108_035_858_829_451_81, 0.212_470_961_416_160_43, 0.338_944_146_550_081_3],
    [-0.923_278_113_627_055_7, -0.681_521_225_229_763_2, -0.552_739_782_940_785_2, -0.427_963_928_414_472_1, -0.291_647_687_719_967_9, -0.169_112_043_867_558_88, -0.089_116_612_625_018_37, -0.038_004_871_212_685_525, 0.000_789_997_039_481_363_8, 0.031_687_194_811_789_93, 0.057_834_385_478_714_09, 0.085_231_544_046_821_97, 0.117_780_064_140_534_13, 0.172_234_296_205_308_86, 0.319_735_213_789_952_2, 0.597_187_779_932_018_4, 0.900_042_825_736_453],
    [-1.852_145_654_985_062, -0.436_962_255_398_685_3, -0.300_723_013_269_808_9, -0.211_004_319_847_112_13, -0.147_995_835_014_106_93, -0.100_240_596_705_708_88, -0.060_627_618_756_786_61, -0.028_203_928_360_859_9, 0.001_160_043_201_718_615_3, 0.027_698_581_592_217_667, 0.050_567_101_500_088_97, 0.071_525_696_263_204_18, 0.091_092_736_452_365, 0.110_048_598_968_874_76, 0.128_646_588_549_855_83, 0.154_360_038_891_101_03, 0.243_986_099_716_661_2],
];

/// The slow accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_SLOW_NODE_RESIDUALS_27: [[f64; 17]; 187] = [
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [5.419_185_498_579_54, 5.867_883_822_225_973, 6.141_935_713_939_622, 6.331_426_905_850_568, 6.503_882_585_682_367, 6.665_384_953_194_608, 6.785_408_104_422_506_5, 6.895_909_258_143_796, 6.997_414_840_128_176, 7.174_645_925_111_599, 7.573_127_898_927_857, 7.899_257_199_034_058, 8.177_796_478_044_282, 8.409_738_612_551_642, 8.703_528_220_270_327, 10.0, 10.0],
    [-0.720_768_481_612_001_1, -0.345_507_372_385_503_55, -0.259_897_729_463_658_5, -0.198_615_143_897_255_38, -0.145_014_929_324_086_83, -0.096_801_051_474_357_69, -0.059_630_836_193_090_7, -0.025_111_531_840_070_776, 0.004_944_697_744_621_722, 0.031_891_160_064_317_39, 0.052_420_533_132_124_84, 0.071_783_596_761_163_57, 0.088_124_073_473_945_16, 0.106_547_577_256_034_25, 0.132_770_892_718_944_16, 0.174_103_694_996_747_33, 0.313_442_755_975_516_63],
    [-0.720_768_481_612_001_1, -0.322_847_242_286_606_94, -0.220_767_211_429_527_35, -0.145_517_761_721_599_58, -0.093_807_245_513_756_78, -0.062_463_612_037_558_985, -0.037_123_233_961_725, -0.010_149_405_449_224_354, 0.013_480_746_507_573_983, 0.036_297_768_392_518_24, 0.056_739_977_057_501_62, 0.074_925_750_251_187_6, 0.091_174_387_083_052_56, 0.107_075_477_060_440_9, 0.129_700_681_573_522_38, 0.163_036_867_949_405_5, 0.370_454_997_038_199_84],
    [-0.706_210_513_456_634_5, -0.316_862_731_992_737_3, -0.199_009_485_852_170_4, -0.137_469_904_169_329_28, -0.079_643_299_834_145_66, -0.046_651_358_928_796_12, -0.023_365_436_459_747_75, -0.004_227_012_214_714_227, 0.016_610_201_850_303_458, 0.035_969_445_075_597, 0.056_567_756_257_931_93, 0.077_168_357_654_447_34, 0.093_885_871_674_006_92, 0.109_115_455_464_620_55, 0.128_995_369_143_957_6, 0.157_663_938_350_586_15, 0.370_454_997_038_199_84],
    [-0.706_210_513_456_634_5, -0.381_284_972_534_515_4, -0.281_610_169_638_891_13, -0.207_926_014_506_729_15, -0.158_705_159_152_635_53, -0.081_386_863_877_661_3, -0.029_131_622_313_252_81, -0.007_101_004_933_252_974, 0.012_933_107_840_274_601, 0.032_757_114_503_352_336, 0.054_993_079_251_630_855, 0.080_961_084_031_318_85, 0.101_454_122_406_032_95, 0.120_252_884_070_898_71, 0.144_351_091_725_878_2, 0.198_432_902_916_543_86, 0.697_986_679_543_248_8],
    [-0.706_210_513_456_634_5, -0.375_053_501_015_675_64, -0.302_921_271_792_307_3, -0.256_794_215_712_917_7, -0.217_359_450_899_782_98, -0.178_245_320_592_657_02, -0.120_986_749_470_104_39, -0.079_787_995_990_534_16, -0.015_257_493_174_657_908, 0.017_174_201_867_239_93, 0.051_235_130_032_969_325, 0.094_017_533_009_831_25, 0.127_298_112_585_494_53, 0.176_215_610_897_580_12, 0.262_112_988_538_960_26, 0.393_890_539_375_359, 0.730_050_774_760_984_8],
    [-0.703_386_392_362_248_2, -0.325_800_970_762_710_7, -0.274_532_853_038_719_44, -0.248_383_686_051_633_72, -0.224_802_956_517_581_53, -0.189_778_990_630_155_4, -0.136_326_016_789_278_66, -0.101_075_672_556_970_81, -0.083_173_787_650_527_16, -0.033_647_991_687_461_24, 0.062_598_477_808_809_9, 0.162_990_308_753_808_5, 0.267_825_386_843_877_97, 0.372_227_291_958_927_23, 0.418_097_818_184_946_9, 0.477_992_671_080_445_85, 0.730_050_774_760_984_8],
    [-0.623_072_413_149_781_6, -0.313_744_441_133_226_2, -0.270_953_073_875_012_2, -0.249_418_577_998_762_38, -0.229_661_996_774_571_94, -0.192_987_661_273_735_04, -0.125_116_248_078_438_27, -0.094_894_537_517_639_46, -0.066_628_857_138_231_73, 0.085_132_463_182_416_23, 0.215_182_066_272_733_95, 0.307_741_052_275_158_8, 0.379_642_457_365_013_56, 0.411_292_169_809_156_2, 0.451_479_142_852_182_77, 0.504_275_005_905_739_8, 0.730_050_774_760_984_8],
    [-0.623_072_413_149_781_6, -0.432_100_297_642_321_16, -0.348_928_835_982_431_5, -0.281_562_973_468_341_7, -0.248_792_224_780_537_36, -0.201_785_531_968_158_43, -0.109_932_956_704_826_24, -0.020_441_837_021_655_966, 0.096_984_751_148_277_01, 0.155_323_302_503_820_18, 0.203_938_676_461_977_1, 0.247_583_247_504_350_35, 0.295_357_907_010_351_3, 0.349_841_489_059_467_16, 0.400_656_954_312_505_4, 0.460_573_703_261_128_23, 0.730_050_774_760_984_8],
    [-0.623_072_413_149_781_6, -0.404_646_689_630_438_76, -0.322_271_840_029_981_05, -0.257_618_857_960_188_35, -0.204_617_054_004_062_6, -0.156_343_942_320_413_43, -0.083_833_073_894_943_17, -0.038_872_917_063_595_615, 0.045_889_348_886_233_076, 0.117_944_494_545_388_51, 0.178_172_635_878_851_5, 0.220_832_598_028_190_3, 0.261_350_603_544_418_25, 0.301_734_007_450_660_5, 0.342_877_958_650_295_86, 0.396_887_830_065_929_2, 0.707_041_704_934_283_9],
    [-0.623_072_413_149_781_6, -0.271_306_167_214_397_1, -0.217_064_116_666_976_4, -0.181_733_858_785_635_44, -0.136_734_542_827_844_97, -0.075_344_946_166_047_74, -0.053_532_392_687_689_44, -0.032_880_549_662_718_04, -0.002_208_227_944_952_732, 0.043_225_831_405_488_524, 0.095_110_609_841_776_6, 0.175_575_460_331_646_92, 0.252_243_608_870_558_13, 0.305_768_367_160_322_35, 0.355_088_648_786_231_64, 0.412_687_119_539_100_4, 0.730_280_085_006_360_9],
    [-0.451_420_601_471_226, -0.204_018_716_023_979_58, -0.174_355_455_282_946_21, -0.138_688_290_815_069_36, -0.064_962_549_956_886_81, -0.047_590_778_564_008_84, -0.035_851_731_959_334_34, -0.022_905_223_145_746_04, -0.006_147_572_437_986_665, 0.019_503_668_463_971_148, 0.046_109_136_389_852_964, 0.075_488_770_634_029_98, 0.110_274_820_426_606_6, 0.197_317_852_796_536_88, 0.301_742_188_186_877_54, 0.383_627_486_749_088_86, 0.730_280_085_006_360_9],
    [-0.289_577_876_904_062_67, -0.181_667_302_798_384_36, -0.153_662_295_019_033_64, -0.079_450_844_202_294_56, -0.045_546_320_269_453_44, -0.036_119_705_138_794_24, -0.026_309_310_939_103_648, -0.016_342_287_430_643_04, -0.004_439_825_169_359_616, 0.012_700_261_266_197_988, 0.033_008_216_895_466_916, 0.054_235_038_614_856_67, 0.075_633_255_504_103_74, 0.102_160_353_385_397_8, 0.135_492_786_682_964_1, 0.283_178_608_796_594_95, 0.730_280_085_006_360_9],
    [-0.236_334_161_408_814_47, -0.166_499_545_423_590_1, -0.140_363_253_423_995_9, -0.048_902_170_194_903_546, -0.037_780_727_552_049_82, -0.030_031_229_427_315_627, -0.021_942_113_610_642_442, -0.012_554_936_343_094_899, -0.001_084_847_016_939_327_2, 0.011_837_686_431_678_07, 0.027_410_841_536_403_14, 0.045_089_134_484_655_645, 0.062_339_540_233_870_14, 0.082_899_158_841_086_58, 0.108_582_497_882_150_66, 0.140_175_954_300_837_57, 0.730_280_085_006_360_9],
    [-0.232_296_127_035_794_22, -0.150_522_039_820_615_52, -0.127_743_569_913_514_15, -0.041_552_125_350_369_68, -0.031_493_142_719_272_844, -0.024_592_199_106_994_617, -0.016_213_188_644_946_9, -0.005_051_961_126_115_238, 0.002_468_262_773_411_378_5, 0.012_766_775_806_391_243, 0.025_656_858_786_533_22, 0.043_733_992_366_135_35, 0.059_720_435_466_186_42, 0.075_510_433_986_954_19, 0.096_666_977_823_681_11, 0.139_063_567_209_593_62, 0.654_007_903_512_477_7],
    [-0.726_738_182_612_015_5, -0.331_773_621_348_832_37, -0.258_498_938_937_827_27, -0.210_810_793_694_697_32, -0.164_841_757_156_878_9, -0.106_336_871_459_298_18, -0.065_588_384_792_973_44, -0.031_838_656_414_959_51, -0.002_697_519_131_610_646_6, 0.030_716_123_853_268_683, 0.067_359_427_414_167_88, 0.103_535_057_557_165_13, 0.144_709_114_393_150_8, 0.222_263_018_143_755_18, 0.314_431_995_967_514_2, 0.401_216_361_377_139_56, 0.730_280_085_006_360_9],
    [-0.726_738_182_612_015_5, -0.298_012_392_339_570_6, -0.241_637_153_049_431_42, -0.180_489_708_548_981_2, -0.128_203_750_234_558_02, -0.085_026_668_998_540_43, -0.051_777_808_425_398_54, -0.025_056_684_589_884_03, -0.000_127_964_476_827_437_15, 0.031_995_124_050_259_165, 0.078_847_869_162_191_73, 0.140_390_449_362_497_2, 0.214_465_889_690_539_67, 0.275_182_284_908_363_66, 0.321_083_212_063_932_26, 0.358_765_902_951_680_76, 0.593_147_532_246_412_6],
    [-0.726_738_182_612_015_5, -0.297_836_508_666_541_9, -0.241_328_778_611_125_53, -0.179_402_372_199_843_84, -0.127_427_056_929_835_94, -0.084_201_939_325_210_94, -0.051_356_701_097_918_9, -0.024_808_079_098_743_163, 0.000_162_282_947_113_112_5, 0.032_413_896_740_990_91, 0.079_270_307_562_618_09, 0.141_038_640_023_949_25, 0.214_523_195_366_230_7, 0.275_302_234_976_627_54, 0.321_081_629_576_518_1, 0.358_488_075_935_289_03, 0.389_854_130_793_781_95],
    [-0.176_209_916_969_264_58, -0.010_617_439_922_291_402, -0.004_405_168_396_892_292, -0.001_459_359_634_937_807, 0.001_100_760_401_514_577_8, 0.003_346_574_744_013_601_3, 0.005_788_863_584_531_624_5, 0.009_442_450_993_184_309, 0.014_353_791_678_214_671, 0.020_276_475_463_428_957, 0.024_261_172_510_125_58, 0.027_302_595_124_715_68, 0.030_704_258_285_258_496, 0.033_631_806_411_527_31, 0.037_025_996_890_983_584, 0.044_391_610_991_834_515, 1.240_263_047_578_083],
    [-0.176_209_916_969_264_58, -0.017_666_251_826_057_16, -0.010_396_056_411_391_585, -0.006_807_523_262_529_734, -0.003_941_756_625_388_325, -0.001_347_584_715_739_299_4, 0.001_202_679_634_572_162, 0.003_849_187_730_878_409_5, 0.006_787_229_514_071_731, 0.011_761_001_122_361_808, 0.016_974_178_111_763_886, 0.020_681_951_923_013_79, 0.024_192_809_143_808_537, 0.027_195_405_867_855_058, 0.030_934_307_871_562_046, 0.035_552_876_020_865_286, 1.313_649_743_725_031_2],
    [-0.176_209_916_969_264_58, -0.023_692_372_724_481_395, -0.015_285_753_916_352_006, -0.011_682_090_009_222_275, -0.008_940_644_699_958_818, -0.006_615_995_866_979_007, -0.004_024_526_555_447_824_5, -0.001_349_174_891_162_833_5, 0.001_652_172_900_939_052_2, 0.005_377_525_949_943_262, 0.010_571_638_580_365_052, 0.015_456_935_611_602_97, 0.018_455_951_204_256_554, 0.021_811_408_614_881_23, 0.025_004_388_922_617_86, 0.029_859_130_501_178_65, 1.313_649_743_725_031_2],
    [-0.176_209_916_969_264_58, -0.027_510_171_644_222_196, -0.020_811_470_366_693_816, -0.015_496_757_135_247_816, -0.013_054_683_116_411_03, -0.010_888_003_702_370_846, -0.008_590_655_689_426_204, -0.006_092_001_815_503_95, -0.003_268_383_413_070_505, 0.000_247_113_077_206_350_7, 0.005_505_081_293_511_516, 0.010_981_956_829_647_926, 0.014_900_541_942_916_451, 0.017_620_116_632_472_52, 0.020_849_933_772_131_89, 0.024_776_605_674_064_957, 1.328_677_793_771_978_5],
    [-0.135_724_716_471_075_6, -0.031_637_658_087_506_425, -0.025_847_746_651_038_075, -0.019_779_921_447_288_407, -0.016_464_721_843_885_17, -0.014_100_039_154_198_449, -0.011_870_469_363_564_427, -0.009_417_569_100_215_682, -0.006_198_823_168_871_989, -0.002_866_811_112_031_366_7, 0.001_890_517_009_399_115_4, 0.007_797_330_829_019_268, 0.011_782_690_204_094_712, 0.014_855_470_851_013_6, 0.017_956_688_870_056_976, 0.022_043_419_925_747_286, 1.484_473_992_154_255],
    [-0.135_724_716_471_075_6, -0.039_125_179_550_483_11, -0.032_050_462_502_510_51, -0.027_129_868_732_384_918, -0.021_706_240_900_805_394, -0.017_978_278_348_118_566, -0.015_548_945_903_492_496, -0.012_513_428_571_266_135, -0.008_985_191_366_778_974, -0.005_081_735_640_877_24, -0.000_778_889_812_243_498_2, 0.005_030_117_609_249_331, 0.009_272_374_990_766_014, 0.013_459_149_346_187_905, 0.017_692_056_157_026_54, 0.208_787_839_944_306_47, 1.484_473_992_154_255],
    [-0.524_377_420_094_197_9, -0.082_306_947_447_193, -0.052_191_069_225_643_85, -0.040_744_117_126_652_85, -0.033_804_908_294_864_745, -0.027_914_439_040_609_956, -0.022_222_592_078_541_34, -0.017_459_333_166_028_74, -0.011_869_810_071_898_343, -0.005_775_446_197_592_219_5, 0.001_385_046_492_447_456, 0.008_282_847_102_960_553, 0.018_613_595_650_976_977, 0.430_438_185_395_599_97, 0.653_076_459_409_447, 0.736_577_918_099_172_5, 1.484_473_992_154_255],
    [-0.541_514_047_082_893, -0.439_075_250_524_931_8, -0.258_177_308_046_149_86, -0.137_926_551_249_825_59, -0.065_410_790_525_444_93, -0.048_286_760_656_832_16, -0.034_313_787_335_506_794, -0.022_764_387_542_587_944, -0.006_651_210_672_490_079, 0.039_371_915_084_490_554, 0.152_440_110_528_349_4, 0.240_514_220_916_013_88, 0.369_578_169_110_736_65, 0.524_654_867_987_388_3, 0.640_748_341_044_214_2, 0.719_963_598_152_444_8, 1.484_473_992_154_255],
    [-0.541_514_047_082_893, -0.497_796_363_236_567_56, -0.472_332_833_263_664_26, -0.366_449_183_994_992_43, -0.190_806_965_680_588_89, -0.084_887_005_447_521_85, -0.044_030_094_765_033_785, 0.002_171_602_309_847_325, 0.051_988_142_909_872_79, 0.093_164_892_542_129_58, 0.130_889_942_402_583_28, 0.165_410_746_816_219_42, 0.201_903_811_968_739_08, 0.244_744_154_910_387_04, 0.357_605_940_123_516_7, 0.567_465_816_601_728_3, 1.132_862_480_527_045_3],
    [-0.541_514_047_082_893, -0.487_729_632_747_083_03, -0.443_838_545_158_904_75, -0.311_164_849_940_897_5, -0.136_580_524_879_964_86, -0.079_570_534_326_601_17, -0.029_727_622_797_274_197, 0.015_156_877_087_084_861, 0.049_557_383_587_322_944, 0.082_656_642_092_263_93, 0.108_633_038_355_313_39, 0.137_795_310_586_064_42, 0.164_153_350_865_085_92, 0.192_924_756_647_323_45, 0.222_360_270_731_258_27, 0.262_252_180_708_849_04, 1.316_531_914_318_746_8],
    [-0.541_514_047_082_893, -0.392_473_170_441_055_06, -0.284_391_923_990_746_75, -0.183_388_172_950_157_05, -0.121_591_374_906_446_54, -0.089_025_224_012_465_8, -0.056_620_746_180_669_36, -0.025_104_159_612_554_484, 0.006_464_674_699_657_376, 0.039_941_053_671_492_82, 0.076_127_167_159_650_5, 0.111_649_150_877_703_76, 0.146_107_005_647_096_28, 0.183_952_648_181_717_3, 0.219_551_383_970_933_77, 0.254_559_474_649_877_05, 1.403_249_998_390_592_4],
    [-0.534_097_349_868_842_3, -0.343_815_896_495_248_35, -0.237_695_204_311_113, -0.178_827_498_171_183_43, -0.128_783_022_257_597_22, -0.096_084_734_704_949_45, -0.067_694_738_517_498_78, -0.040_786_752_651_606_24, -0.014_938_175_358_146_84, 0.008_525_888_211_559_418, 0.033_565_585_735_138_26, 0.060_218_241_374_989_49, 0.096_456_767_054_870_15, 0.146_387_246_136_139_4, 0.201_073_496_853_853_84, 0.248_403_663_451_113_6, 1.441_745_845_020_250_3],
    [-0.587_526_645_910_623_5, -0.368_448_635_484_524_7, -0.267_130_301_620_562_3, -0.192_566_734_754_226_78, -0.135_072_508_496_632_55, -0.090_350_247_047_177_01, -0.056_685_471_305_521_165, -0.025_802_478_860_238_138, -0.001_379_445_501_505_822_3, 0.019_751_114_440_037_222, 0.038_444_039_132_108_095, 0.056_159_523_776_073_965, 0.072_447_150_197_316_68, 0.104_079_125_840_440_7, 0.169_051_313_277_636_12, 0.241_195_803_656_676_67, 1.470_939_886_252_581_3],
    [-1.235_470_057_982_246_6, -0.456_605_557_293_942_9, -0.379_752_876_922_831_94, -0.260_839_150_070_297_84, -0.169_716_341_651_622_42, -0.081_848_294_293_122_51, -0.028_880_396_123_744_018, 0.002_712_124_554_382_068, 0.024_295_214_659_047_187, 0.042_370_139_657_492_37, 0.056_597_019_679_293_675, 0.067_631_008_844_133_4, 0.082_222_507_393_410_42, 0.123_607_572_459_410_95, 0.224_911_424_290_396_87, 0.752_985_935_258_510_4, 1.470_939_886_252_581_3],
    [-1.334_709_146_638_762_8, -0.743_655_159_240_029_1, -0.533_820_995_275_51, -0.462_915_653_813_215_26, -0.381_748_866_262_063_45, -0.193_632_033_797_910_89, -0.015_694_707_951_804_977, 0.024_627_868_550_152_78, 0.049_492_689_773_578_36, 0.064_847_239_492_499_03, 0.082_978_093_781_794_95, 0.143_800_124_998_649, 0.394_165_590_095_416_24, 0.646_470_865_166_489_3, 0.746_534_054_818_396_6, 0.955_390_667_299_029, 1.470_939_886_252_581_3],
    [-1.343_097_014_740_407_8, -1.197_921_558_695_464, -0.915_216_362_974_201_6, -0.642_505_013_605_159, -0.502_324_451_283_536_1, -0.151_619_846_013_573_87, -0.020_895_431_953_627_06, 0.014_389_558_211_340_182, 0.045_253_616_164_760_95, 0.080_400_416_071_836_11, 0.140_365_315_544_418_22, 0.239_334_054_072_674_6, 0.354_971_136_980_019_5, 0.571_736_029_981_120_7, 0.699_918_225_855_831_5, 0.832_307_982_487_557_1, 1.470_939_886_252_581_3],
    [-1.343_097_014_740_407_8, -1.256_668_307_552_946_1, -0.805_700_045_463_189_4, -0.195_588_895_116_347_36, -0.080_104_944_525_652_49, -0.043_905_539_419_340_19, -0.018_870_201_706_445_367, -0.002_377_159_531_563_721_6, 0.015_731_328_799_698_928, 0.040_556_626_365_515_2, 0.060_259_349_593_861_31, 0.084_003_849_672_746_68, 0.132_189_432_157_627_82, 0.186_949_071_668_018_1, 0.336_996_714_755_171_57, 0.709_769_063_878_318_4, 1.300_200_974_236_700_2],
    [-0.151_370_841_900_281_66, -0.005_024_584_413_723_314, -0.000_645_981_000_662_952_1, 0.001_897_337_842_401_713_3, 0.004_114_936_381_256_088, 0.006_799_416_734_531_388, 0.009_107_487_131_644_75, 0.011_218_482_353_786_65, 0.013_412_233_900_976_464, 0.015_019_073_514_268_955, 0.017_059_641_997_912_517, 0.019_210_720_482_073_985, 0.021_043_002_119_928_72, 0.023_633_679_114_107_88, 0.026_049_834_920_335_663, 0.029_687_287_228_439_684, 0.070_786_319_166_584_73],
    [-0.151_370_841_900_281_66, -0.012_260_984_669_113_031, -0.007_185_169_032_916_741, -0.004_291_268_043_308_475, -0.002_053_155_752_179_183_6, -0.000_133_164_541_507_758_95, 0.001_674_328_173_140_905_4, 0.003_283_856_111_049_476, 0.005_148_866_942_588_908, 0.007_397_497_627_764_699, 0.009_684_655_225_601_35, 0.012_278_751_410_602_116, 0.014_552_906_518_884_986, 0.017_414_921_239_369_81, 0.020_569_532_014_850_057, 0.025_028_496_827_796_41, 0.047_365_826_639_934_16],
    [-0.146_324_071_204_326_72, -0.018_226_091_458_231_86, -0.013_213_914_820_720_185, -0.009_914_331_107_033_78, -0.007_885_227_930_770_13, -0.006_122_355_929_626_439_5, -0.004_531_095_749_844_536, -0.003_001_332_567_368_371, -0.001_446_277_369_257_491, 0.000_182_281_129_093_018_38, 0.001_821_410_140_956_960_4, 0.003_332_860_969_908_261_3, 0.005_620_894_553_432_592, 0.009_213_714_662_636_149, 0.013_626_591_415_008_535, 0.017_901_850_219_720_02, 0.041_547_986_721_067_13],
    [-0.144_187_734_572_365_9, -0.021_732_505_581_985_52, -0.018_167_076_417_367_842, -0.015_284_195_464_668_23, -0.012_464_647_494_389_447, -0.010_310_220_903_982_542, -0.008_649_399_379_926_36, -0.007_419_546_957_606_022, -0.006_192_300_811_896_074, -0.004_827_708_020_949_786, -0.003_447_851_809_610_403_5, -0.001_901_729_126_421_028, -8.166_969_119_115_716e-5, 0.001_518_065_673_612_058_6, 0.003_188_763_593_083_272, 0.012_433_484_992_530_096, 0.026_470_891_991_132_81],
    [-0.144_187_734_572_365_9, -0.022_683_486_981_499_086, -0.020_848_541_872_852_75, -0.018_041_452_389_959_703, -0.015_747_883_058_974_108, -0.013_121_117_397_099_043, -0.011_261_804_106_965_641, -0.009_389_707_552_380_047, -0.008_022_286_508_301_859, -0.006_667_076_458_180_185, -0.005_509_326_653_808_501, -0.004_060_556_216_386_294, -0.002_059_154_153_281_979_7, 0.000_447_293_960_635_895_73, 0.001_947_881_444_707_181_3, 0.012_064_361_168_012_325, 0.018_367_526_656_161_842],
    [-0.144_187_734_572_365_9, -0.022_850_025_449_648_4, -0.021_440_326_326_658_907, -0.019_579_892_377_554_284, -0.017_273_263_801_156_246, -0.015_010_319_431_397_825, -0.012_339_894_738_437_926, -0.010_325_475_546_786_608, -0.008_452_126_043_754_421, -0.006_878_305_626_487_705, -0.005_755_072_301_112_479_5, -0.004_330_218_027_538_298, -0.002_247_103_394_386_848, 0.000_202_126_468_235_942_84, 0.001_919_103_763_213_215_8, 0.012_284_909_733_882_054, 0.020_972_704_568_347_565],
    [-0.079_029_730_666_695_97, -0.023_392_435_157_277_437, -0.021_642_689_714_532_294, -0.019_980_479_785_946_18, -0.017_816_929_717_924_836, -0.015_998_503_910_084_44, -0.013_134_078_856_282_146, -0.010_569_606_422_807_82, -0.008_809_477_872_422_774, -0.006_802_479_154_589_844, -0.005_689_971_577_242_873, -0.003_884_666_127_717_434, -0.002_272_904_724_304_224, -0.000_144_842_403_479_514_37, 0.001_734_056_531_482_742_5, 0.013_051_837_326_745_612, 0.022_537_637_046_290_936],
    [-0.083_033_336_683_806_66, -0.022_948_215_307_967_982, -0.020_530_859_816_785_905, -0.018_869_601_891_294_892, -0.016_996_045_449_458_41, -0.015_495_392_555_169_829, -0.012_625_326_012_909_477, -0.009_899_200_784_168_34, -0.008_099_829_592_506_493, -0.006_059_848_937_698_157, -0.004_062_188_193_160_194, -0.002_721_215_929_360_444, -0.001_132_801_899_275_470_8, 0.000_762_013_311_529_523_6, 0.002_976_412_494_842_015, 0.009_497_648_882_221_535, 0.023_774_551_871_941_614],
    [-0.083_033_336_683_806_66, -0.021_343_707_911_265_25, -0.019_096_825_824_775_863, -0.017_006_682_550_810_368, -0.015_787_889_283_193_753, -0.013_717_739_645_044_956, -0.011_164_020_283_980_891, -0.008_785_553_435_980_3, -0.006_701_495_292_903_621, -0.004_036_776_347_970_097, -0.002_717_866_595_902_27, -0.001_330_641_697_236_737_6, 0.000_239_437_257_158_917_2, 0.002_045_700_619_854_976_5, 0.005_125_082_784_039_397, 0.007_462_730_428_225_941, 0.023_774_551_871_941_614],
    [-0.264_115_726_135_348_3, -0.021_848_955_041_450_88, -0.018_887_236_496_364_156, -0.016_732_990_616_598_397, -0.015_614_494_867_005_588, -0.013_265_658_310_082_664, -0.011_164_316_680_796_057, -0.008_777_040_123_198_025, -0.006_495_641_176_955_611_5, -0.003_837_357_995_273_164_7, -0.002_688_411_090_785_576_5, -0.001_351_786_242_867_050_7, 0.000_274_523_042_013_439_16, 0.001_914_980_268_298_898_9, 0.004_492_432_057_456_486, 0.007_031_638_256_802_15, 0.023_774_551_871_941_614],
    [-0.264_115_726_135_348_3, -0.022_444_016_945_013_624, -0.019_637_290_397_577_46, -0.016_869_472_475_521_525, -0.015_729_546_353_676_65, -0.014_076_486_955_269_28, -0.011_531_864_392_911_078, -0.009_454_057_031_050_157, -0.006_681_518_429_852_866, -0.004_004_033_776_038_612, -0.002_690_308_591_349_667_8, -0.001_567_828_249_281_040_7, 0.000_121_508_612_969_720_76, 0.001_313_970_321_309_576_8, 0.003_830_606_906_706_033, 0.005_909_790_651_777_64, 0.023_774_551_871_941_614],
    [-0.264_115_726_135_348_3, -0.022_165_635_174_127_986, -0.019_275_309_268_552_37, -0.016_932_472_613_142_72, -0.016_202_817_375_093_565, -0.014_790_707_019_735_536, -0.011_871_722_550_814_95, -0.009_155_552_368_626_729, -0.005_780_406_356_359_088, -0.003_825_800_091_817_070_7, -0.002_267_559_612_962_526_4, -0.001_005_423_565_321_668, -8.316_896_201_286_867e-6, 0.002_347_740_686_585_122_6, 0.004_530_081_302_262_09, 0.007_506_605_962_552_911, 0.023_318_013_543_993_53],
    [-0.264_115_726_135_348_3, -0.023_270_624_338_532_07, -0.019_931_062_270_359_453, -0.017_930_368_334_954_087, -0.016_689_985_193_639_727, -0.015_968_219_891_243_14, -0.012_321_082_274_706_347, -0.010_577_359_980_717_283, -0.007_992_472_091_450_374, -0.005_110_421_542_088_428, -0.003_672_366_448_404_406_3, -0.002_246_756_731_909_794_4, -0.000_749_495_571_540_363_6, 0.002_089_483_331_530_244_7, 0.004_380_498_616_230_34, 0.012_897_595_930_590_896, 0.180_773_666_735_245_86],
    [-0.068_525_071_190_601_56, -0.025_127_287_295_940_572, -0.023_378_223_658_745_288, -0.019_897_825_495_752_92, -0.018_336_395_498_564_72, -0.016_577_188_268_608_04, -0.012_666_098_216_365_32, -0.011_706_123_126_168_876, -0.010_580_137_998_213_814, -0.009_058_644_489_947_174, -0.007_418_341_713_026_926, -0.003_997_021_051_904_66, -0.003_185_093_623_159_419, -0.000_600_832_027_586_513_2, 0.003_908_296_795_173_861, 0.015_606_041_485_666_656, 0.180_773_666_735_245_86],
    [-0.068_525_071_190_601_56, -0.024_943_583_094_253_503, -0.023_631_510_438_720_724, -0.020_268_548_809_022_877, -0.019_122_361_714_029_642, -0.015_780_786_471_510_8, -0.012_526_488_492_511_641, -0.011_899_853_624_924_335, -0.010_728_871_747_460_159, -0.009_355_049_447_445_602, -0.007_517_131_243_515_734, -0.004_750_642_991_931_842, -0.002_405_322_635_892_406, 0.001_936_860_536_654_429_8, 0.012_731_032_717_756_02, 0.017_454_498_110_111_027, 0.180_773_666_735_245_86],
    [-0.051_073_114_129_384_256, -0.025_235_558_629_281_24, -0.020_458_228_310_960_39, -0.018_718_883_849_094_228, -0.015_459_167_048_024_374, -0.014_638_486_076_396_18, -0.013_021_438_687_519_56, -0.011_851_208_148_373_642, -0.008_977_236_630_926_234, -0.006_783_844_526_858_491, -0.003_558_440_872_089_973, 0.000_312_046_888_924_910_2, 0.001_320_678_224_994_660_7, 0.002_795_449_593_797_29, 0.008_854_379_760_548_672, 0.020_410_197_363_776_31, 0.395_947_944_721_579_57],
    [-0.492_342_081_368_886_3, -0.021_835_691_425_734_96, -0.017_953_208_039_489_56, -0.014_643_052_031_393_109, -0.011_061_717_988_363_282, -0.008_517_171_827_344_018, -0.006_525_843_616_451_379, -0.004_683_597_599_473_965_5, -0.002_857_045_936_395_703, -0.000_830_458_477_713_455_4, 0.001_007_599_059_707_691_3, 0.002_768_688_692_257_747_7, 0.005_930_288_766_903_061, 0.011_150_510_550_934_349, 0.015_914_759_361_132_836, 0.021_029_147_984_204_83, 0.395_947_944_721_579_57],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.178_584_534_245_291_7, -0.144_057_447_441_326_31, -0.109_444_157_045_189_03, -0.080_603_337_772_916_09, -0.060_959_163_106_258_37, -0.037_174_956_671_665_32, -0.021_934_673_424_456_708, 0.003_559_428_520_366_058_3, 0.035_655_742_737_238_4, 0.083_745_320_547_807_88, 0.114_633_284_462_437_08, 0.152_127_421_487_964_51, 0.200_926_997_220_995_58, 0.281_488_932_228_024_46, 0.339_405_337_098_167_17, 4.350_198_912_024_838_5],
    [-0.680_140_407_031_776_9, -0.230_413_933_875_977_94, -0.188_392_131_538_432_1, -0.153_547_718_600_585_9, -0.129_932_936_445_206_73, -0.098_220_180_880_233_9, -0.068_081_615_058_759_77, -0.035_221_949_757_120_43, -0.012_082_349_121_690_709, 0.036_489_849_647_482_454, 0.107_627_846_786_657_43, 0.152_076_851_067_270_26, 0.199_724_752_897_515_2, 0.264_672_887_839_630_83, 0.291_501_827_430_949, 0.353_643_474_118_248_76, 0.549_684_086_877_431_8],
    [-0.620_329_110_949_169_1, -0.188_114_117_334_752_45, -0.153_510_630_982_401_11, -0.133_078_798_415_901_56, -0.110_430_812_660_078_84, -0.089_305_523_640_382_63, -0.069_023_606_688_485_84, -0.041_435_256_302_133_114, -0.027_760_882_858_754_225, 0.004_156_256_774_460_593, 0.045_587_519_075_448_6, 0.106_000_233_715_406_3, 0.151_870_501_458_407_14, 0.199_975_156_044_360_56, 0.284_190_817_582_258_17, 0.341_636_241_751_957_7, 0.549_684_086_877_431_8],
    [-0.296_803_040_305_204, -0.163_265_626_412_764_68, -0.129_542_097_753_852, -0.108_859_129_335_006_94, -0.088_200_896_165_681_3, -0.069_543_597_690_956_9, -0.044_706_701_603_309_75, -0.031_968_975_258_477_195, -0.012_088_763_771_820_707, 0.027_757_389_144_172_78, 0.059_058_454_732_123_25, 0.105_253_288_191_648_73, 0.141_823_451_174_161_48, 0.180_246_845_015_915_29, 0.268_900_596_344_053_17, 0.327_011_287_029_882_14, 0.549_684_086_877_431_8],
    [-0.247_622_387_358_454_65, -0.142_992_742_854_083_34, -0.111_938_495_998_156_3, -0.082_417_372_275_526_45, -0.068_959_820_802_065_9, -0.043_806_251_160_264_19, -0.032_569_964_168_687_24, -0.015_559_223_271_987_908, 0.025_436_127_196_833_677, 0.043_027_156_541_117_06, 0.086_567_707_146_864_96, 0.112_212_572_965_740_89, 0.140_510_018_817_999_38, 0.174_736_649_340_583_28, 0.240_819_036_667_962_95, 0.315_266_972_372_390_1, 0.549_684_086_877_431_8],
    [-0.197_216_947_416_019_03, -0.133_651_284_275_913_58, -0.098_584_173_217_928_76, -0.069_984_906_465_568_2, -0.043_938_497_517_232_07, -0.036_859_057_135_260_31, -0.027_167_769_856_291_37, -0.000_893_554_825_150_971_4, 0.025_892_726_258_372_935, 0.038_493_934_092_514_22, 0.090_258_558_932_804_07, 0.109_017_797_783_207_42, 0.118_866_899_439_728_58, 0.142_910_350_213_759_86, 0.204_623_808_208_231_56, 0.297_261_443_617_567_5, 0.349_235_562_228_855_66],
    [-0.083_578_836_357_306_9, -0.020_914_223_604_772_53, -0.008_239_273_587_345_374, 0.004_232_709_920_309_58, 0.015_278_163_645_795_364, 0.023_729_886_255_825_283, 0.030_696_179_686_865_468, 0.037_054_068_409_497_994, 0.045_037_614_152_823_04, 0.063_497_909_811_111_66, 0.115_022_491_127_705_16, 0.153_387_192_738_968_75, 0.187_458_307_857_068_24, 0.239_981_672_123_438_46, 0.316_154_028_233_843_2, 0.375_526_436_749_560_27, 0.517_273_367_645_503_9],
    [-0.087_062_278_931_240_08, -0.020_577_293_454_256_212, -0.009_004_069_898_978_485, 0.000_682_958_674_823_082, 0.009_120_660_089_931_678, 0.016_802_965_244_112_2, 0.023_708_017_156_859_777, 0.029_489_517_948_810_703, 0.035_373_250_452_814_906, 0.041_604_930_395_010_15, 0.049_518_943_384_862_84, 0.095_763_804_344_856_39, 0.283_000_960_918_927_35, 0.377_197_404_856_144_5, 0.458_711_929_283_930_06, 0.540_554_197_509_694_2, 0.739_887_069_791_329_3],
    [-0.184_111_841_041_168_84, -0.079_067_751_408_559_7, -0.052_952_610_823_765_185, -0.035_349_746_996_430_98, -0.022_638_356_142_078_608, -0.012_121_622_398_449_92, -0.002_879_899_401_093_745_5, 0.005_536_028_859_201_34, 0.014_535_631_696_072_544, 0.024_063_991_859_907_35, 0.033_123_714_353_841_56, 0.044_837_759_525_983_24, 0.341_834_377_254_014_94, 0.508_811_101_708_308_9, 0.586_388_576_706_081_7, 0.670_672_571_965_724_3, 0.842_569_165_567_405_9],
    [-1.027_022_028_902_709_7, -0.137_289_578_294_180_54, -0.108_663_466_192_476_7, -0.087_908_547_762_321_2, -0.070_761_117_072_376_8, -0.054_088_941_463_591_26, -0.039_557_920_913_265_27, -0.026_026_081_858_347_566, -0.013_219_714_237_466_287, -0.000_763_537_337_446_491_1, 0.013_188_205_255_190_339, 0.035_059_436_792_023_8, 0.485_834_033_065_930_1, 0.609_227_196_412_940_1, 0.679_440_939_290_876_9, 0.741_358_129_902_655_8, 0.842_569_165_567_405_9],
    [-1.027_022_028_902_709_7, -0.246_934_079_147_055_5, -0.140_803_585_516_160_48, -0.118_510_670_151_580_23, -0.102_801_201_552_025_74, -0.085_808_042_661_361_45, -0.068_483_042_746_172_63, -0.048_546_384_034_968_96, -0.026_563_363_596_828_96, -0.000_110_349_997_611_923_84, 0.132_517_279_549_229_4, 0.250_811_149_420_450_63, 0.479_983_120_327_845_1, 0.629_190_520_995_683_4, 0.706_078_110_209_58, 0.763_763_282_025_881_8, 0.842_569_165_567_405_9],
    [-1.027_022_028_902_709_7, -0.274_190_319_758_752_47, -0.145_641_872_076_444_88, -0.118_266_401_350_133_4, -0.097_609_750_966_675_48, -0.073_219_984_704_145_91, -0.041_268_391_981_940_03, 0.019_725_845_045_121_745, 0.114_868_066_021_895_89, 0.149_350_812_923_579_7, 0.169_647_307_700_983_24, 0.192_239_352_648_145_48, 0.285_561_430_175_442_3, 0.416_802_375_235_119_15, 0.579_145_974_315_922_5, 0.737_644_635_996_353_7, 0.842_569_165_567_405_9],
    [-1.027_022_028_902_709_7, -0.174_669_961_504_908_8, -0.118_187_321_807_033_38, -0.085_794_003_283_397_26, -0.059_996_061_227_511_85, -0.032_632_724_804_262_29, -0.005_499_564_420_031_385, 0.022_793_736_948_780_185, 0.054_982_788_700_880_38, 0.087_207_578_184_065_49, 0.122_686_802_128_487_01, 0.144_522_441_172_823_53, 0.163_211_393_652_276_1, 0.178_590_627_439_068_7, 0.201_140_208_638_505_57, 0.336_852_138_266_858_17, 0.832_921_162_749_05],
    [-0.301_822_333_533_877_1, -0.158_392_491_000_810_65, -0.118_236_392_328_388_94, -0.091_371_579_515_717_75, -0.070_702_892_161_864_11, -0.051_189_836_215_468_09, -0.032_238_119_727_887_65, -0.013_782_073_172_099, 0.004_289_707_060_738_873, 0.022_484_736_823_812_56, 0.041_837_995_816_921_006, 0.060_000_555_717_213_45, 0.077_160_063_353_071_48, 0.098_902_893_882_308_07, 0.120_989_857_687_365_21, 0.157_952_365_122_249_56, 0.485_787_356_302_320_84],
    [-0.284_912_573_553_809_6, -0.135_175_252_834_373_93, -0.100_316_664_134_176_3, -0.076_294_505_160_345_03, -0.056_479_142_039_091_36, -0.039_672_843_455_171_206, -0.023_624_083_286_521_776, -0.009_091_500_965_501_556, 0.005_235_832_651_713_203_5, 0.019_383_156_336_783_82, 0.033_128_891_075_756_756, 0.049_328_832_859_733_5, 0.061_520_319_710_566_1, 0.074_211_525_753_742_96, 0.091_735_790_668_091_68, 0.110_245_335_588_397_48, 0.516_795_333_802_096_5],
    [-0.251_625_203_759_693, -0.104_298_684_463_102_2, -0.075_957_226_842_048_68, -0.057_435_804_014_753_21, -0.043_090_556_948_740_47, -0.029_908_780_366_733_004, -0.016_364_420_355_622_595, -0.004_973_017_706_815_078, 0.007_053_872_371_810_079, 0.018_240_457_273_181_77, 0.029_255_435_528_390_036, 0.043_756_571_015_951_116, 0.054_781_347_799_209_845, 0.066_755_571_625_054_31, 0.081_921_015_551_318_45, 0.102_466_260_418_911_96, 0.554_480_805_323_174_9],
    [-0.399_864_242_402_805_77, -0.085_042_788_385_560_77, -0.064_022_575_340_501_89, -0.049_120_269_720_002_52, -0.036_301_778_146_360_85, -0.024_929_594_029_293_272, -0.014_562_124_153_188_059, -0.005_117_541_968_470_889, 0.005_017_161_244_215_166, 0.015_373_257_254_225_796, 0.023_065_488_636_021_817, 0.035_788_201_293_193_315, 0.046_956_938_581_310_705, 0.062_592_342_337_093_68, 0.088_803_393_676_322_87, 0.469_211_039_020_066_17, 0.554_480_805_323_174_9],
    [-0.510_593_370_883_813_5, -0.105_541_033_918_082_35, -0.070_513_800_697_446_38, -0.052_190_799_593_783_054, -0.036_363_856_567_979_6, -0.025_190_016_580_175_41, -0.013_327_240_269_119_781, -0.001_541_290_477_148_309_9, 0.009_623_037_406_462_514, 0.020_528_863_569_838_28, 0.035_104_352_208_431_86, 0.048_411_314_354_938_45, 0.070_667_731_488_404_4, 0.219_303_209_770_624_54, 0.382_059_372_363_796_33, 0.504_838_251_730_186_3, 0.554_480_805_323_174_9],
    [-0.552_759_878_989_978_9, -0.124_137_413_391_975_45, -0.089_896_374_266_169_93, -0.069_382_004_716_369_95, -0.050_563_265_072_361_224, -0.033_766_382_385_202_02, -0.017_540_191_686_266_426, -0.001_390_566_893_543_736_6, 0.018_017_722_153_756_556, 0.031_068_065_587_710_19, 0.044_816_459_817_462_614, 0.055_036_732_991_676_766, 0.067_772_800_663_481_46, 0.078_525_888_739_965_33, 0.176_200_999_869_559_87, 0.316_265_629_938_511_04, 0.554_480_805_323_174_9],
    [-0.610_991_692_431_182_5, -0.123_334_997_506_282_71, -0.091_344_469_793_660_63, -0.071_558_665_897_184_11, -0.053_345_075_085_584_934, -0.037_260_777_412_010_865, -0.022_787_519_668_902_346, -0.008_927_067_944_345_043, 0.012_704_677_986_147_495, 0.027_471_186_298_151_856, 0.038_773_521_692_383_926, 0.048_144_091_837_273_48, 0.058_300_834_641_860_61, 0.068_022_376_736_911_86, 0.077_109_250_587_042_47, 0.091_846_065_639_877_11, 0.517_118_427_281_609_6],
    [-0.610_991_692_431_182_5, -0.505_633_422_849_584_3, -0.097_367_858_424_068_73, -0.070_529_220_303_647_54, -0.047_636_649_592_657_12, -0.034_056_666_476_476_26, -0.019_324_725_981_107_49, -0.008_145_752_600_179_383, 0.005_885_110_056_605_696_5, 0.026_134_065_179_127_74, 0.041_222_382_763_049_93, 0.051_547_358_017_760_99, 0.064_584_294_974_626_2, 0.074_183_563_837_495_94, 0.082_367_971_425_324_73, 0.101_770_161_208_528_47, 0.553_836_662_162_159_9],
    [-0.610_991_692_431_182_5, -0.545_745_189_872_159_1, -0.499_940_647_475_254_5, -0.127_051_877_320_244_6, -0.052_329_791_682_686_37, -0.034_870_583_351_069_18, -0.018_542_653_273_473_316, -0.006_133_808_637_387_834, 0.006_673_906_465_418_284, 0.024_645_767_336_398_02, 0.060_110_503_697_339_56, 0.074_186_857_950_018_78, 0.085_785_833_231_598_57, 0.103_750_912_252_790_86, 0.125_195_395_306_205_13, 0.312_761_941_433_933_77, 0.695_200_670_104_314_1],
    [-0.610_991_692_431_182_5, -0.503_264_604_782_622, -0.204_480_967_950_091_32, -0.085_096_723_908_779_18, -0.061_548_070_871_314_5, -0.041_726_917_988_909_92, -0.035_182_240_566_095_944, -0.027_824_298_639_230_215, -0.014_052_672_333_842_608, -0.008_481_297_646_876_738, 0.005_533_765_756_740_629, 0.032_781_357_388_757_96, 0.106_021_063_826_406_97, 0.207_059_323_065_953_87, 0.455_776_564_262_818_54, 0.578_659_289_361_261_3, 0.695_200_670_104_314_1],
    [-0.101_516_406_530_789_64, -0.017_310_644_019_847_003, -0.011_338_015_267_668_991, -0.006_125_296_433_708_744, -0.002_975_817_121_301_008_3, 0.000_376_467_084_149_508_1, 0.002_942_457_103_185_955, 0.004_478_590_681_884_457, 0.005_316_617_877_003_080_4, 0.005_943_527_820_627_752, 0.006_709_876_635_443_933, 0.007_369_929_840_312_012, 0.007_928_043_548_854_976, 0.008_645_317_671_290_344, 0.009_332_227_449_572_702, 0.010_306_360_538_891_95, 2.260_971_381_516_218_7],
    [-0.001_469_588_428_703_483_3, -0.000_244_504_074_692_973_1, 0.000_445_437_453_319_948_23, 0.001_311_245_524_345_112, 0.001_917_483_267_167_385_6, 0.002_437_593_162_889_751, 0.003_122_395_479_565_549, 0.004_380_627_649_186_071, 0.004_988_943_644_801_673, 0.005_269_433_732_930_043_5, 0.005_522_234_371_705_372, 0.006_036_914_677_248_309_5, 0.007_665_956_306_280_463_5, 0.008_396_730_084_103_021, 0.009_780_756_456_615_419, 0.011_320_317_674_844_513, 1.669_422_401_046_923],
    [-0.001_469_588_428_703_483_3, 0.001_746_614_025_096_674_8, 0.003_007_912_454_627_703_3, 0.004_515_735_151_484_046, 0.005_242_378_338_705_116, 0.005_350_568_544_924_343, 0.005_851_453_569_971_027, 0.006_404_930_191_424_466, 0.007_493_740_774_295_305, 0.008_227_844_400_037_51, 0.008_485_777_015_632_186, 0.008_877_411_932_320_434, 0.009_191_954_979_327_548, 0.009_794_719_341_630_323, 0.010_579_091_703_995_088, 0.011_476_435_368_650_851, 1.726_859_896_389_463_5],
    [-0.000_248_224_263_765_806_1, 0.002_717_755_725_111_043_8, 0.003_923_374_605_735_554, 0.004_790_798_716_704_307, 0.005_349_408_276_729_466, 0.005_858_910_527_059_766, 0.006_466_380_328_915_829, 0.007_052_623_746_421_828, 0.007_708_053_896_600_296, 0.008_268_844_465_103_814, 0.008_696_447_852_190_523, 0.008_930_362_297_464_977, 0.009_286_639_595_451_647, 0.009_740_496_025_489_896, 0.010_536_481_721_224_943, 0.011_674_935_174_067_943, 1.726_859_896_389_463_5],
    [0.001_475_732_076_416_047_5, 0.002_352_977_002_792_718_8, 0.002_966_641_156_975_267, 0.003_780_134_259_511_784, 0.004_580_985_892_154_711, 0.005_268_156_201_301_663, 0.005_794_604_761_038_034, 0.006_067_042_481_866_252_5, 0.006_821_818_200_864_961, 0.007_260_845_277_091_139, 0.007_710_537_050_199_731, 0.008_282_456_571_649_645, 0.008_771_199_050_459_207, 0.009_238_324_647_418_22, 0.009_500_128_542_775_712, 0.010_520_165_077_423_019, 1.726_859_896_389_463_5],
    [0.001_475_732_076_416_047_5, 0.002_661_435_151_477_115, 0.003_644_486_438_988_991_6, 0.004_673_895_408_419_11, 0.005_366_031_471_552_025, 0.005_795_286_507_629_423, 0.006_082_158_643_271_317, 0.006_667_358_329_076_708, 0.007_202_186_638_640_351, 0.007_731_840_376_909_162, 0.008_272_081_346_246_123, 0.008_728_552_845_949_724, 0.009_084_703_321_259_662, 0.009_597_917_594_726_531, 0.010_442_369_699_190_853, 0.011_108_495_735_195_56, 1.726_859_896_389_463_5],
    [0.001_475_732_076_416_047_5, 0.004_735_116_579_200_982, 0.005_615_563_692_376_687, 0.006_057_850_366_366_069_5, 0.006_320_128_034_965_065, 0.007_018_243_515_480_541_5, 0.007_562_798_968_326_881, 0.008_116_573_821_757_47, 0.008_530_727_993_388_982, 0.008_909_985_936_542_415, 0.009_287_294_124_452_311, 0.009_975_895_167_754_113, 0.010_426_112_363_384_487, 0.010_969_969_087_752_298, 0.011_393_210_952_829_746, 0.011_831_083_026_589_336, 1.825_039_580_831_490_5],
    [0.002_963_521_768_990_729, 0.004_738_911_165_487_478, 0.005_467_132_924_233_139, 0.006_052_249_599_990_892, 0.006_249_624_631_706_37, 0.006_714_825_191_220_239, 0.007_158_304_114_512_461, 0.007_789_949_543_575_235, 0.008_391_864_178_187_896, 0.008_672_860_811_146_84, 0.008_982_924_493_513_324, 0.009_444_926_309_536_205, 0.010_044_188_756_702_314, 0.010_564_837_992_188_207, 0.011_156_738_324_575_989, 0.011_732_274_976_616_197, 1.825_039_580_831_490_5],
    [0.003_232_990_010_511_027_6, 0.004_256_704_977_246_631, 0.004_989_089_615_224_59, 0.005_540_718_455_574_429, 0.006_054_105_641_256_515, 0.006_319_738_351_777_011, 0.006_703_907_762_366_07, 0.007_092_785_674_784_582, 0.007_594_578_845_446_480_5, 0.008_150_022_214_760_402, 0.008_579_901_582_248_6, 0.008_906_546_589_888_062, 0.009_065_714_757_428_624, 0.009_569_340_949_195_856, 0.010_044_195_852_050_03, 0.010_970_158_053_738_546, 1.825_039_580_831_490_5],
    [0.003_232_990_010_511_027_6, 0.004_545_836_903_472_446, 0.005_531_319_679_635_531, 0.006_135_948_374_810_143, 0.006_583_712_437_063_389, 0.007_002_479_075_577_122, 0.007_448_474_585_876_344, 0.007_801_635_406_032_892, 0.008_388_923_322_034_338, 0.008_844_422_592_170_855, 0.009_020_469_710_716_142, 0.009_272_848_474_834_018, 0.009_678_353_218_256_403, 0.010_202_068_453_880_579, 0.010_789_367_777_422_103, 0.011_691_501_065_679_058, 1.825_039_580_831_490_5],
    [0.003_232_990_010_511_027_6, 0.005_474_430_217_415_439, 0.006_204_473_032_581_2, 0.006_787_946_151_358_404_5, 0.007_241_049_217_032_574, 0.007_663_581_523_801_825, 0.008_005_645_177_284_47, 0.008_475_547_532_101_264, 0.008_889_822_119_113_45, 0.009_146_879_481_412_927, 0.009_459_344_546_063_722, 0.009_898_932_609_201_687, 0.010_253_385_884_525_006, 0.010_771_935_295_106_477, 0.011_352_090_653_312_236, 0.011_996_100_439_942_698, 2.260_971_381_516_218_7],
    [0.003_852_443_099_584_768, 0.005_268_339_074_386_49, 0.005_668_505_928_348_808, 0.006_264_696_926_945_252, 0.006_805_056_777_063_513, 0.007_222_771_320_577_117, 0.007_492_837_905_956_973, 0.007_781_386_867_117_094, 0.008_112_552_131_352_523, 0.008_527_809_149_394_728, 0.008_924_836_178_706_043, 0.009_200_847_594_095_903, 0.009_634_836_236_758_78, 0.010_010_959_740_469_033, 0.010_467_032_655_441_622, 0.011_314_224_428_500_115, 2.260_971_381_516_218_7],
    [0.001_514_298_632_935_062_7, 0.004_994_232_384_511_694, 0.005_280_911_135_546_232, 0.005_576_375_043_783_49, 0.005_895_888_099_139_576, 0.006_435_612_852_720_839_5, 0.006_897_937_895_721_972, 0.007_209_769_189_583_635, 0.007_449_052_962_649_989_5, 0.007_682_540_933_984_581, 0.007_959_678_639_427_726, 0.008_307_290_099_035_19, 0.008_707_527_764_754_63, 0.009_111_370_811_986_42, 0.009_604_800_894_501_85, 0.010_003_417_867_342_159, 2.260_971_381_516_218_7],
    [-0.009_304_919_040_006_965, -0.001_566_576_705_905_872_5, 0.001_514_118_563_241_31, 0.002_898_304_514_755_703_8, 0.003_912_666_696_154_12, 0.004_623_590_910_645_741_4, 0.005_184_782_238_952_428, 0.005_536_115_204_861_78, 0.005_878_629_903_951_188, 0.006_401_650_867_632_504, 0.006_958_523_526_105_782_5, 0.007_354_224_880_580_697, 0.007_681_184_762_897_823, 0.008_062_073_169_013_726, 0.008_610_837_649_335_212, 0.009_355_573_342_133_683, 2.260_971_381_516_218_7],
    [-0.026_558_997_855_063_105, -0.012_769_159_896_826_565, -0.008_114_834_746_487_574, -0.005_492_162_915_263_893, -0.003_851_764_995_028_091_3, -0.002_242_332_040_487_867, -0.000_744_675_371_625_418_9, 0.000_826_113_400_269_165_5, 0.002_135_956_734_080_968_4, 0.002_925_723_037_259_348, 0.003_741_871_521_864_507_7, 0.004_453_739_927_000_871, 0.005_188_703_078_453_07, 0.005_841_295_394_396_651, 0.006_894_461_328_305_575, 0.007_932_101_773_046_808, 0.023_244_546_013_342_582],
    [-0.054_352_043_406_043_826, -0.020_259_485_474_615_7, -0.017_148_766_022_439_737, -0.014_516_840_899_445_537, -0.012_426_242_188_629_094, -0.010_356_772_837_975_889, -0.008_429_902_412_751_962, -0.006_869_388_962_295_442, -0.005_488_166_941_242_534, -0.004_394_616_019_340_151_5, -0.003_106_747_280_213_656, -0.001_776_598_808_111_522, -0.000_293_420_168_836_334_7, 0.001_747_022_981_607_617_4, 0.003_179_935_595_126_562_7, 0.004_767_802_378_793_419, 0.011_148_451_849_507_08],
    [-0.089_266_707_755_831_12, -0.025_732_374_888_014_053, -0.023_342_231_324_642_81, -0.020_825_539_306_609_936, -0.019_132_645_713_060_63, -0.017_665_215_285_089_964, -0.016_268_743_768_091_936, -0.014_745_359_389_523_647, -0.013_326_858_069_984_784, -0.011_784_424_884_497_61, -0.009_913_960_594_498_502, -0.008_257_558_986_480_392, -0.006_508_685_816_052_372, -0.005_155_406_521_062_031, -0.003_716_414_124_745_069_7, -0.001_748_019_022_624_035_2, 0.005_520_559_198_407_771],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.122_651_993_386_103_77, -0.086_787_735_097_424_46, -0.073_490_468_074_499_33, -0.046_451_311_802_008_406, -0.036_610_233_237_075_81, -0.029_770_664_232_707_657, -0.012_895_195_874_953_606, -0.007_347_864_192_160_003, 0.004_736_700_167_538_784, 0.006_824_372_588_739_910_5, 0.015_627_167_842_236_306, 0.021_251_624_563_218_423, 0.035_549_827_909_175_026, 0.049_576_813_162_255_62, 0.061_958_760_535_472_8, 0.081_193_245_610_955_62, 0.113_549_717_623_364_91],
    [-0.123_577_932_405_161_24, -0.087_384_923_092_157_63, -0.065_146_081_883_348_21, -0.052_929_999_487_980_03, -0.044_464_981_757_560_57, -0.033_358_204_261_705_36, -0.017_803_435_106_241_102, -0.010_408_931_863_566_418, 0.003_179_956_886_255_405_7, 0.011_824_163_680_926_364, 0.020_618_333_118_506_013, 0.028_127_772_636_237_065, 0.036_285_104_426_687_385, 0.052_508_354_829_491_06, 0.061_219_187_200_241_97, 0.080_431_634_796_890_49, 0.113_549_717_623_364_91],
    [-0.123_577_932_405_161_24, -0.087_517_987_445_487_24, -0.065_970_878_218_188_13, -0.059_701_841_709_757_45, -0.047_692_018_246_728_904, -0.039_258_006_333_453_78, -0.022_097_980_440_520_346, -0.010_293_960_493_911_524, 0.002_062_353_796_303_848_7, 0.014_216_153_815_117_02, 0.021_466_177_088_116_26, 0.031_157_921_263_178_477, 0.040_339_427_674_313_83, 0.051_430_988_599_697_004, 0.058_250_853_529_381_745, 0.078_504_406_295_919_63, 0.113_549_717_623_364_91],
    [-0.123_577_932_405_161_24, -0.085_633_972_093_269_19, -0.070_494_055_664_326_76, -0.060_178_758_466_048_87, -0.046_751_236_100_625_9, -0.039_417_227_601_896_89, -0.015_128_656_363_213_162, -0.006_752_143_665_888_618, 0.001_792_778_379_938_572_4, 0.016_235_047_088_583_06, 0.028_318_377_216_100_168, 0.039_964_957_677_239_66, 0.048_995_461_854_695_68, 0.053_214_152_031_580_87, 0.063_581_619_341_148_35, 0.085_698_852_293_649_28, 0.117_022_224_010_421_06],
    [-0.123_577_932_405_161_24, -0.087_128_690_811_286_7, -0.076_402_062_288_356_93, -0.062_262_888_130_640_834, -0.046_837_991_663_142_95, -0.039_261_981_573_343_076, -0.019_481_014_752_811_81, -0.003_159_464_393_063_19, 0.001_001_907_804_322_314_7, 0.013_953_602_642_734_066, 0.026_717_367_345_812_135, 0.048_135_983_470_961_805, 0.051_580_358_928_257_325, 0.065_191_711_660_101_28, 0.080_148_475_895_384_53, 0.102_732_205_493_563_95, 0.117_022_224_010_421_06],
    [-0.117_180_842_233_406_08, -0.082_770_824_568_532_94, -0.070_168_547_608_263_09, -0.058_929_302_103_641_29, -0.044_683_949_691_997_15, -0.033_866_209_320_504_09, -0.023_634_271_699_229_35, -0.010_612_482_922_653_698, 0.000_969_418_607_747_430_8, 0.007_953_018_667_317_01, 0.017_392_355_142_852_204, 0.029_659_074_296_350_993, 0.049_653_851_625_965_195, 0.065_207_784_720_476_25, 0.090_184_966_803_416_37, 0.111_750_941_629_360_74, 0.125_507_928_485_441_34],
    [-0.102_338_758_665_737_16, -0.069_349_787_653_156_97, -0.051_335_085_964_526_996, -0.041_765_562_088_511_28, -0.035_995_139_315_146_17, -0.025_430_625_395_561_412, -0.018_596_818_457_481_686, -0.010_214_778_919_139_921, -0.001_878_252_657_066_197, 0.004_546_099_235_069_868, 0.009_558_409_317_066_406, 0.020_342_936_978_604_23, 0.029_680_344_438_342_48, 0.042_373_180_040_175_65, 0.066_621_723_332_472_22, 0.110_702_226_038_989_09, 0.125_507_928_485_441_34],
    [-0.081_831_213_047_211_53, -0.048_175_568_367_728_37, -0.039_418_300_254_946_116, -0.033_496_205_869_452_726, -0.028_607_069_663_446_637, -0.019_278_183_401_415_506, -0.011_906_504_114_96, -0.008_002_659_140_160_698, -0.001_571_304_559_673_809_8, 0.002_752_125_767_445_367, 0.006_494_691_382_478_788, 0.008_985_563_918_601_553, 0.017_696_015_082_830_27, 0.027_518_605_741_213_002, 0.035_680_778_714_373_76, 0.073_036_384_099_056_5, 0.125_507_928_485_441_34],
    [-0.051_552_737_253_638_4, -0.038_022_428_832_549_374, -0.032_414_436_883_214_67, -0.026_968_440_003_148_17, -0.016_931_220_942_577_693, -0.012_135_148_699_707_65, -0.008_475_020_086_598_263, -0.004_232_212_911_048_899_5, 0.000_118_706_974_643_753_94, 0.003_669_744_778_178_507, 0.006_615_792_495_130_492, 0.008_760_937_777_670_398, 0.014_046_930_976_878_024, 0.026_070_490_917_894_29, 0.029_831_695_074_482_096, 0.041_502_653_643_585_28, 0.125_507_928_485_441_34],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.123_577_932_405_161_24, -0.080_767_569_175_768_4, -0.061_040_446_642_268_69, -0.046_637_568_575_044_384, -0.036_665_001_894_038_46, -0.025_072_229_973_436_857, -0.012_537_092_476_769_282, -0.006_239_423_221_353_63, 0.000_982_981_587_946_746_8, 0.006_548_462_286_420_02, 0.014_394_763_425_974_33, 0.025_610_658_580_826_81, 0.035_704_820_307_601_79, 0.049_441_028_788_988_55, 0.061_713_199_521_451_36, 0.092_372_212_623_383_65, 0.562_299_322_024_696_8],
    [-0.185_257_318_695_862_43, -0.095_456_233_513_088_49, -0.077_294_467_714_502_55, -0.064_433_488_773_079_22, -0.052_062_736_367_155_65, -0.042_842_008_321_364_956, -0.034_548_450_380_883_694, -0.027_552_823_790_968_98, -0.019_700_172_140_596_42, -0.011_695_880_983_006_526, -0.004_642_710_966_026_573, 0.004_210_665_852_802_895, 0.012_752_870_938_246_138, 0.024_375_658_924_013_048, 0.036_443_098_465_382_82, 0.052_737_700_473_509_23, 0.108_225_933_108_892_81],
    [-0.264_783_334_127_278_84, -0.081_303_934_877_014_58, -0.062_117_837_534_664_04, -0.048_857_033_332_576_91, -0.039_114_761_827_999_965, -0.030_722_410_613_760_912, -0.023_591_613_426_670_26, -0.015_750_202_051_383_01, -0.008_627_845_708_311_514, -0.002_405_316_877_359_551, 0.006_182_184_843_939_351, 0.013_981_813_796_874_82, 0.023_424_432_018_208_156, 0.033_158_396_396_382_86, 0.044_905_691_171_154_47, 0.056_641_746_399_348_526, 0.108_225_933_108_892_81],
    [-0.414_515_785_524_366_1, -0.069_373_985_668_660_32, -0.055_068_385_318_895_99, -0.045_107_801_096_791_7, -0.036_424_847_778_451_085, -0.029_282_433_844_508_81, -0.022_987_081_320_181_94, -0.016_925_432_319_904_017, -0.010_869_673_727_946_297, -0.005_128_551_189_341_13, 0.001_204_785_243_568_117_8, 0.007_708_401_320_111_692_5, 0.015_046_844_877_855_64, 0.022_699_328_325_316_088, 0.032_166_528_567_445_6, 0.046_261_179_531_016_28, 0.219_134_422_641_193_6],
    [-0.462_164_890_989_437_64, -0.064_062_196_969_568_37, -0.049_801_728_743_686_03, -0.039_355_816_352_578_31, -0.030_304_605_491_766_556, -0.022_758_194_482_592_1, -0.015_941_997_969_597_75, -0.009_670_597_142_599_994, -0.003_642_243_479_112_181_7, 0.002_620_269_171_086_744, 0.008_917_914_290_194_883, 0.015_717_530_443_892_98, 0.022_244_958_962_179_23, 0.029_862_567_331_633_415, 0.039_037_786_115_203_56, 0.050_472_343_583_699_28, 0.219_134_422_641_193_6],
    [-0.462_164_890_989_437_64, -0.056_395_809_500_289_46, -0.036_604_476_932_396_51, -0.023_041_158_186_781_745, -0.013_370_457_623_914_629, -0.005_144_554_134_004_811_6, 0.002_951_720_673_963_732, 0.010_200_737_410_821_755, 0.016_728_513_216_871_66, 0.022_708_082_676_887_36, 0.028_153_188_823_563_463, 0.033_913_623_391_558_66, 0.040_093_618_519_340_22, 0.045_937_259_571_854_66, 0.052_382_363_886_028_1, 0.058_426_368_364_505_53, 0.219_134_422_641_193_6],
    [-0.462_164_890_989_437_64, -0.083_692_951_859_248_69, -0.021_205_267_550_200_614, -0.004_056_266_954_562_998, 0.007_742_612_289_234_145, 0.015_114_145_577_765_274, 0.021_316_373_395_286_886, 0.026_472_761_437_178_815, 0.031_164_819_449_772_906, 0.035_707_400_777_306_01, 0.039_980_644_087_896_03, 0.043_725_903_838_953_493, 0.047_849_359_258_061_344, 0.051_789_291_548_061_52, 0.056_075_284_810_885_56, 0.059_953_307_648_026_054, 0.219_134_422_641_193_6],
    [-0.462_164_890_989_437_64, -0.338_748_440_248_705_3, -0.301_782_815_619_651_26, -0.118_378_301_141_480_01, -0.021_976_665_198_174_893, -0.000_124_389_065_420_453_05, 0.011_920_679_396_047_767, 0.019_750_081_972_861_07, 0.025_832_571_564_818_697, 0.031_143_962_743_001_567, 0.035_940_469_771_242_81, 0.040_090_286_111_187_375, 0.044_011_652_696_900_785, 0.047_896_869_497_202_68, 0.051_913_843_551_921_54, 0.057_399_725_935_984_594, 0.183_728_784_809_951_8],
    [-0.427_934_864_579_216_9, -0.323_667_499_321_330_7, -0.291_344_114_131_417_1, -0.256_712_287_720_141_04, -0.191_801_657_555_633_64, -0.071_329_775_158_232_16, -0.010_431_564_839_497_783, 0.010_182_073_667_840_703, 0.021_198_703_903_877_143, 0.028_251_962_870_330_955, 0.034_733_693_134_660_9, 0.039_972_454_176_340_67, 0.044_978_604_506_410_57, 0.048_804_450_531_011_255, 0.053_693_226_971_675_34, 0.071_273_239_155_546_62, 0.265_882_223_047_982_15],
    [-0.427_934_864_579_216_9, -0.280_031_450_743_149_4, -0.235_173_555_973_495_68, -0.184_482_368_598_355_6, -0.120_401_404_265_250_21, -0.063_788_415_891_827_68, -0.003_101_456_529_612_523, 0.020_618_194_350_381_15, 0.033_051_049_043_391_3, 0.041_773_088_236_943_16, 0.048_148_595_924_363_91, 0.054_744_613_100_887_275, 0.071_664_014_965_752_78, 0.098_365_318_918_874_6, 0.130_950_416_290_804_73, 0.173_070_303_597_836_48, 0.326_797_034_328_499_77],
    [-0.413_495_359_128_833_7, -0.221_712_925_821_574_65, -0.129_977_592_558_374_46, -0.095_953_375_528_510_55, -0.074_175_739_838_958_23, -0.056_818_125_770_721_89, -0.039_936_222_986_433_505, -0.021_913_102_354_436_77, 0.005_480_762_279_181_732_6, 0.040_411_715_615_769_79, 0.064_523_199_038_544_25, 0.102_532_291_437_315_53, 0.139_885_735_386_521_35, 0.171_916_071_793_898_8, 0.203_475_067_561_928_62, 0.239_160_395_563_670_36, 0.338_944_146_550_081_3],
    [-0.413_495_359_128_833_7, -0.139_616_609_368_680_9, -0.098_664_894_406_452_54, -0.077_911_819_119_912_49, -0.062_749_186_524_146_24, -0.049_808_692_489_293_55, -0.039_330_689_065_875_4, -0.029_054_691_924_718_61, -0.019_751_678_750_462_208, -0.008_647_950_434_899_502, 0.011_689_904_889_886_504, 0.053_679_741_995_241_906, 0.119_890_362_362_327_31, 0.195_919_174_843_982_86, 0.234_047_906_665_596_74, 0.253_158_459_349_327_3, 0.338_944_146_550_081_3],
    [-0.355_659_621_015_381_73, -0.119_830_630_325_902_49, -0.086_556_075_770_426_19, -0.064_280_920_271_165_01, -0.049_277_768_041_266_004, -0.038_795_831_543_755_516, -0.029_529_201_010_295_68, -0.021_983_858_753_164_547, -0.014_691_057_555_705_319, -0.005_596_587_009_879_29, 0.011_521_120_917_532_615, 0.037_501_623_583_819_4, 0.073_993_594_186_373_52, 0.152_621_396_632_406_37, 0.233_895_037_549_614_33, 0.252_971_423_962_871_26, 0.338_944_146_550_081_3],
    [-0.337_647_045_722_911_04, -0.103_142_523_882_546_16, -0.071_001_983_700_067, -0.049_543_533_868_734_52, -0.036_631_435_485_481_31, -0.027_415_249_338_930_834, -0.019_781_776_628_694_61, -0.011_708_695_387_988_49, -0.001_745_618_643_377_878_5, 0.014_241_932_220_893_476, 0.033_285_507_419_144_135, 0.052_274_767_625_399_426, 0.077_937_161_853_860_27, 0.121_230_842_683_826_47, 0.216_821_980_839_822_57, 0.243_712_608_374_925_2, 0.338_944_146_550_081_3],
    [-0.317_127_089_060_780_3, -0.100_470_501_089_634_48, -0.068_262_871_672_027_77, -0.046_410_960_001_217_23, -0.031_025_093_811_988_022, -0.020_401_250_456_632_93, -0.009_050_742_157_773_193, 0.004_910_807_327_005_119, 0.022_154_341_854_380_07, 0.036_048_975_993_683_75, 0.050_058_417_063_100_77, 0.064_856_037_554_126_65, 0.086_435_953_113_879_82, 0.133_180_322_453_648_94, 0.195_984_001_153_803_77, 0.221_644_247_819_035_22, 0.326_324_679_997_560_65],
    [-0.317_127_089_060_780_3, -0.115_306_509_913_733_16, -0.089_539_529_053_885_97, -0.059_177_303_195_760_78, -0.034_950_627_454_420_45, -0.015_992_563_218_313_546, 0.002_157_779_344_211_66, 0.019_651_644_138_407_49, 0.031_147_897_157_689_636, 0.043_005_766_526_105_01, 0.055_967_874_066_783_22, 0.067_999_484_829_792_25, 0.093_261_142_153_275_69, 0.153_700_155_057_926_1, 0.177_897_107_477_692_9, 0.200_781_788_466_573_69, 0.286_529_792_771_750_6],
    [-0.246_197_789_275_110_88, -0.144_162_933_786_600_78, -0.124_141_688_762_102_2, -0.102_531_032_574_032_11, -0.073_009_164_880_776_8, -0.034_167_462_620_460_83, -0.002_921_063_749_870_648, 0.014_500_228_153_170_404, 0.028_502_920_060_835_75, 0.039_100_724_970_321_15, 0.050_519_354_140_095_48, 0.063_094_657_991_819_44, 0.087_372_311_410_286_35, 0.144_362_692_375_200_08, 0.163_631_233_441_235_42, 0.177_764_684_201_100_36, 0.251_857_009_507_778_9],
    [-0.246_197_789_275_110_88, -0.152_715_493_776_168_75, -0.143_741_564_606_647_94, -0.133_095_038_697_027_43, -0.121_360_219_397_615_1, -0.100_755_856_232_184_12, -0.072_764_487_716_657_85, -0.024_751_376_015_495_326, 0.008_228_059_480_799_223, 0.027_057_468_801_233_79, 0.039_894_909_311_024_04, 0.053_223_900_006_369_534, 0.065_694_267_566_481_78, 0.088_912_196_070_356_03, 0.133_639_567_771_671_66, 0.164_808_886_151_564_28, 0.207_426_005_314_510_4],
    [-0.637_061_715_320_067_3, -0.155_392_967_322_560_64, -0.120_674_418_444_609_37, -0.086_212_735_181_408_83, -0.061_309_908_572_589_78, -0.039_418_727_282_423_23, -0.018_713_506_572_025_3, 0.001_297_156_416_489_487_3, 0.016_866_065_837_449_665, 0.027_880_765_233_150_567, 0.037_717_283_584_632_735, 0.047_050_013_246_011_484, 0.057_444_524_715_702_446, 0.069_748_425_485_492_86, 0.085_999_554_666_873_99, 0.111_837_248_303_310_13, 0.515_819_375_439_43],
    [-0.861_505_904_184_794_1, -0.157_088_955_932_812_25, -0.119_741_566_168_575_16, -0.083_777_390_422_566_4, -0.056_422_163_659_365_39, -0.033_155_191_233_003_09, -0.009_443_967_091_206_436, 0.012_399_567_000_372_277, 0.029_805_104_811_291_65, 0.042_867_484_133_380_145, 0.055_514_349_615_552_495, 0.069_247_230_901_282_18, 0.086_546_282_881_154_18, 0.112_481_367_073_468_88, 0.164_405_715_854_265_44, 0.292_763_654_811_761_14, 0.721_112_853_970_942_3],
    [-0.861_505_904_184_794_1, -0.571_564_036_421_215_9, -0.336_078_908_381_154_7, -0.158_536_624_870_606_19, -0.094_309_374_294_118_55, -0.052_314_467_136_378_925, -0.019_412_070_691_238_23, 0.013_576_069_751_368_01, 0.046_039_990_359_231_765, 0.074_016_225_596_095_51, 0.108_274_980_156_110_42, 0.157_937_076_915_429_05, 0.219_889_871_455_816_48, 0.325_258_755_464_339_74, 0.491_679_161_322_253_5, 0.592_105_315_070_231_7, 0.792_722_916_675_189_5],
    [-0.861_505_904_184_794_1, -0.611_312_504_381_347_9, -0.494_721_594_742_888_6, -0.351_882_393_643_384_36, -0.230_204_436_698_853_68, -0.133_934_865_217_096_4, -0.068_995_104_982_396_22, -0.029_418_443_055_218_44, 0.008_992_480_999_985_349, 0.051_851_058_119_182_17, 0.098_568_921_246_398_13, 0.160_468_163_023_735_43, 0.239_182_869_305_754_25, 0.404_830_638_036_632_5, 0.572_139_842_128_668_4, 0.644_679_340_445_799_3, 0.827_415_028_731_787_8],
    [-0.861_505_904_184_794_1, -0.592_711_023_070_911_5, -0.494_416_172_207_053_6, -0.384_043_058_462_176_5, -0.296_072_777_164_259, -0.208_934_568_578_837_33, -0.131_351_721_995_468_3, -0.069_227_995_815_282_58, -0.025_718_719_359_985_622, 0.017_964_621_704_656_153, 0.063_486_894_233_492_9, 0.125_895_504_096_590_28, 0.205_097_921_512_939_6, 0.344_051_526_927_005_86, 0.595_353_457_223_924_6, 0.678_107_965_963_273_2, 0.875_640_017_043_210_8],
    [-0.744_617_776_907_598_4, -0.550_546_864_067_166, -0.462_840_967_812_248_76, -0.374_755_787_120_632_7, -0.303_337_520_260_140_05, -0.235_650_235_499_591_5, -0.164_510_609_572_862_1, -0.100_121_621_554_268_44, -0.046_421_131_194_640_35, 0.002_310_915_332_321_135, 0.050_645_643_604_480_54, 0.110_685_322_209_079_99, 0.181_171_670_441_567_9, 0.298_661_013_305_326_24, 0.609_468_441_805_316_8, 0.709_967_620_221_736_1, 0.875_640_017_043_210_8],
    [-0.655_901_122_875_969_2, -0.507_892_375_717_305, -0.420_626_726_094_710_4, -0.350_171_079_542_667_3, -0.282_562_488_524_294_7, -0.225_116_322_916_184_72, -0.151_674_875_376_035_02, -0.092_979_598_996_004_01, -0.042_343_487_650_184_77, 0.013_399_888_099_779_04, 0.059_782_754_236_290_82, 0.113_188_688_440_452_32, 0.167_402_610_045_353_8, 0.246_791_445_664_313_63, 0.408_005_486_750_868_6, 0.737_646_377_417_507_7, 0.900_042_825_736_453],
    [-0.614_938_061_661_100_7, -0.521_537_303_335_577_4, -0.438_738_776_275_097_6, -0.362_208_088_814_235_33, -0.277_829_705_765_466_8, -0.224_909_354_598_581_44, -0.168_598_001_592_927_4, -0.110_220_261_502_171_39, -0.062_664_911_996_005_75, -0.007_983_686_698_911_585, 0.050_650_163_203_313_88, 0.111_877_964_984_767_11, 0.193_784_260_918_149_8, 0.309_234_766_893_447_64, 0.636_351_279_546_378_8, 0.708_084_652_531_120_6, 0.900_042_825_736_453],
    [-0.910_628_070_293_452_6, -0.561_343_045_184_652_3, -0.481_205_928_634_690_47, -0.415_921_009_874_464_06, -0.338_710_725_069_315_43, -0.261_848_826_925_183_1, -0.198_176_094_923_488_92, -0.132_115_006_009_241_06, -0.075_098_971_399_903_96, -0.006_567_152_408_242_213, 0.091_265_280_086_450_95, 0.217_004_518_332_980_1, 0.376_275_888_785_151_9, 0.554_156_092_228_497_5, 0.630_895_220_801_183_1, 0.682_003_340_560_781_4, 0.900_042_825_736_453],
    [-0.923_278_113_627_055_7, -0.714_101_586_772_271_4, -0.594_201_542_248_106_3, -0.520_588_314_837_836_7, -0.442_316_021_912_612_05, -0.363_076_128_433_593_5, -0.274_499_984_732_757_05, -0.176_071_821_809_625_44, -0.079_486_755_540_811_53, 0.028_619_088_714_228_153, 0.100_149_817_830_052_9, 0.160_216_243_916_694_42, 0.261_804_179_619_932_85, 0.390_708_855_213_265_65, 0.541_235_037_323_449_6, 0.635_863_321_572_936_2, 0.900_042_825_736_453],
    [-0.923_278_113_627_055_7, -0.805_393_590_069_314_9, -0.710_096_694_622_721_5, -0.633_895_674_250_225_9, -0.548_497_125_011_335_4, -0.452_176_541_574_100_5, -0.334_913_480_482_657_95, -0.172_642_222_918_569_6, -0.013_389_275_101_934_38, 0.036_805_109_486_141_935, 0.068_688_427_865_116_07, 0.095_296_565_943_499_35, 0.121_265_310_837_832_26, 0.155_138_584_012_946_1, 0.236_481_486_549_345_62, 0.417_381_359_063_688_84, 0.783_101_321_676_087_6],
    [-0.923_278_113_627_055_7, -0.804_976_149_893_554_1, -0.719_529_357_376_281_8, -0.647_273_071_580_178, -0.542_686_537_985_540_8, -0.411_111_745_181_934_55, -0.164_085_821_814_375_25, -0.025_259_387_687_837_722, 0.009_020_756_961_874_012, 0.032_752_523_686_171_195, 0.051_885_494_796_943_54, 0.069_005_381_998_298_9, 0.086_275_912_871_027_48, 0.103_535_956_864_270_65, 0.124_974_205_799_540_31, 0.158_281_429_974_538_17, 0.697_441_192_097_338_7],
    [-0.923_278_113_627_055_7, -0.760_929_834_235_527_4, -0.687_657_496_740_836_8, -0.567_179_806_055_405_6, -0.369_627_621_610_097_16, -0.123_818_729_816_878_46, -0.037_902_796_271_615_3, -0.007_972_003_104_244_825, 0.013_622_412_675_494_16, 0.030_758_930_687_309_248, 0.044_771_159_530_483_22, 0.059_767_015_970_463_015, 0.073_275_956_888_444_22, 0.087_490_916_716_655_92, 0.103_726_399_055_034_61, 0.125_481_069_306_093_76, 0.244_722_439_047_928_56],
    [-0.866_795_925_044_015_6, -0.721_407_386_834_222_3, -0.609_055_737_862_099, -0.375_213_791_320_379_96, -0.161_134_722_944_152_93, -0.063_012_271_865_418_44, -0.026_518_484_285_202_287, -0.004_625_131_526_514_061, 0.012_886_982_424_175_212, 0.029_529_615_015_938_763, 0.041_951_632_049_096_826, 0.055_464_763_037_590_01, 0.068_777_864_405_591_1, 0.082_945_748_465_825_43, 0.096_620_386_857_654_6, 0.118_287_755_550_177, 0.259_524_501_680_033_6],
    [-0.746_145_322_413_733_8, -0.208_539_060_999_568_65, -0.143_126_156_964_568_8, -0.115_361_595_370_573_14, -0.096_754_994_110_600_43, -0.073_357_625_824_225_45, -0.054_020_292_124_890_69, -0.043_338_331_133_848_8, -0.029_945_231_125_725_67, -0.008_657_768_682_039_437, 0.005_352_988_701_392_842, 0.025_784_416_833_892_49, 0.049_392_845_103_016_54, 0.072_950_137_663_676_58, 0.104_392_176_224_202_9, 0.138_030_681_133_780_11, 0.259_524_501_680_033_6],
    [-0.573_055_826_314_676_6, -0.132_996_991_771_996_9, -0.110_621_900_484_779_57, -0.091_704_046_800_354_1, -0.068_306_592_740_624_13, -0.052_166_301_890_518_19, -0.037_993_935_325_035_935, -0.023_826_260_692_675_21, -0.006_342_644_855_005_54, 0.006_360_931_457_086_182_5, 0.020_415_810_437_180_24, 0.037_608_333_991_708_855, 0.055_679_870_564_157_21, 0.074_472_181_221_121_81, 0.100_328_833_807_778_46, 0.128_154_931_918_716_55, 0.266_824_839_822_740_25],
    [-0.272_256_341_627_357_23, -0.115_873_028_990_047_09, -0.099_424_155_351_307_18, -0.073_548_767_301_003_03, -0.054_534_995_251_237_374, -0.036_763_561_405_711_236, -0.023_961_888_012_557_19, -0.008_137_153_608_576_763, 0.004_188_557_203_081_694_6, 0.016_297_544_619_685_193, 0.029_202_054_662_327_732, 0.044_152_656_856_032_3, 0.058_820_371_812_314_29, 0.075_128_320_623_713_45, 0.099_071_602_801_430_57, 0.126_262_810_240_843_96, 0.473_084_056_728_986_24],
    [-0.404_818_465_305_281_1, -0.132_907_532_837_976_68, -0.091_944_214_862_267_77, -0.059_807_589_831_074_065, -0.040_561_144_686_465_454, -0.024_941_342_512_717_554, -0.012_317_572_164_088_396, -0.000_691_676_170_556_302_4, 0.012_210_329_383_299_285, 0.021_904_268_936_564_417, 0.031_977_747_152_625_69, 0.040_665_609_647_827_94, 0.053_670_572_337_408_82, 0.070_604_868_331_646_03, 0.093_806_540_257_521_69, 0.112_964_440_364_729_98, 0.157_443_444_494_045_8],
    [-0.489_459_348_960_455_3, -0.188_710_549_643_300_28, -0.132_693_250_061_870_85, -0.097_358_263_236_773_82, -0.071_673_326_173_410_17, -0.049_688_477_360_898_84, -0.031_427_076_683_186_82, -0.014_145_704_791_999_055, 0.000_303_964_533_692_508_36, 0.014_762_823_220_986_652, 0.027_156_641_192_364_44, 0.040_154_206_843_962_27, 0.052_628_026_405_214_474, 0.066_304_878_075_012_17, 0.082_510_996_158_390_14, 0.113_969_854_408_330_9, 0.173_614_719_393_446_75],
    [-0.679_212_972_270_764_9, -0.248_734_410_295_691_7, -0.171_393_911_212_093_8, -0.127_128_633_910_812_58, -0.094_079_411_289_412_16, -0.069_097_603_255_837_82, -0.045_723_208_613_958_65, -0.023_547_181_982_915_767, -0.003_349_268_782_950_497_7, 0.014_320_948_091_168_536, 0.031_586_599_396_764_57, 0.046_482_497_008_746_78, 0.060_639_825_190_815_53, 0.074_803_323_612_515_79, 0.086_842_277_512_641_3, 0.123_144_546_804_514_41, 0.184_286_029_331_717_67],
    [-0.717_998_580_788_911_8, -0.312_202_857_684_433_8, -0.211_674_489_738_016_3, -0.154_643_726_552_438_5, -0.112_549_641_106_844_28, -0.079_874_388_863_491_81, -0.052_508_762_693_055_01, -0.027_817_069_937_906_527, -0.000_331_420_134_182_285_86, 0.020_705_841_960_310_233, 0.038_992_391_466_933_965, 0.056_095_617_765_921_33, 0.071_329_993_379_999_33, 0.083_577_506_816_379_2, 0.098_727_217_199_866_93, 0.136_932_466_112_324_82, 0.195_610_620_041_486_88],
    [-0.816_446_915_607_873_1, -0.349_002_314_131_627_13, -0.250_870_077_628_458_03, -0.177_800_596_121_130_8, -0.124_082_501_989_114_77, -0.085_427_517_438_034_4, -0.053_013_860_226_784_386, -0.027_337_823_261_786_697, 0.002_747_445_810_779_027, 0.025_420_004_706_130_28, 0.044_271_443_095_163_8, 0.062_963_104_494_242_61, 0.079_709_201_947_592_01, 0.092_807_798_862_492_78, 0.107_615_039_474_309_27, 0.145_588_149_145_533_84, 0.210_665_869_428_018_13],
    [-0.940_600_900_692_532, -0.388_983_706_991_164_5, -0.276_453_803_637_824_46, -0.194_081_006_845_287_1, -0.138_683_438_774_111_1, -0.094_849_470_758_858_18, -0.056_519_039_387_923_42, -0.027_808_668_392_572_46, 0.001_663_096_303_985_190_7, 0.026_890_925_936_666_834, 0.047_403_205_628_474_505, 0.067_541_364_222_027_18, 0.084_852_678_698_857_2, 0.099_196_751_598_899_12, 0.113_868_594_460_634_23, 0.145_475_306_806_695_04, 0.221_720_550_573_786_92],
    [-1.062_524_535_896_873_3, -0.436_736_268_921_402_6, -0.305_357_464_626_597_74, -0.214_040_779_025_147_6, -0.153_146_997_342_268_08, -0.105_281_108_902_676_17, -0.064_916_709_495_551_33, -0.029_747_203_666_109_077, 0.000_443_387_122_642_478_1, 0.028_719_185_949_146_513, 0.052_136_921_948_654_12, 0.072_803_850_504_664_26, 0.091_773_580_222_812_83, 0.107_904_927_888_206_44, 0.123_656_587_563_706_15, 0.150_348_433_754_841_3, 0.234_190_031_624_370_58],
    [-1.113_773_480_226_772_3, -0.476_262_973_001_637_3, -0.333_771_290_294_549_05, -0.242_174_057_846_426_73, -0.169_206_804_713_522_45, -0.117_509_420_827_123_54, -0.071_228_802_201_635_6, -0.033_593_437_367_963_41, 9.026_932_239_816_827e-5, 0.031_550_524_019_276_32, 0.056_675_323_591_584_59, 0.078_519_429_709_203_23, 0.098_751_565_490_676_87, 0.117_487_003_567_184_76, 0.133_552_838_949_131_28, 0.155_088_428_470_951_83, 0.242_423_972_099_301_1],
    [-1.199_467_107_770_656_7, -0.497_818_359_801_853_4, -0.353_362_325_356_194_1, -0.263_439_075_892_370_14, -0.187_436_996_254_573_05, -0.127_062_921_842_739_8, -0.077_506_844_044_339_91, -0.037_924_421_858_201_51, -0.002_535_238_034_744_019_7, 0.031_089_074_852_880_79, 0.057_905_627_691_219_3, 0.081_790_089_579_978_9, 0.104_392_391_360_596_64, 0.122_747_810_589_455_68, 0.139_387_093_149_950_5, 0.157_780_100_545_178_7, 0.243_986_099_716_661_2],
    [-1.308_043_316_530_099_5, -0.505_533_273_062_931_3, -0.356_061_537_551_609_1, -0.265_824_723_436_328_9, -0.191_288_707_174_804_95, -0.130_020_308_850_346_24, -0.079_664_650_935_246_19, -0.039_524_074_028_046_76, -0.003_362_503_181_977_174, 0.030_714_851_588_606_734, 0.059_811_366_821_826_55, 0.084_715_412_824_267_59, 0.108_377_724_155_692_86, 0.126_670_241_556_746_55, 0.143_706_230_300_177_93, 0.160_775_882_777_677_4, 0.243_986_099_716_661_2],
    [-1.453_544_435_862_257_3, -0.507_641_961_319_276_1, -0.348_584_672_229_132_25, -0.258_064_933_463_600_04, -0.184_145_984_316_535_17, -0.124_966_137_807_687_21, -0.077_444_791_833_808_59, -0.037_056_365_396_670_19, -0.001_641_751_554_978_047, 0.030_577_127_007_483_915, 0.060_141_862_811_070_715, 0.085_154_277_889_691_39, 0.108_657_313_001_005_13, 0.127_771_850_956_293_62, 0.145_207_453_515_861_56, 0.162_535_992_088_199_13, 0.243_986_099_716_661_2],
    [-1.520_074_638_783_963_6, -0.490_545_650_440_942_06, -0.338_699_683_043_857, -0.245_767_548_157_327_26, -0.171_679_515_437_829_7, -0.119_425_893_451_864_77, -0.075_073_084_345_225_36, -0.035_338_630_084_573_98, -0.000_473_652_021_644_684_7, 0.030_067_458_242_535_594, 0.056_405_669_665_958_204, 0.080_938_490_690_358_59, 0.103_768_523_630_046_61, 0.123_254_761_288_267_01, 0.141_104_608_127_078_5, 0.161_603_823_157_430_52, 0.243_986_099_716_661_2],
    [-1.520_074_638_783_963_6, -0.462_753_815_309_668_97, -0.320_095_955_772_537_24, -0.226_179_193_399_586_06, -0.159_350_395_550_460_46, -0.111_489_017_989_177_5, -0.068_794_649_359_124_76, -0.032_234_260_202_758_554, 0.000_861_033_482_331_646, 0.031_853_200_721_698_435, 0.055_231_753_457_774_105, 0.077_859_734_302_839_59, 0.098_527_177_663_482_15, 0.117_782_283_515_769_29, 0.133_441_848_590_614_42, 0.159_967_879_112_590_8, 0.243_578_573_673_492_12],
    [-1.852_145_654_985_062, -0.440_339_925_540_735_26, -0.305_393_095_518_349_76, -0.218_244_717_333_276_24, -0.152_882_478_743_748_4, -0.105_694_686_251_656_61, -0.064_460_320_553_737_18, -0.028_373_591_445_416_863, 0.003_277_798_701_610_826, 0.032_767_888_060_905_58, 0.055_328_678_511_275_105, 0.077_871_569_221_576_05, 0.097_640_513_702_202_49, 0.114_068_351_254_551_13, 0.127_429_170_449_770_37, 0.158_591_993_090_261_04, 0.232_963_999_320_565_66],
    [-1.852_145_654_985_062, -0.443_500_202_614_186, -0.307_104_358_777_815_7, -0.216_076_796_267_444_4, -0.150_150_072_038_657_1, -0.103_222_200_498_656_81, -0.063_353_678_899_538_26, -0.028_298_521_449_888_023, 0.000_607_109_904_386_793, 0.029_213_577_204_994_255, 0.052_981_200_348_616_32, 0.074_400_625_099_599_5, 0.094_151_685_048_979_01, 0.110_250_561_337_212_91, 0.123_615_103_298_899_6, 0.155_476_267_417_226_28, 0.224_884_766_104_850_1],
    [-1.852_145_654_985_062, -0.448_173_207_879_082_8, -0.306_589_667_675_813_75, -0.210_619_797_981_867_64, -0.150_135_099_593_076_05, -0.100_831_473_915_450_1, -0.059_946_489_913_906_35, -0.028_302_393_958_291_188, -0.001_291_130_392_277_173_8, 0.024_905_390_091_023_28, 0.047_834_954_150_666_25, 0.067_847_970_904_246_07, 0.086_961_303_906_574_39, 0.102_466_760_983_737_43, 0.116_506_673_273_562_3, 0.146_582_228_498_739_28, 0.213_493_673_624_610_03],
    [-1.852_145_654_985_062, -0.415_878_440_901_584_55, -0.264_898_727_129_726_1, -0.187_683_041_082_387_6, -0.129_353_715_795_401_06, -0.081_101_562_106_461_16, -0.045_129_341_046_536_97, -0.019_995_586_506_522_875, 0.005_922_241_478_767_976, 0.028_607_677_665_426_7, 0.048_967_373_420_142_95, 0.064_250_047_524_764_85, 0.083_638_179_332_973_14, 0.096_679_541_527_589_02, 0.109_958_217_453_048_32, 0.128_485_348_824_602_5, 0.208_462_417_022_051_48],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_27: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.501_074_883_678_082, 10.0),
    (-12.166_812_191_242_72, -9.620_029_993_453_509),
    (-12.796_115_715_565_63, -10.126_995_841_024_593),
    (-12.879_745_495_116_444, -9.361_260_374_613_353),
    (-0.180_455_365_677_601_65, 1.407_648_545_125_163_7),
    (-0.291_485_340_957_659_4, 0.281_121_923_344_690_6),
    (1.647_633_142_541_790_3, 2.698_845_169_412_556_4),
    (0.354_898_490_315_634_7, 1.392_508_367_046_286_8),
    (-2.165_111_255_070_305, 0.055_173_581_931_575_68),
    (0.062_919_529_664_505_13, 0.848_224_854_290_727_1),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_27: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.117_935_604_384_116_7, 1.114_487_032_325_637_7, 1.090_201_752_195_308_5, 1.051_444_206_459_751_1, 1.010_383_950_401_399_8, 0.633_938_722_316_630_4, 0.627_083_117_046_261, 0.986_820_659_620_724_1, 0.805_986_486_550_640_5, 0.621_751_340_094_324_4, 0.582_719_766_060_352_6, 0.573_228_844_156_325_7, 0.572_978_884_035_457_7, 0.576_235_881_087_072_6, 0.575_274_732_062_552_6, 1.049_750_231_613_523_4, 1.050_066_615_127_812_6],
    [0.226_133_185_437_730_8, 0.206_567_909_081_818_93, 0.186_188_406_785_474_77, 0.161_585_699_714_502_43, 0.135_626_516_868_201_7, 0.108_556_624_986_947_35, 0.090_058_866_014_508_38, 0.101_350_708_340_125_45, 0.512_426_777_866_007_5, 0.467_925_670_638_328_5, 0.332_442_724_931_172_7, 0.263_227_989_166_130_27, 0.271_363_116_172_021_1, 0.308_232_080_609_249_7, 0.366_679_041_619_352_9, 1.040_557_638_599_908_2, 1.064_640_638_195_107_7],
    [0.252_539_781_304_446_15, 0.235_521_139_482_789_22, 0.209_895_005_830_864_2, 0.176_866_901_602_481_25, 0.139_621_736_867_567_63, 0.104_196_592_561_112_92, 0.069_594_414_962_660_21, 0.030_351_389_546_930_972, -0.008_457_507_620_124_757, -0.039_831_529_360_759_73, -0.078_615_658_567_338_74, -0.113_460_615_678_848_52, -0.148_107_741_417_022_74, -0.175_763_620_338_672_2, -0.211_482_541_674_388_15, -0.256_547_343_739_451_87, -0.291_497_488_660_539_1],
    [2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.170_993_063_458_994_2, 1.940_994_128_523_133_6, 1.931_390_963_629_126_2, 1.974_869_796_795_872, 2.039_038_539_925_477, 2.092_289_835_713_160_5],
    [-1.290_719_882_578_411_6, -1.420_678_625_516_433_2, -1.538_725_255_845_76, -1.618_876_175_614_689_7, -1.633_187_644_775_155, -0.809_322_206_588_348_2, -0.685_461_398_446_846_4, -0.639_990_370_036_259_2, -0.608_174_079_605_855_9, -0.576_120_526_201_756_7, -0.548_846_447_679_690_7, -0.500_185_599_964_211_2, -0.003_302_503_925_102_202_8, 0.013_625_966_377_394_21, 0.026_680_260_916_817_338, 0.032_430_179_857_120_1, -0.488_979_343_546_308_4],
    [-1.611_852_982_398_538_4, -1.649_091_439_165_757_4, -1.689_593_477_648_851_3, -1.727_882_958_316_754_8, -1.758_834_142_543_077_4, -1.796_676_045_548_839_5, -1.835_610_723_641_158_3, -1.871_927_494_917_804, -1.905_246_869_869_773, -1.941_558_565_532_269_6, -1.981_585_254_284_951, -2.015_277_271_917_753, -2.050_962_810_368_239, -2.087_779_957_179_765, -2.119_653_051_434_937, -2.144_868_881_926_853_5, -2.163_182_901_645_856_6],
    [-0.596_108_799_753_416_4, -0.596_108_799_753_416_4, -0.597_581_038_254_729_6, -0.609_286_255_793_812_6, -0.618_886_125_247_266_6, -0.625_080_083_250_130_5, -0.643_708_263_304_739_6, -0.672_109_836_374_504, -0.700_775_818_160_529_3, -0.704_106_937_062_799_6, -0.703_176_204_975_496_5, -0.696_781_197_424_017_6, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1],
    [0.138_724_966_591_984_63, 0.177_618_502_591_340_27, 0.283_968_526_388_825_3, 0.330_473_651_685_377_6, 0.380_085_019_574_329_7, 0.446_961_557_224_955_23, 0.483_180_487_479_111_7, 0.527_723_498_191_182_6, 0.578_945_047_362_877_6, 0.505_845_632_162_852_8, 0.476_429_207_704_293_7, 0.501_987_152_647_045_1, 0.554_032_977_435_809_9, 0.614_543_121_726_524_5, 0.677_729_110_205_994_4, 0.721_060_211_279_945_1, 0.726_387_689_201_915_3],
    [0.288_168_932_150_122_24, 0.241_104_809_428_741_07, -0.141_557_035_725_450_2, -0.345_884_157_488_926_1, -0.430_980_364_344_758_45, -0.499_737_632_590_354_7, -0.550_665_039_341_276_5, -0.538_044_703_413_578, -0.449_230_818_035_189_16, -0.335_810_257_754_727_45, 0.148_216_300_391_751_07, 0.228_406_258_277_448_8, 0.246_659_549_165_114_1, 0.253_225_165_903_769, 0.108_874_587_885_642_46, 0.137_038_553_499_730_43, 0.147_671_682_097_315_74],
    [9.957_408_470_516_405, 9.934_699_131_470_515, 9.913_604_854_093_151, 9.902_398_736_805_894, 9.892_369_957_189_192, 9.879_706_959_481_314, 9.866_584_866_578_167, 9.854_959_690_613_024, 9.842_391_833_134_414, 9.834_662_796_913_012, 9.835_316_869_894_449, 9.841_128_586_362_608, 9.851_894_243_022_38, 9.864_162_182_681_103, 9.869_449_624_406_773, 9.872_190_098_018_615, 9.882_781_519_981_215],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_27: [(f64, f64); 11] = [
    (5.419_185_498_579_54, 10.0),
    (0.320_048_993_757_405_53, 1.38),
    (-0.278_456_376_545_300_03, 1.971_095_891_507_528_3),
    (-0.783_839_570_029_425_4, 0.359_480_989_581_144_33),
    (1.506_939_585_167_018_6, 6.442_488_747_737_999_5),
    (-1.913_579_116_770_117_7, 0.209_409_169_898_635),
    (-2.264_699_308_176_646, 0.243_432_298_064_600_95),
    (-0.769_398_248_555_516_1, -0.125_262_481_778_891_35),
    (-0.046_532_352_103_877_81, 0.909_956_226_855_928_4),
    (-1.096_848_077_448_504_3, 0.620_755_738_826_302),
    (8.017_791_846_134_891, 10.079_132_695_443_889),
];

/// The slow accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_DONOR_KINDS_27: [f64; 3] = [
    0.891_475_729_148_437_2,
    0.107_710_428_459_213_54,
    0.000_813_842_392_349_314,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    1.064_061_937_644_029_7e-7,
    7.631_717_778_889_69e-6,
    1.437_745_834_857_519_5e-6,
    1.743_236_554_517_921_6e-6,
    2.680_708_364_919_044e-6,
    2.687_469_143_536_8e-6,
    4.596_419_042_158_226_5e-7,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.107_500_734_303_950_19,
    0.868_374_128_670_013_4,
    0.024_125_137_026_036_535,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    3.338_030_896_214_244_7e-7,
    6.931_782_964_381_493e-5,
    0.000_109_970_723_913_020_92,
    9.262_869_513_933_568e-5,
    8.253_048_789_357_352e-5,
    8.428_092_105_255_645e-5,
    0.000_123_467_424_984_871,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.378_091_638_579_261_1,
    0.607_825_453_741_839_3,
    0.014_082_907_678_899_683,
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
// sim-fingerprint: 5a071611475533bb2b16ef5be3305f0f52fb6d60273009b86acf833166d133fe
// since-generator-version: 27
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the total against the brainstorm's 10⁴, the black-hole transients pinned at
//   Corral-Santana et al.'s (2016, A&A 587, A61) observed 1,300
// acceptance: slab members by kind 274/7347/5/67/9/0/968/4/0; young members by kind
//   423/288/96/133/1046/6/128/0/8; census kinds
//   0.0315/0.8849/0.0005/0.0060/0.0012/0.0000/0.0755/0.0003/0.0000; weights
//   0.0276/0.7746/0.0004/0.1300/0.0661/0.0003 and 0.0010/0.0000/0.0000; black-hole transients
//   1300 of 1e4 (the census's 60); high-mass hosts 10; donor mass's own nodes 13; layers C 0.000
//   D 0.240 E 0.760; rates a system 1.49e-4/1.86e-4/1.17e-4/1.17e-4/1.32e-4/1.27e-4/3.29e-5;
//   scratch marks: NeutronStarRecurrence, BlackHoleRecurrence, SupergiantPeriod,
//   SupergiantDonorMass, SupergiantRate, SupergiantRecoil, SupergiantSinceSupernova,
//   UltracompactRecurrence, SymbioticPeriod, SymbioticDonorMass, SymbioticFilling,
//   BlackHoleHighMassAccretorMass, BlackHoleHighMassPeriod, BlackHoleHighMassDonorMass,
//   BlackHoleHighMassRate; the K–S tests against the other half of the census are the sim's slow
//   `fitted_tables::the_x_ray_binary_sampler_against_the_engine`
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
    [1.164_948_487_059_780_8, 1.268_021_545_188_498, 1.281_610_049_762_752_3, 1.293_073_547_904_374_5, 1.303_957_729_543_086_9, 1.314_670_181_796_624, 1.324_907_837_735_311_4, 1.336_822_965_108_258_8, 1.350_176_375_241_923_1, 1.365_817_958_602_859_8, 1.381_694_867_310_580_4, 1.399_257_295_080_483, 1.508_292_593_359_879_4, 1.574_518_298_453_745, 1.632_084_803_294_025_6, 1.716_830_798_174_284_5, 1.996_852_964_720_374_8],
    [2.001_022_308_104_295_7, 2.260_329_488_630_704, 2.405_854_133_999_657, 2.601_498_967_319_422, 2.747_115_108_576_725_4, 2.970_382_637_238_697, 3.026_433_577_644_421_6, 3.089_678_673_767_605_4, 3.174_519_849_645_016_7, 3.279_000_587_133_749_5, 3.432_471_695_011_089, 3.652_776_501_414_633_6, 4.005_809_801_474_093, 5.081_404_501_452_646_5, 5.493_278_903_571_973, 6.087_332_027_610_373, 8.787_813_035_339_518],
    [-0.365_939_117_930_022_2, 0.056_166_668_303_526_936, 0.137_125_835_675_200_26, 0.182_765_686_740_231, 0.214_561_110_493_660_86, 0.237_642_851_487_947_98, 0.313_336_939_929_076, 0.380_714_597_979_628_14, 0.437_938_534_851_382_9, 0.488_130_348_572_091_53, 0.540_326_131_065_720_2, 0.628_366_411_984_533, 0.979_180_199_114_559_3, 1.172_324_332_793_174_7, 1.352_559_296_416_171_7, 1.581_225_132_394_336, 3.229_107_781_829_91],
    [-0.909_776_566_129_924_6, -0.442_460_888_689_513, -0.301_264_553_335_145_24, -0.167_683_077_449_680_9, -0.086_140_179_066_314_56, -0.043_889_174_106_320_49, -0.010_981_513_470_360_122, 0.008_867_322_929_916_434, 0.024_248_013_075_943_382, 0.041_793_086_606_155_73, 0.063_128_766_447_470_95, 0.087_374_622_099_954_45, 0.111_391_588_795_496_67, 0.139_818_063_313_364_85, 0.191_103_092_660_864_9, 0.328_349_395_700_794_24, 0.780_018_575_195_007_6],
    [-5.596_358_743_396_394, -2.275_513_817_712_313_5, -2.108_047_738_049_987_4, -1.923_401_080_791_588, -1.742_769_012_555_939_9, -1.580_708_848_899_377_2, -1.467_444_669_946_572_9, -1.373_635_817_812_542, -1.277_750_261_476_392_3, -1.170_129_707_110_152_6, -1.071_569_750_566_599_5, -0.967_344_557_000_922_6, -0.880_837_359_055_672_5, -0.799_917_434_498_952_7, -0.676_459_938_655_814_3, -0.411_835_721_078_538_7, 3.655_714_600_432_353_3],
    [1.75, 2.254_32, 2.435_993_5, 2.563_591, 2.667_624_5, 2.758_895_5, 2.842_726, 2.922_329_5, 3.0, 3.077_670_5, 3.157_274, 3.241_104_5, 3.332_375_5, 3.436_409, 3.564_006_5, 3.745_68, 4.25],
    [1.25, 1.754_319_999_999_999_9, 1.935_993_5, 2.063_591, 2.167_624_5, 2.258_895_5, 2.342_726, 2.422_329_5, 2.5, 2.577_670_5, 2.657_274, 2.741_104_5, 2.832_375_5, 2.936_409, 3.064_006_5, 3.245_68, 3.75],
    [0.355_095_299_505_206_4, 1.169_761_756_034_391_8, 1.384_340_137_839_243_3, 1.539_236_117_653_588_5, 1.632_562_181_348_657_5, 1.714_226_918_262_311_8, 1.795_349_541_787_276_2, 1.881_642_941_775_921_5, 1.949_343_254_163_796_5, 2.010_934_172_374_756_5, 2.058_830_056_263_069_3, 2.120_922_449_015_254, 2.211_198_163_226_160_6, 2.284_090_752_455_546_3, 2.370_760_126_456_989, 2.524_860_045_726_495_4, 2.984_609_187_822_059_5],
    [0.004_937_568_651_288_154, 0.023_328_197_862_470_46, 0.032_917_698_367_333_3, 0.042_882_995_487_206_98, 0.051_403_500_363_037_49, 0.059_244_524_222_571_544, 0.067_485_174_614_213_37, 0.076_231_023_373_804_26, 0.085_742_664_915_565_09, 0.094_462_784_814_699_69, 0.104_387_003_378_470_89, 0.115_423_180_761_273_39, 0.130_718_355_455_854_83, 0.153_346_702_698_681_92, 0.189_303_749_725_468_65, 0.249_058_407_691_686_02, 0.959_466_521_442_529_1],
    [8.009_209_165_630_322, 8.380_453_710_410_395, 8.770_123_447_567_586, 9.135_237_012_633_107, 9.460_244_155_156_348, 9.768_406_525_487_56, 10.029_991_211_629_05, 10.351_734_846_096_8, 10.673_741_923_197_829, 11.056_728_272_915_91, 11.481_577_655_795_57, 11.904_715_258_394_884, 12.499_130_223_375_852, 13.161_864_416_236_051, 13.895_709_191_694_083, 15.495_319_050_795_388, 27.169_795_023_354_9],
    [-0.150_000_000_000_000_02, 0.152_592, 0.261_596_1, 0.338_154_599_999_999_97, 0.400_574_699_999_999_95, 0.455_337_299_999_999_94, 0.505_635_6, 0.553_397_7, 0.6, 0.646_602_3, 0.694_364_4, 0.744_662_7, 0.799_425_3, 0.861_845_4, 0.938_403_9, 1.047_408, 1.35],
    [10.0, 14.034_559_999_999_999, 15.487_948, 16.508_728, 17.340_996, 18.071_164, 18.741_808, 19.378_636, 20.0, 20.621_364, 21.258_192, 21.928_836, 22.659_004, 23.491_272, 24.512_052, 25.965_44, 30.0],
    [-10.25, -9.745_68, -9.564_006_5, -9.436_409, -9.332_375_5, -9.241_104_5, -9.157_274, -9.077_670_5, -9.0, -8.922_329_5, -8.842_726, -8.758_895_5, -8.667_624_5, -8.563_591, -8.435_993_5, -8.254_32, -7.75],
    [0.494_476_138_824_081_7, 1.491_948_534_551_346_8, 1.923_745_522_426_236_5, 2.434_575_376_728_411, 2.941_865_847_225_992_4, 3.356_608_474_816_133_6, 3.811_645_608_233_535_5, 4.237_768_882_402_516, 4.875_256_420_266_838, 5.411_367_498_914_832, 5.984_333_496_142_037, 6.551_570_574_957_023, 7.390_931_107_010_407, 8.407_187_832_733_856, 9.922_781_104_399_045, 17.785_911_706_440_924, 59.273_080_342_508_64],
    [0.0, 21.541_471_172_88, 27.883_959_356_160_005, 32.722_835_660_352_004, 36.900_838_535_952_005, 40.732_034_499_504, 44.381_465_974_608_005, 47.957_508_026_832, 51.545_915_694_144_01, 55.228_255_120_512, 59.095_911_217_440_005, 63.268_150_174_896, 67.923_921_544_560_02, 73.375_080_106_944_01, 80.282_901_424_464, 90.636_775_035_600_01, 120.071_697_528_624_01],
    [4.174_421_988_060_552, 6.119_911_536_557_706, 6.298_018_858_984_609, 6.383_898_012_234_356, 6.496_032_917_364_872, 6.596_707_095_802_745, 6.674_675_749_457_917, 6.755_329_621_031_454, 6.845_890_944_157_303, 6.900_333_098_746_479, 6.969_037_135_241_684, 7.032_422_792_947_482, 7.081_159_085_698_664, 7.138_457_283_736_121, 7.199_779_843_993_819, 7.283_856_488_226_333, 7.477_784_022_709_876],
    [5.7, 5.901_728, 5.974_397_4, 6.025_436_4, 6.067_049_8, 6.103_558_2, 6.137_090_4, 6.168_931_8, 6.2, 6.231_068_2, 6.262_909_6, 6.296_441_8, 6.332_950_2, 6.374_563_6, 6.425_602_6, 6.498_272, 6.7],
    [0.575_937_324_311_977_3, 0.792_475_053_444_555_5, 0.864_132_398_315_645_9, 0.884_515_440_000_435_8, 0.903_639_785_154_855_9, 0.918_205_134_891_032_1, 0.936_247_225_852_860_1, 0.953_411_705_301_475_7, 0.970_560_635_886_050_8, 0.987_099_088_141_344_1, 1.004_973_040_269_759_9, 1.020_270_161_964_678_1, 1.034_601_480_235_806_8, 1.048_187_777_491_223, 1.063_666_358_287_282_5, 1.088_000_320_429_133_7, 1.503_229_118_814_494_2],
    [1.036_815_700_130_632_4, 1.044_327_511_963_957_5, 1.056_951_387_018_574_7, 1.075_350_047_798_501_9, 1.078_262_178_171_371_5, 1.082_819_795_944_205_4, 1.090_227_967_654_385_6, 1.106_032_599_191_161_3, 1.110_614_194_162_681_3, 1.117_585_594_105_209_5, 1.131_708_413_575_683_2, 1.144_310_737_989_695_3, 1.214_991_011_483_284_5, 1.297_959_196_893_821_8, 1.346_168_103_605_835_5, 1.372_302_669_204_325_4, 1.629_310_277_259_491_8],
    [-0.000_143_975_019_286_123_4, 0.286_983_412_763_035_8, 0.453_709_434_105_978_8, 0.649_960_986_791_642_4, 0.916_428_523_436_701_1, 1.107_477_427_248_308_3, 1.214_895_490_075_475_1, 1.301_847_130_601_420_3, 1.376_873_868_260_654_5, 1.460_774_879_200_171_1, 1.560_992_210_594_092_3, 1.703_973_624_416_465_8, 2.145_966_848_094_036_4, 2.290_627_219_649_12, 2.388_936_186_119_575, 2.475_914_899_010_767_6, 2.818_822_331_560_381_3],
    [1.282_993_821_330_733_3, 1.580_473_209_186_277_6, 1.662_880_716_346_868, 1.713_587_185_847_890_6, 1.743_418_069_249_431_7, 1.765_185_061_929_958_5, 1.786_356_791_727_159_7, 1.804_636_242_269_647_1, 1.817_502_717_512_929_5, 1.829_821_744_797_288, 1.841_809_283_107_504, 1.854_007_842_966_066_2, 1.862_131_130_781_224_3, 1.871_291_498_749_482_9, 1.879_619_144_014_982, 1.887_858_181_817_592_4, 3.131_830_039_835_625_4],
    [0.0, 0.403_455_999_999_999_9, 0.548_794_8, 0.650_872_799_999_999_9, 0.734_099_6, 0.807_116_4, 0.874_180_8, 0.937_863_6, 1.0, 1.062_136_4, 1.125_819_2, 1.192_883_6, 1.265_900_4, 1.349_127_2, 1.451_205_2, 1.596_544_000_000_000_2, 2.0],
    [0.092_834_707_932_504_24, 0.165_692_745_814_921_55, 0.197_367_496_432_614_45, 0.224_213_056_652_051_32, 0.243_721_718_263_835_5, 0.262_363_860_196_851_9, 0.276_822_380_004_719_8, 0.294_598_743_691_531_83, 0.307_722_903_963_881_7, 0.320_284_315_859_534_4, 0.332_371_962_390_363_2, 0.344_500_360_735_504_83, 0.363_502_981_130_990_2, 0.389_383_443_070_388_75, 0.418_028_717_683_201_4, 0.465_462_580_956_046, 1.138_140_535_555_715_3],
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
    [-0.070_558_146_984_202_93, 0.015_417_025_977_229_732, 0.038_949_369_003_552_81, 0.061_943_930_136_461_72, 0.079_976_161_662_736_08, 0.093_757_644_660_004_5, 0.105_177_034_306_174_62, 0.112_600_362_669_018_11, 0.119_547_593_715_365_76, 0.127_419_388_697_355_06, 0.137_273_533_553_716_7, 0.167_703_516_571_291_12, 0.335_543_471_069_290_3, 0.411_222_134_388_516_4, 0.523_479_236_786_85, 0.629_469_349_814_551_3, 0.759_602_474_838_579],
    [-0.653_824_046_729_955_6, -0.454_741_506_746_513_8, -0.369_709_948_614_924_95, -0.268_229_019_369_071_5, -0.156_684_688_018_382_53, -0.061_156_243_233_651_32, -0.012_646_246_147_860_082, 0.003_909_816_759_610_473, 0.022_924_832_650_435_635, 0.045_057_264_525_677_47, 0.072_414_801_279_134_19, 0.097_237_800_795_742_83, 0.117_449_869_325_575_59, 0.142_842_358_263_224_6, 0.431_508_153_269_743_23, 0.626_090_927_405_801_3, 0.780_018_575_195_007_6],
    [-0.653_824_046_729_955_6, -0.497_521_995_947_647_6, -0.431_557_969_853_333_94, -0.368_565_204_410_342_5, -0.291_953_274_184_224_9, -0.207_226_345_127_926_95, -0.114_771_096_960_602_93, -0.044_508_759_716_019_73, -0.014_839_421_328_626_28, 0.002_765_734_107_948_387, 0.023_275_987_250_695_39, 0.050_261_620_811_339_07, 0.086_890_533_326_136_7, 0.124_121_720_885_671_82, 0.235_283_280_616_846_36, 0.486_093_164_208_739_05, 0.780_018_575_195_007_6],
    [-0.653_824_046_729_955_6, -0.490_304_177_097_029_95, -0.405_588_645_262_241_86, -0.311_500_773_885_044_44, -0.183_846_460_703_227_02, -0.077_114_539_215_036_54, -0.032_900_711_872_150_11, -0.009_656_040_448_301_944, 0.012_428_400_946_801_182, 0.036_270_307_183_278_21, 0.062_012_625_982_343_084, 0.088_770_934_136_819_14, 0.109_677_298_040_300_85, 0.142_649_464_646_741_33, 0.182_750_630_099_559_97, 0.297_114_094_087_058_3, 0.780_018_575_195_007_6],
    [-0.653_824_046_729_955_6, -0.340_223_463_923_047_1, -0.123_923_458_774_930_05, -0.061_992_106_668_467_77, -0.029_435_826_750_173_345, -0.007_343_241_274_453_7, 0.017_823_188_953_509_955, 0.037_998_462_077_328_65, 0.056_286_710_165_489_01, 0.075_863_967_897_175_8, 0.093_773_240_241_306_91, 0.108_378_221_656_423_3, 0.127_244_228_748_332_13, 0.146_256_573_788_766_77, 0.170_084_025_147_603_48, 0.200_398_735_297_088_1, 0.780_018_575_195_007_6],
    [-0.474_176_676_782_913_4, -0.156_518_166_467_659_12, -0.079_019_036_627_298_26, -0.044_494_405_251_687_213, -0.016_044_043_411_622_556, 0.006_496_657_101_097_284, 0.029_029_125_744_578_284, 0.046_589_505_036_151_634, 0.065_892_869_999_184_88, 0.083_556_426_717_948_76, 0.099_436_088_755_235_27, 0.115_301_476_112_990_48, 0.131_303_143_463_713_6, 0.148_484_147_455_002_38, 0.170_393_822_592_904_6, 0.202_569_144_145_740_9, 0.460_109_076_877_063_76],
    [-0.604_520_802_094_064_6, -0.176_168_156_089_202_4, -0.130_927_706_334_388_05, -0.091_826_714_625_267_55, -0.050_387_991_264_239_256, -0.015_998_513_569_011_854, 0.008_690_672_008_714_095, 0.031_187_708_273_706_75, 0.050_585_040_942_917_596, 0.074_328_007_303_455_82, 0.097_606_344_894_531_3, 0.121_790_541_941_853_78, 0.145_441_868_343_072_9, 0.170_067_971_360_104_35, 0.231_447_050_330_586_8, 0.331_060_415_079_367_45, 0.460_109_076_877_063_76],
    [-0.662_584_866_407_975_2, -0.179_593_393_555_909_95, -0.116_913_650_459_015_57, -0.090_615_007_732_039_57, -0.066_055_263_102_311_75, -0.044_390_055_497_942_2, -0.016_075_531_419_746_02, -0.001_797_895_924_329_202_2, 0.010_195_041_899_175_297, 0.022_313_815_139_033_886, 0.033_339_718_489_928_5, 0.042_586_606_526_455_43, 0.057_342_834_199_535_134, 0.081_601_183_134_776_35, 0.147_900_546_163_987_37, 0.239_865_305_114_272_87, 0.460_109_076_877_063_76],
    [-0.734_370_949_861_831_8, -0.206_996_090_205_509_9, -0.112_826_608_216_967_69, -0.081_619_809_432_864_07, -0.064_804_025_011_853_57, -0.046_060_742_468_846_55, -0.011_186_109_637_123_328, 0.001_955_139_933_335_803_4, 0.010_315_419_989_977_634, 0.020_023_430_568_425_32, 0.027_469_971_529_875_375, 0.036_018_338_035_852_394, 0.042_438_499_785_095_556, 0.053_800_976_816_956_954, 0.070_707_881_981_343_8, 0.115_898_610_873_600_83, 0.436_640_985_983_803_2],
    [-0.909_776_566_129_924_6, -0.333_614_507_958_819_64, -0.168_040_216_666_198_76, -0.083_011_399_891_612_28, -0.064_274_831_716_860_16, -0.025_016_311_559_711_908, 0.002_768_501_565_088_391, 0.010_093_925_631_980_102, 0.014_983_647_974_731_363, 0.021_490_596_706_461_442, 0.028_414_458_991_076_377, 0.037_821_735_893_723_1, 0.047_313_154_798_547_266, 0.067_888_774_915_246_4, 0.092_430_322_730_778_09, 0.144_978_633_710_805_16, 0.436_640_985_983_803_2],
    [-0.909_776_566_129_924_6, -0.508_463_763_836_345_8, -0.318_728_034_005_040_9, -0.164_143_076_171_672_65, -0.069_117_742_064_171_08, 0.000_319_919_532_089_688_5, 0.009_606_713_380_395_124, 0.013_140_967_801_795_92, 0.019_839_243_287_978_627, 0.028_155_996_390_779_793, 0.038_987_694_208_022_63, 0.046_358_306_651_397_52, 0.068_537_420_518_923_13, 0.086_197_525_933_738_74, 0.122_008_217_346_668_68, 0.195_914_097_158_057_83, 0.436_640_985_983_803_2],
    [-0.909_776_566_129_924_6, -0.634_537_341_885_528_9, -0.449_431_015_143_400_34, -0.319_592_938_387_955_7, -0.214_219_966_049_165_74, -0.085_589_219_366_898_26, 0.008_800_401_228_716_38, 0.013_676_238_662_363_237, 0.036_600_672_820_966_24, 0.042_658_258_393_462_975, 0.047_020_402_952_552_61, 0.074_111_152_851_808_91, 0.092_253_704_125_895_16, 0.131_626_193_071_341_1, 0.281_598_631_931_076_9, 0.332_114_815_222_208_4, 0.418_813_514_099_166_1],
    [-0.909_776_566_129_924_6, -0.442_460_888_689_513, -0.301_264_553_335_145_24, -0.167_683_077_449_680_9, -0.086_140_179_066_314_56, -0.043_889_174_106_320_49, -0.010_981_513_470_360_122, 0.008_867_322_929_916_434, 0.024_248_013_075_943_382, 0.041_793_086_606_155_73, 0.063_128_766_447_470_95, 0.087_374_622_099_954_45, 0.111_391_588_795_496_67, 0.139_818_063_313_364_85, 0.191_103_092_660_864_9, 0.328_349_395_700_794_24, 0.780_018_575_195_007_6],
    [-0.909_776_566_129_924_6, -0.442_460_888_689_513, -0.301_264_553_335_145_24, -0.167_683_077_449_680_9, -0.086_140_179_066_314_56, -0.043_889_174_106_320_49, -0.010_981_513_470_360_122, 0.008_867_322_929_916_434, 0.024_248_013_075_943_382, 0.041_793_086_606_155_73, 0.063_128_766_447_470_95, 0.087_374_622_099_954_45, 0.111_391_588_795_496_67, 0.139_818_063_313_364_85, 0.191_103_092_660_864_9, 0.328_349_395_700_794_24, 0.780_018_575_195_007_6],
    [-0.726_533_636_550_753_6, -0.290_549_415_351_571_2, -0.229_325_944_286_375_7, -0.179_008_616_870_232_73, -0.156_003_191_796_169_25, -0.130_847_572_974_255_28, -0.108_154_613_421_008_07, -0.074_142_157_405_885_23, -0.036_145_240_490_856_094, -0.016_664_015_919_047_533, 0.008_524_916_117_564_691, 0.046_680_202_141_166_09, 0.079_500_574_698_345_03, 0.133_079_853_679_244_3, 0.236_610_413_644_557_1, 0.488_551_359_401_243_1, 0.517_186_494_530_154_8],
    [-0.909_776_566_129_924_6, -0.442_460_888_689_513, -0.301_264_553_335_145_24, -0.167_683_077_449_680_9, -0.086_140_179_066_314_56, -0.043_889_174_106_320_49, -0.010_981_513_470_360_122, 0.008_867_322_929_916_434, 0.024_248_013_075_943_382, 0.041_793_086_606_155_73, 0.063_128_766_447_470_95, 0.087_374_622_099_954_45, 0.111_391_588_795_496_67, 0.139_818_063_313_364_85, 0.191_103_092_660_864_9, 0.328_349_395_700_794_24, 0.780_018_575_195_007_6],
    [-0.909_776_566_129_924_6, -0.442_460_888_689_513, -0.301_264_553_335_145_24, -0.167_683_077_449_680_9, -0.086_140_179_066_314_56, -0.043_889_174_106_320_49, -0.010_981_513_470_360_122, 0.008_867_322_929_916_434, 0.024_248_013_075_943_382, 0.041_793_086_606_155_73, 0.063_128_766_447_470_95, 0.087_374_622_099_954_45, 0.111_391_588_795_496_67, 0.139_818_063_313_364_85, 0.191_103_092_660_864_9, 0.328_349_395_700_794_24, 0.780_018_575_195_007_6],
];

/// The X-ray binaries' marks' medians' spans of their conditions, first and last node, in
/// [`XrbMark`]'s order; only the low-mass donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIAN_SPANS_27: [(f64, f64); 30] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (-0.164_055_905_325_488_75, 2.425_544_873_470_187_7),
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
    [-1.467_718_551_708_774_7, -1.651_574_301_411_084_9, -1.686_345_791_423_874_2, -0.895_588_010_881_416_5, -0.711_599_278_337_675_3, -0.627_005_151_749_216_8, -0.511_856_301_981_824_3, -0.007_966_084_332_636_012, 0.058_880_628_078_956_27, 0.109_868_758_207_804_81, 0.189_023_409_815_118_28, 0.208_460_266_283_031_7, 0.066_621_979_106_821_51, -0.321_278_701_997_123_94, -0.442_644_817_505_147_2, -0.508_270_919_603_414, -0.515_189_039_357_603_7],
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
    (1.164_948_487_059_780_8, 1.996_852_964_720_374_8),
    (2.001_022_308_104_295_7, 8.787_813_035_339_518),
    (-0.365_939_117_930_022_2, 3.229_107_781_829_91),
    (-1.857_435_704_027_665_7, 0.596_197_784_939_810_1),
    (-5.596_358_743_396_394, 3.655_714_600_432_353_3),
    (1.75, 4.25),
    (1.25, 3.75),
    (0.355_095_299_505_206_4, 2.984_609_187_822_059_5),
    (0.004_937_568_651_288_154, 0.959_466_521_442_529_1),
    (8.009_209_165_630_322, 27.169_795_023_354_9),
    (0.1, 1.2),
    (10.0, 35.0),
    (-10.25, -7.75),
    (0.494_476_138_824_081_7, 59.273_080_342_508_64),
    (0.0, 120.1),
    (4.174_421_988_060_552, 7.477_784_022_709_876),
    (5.5, 6.7),
    (0.575_937_324_311_977_3, 1.503_229_118_814_494_2),
    (1.036_815_700_130_632_4, 1.629_310_277_259_491_8),
    (-0.000_143_975_019_286_123_4, 2.818_822_331_560_381_3),
    (1.282_993_821_330_733_3, 3.131_830_039_835_625_4),
    (0.0, 2.0),
    (0.092_834_707_932_504_24, 1.138_140_535_555_715_3),
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
    0.027_592_043_632_625_79,
    0.774_557_961_150_834_6,
    0.000_440_272_394_776_250_33,
    0.13,
    0.066_106_831_688_015_29,
    0.000_277_960_490_062_343_4,
];

/// The high-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.
#[rustfmt::skip]
pub const XRB_HIGH_MASS_KINDS_27: [f64; 3] = [
    0.001_011_394_133_781_865,
    5.801_361_387_395_067e-6,
    7.735_148_516_526_755e-6,
];

/// The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const XRB_HOSTS_PER_FORMED_MASS_27: [f64; 7] = [
    1.018_996_489_667_687_4e-7,
    1.272_857_270_112_822e-7,
    7.959_838_134_396_699e-8,
    7.988_083_639_276_28e-8,
    9.040_894_977_830_377e-8,
    8.658_341_165_715_493e-8,
    2.248_602_311_453_657e-8,
];

/// The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const XRB_LAYER_SHARES_27: [f64; 5] = [
    0.0,
    0.0,
    0.0,
    0.240_054_175_239_811_4,
    0.759_945_824_760_188_6,
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
