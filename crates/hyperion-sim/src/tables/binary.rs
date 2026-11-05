//! Plan 11's binary tables (P11.T6–T8): their shapes are plan 11's, their contents plan 15's
//! (P15.T9.b, P15.T10.b), which P11.T15 swaps in with a version bump.
//!
//! [`IA_YIELD`] (P11.T6) is the Type Ia explosion mark's yield η: the share of the binary engine's
//! detonatable pooled Type Ia candidates (ruling 129.1's rule) that explode, by delay bin and
//! channel. **Fitted** by plan 15's P15.T9.b against the engine's pool at generator version 30's
//! rules ([`IA_YIELD_ETA`], task `type_ia_yield`'s block below): η = min(1, target ÷ pool) is 1 in
//! every bin, as the scratch had it, since the detonatable pool is 0.239 of the delay-time
//! distribution's yield over a Hubble time (mergers 0.056, accretions 0.183; `α_CE` λ = 0.5) and
//! falls short in every bin, so no generated value moves.
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
//! the engine at generator version 29, and **scratch** for the two merger classes: every
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
//! The fitted tables are tasks `binary_awd`'s and `binary_xrb`'s blocks below, refitted at 29 on the
//! census of the pairs the generator itself draws at a record's first attempt (train 27's finding:
//! the refit at 27 drew every record's first attempt under `Free` multiplicity on its primary's own
//! draws, which the generator does not publish).

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

/// The yield (module documentation): plan 15's P15.T9.b fit, [`IA_YIELD_ETA`], 1 in every delay bin
/// and for both channels, every detonatable candidate exploding, since the detonatable pool falls
/// short of the yield in every bin, as plan 11's scratch had it (Design note 13).
pub const IA_YIELD: IaYieldTable = IaYieldTable::new(IA_YIELD_ETA);

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
/// median's span and nodes, and its limits (P15.T10.b). The recurrence is
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

/// The fast accreting white dwarfs' marks refitted at 29 (P15.T10.b), from task `binary_awd`'s
/// block.
const AWD_FAST_MARKS: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_FAST_RESIDUALS_29,
    &AWD_FAST_NODE_RESIDUALS_29,
    &AWD_FAST_MEDIAN_SPANS_29,
    &AWD_FAST_MEDIANS_29,
    &AWD_FAST_LIMITS_29,
);

/// The slow accreting white dwarfs' marks refitted at 29 (P15.T10.b), from task `binary_awd`'s
/// block.
const AWD_SLOW_MARKS: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_SLOW_RESIDUALS_29,
    &AWD_SLOW_NODE_RESIDUALS_29,
    &AWD_SLOW_MEDIAN_SPANS_29,
    &AWD_SLOW_MEDIANS_29,
    &AWD_SLOW_LIMITS_29,
);

/// The fast accreting white dwarfs' sampler fitted against the engine (P15.T10.b's refit at
/// generator version 29): [`AwdMark`]'s marks and the donor's kind, each mark fitted to the
/// engine's fast members alone, so that its recurrence lies all within L and its every rank is the
/// fast hosts', on the census of the pairs the generator itself draws at a record's first attempt
/// (`stellar::binary::testing::class_census_in`): under the record's own multiplicity, a displaced
/// record single, and on its primary's kept kick. Each member is weighted by the hosts its donor's
/// kind stands for in the rows, which the kinds' shares are, so that the marks the kinds share
/// follow the observed mix of kinds and not the engine's. Where P11.T8.a's scratch sampler served
/// both classes, split at L, each class has its own: the engine's two sides differ in their donors
/// and rates, and the fast hosts are an eighth of the members, too few of a shared law's 17 ranks.
/// A mark too few members define keeps the scratch sampler's law (task `binary_awd`'s block says
/// which).
pub const AWD_FAST_SAMPLER: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_FAST_MARKS,
    &[SamplerPick::new("donor_kind", &AWD_FAST_DONOR_KINDS_29)],
);

/// The slow accreting white dwarfs' sampler refitted at 29, as [`AWD_FAST_SAMPLER`] is the fast
/// ones', its recurrence all beyond L.
pub const AWD_SLOW_SAMPLER: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_SLOW_MARKS,
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

/// The X-ray binaries' fitted marks (P15.T10.b) from task `binary_xrb`'s block, in
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

/// The X-ray binaries' marks refitted at 29, from task `binary_xrb`'s block.
const XRB_MARKS: [SamplerMark; 30] = fitted_xrb_marks(
    &XRB_RESIDUALS_29,
    &XRB_DONOR_MASS_NODE_RESIDUALS_29,
    &XRB_MEDIAN_SPANS_29,
    &XRB_MEDIANS_29,
    &XRB_LIMITS_29,
);

/// The X-ray binaries' sampler fitted against the engine (P15.T10.b's refit at generator version
/// 29), on the census of the pairs the generator itself draws at a record's first attempt, as
/// [`AWD_FAST_SAMPLER`] is: [`XrbMark`]'s marks and the two kind choices over all nine kinds the
/// carving counts, fitted to the engine's members of each kind, each mark too few members define
/// kept as P11.T8.b's hand-made scratch sampler had it, from the observed figures of plan 11's
/// design note 13 (task `binary_xrb`'s block says which). Each kind is weighted by the galaxy's
/// share of the engine's members of it, but the black-hole transients, pinned at
/// [`BLACK_HOLE_TRANSIENT_SHARE`], so that the kinds the class draws are those its row counts
/// (P11.T8.b's fix).
pub const XRB_SAMPLER: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    &XRB_MARKS,
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
/// fitted against the engine at generator version 29 from task `binary_awd`'s block, each donor
/// kind scaled to its observed figure, and the X-ray binaries' from task `binary_xrb`'s, on the
/// census of the pairs the generator itself draws, and the two merger classes' scratch until their
/// own fits, 9 × 10⁴ stellar mergers and 10 neutron-star mergers in the source horizon at Milky Way
/// parameters.
pub const CLASS_SHARES: ClassShareTable = ClassShareTable {
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

/// The stellar mergers' fitted marks (P15.T10.b's merger step, at 31), from task `binary_merger`'s
/// block, in [`MergerMark`]'s order: each mark's residual quantiles over all its members, its
/// median's span and nodes and its laws at the nodes (a mark conditional on the layer, the layer's
/// index its condition, holding layer b's law at node 4b), and its limits.
const MERGER_MARKS_31: [SamplerMark; 10] = {
    let mut out = [SamplerMark::new("", [0.0; 17], None, (0.0, 0.0)); 10];
    let mut k = 0;
    while k < 10 {
        let median = MedianCurve::new(
            MERGER_MEDIAN_SPANS_31[k].0,
            MERGER_MEDIAN_SPANS_31[k].1,
            MERGER_MEDIANS_31[k],
        );
        out[k] = SamplerMark::new(
            MERGER_MARKS[k].name,
            MERGER_RESIDUALS_31[k],
            Some(median),
            MERGER_LIMITS_31[k],
        )
        .with_node_residuals(node_rows(&MERGER_NODE_RESIDUALS_31, k));
        k += 1;
    }
    out
};

/// The stellar mergers' sampler fitted against the engine at generator version 31 (P15.T10.b's
/// merger step) on the merger census of the pairs the generator draws at each record's first
/// attempt: [`MergerMark`]'s marks, the marks the class draws after the layer conditional on it,
/// the layer choice at its fixed point and the channels' weights the covered mergers' (task
/// `binary_merger`'s block says which marks keep [`MERGER_SAMPLER`]'s law). A galaxy's class reads
/// it from 31 (`MergerTableRules::Fitted`).
pub const MERGER_SAMPLER_31: ClassSamplerTable<MergerMark, MergerPick> = ClassSamplerTable::new(
    &MERGER_MARKS_31,
    &[
        SamplerPick::new("layer", &MERGER_LAYERS_31),
        SamplerPick::new("engine_channel", &MERGER_CHANNELS_31),
    ],
);

/// The neutron-star mergers' fitted marks (P15.T10.b's merger step, at 31), from task
/// `binary_nsm`'s block, in [`NsmMark`]'s order, each unconditional.
const NSM_MARKS_31: [SamplerMark; 5] = {
    let mut out = [SamplerMark::new("", [0.0; 17], None, (0.0, 0.0)); 5];
    let mut k = 0;
    while k < 5 {
        out[k] = SamplerMark::new(
            NSM_MARKS[k].name,
            NSM_RESIDUALS_31[k],
            None,
            NSM_LIMITS_31[k],
        );
        k += 1;
    }
    out
};

/// The neutron-star mergers' sampler fitted against the engine at generator version 31, as
/// [`MERGER_SAMPLER_31`] is the stellar mergers' (task `binary_nsm`'s block says which marks keep
/// [`NSM_SAMPLER`]'s law). A galaxy's class reads it from 31 (`MergerTableRules::Fitted`).
pub const NSM_SAMPLER_31: ClassSamplerTable<NsmMark, NsmPick> = ClassSamplerTable::new(
    &NSM_MARKS_31,
    &[SamplerPick::new("pair_kind", &NSM_PAIR_KINDS_31)],
);

/// `table` with its two merger classes' rows those of tasks `binary_merger`'s and `binary_nsm`'s
/// blocks (P15.T10.b's merger step, at 31).
const fn with_fitted_mergers(table: &ClassShareTable) -> ClassShareTable {
    let mut rows = table.rows;
    rows[3] = ClassShareRow {
        class: CarvedClass::StellarMerger,
        hosts_per_formed_mass: MERGER_HOSTS_PER_FORMED_MASS_31,
        layer_shares: MERGER_LAYER_SHARES_31,
    };
    rows[4] = ClassShareRow {
        class: CarvedClass::NeutronStarMerger,
        hosts_per_formed_mass: NSM_HOSTS_PER_FORMED_MASS_31,
        layer_shares: NSM_LAYER_SHARES_31,
    };
    ClassShareTable { rows }
}

/// The share rows from generator version 31: [`CLASS_SHARES`]', its two merger classes' rows
/// fitted on the generator's own pairs (P15.T10.b's merger step). A galaxy reads it from 31.
pub const CLASS_SHARES_31: ClassShareTable = with_fitted_mergers(&CLASS_SHARES);

// @begin-table binary_awd
// The accreting white dwarfs' samplers and share rows refitted against plan 11's engine at
// generator version 29 on the generator's own pairs (plan 15, P15.T10.b).
//
// @generated by hyperion-fit 0.1.0, task `binary_awd` revision 0. Do not edit.
// inputs-sha256: 05097b201a6558b687e2193318b21c8feb83639650a9d963d38e82b56575b15d
// manifest: crates/hyperion-fit/manifests/binary_awd.toml
// data: none
// sim-fingerprint: e90be681791ba21923f19dd95e44fb857e830ac8e3e63be0947a49dcc67daf33
// since-generator-version: 29
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the rows by donor kind against Pala et al. 2020, MNRAS 494, 3799, Carter et al. 2013,
//   MNRAS 429, 2143, and Lü, Yungelson and Han 2006, MNRAS 372, 1389, the total against the
//   brainstorm's 6–12 × 10⁶
// acceptance: kind scales 1.6815e-1/7.6158e-2/1.0693e-2; fast: 2388 members, own nodes
//   0/15/14/0/6/17/0/4/16/17/17, donors 0.992/0.000/0.008, layers C 0.112 D 0.871 E 0.017, rates
//   a system 7.01e-7/5.25e-5/1.02e-5/1.54e-5/1.89e-5/2.03e-5/6.31e-6, not its own: HeliumPeriod
//   Scratch, HeliumDonorMass Scratch; slow: 25240 members, own nodes
//   0/16/17/16/5/17/16/9/17/17/17, donors 0.891/0.108/0.001, layers C 0.380 D 0.609 E 0.011,
//   rates a system 4.91e-6/5.15e-4/8.47e-4/6.96e-4/6.22e-4/6.17e-4/9.98e-4; the K–S tests
//   against the other half of the census are the sim's slow
//   `fitted_tables::the_accreting_white_dwarf_samplers_against_the_engine`
//
// Read by `AWD_FAST_SAMPLER`, `AWD_SLOW_SAMPLER` and `CLASS_SHARES`, after the block. Each
// class's marks are fitted to its own members of the engine near the solar circle at generator
// version 30's rules (hyperion-fit's `tasks::binary_classes`): a median that follows the
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
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [-0.804_512_808_064_063_8, -0.360_722_057_204_749_87, -0.246_641_651_153_888_42, -0.188_477_397_969_563_75, -0.144_340_863_697_627_52, -0.106_688_207_180_291_11, -0.072_837_430_391_747_84, -0.031_404_559_336_946_004, 3.492_271_609_479_856e-5, 0.026_848_101_454_901_22, 0.052_170_232_566_092_65, 0.079_017_920_707_960_27, 0.108_583_749_499_592_12, 0.146_593_844_787_835_83, 0.185_651_285_486_331_59, 0.219_012_996_817_087_55, 0.317_876_168_358_798_5],
    [-1.284_822_141_770_847_2, -0.533_379_554_754_279_3, -0.200_990_953_241_653_97, -0.117_720_897_234_180_83, -0.089_445_826_137_619_67, -0.068_283_232_116_912_2, -0.046_953_597_472_316_56, -0.025_111_953_188_780_612, -0.001_168_348_942_622_388, 0.018_708_277_120_429_193, 0.043_459_147_668_355_05, 0.095_223_789_645_874_12, 0.185_140_314_055_875_22, 0.321_772_873_700_455_1, 0.456_072_956_046_488_23, 0.646_309_916_774_121_6, 1.283_121_126_437_199_3],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.653_993_462_600_217_6, -0.185_679_223_770_099_35, -0.124_541_250_913_662_2, -0.091_539_645_179_789_64, -0.070_259_833_971_829_56, -0.050_402_638_419_411_144, -0.035_643_453_615_766_166, -0.020_941_336_178_452_313, -0.004_974_508_812_091_423_5, 0.007_229_857_485_868_971, 0.020_128_218_298_479_165, 0.036_122_699_106_862_8, 0.051_290_303_039_697_18, 0.071_292_836_750_616_58, 0.096_810_902_884_048_2, 0.141_763_259_900_989_7, 0.743_230_475_997_371_8],
    [-0.125, -0.074_568_000_000_000_01, -0.056_400_65, -0.043_640_9, -0.033_237_55, -0.024_110_450_000_000_002, -0.015_727_4, -0.007_767_050_000_000_001, 0.0, 0.007_767_050_000_000_001, 0.015_727_4, 0.024_110_450_000_000_002, 0.033_237_55, 0.043_640_9, 0.056_400_65, 0.074_568_000_000_000_01, 0.125],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.285_608_117_672_971_7, -0.127_435_319_999_515_77, -0.089_244_501_596_416_86, -0.068_161_131_606_152_25, -0.052_791_073_420_143_03, -0.041_999_946_959_358_6, -0.030_077_879_817_660_46, -0.018_820_905_571_095_47, -0.005_942_807_926_910_568, 0.011_756_391_871_935_005, 0.030_348_473_493_305_505, 0.050_495_767_731_474_03, 0.068_897_665_739_446_33, 0.087_790_572_770_589_83, 0.107_511_030_314_857_83, 0.175_527_044_316_753_5, 0.326_403_832_353_968_9],
    [-0.606_247_161_633_360_6, -0.242_250_955_899_395_24, -0.185_082_907_383_769_98, -0.152_522_988_448_635_32, -0.114_617_375_707_970_37, -0.080_095_975_970_826_89, -0.050_465_389_078_813_24, -0.022_101_157_058_965_44, 0.000_679_348_891_968_755, 0.021_974_533_509_934_04, 0.042_707_840_060_459_83, 0.063_974_322_359_037_31, 0.082_479_930_832_420_96, 0.101_456_721_130_277_79, 0.126_395_317_300_542_68, 0.173_826_429_866_233_23, 0.398_998_014_355_687],
    [-1.436_290_988_303_229, -0.472_850_528_398_152_4, -0.341_841_080_942_534_6, -0.258_514_182_208_325_3, -0.202_847_285_365_526_96, -0.144_926_867_484_760_27, -0.095_397_440_131_179_79, -0.045_453_727_832_414_4, -0.000_165_896_652_977_740_95, 0.048_968_220_439_670_51, 0.096_434_081_941_480_41, 0.146_306_890_508_735_02, 0.201_144_861_396_501_84, 0.255_769_769_539_856_2, 0.325_583_754_513_264_8, 0.443_130_876_525_321_1, 0.692_197_646_476_872_2],
];

/// The fast accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_FAST_NODE_RESIDUALS_29: [[f64; 17]; 187] = [
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [2.813_400_083_155_906_7, 3.866_345_845_289_923, 4.226_573_077_268_001, 4.435_607_889_915_984, 4.567_515_824_155_216, 4.682_739_123_058_778, 4.775_154_040_370_428_5, 4.855_881_673_671_2, 4.916_311_327_826_549, 4.985_715_909_250_591, 5.068_161_464_227_719, 5.133_779_098_880_585, 5.199_553_752_568_73, 5.255_887_672_374_951, 5.309_314_122_183_338, 5.358_819_212_103_386, 5.418_370_850_344_337_5],
    [-0.804_512_808_064_063_8, -0.360_722_057_204_749_87, -0.246_641_651_153_888_42, -0.188_477_397_969_563_75, -0.144_340_863_697_627_52, -0.106_688_207_180_291_11, -0.072_837_430_391_747_84, -0.031_404_559_336_946_004, 3.492_271_609_479_856e-5, 0.026_848_101_454_901_22, 0.052_170_232_566_092_65, 0.079_017_920_707_960_27, 0.108_583_749_499_592_12, 0.146_593_844_787_835_83, 0.185_651_285_486_331_59, 0.219_012_996_817_087_55, 0.317_876_168_358_798_5],
    [-0.804_512_808_064_063_8, -0.360_722_057_204_749_87, -0.246_641_651_153_888_42, -0.188_477_397_969_563_75, -0.144_340_863_697_627_52, -0.106_688_207_180_291_11, -0.072_837_430_391_747_84, -0.031_404_559_336_946_004, 3.492_271_609_479_856e-5, 0.026_848_101_454_901_22, 0.052_170_232_566_092_65, 0.079_017_920_707_960_27, 0.108_583_749_499_592_12, 0.146_593_844_787_835_83, 0.185_651_285_486_331_59, 0.219_012_996_817_087_55, 0.317_876_168_358_798_5],
    [-0.804_512_808_064_063_8, -0.687_325_028_199_475_5, -0.528_320_232_054_719_7, -0.436_000_321_734_500_04, -0.391_153_503_000_888_8, -0.198_866_366_269_921_52, -0.138_511_137_654_411, -0.120_493_134_700_177_83, -0.041_801_720_438_095_08, 0.029_537_403_296_721_74, 0.113_098_578_542_321_78, 0.149_371_201_446_8, 0.169_130_466_914_227_98, 0.202_637_579_553_185_65, 0.240_099_916_040_268_45, 0.266_482_199_188_015_7, 0.294_840_447_496_019_1],
    [-0.786_775_660_469_563, -0.662_559_200_423_453, -0.548_080_779_199_196_5, -0.375_361_410_562_027_4, -0.231_087_126_307_617_3, -0.184_906_422_247_783_75, -0.127_494_317_512_186_37, -0.084_861_497_231_299_74, -0.022_407_883_734_981_163, 0.039_080_756_269_059_984, 0.103_910_102_628_090_08, 0.137_893_977_093_997_48, 0.150_477_085_095_829_97, 0.162_897_538_168_637_58, 0.194_362_677_367_646_17, 0.239_169_811_886_508_3, 0.294_840_447_496_019_1],
    [-0.786_775_660_469_563, -0.622_384_552_557_852_4, -0.365_702_727_130_367_3, -0.231_892_725_775_373_34, -0.171_597_303_419_306_04, -0.112_563_784_821_508_47, -0.084_291_470_224_845_2, -0.044_803_399_457_659_02, 0.005_803_196_610_009_171, 0.044_492_794_027_189_41, 0.081_011_079_620_783_84, 0.104_703_421_398_059_88, 0.124_729_538_340_251_63, 0.145_413_769_972_548_54, 0.162_662_950_322_057_46, 0.196_456_711_049_170_5, 0.266_790_831_031_053_83],
    [-0.786_775_660_469_563, -0.504_913_861_148_363_7, -0.218_888_310_404_831_7, -0.144_617_453_578_491_93, -0.111_200_341_236_999_76, -0.088_423_985_155_340_79, -0.049_620_499_841_642_046, -0.030_132_569_111_167_778, 0.012_610_297_525_828_438, 0.039_929_076_543_035_8, 0.059_536_467_544_579_766, 0.091_005_728_098_501_44, 0.108_156_329_963_201, 0.128_987_406_683_382_5, 0.164_863_563_795_936_95, 0.193_870_950_180_116_5, 0.257_063_310_518_100_83],
    [-0.737_233_355_584_825_2, -0.285_952_395_286_065_8, -0.160_834_395_801_016_8, -0.127_610_150_173_245_65, -0.107_492_271_606_887_01, -0.089_436_522_289_464_64, -0.052_773_506_621_290_434, -0.030_204_701_935_255_245, 0.008_153_655_365_974_696, 0.026_157_880_548_695_733, 0.046_109_121_658_671_044, 0.069_476_646_302_273_64, 0.104_327_732_004_404_33, 0.125_943_481_781_812_25, 0.170_912_817_932_381_42, 0.194_321_012_335_446_02, 0.268_293_324_215_300_3],
    [-0.737_233_355_584_825_2, -0.300_924_024_662_240_44, -0.150_650_743_953_606_23, -0.126_306_962_892_317_44, -0.106_524_893_208_244_03, -0.082_291_904_528_807_94, -0.052_666_476_771_362_49, -0.030_447_222_692_021_22, 0.000_114_983_395_497_474_1, 0.022_468_060_245_900_4, 0.042_398_768_194_731_98, 0.064_448_667_526_543_88, 0.100_307_689_529_623_2, 0.136_565_179_988_394_8, 0.185_422_212_386_631_8, 0.210_760_827_164_867_74, 0.283_240_803_105_073_93],
    [-0.737_233_355_584_825_2, -0.252_679_148_002_837_9, -0.143_535_122_153_133_58, -0.121_551_354_492_824_63, -0.096_974_457_293_904_61, -0.067_174_924_244_486_36, -0.044_057_329_913_161_98, -0.026_202_293_891_193_735, 0.005_862_408_904_369_804, 0.026_522_402_063_094_964, 0.041_742_655_625_137_65, 0.072_091_493_089_327_5, 0.120_503_124_366_056_06, 0.156_038_289_484_669_94, 0.198_890_448_877_372_67, 0.239_719_015_844_160_05, 0.283_240_803_105_073_93],
    [-0.737_233_355_584_825_2, -0.280_304_415_223_407_43, -0.188_608_781_270_991, -0.126_512_151_575_308_1, -0.102_920_250_189_399_07, -0.079_659_655_627_339_73, -0.052_528_030_998_301_79, -0.029_389_806_060_895_002, 0.004_548_637_383_617_046, 0.025_942_386_174_976_694, 0.042_734_285_390_789_176, 0.071_144_423_330_141_59, 0.108_898_311_538_842_87, 0.146_375_903_915_663_94, 0.199_443_208_786_891_23, 0.240_005_828_828_102_5, 0.313_363_753_237_800_4],
    [-0.734_384_710_244_382_8, -0.308_421_651_439_333_5, -0.206_722_219_711_099_8, -0.147_051_399_088_241_5, -0.116_091_093_103_755_58, -0.090_362_701_533_564_38, -0.068_487_443_271_950_78, -0.031_296_037_680_418_29, -0.005_612_853_001_572_146, 0.023_825_882_743_398_68, 0.045_753_050_854_896_35, 0.073_259_243_948_742_09, 0.108_313_906_976_073_61, 0.145_135_176_844_684_47, 0.203_477_273_081_633_13, 0.232_830_687_941_346_31, 0.313_363_753_237_800_4],
    [-0.734_384_710_244_382_8, -0.332_667_553_324_306_36, -0.227_490_543_809_770_24, -0.167_860_188_103_379_16, -0.123_510_212_936_123_41, -0.099_316_040_885_087_28, -0.069_386_212_499_619_11, -0.028_994_339_261_891_8, 0.000_831_854_248_445_830_1, 0.028_974_461_810_038_404, 0.052_854_847_029_903_72, 0.082_739_053_497_854_54, 0.117_709_279_005_754_63, 0.172_131_341_609_386_17, 0.211_449_025_340_603_84, 0.232_194_334_025_239_76, 0.317_876_168_358_798_5],
    [-0.705_275_891_272_784_3, -0.366_669_930_692_49, -0.244_476_642_596_379_33, -0.183_152_748_803_418_11, -0.144_177_024_242_057_44, -0.106_850_301_612_775_33, -0.078_231_456_593_275_89, -0.027_886_055_334_432_61, 0.004_579_081_756_717_197, 0.026_296_045_928_244_54, 0.058_594_807_993_199_47, 0.088_403_108_502_640_22, 0.125_380_848_896_674_72, 0.167_750_295_666_018_26, 0.207_685_370_964_517_86, 0.227_199_009_431_263_48, 0.317_876_168_358_798_5],
    [-0.705_275_891_272_784_3, -0.367_029_297_987_950_5, -0.254_480_442_712_058_5, -0.207_764_952_195_367_42, -0.158_689_593_732_356_67, -0.121_703_013_864_868_12, -0.085_935_496_660_799_26, -0.038_809_245_960_236_18, -0.001_342_786_153_293_169_3, 0.021_873_323_484_063_435, 0.057_872_220_730_187_55, 0.085_298_623_253_853_22, 0.111_407_025_825_293_38, 0.153_958_166_671_341_68, 0.195_693_919_739_235_86, 0.219_369_592_191_578_28, 0.317_876_168_358_798_5],
    [-0.695_303_977_832_231_6, -0.355_547_298_764_839_37, -0.251_780_328_623_497_03, -0.207_048_938_686_199, -0.168_310_154_486_498_5, -0.127_090_176_564_968_04, -0.086_895_760_515_892_72, -0.036_117_191_296_157_404, -0.000_761_713_487_104_592_7, 0.027_089_603_959_924_824, 0.053_148_093_207_418_444, 0.076_125_554_108_536_55, 0.099_880_005_339_391_55, 0.138_909_550_600_000_57, 0.174_812_075_919_897_97, 0.204_184_724_769_552_1, 0.317_876_168_358_798_5],
    [-0.575_303_067_033_400_8, -0.337_983_322_216_496_13, -0.247_859_498_013_129_98, -0.203_074_054_759_192_1, -0.165_520_082_804_446_3, -0.120_991_393_093_438_81, -0.079_479_523_948_752_86, -0.031_375_251_743_140_81, 0.001_545_034_076_810_805_8, 0.028_537_015_298_538_044, 0.049_187_799_142_620_38, 0.071_280_931_241_891_3, 0.095_661_638_025_576_56, 0.134_256_405_521_618_94, 0.164_449_433_163_622_48, 0.197_100_913_119_072_46, 0.314_240_019_286_312_6],
    [-0.575_303_067_033_400_8, -0.336_301_014_006_766_1, -0.258_908_002_703_295_1, -0.206_641_383_498_910_06, -0.164_721_361_397_965_12, -0.126_866_242_714_646_08, -0.079_238_465_103_525_02, -0.031_046_155_001_488_555, -0.003_683_774_169_056_118, 0.027_025_795_921_906_383, 0.047_394_354_293_014_05, 0.069_408_849_227_508_69, 0.095_947_329_904_739_1, 0.136_136_260_245_995_48, 0.162_522_526_721_234_32, 0.196_237_078_329_213_9, 0.287_970_283_633_656_4],
    [-0.470_301_311_188_763_5, -0.429_323_890_461_482_5, -0.174_444_758_372_347_24, -0.143_188_334_104_069_03, -0.123_275_132_967_790_26, -0.088_678_937_609_609_46, -0.044_988_788_436_573_46, 0.004_519_187_202_292_265_4, 0.030_057_232_083_456_988, 0.049_697_006_801_945_31, 0.065_527_821_690_349_15, 0.080_716_722_978_979_33, 0.113_787_921_889_559_16, 0.160_970_356_184_356_12, 0.187_511_524_074_251_76, 0.233_637_591_298_438_28, 1.283_121_126_437_199_3],
    [-0.998_825_940_490_546_2, -0.471_303_040_764_579_3, -0.336_678_321_327_785_86, -0.229_704_520_516_211_7, -0.188_629_339_983_688_86, -0.140_784_439_734_978, -0.116_109_404_529_424_21, -0.077_039_138_196_787_68, -0.037_949_434_506_184_18, -0.000_971_781_431_311_694_3, 0.030_882_717_405_288_42, 0.050_430_696_428_763_96, 0.072_413_828_327_986_5, 0.121_385_466_162_347_74, 0.188_346_785_351_621_42, 0.602_443_580_216_320_3, 1.283_121_126_437_199_3],
    [-1.267_826_853_173_089, -0.597_793_535_432_996_8, -0.507_867_372_598_691_6, -0.437_850_750_477_726_9, -0.308_402_366_241_730_3, -0.229_131_565_817_577_32, -0.183_589_948_649_072_8, -0.128_538_900_541_380_42, -0.070_823_566_405_028_03, -0.025_105_456_126_478_18, 0.017_259_551_632_913_508, 0.045_705_672_104_246_496, 0.087_075_214_506_946_16, 0.149_142_398_301_855_97, 0.314_580_949_034_460_77, 0.608_805_319_370_335_9, 1.283_121_126_437_199_3],
    [-1.271_497_852_368_648_2, -0.809_164_491_377_210_2, -0.564_744_154_756_375_3, -0.491_994_460_275_548_55, -0.341_923_067_602_597_16, -0.200_249_973_451_445_52, -0.096_481_345_461_683_92, -0.039_574_156_006_564_58, 0.004_351_144_714_305_286, 0.029_612_029_200_197_874, 0.045_677_402_497_136_074, 0.079_551_137_487_205_3, 0.109_471_737_422_073, 0.150_162_616_449_929_45, 0.217_749_089_639_183_9, 0.406_222_595_657_180_83, 1.283_121_126_437_199_3],
    [-1.284_822_141_770_847_2, -1.170_965_255_196_704_7, -0.519_433_764_162_387_7, -0.118_792_192_253_999_71, -0.083_663_063_359_925_61, -0.039_224_460_202_026_96, -0.010_047_477_940_191_125, 0.009_130_161_284_142_123, 0.023_647_171_226_843_677, 0.037_044_620_972_782_49, 0.048_363_446_840_045_864, 0.075_397_629_620_440_44, 0.096_162_949_017_133_42, 0.125_798_582_290_228_06, 0.181_604_888_559_786_7, 0.270_604_499_110_552_34, 0.956_443_364_751_707_7],
    [-1.284_822_141_770_847_2, -1.211_255_857_444_559_3, -0.127_789_214_597_033_77, -0.083_936_396_060_080_2, -0.053_855_977_602_171_284, -0.032_883_516_871_429_73, -0.013_820_918_739_701_864, 0.000_159_989_952_060_764_56, 0.010_386_382_392_243_291, 0.021_289_677_551_702_933, 0.034_061_845_827_968_125, 0.045_793_064_498_288_696, 0.077_721_899_299_828_5, 0.118_081_401_672_689_53, 0.184_776_322_381_933_12, 0.297_999_125_882_713_8, 0.901_033_451_327_329_6],
    [-1.284_822_141_770_847_2, -0.582_172_693_787_141_3, -0.100_586_907_579_220_6, -0.070_323_425_555_642_79, -0.054_145_210_661_792_1, -0.038_962_576_884_241_87, -0.024_445_552_059_612_61, -0.011_904_428_173_422_086, -0.000_938_454_768_934_682_6, 0.009_103_810_761_124_492, 0.020_824_575_901_687_452, 0.036_697_789_023_618_67, 0.074_436_357_959_243_54, 0.182_102_049_130_847_2, 0.295_836_091_994_329_53, 0.520_471_242_868_377, 0.940_413_392_638_081_5],
    [-1.284_822_141_770_847_2, -0.144_563_467_160_463_06, -0.089_582_616_592_220_5, -0.074_544_300_751_278_71, -0.061_640_385_188_856_37, -0.047_473_209_682_586_02, -0.036_432_591_762_820_61, -0.021_632_055_634_357_994, -0.009_407_926_250_084_114, 0.004_555_056_319_064_429, 0.021_494_224_927_161_784, 0.068_112_704_935_568_9, 0.224_081_069_701_557_36, 0.345_853_670_345_918, 0.484_736_769_938_812_2, 0.630_641_396_651_501_3, 1.028_145_022_668_935_5],
    [-1.278_321_194_533_442_3, -0.119_657_841_063_136_76, -0.096_799_868_479_791_73, -0.085_816_655_143_658_24, -0.075_904_555_225_933_85, -0.065_715_018_258_549_61, -0.049_704_463_545_305_01, -0.038_852_690_633_376_42, -0.018_438_286_802_194_545, 0.006_761_273_925_376_501, 0.155_356_770_996_497_6, 0.292_773_570_988_795_5, 0.372_033_347_460_291_8, 0.453_910_051_740_461_7, 0.571_539_061_216_529_4, 0.717_354_533_406_659_3, 1.048_016_783_166_329_2],
    [-1.278_321_194_533_442_3, -0.139_816_774_456_973_16, -0.114_439_111_596_574_81, -0.101_324_338_255_118_41, -0.088_338_730_617_372_9, -0.081_352_669_328_493_69, -0.067_552_345_026_320_04, -0.049_440_041_617_172_38, -0.026_117_954_390_545_19, 0.140_877_980_523_810_26, 0.304_855_317_931_349_35, 0.373_629_682_587_475_9, 0.427_517_976_548_440_5, 0.539_723_079_534_255_4, 0.659_895_986_810_406_6, 0.762_462_667_392_811_5, 1.048_016_783_166_329_2],
    [-0.549_820_302_063_406_7, -0.161_817_050_383_952_25, -0.145_230_167_902_110_77, -0.128_240_575_363_553_82, -0.111_894_780_361_178_9, -0.099_850_620_837_575_22, -0.084_338_036_820_550_2, -0.063_261_625_843_954_34, -0.035_932_070_591_720_94, 0.162_711_288_158_593_31, 0.349_341_307_954_628_7, 0.406_814_620_236_186_14, 0.502_726_716_227_706_8, 0.635_593_081_164_092, 0.716_869_263_663_132_2, 0.814_923_661_820_788_3, 1.048_016_783_166_329_2],
    [-0.806_907_638_677_474_5, -0.209_488_523_344_782_5, -0.173_301_961_849_658_04, -0.160_093_783_157_058_24, -0.146_868_984_001_838_1, -0.136_553_953_332_532_26, -0.113_455_141_560_143_76, -0.089_388_818_621_898_29, -0.055_925_172_356_628_15, 0.051_427_322_815_101_574, 0.405_039_830_985_497_56, 0.495_881_645_328_449_03, 0.595_741_056_855_228_2, 0.655_916_722_737_114_4, 0.745_622_904_771_677, 0.825_028_081_510_816_9, 1.048_016_783_166_329_2],
    [-1.026_234_151_146_805_8, -0.429_472_455_636_703, -0.330_104_106_693_861_25, -0.217_595_155_764_807_7, -0.187_833_702_300_182_26, -0.171_478_708_325_001_42, -0.153_008_838_834_600_34, -0.136_159_041_350_699_28, -0.068_506_044_025_861_54, 0.097_912_731_350_276, 0.370_344_994_281_806_5, 0.485_765_666_415_826, 0.554_154_964_974_240_5, 0.633_211_671_145_783_9, 0.688_170_692_795_386_9, 0.822_566_342_215_144, 0.999_914_489_917_977_5],
    [-1.039_318_554_798_146_3, -0.922_652_625_301_594_6, -0.835_539_792_629_076_7, -0.459_383_824_573_968_5, -0.346_415_972_125_025_3, -0.232_770_823_293_713_1, -0.182_852_073_596_785_35, 0.028_612_177_400_746_563, 0.073_795_826_079_779_97, 0.095_459_609_349_964, 0.149_330_911_167_811_33, 0.166_756_849_223_308_4, 0.194_578_882_417_341_78, 0.373_207_814_938_800_1, 0.483_160_866_297_450_64, 0.640_501_216_516_712_9, 0.999_914_489_917_977_5],
    [-1.284_822_141_770_847_2, -0.533_379_554_754_279_3, -0.200_990_953_241_653_97, -0.117_720_897_234_180_83, -0.089_445_826_137_619_67, -0.068_283_232_116_912_2, -0.046_953_597_472_316_56, -0.025_111_953_188_780_612, -0.001_168_348_942_622_388, 0.018_708_277_120_429_193, 0.043_459_147_668_355_05, 0.095_223_789_645_874_12, 0.185_140_314_055_875_22, 0.321_772_873_700_455_1, 0.456_072_956_046_488_23, 0.646_309_916_774_121_6, 1.283_121_126_437_199_3],
    [-1.284_822_141_770_847_2, -0.533_379_554_754_279_3, -0.200_990_953_241_653_97, -0.117_720_897_234_180_83, -0.089_445_826_137_619_67, -0.068_283_232_116_912_2, -0.046_953_597_472_316_56, -0.025_111_953_188_780_612, -0.001_168_348_942_622_388, 0.018_708_277_120_429_193, 0.043_459_147_668_355_05, 0.095_223_789_645_874_12, 0.185_140_314_055_875_22, 0.321_772_873_700_455_1, 0.456_072_956_046_488_23, 0.646_309_916_774_121_6, 1.283_121_126_437_199_3],
    [-1.284_822_141_770_847_2, -0.533_379_554_754_279_3, -0.200_990_953_241_653_97, -0.117_720_897_234_180_83, -0.089_445_826_137_619_67, -0.068_283_232_116_912_2, -0.046_953_597_472_316_56, -0.025_111_953_188_780_612, -0.001_168_348_942_622_388, 0.018_708_277_120_429_193, 0.043_459_147_668_355_05, 0.095_223_789_645_874_12, 0.185_140_314_055_875_22, 0.321_772_873_700_455_1, 0.456_072_956_046_488_23, 0.646_309_916_774_121_6, 1.283_121_126_437_199_3],
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
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.315_365_273_587_887_13, -0.273_547_666_243_999_84, -0.190_533_303_131_171_53, -0.150_747_917_863_138_99, -0.131_310_358_407_593_8, -0.104_758_811_661_199_96, -0.053_943_255_578_037_926, -0.040_520_324_708_773_55, -0.029_873_425_606_763_444, 0.003_907_359_353_695_202_5, 0.033_974_475_581_333_56, 0.044_658_263_011_075_16, 0.065_994_355_705_157_06, 0.109_162_758_185_420_51, 0.174_516_999_071_723_86, 0.416_508_857_002_017_1, 0.674_501_793_483_576_4],
    [-0.419_084_415_248_123_45, -0.299_880_104_706_010_4, -0.262_552_351_046_248_13, -0.179_268_748_497_252_97, -0.138_262_706_991_810_95, -0.109_036_249_327_626_46, -0.046_718_384_549_740_52, -0.025_427_171_095_790_374, 0.030_929_428_855_594_267, 0.041_897_469_847_329_44, 0.053_423_588_349_245_69, 0.077_700_677_198_127_43, 0.100_956_112_268_095_49, 0.137_616_725_925_156_97, 0.205_787_652_874_207_94, 0.358_910_623_120_212_05, 0.674_501_793_483_576_4],
    [-0.419_084_415_248_123_45, -0.301_227_021_289_371_1, -0.263_559_024_002_867_6, -0.186_742_250_043_994_7, -0.157_736_864_306_329_66, -0.123_279_348_978_565_21, -0.082_996_034_394_430_4, -0.028_152_354_568_227_1, 0.037_512_224_259_599_47, 0.050_057_264_623_649_03, 0.077_681_627_468_931_2, 0.091_546_310_358_296_97, 0.129_978_166_446_000_22, 0.187_245_907_561_358_87, 0.237_550_480_903_064_63, 0.298_867_342_115_262, 0.674_501_793_483_576_4],
    [-0.419_084_415_248_123_45, -0.286_424_282_429_402_7, -0.231_621_036_909_565_4, -0.185_330_836_441_335_72, -0.155_399_675_228_408_9, -0.136_505_739_028_243_95, -0.109_287_006_339_118_24, -0.084_331_116_119_065_57, 0.011_136_449_107_862_965, 0.045_397_385_898_387_044, 0.083_784_651_241_348_22, 0.109_102_205_634_763_82, 0.170_208_335_231_298_75, 0.224_580_756_871_450_96, 0.273_541_917_352_330_63, 0.333_787_612_925_920_74, 0.740_318_943_749_876_7],
    [-0.419_084_415_248_123_45, -0.262_006_364_232_289_4, -0.202_900_358_821_970_6, -0.185_217_542_508_922_33, -0.151_744_242_483_632_77, -0.125_993_281_804_485_1, -0.103_748_023_012_963_33, -0.080_503_590_163_619_62, -0.028_589_488_887_936_52, 0.031_495_301_465_114_99, 0.084_389_638_012_114_78, 0.147_236_599_120_501_44, 0.200_829_528_051_537_53, 0.263_235_558_738_195_8, 0.328_615_506_632_372_8, 0.452_602_151_705_635_5, 0.740_318_943_749_876_7],
    [-0.317_334_739_557_148_4, -0.252_755_452_679_289_1, -0.199_766_277_434_377_2, -0.174_090_394_952_713_45, -0.130_640_461_283_440_7, -0.102_303_298_894_155_29, -0.069_105_429_609_010_31, -0.041_647_970_290_115_51, -0.000_806_046_140_578_849_3, 0.023_606_419_338_779_03, 0.088_252_579_224_441_05, 0.164_698_983_201_812_73, 0.262_081_878_143_643_5, 0.359_983_761_864_365_8, 0.464_868_127_183_055_1, 0.576_491_023_886_518_2, 0.740_318_943_749_876_7],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.419_084_415_248_123_45, -0.282_535_101_638_476_1, -0.209_879_988_153_377_72, -0.165_352_867_510_905_1, -0.137_444_518_902_343_88, -0.109_005_100_743_953_55, -0.072_437_646_303_989_74, -0.036_881_449_046_819_57, 0.010_146_265_344_940_35, 0.039_477_618_611_131_79, 0.065_686_506_150_688_19, 0.099_217_689_289_759_73, 0.162_548_562_442_725_97, 0.255_265_879_592_665_17, 0.342_440_632_480_736_7, 0.559_877_902_516_471, 5.127_495_246_249_504],
    [-0.440_197_735_794_020_55, -0.127_953_612_741_141_98, -0.095_507_192_713_084_47, -0.084_257_382_980_132_22, -0.070_225_292_292_249_05, -0.043_186_135_921_292_3, -0.026_789_598_088_992_565, -0.009_538_819_884_776_721, 0.000_308_022_957_256_363_6, 0.008_098_486_299_540_94, 0.016_862_480_806_489_673, 0.028_588_607_812_707_223, 0.044_230_639_998_811_654, 0.077_917_098_939_264_2, 0.091_466_186_188_458_61, 0.119_255_319_523_536_59, 0.135_268_872_362_974_02],
    [-0.653_993_462_600_217_6, -0.293_956_124_981_179_8, -0.178_265_773_479_100_36, -0.110_658_616_897_742_77, -0.095_804_970_515_566_76, -0.084_398_005_320_679_72, -0.070_840_652_152_795_95, -0.041_385_988_985_241_91, -0.021_217_941_077_715_68, -0.000_175_401_065_409_351_27, 0.014_071_105_026_785_925, 0.026_679_541_871_661_806, 0.059_941_134_959_529_345, 0.086_123_436_394_243_37, 0.102_200_338_306_520_41, 0.126_510_640_324_177_97, 0.135_268_872_362_974_02],
    [-0.653_993_462_600_217_6, -0.449_253_421_609_131_9, -0.325_562_621_922_092_84, -0.225_014_139_283_807_78, -0.138_524_790_558_994_9, -0.106_255_536_713_631, -0.087_254_575_702_503_06, -0.072_915_170_558_703_79, -0.049_121_261_940_828_345, -0.032_827_732_724_054_995, -0.016_637_163_448_541_263, 0.005_511_556_937_165_885, 0.014_314_943_467_679_678, 0.049_973_901_055_149_37, 0.074_369_373_108_592_7, 0.119_222_885_021_337_21, 0.135_268_872_362_974_02],
    [-0.653_993_462_600_217_6, -0.305_222_797_334_580_5, -0.204_343_652_313_987_13, -0.151_827_486_996_362_2, -0.129_309_647_273_331_63, -0.108_536_908_583_625_48, -0.089_465_729_015_273_72, -0.076_749_452_624_879_96, -0.069_484_376_867_430_07, -0.059_704_765_375_116_924, -0.049_669_385_986_533_51, -0.039_972_501_591_913_07, -0.026_885_600_074_428_143, -0.010_739_218_633_399_98, 0.011_507_999_824_906_136, 0.065_629_115_387_680_21, 0.254_798_574_967_593_84],
    [-0.653_993_462_600_217_6, -0.202_640_922_856_684, -0.152_134_952_610_529_53, -0.129_052_371_881_347_77, -0.106_546_975_655_581_5, -0.088_275_207_123_593_57, -0.072_834_210_777_224_23, -0.062_931_367_966_792_16, -0.052_174_159_526_010_795, -0.043_004_759_434_701_05, -0.021_264_139_965_363_84, -0.006_130_456_104_651_458, 0.006_817_076_461_936_133_5, 0.035_339_088_376_290_274, 0.095_465_226_778_922_77, 0.188_664_919_520_703_8, 0.254_798_574_967_593_84],
    [-0.304_498_069_373_758_9, -0.179_592_570_654_115_85, -0.129_864_911_349_305_45, -0.098_250_365_946_401_82, -0.073_079_588_923_172_22, -0.059_440_046_515_314_14, -0.047_746_756_279_190_686, -0.031_507_951_433_309_66, -0.015_753_118_679_311_652, -0.002_145_186_143_193_469_5, 0.007_142_057_122_597_726, 0.025_363_548_447_967_146, 0.048_058_198_505_505_94, 0.068_617_491_176_709_9, 0.110_203_761_610_865_76, 0.185_118_816_924_672_7, 0.292_334_753_416_749_1],
    [-0.405_440_017_597_044_2, -0.216_491_789_954_942_78, -0.148_111_945_838_425_66, -0.106_451_575_862_226_46, -0.076_236_372_674_616_39, -0.057_628_112_244_178_145, -0.042_859_654_365_271_79, -0.028_944_464_052_949_773, -0.008_009_904_816_064_358, 0.002_225_662_037_775_560_7, 0.013_139_986_557_375_381, 0.026_276_499_573_143_21, 0.046_373_764_234_082_42, 0.064_571_566_652_273_34, 0.099_414_078_152_738_02, 0.159_542_334_783_201_54, 0.292_334_753_416_749_1],
    [-0.405_440_017_597_044_2, -0.251_122_440_596_392_05, -0.183_527_674_004_010_96, -0.145_191_455_806_323_95, -0.109_825_677_519_122_13, -0.081_401_874_031_596_57, -0.057_049_153_343_968_58, -0.041_205_139_790_735_08, -0.027_124_902_304_508_217, -0.010_420_980_243_174_1, 0.001_402_113_821_225_566_3, 0.010_090_425_884_948_262, 0.018_959_803_779_922_44, 0.035_030_445_008_901_394, 0.058_994_445_759_039_275, 0.098_304_287_598_608_35, 0.292_334_753_416_749_1],
    [-0.405_440_017_597_044_2, -0.200_228_800_799_892_08, -0.122_315_136_427_793_46, -0.095_851_871_814_479_96, -0.074_730_994_225_610_89, -0.050_378_458_527_397_155, -0.038_107_063_048_771_23, -0.024_470_624_712_814_432, -0.012_316_270_497_233_913, -0.000_633_797_737_642_868_2, 0.008_247_620_345_083_353, 0.014_918_615_749_831_71, 0.028_493_958_150_185_076, 0.040_467_056_529_304_46, 0.058_248_275_038_150_34, 0.083_156_672_501_010_19, 0.292_334_753_416_749_1],
    [-0.405_440_017_597_044_2, -0.129_871_533_128_459_45, -0.086_494_098_040_821_74, -0.065_346_619_247_029_53, -0.048_050_857_002_265_766, -0.035_647_391_315_432_674, -0.023_170_436_059_614_205, -0.012_552_482_444_196_917, -0.000_867_735_266_835_398_4, 0.008_534_552_270_539_699, 0.018_003_156_931_390_905, 0.030_525_578_069_693_544, 0.040_553_516_153_742_85, 0.054_233_135_062_347_87, 0.072_459_029_691_623_3, 0.088_285_392_435_471_33, 0.183_334_911_636_011_37],
    [-0.389_488_391_569_674_74, -0.108_112_724_721_626_8, -0.074_775_363_922_954_63, -0.055_895_189_768_792_22, -0.040_907_531_377_828_58, -0.028_267_689_879_090_317, -0.016_564_427_598_866_838, -0.004_957_891_900_969_512, 0.005_612_901_065_832_32, 0.016_753_909_310_408_335, 0.029_118_280_984_810_962, 0.039_376_793_284_855_67, 0.051_505_375_907_408_266, 0.066_601_643_459_077_38, 0.084_028_385_489_325_9, 0.102_403_182_894_093_36, 0.202_613_286_503_485_12],
    [-0.373_322_964_753_286_14, -0.153_078_155_675_547_86, -0.091_462_418_019_311_9, -0.068_980_269_847_865_37, -0.046_050_069_222_228_02, -0.029_511_268_610_368_32, -0.015_883_989_661_120_51, -0.001_424_835_532_854_598_7, 0.012_024_987_810_914_455, 0.023_312_822_321_686_24, 0.036_373_401_325_342_12, 0.048_773_203_213_752_51, 0.061_454_906_133_252_56, 0.078_851_948_549_317_24, 0.096_788_229_892_004_33, 0.126_366_386_058_582_25, 0.395_226_550_811_266_3],
    [-0.373_322_964_753_286_14, -0.245_730_260_851_227_7, -0.183_059_146_189_141_32, -0.134_375_105_219_952_97, -0.097_787_503_083_331_59, -0.070_509_377_446_986_54, -0.041_079_231_636_720_25, -0.013_653_682_045_258_306, 0.010_690_602_729_540_02, 0.027_163_136_299_467_84, 0.045_502_519_027_569_57, 0.057_271_274_519_556_02, 0.075_812_484_405_074_03, 0.098_322_586_448_342_49, 0.141_372_363_057_989_9, 0.184_947_901_143_493_5, 0.514_042_337_340_388],
    [-0.373_322_964_753_286_14, -0.267_471_461_616_799_9, -0.213_897_991_596_652_65, -0.175_015_535_159_689_15, -0.139_492_182_689_236_8, -0.111_265_528_816_523_1, -0.083_715_435_813_659_78, -0.053_747_956_325_427_05, -0.018_825_321_898_967_81, 0.023_714_729_113_886_234, 0.056_399_539_869_708_05, 0.095_251_117_309_673_45, 0.127_549_299_605_704_24, 0.164_655_178_776_335_84, 0.211_369_789_721_195_5, 0.312_754_738_069_985_05, 0.609_973_477_838_170_7],
    [-0.355_542_384_949_490_64, -0.216_314_611_019_250_4, -0.167_795_684_903_112_09, -0.134_256_851_312_267_83, -0.112_242_025_962_072_24, -0.085_688_695_866_875, -0.063_465_682_861_928_66, -0.039_827_014_163_003_51, -0.021_049_501_756_311_113, 0.019_821_468_304_828_36, 0.065_954_881_462_722_87, 0.106_689_780_096_412_54, 0.141_643_722_957_199_99, 0.175_071_543_581_354_4, 0.253_677_828_820_794_7, 0.403_485_112_693_786_56, 0.743_230_475_997_371_8],
    [-0.286_775_150_472_144_75, -0.144_076_251_036_894_42, -0.124_506_239_462_742_32, -0.107_036_211_638_517_6, -0.082_609_057_472_012_79, -0.064_790_995_645_343_77, -0.048_466_940_918_354_53, -0.036_643_626_049_000_28, -0.026_850_331_813_845_13, 0.001_836_426_633_470_718_3, 0.024_596_914_186_528_113, 0.059_475_462_190_469_446, 0.104_173_740_211_210_24, 0.120_479_386_561_464_69, 0.170_253_358_556_870_38, 0.403_849_689_357_808_8, 0.743_230_475_997_371_8],
    [-0.202_315_456_619_058_48, -0.137_544_286_010_973_68, -0.114_730_542_134_507_56, -0.089_975_107_244_475_63, -0.081_268_057_566_727_99, -0.060_309_392_902_170_85, -0.048_460_796_571_576_32, -0.040_334_668_104_915_94, -0.035_346_402_896_863_076, -0.016_420_963_867_997_42, -8.636_914_916_858_729e-5, 0.020_172_379_263_366_25, 0.049_978_721_339_628_746, 0.103_703_285_871_876_77, 0.122_711_062_526_734_83, 0.186_982_669_312_625_44, 0.743_230_475_997_371_8],
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
    [-0.255_464_144_356_835_07, -0.167_586_737_494_564_57, -0.115_757_874_447_443_27, -0.094_249_614_609_690_67, -0.066_631_028_443_631_32, -0.021_251_017_953_460_56, -0.008_989_023_772_661_61, 0.006_219_696_096_674_905, 0.026_944_718_528_690_57, 0.049_686_959_988_441_17, 0.062_526_455_944_085_75, 0.087_341_526_741_451_1, 0.104_004_573_666_220_29, 0.127_716_365_658_365_08, 0.156_661_302_599_056_95, 0.170_565_343_370_386_54, 0.283_253_388_488_978_1],
    [-0.255_464_144_356_835_07, -0.157_599_873_392_047_78, -0.117_444_660_330_503_87, -0.097_207_534_531_522_24, -0.078_928_765_480_730_06, -0.061_041_247_842_920_435, -0.028_827_592_905_313_615, -0.013_919_852_276_432_656, 0.001_678_159_654_749_046_7, 0.017_053_830_341_477_19, 0.046_789_692_349_113_655, 0.058_852_942_217_863_75, 0.085_722_674_955_806_6, 0.105_107_674_018_728_42, 0.147_031_254_952_098_18, 0.164_022_530_765_314_67, 0.283_253_388_488_978_1],
    [-0.255_464_144_356_835_07, -0.129_385_084_505_875_24, -0.103_447_986_930_667_4, -0.090_438_502_807_387_79, -0.077_323_125_769_867_59, -0.066_044_956_003_505_18, -0.044_395_233_996_503_41, -0.034_742_595_518_997_91, -0.015_198_796_823_029_326, -0.002_026_261_168_007_667_4, 0.007_819_667_000_397_058, 0.023_406_052_122_577_87, 0.050_505_851_400_299_12, 0.069_355_455_881_403_96, 0.102_009_105_415_690_12, 0.149_120_955_042_370_36, 0.283_253_388_488_978_1],
    [-0.238_046_308_663_698_8, -0.100_632_368_377_711_94, -0.090_338_604_067_602_8, -0.076_450_032_531_249_75, -0.066_222_603_673_571_13, -0.057_422_185_612_483_36, -0.041_902_474_201_644_196, -0.039_458_187_049_506_2, -0.025_445_050_047_982_254, -0.012_393_295_814_666_727, -0.000_121_531_123_890_570_84, 0.009_482_028_954_023_686, 0.020_209_924_805_325_474, 0.036_990_774_280_802_44, 0.062_282_873_046_700_285, 0.108_752_003_593_136_08, 0.167_179_694_554_398_63],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.255_464_144_356_835_07, -0.132_463_925_518_708_14, -0.103_344_592_894_638_38, -0.088_713_044_938_823_6, -0.069_387_314_027_712_8, -0.044_316_350_673_269_01, -0.027_252_728_636_278_926, -0.013_533_311_933_880_12, 0.000_103_936_236_010_058_25, 0.015_236_740_984_718_165, 0.037_173_339_687_502_735, 0.055_814_328_155_467_975, 0.086_292_144_794_670_66, 0.108_509_915_003_863_46, 0.149_044_904_598_150_48, 0.173_463_120_479_560_94, 0.938_901_394_112_795],
    [-0.285_608_117_672_971_7, -0.127_435_319_999_515_77, -0.089_244_501_596_416_86, -0.068_161_131_606_152_25, -0.052_791_073_420_143_03, -0.041_999_946_959_358_6, -0.030_077_879_817_660_46, -0.018_820_905_571_095_47, -0.005_942_807_926_910_568, 0.011_756_391_871_935_005, 0.030_348_473_493_305_505, 0.050_495_767_731_474_03, 0.068_897_665_739_446_33, 0.087_790_572_770_589_83, 0.107_511_030_314_857_83, 0.175_527_044_316_753_5, 0.326_403_832_353_968_9],
    [-0.260_194_445_346_832_9, -0.096_371_922_638_820_37, -0.046_130_717_188_850_24, -0.044_777_661_742_476_61, -0.031_614_259_431_604_896, -0.026_215_386_745_057_58, -0.020_134_363_698_074_895, -0.009_573_973_445_082_264, 0.000_215_467_147_424_351_43, 0.007_615_311_527_783_975, 0.014_936_306_565_172_12, 0.020_302_865_738_311_755, 0.029_138_782_437_313_75, 0.038_998_280_070_760_87, 0.046_519_615_649_297_01, 0.048_547_914_429_082_63, 0.049_974_227_675_641_8],
    [-0.260_194_445_346_832_9, -0.159_395_215_229_489_82, -0.098_426_633_518_047_98, -0.061_042_614_644_978_99, -0.034_772_802_678_391_59, -0.022_585_897_007_666_02, -0.010_433_113_314_303_006, 0.000_646_377_152_800_950_6, 0.012_460_420_334_223_234, 0.015_590_984_189_157_372, 0.021_765_057_138_677_35, 0.028_131_871_778_164_178, 0.034_569_539_032_919_19, 0.038_134_674_914_408_02, 0.043_273_309_228_663_306, 0.048_616_114_098_757_2, 0.074_009_998_276_399_26],
    [-0.260_194_445_346_832_9, -0.183_399_736_647_274_33, -0.130_283_721_903_955_32, -0.099_384_018_830_324_51, -0.067_174_632_574_530_67, -0.033_436_071_107_687_024, -0.011_502_566_798_031_097, -0.001_479_184_858_155_273_1, 0.012_152_890_999_957_805, 0.016_896_293_078_184_08, 0.026_610_756_583_322_827, 0.031_956_320_386_460_19, 0.035_524_045_385_501_62, 0.038_958_533_311_877_167, 0.047_283_674_326_744_365, 0.063_795_415_380_248_77, 0.109_351_771_975_293_72],
    [-0.260_194_445_346_832_9, -0.181_607_473_712_992_68, -0.140_463_314_656_726_37, -0.112_616_071_924_639_97, -0.088_169_514_989_710_57, -0.067_703_208_204_641_61, -0.028_661_990_270_669_443, -0.007_334_705_056_646_414_5, 0.013_247_602_381_098_381, 0.022_165_638_804_493_525, 0.032_006_760_633_413_97, 0.035_776_897_169_770_94, 0.044_221_176_957_416_61, 0.058_687_235_202_177_24, 0.073_111_986_349_036_4, 0.092_435_959_076_662_84, 0.297_502_933_603_551_73],
    [-0.244_333_444_569_244_83, -0.160_035_616_523_961_2, -0.137_552_983_808_488_3, -0.110_932_307_123_890_4, -0.099_062_334_016_630_96, -0.077_773_869_085_100_3, -0.058_637_471_320_593_45, -0.026_906_231_942_638_11, 0.002_811_360_477_367_209_3, 0.024_168_441_717_846_864, 0.034_073_587_652_398_47, 0.048_275_888_954_593_86, 0.069_985_340_988_945_88, 0.090_095_043_629_291_05, 0.099_576_523_269_862_34, 0.127_455_217_927_339_6, 0.318_563_982_856_447_16],
    [-0.244_333_444_569_244_83, -0.148_256_172_256_427_12, -0.133_619_645_496_525_22, -0.107_373_124_135_291_36, -0.095_027_019_490_266_48, -0.077_048_684_880_752_53, -0.057_932_472_844_069_036, -0.032_733_768_618_973_31, -0.012_171_180_829_403_74, 0.016_382_427_412_075_025, 0.029_193_714_325_406_67, 0.048_475_717_642_897_126, 0.076_961_364_207_009_5, 0.097_373_680_307_047_17, 0.122_351_184_175_978_1, 0.177_798_519_690_202_36, 0.326_403_832_353_968_9],
    [-0.177_421_284_544_752_76, -0.144_784_472_306_714, -0.119_682_312_743_899_37, -0.102_004_723_993_662_44, -0.086_017_449_857_706_28, -0.072_175_050_842_787_58, -0.054_098_340_249_019_716, -0.032_269_773_316_129_61, -0.014_508_344_410_580_083, 0.012_324_913_849_821_85, 0.026_470_871_453_086_16, 0.049_811_648_879_245_81, 0.069_671_470_361_182_71, 0.100_306_190_193_020_14, 0.128_187_149_707_677_67, 0.214_733_924_350_845_85, 0.326_403_832_353_968_9],
    [-0.175_465_902_045_686_07, -0.135_587_729_326_150_99, -0.104_730_943_432_626_62, -0.086_817_106_504_733_27, -0.073_879_610_788_623_58, -0.058_388_923_886_162_41, -0.045_612_228_599_873_49, -0.032_269_753_950_797_42, -0.018_825_473_186_593_895, 0.001_767_918_414_027_954_1, 0.024_931_859_255_909_1, 0.050_791_095_939_088_55, 0.070_544_477_218_394_84, 0.104_051_057_169_900_89, 0.161_104_598_994_693_34, 0.232_449_244_462_666_68, 0.326_403_832_353_968_9],
    [-0.175_465_902_045_686_07, -0.108_984_688_557_801_57, -0.084_834_299_760_293_89, -0.067_547_544_316_137_05, -0.053_305_177_492_988_73, -0.045_764_777_161_281_106, -0.039_057_694_545_209_37, -0.031_181_632_782_427_45, -0.020_152_730_583_122_7, -0.005_997_074_710_731_564_6, 0.021_674_550_170_742_702, 0.050_469_500_411_956_82, 0.076_740_355_975_364_33, 0.113_723_443_487_862_3, 0.203_349_371_661_693_67, 0.237_573_494_262_179_3, 0.326_403_832_353_968_9],
    [-0.186_428_543_271_858_87, -0.094_958_766_891_826_89, -0.070_191_608_621_885, -0.057_374_741_940_920_695, -0.049_904_238_803_931_134, -0.043_101_934_966_789_52, -0.037_423_222_613_147_37, -0.030_368_163_299_300_876, -0.019_231_283_948_664_104, -0.002_516_735_541_323_404_5, 0.021_546_894_713_216_854, 0.044_853_501_116_822_48, 0.072_650_271_855_078_29, 0.105_210_216_498_520_8, 0.189_717_259_312_619_54, 0.227_826_522_656_964_51, 0.322_686_737_902_926_1],
    [-0.186_428_543_271_858_87, -0.098_185_535_556_799_25, -0.072_569_657_568_528_47, -0.060_674_139_416_916_42, -0.052_777_266_162_604_16, -0.047_030_792_122_724_964, -0.036_729_378_931_783_56, -0.027_689_338_426_426_457, -0.016_400_965_383_915_563, 0.001_116_748_882_075_376_6, 0.021_642_004_354_657_112, 0.040_099_218_499_309_78, 0.061_554_347_475_102_696, 0.093_617_714_918_815_4, 0.158_754_841_706_074_82, 0.200_684_984_650_220_4, 0.298_753_236_058_077],
    [-0.248_084_258_530_317_13, -0.113_512_521_441_341_94, -0.081_343_169_046_492_26, -0.063_267_475_619_381_36, -0.053_289_705_504_161_955, -0.042_008_862_519_325_264, -0.029_469_012_672_422_225, -0.018_194_248_400_997_6, -0.005_981_313_987_429_28, 0.009_956_745_186_973_048, 0.026_041_861_253_058_986, 0.046_703_898_460_038_14, 0.069_050_213_539_423_91, 0.094_024_744_614_180_27, 0.125_785_789_791_661_6, 0.164_757_733_742_076_94, 0.284_693_906_517_943_6],
    [-0.248_084_258_530_317_13, -0.133_904_149_838_656_42, -0.085_633_193_875_704_35, -0.060_182_428_107_903_82, -0.042_803_655_314_994_42, -0.029_314_470_753_902_762, -0.016_656_770_427_501_903, -0.004_763_712_114_172_15, 0.011_253_558_072_384_278, 0.030_052_573_748_844_572, 0.052_837_968_891_335_67, 0.068_132_497_830_168_39, 0.077_781_495_093_376_45, 0.093_398_966_677_898_56, 0.104_645_581_575_338_01, 0.132_768_952_324_053_4, 0.235_276_837_856_624_23],
    [-0.248_084_258_530_317_13, -0.129_229_297_040_178_05, -0.072_938_660_510_610_78, -0.048_881_058_687_978_42, -0.033_468_740_871_227_48, -0.025_493_343_993_456_66, -0.012_757_217_584_923_895, -0.000_236_579_982_397_967_55, 0.026_889_945_389_687_846, 0.053_351_413_793_056_49, 0.063_764_940_599_284_89, 0.070_899_304_850_019_98, 0.076_179_404_307_331_1, 0.084_389_268_453_832_68, 0.094_740_038_161_162_96, 0.103_306_797_366_578_73, 0.200_717_577_519_210_82],
    [-0.248_084_258_530_317_13, -0.085_578_896_774_774_6, -0.056_121_838_042_799_75, -0.043_693_429_632_375_61, -0.030_230_637_508_205_61, -0.023_085_795_079_809_5, -0.013_600_809_470_812_445, -0.005_551_406_421_029_989, 0.008_565_348_183_748_57, 0.040_272_758_106_411_7, 0.057_464_585_822_349_874, 0.067_910_820_904_737_77, 0.074_076_772_698_915_58, 0.081_428_644_262_725_14, 0.088_949_716_374_110_84, 0.098_107_565_874_974_16, 0.144_970_956_769_633_56],
    [-0.192_523_418_947_619_48, -0.058_827_389_656_356_86, -0.043_731_455_901_795_91, -0.031_060_795_293_000_515, -0.024_766_826_542_274_942, -0.019_900_825_127_746_977, -0.012_168_896_456_828_024, -0.007_416_048_431_579_896, -0.003_288_128_226_357_375_5, 0.002_548_138_523_363_438_7, 0.010_205_324_342_850_97, 0.035_139_595_251_871_804, 0.063_269_951_342_713_82, 0.076_597_489_774_841_08, 0.087_997_103_008_576_5, 0.094_158_956_466_458, 0.112_079_895_000_327_33],
    [-0.484_934_665_663_733_2, -0.400_664_808_832_842_7, -0.387_244_861_899_906_1, -0.377_478_073_994_585_5, -0.355_402_042_495_684_5, -0.304_329_426_808_923_8, -0.266_999_995_009_169_8, -0.086_968_434_740_426_34, 0.010_082_986_684_704_335, 0.046_280_606_077_327_355, 0.058_655_804_954_370_82, 0.065_508_383_844_732_5, 0.072_773_935_984_058_46, 0.085_938_599_596_969_52, 0.092_689_851_521_650_57, 0.097_940_337_528_338_18, 0.126_570_488_334_599_2],
    [-0.484_934_665_663_733_2, -0.402_739_401_442_126, -0.387_612_062_846_388_27, -0.379_415_277_445_954, -0.365_521_808_670_946_5, -0.310_216_337_929_699_55, -0.267_719_046_924_972_34, -0.172_983_873_666_138_04, -0.001_224_054_945_584_236_4, 0.016_465_196_228_165_713, 0.057_479_843_594_588_16, 0.071_972_034_515_428_33, 0.083_931_735_812_386_8, 0.094_143_806_410_935_52, 0.100_445_202_899_231_15, 0.121_032_948_018_280_5, 0.268_431_751_102_946_7],
    [-0.484_934_665_663_733_2, -0.387_168_273_541_095_73, -0.369_645_381_498_296_5, -0.324_728_831_834_965_76, -0.265_429_764_287_923_03, -0.188_990_012_397_933_42, -0.111_479_352_534_813_86, -0.049_971_525_177_131_38, -0.010_017_099_544_031_256, 0.052_319_976_645_031_13, 0.079_923_810_982_399_78, 0.097_134_917_530_368_35, 0.112_628_027_418_702_94, 0.123_923_692_984_649_27, 0.162_221_967_757_378_24, 0.256_795_064_010_052_64, 0.364_389_131_429_970_5],
    [-0.484_934_665_663_733_2, -0.221_509_080_417_947_48, -0.144_427_446_807_809_36, -0.122_275_034_273_879_34, -0.109_463_190_963_532_84, -0.099_172_167_663_067_88, -0.073_534_511_798_516_47, -0.024_101_747_702_832_608, -0.010_635_504_048_678_732, 0.079_281_737_574_726_04, 0.117_137_346_015_777_45, 0.162_236_922_546_302_68, 0.269_421_451_361_024_6, 0.307_172_637_866_421_1, 0.343_468_629_913_666_06, 0.352_278_845_615_401_7, 0.364_389_131_429_970_5],
    [-0.322_019_219_949_621_6, -0.144_418_458_549_112_53, -0.121_889_017_460_951_8, -0.105_654_350_891_008_6, -0.098_177_305_268_300_54, -0.074_693_549_492_019_66, -0.043_298_874_773_439_57, -0.024_604_152_801_855_81, -0.007_197_894_871_339_123, 0.036_706_376_641_460_2, 0.279_157_951_637_732_1, 0.297_666_650_514_761_9, 0.313_408_039_883_832_03, 0.329_036_517_337_899_5, 0.343_492_076_308_079_7, 0.352_369_706_440_836_15, 0.380_099_862_439_780_67],
    [-0.189_302_711_152_969_7, -0.147_555_901_259_351_2, -0.138_358_050_042_566_24, -0.105_358_525_496_339_9, -0.095_720_241_237_607_78, -0.073_313_285_392_806_3, -0.048_349_733_385_708_27, -0.023_735_301_372_280_224, -0.006_609_647_856_001_399, 0.039_440_704_529_305_74, 0.275_460_074_192_251_66, 0.296_155_498_655_664_93, 0.308_462_885_409_487_2, 0.326_812_871_354_904_17, 0.339_908_005_141_730_6, 0.351_682_963_289_731_1, 0.393_583_643_017_133_06],
    [-0.588_156_537_328_690_7, -0.170_588_122_427_602, -0.154_686_543_924_113_2, -0.138_711_166_283_630_92, -0.111_294_124_646_287_92, -0.096_340_232_301_283_65, -0.079_842_034_578_742_2, -0.040_512_759_014_278_64, -0.006_904_896_190_538_481, 0.040_837_189_740_688_94, 0.215_563_732_975_778_9, 0.253_571_940_320_982_96, 0.274_690_733_894_888_13, 0.297_029_509_898_693_6, 0.317_796_514_394_106_3, 0.343_554_826_800_480_4, 0.398_998_014_355_687],
    [-0.588_156_537_328_690_7, -0.218_345_584_751_373_22, -0.180_199_414_405_048_6, -0.163_756_254_087_677_22, -0.143_659_976_209_845_16, -0.113_051_217_356_056_3, -0.096_736_924_159_507_13, -0.072_585_515_847_983_99, -0.014_539_768_132_199_687, 0.117_245_664_404_122_83, 0.175_999_694_272_110_5, 0.202_221_351_053_039_04, 0.224_984_194_151_341_4, 0.249_421_115_862_970_86, 0.274_244_508_580_519_85, 0.307_095_359_427_891, 0.398_998_014_355_687],
    [-0.606_247_161_633_360_6, -0.276_844_691_325_433_8, -0.223_129_058_057_533_14, -0.184_936_342_384_586_64, -0.163_183_240_091_714_4, -0.138_626_532_450_703_1, -0.106_882_697_480_056_26, -0.074_802_764_417_532_31, -0.023_447_631_461_025_957, 0.080_174_170_363_568_83, 0.131_426_298_020_342_72, 0.164_432_138_070_286_8, 0.179_154_988_912_308_27, 0.201_425_287_059_579_88, 0.221_841_951_514_232_7, 0.257_629_172_353_594_4, 0.398_998_014_355_687],
    [-0.606_247_161_633_360_6, -0.350_544_807_803_265_7, -0.251_174_886_235_000_8, -0.182_024_589_307_206_36, -0.153_340_123_610_196_14, -0.132_073_793_893_708_3, -0.096_044_568_012_658_76, -0.058_524_068_499_594_455, -0.020_281_848_561_592_74, 0.042_097_621_112_223_32, 0.080_153_550_858_193_48, 0.108_709_184_476_693_64, 0.140_301_215_927_598_56, 0.162_342_627_514_489_22, 0.179_701_560_698_377_48, 0.211_800_863_468_741_47, 0.398_998_014_355_687],
    [-0.606_247_161_633_360_6, -0.297_084_667_728_330_1, -0.180_636_497_340_786_32, -0.149_755_817_643_156_54, -0.131_639_524_879_924_1, -0.099_895_411_993_235_54, -0.070_188_047_775_820_01, -0.044_211_158_424_477_65, -0.010_540_771_010_649_645, 0.022_751_458_096_578_865, 0.053_758_500_176_056_62, 0.075_098_133_741_095_07, 0.089_592_040_464_652_6, 0.111_871_806_097_106_59, 0.140_023_313_041_825_8, 0.170_982_999_658_427_64, 0.324_466_506_687_670_1],
    [-0.606_247_161_633_360_6, -0.228_398_120_194_051_9, -0.167_951_516_876_413_02, -0.138_793_304_457_580_23, -0.110_030_221_874_403_8, -0.078_039_056_797_492_72, -0.050_855_309_930_512_516, -0.020_452_945_011_649_49, 0.006_283_749_134_784_696, 0.025_023_451_890_223_28, 0.048_433_487_088_631_83, 0.069_135_891_248_181_27, 0.081_976_627_561_649_97, 0.099_454_457_017_376_62, 0.114_061_031_994_777_67, 0.140_048_428_186_479_53, 0.255_291_241_117_324_26],
    [-0.577_797_111_976_815_9, -0.241_984_031_577_965_94, -0.181_170_582_992_654_26, -0.145_318_435_084_810_46, -0.109_684_187_143_671_73, -0.073_260_272_254_015_2, -0.045_221_719_846_250_89, -0.020_200_205_582_148_984, 0.001_575_043_593_446_419_8, 0.019_859_909_111_170_69, 0.040_702_556_636_025_816, 0.060_544_781_301_773_17, 0.077_841_456_293_190_9, 0.093_955_591_454_419_73, 0.113_528_848_325_221_63, 0.137_461_972_205_955_13, 0.255_723_825_251_186_53],
    [-0.516_922_498_910_167_7, -0.230_443_548_959_463_53, -0.188_175_072_306_868_1, -0.153_652_300_392_485_76, -0.108_990_835_813_919_88, -0.071_088_682_956_069_58, -0.046_108_402_459_433_45, -0.023_229_566_913_942_165, -0.004_722_888_897_815_646, 0.015_718_613_831_557_468, 0.034_316_665_529_812_18, 0.051_760_274_175_896_75, 0.070_971_572_152_569_07, 0.085_303_499_045_975_66, 0.105_207_669_127_296_61, 0.130_914_602_726_621_2, 0.255_723_825_251_186_53],
    [-0.387_199_696_886_225, -0.218_918_849_784_594_28, -0.184_925_173_013_936_24, -0.149_869_405_826_286_55, -0.101_370_011_006_755_1, -0.066_986_120_668_622_9, -0.041_930_990_230_316_566, -0.016_933_576_316_312_583, 0.000_684_968_378_964_870_1, 0.018_763_237_121_270_152, 0.035_734_996_147_414_49, 0.050_750_694_869_470_34, 0.067_777_356_790_278_96, 0.083_999_587_314_184_35, 0.100_161_487_639_939_36, 0.122_518_687_660_294, 0.255_723_825_251_186_53],
    [-0.319_595_287_616_271_3, -0.201_973_168_880_816_1, -0.170_633_843_788_847_3, -0.135_174_028_659_104_22, -0.091_814_890_529_287_2, -0.055_578_192_077_125_575, -0.031_384_928_797_753_59, -0.009_920_533_740_621_865, 0.009_170_671_610_866_3, 0.025_046_801_514_147_697, 0.038_721_344_424_224_72, 0.052_022_638_879_965_94, 0.069_001_291_641_654_44, 0.084_194_366_539_966_48, 0.099_877_273_535_725_6, 0.121_176_328_112_954_84, 0.255_723_825_251_186_53],
    [-0.246_528_087_973_834_4, -0.170_404_105_527_884_35, -0.145_050_253_767_628_65, -0.100_098_785_058_136_56, -0.075_811_286_297_701_06, -0.049_556_103_844_396_23, -0.024_534_856_231_418_517, -0.009_907_826_989_225_35, 0.007_872_405_651_884_368, 0.022_344_716_462_599_92, 0.035_598_057_234_772_65, 0.046_754_880_417_394_465, 0.066_183_086_107_974_94, 0.083_805_638_641_785_78, 0.101_621_298_932_328_62, 0.127_619_680_827_293_22, 0.227_087_841_371_624_22],
    [-0.340_176_572_255_805, -0.251_143_997_382_963, -0.224_224_531_200_740_8, -0.201_745_079_034_320_7, -0.162_381_532_685_761_91, -0.116_370_853_157_684_17, -0.042_828_966_840_380_706, -0.014_717_183_026_832_786, 0.017_737_915_504_747_938, 0.076_257_698_241_177_38, 0.125_901_803_575_826_75, 0.162_766_274_197_797_96, 0.215_956_341_342_495_72, 0.296_447_372_118_709_6, 0.382_593_032_544_843_95, 0.457_619_716_012_156_06, 0.606_883_116_638_695],
    [-0.357_872_118_173_570_9, -0.274_374_975_693_473_6, -0.230_897_941_024_123_14, -0.201_740_530_887_192_18, -0.173_929_805_941_761_47, -0.135_777_436_463_694_83, -0.081_767_391_046_428_88, -0.032_677_156_714_595_17, -0.000_658_230_607_411_865, 0.061_836_142_192_404_77, 0.125_985_410_947_356_52, 0.165_933_029_741_871_68, 0.215_539_235_451_624_37, 0.303_357_371_158_509_75, 0.383_599_336_142_813_5, 0.478_177_777_872_330_5, 0.606_883_116_638_695],
    [-0.435_780_477_874_603_3, -0.300_109_848_565_059_46, -0.245_822_735_231_683_77, -0.210_194_658_565_854_73, -0.185_858_911_315_940_32, -0.151_200_079_803_230_28, -0.114_423_331_451_079_88, -0.058_166_831_304_511_155, -0.005_294_031_565_156_084, 0.054_781_144_043_866_026, 0.125_020_385_763_396_27, 0.177_831_921_506_924_42, 0.218_301_553_012_257_26, 0.305_946_465_315_844_96, 0.396_111_421_094_197_3, 0.518_431_788_441_611_4, 0.633_138_921_632_792_1],
    [-0.577_124_510_103_445_2, -0.320_607_759_844_113, -0.256_078_263_513_351_1, -0.229_377_166_693_438_97, -0.195_149_869_766_360_6, -0.157_624_364_480_324_5, -0.129_906_401_110_096_08, -0.067_331_198_360_216_45, -0.007_374_041_008_188_718, 0.052_052_747_578_884_26, 0.112_624_563_433_593_32, 0.183_668_289_825_726_36, 0.254_460_722_083_956_9, 0.320_660_249_125_490_7, 0.411_358_757_505_240_54, 0.538_559_423_632_924_3, 0.645_191_622_448_802_3],
    [-0.730_549_240_622_719_9, -0.357_089_960_961_891, -0.288_535_318_467_825_15, -0.232_444_045_153_803_43, -0.199_875_036_079_272_2, -0.155_371_573_287_808_46, -0.109_727_554_982_524_07, -0.058_983_682_214_136_15, -0.010_712_124_043_073_548, 0.051_271_621_119_619_82, 0.103_133_231_421_270_01, 0.173_477_954_736_026_78, 0.252_548_434_914_285_2, 0.330_119_689_917_198_37, 0.424_038_480_613_862_84, 0.540_524_647_623_480_8, 0.660_834_024_616_543],
    [-0.730_549_240_622_719_9, -0.387_653_448_725_805_1, -0.309_052_324_492_441_1, -0.234_347_524_674_725_44, -0.191_225_584_011_208_1, -0.142_258_639_495_720_1, -0.088_014_297_429_554_97, -0.046_806_625_350_958_454, -8.334_289_219_549_817e-5, 0.052_916_613_310_746_21, 0.104_862_810_661_701_44, 0.154_900_090_554_849_2, 0.230_214_152_292_929_24, 0.304_148_028_362_968_6, 0.410_386_597_619_054_5, 0.487_847_839_449_002, 0.660_834_024_616_543],
    [-0.811_417_731_120_897_7, -0.397_267_206_046_376_94, -0.311_414_776_013_982_93, -0.244_195_446_729_603_3, -0.184_323_721_628_799_43, -0.127_224_473_908_284_54, -0.088_148_162_196_850_29, -0.042_588_905_360_740_6, 0.003_298_269_977_089_14, 0.051_306_278_065_274_84, 0.100_693_045_890_621_39, 0.155_589_574_745_462_73, 0.213_910_786_713_260_14, 0.282_443_975_602_668, 0.370_701_874_045_210_27, 0.476_917_002_672_583_4, 0.689_739_368_673_047_1],
    [-0.811_417_731_120_897_7, -0.403_982_787_621_656_2, -0.303_838_755_552_067_46, -0.241_207_684_692_648_1, -0.168_875_657_238_336_6, -0.115_446_974_680_503_95, -0.090_292_140_313_115_25, -0.050_960_364_780_884_386, -0.001_881_999_512_324_943_4, 0.038_474_698_569_474_185, 0.088_867_512_420_286_95, 0.145_268_023_260_170_05, 0.200_197_603_585_320_63, 0.262_686_873_034_65, 0.342_707_436_723_814_1, 0.466_177_636_318_609_65, 0.689_739_368_673_047_1],
    [-0.811_417_731_120_897_7, -0.404_717_292_822_349_03, -0.302_199_034_649_081_2, -0.229_136_948_193_377_8, -0.157_861_746_595_601_57, -0.115_650_737_419_767_16, -0.090_317_527_703_412_28, -0.052_513_492_897_112_01, -0.000_802_407_611_196_016_1, 0.032_836_494_459_989_485, 0.076_143_877_788_400_62, 0.133_599_055_285_527_3, 0.184_799_480_579_005_14, 0.240_112_862_902_708_4, 0.319_919_981_048_462_8, 0.449_267_021_299_392_83, 0.689_739_368_673_047_1],
    [-0.811_417_731_120_897_7, -0.404_730_186_963_080_16, -0.308_609_113_806_938_2, -0.228_877_135_527_177_9, -0.157_690_777_749_718_26, -0.117_058_179_449_392_73, -0.081_457_557_276_770_27, -0.036_908_149_095_059_475, 0.004_791_858_610_148_707, 0.035_255_881_905_602_686, 0.068_551_168_147_353_15, 0.128_364_635_767_660_6, 0.188_551_145_093_034_72, 0.241_402_875_079_967_05, 0.317_061_366_032_528_44, 0.464_581_746_640_104_15, 0.692_197_646_476_872_2],
    [-1.420_784_585_102_051_5, -0.432_697_347_682_548_47, -0.342_512_651_753_623_94, -0.259_996_216_401_329_75, -0.217_567_144_316_726_22, -0.134_044_666_108_199_9, -0.085_275_328_310_602_08, -0.040_755_323_430_473_375, -0.008_095_278_540_261_94, 0.026_106_103_850_505_644, 0.074_057_980_120_908_51, 0.115_621_141_167_216_79, 0.190_865_506_225_213_56, 0.239_256_969_309_173_53, 0.313_074_565_735_668_94, 0.485_927_284_729_340_2, 0.692_197_646_476_872_2],
    [-1.420_784_585_102_051_5, -0.512_567_655_290_683, -0.405_994_345_821_736_1, -0.322_805_082_653_787_43, -0.257_202_934_364_124_84, -0.180_314_857_320_894_5, -0.124_982_813_726_587_6, -0.067_173_965_232_525_5, -0.024_336_314_861_852_26, 0.012_882_191_183_053_353, 0.072_840_685_770_422_8, 0.112_530_894_795_323_38, 0.185_019_691_270_064_8, 0.237_592_888_632_725_54, 0.312_697_139_216_801_4, 0.447_638_482_238_186_9, 0.692_197_646_476_872_2],
    [-1.436_290_988_303_229, -0.598_321_669_362_158_9, -0.447_553_899_700_420_5, -0.361_497_065_401_504_6, -0.273_508_508_138_146_3, -0.191_063_455_186_102_68, -0.127_474_607_129_348_36, -0.071_205_630_771_113_43, -0.014_868_421_065_079_736, 0.033_193_210_340_998_53, 0.099_164_137_819_109_04, 0.144_844_378_855_203_82, 0.203_567_627_619_485_44, 0.264_230_233_626_325_86, 0.328_170_906_687_335_4, 0.403_260_738_943_481_13, 0.692_197_646_476_872_2],
    [-1.436_290_988_303_229, -0.658_917_921_309_082, -0.520_153_180_560_394_5, -0.393_254_254_654_668_84, -0.278_268_238_951_242_64, -0.184_855_315_580_425_67, -0.111_867_767_189_969_52, -0.037_727_181_086_770_534, 0.019_845_575_202_441_67, 0.071_316_252_496_923_4, 0.122_923_093_539_294_3, 0.168_281_166_830_684_7, 0.213_411_485_688_967_34, 0.270_603_560_843_882_7, 0.316_497_484_635_349_2, 0.379_076_236_506_285_75, 0.561_575_431_314_210_5],
    [-1.436_290_988_303_229, -0.674_467_676_450_985_7, -0.515_107_385_997_629, -0.388_980_168_909_034_4, -0.266_388_888_478_486_2, -0.169_514_412_190_453_63, -0.096_113_969_744_138_62, -0.022_801_703_689_039_686, 0.029_542_478_337_628_91, 0.074_214_680_133_263_6, 0.113_078_487_612_340_21, 0.145_307_155_353_571_33, 0.193_845_287_777_832_16, 0.225_029_606_643_059_85, 0.279_058_216_286_112_6, 0.328_764_393_168_412_05, 0.503_763_047_526_687_7],
    [-1.436_290_988_303_229, -0.677_390_447_606_989_4, -0.470_712_591_421_966_86, -0.350_315_415_401_203_03, -0.233_984_636_300_728_05, -0.157_420_734_354_721_6, -0.095_639_434_660_333_99, -0.032_603_821_977_437_48, 0.020_918_625_579_833_806, 0.061_332_577_120_117_93, 0.092_497_249_113_004_53, 0.123_894_620_116_629_33, 0.167_354_677_088_851_7, 0.205_731_432_463_253_68, 0.233_492_696_369_504_2, 0.282_400_779_885_775_83, 0.400_184_343_475_613_6],
    [-1.164_051_209_874_257_3, -0.686_937_925_639_449_7, -0.456_928_498_549_294_74, -0.353_430_000_847_864_25, -0.245_505_378_112_876_73, -0.178_822_496_358_932_16, -0.110_425_101_683_837_43, -0.067_215_355_310_649_36, -0.014_713_779_526_391_784, 0.038_049_054_248_510_41, 0.078_946_956_520_843_17, 0.112_953_457_314_247_51, 0.152_357_497_336_046_87, 0.183_646_794_791_843, 0.223_166_576_314_538_87, 0.276_215_840_285_319_87, 0.337_289_013_838_844_9],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_29: [(f64, f64); 11] = [
    (0.0, 1.0),
    (3.079_455_469_509_472_5, 5.410_306_128_901_295),
    (-10.535_321_417_930_039, -7.775_387_433_100_905),
    (-12.0, -8.5),
    (-10.054_407_683_192_723, -7.558_972_820_267_979),
    (-0.268_626_165_636_977, 1.736_723_143_815_747),
    (-1.0, 0.1),
    (1.738_363_195_707_885_6, 6.136_954_472_824_994_5),
    (0.545_656_281_089_801_1, 1.403_283_747_629_163_3),
    (-1.324_005_532_674_345_4, 0.167_802_210_183_634_06),
    (0.270_709_675_080_002_93, 0.893_364_187_226_787),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_29: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.255_296_325_384_692, 1.065_646_958_294_410_6, 1.065_646_958_294_410_6, 1.136_426_373_669_9, 1.177_926_550_976_836_6, 1.191_432_969_234_772_7, 1.175_135_815_592_988_3, 1.161_790_615_391_08, 1.152_971_118_946_034, 1.141_242_472_796_460_7, 1.124_385_016_900_318_2, 1.130_522_819_493_955_2, 1.124_319_968_301_812, 1.109_876_773_035_399_1, 1.108_205_338_901_289_1, 1.109_843_737_780_821_2, 1.102_136_570_469_523_6],
    [0.426_387_110_144_478_55, 0.372_013_784_791_427_7, 0.391_478_680_199_051_85, 0.971_703_929_948_255_7, 1.033_620_493_983_27, 1.002_070_067_008_568_7, 0.955_078_765_557_400_8, 0.932_765_030_854_247, 0.905_249_446_888_019_6, 0.869_665_566_375_269_3, 0.820_210_104_122_109_3, 0.774_344_128_899_018_8, 0.722_351_187_399_242, 1.249_514_140_719_662, 1.320_438_229_053_984, 1.227_473_233_575_388_8, 0.868_212_273_113_022_3],
    [0.0, -0.043_75, -0.087_5, -0.131_249_999_999_999_98, -0.175, -0.218_75, -0.262_499_999_999_999_96, -0.306_249_999_999_999_97, -0.35, -0.393_75, -0.437_5, -0.481_249_999_999_999_96, -0.524_999_999_999_999_9, -0.568_75, -0.612_499_999_999_999_9, -0.656_25, -0.7],
    [1.951_127_281_357_212_3, 1.958_048_269_549_322_9, 2.028_903_318_778_159_5, 2.128_675_160_818_731_5, 2.170_205_509_156_235_6, 2.078_027_786_417_166_6, 2.017_545_975_581_15, 2.040_327_730_360_165_7, 2.017_545_975_581_15, 2.017_545_975_581_15, 2.017_545_975_581_15, 2.017_545_975_581_15, 2.017_545_975_581_15, 2.017_545_975_581_15, 2.017_545_975_581_15, 2.017_545_975_581_15, 2.017_545_975_581_15],
    [-1.224_030_251_234_335_3, -1.239_577_734_289_407_7, -1.074_532_594_187_800_6, -0.903_316_117_607_871_4, -0.722_723_812_388_546_7, -0.624_481_153_262_451_6, -0.536_615_622_490_613_6, -0.270_850_392_195_551_8, -0.123_025_440_922_779_14, -0.084_201_616_756_062_2, -0.063_250_341_115_812_29, -0.048_783_970_564_886_95, -0.049_426_099_052_301_93, -0.141_628_643_480_397_28, -0.268_587_143_141_168_4, -0.345_648_832_344_592_37, -0.387_544_156_898_697_1],
    [0.0, -0.137_5, -0.275, -0.412_500_000_000_000_03, -0.55, -0.687_5, -0.825_000_000_000_000_1, -0.962_500_000_000_000_1, -1.1, -1.237_5, -1.375, -1.512_500_000_000_000_2, -1.650_000_000_000_000_1, -1.787_5, -1.925_000_000_000_000_3, -2.062_5, -2.2],
    [-0.448_444_177_711_024_3, -0.499_929_449_029_641_26, -0.569_899_861_985_908_7, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4, -0.602_123_677_642_527_4],
    [0.313_152_836_889_990_6, 0.376_881_557_912_596_2, 0.442_343_544_940_346_4, 0.476_228_148_759_899_3, 0.481_146_737_863_983_43, 0.456_348_431_014_279_63, 0.431_929_185_204_438_85, 0.437_329_524_094_555_83, 0.462_158_830_193_314_6, 0.501_354_748_158_597_5, 0.545_366_928_323_589_4, 0.597_096_135_652_397_3, 0.652_980_536_168_888_5, 0.735_071_090_644_767_8, 0.799_078_964_002_644_2, 0.801_753_969_068_177_4, 0.793_524_578_983_625_3],
    [0.234_433_560_700_501_4, 0.214_575_729_794_121_73, 0.186_353_167_266_156_97, -0.016_092_733_926_683_214, -0.016_092_733_926_683_214, 0.007_950_303_508_438_849, 0.025_404_295_716_517_877, 0.044_939_167_544_294_445, 0.112_621_836_776_284_11, 0.163_662_739_530_974_6, 0.188_854_702_186_503_77, 0.220_249_332_295_745_8, 0.222_676_845_109_867_68, 0.220_390_599_044_332_67, 0.229_732_429_428_806_26, 0.241_793_482_630_948_77, 0.247_014_150_509_760_47],
    [9.521_958_464_693_942, 9.467_661_111_392_209, 9.406_341_123_986_309, 9.372_915_726_533_082, 9.360_642_881_220_814, 9.370_971_927_443_762, 9.375_755_666_487_617, 9.370_204_460_706_26, 9.372_604_767_410_609, 9.386_150_714_144_106, 9.386_150_714_144_106, 9.386_150_714_144_106, 9.470_886_738_513_21, 9.586_735_920_993_444, 9.689_063_192_411_808, 9.720_344_029_339_91, 9.686_189_656_678_529],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_29: [(f64, f64); 11] = [
    (2.094_084_967_506_009_6, 5.418_370_850_344_337_5),
    (0.354_130_180_715_045, 1.38),
    (-0.356_948_201_150_830_5, 1.930_396_850_291_960_2),
    (-1.045_757, 0.038_753),
    (1.667_602_297_987_058_8, 7.145_041_221_830_653),
    (-1.617_989_701_238_915_5, 0.363_524_866_293_374_1),
    (-2.301_03, -0.698_97),
    (-0.746_275_547_186_667_8, 0.336_777_716_470_267_6),
    (0.027_544_719_217_018_96, 0.911_945_639_038_323_8),
    (-0.536_091_065_693_653_5, 0.479_445_425_224_087_34),
    (8.037_121_290_432_072, 10.078_348_360_620_978),
];

/// The fast accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_DONOR_KINDS_29: [f64; 3] = [
    0.991_611_356_373_069_6,
    0.0,
    0.008_388_643_626_930_354,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16 over all their
/// members, in [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_29: [[f64; 17]; 11] = [
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [-0.723_275_126_768_336_9, -0.331_869_240_240_68, -0.257_811_692_886_370_6, -0.210_234_746_580_135_16, -0.164_474_735_618_867_58, -0.106_046_039_828_374_28, -0.065_499_809_171_619_5, -0.031_704_247_601_867_35, -0.003_083_998_955_960_351_4, 0.030_105_698_527_668_42, 0.066_184_228_424_554_17, 0.102_934_004_193_623_56, 0.143_830_413_231_181_12, 0.220_704_154_028_089_6, 0.312_969_416_828_199_4, 0.401_595_834_562_379_57, 0.730_809_433_035_948_8],
    [-1.344_170_630_435_599, -0.389_596_589_141_571_47, -0.209_370_948_515_217_93, -0.107_171_736_993_723_45, -0.052_506_776_108_157_543, -0.027_405_361_549_322_71, -0.014_803_981_473_723_577, -0.006_899_413_463_100_695, 0.001_455_640_461_801_186_5, 0.011_868_368_209_334_032, 0.021_865_207_378_506_196, 0.037_927_731_195_248_97, 0.073_567_385_011_002_45, 0.135_047_981_941_438_27, 0.209_457_451_694_780_1, 0.379_576_807_204_029, 1.484_495_518_323_049_3],
    [-0.466_213_505_517_060_37, -0.021_541_429_238_782_144, -0.017_686_208_580_568_547, -0.014_477_584_580_067_081, -0.010_979_245_413_443_979, -0.008_433_688_838_672_378, -0.006_423_456_794_777_639, -0.004_623_897_242_449_486, -0.002_849_243_421_175_967_4, -0.000_858_547_212_262_151_8, 0.000_979_667_759_142_775, 0.002_700_981_420_255_158, 0.005_596_100_208_640_479, 0.010_640_760_789_096_305, 0.015_532_300_278_292_938, 0.020_757_640_039_003_128, 0.395_947_944_721_579_57],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.864_409_039_538_360_6, -0.149_445_011_560_881_64, -0.110_648_016_836_538_52, -0.085_475_677_776_861_24, -0.063_919_587_653_983_09, -0.042_726_954_612_412_23, -0.023_997_359_121_184_853, -0.006_958_944_263_112_808, 0.008_743_115_674_475_426, 0.027_138_958_954_612_48, 0.049_046_447_543_239_35, 0.097_091_934_489_805_8, 0.165_583_428_652_168_44, 0.331_080_089_093_356_4, 0.547_395_100_907_802_2, 0.691_751_559_050_989_1, 0.841_994_029_494_507_9],
    [-0.101_628_726_896_669_35, -0.017_331_771_672_381_785, -0.011_214_972_067_546_704, -0.006_030_305_790_202_95, -0.002_908_295_355_516_265_7, 0.000_460_262_679_591_873_8, 0.002_942_232_289_080_840_3, 0.004_466_401_999_327_165, 0.005_300_115_806_597_054, 0.005_962_650_409_777_112, 0.006_729_926_611_308_663, 0.007_325_224_328_440_065, 0.007_880_483_646_882_456, 0.008_613_097_361_920_851, 0.009_366_525_389_080_465, 0.010_331_279_441_503_093, 2.260_880_493_270_129],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.462_761_441_031_369_1, -0.124_931_538_867_610_59, -0.081_694_790_719_021_54, -0.057_373_184_002_034_93, -0.041_643_191_084_230_445, -0.029_238_464_102_036_506, -0.019_111_481_754_323_586, -0.009_100_262_290_476_54, 0.002_034_242_543_918_928_5, 0.013_833_891_394_418_905, 0.025_529_267_232_238_403, 0.036_660_563_075_768_64, 0.047_982_670_344_852_44, 0.060_300_929_370_655_905, 0.106_099_744_513_726_02, 0.212_258_421_351_083_6, 0.338_488_021_998_251_56],
    [-0.920_755_937_186_128_8, -0.680_610_934_430_371_5, -0.551_769_398_100_469_1, -0.427_225_004_788_398_4, -0.290_073_154_300_820_26, -0.168_464_268_428_163_1, -0.089_109_972_643_901_51, -0.038_013_012_969_409_34, 0.000_666_433_591_544_125_4, 0.031_968_927_244_966_85, 0.058_097_752_460_769_57, 0.085_454_735_533_281_62, 0.118_175_057_094_057_25, 0.172_683_958_903_959_05, 0.319_809_729_873_525_7, 0.596_685_982_139_133_2, 0.901_592_941_214_191_6],
    [-1.851_802_123_752_937_8, -0.437_365_411_648_196_64, -0.300_721_627_312_742_7, -0.210_663_338_606_642_77, -0.147_885_734_532_717_67, -0.099_970_995_801_931_25, -0.060_434_189_454_490_44, -0.027_996_132_247_367_533, 0.001_264_165_131_009_542, 0.027_771_416_204_210_926, 0.050_655_243_333_085_29, 0.071_577_676_949_349_48, 0.091_154_278_524_341_62, 0.110_150_423_243_706_43, 0.128_766_772_377_342_83, 0.154_324_165_182_861, 0.244_100_836_069_868_88],
];

/// The slow accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's
/// 17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the
/// recurrence's, which is unconditional, are not read.
#[rustfmt::skip]
pub static AWD_SLOW_NODE_RESIDUALS_29: [[f64; 17]; 187] = [
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [5.419_185_498_579_54, 5.868_277_318_418_104, 6.142_473_289_203_269, 6.331_591_903_290_811_5, 6.503_740_366_156_355_5, 6.665_103_567_595_873, 6.785_266_575_972_596, 6.895_376_375_606_936, 6.996_964_526_872_715, 7.172_724_795_598_079, 7.570_941_967_963_136, 7.896_105_076_670_118, 8.175_426_272_308_716, 8.409_620_209_370_852, 8.703_347_645_398_06, 10.0, 10.0],
    [-0.721_105_341_092_330_2, -0.345_976_185_670_864_63, -0.262_025_923_584_140_3, -0.198_721_949_259_072_5, -0.145_207_972_738_349_9, -0.096_983_233_293_350_26, -0.059_561_054_698_059_01, -0.025_446_395_746_030_17, 0.004_864_059_038_077_523, 0.031_750_970_906_156_195, 0.052_264_761_991_158_704, 0.071_357_952_894_088_15, 0.087_639_643_548_214_52, 0.106_155_143_212_812_32, 0.132_052_257_745_020_36, 0.173_683_250_613_087_04, 0.313_426_457_832_361_1],
    [-0.721_105_341_092_330_2, -0.324_414_949_266_018_2, -0.220_442_769_902_155_03, -0.145_141_803_352_328_76, -0.093_641_627_055_546_17, -0.062_428_833_093_532_53, -0.036_992_622_685_163_117, -0.009_940_231_298_058_268, 0.013_433_944_846_549_818, 0.036_354_003_465_537_396, 0.056_742_803_446_361_87, 0.074_805_721_400_738_52, 0.091_018_713_253_606_16, 0.107_060_949_463_934_93, 0.129_629_527_452_602_45, 0.162_678_388_553_671_3, 0.370_682_947_838_289_15],
    [-0.706_012_910_349_216_3, -0.318_074_328_258_830_1, -0.198_658_979_972_662_85, -0.137_278_251_712_044_7, -0.079_913_861_046_196_88, -0.046_618_935_471_091_64, -0.023_306_390_059_310_9, -0.004_197_099_932_742_749, 0.016_476_400_905_892_336, 0.035_931_608_599_924_67, 0.056_513_503_187_636_93, 0.077_050_847_123_259_71, 0.093_761_896_478_657_71, 0.109_097_964_116_501_06, 0.129_151_392_006_081_7, 0.157_806_742_303_856_63, 0.370_682_947_838_289_15],
    [-0.706_012_910_349_216_3, -0.381_161_697_278_285_14, -0.281_310_849_115_808_4, -0.207_284_500_182_435_44, -0.158_625_270_943_608_37, -0.081_995_815_016_818_1, -0.029_097_791_880_205_632, -0.007_115_624_896_834_949, 0.012_807_874_785_268_684, 0.032_710_799_417_412_825, 0.054_688_279_388_758_25, 0.080_769_833_837_903_93, 0.101_189_266_136_893_11, 0.120_214_286_278_605_92, 0.144_563_102_537_017_87, 0.198_896_473_415_503_04, 0.699_768_245_594_656_9],
    [-0.706_012_910_349_216_3, -0.375_378_764_082_516, -0.301_971_856_230_132, -0.256_047_626_602_265, -0.216_560_535_391_328_8, -0.177_623_915_921_467_1, -0.120_808_395_329_661_48, -0.079_411_699_545_213_92, -0.015_322_863_117_321_967, 0.017_206_650_485_741_878, 0.051_249_864_834_189_25, 0.094_008_146_564_701_8, 0.127_186_396_735_174_48, 0.176_297_213_413_980_39, 0.262_957_714_975_152_4, 0.393_935_676_515_770_05, 0.722_364_429_427_218_7],
    [-0.703_144_967_985_886, -0.324_555_670_292_870_14, -0.273_521_152_338_739_75, -0.247_492_489_964_185_2, -0.223_846_793_758_990_08, -0.189_767_387_541_969_67, -0.135_065_775_433_591_6, -0.100_108_610_466_892_72, -0.082_869_032_713_462_65, -0.035_014_789_747_158_576, 0.060_604_649_575_310_39, 0.161_730_527_210_072_86, 0.267_818_031_808_362_9, 0.372_139_556_053_809_83, 0.418_136_164_213_582_4, 0.477_901_589_153_604_26, 0.722_364_429_427_218_7],
    [-0.622_349_533_803_921_7, -0.312_938_615_952_372_3, -0.269_903_412_672_298_1, -0.248_622_870_949_883_58, -0.228_928_800_933_123_5, -0.192_596_820_609_660_66, -0.124_361_509_033_931_82, -0.094_194_551_961_552_98, -0.066_504_191_222_145_65, 0.081_849_349_568_518_14, 0.213_823_357_214_734_86, 0.306_773_076_059_564_33, 0.380_143_986_830_818_96, 0.411_105_811_552_939_46, 0.451_877_949_242_118_1, 0.503_133_694_508_404_7, 0.722_364_429_427_218_7],
    [-0.622_349_533_803_921_7, -0.431_316_402_179_820_6, -0.348_363_015_603_470_44, -0.281_111_236_726_902_8, -0.247_846_107_651_437_86, -0.201_412_510_315_490_9, -0.109_390_893_958_706_09, -0.021_996_730_335_982_873, 0.096_637_583_327_042_48, 0.154_853_970_892_235_1, 0.203_211_039_827_942_28, 0.246_760_013_066_438_22, 0.294_599_984_087_915_16, 0.348_054_634_239_680_47, 0.401_039_594_911_884_84, 0.461_008_644_885_583_2, 0.722_364_429_427_218_7],
    [-0.622_349_533_803_921_7, -0.404_109_840_128_787_8, -0.322_966_532_419_040_55, -0.258_165_587_728_483_47, -0.204_289_595_754_88, -0.156_144_305_343_821, -0.083_400_214_198_022_1, -0.038_335_248_850_402_69, 0.046_402_852_984_226_54, 0.118_105_771_828_963_74, 0.178_513_675_928_898_5, 0.219_933_615_086_955_94, 0.260_207_262_086_160_4, 0.301_393_455_281_368_85, 0.341_696_613_936_190_16, 0.396_511_597_864_456_06, 0.707_671_630_685_493],
    [-0.622_349_533_803_921_7, -0.271_015_147_492_854_27, -0.216_950_074_182_025_08, -0.181_329_495_076_993_67, -0.137_453_042_643_716_88, -0.074_931_078_243_979_74, -0.053_058_288_451_782_835, -0.032_506_068_275_037_6, -0.002_554_279_767_375_138, 0.043_274_509_415_410_37, 0.095_429_803_101_902_58, 0.174_594_805_858_173_66, 0.251_978_149_628_411_4, 0.305_522_140_946_943_35, 0.354_506_494_681_428_64, 0.412_627_681_161_924_05, 0.730_809_433_035_948_8],
    [-0.451_293_923_108_388_7, -0.203_661_673_899_558_03, -0.174_349_011_025_508_26, -0.138_502_566_168_912_93, -0.064_844_388_315_053_85, -0.047_181_472_434_209_99, -0.035_398_389_107_560_55, -0.022_678_849_373_213_815, -0.006_117_668_963_003_386, 0.019_215_480_600_925_906, 0.046_489_596_226_413_93, 0.075_139_069_355_814_98, 0.109_762_278_331_285_1, 0.192_930_603_206_964_29, 0.302_231_761_961_488_2, 0.383_684_531_396_483_84, 0.730_809_433_035_948_8],
    [-0.288_887_412_616_873_1, -0.181_295_782_385_221_26, -0.153_663_139_709_342_17, -0.082_786_786_964_132_33, -0.045_313_570_851_529_17, -0.035_726_872_246_197_326, -0.026_025_505_249_766_26, -0.016_328_697_978_119_026, -0.004_837_966_663_445_07, 0.012_310_668_072_231_192, 0.033_294_277_918_253_7, 0.054_343_941_950_039_394, 0.075_200_207_094_111_57, 0.101_262_492_887_465_46, 0.134_199_966_266_667_8, 0.282_444_252_067_798_16, 0.730_809_433_035_948_8],
    [-0.235_674_835_099_928_37, -0.166_053_925_855_540_44, -0.139_955_873_981_311_1, -0.048_680_456_126_973_214, -0.037_562_133_632_259_09, -0.029_645_207_038_532_385, -0.021_853_541_854_457_228, -0.012_484_580_057_691_335, -0.000_771_298_934_022_147_9, 0.011_703_644_510_519_232, 0.027_567_729_433_259_7, 0.045_150_733_872_881_45, 0.062_548_551_619_762_71, 0.082_842_717_241_527_61, 0.108_160_358_158_795_3, 0.139_410_024_358_665_79, 0.730_809_433_035_948_8],
    [-0.231_628_013_450_464_08, -0.150_241_468_541_815_86, -0.127_670_043_226_588_76, -0.041_097_878_813_834_546, -0.031_065_144_552_422_982, -0.024_276_256_449_716_024, -0.015_877_059_030_580_44, -0.005_158_309_656_136_922, 0.002_675_727_340_255_191_3, 0.013_128_589_554_612_963, 0.025_442_092_723_360_534, 0.044_221_785_970_808_176, 0.060_146_663_752_518_92, 0.075_444_453_718_401_79, 0.096_318_830_090_377_64, 0.137_792_413_761_125_08, 0.654_465_956_350_008_9],
    [-0.723_275_126_768_336_9, -0.331_869_240_240_68, -0.257_811_692_886_370_6, -0.210_234_746_580_135_16, -0.164_474_735_618_867_58, -0.106_046_039_828_374_28, -0.065_499_809_171_619_5, -0.031_704_247_601_867_35, -0.003_083_998_955_960_351_4, 0.030_105_698_527_668_42, 0.066_184_228_424_554_17, 0.102_934_004_193_623_56, 0.143_830_413_231_181_12, 0.220_704_154_028_089_6, 0.312_969_416_828_199_4, 0.401_595_834_562_379_57, 0.730_809_433_035_948_8],
    [-0.723_275_126_768_336_9, -0.295_210_672_240_035_2, -0.241_013_083_550_702_47, -0.180_812_665_635_840_67, -0.128_396_761_026_850_27, -0.084_760_282_216_270_58, -0.051_807_520_609_385_39, -0.024_747_537_576_084_434, -0.000_314_140_936_443_459_64, 0.028_413_920_835_377_76, 0.072_524_226_752_390_47, 0.128_712_165_961_483_35, 0.202_060_227_358_206_33, 0.267_031_006_464_608_8, 0.314_597_622_593_993_93, 0.356_410_191_731_163_9, 0.593_211_271_051_458_2],
    [-0.723_275_126_768_336_9, -0.294_562_651_579_969_6, -0.239_880_256_287_261_6, -0.180_531_739_418_333_8, -0.127_172_218_324_110_16, -0.084_124_963_999_178_4, -0.051_355_469_406_158_226, -0.024_187_831_203_972_048, -1.857_995_738_099_152_7e-5, 0.028_711_349_086_410_373, 0.073_231_535_514_361_15, 0.129_614_075_726_711_55, 0.202_447_204_985_024, 0.267_238_533_732_037_5, 0.314_469_854_029_566_1, 0.356_175_218_633_312_5, 0.393_317_186_637_460_5],
    [-0.176_195_467_356_281_1, -0.010_809_383_765_410_69, -0.004_667_007_549_560_270_5, -0.001_585_289_092_547_295_3, 0.000_761_090_692_576_452_4, 0.003_335_253_571_191_787, 0.005_632_301_141_113_421, 0.009_157_522_687_146_126, 0.013_361_907_385_493_326, 0.019_230_012_945_957_97, 0.023_930_159_693_975_654, 0.026_837_511_631_662_97, 0.030_489_933_333_151_968, 0.033_350_966_508_469_79, 0.036_681_301_546_070_05, 0.043_597_335_231_183_84, 1.240_285_347_201_41],
    [-0.176_195_467_356_281_1, -0.017_696_508_171_749_985, -0.010_534_100_200_735_991, -0.006_875_277_790_019_132, -0.003_976_020_187_384_316, -0.001_376_907_934_726_065, 0.001_013_080_958_082_938_5, 0.003_814_351_639_419_105, 0.006_626_557_869_674_634, 0.011_578_028_351_615_833, 0.016_723_832_350_831_032, 0.020_577_334_350_135_847, 0.024_089_799_979_283_79, 0.026_926_573_250_583_73, 0.030_769_320_626_575_242, 0.035_405_181_150_886_05, 1.313_632_446_895_950_2],
    [-0.176_195_467_356_281_1, -0.023_906_865_625_860_065, -0.015_398_861_779_925_575, -0.011_746_371_811_296_244, -0.009_093_174_656_777_928, -0.006_701_382_204_759_085, -0.004_104_786_961_482_299, -0.001_386_492_606_213_677, 0.001_544_735_212_278_321_6, 0.005_311_920_341_735_851, 0.010_527_404_281_927_496, 0.015_310_721_263_754_224, 0.018_335_921_616_994_95, 0.021_649_839_364_487_16, 0.024_872_339_901_854_047, 0.029_724_260_833_154_8, 1.313_632_446_895_950_2],
    [-0.176_195_467_356_281_1, -0.027_611_003_756_722_28, -0.020_950_187_275_962_8, -0.015_583_129_703_241_577, -0.013_132_810_869_598_041, -0.010_956_082_539_687_146, -0.008_704_400_275_772_264, -0.006_186_065_661_753_69, -0.003_334_361_761_691_281_3, 0.000_184_687_342_754_741_9, 0.005_377_165_994_520_337, 0.010_926_886_420_779_162, 0.014_831_272_807_307_751, 0.017_544_440_435_166_28, 0.020_711_968_339_799_228, 0.024_678_785_692_898_067, 1.328_614_456_903_953_5],
    [-0.135_815_040_381_333_96, -0.031_701_851_087_201_84, -0.025_948_246_957_180_97, -0.019_830_561_994_470_48, -0.016_566_614_940_607_39, -0.014_222_536_044_597_674, -0.011_948_327_575_517_611, -0.009_523_613_760_661_591, -0.006_304_151_499_747_927, -0.002_946_615_310_347_367_2, 0.001_887_749_983_075_2, 0.007_755_788_151_481_559, 0.011_734_268_748_995_12, 0.014_781_295_965_648_89, 0.017_825_637_767_236_335, 0.021_909_873_448_552_233, 1.484_495_518_323_049_3],
    [-0.135_815_040_381_333_96, -0.039_260_838_099_707_124, -0.032_131_105_901_487_336, -0.027_292_759_152_326_31, -0.021_773_649_762_026_61, -0.018_167_095_385_357_806, -0.015_657_961_910_148_86, -0.012_651_194_914_726_622, -0.009_131_492_560_834_583, -0.005_180_892_767_933_336, -0.000_829_569_452_028_044_2, 0.005_050_854_908_996_074, 0.009_225_771_144_886_76, 0.013_440_819_468_726_25, 0.017_623_107_339_862_512, 0.212_179_769_887_811_6, 1.484_495_518_323_049_3],
    [-0.524_644_187_535_330_7, -0.082_818_509_728_139_4, -0.052_505_746_913_959_27, -0.040_929_360_027_514, -0.034_020_527_665_595_976, -0.028_151_467_577_170_637, -0.022_496_981_950_010_795, -0.017_586_365_828_593_114, -0.011_954_168_778_878_272, -0.005_744_912_259_260_111, 0.001_439_568_934_943_752_6, 0.008_306_298_075_738_527, 0.018_824_593_575_195_423, 0.434_500_421_470_607_36, 0.653_674_019_387_102_3, 0.736_388_291_227_020_7, 1.484_495_518_323_049_3],
    [-0.542_381_786_983_196_6, -0.440_315_442_157_388_16, -0.260_446_290_708_375_7, -0.136_608_874_981_141_58, -0.065_889_446_218_824_84, -0.048_779_340_250_939_676, -0.034_465_793_410_716_06, -0.022_839_709_317_363_02, -0.006_020_894_400_039_393, 0.041_314_379_420_995_79, 0.153_719_645_028_213_64, 0.242_830_607_298_629_76, 0.369_706_019_778_815_84, 0.525_106_860_116_931_2, 0.641_104_940_870_762_6, 0.719_256_497_787_711_6, 1.484_495_518_323_049_3],
    [-0.542_381_786_983_196_6, -0.498_392_230_490_299_1, -0.472_418_592_784_719_87, -0.367_082_568_646_462_3, -0.190_154_794_819_427_93, -0.084_060_205_905_929_98, -0.043_798_217_440_272_87, 0.002_994_728_988_592_506, 0.051_837_125_907_766_26, 0.093_381_077_242_457_08, 0.130_452_723_974_827_16, 0.165_048_995_476_495_85, 0.202_127_608_692_093, 0.244_458_784_862_116_67, 0.357_159_457_881_595_75, 0.565_769_782_258_005_4, 1.132_899_069_726_629_7],
    [-0.542_381_786_983_196_6, -0.488_448_042_524_394_6, -0.443_560_028_201_281_2, -0.310_585_088_321_715_05, -0.135_860_371_768_675_74, -0.078_874_159_461_164_5, -0.029_515_340_172_175_226, 0.015_011_431_734_531_492, 0.049_447_111_873_137_49, 0.082_354_241_833_189_15, 0.108_624_579_128_848_01, 0.138_513_608_077_319_55, 0.164_403_465_805_963_3, 0.192_815_995_220_974_62, 0.222_393_187_875_224_6, 0.262_863_857_976_895_04, 1.316_777_803_019_629_5],
    [-0.542_381_786_983_196_6, -0.391_428_943_814_452_26, -0.283_187_133_632_305_24, -0.182_397_211_079_894_62, -0.120_769_979_023_464_59, -0.088_367_491_720_696_05, -0.056_140_768_856_251_74, -0.024_913_266_359_712_29, 0.006_318_236_516_177_522, 0.039_621_289_302_215_486, 0.076_444_613_698_050_65, 0.112_093_253_471_638_68, 0.146_449_339_664_701_42, 0.184_767_791_031_804_9, 0.220_385_030_099_831_35, 0.255_079_617_709_094_54, 1.403_258_735_864_911_8],
    [-0.534_128_685_676_260_2, -0.343_237_243_601_382_64, -0.237_343_604_001_243_02, -0.177_976_330_134_430_1, -0.128_072_443_701_457_76, -0.095_355_736_090_124_35, -0.067_286_193_435_035_92, -0.040_466_570_432_673_835, -0.014_597_406_366_914_138, 0.008_546_508_853_941_885, 0.033_629_239_218_948, 0.060_461_696_787_589_75, 0.097_217_140_487_180_04, 0.146_983_479_557_359_85, 0.201_472_174_221_714_4, 0.249_305_266_447_596_35, 1.441_719_066_405_594_3],
    [-0.587_177_887_858_856_4, -0.368_776_265_916_342_05, -0.267_160_418_799_522_5, -0.192_471_267_880_839_18, -0.134_806_544_025_810_77, -0.089_715_586_040_226_53, -0.056_140_653_806_640_78, -0.025_561_989_423_531_554, -0.001_331_862_811_634_840_3, 0.019_867_082_228_723_7, 0.038_417_544_244_214_806, 0.056_179_011_622_330_424, 0.072_409_160_531_401_91, 0.104_428_287_224_847_29, 0.169_783_831_086_274_46, 0.241_759_186_061_062_14, 1.471_090_248_083_119_2],
    [-1.238_175_065_286_178_7, -0.456_841_704_851_272_2, -0.380_706_548_175_769_04, -0.261_431_052_010_250_1, -0.169_222_021_783_556_08, -0.081_784_539_287_810_74, -0.028_778_540_263_414_06, 0.002_709_933_212_044_275, 0.024_280_555_232_699_917, 0.042_335_727_268_667_654, 0.056_498_037_605_681_77, 0.067_316_127_195_535_44, 0.082_081_897_702_564_5, 0.123_015_119_831_865_28, 0.224_247_797_926_414_36, 0.754_595_159_677_772_8, 1.471_090_248_083_119_2],
    [-1.335_824_633_464_970_8, -0.747_466_896_246_367_9, -0.534_011_726_006_495_7, -0.463_179_684_251_223_7, -0.384_744_879_877_700_2, -0.203_007_871_059_665_14, -0.018_097_959_522_631_25, 0.023_570_086_863_296_5, 0.048_816_863_494_201_904, 0.064_247_586_608_445_78, 0.082_081_853_154_631_17, 0.138_909_412_564_070_33, 0.382_620_930_484_838_25, 0.646_908_747_789_044_3, 0.746_785_905_981_930_8, 0.961_283_487_130_867_3, 1.471_090_248_083_119_2],
    [-1.344_170_630_435_599, -1.201_775_869_963_615_9, -0.918_949_503_443_575_7, -0.652_825_737_350_856_4, -0.508_099_064_797_642_1, -0.165_489_526_782_382_27, -0.024_294_661_832_660_852, 0.011_918_887_860_206_574, 0.043_537_965_291_403_385, 0.077_857_164_990_644_45, 0.138_652_857_617_801_47, 0.235_395_181_944_524_36, 0.350_807_737_261_320_25, 0.572_363_273_981_600_5, 0.699_845_957_698_023_4, 0.834_013_393_677_307_2, 1.471_090_248_083_119_2],
    [-1.344_170_630_435_599, -1.258_653_604_836_326_4, -0.838_847_818_465_883_2, -0.201_940_899_447_466_43, -0.082_210_602_749_073_71, -0.046_205_038_806_033_42, -0.019_972_143_637_379_966, -0.002_142_228_574_248_869_7, 0.015_074_116_664_394_913, 0.039_670_200_292_265_22, 0.060_032_198_371_957_245, 0.083_804_809_037_358_22, 0.131_385_779_597_777_28, 0.187_193_518_296_008_47, 0.336_197_855_183_086_76, 0.710_302_366_550_699_8, 1.298_604_427_579_679_5],
    [-0.151_221_590_827_323_73, -0.004_802_861_468_378_825, -0.000_597_700_151_094_113_1, 0.001_910_228_427_849_794_4, 0.004_132_726_342_264_695_5, 0.006_780_757_642_769_503, 0.009_083_081_902_690_256, 0.011_142_085_246_246_587, 0.013_391_808_940_493_579, 0.014_972_114_797_232_16, 0.016_910_723_192_949_968, 0.019_045_426_691_406_266, 0.020_876_058_519_483_483, 0.023_526_426_433_730_16, 0.026_005_826_116_310_585, 0.029_694_802_381_210_2, 0.070_935_570_239_542_67],
    [-0.151_221_590_827_323_73, -0.011_854_774_680_274_354, -0.007_027_764_124_070_203, -0.004_233_363_195_621_342_4, -0.002_038_873_450_721_680_5, -0.000_135_274_586_713_840_1, 0.001_624_470_623_758_804_6, 0.003_233_991_687_133_093, 0.005_085_560_187_538_707, 0.007_266_546_827_452_252, 0.009_515_859_773_760_219, 0.012_080_562_663_765_53, 0.014_422_473_062_445_566, 0.017_033_578_551_667_726, 0.020_431_067_423_247_365, 0.024_908_197_413_853_5, 0.047_515_077_712_892_095],
    [-0.146_300_703_458_729_6, -0.017_979_660_592_653_064, -0.013_030_468_689_740_7, -0.009_788_256_946_271_74, -0.007_669_300_802_544_172, -0.006_038_078_969_559_387, -0.004_496_061_451_366, -0.002_968_618_761_532_86, -0.001_492_709_148_665_186_5, 0.000_143_136_575_912_726_88, 0.001_689_305_502_821_778_2, 0.003_187_728_696_034_947_5, 0.005_454_590_929_308_142, 0.008_906_553_940_623_231, 0.013_085_916_379_477_739, 0.017_554_282_177_998_506, 0.041_661_418_166_651_39],
    [-0.138_269_381_713_024_36, -0.021_377_455_077_248_295, -0.017_933_202_724_661_63, -0.015_043_492_178_597_198, -0.012_286_890_557_968_843, -0.010_153_467_792_026_527, -0.008_547_522_294_008_096, -0.007_313_382_922_681_764_5, -0.006_144_552_982_525_129, -0.004_781_697_414_354_685, -0.003_480_148_333_203_814_5, -0.002_011_617_764_807_274, -0.000_190_047_727_891_052_98, 0.001_366_798_241_520_485_3, 0.002_887_646_668_138_762, 0.010_636_666_894_193_97, 0.026_467_926_033_913_486],
    [-0.138_269_381_713_024_36, -0.022_397_243_245_656_607, -0.020_586_097_758_076_894, -0.017_781_093_649_736_894, -0.015_582_415_434_054_057, -0.012_989_214_979_263_398, -0.011_070_057_474_290_228, -0.009_295_138_171_443_35, -0.007_902_045_412_411_636, -0.006_558_376_796_244_950_5, -0.005_387_747_326_735_659, -0.004_036_128_061_153_484, -0.002_065_914_095_408_739, 0.000_370_109_658_500_176_46, 0.001_957_788_301_780_325, 0.003_983_740_283_572_002, 0.018_699_482_735_494_544],
    [-0.062_488_222_484_274_59, -0.022_718_371_733_177_666, -0.021_240_413_564_415_63, -0.019_386_572_968_977_22, -0.017_162_328_835_211_894, -0.014_791_873_466_866_839, -0.012_201_080_075_627_17, -0.010_208_815_428_590_395, -0.008_320_590_353_522_408, -0.006_820_445_112_265_656, -0.005_543_358_019_189_684_5, -0.004_181_192_435_006_116_4, -0.002_228_248_357_724_561_3, 0.000_250_897_456_163_124_2, 0.002_004_487_192_165_921, 0.003_842_641_442_990_426, 0.020_897_244_132_269_06],
    [-0.064_132_214_484_714_34, -0.023_125_293_778_859_13, -0.021_411_507_440_468_686, -0.019_831_728_234_218_15, -0.017_755_423_250_005_422, -0.015_877_483_945_486_298, -0.012_998_879_761_153_95, -0.010_459_680_639_760_232, -0.008_664_291_910_734_524, -0.006_821_724_178_815_419, -0.005_530_966_230_879_41, -0.003_758_936_735_680_299_3, -0.002_333_217_891_961_231, -7.032_580_315_319_768e-5, 0.001_824_738_845_236_317_1, 0.006_075_901_584_089_603, 0.021_183_900_026_279_383],
    [-0.072_918_545_376_448_87, -0.022_893_781_097_064_556, -0.020_515_265_465_035_442, -0.018_883_520_955_727_828, -0.016_994_583_942_954_31, -0.015_441_478_391_381_306, -0.012_496_887_466_528_066, -0.009_968_937_671_997_406, -0.008_141_398_643_626_252, -0.006_113_735_106_221_608, -0.004_143_578_493_267_364, -0.002_840_076_930_051_064, -0.001_241_385_646_380_925_6, 0.000_631_850_137_583_519_5, 0.002_709_839_701_340_153, 0.007_288_964_173_963_523_5, 0.021_183_900_026_279_383],
    [-0.072_918_545_376_448_87, -0.021_265_081_661_470_25, -0.018_901_361_154_902_294, -0.017_044_102_291_624_31, -0.015_761_979_246_544_634, -0.013_646_060_517_336_061, -0.011_202_992_279_940_758, -0.008_854_410_998_656_977, -0.006_778_902_491_564_110_4, -0.004_137_615_926_812_005_5, -0.002_838_988_248_276_934, -0.001_419_263_932_179_976_5, 3.914_694_960_777_98e-5, 0.001_802_791_666_292_75, 0.004_102_931_811_437_098, 0.007_088_793_400_762_661, 0.021_183_900_026_279_383],
    [-0.264_735_452_819_129_64, -0.021_432_208_088_770_555, -0.018_830_443_618_687_027, -0.016_799_658_424_090_404, -0.015_678_119_043_265_534, -0.013_353_274_259_193_068, -0.011_210_301_234_514_204, -0.008_853_797_812_028_3, -0.006_037_147_685_846_88, -0.003_913_956_081_798_64, -0.002_801_408_491_474_386_4, -0.001_433_981_604_606_063_3, 0.000_127_116_282_037_857_65, 0.001_583_709_242_606_661_7, 0.004_021_716_040_348_369_4, 0.005_939_461_587_377_501, 0.022_170_989_421_189_197],
    [-0.264_735_452_819_129_64, -0.022_116_137_575_530_395, -0.019_419_561_244_747_943, -0.017_053_300_778_717_99, -0.015_918_485_976_465_55, -0.014_000_445_357_320_897, -0.011_631_166_525_497_023, -0.009_512_056_074_205_752, -0.006_106_849_287_803_221, -0.004_073_419_566_946_594, -0.002_814_588_390_009_548_8, -0.001_754_171_840_277_971, -6.475_067_692_354_21e-5, 0.001_202_886_940_583_092_1, 0.003_712_317_512_218_064_5, 0.005_719_000_747_141_048, 0.022_847_659_028_362_277],
    [-0.264_735_452_819_129_64, -0.021_368_691_266_659_97, -0.018_884_989_620_943_607, -0.017_324_222_170_281_28, -0.016_541_132_106_937_78, -0.014_783_421_093_338_592, -0.012_066_093_690_545_574, -0.009_353_206_324_397_076, -0.005_689_209_840_464_233, -0.004_055_144_221_745_187, -0.002_801_469_870_961_803_6, -0.001_669_731_726_263_952, -0.000_258_490_470_721_916_4, 0.002_000_649_750_350_163_3, 0.004_240_062_580_063_89, 0.006_821_693_095_500_21, 0.022_847_659_028_362_277],
    [-0.264_735_452_819_129_64, -0.022_689_496_479_535_14, -0.019_236_484_667_766_517, -0.018_134_281_093_041_132, -0.016_992_583_893_343_786, -0.016_000_007_972_091_036, -0.012_467_217_421_187_848, -0.010_448_782_847_958_086, -0.007_880_323_616_675_108, -0.005_534_721_338_448_253, -0.003_793_950_213_167_282_4, -0.002_417_304_911_250_226, -0.000_926_446_903_599_435, 0.001_934_065_380_103_696_8, 0.004_233_154_827_109_726_5, 0.014_669_508_240_569_338, 0.180_587_162_678_934],
    [-0.028_660_795_519_512_2, -0.024_885_434_005_747_48, -0.021_916_358_807_050_534, -0.019_737_915_301_429_706, -0.018_303_135_740_718_856, -0.016_321_421_021_402_657, -0.012_519_411_386_322_874, -0.011_925_989_747_772_306, -0.010_948_311_201_836_543, -0.009_441_611_952_957_557, -0.007_597_040_054_264_197, -0.004_536_141_633_127_428, -0.003_141_417_806_143_447_6, -0.000_842_213_290_565_715_7, 0.003_807_222_682_680_436, 0.015_052_534_458_810_536, 0.180_587_162_678_934],
    [-0.028_660_795_519_512_2, -0.024_991_462_890_368_504, -0.022_542_881_859_717_354, -0.020_153_994_467_685_687, -0.018_638_145_243_384_804, -0.015_561_836_003_671_664, -0.012_508_455_749_343_526, -0.012_055_468_252_070_349, -0.011_217_605_648_486_942, -0.009_451_811_966_074_395, -0.007_630_341_020_947_79, -0.004_544_739_008_669_856, -0.002_680_881_709_390_677, 0.001_736_692_904_138_487_4, 0.012_459_355_640_967_076, 0.017_271_576_752_379_83, 0.180_587_162_678_934],
    [-0.051_073_114_129_384_256, -0.024_916_899_297_502_18, -0.020_514_348_367_513_948, -0.018_651_523_795_278_513, -0.015_597_796_936_297_441, -0.014_754_747_432_027_279, -0.012_724_059_507_474_246, -0.011_961_424_137_871_532, -0.008_464_621_712_834_96, -0.005_302_903_321_179_006_4, -0.003_843_033_925_715_816_5, 7.164_812_720_720_182e-5, 0.001_144_333_218_963_742_3, 0.002_494_225_630_530_43, 0.007_799_332_132_549_387, 0.019_878_231_331_372_26, 0.395_947_944_721_579_57],
    [-0.466_213_505_517_060_37, -0.021_541_429_238_782_144, -0.017_686_208_580_568_547, -0.014_477_584_580_067_081, -0.010_979_245_413_443_979, -0.008_433_688_838_672_378, -0.006_423_456_794_777_639, -0.004_623_897_242_449_486, -0.002_849_243_421_175_967_4, -0.000_858_547_212_262_151_8, 0.000_979_667_759_142_775, 0.002_700_981_420_255_158, 0.005_596_100_208_640_479, 0.010_640_760_789_096_305, 0.015_532_300_278_292_938, 0.020_757_640_039_003_128, 0.395_947_944_721_579_57],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.179_638_195_845_890_1, -0.144_823_583_593_579_66, -0.110_561_541_451_386_5, -0.081_102_260_099_694_05, -0.064_781_116_191_663_45, -0.038_110_767_325_377_985, -0.024_491_406_863_615_933, 0.003_674_931_161_788_617_4, 0.034_419_186_660_318_526, 0.077_837_969_490_589_34, 0.113_635_821_462_313_9, 0.149_110_749_372_824_23, 0.199_890_416_904_580_34, 0.278_789_644_974_851_54, 0.337_298_499_461_007_1, 4.350_198_912_024_838_5],
    [-0.697_645_444_198_611_7, -0.234_036_302_028_132_83, -0.189_937_372_030_546_34, -0.154_746_844_159_207_59, -0.135_800_438_175_855_08, -0.101_591_731_985_079_45, -0.071_738_651_717_405_98, -0.038_023_755_233_999_98, -0.011_972_473_648_274_301, 0.039_512_428_135_265_01, 0.104_127_330_330_901_71, 0.146_699_104_041_267_4, 0.199_040_036_298_274_16, 0.259_011_911_984_734_5, 0.286_350_782_977_096_4, 0.339_711_718_826_921_64, 0.549_121_467_614_262_2],
    [-0.640_470_106_597_601_3, -0.188_806_079_606_912_48, -0.154_429_544_623_430_92, -0.135_594_323_432_661_92, -0.111_233_204_693_630_93, -0.090_381_730_007_632_29, -0.070_155_200_097_888_52, -0.044_075_764_363_517_29, -0.028_960_422_412_097_855, 0.004_301_136_471_859_416, 0.048_408_613_393_468_41, 0.104_464_382_570_607_15, 0.150_539_519_116_774_32, 0.199_580_661_816_781_47, 0.283_158_761_774_631_8, 0.339_268_356_874_964_3, 0.549_121_467_614_262_2],
    [-0.302_242_031_195_399_7, -0.163_918_680_346_099_9, -0.130_193_914_989_281_65, -0.109_705_478_147_358_59, -0.088_876_720_166_713_32, -0.070_095_273_569_264_98, -0.045_159_275_266_011_4, -0.033_294_325_067_911_11, -0.012_059_643_309_978_132, 0.028_991_546_170_752_905, 0.058_352_128_650_191_56, 0.104_747_021_478_348_23, 0.141_499_287_148_464_77, 0.180_448_642_619_404_88, 0.268_201_479_170_172_8, 0.327_369_529_311_247_3, 0.549_121_467_614_262_2],
    [-0.248_199_228_013_739_95, -0.143_842_421_318_614_88, -0.112_592_503_236_542_28, -0.082_452_519_316_159_89, -0.069_015_998_446_253_67, -0.044_003_905_715_482_75, -0.033_231_380_649_179_375, -0.015_769_881_917_658_96, 0.025_642_814_892_888_326, 0.042_238_884_318_498_94, 0.085_984_077_447_390_81, 0.112_176_695_097_360_13, 0.139_994_630_561_317_3, 0.176_811_210_702_973_65, 0.240_379_897_175_705_68, 0.314_765_199_864_343_6, 0.549_121_467_614_262_2],
    [-0.197_187_486_681_730_47, -0.134_147_666_927_104, -0.098_583_855_040_780_38, -0.070_337_107_872_226_65, -0.044_005_390_082_603_65, -0.036_854_424_554_486_6, -0.027_482_027_031_776_348, -0.000_900_911_612_207_63, 0.025_745_609_781_047_58, 0.038_302_587_725_489_57, 0.090_044_492_712_179_7, 0.109_006_222_652_941_96, 0.118_793_608_321_927_81, 0.142_641_357_274_297_86, 0.204_591_685_452_293_8, 0.297_245_230_614_011_65, 0.349_053_675_077_440_23],
    [-0.082_844_139_553_368_78, -0.020_430_489_533_559_634, -0.007_826_776_437_115_051, 0.004_828_966_412_504_707, 0.015_240_910_743_060_075, 0.024_078_225_376_066_048, 0.031_106_552_521_245_757, 0.037_411_327_719_368_51, 0.045_168_861_354_312_376, 0.063_749_129_706_924_34, 0.116_647_896_787_717_57, 0.152_761_833_574_970_72, 0.187_925_807_776_347_35, 0.240_262_798_122_743_6, 0.312_338_614_608_579_3, 0.373_958_419_642_206_55, 0.515_274_743_427_020_6],
    [-0.086_949_409_397_751_2, -0.020_361_414_125_465_03, -0.008_642_064_429_282_363, 0.000_747_846_525_956_013_8, 0.009_350_404_825_025_586, 0.017_021_058_676_319_96, 0.023_766_569_695_350_917, 0.029_620_313_836_628_92, 0.035_236_648_094_650_394, 0.041_636_749_905_074_34, 0.049_524_520_742_801_54, 0.106_560_882_864_518_51, 0.283_206_661_464_008_47, 0.374_782_480_280_494_93, 0.454_428_273_016_622_64, 0.538_041_884_961_695_8, 0.740_008_280_830_708_9],
    [-0.184_116_157_844_391_06, -0.078_452_065_067_456_15, -0.052_515_384_429_743_04, -0.035_047_167_809_667_885, -0.022_254_400_778_760_036, -0.011_736_634_287_293_567, -0.002_627_130_813_227_731_8, 0.005_819_436_931_358_843, 0.014_763_835_101_813_214, 0.024_054_076_602_218_588, 0.033_085_193_248_418_715, 0.044_793_240_266_047_94, 0.341_937_034_249_111_37, 0.506_997_583_617_831_6, 0.584_524_371_656_039_5, 0.669_801_978_636_068_6, 0.841_876_960_494_649_3],
    [-0.864_409_039_538_360_6, -0.136_859_967_881_258_98, -0.108_410_336_178_520_28, -0.087_582_321_183_254_96, -0.070_487_999_978_631_23, -0.053_889_901_431_268_29, -0.039_513_540_347_660_234, -0.025_902_555_431_401_44, -0.013_148_753_142_006_916, -0.000_624_974_171_236_646_3, 0.013_321_039_605_845_88, 0.034_836_984_527_554_57, 0.484_410_673_877_168_1, 0.606_714_258_231_456_4, 0.678_021_608_295_229_5, 0.740_447_969_730_081_2, 0.841_994_029_494_507_9],
    [-0.864_409_039_538_360_6, -0.248_405_140_815_996_32, -0.140_239_617_302_886_67, -0.118_181_466_464_240_96, -0.102_513_057_447_822_94, -0.085_687_129_504_975_84, -0.068_252_681_567_426_05, -0.048_554_967_647_009_36, -0.026_529_629_686_062_643, -0.000_106_700_830_555_005_77, 0.131_535_733_943_725_36, 0.248_156_050_294_239_04, 0.475_745_390_331_256_85, 0.628_854_139_699_089_1, 0.704_901_296_352_572_7, 0.761_935_588_984_185, 0.841_994_029_494_507_9],
    [-0.864_409_039_538_360_6, -0.274_345_799_438_496_1, -0.144_902_271_739_276_64, -0.117_854_101_224_365_67, -0.097_567_151_568_199_78, -0.073_021_752_476_715_3, -0.041_127_544_922_031_15, 0.017_412_328_052_897_424, 0.114_249_729_797_611_83, 0.148_903_294_024_186_95, 0.168_891_923_157_056_06, 0.192_336_132_273_091_18, 0.283_052_892_980_741, 0.413_830_570_737_368_5, 0.577_787_401_980_391_8, 0.736_333_720_426_539_7, 0.841_994_029_494_507_9],
    [-0.864_409_039_538_360_6, -0.175_103_662_401_168_26, -0.118_389_253_705_483_69, -0.086_159_355_341_232_09, -0.060_075_217_174_975_05, -0.032_583_255_727_864_545, -0.005_073_550_397_663_889, 0.022_853_147_175_487_785, 0.055_953_014_547_821_686, 0.088_793_277_019_954_87, 0.123_536_544_553_821_1, 0.145_085_315_110_930_26, 0.162_734_492_621_996_8, 0.178_096_440_748_625_82, 0.200_077_817_773_238_46, 0.334_221_353_647_302_2, 0.841_994_029_494_507_9],
    [-0.302_775_632_550_775_66, -0.158_747_539_243_543_32, -0.118_807_424_390_927_12, -0.091_934_868_859_605_86, -0.071_097_389_613_253_25, -0.051_703_972_085_740_35, -0.032_377_305_315_782_63, -0.013_978_050_091_352_572, 0.004_220_760_759_931_589, 0.022_405_188_566_369_93, 0.041_995_235_647_504_89, 0.059_956_890_430_873_21, 0.077_415_741_565_561_75, 0.099_279_222_968_675_58, 0.121_268_946_120_372_05, 0.158_683_175_195_888_43, 0.485_594_614_151_342_1],
    [-0.285_140_091_391_454_13, -0.135_491_081_342_389_52, -0.100_929_889_480_211_8, -0.076_956_245_145_195_03, -0.057_234_528_129_496_795, -0.040_288_830_165_729_72, -0.023_911_958_696_831_614, -0.009_444_311_902_974_38, 0.004_924_729_556_598_552, 0.019_225_296_441_383_783, 0.032_982_620_879_497_31, 0.049_130_370_370_901_21, 0.061_331_052_719_019_45, 0.074_194_336_470_309_96, 0.091_504_509_684_950_33, 0.110_062_335_817_169_62, 0.516_637_425_121_593_5],
    [-0.251_829_132_176_685, -0.104_423_148_546_632_9, -0.076_134_471_928_987_95, -0.057_786_737_147_272_59, -0.043_355_371_971_255_96, -0.030_067_532_321_264_762, -0.016_493_870_511_024_553, -0.004_995_515_011_276_122, 0.006_833_319_763_520_287, 0.018_222_081_191_955_383, 0.029_235_786_439_602_96, 0.043_579_351_291_799_79, 0.054_645_360_341_763_156, 0.066_683_295_776_800_57, 0.081_742_161_269_482_72, 0.101_964_094_089_274_01, 0.554_636_762_054_285],
    [-0.403_575_746_732_217_74, -0.085_046_268_810_524_38, -0.063_675_648_948_435_83, -0.049_246_036_228_398_52, -0.036_405_864_555_191_62, -0.024_998_675_181_182_716, -0.014_015_256_828_804_792, -0.005_177_920_592_327_961, 0.004_996_575_227_575_417, 0.015_164_070_436_233_66, 0.022_854_331_825_224_4, 0.035_061_680_862_990_43, 0.046_895_252_416_370_04, 0.062_086_477_582_499_3, 0.086_869_804_603_973_31, 0.465_900_063_274_671_6, 0.554_636_762_054_285],
    [-0.510_516_528_292_459_1, -0.104_512_374_525_894_58, -0.071_553_578_596_599_44, -0.050_947_807_715_340_5, -0.037_914_325_339_727_9, -0.025_004_790_168_197_82, -0.012_680_389_476_441_657, -0.001_496_190_832_267_089, 0.009_560_059_914_752_627, 0.020_012_722_039_883_454, 0.034_130_244_151_327_34, 0.047_798_386_977_986_83, 0.068_164_769_388_720_74, 0.214_088_774_440_692_74, 0.379_149_320_340_610_05, 0.504_811_082_505_980_6, 0.554_636_762_054_285],
    [-0.553_555_130_732_858_6, -0.125_928_273_861_182_24, -0.091_318_103_959_444_04, -0.069_458_701_395_137_67, -0.050_895_266_612_918_4, -0.033_824_767_842_105_516, -0.017_576_611_434_347_154, -0.001_385_360_572_388_394_7, 0.018_270_541_072_819_63, 0.030_918_516_303_541_206, 0.044_350_014_236_291_695, 0.055_116_329_526_835_74, 0.065_811_253_666_431_42, 0.078_594_662_494_117_09, 0.172_880_689_144_286_17, 0.316_672_411_327_648_23, 0.554_636_762_054_285],
    [-0.585_008_222_767_386_7, -0.123_430_549_235_264_04, -0.091_320_649_121_231_15, -0.071_807_226_179_300_8, -0.053_817_239_028_741_21, -0.037_713_075_587_599_77, -0.023_346_380_595_128_682, -0.009_077_141_811_743_336, 0.011_750_565_402_452_848, 0.027_419_191_479_285_14, 0.038_922_977_903_088_54, 0.048_044_197_005_626_094, 0.058_349_215_111_762_505, 0.067_513_354_123_267_38, 0.076_735_114_884_728_48, 0.091_029_173_883_773_24, 0.551_893_288_964_862_8],
    [-0.610_684_022_889_598_8, -0.502_747_004_589_376_3, -0.096_547_479_607_090_36, -0.069_799_847_479_641_59, -0.048_390_622_937_139_96, -0.033_898_151_698_964_99, -0.019_714_764_696_301_71, -0.008_734_878_052_506, 0.003_374_004_123_026_794_4, 0.026_354_475_321_730_887, 0.041_467_662_770_164_57, 0.051_380_258_536_034_32, 0.064_412_319_089_604_07, 0.072_884_556_189_714_77, 0.081_765_201_161_556_9, 0.099_886_890_486_287_27, 0.553_528_933_579_941_6],
    [-0.610_684_022_889_598_8, -0.545_261_268_162_740_9, -0.489_411_716_081_084_5, -0.102_160_821_915_914_16, -0.048_551_168_009_957_48, -0.037_243_937_934_572_28, -0.017_893_792_778_584_987, -0.006_036_865_704_316_103_5, 0.004_618_529_792_076_565, 0.033_898_881_019_354_33, 0.061_701_915_850_697_2, 0.072_761_375_915_075_12, 0.085_994_075_572_628_35, 0.103_211_556_603_206, 0.121_775_998_241_199_29, 0.304_637_607_634_591_8, 0.690_268_783_898_904_3],
    [-0.610_684_022_889_598_8, -0.492_691_237_643_444_64, -0.137_706_611_761_575_8, -0.086_472_696_174_329_03, -0.063_253_074_160_333_88, -0.044_377_018_715_478_48, -0.040_109_713_868_695_96, -0.031_161_033_042_444_28, -0.018_239_097_770_691_038, -0.012_721_874_063_953_737, 0.002_994_292_219_824_506, 0.045_398_839_004_343_91, 0.112_616_505_619_164_77, 0.214_763_372_842_413_5, 0.459_357_585_591_927_85, 0.573_717_082_914_264_5, 0.690_268_783_898_904_3],
    [-0.101_628_726_896_669_35, -0.017_331_771_672_381_785, -0.011_214_972_067_546_704, -0.006_030_305_790_202_95, -0.002_908_295_355_516_265_7, 0.000_460_262_679_591_873_8, 0.002_942_232_289_080_840_3, 0.004_466_401_999_327_165, 0.005_300_115_806_597_054, 0.005_962_650_409_777_112, 0.006_729_926_611_308_663, 0.007_325_224_328_440_065, 0.007_880_483_646_882_456, 0.008_613_097_361_920_851, 0.009_366_525_389_080_465, 0.010_331_279_441_503_093, 2.260_880_493_270_129],
    [-0.001_471_993_753_716_649_4, -0.000_254_555_981_532_262_8, 0.000_438_479_380_270_151_33, 0.001_302_467_448_593_929_7, 0.001_907_006_203_642_666_3, 0.002_429_400_284_861_324_6, 0.003_002_518_782_267_537_7, 0.004_366_883_860_609_194, 0.004_981_009_777_920_497, 0.005_259_984_901_026_053, 0.005_618_169_600_739_926, 0.006_192_134_554_239_238, 0.007_781_520_825_095_528_5, 0.008_614_719_161_265_747, 0.010_346_845_926_025_878, 0.012_313_671_252_169_709, 1.670_528_858_325_257_3],
    [-0.001_471_993_753_716_649_4, 0.001_717_835_356_193_573_8, 0.002_992_588_035_463_783_4, 0.004_434_772_273_939_257, 0.005_259_636_016_477_555, 0.005_664_586_300_805_479, 0.006_466_247_806_192_446, 0.007_260_003_173_599_591, 0.008_360_122_940_794_752, 0.009_112_872_592_045_998, 0.009_499_599_613_144_331, 0.009_797_885_504_352_106, 0.010_172_621_902_397_764, 0.010_470_982_030_121_935, 0.011_469_795_777_651_217, 0.012_716_424_775_693_805, 1.670_528_858_325_257_3],
    [-0.000_257_168_527_449_058_7, 0.002_199_413_239_914_972_8, 0.003_010_859_050_229_761, 0.004_368_919_127_882_25, 0.005_144_746_298_350_187, 0.005_977_732_981_860_332, 0.006_592_479_472_896_95, 0.007_390_860_819_611_903_5, 0.008_357_509_568_923_727, 0.009_075_060_446_754_7, 0.009_502_560_616_028_874, 0.009_801_792_900_263_475, 0.010_106_481_911_363_359, 0.010_412_582_186_749_83, 0.011_229_455_762_498_005, 0.012_812_976_954_664_261, 1.670_528_858_325_257_3],
    [0.000_318_189_543_730_529_3, 0.001_468_483_165_434_284_1, 0.002_245_574_207_265_327, 0.002_967_999_086_364_134_4, 0.004_018_243_149_757_549, 0.004_712_390_544_758_398_4, 0.005_120_535_738_039_648, 0.005_622_680_754_105_517, 0.006_136_342_997_065_835, 0.006_757_355_281_049_718, 0.007_399_062_600_783_418, 0.008_012_013_201_783_783, 0.008_525_610_003_042_987, 0.009_399_769_597_045_561, 0.010_007_088_199_198_814, 0.010_858_577_615_651_79, 1.670_528_858_325_257_3],
    [0.000_318_189_543_730_529_3, 0.001_966_578_154_738_481, 0.002_909_244_546_138_373_3, 0.004_179_572_019_635_396, 0.004_882_077_886_014_913, 0.005_264_256_061_570_223, 0.005_831_155_719_812_174, 0.006_187_219_211_402_556, 0.006_801_990_187_861_337_5, 0.007_384_260_642_275_98, 0.007_878_625_766_156_071, 0.008_424_934_331_773_779, 0.008_747_302_835_259_992, 0.009_488_363_398_809_076, 0.010_241_666_945_822_01, 0.011_158_148_830_064_052, 1.650_849_129_670_607_8],
    [0.000_318_189_543_730_529_3, 0.004_627_389_985_133_314, 0.005_371_212_921_863_313, 0.006_052_654_748_975_895, 0.006_244_896_989_022_939, 0.006_917_917_608_633_707, 0.007_562_296_946_557_209, 0.008_076_227_615_301_275, 0.008_454_269_757_780_734, 0.008_752_905_366_376_1, 0.009_268_309_359_287_462, 0.009_937_095_407_424_571, 0.010_459_748_679_858_924, 0.010_857_535_297_165_506, 0.011_406_298_371_138_782, 0.011_843_733_067_941_16, 1.825_107_193_698_466_4],
    [0.002_360_499_882_475_331, 0.004_990_634_329_429_499, 0.005_638_413_487_650_149_5, 0.006_090_851_319_817_885, 0.006_353_641_280_106_434, 0.006_902_972_760_220_488, 0.007_372_994_668_803_832, 0.007_989_553_633_006_58, 0.008_462_941_363_153_904, 0.008_754_239_696_580_802, 0.009_144_114_476_108_698, 0.009_509_182_722_355_965, 0.010_103_640_416_291_97, 0.010_580_363_032_367_411, 0.011_189_996_927_871_202, 0.011_701_880_057_731_739, 1.825_107_193_698_466_4],
    [0.003_656_006_662_206_668, 0.004_522_811_864_850_631, 0.005_343_310_918_691_852, 0.005_917_182_573_735_719_5, 0.006_237_039_434_100_383, 0.006_561_483_832_233_82, 0.006_904_616_226_428_503, 0.007_270_765_065_836_833, 0.007_773_486_420_388_636, 0.008_363_949_781_413_853, 0.008_725_641_335_527_617, 0.009_109_905_017_840_142, 0.009_259_160_668_110_54, 0.009_716_360_402_179_705, 0.010_133_898_451_142_749, 0.010_875_764_844_127_242, 1.825_107_193_698_466_4],
    [0.003_656_006_662_206_668, 0.004_805_874_082_523_347, 0.005_927_346_863_927_826, 0.006_349_417_517_754_935, 0.006_762_556_069_163_511, 0.007_149_106_717_814_258, 0.007_570_598_462_253_72, 0.007_969_017_889_593_278, 0.008_499_024_189_267_491, 0.008_966_655_518_712_83, 0.009_146_863_362_817_989, 0.009_368_538_244_937_73, 0.009_806_325_094_496_53, 0.010_203_214_022_565_266, 0.010_723_638_909_523_927, 0.011_653_470_914_352_754, 1.825_107_193_698_466_4],
    [0.003_656_006_662_206_668, 0.005_568_652_439_143_487, 0.006_312_441_785_975_431, 0.006_827_146_318_580_978_5, 0.007_265_498_234_175_515, 0.007_632_763_262_228_314, 0.007_997_687_500_724_032, 0.008_470_318_871_362_431, 0.008_890_547_257_233_544, 0.009_124_562_480_438_698, 0.009_465_941_748_908_465, 0.009_904_371_735_473_7, 0.010_198_990_476_630_455, 0.010_684_162_850_937_66, 0.011_272_292_168_228_164, 0.011_919_215_520_654_764, 2.260_880_493_270_129],
    [0.004_506_299_589_280_838, 0.005_252_457_581_881_243, 0.005_695_974_543_860_754_5, 0.006_309_098_662_013_689, 0.006_835_061_163_508_176_5, 0.007_215_361_261_557_012, 0.007_461_549_530_348_072, 0.007_769_058_875_355_891, 0.008_091_895_173_699_262, 0.008_529_632_046_918_006, 0.008_884_275_892_121_627, 0.009_158_571_386_820_274, 0.009_610_745_339_528_956, 0.009_969_271_946_190_35, 0.010_376_979_233_706_278, 0.011_215_643_894_070_683, 2.260_880_493_270_129],
    [0.001_534_263_229_183_263_6, 0.005_019_691_163_277_865, 0.005_251_405_111_932_623_5, 0.005_512_756_072_236_96, 0.005_904_300_754_916_494, 0.006_446_246_284_471_974, 0.006_858_807_076_537_14, 0.007_179_321_928_151_27, 0.007_388_948_391_910_934, 0.007_643_937_704_226_512, 0.007_883_349_189_628_132, 0.008_254_205_910_657_928, 0.008_630_749_509_286_616, 0.009_067_338_838_625_16, 0.009_527_861_779_259_659, 0.009_951_414_142_956_031, 2.260_880_493_270_129],
    [-0.009_239_620_332_955_223, -0.001_503_252_638_862_253, 0.001_543_386_959_354_292, 0.002_914_554_685_589_959_6, 0.003_925_203_242_526_707, 0.004_595_467_848_721_254_5, 0.005_140_456_911_123_574, 0.005_433_626_289_613_368, 0.005_813_860_662_389_028, 0.006_367_233_273_425_14, 0.006_867_572_407_852_329, 0.007_245_435_345_037_388, 0.007_561_747_095_879_454, 0.007_968_301_804_728_505, 0.008_492_618_480_638_349, 0.009_253_086_281_470_986, 2.260_880_493_270_129],
    [-0.026_742_314_612_417_495, -0.012_747_956_717_836_491, -0.008_074_215_525_033_034, -0.005_481_238_886_496_36, -0.003_819_668_128_897_382, -0.002_198_487_221_343_046, -0.000_739_039_986_206_007_5, 0.000_870_390_709_290_434_5, 0.002_159_495_838_527_991, 0.002_935_498_159_007_062, 0.003_752_569_802_599_395_7, 0.004_415_609_931_670_507, 0.005_127_618_658_894_347, 0.005_769_323_778_542_186, 0.006_745_685_847_721_52, 0.007_787_302_207_824_850_5, 0.011_217_733_800_531_26],
    [-0.054_464_363_771_923_54, -0.020_251_565_695_448_708, -0.017_169_192_720_844_335, -0.014_436_984_966_207_272, -0.012_248_737_168_342_587, -0.010_215_147_842_502_96, -0.008_305_675_893_172_427, -0.006_798_607_607_756_355, -0.005_426_268_702_368_967, -0.004_337_848_596_139_673, -0.003_059_530_763_586_235_4, -0.001_731_926_418_829_586_8, -0.000_232_246_425_437_409_48, 0.001_790_575_068_784_115_5, 0.003_199_199_659_934_746, 0.004_751_042_612_299_89, 0.011_065_505_260_271_28],
    [-0.089_379_028_121_710_84, -0.025_969_312_488_817_124, -0.023_478_595_291_551_09, -0.020_882_312_622_153_34, -0.019_086_482_584_658_827, -0.017_669_062_784_402_83, -0.016_259_097_925_295_39, -0.014_684_165_260_016_466, -0.013_087_022_876_595_529, -0.011_720_441_994_882_86, -0.009_729_403_770_498_55, -0.008_094_013_033_779_095, -0.006_409_059_800_979_733, -0.005_064_271_049_499_993, -0.003_638_911_177_287_255, -0.001_686_082_158_097_631_8, 0.005_583_449_650_307_326],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_894_72, -0.087_048_558_793_213_23, -0.074_368_617_811_434_61, -0.048_362_272_907_489_83, -0.038_521_905_458_097_756, -0.031_437_658_801_262_46, -0.013_998_251_413_628_525, -0.008_152_181_858_794_42, 0.002_973_560_866_906_219, 0.006_726_506_231_464_001, 0.014_964_465_688_434_668, 0.021_272_254_177_967_045, 0.034_500_263_022_034_86, 0.048_766_812_068_680_73, 0.060_020_925_288_596_06, 0.080_342_509_064_802_11, 0.111_004_919_151_147_13],
    [-0.124_586_499_325_894_72, -0.085_322_636_023_007_77, -0.064_880_345_857_038_38, -0.051_120_913_300_806_13, -0.042_761_145_392_181_14, -0.033_814_849_358_779_34, -0.017_090_634_498_119_796, -0.011_755_298_617_766_769, 0.002_538_892_909_834_371, 0.013_856_964_298_819_186, 0.019_385_549_481_935_473, 0.027_996_371_734_34, 0.036_010_412_078_698_505, 0.052_729_625_202_692_07, 0.059_414_352_520_648_91, 0.079_680_711_175_887_56, 0.111_004_919_151_147_13],
    [-0.124_586_499_325_894_72, -0.082_747_271_408_374_32, -0.065_024_139_836_817_25, -0.057_609_731_614_704_04, -0.047_948_132_888_920_16, -0.039_347_733_124_643_58, -0.020_401_668_100_781_16, -0.010_497_570_067_548_903, 0.002_155_179_264_224_335_2, 0.015_492_820_043_366_636, 0.022_606_114_063_730_774, 0.031_586_837_244_242_695, 0.040_088_897_168_333_4, 0.051_457_728_413_027_26, 0.057_599_071_755_410_07, 0.078_284_361_022_205_94, 0.117_947_229_490_634_96],
    [-0.123_707_386_673_730_6, -0.084_356_279_717_963_9, -0.067_892_877_906_179_61, -0.059_995_619_282_329_27, -0.046_495_007_747_686_29, -0.039_374_698_524_121_68, -0.014_301_542_489_828_28, -0.006_535_474_030_698_485, 0.001_972_059_205_327_287_7, 0.015_845_961_573_910_668, 0.028_694_933_172_102_133, 0.040_050_810_880_801_105, 0.049_174_074_518_170_31, 0.053_600_570_014_004_58, 0.064_267_405_557_694_25, 0.087_377_300_513_666_98, 0.117_947_229_490_634_96],
    [-0.123_707_386_673_730_6, -0.086_275_803_661_360_2, -0.076_650_650_399_717_57, -0.062_038_008_781_771_82, -0.047_119_624_562_036_93, -0.039_372_902_918_041_29, -0.019_984_288_763_463_48, -0.003_140_381_819_062_305_6, 0.001_693_479_948_961_853_2, 0.013_861_967_971_857_146, 0.027_244_256_790_873_82, 0.048_000_728_383_138_666, 0.052_294_386_149_546_12, 0.064_700_460_903_940_87, 0.080_657_446_063_396_96, 0.103_533_540_700_827_58, 0.117_947_229_490_634_96],
    [-0.117_000_137_386_871_43, -0.082_875_937_424_468_86, -0.069_967_772_971_314_51, -0.056_522_236_432_070_71, -0.045_169_195_866_172_525, -0.033_551_378_828_753_614, -0.024_186_336_415_724_58, -0.011_017_056_823_425_811, 0.001_506_609_640_686_998, 0.007_550_618_240_101_079, 0.016_782_896_836_014_24, 0.028_731_141_037_365_67, 0.049_730_253_486_821_23, 0.064_754_659_209_096_36, 0.090_596_287_718_942_08, 0.111_381_979_445_152_64, 0.125_442_659_204_805_68],
    [-0.101_427_562_605_890_92, -0.069_830_983_906_859_92, -0.051_170_536_800_909_84, -0.041_516_582_845_886_385, -0.035_286_861_033_579_53, -0.025_550_188_841_154_53, -0.018_696_999_461_681_66, -0.010_465_989_271_569_267, -0.001_863_752_795_762_753, 0.004_507_636_339_510_515, 0.009_248_515_765_905_206, 0.019_763_213_496_089_917, 0.029_459_597_641_530_08, 0.041_788_053_011_370_57, 0.067_212_039_256_518_95, 0.110_303_740_241_102_02, 0.125_442_659_204_805_68],
    [-0.082_305_206_996_401_54, -0.048_235_079_461_757_66, -0.039_541_473_405_432_966, -0.033_036_382_383_034_18, -0.028_478_510_250_237_014, -0.018_769_241_269_790_293, -0.011_866_595_460_261_439, -0.007_827_199_247_497_389, -0.001_547_016_222_730_274, 0.002_314_810_727_795_34, 0.006_449_937_011_173_073, 0.009_021_891_348_519_747, 0.016_607_334_250_977_827, 0.026_956_477_279_833_392, 0.035_485_785_430_559_39, 0.070_693_851_662_319_1, 0.125_442_659_204_805_68],
    [-0.051_623_519_897_587_955, -0.038_033_215_298_189_094, -0.032_247_195_636_244_654, -0.026_612_997_827_596_307, -0.016_888_902_074_373_494, -0.012_163_170_359_360_483, -0.008_430_708_566_231_303, -0.004_103_340_083_393_942, 0.000_121_974_275_689_751_09, 0.003_644_563_262_791_634_8, 0.006_640_519_532_524_019, 0.008_916_426_043_169_45, 0.014_223_459_945_265_55, 0.026_091_474_960_896_276, 0.029_751_053_568_852_25, 0.041_600_800_294_899_83, 0.125_442_659_204_805_68],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.124_586_499_325_894_72, -0.080_164_689_380_887_53, -0.060_497_840_028_597_51, -0.046_600_434_333_703_804, -0.037_279_054_989_131_144, -0.025_043_402_186_129_465, -0.012_893_345_477_724_513, -0.006_025_138_013_617_865, 0.001_695_618_951_147_883_2, 0.006_582_721_264_923_117, 0.014_354_715_808_001_392, 0.025_582_155_080_345_96, 0.035_559_769_651_648_21, 0.049_710_396_993_590_43, 0.060_035_979_866_168_716, 0.091_436_848_511_731_14, 0.562_299_322_024_696_8],
    [-0.184_847_486_617_969_74, -0.095_093_439_106_329_49, -0.077_100_098_330_708_19, -0.064_073_500_559_784_88, -0.051_792_135_334_162_6, -0.042_611_718_770_823_856, -0.034_205_904_403_008_414, -0.026_881_140_733_150_604, -0.019_297_686_246_914_626, -0.011_210_390_548_976_806, -0.004_062_105_173_941_371, 0.004_744_183_887_156_302, 0.013_460_579_835_994_508, 0.024_813_513_894_956_105, 0.037_461_245_232_919_18, 0.053_430_744_960_838_8, 0.109_448_761_996_901_4],
    [-0.264_698_199_743_428_45, -0.080_910_940_425_961_84, -0.061_715_542_592_038_71, -0.048_400_465_568_978_014, -0.038_372_155_887_930_616, -0.030_043_127_407_100_613, -0.022_886_519_263_571_42, -0.015_152_654_122_180_732, -0.007_957_038_127_634_966, -0.001_620_167_407_668_009, 0.007_201_374_118_238_369, 0.015_339_553_808_952_897, 0.024_410_610_415_546_402, 0.034_124_900_135_875_26, 0.046_016_325_437_325_21, 0.058_052_274_187_239_64, 0.109_448_761_996_901_4],
    [-0.414_108_675_674_678_7, -0.069_247_310_600_573_07, -0.054_878_348_101_409_57, -0.044_860_835_275_700_596, -0.036_333_485_719_492_11, -0.029_209_973_853_449_66, -0.022_984_206_248_796_754, -0.017_004_242_976_821_557, -0.010_974_857_748_060_96, -0.005_217_855_208_013_908, 0.001_126_436_025_408_975_7, 0.007_653_872_681_762_821, 0.015_009_363_427_065_206, 0.022_590_019_034_340_224, 0.032_345_276_659_040_47, 0.046_647_955_251_043_67, 0.109_448_761_996_901_4],
    [-0.462_761_441_031_369_1, -0.064_077_738_748_627_43, -0.050_201_234_948_722_494, -0.039_779_177_878_820_654, -0.030_700_439_606_682_19, -0.023_488_799_189_469_97, -0.016_687_090_046_413_913, -0.010_279_811_268_121_964, -0.004_268_015_231_243_034, 0.001_913_013_118_950_781_4, 0.008_191_906_917_427_641, 0.015_083_601_787_343_942, 0.021_509_877_326_736_208, 0.028_963_551_097_489_2, 0.038_051_218_215_250_926, 0.050_014_353_818_805_445, 0.183_021_584_985_223_2],
    [-0.462_761_441_031_369_1, -0.056_877_565_355_744_09, -0.037_042_433_183_490_54, -0.023_728_530_547_636_587, -0.013_819_756_558_735_278, -0.005_715_609_432_659_388, 0.002_301_094_422_323_847_4, 0.009_336_994_570_967_28, 0.015_881_799_460_315_402, 0.022_107_576_360_402_03, 0.027_625_492_922_477_62, 0.033_274_631_979_942_17, 0.039_692_243_237_577_716, 0.045_417_694_195_600_09, 0.052_026_349_626_793_35, 0.058_317_197_089_011_13, 0.183_021_584_985_223_2],
    [-0.462_761_441_031_369_1, -0.075_929_271_268_993_63, -0.020_390_297_106_049_556, -0.004_239_669_640_104_924_5, 0.007_470_394_357_809_681, 0.014_693_595_441_123_213, 0.020_980_724_284_589_16, 0.026_108_770_042_263_037, 0.030_695_098_241_936_978, 0.035_473_737_590_980_11, 0.039_723_159_499_774_67, 0.043_408_994_832_344_56, 0.047_753_222_483_435_59, 0.051_730_093_316_033_68, 0.056_014_714_162_338_97, 0.059_948_541_677_322_005, 0.183_021_584_985_223_2],
    [-0.462_761_441_031_369_1, -0.338_555_128_336_054_06, -0.299_770_135_658_955_2, -0.102_549_760_884_673_75, -0.020_151_755_689_976, 0.001_020_377_602_967_454_7, 0.012_365_216_926_045_981, 0.020_215_026_314_024_366, 0.026_399_228_926_286_1, 0.031_402_408_628_463_67, 0.036_269_244_831_381_12, 0.040_577_785_043_129_4, 0.044_360_461_075_585_826, 0.048_482_662_684_254_26, 0.052_614_528_104_175_38, 0.057_624_860_208_762_32, 0.183_021_584_985_223_2],
    [-0.445_735_629_807_484_7, -0.324_808_002_535_567_4, -0.293_107_903_010_754, -0.260_412_872_644_968_8, -0.197_314_992_026_009_63, -0.075_566_714_909_877_68, -0.010_666_478_152_481_215, 0.010_401_316_175_848_394, 0.021_772_341_961_223_407, 0.029_016_386_768_669_286, 0.035_472_245_619_056_146, 0.040_817_069_209_908_65, 0.045_970_680_335_830_99, 0.049_695_301_416_026_884, 0.054_469_837_122_953_16, 0.069_533_623_250_460_68, 0.259_427_767_498_806_9],
    [-0.427_155_736_425_441_7, -0.280_967_298_497_566_4, -0.242_444_529_197_152_1, -0.190_161_956_279_603_53, -0.131_607_548_246_379_46, -0.071_762_891_456_680_04, -0.005_611_858_734_821_158, 0.021_202_584_921_680_41, 0.033_661_407_103_997_815, 0.041_845_654_221_283_37, 0.048_656_051_452_084_584, 0.055_011_872_024_642_146, 0.068_610_790_755_102_11, 0.092_897_421_679_515_9, 0.122_215_482_193_560_95, 0.162_486_120_820_542_8, 0.325_608_094_377_949_35],
    [-0.410_921_372_868_238_2, -0.225_729_276_298_063_1, -0.139_689_399_151_563_9, -0.100_827_231_665_190_09, -0.077_319_015_955_067_18, -0.058_733_913_868_960_41, -0.040_330_604_879_945_296, -0.020_655_322_368_897_944, 0.009_320_026_601_950_079, 0.043_154_144_876_149_27, 0.067_482_813_422_533_37, 0.100_614_825_344_421_01, 0.131_302_774_615_195_38, 0.163_058_772_546_443_96, 0.197_848_446_467_303_5, 0.237_187_982_455_997_95, 0.338_488_021_998_251_56],
    [-0.410_921_372_868_238_2, -0.145_842_297_797_596_55, -0.100_702_862_214_577_51, -0.078_748_230_646_985_75, -0.063_403_987_395_799_03, -0.050_255_165_917_629_4, -0.039_565_472_877_930_58, -0.028_671_960_863_997_432, -0.019_571_570_261_369_353, -0.007_856_013_552_594_573, 0.013_140_497_574_809_123, 0.058_349_341_846_257_99, 0.121_975_302_873_786_38, 0.193_426_457_466_236_05, 0.233_402_262_162_513_67, 0.253_107_609_664_018_85, 0.338_488_021_998_251_56],
    [-0.360_737_004_062_220_4, -0.121_465_629_338_147_43, -0.087_759_406_485_661_18, -0.065_460_798_015_287_47, -0.050_098_481_319_290_26, -0.039_251_479_014_694_717, -0.029_908_266_481_063_04, -0.022_457_222_502_474_93, -0.014_951_078_555_367_148, -0.005_989_447_158_556_688, 0.009_744_016_629_047_917, 0.036_287_738_512_655_404, 0.073_590_876_201_670_07, 0.153_573_327_319_009_88, 0.234_411_752_306_174_7, 0.253_298_219_489_455_4, 0.338_488_021_998_251_56],
    [-0.337_458_074_510_097_85, -0.104_155_086_722_313_83, -0.071_497_528_321_519_52, -0.050_256_950_427_175_68, -0.037_511_903_585_830_085, -0.027_982_972_196_153_39, -0.020_401_739_394_407_127, -0.012_644_783_735_148_346, -0.003_629_615_796_474_158_4, 0.012_292_241_372_490_671, 0.031_394_474_298_076_75, 0.050_630_527_169_699_47, 0.076_533_799_227_937_17, 0.118_412_793_787_877_15, 0.217_707_000_649_423_25, 0.245_227_267_075_873_33, 0.338_488_021_998_251_56],
    [-0.316_884_353_496_951_8, -0.099_560_956_744_846_22, -0.067_400_336_851_184_49, -0.045_930_268_222_508_85, -0.031_065_160_894_988_585, -0.020_720_125_966_597_308, -0.009_832_320_382_326_32, 0.004_125_438_042_963_163, 0.020_868_581_746_013_18, 0.034_803_483_663_841_16, 0.049_289_114_435_827_69, 0.064_075_381_248_133_4, 0.085_245_106_794_111_14, 0.127_646_577_262_883_5, 0.196_739_230_404_946_95, 0.223_158_203_050_208_36, 0.330_638_018_912_407_7],
    [-0.316_884_353_496_951_8, -0.113_211_478_563_130_91, -0.086_861_380_633_436_94, -0.056_810_664_836_420_88, -0.033_880_185_548_519_88, -0.016_079_662_250_756_33, 0.001_947_116_855_420_456, 0.019_343_850_206_448_79, 0.031_525_175_601_471_03, 0.043_192_295_060_305_79, 0.055_914_998_247_662_81, 0.067_754_720_515_628_18, 0.091_375_879_962_502_83, 0.153_469_397_542_359_66, 0.179_438_425_238_795_22, 0.203_143_023_607_081_45, 0.286_496_225_486_833_07],
    [-0.245_486_105_315_374_35, -0.136_523_932_585_440_5, -0.117_865_352_885_008_27, -0.096_212_925_204_222_87, -0.068_280_601_897_234_55, -0.030_707_852_038_926_235, -0.001_272_244_669_923_450_5, 0.016_286_763_702_433_548, 0.029_629_764_853_115_633, 0.040_477_078_316_550_09, 0.051_343_221_630_544_764, 0.063_629_111_450_395_46, 0.082_394_560_451_201_42, 0.147_402_264_527_272_95, 0.166_829_113_781_832_07, 0.181_124_268_636_262_77, 0.251_976_947_233_273_24],
    [-0.245_486_105_315_374_35, -0.143_069_165_205_577_5, -0.134_441_451_961_198_47, -0.124_381_563_544_506_5, -0.114_504_558_813_380_63, -0.097_125_962_880_218_92, -0.070_757_587_806_030_99, -0.037_807_461_447_053_714, 0.005_518_752_557_903_339, 0.023_818_393_432_899_71, 0.038_883_930_638_734_52, 0.051_086_440_316_145_836, 0.064_256_748_374_930_37, 0.081_241_866_840_673_04, 0.147_339_004_707_177_92, 0.171_323_563_916_963_5, 0.208_840_990_422_074_5],
    [-0.639_873_944_478_748_8, -0.154_589_317_114_375_85, -0.120_141_294_568_554_64, -0.085_952_840_563_770_53, -0.061_428_737_280_563_9, -0.039_560_787_270_739_216, -0.018_584_723_275_772_35, 0.001_573_037_969_569_827_2, 0.017_182_318_052_806_105, 0.028_135_311_254_272_793, 0.038_433_792_869_370_72, 0.047_714_629_153_308_15, 0.058_219_704_044_121_65, 0.070_603_050_799_787_71, 0.087_089_918_996_941_96, 0.112_715_523_492_607_69, 0.512_897_840_388_249_3],
    [-0.863_503_096_075_225_2, -0.154_699_874_169_600_32, -0.118_820_005_537_884_5, -0.083_476_670_148_193_32, -0.055_918_074_152_157, -0.032_744_776_260_001_14, -0.009_600_273_114_788_409, 0.012_871_522_001_686_515, 0.030_237_476_177_493_6, 0.043_656_918_970_829_83, 0.055_974_764_027_382_815, 0.070_188_543_446_282_07, 0.087_218_747_594_474_37, 0.113_267_701_249_162, 0.163_616_738_868_993_83, 0.287_289_969_665_200_26, 0.720_427_638_392_060_3],
    [-0.863_503_096_075_225_2, -0.570_526_872_584_597_4, -0.330_201_626_762_115_6, -0.154_370_634_552_086_16, -0.092_933_228_982_453_58, -0.051_612_986_803_565_505, -0.018_626_066_665_966_86, 0.014_681_375_282_002_57, 0.046_496_521_510_127_92, 0.074_377_048_877_855_86, 0.108_556_212_880_994_29, 0.158_802_218_610_720_83, 0.219_622_310_950_045_2, 0.324_991_376_359_528_85, 0.491_253_180_961_484_1, 0.592_602_880_427_810_3, 0.792_587_271_650_644_2],
    [-0.863_503_096_075_225_2, -0.611_826_182_190_608_8, -0.493_775_329_212_269_13, -0.349_529_248_224_721_1, -0.228_248_814_322_857_2, -0.132_209_181_186_974, -0.068_940_976_364_840_73, -0.029_505_810_611_170_316, 0.009_551_823_720_056_371, 0.052_440_539_024_526_72, 0.098_814_133_752_001_53, 0.160_901_042_526_970_54, 0.239_277_583_879_898_28, 0.407_398_268_181_415_83, 0.572_581_485_655_63, 0.644_742_238_602_267_6, 0.827_347_615_176_185_7],
    [-0.863_503_096_075_225_2, -0.592_700_148_463_977_9, -0.494_305_660_145_667_6, -0.382_152_935_317_027_76, -0.293_859_955_166_138_7, -0.207_259_448_142_453_26, -0.128_911_132_788_878_8, -0.069_139_127_043_041_16, -0.025_901_473_450_721_073, 0.018_078_226_224_579_427, 0.063_268_006_570_416_69, 0.125_666_471_339_354_6, 0.204_812_076_767_884_63, 0.344_863_722_846_249_1, 0.595_298_826_286_170_1, 0.677_771_532_863_099_4, 0.875_770_113_818_131_9],
    [-0.745_050_213_043_267_9, -0.550_556_154_489_880_3, -0.462_603_241_258_452_2, -0.373_083_160_765_915_4, -0.302_031_731_000_465_76, -0.234_888_685_604_372_82, -0.163_604_435_281_888_56, -0.099_528_616_723_746_48, -0.046_570_232_303_528_11, 0.001_964_288_649_520_046, 0.050_354_662_279_908_59, 0.109_913_990_641_394_18, 0.180_775_523_450_896_23, 0.298_242_647_157_461_57, 0.609_119_545_381_929_9, 0.709_680_694_522_781_5, 0.875_770_113_818_131_9],
    [-0.655_994_690_562_037_3, -0.506_877_534_921_693_2, -0.420_004_359_904_286_6, -0.349_688_210_835_119_53, -0.282_449_709_216_133_05, -0.223_626_387_751_448_44, -0.150_164_218_919_441_65, -0.092_020_254_328_522_82, -0.042_636_257_471_729_38, 0.013_398_382_150_306_47, 0.059_877_962_370_813_63, 0.112_988_183_430_685_99, 0.166_946_136_296_321_68, 0.246_578_637_261_958_58, 0.408_168_666_211_419, 0.738_223_990_114_45, 0.901_592_941_214_191_6],
    [-0.613_966_315_246_134_9, -0.520_004_125_992_130_2, -0.437_815_755_322_738_64, -0.363_413_616_799_721_45, -0.276_706_166_021_398_2, -0.223_655_105_797_285_04, -0.167_901_702_455_349_1, -0.110_274_811_965_089_45, -0.062_929_572_447_147_46, -0.009_768_977_399_832_759, 0.048_747_752_517_087_78, 0.112_464_795_668_186_98, 0.192_190_039_028_252_8, 0.308_906_087_868_584_1, 0.635_807_944_264_652, 0.707_969_869_071_044_8, 0.901_592_941_214_191_6],
    [-0.902_863_230_596_388, -0.560_824_198_502_327, -0.480_334_128_250_135_5, -0.415_311_169_495_445_1, -0.338_397_547_879_529_97, -0.261_321_042_996_425_2, -0.198_213_339_350_429_26, -0.131_999_020_497_197_5, -0.074_917_086_747_488_52, -0.007_222_289_334_106_671, 0.087_601_320_537_097_42, 0.217_185_249_233_902_62, 0.376_895_383_248_059_3, 0.553_161_329_638_809_6, 0.631_153_748_956_625_3, 0.682_342_422_112_681_8, 0.901_592_941_214_191_6],
    [-0.920_755_937_186_128_8, -0.710_141_287_656_473_2, -0.592_827_248_071_066_3, -0.519_025_598_624_564, -0.440_836_048_115_675_06, -0.363_505_411_390_675_6, -0.273_692_890_653_433_2, -0.176_558_253_359_169_86, -0.079_125_226_736_731_85, 0.027_781_187_622_181_916, 0.101_536_454_329_928_72, 0.162_012_991_766_589_8, 0.262_843_752_008_780_5, 0.390_625_179_484_745_1, 0.541_098_402_677_379_4, 0.636_037_821_959_137_2, 0.901_592_941_214_191_6],
    [-0.920_755_937_186_128_8, -0.803_255_860_445_082_5, -0.708_005_291_248_883_7, -0.631_424_054_577_770_3, -0.546_399_270_325_682_3, -0.450_380_333_119_142_44, -0.333_915_617_262_261_7, -0.173_275_356_019_927_83, -0.013_365_123_856_532_449, 0.037_392_882_993_615_456, 0.069_300_393_007_057_77, 0.096_039_290_759_338_84, 0.122_558_649_499_093_76, 0.156_324_420_595_892, 0.238_636_460_969_728_1, 0.418_872_088_295_703_7, 0.783_785_013_742_440_3],
    [-0.920_755_937_186_128_8, -0.803_187_293_108_351_4, -0.717_261_711_791_685_6, -0.646_035_921_780_853_6, -0.541_158_226_839_315_1, -0.410_520_758_595_078_2, -0.165_935_011_323_558_76, -0.025_216_302_238_137_228, 0.009_327_248_293_633_189, 0.033_122_752_268_002_45, 0.052_297_710_923_795_57, 0.069_472_494_514_155_12, 0.086_547_135_129_074_42, 0.104_133_874_755_837_57, 0.125_654_751_937_075_84, 0.159_507_030_155_000_8, 0.698_566_275_755_307_7],
    [-0.920_755_937_186_128_8, -0.760_075_925_786_679_5, -0.686_834_335_501_453_8, -0.566_650_942_256_273_5, -0.371_771_725_212_445_34, -0.125_142_661_005_905_4, -0.037_980_432_748_557_24, -0.007_904_262_662_389_56, 0.013_271_715_697_729_627, 0.030_860_932_602_102_317, 0.045_037_183_188_357_366, 0.059_971_287_956_399_42, 0.073_396_100_440_661_83, 0.087_673_006_612_504_7, 0.103_934_063_601_676_8, 0.125_666_332_856_403_1, 0.247_162_265_107_227_97],
    [-0.866_511_187_328_192_4, -0.720_840_882_632_297_2, -0.608_847_872_213_693_9, -0.378_115_919_041_590_9, -0.161_338_409_750_569_6, -0.063_137_290_457_955_69, -0.026_532_141_641_274_036, -0.004_776_473_870_777_587, 0.012_812_255_816_288_537, 0.029_522_160_969_291_92, 0.042_056_831_795_669_31, 0.055_591_980_913_582_26, 0.068_816_862_018_398_55, 0.082_669_468_880_151_85, 0.096_760_679_028_769_92, 0.118_458_061_739_123_97, 0.258_909_643_242_67],
    [-0.745_987_624_638_186_9, -0.211_155_083_871_896_71, -0.144_633_958_690_871_9, -0.116_618_494_185_182_58, -0.097_900_766_015_019_4, -0.074_714_948_760_325_74, -0.054_716_695_807_213_71, -0.043_680_753_184_243_36, -0.029_470_047_267_470_77, -0.008_443_969_957_197_543, 0.005_228_879_935_282_407, 0.025_295_152_023_436_993, 0.048_848_192_170_543_94, 0.073_267_179_496_986_29, 0.103_469_954_228_200_5, 0.137_125_365_131_055_5, 0.258_909_643_242_67],
    [-0.572_873_517_910_272_4, -0.134_572_092_701_878_42, -0.111_499_510_541_452_12, -0.092_543_898_102_599_87, -0.069_250_152_884_494_06, -0.051_796_877_010_848_875, -0.038_313_482_733_849_22, -0.024_252_638_304_888_562, -0.006_879_977_266_479_112, 0.005_404_075_154_171_488, 0.019_905_671_816_623_467, 0.037_074_719_459_843_7, 0.055_182_386_279_056_57, 0.073_435_116_126_666_66, 0.099_189_755_790_037_75, 0.126_939_676_579_348_38, 0.265_559_831_599_769_15],
    [-0.272_719_035_897_590_17, -0.117_119_139_902_554_1, -0.099_416_172_582_457_97, -0.073_870_207_974_845_59, -0.054_481_402_246_089_96, -0.036_880_171_140_062_55, -0.024_279_214_909_932_102, -0.008_775_048_150_580_698, 0.003_202_381_714_283_961_6, 0.016_575_656_412_409_41, 0.028_070_352_675_833_99, 0.043_421_475_329_767_19, 0.058_500_361_474_067_786, 0.074_136_058_787_557_4, 0.097_936_742_410_875_5, 0.125_045_017_123_806_78, 0.471_819_048_506_015_2],
    [-0.404_703_352_366_068_36, -0.132_905_535_125_641_68, -0.090_078_189_807_561_56, -0.059_842_427_384_790_17, -0.039_864_640_092_215_374, -0.025_041_275_654_552_813, -0.012_225_367_049_386_137, -0.000_666_998_192_228_447_2, 0.012_428_300_057_894_67, 0.021_793_139_331_047_748, 0.031_511_621_125_322_894, 0.040_507_047_942_397_616, 0.053_611_715_751_817_75, 0.070_469_027_672_614_47, 0.093_788_202_865_308_81, 0.112_267_494_596_076_78, 0.157_558_967_731_404_2],
    [-0.489_422_419_217_909_5, -0.188_483_412_502_280_1, -0.132_534_271_170_495_04, -0.097_055_851_137_732_78, -0.071_374_241_681_833_84, -0.048_593_853_679_571_516, -0.031_307_607_203_604_246, -0.013_800_892_615_171_015, 0.000_548_048_636_878_248_7, 0.014_837_972_007_569_903, 0.027_029_206_875_730_787, 0.040_123_500_456_958_04, 0.052_710_314_224_389_42, 0.066_390_532_745_529_53, 0.082_501_236_123_479_63, 0.113_868_441_554_093_9, 0.173_765_176_598_886_13],
    [-0.678_868_500_103_437_4, -0.247_931_546_902_322_33, -0.171_084_450_246_602_82, -0.127_019_481_968_186_85, -0.094_073_686_466_899_05, -0.069_052_633_940_055_98, -0.045_434_126_259_788_15, -0.023_361_326_365_382_37, -0.003_051_275_528_910_847, 0.014_471_824_414_229_817, 0.031_558_010_110_424_77, 0.046_612_908_276_713_265, 0.060_644_369_720_559_29, 0.074_783_181_052_542_52, 0.086_804_037_029_540_07, 0.123_277_557_653_872_4, 0.184_669_611_853_623_3],
    [-0.717_676_194_151_064_1, -0.311_709_420_061_367_6, -0.211_261_969_481_300_5, -0.154_264_134_690_800_08, -0.112_353_040_202_622_16, -0.079_990_064_287_287_01, -0.052_627_330_214_967_49, -0.027_784_943_218_567_892, -0.001_074_221_762_231_042, 0.020_433_377_965_155_62, 0.039_050_197_889_105_735, 0.055_799_894_671_970_725, 0.071_420_094_150_175_36, 0.083_501_198_774_732_85, 0.099_036_858_874_567_67, 0.137_127_625_380_382_67, 0.195_942_204_947_863_63],
    [-0.816_302_860_767_734_7, -0.348_687_754_125_841_2, -0.250_851_836_394_927_5, -0.177_325_839_287_968_97, -0.123_890_933_762_540_89, -0.085_618_137_146_337_88, -0.052_813_465_443_566_23, -0.027_271_511_352_429_764, 0.002_943_110_738_042_636_6, 0.025_498_124_204_156_637, 0.044_432_332_998_040_974, 0.062_883_508_581_595_75, 0.079_653_535_277_178_3, 0.093_065_361_433_383_48, 0.107_888_762_243_693_62, 0.146_050_166_541_633_6, 0.210_811_899_320_320_96],
    [-0.940_570_157_999_248_5, -0.388_683_970_914_339_2, -0.276_366_105_531_732_46, -0.193_328_280_898_166_1, -0.138_104_730_306_495_45, -0.094_562_984_661_836_6, -0.056_322_341_846_254_09, -0.027_852_159_640_982_76, 0.001_562_100_033_004_385_8, 0.026_864_727_015_456_412, 0.047_400_450_047_202_4, 0.067_145_075_312_144_73, 0.084_723_941_848_543_66, 0.099_274_747_019_228_78, 0.113_950_818_654_069_63, 0.145_778_416_701_281_9, 0.221_708_801_441_090_84],
    [-1.062_695_525_593_916_6, -0.436_189_320_917_876_1, -0.304_857_163_412_571, -0.213_591_934_259_560_97, -0.151_991_624_993_098_84, -0.105_179_452_081_477_3, -0.064_689_018_263_228_05, -0.029_678_988_052_244_19, 0.000_463_985_096_627_406_7, 0.028_554_595_752_080_882, 0.051_989_745_902_130_916, 0.072_509_349_386_427_24, 0.091_403_018_611_997_48, 0.107_700_703_798_642_08, 0.123_421_608_140_225_44, 0.149_502_044_145_193_2, 0.234_012_505_187_457_98],
    [-1.113_557_457_621_278, -0.475_605_022_454_032_9, -0.333_745_176_632_303, -0.241_861_324_334_544_85, -0.168_967_863_332_009_42, -0.117_582_586_037_865_03, -0.070_871_356_668_751_5, -0.033_352_322_645_029_686, 0.000_139_198_548_891_496_48, 0.031_469_560_470_827_21, 0.056_519_165_989_616_78, 0.078_398_369_124_865_39, 0.098_591_255_259_091_04, 0.117_389_819_466_087, 0.133_310_314_139_400_03, 0.154_858_582_228_360_88, 0.242_562_942_058_587_08],
    [-1.199_402_202_572_441_6, -0.497_668_413_187_188_5, -0.353_917_129_579_167_6, -0.263_688_543_189_081_6, -0.187_439_313_674_824_41, -0.127_380_386_633_837_72, -0.077_717_003_078_233_45, -0.037_679_286_656_539_57, -0.002_545_083_281_175_562, 0.031_080_563_640_321_288, 0.057_877_992_321_212_88, 0.081_669_688_435_998_26, 0.104_237_968_968_081_86, 0.122_750_108_014_317_4, 0.139_273_963_736_015_03, 0.157_729_563_058_550_54, 0.244_100_836_069_868_88],
    [-1.307_775_554_757_475, -0.506_200_576_391_569_8, -0.357_004_624_993_065_84, -0.266_144_587_663_942_44, -0.192_797_824_978_426_98, -0.130_624_119_088_388_17, -0.080_386_144_185_161_71, -0.039_500_158_778_386_606, -0.003_440_928_861_682_513_8, 0.030_539_709_595_384_476, 0.059_388_305_504_028_75, 0.084_866_182_374_696_41, 0.108_367_886_596_336_13, 0.126_719_965_419_787_78, 0.143_464_712_640_184_34, 0.160_718_926_707_347_55, 0.244_100_836_069_868_88],
    [-1.436_661_104_535_867, -0.510_060_194_430_927_9, -0.350_559_368_199_875_93, -0.259_703_516_174_598, -0.185_554_344_830_277_3, -0.126_208_117_509_146_23, -0.077_903_198_390_032_49, -0.037_323_875_591_195_155, -0.002_285_246_854_617_951, 0.030_060_330_247_600_176, 0.060_195_349_194_404_75, 0.085_391_512_430_870_04, 0.108_781_247_106_906_36, 0.127_926_033_166_529_15, 0.145_164_612_375_572_02, 0.162_283_583_867_093_05, 0.244_100_836_069_868_88],
    [-1.519_175_973_010_455, -0.493_605_741_347_818_37, -0.340_558_878_398_500_85, -0.246_854_471_456_805_3, -0.173_928_350_387_560_3, -0.119_773_648_930_717_15, -0.075_484_388_461_525_3, -0.035_264_284_759_675_22, -0.000_847_558_353_495_230_3, 0.029_829_127_247_662_91, 0.056_546_545_404_976_924, 0.081_436_180_259_956_91, 0.104_369_164_348_910_22, 0.123_643_835_907_596_09, 0.141_714_707_154_584_37, 0.161_323_005_414_330_42, 0.244_100_836_069_868_88],
    [-1.519_175_973_010_455, -0.466_781_619_370_088_94, -0.321_801_048_725_007_5, -0.227_409_827_031_048_28, -0.160_276_755_331_813_38, -0.111_372_954_183_789_62, -0.068_745_658_404_016, -0.031_969_761_294_450_796, 0.000_996_496_503_804_730_7, 0.032_063_698_210_668_12, 0.055_644_425_956_285_05, 0.078_480_453_448_495_29, 0.099_395_514_748_266_27, 0.118_667_558_180_981_46, 0.134_454_302_028_290_67, 0.160_085_610_585_324_58, 0.243_620_482_887_964_23],
    [-1.851_802_123_752_937_8, -0.442_901_924_798_356_94, -0.306_241_224_105_199_66, -0.218_393_896_840_544_55, -0.152_781_928_936_749_1, -0.105_302_167_725_466_2, -0.063_731_788_163_966_09, -0.027_993_950_693_936_04, 0.003_107_031_661_834_56, 0.033_211_399_261_393_7, 0.055_920_611_341_366_94, 0.078_296_182_835_739_72, 0.098_257_049_364_729, 0.115_019_577_606_951_1, 0.128_305_441_312_059_32, 0.159_486_563_722_783_9, 0.233_369_749_801_374_48],
    [-1.851_802_123_752_937_8, -0.441_413_014_223_692_67, -0.306_277_440_524_565_1, -0.213_614_985_474_908_87, -0.148_351_023_778_734_46, -0.101_741_047_353_626_23, -0.062_273_845_771_771_11, -0.027_477_807_998_279_33, 0.001_477_473_351_013_586_2, 0.030_437_413_105_742_873, 0.053_309_394_844_716_41, 0.074_537_098_985_525_31, 0.094_704_386_072_976_56, 0.110_518_957_690_553_31, 0.124_499_572_624_522_53, 0.156_169_444_422_676, 0.225_667_619_320_104_15],
    [-1.851_802_123_752_937_8, -0.439_765_298_188_011_47, -0.302_557_288_685_579_13, -0.205_485_685_024_377_56, -0.146_477_335_595_111_42, -0.098_167_046_260_058_16, -0.059_104_947_067_330_12, -0.027_460_844_548_359_3, -0.000_873_902_434_962_930_7, 0.025_849_486_463_640_47, 0.049_358_580_621_064_9, 0.068_589_973_303_693_51, 0.087_279_006_418_795_95, 0.102_809_878_569_247_81, 0.117_644_882_895_393_47, 0.147_768_004_221_790_2, 0.215_208_197_471_254_08],
    [-1.851_802_123_752_937_8, -0.407_444_494_949_185_55, -0.258_904_408_695_578_35, -0.177_421_683_061_832_7, -0.123_822_143_592_114_87, -0.077_151_142_475_727_44, -0.044_660_789_913_016_93, -0.019_927_144_661_102_302, 0.006_133_796_477_879_988, 0.029_444_197_668_747_11, 0.049_296_281_841_249_415, 0.064_650_749_746_798_28, 0.083_475_321_376_793_9, 0.096_742_496_895_565_85, 0.110_238_829_193_926_39, 0.132_328_309_671_629_3, 0.208_668_665_141_301_45],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_29: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.500_123_735_956_824, 10.0),
    (-12.165_503_204_856_376, -9.620_754_454_287_962),
    (-12.796_150_338_571_33, -10.126_203_710_225_14),
    (-12.884_547_191_599_696, -9.361_087_481_593_843),
    (-0.181_550_284_144_111_98, 1.406_811_870_546_060_8),
    (-0.291_487_435_389_190_36, 0.281_269_737_822_387_23),
    (1.647_537_087_812_366_3, 2.701_447_054_508_357_8),
    (0.354_477_216_010_315_73, 1.386_110_006_500_287_6),
    (-2.165_006_041_066_889_4, 0.055_210_422_336_245_37),
    (0.062_682_225_863_035_65, 0.845_229_089_559_448_3),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_29: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.118_272_463_864_445_7, 1.114_673_093_691_885_8, 1.090_145_923_599_326_5, 1.051_306_494_417_931, 1.010_355_871_746_886, 0.633_014_841_554_321_3, 0.625_681_037_160_974_2, 0.986_382_153_392_992, 0.806_267_350_655_852_4, 0.621_119_454_697_435_5, 0.582_045_901_467_825_4, 0.572_777_510_505_312_9, 0.572_513_203_113_305_9, 0.576_031_877_305_889_2, 0.575_274_732_062_552_6, 1.045_771_166_948_637_8, 1.046_603_559_284_134],
    [0.225_776_365_579_176_6, 0.206_379_420_897_873_64, 0.186_064_863_777_886_75, 0.161_441_204_819_215_14, 0.135_593_958_916_356_4, 0.108_473_676_454_350_71, 0.089_923_412_506_427_26, 0.102_394_123_982_405, 0.513_449_461_737_117_4, 0.467_925_670_638_328_5, 0.330_909_417_536_605_6, 0.263_146_851_560_224_05, 0.271_363_116_172_021_1, 0.308_176_743_817_040_85, 0.366_100_396_651_261_26, 1.040_804_983_193_066, 1.065_714_253_890_298_8],
    [0.252_390_530_231_488_2, 0.235_521_139_482_789_22, 0.209_905_036_234_318_42, 0.176_716_289_236_678_18, 0.139_494_958_970_862_48, 0.103_690_943_571_078_84, 0.069_594_414_962_660_21, 0.030_351_389_546_930_972, -0.008_457_507_620_124_757, -0.039_831_529_360_759_73, -0.078_615_658_567_338_74, -0.112_739_724_153_071_58, -0.148_107_741_417_022_74, -0.175_259_024_143_302_5, -0.211_482_541_674_388_15, -0.256_547_343_739_451_87, -0.291_497_488_660_539_1],
    [2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.245_367_068_744_538, 2.196_341_216_618_717, 1.946_833_865_057_706_2, 1.931_889_774_746_453_8, 1.975_295_353_626_568_7, 2.039_038_539_925_477, 2.092_289_835_713_160_5],
    [-1.290_031_723_366_974_7, -1.419_547_221_349_201_5, -1.536_954_977_928_027_4, -1.618_234_158_261_555_1, -1.633_100_687_658_839_9, -0.810_552_165_005_758_7, -0.685_722_504_293_830_1, -0.640_111_538_613_932_3, -0.608_242_462_525_663_4, -0.576_277_813_844_949_6, -0.548_899_483_806_281, -0.501_906_489_989_096_5, -0.003_955_085_855_284_692, 0.013_625_966_377_394_21, 0.027_923_358_445_733_1, 0.036_376_343_345_723_24, -0.484_047_457_340_898_66],
    [-1.611_852_982_398_538_4, -1.649_091_439_165_757_4, -1.689_593_477_648_851_3, -1.729_807_083_071_956, -1.757_615_884_825_320_7, -1.796_099_217_572_663_6, -1.835_610_723_641_158_3, -1.871_982_217_820_693_5, -1.905_510_506_555_278_4, -1.941_894_563_996_794_2, -1.981_522_942_026_141, -2.015_277_271_917_753, -2.051_133_864_279_816, -2.087_649_672_333_279, -2.119_806_885_576_841, -2.145_031_748_628_995, -2.163_070_581_279_977],
    [-0.596_108_799_753_416_4, -0.596_108_799_753_416_4, -0.597_581_038_254_729_6, -0.606_649_677_122_446_9, -0.618_808_558_486_542_7, -0.625_080_083_250_130_5, -0.645_016_245_032_328_9, -0.672_109_836_374_504, -0.700_775_818_160_529_3, -0.704_106_937_062_799_6, -0.703_176_204_975_496_5, -0.696_781_197_424_017_6, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1, -0.687_561_803_803_588_1],
    [0.138_315_134_514_091_94, 0.175_756_678_132_290_15, 0.282_414_311_099_740_2, 0.329_485_013_519_880_13, 0.378_186_866_920_106_6, 0.445_400_022_298_886_1, 0.481_464_316_089_394_4, 0.523_045_477_584_327_8, 0.575_545_463_150_330_8, 0.517_359_700_867_256_6, 0.475_492_842_644_005_65, 0.498_538_725_558_344_9, 0.549_678_776_756_298_4, 0.609_243_036_682_299_3, 0.672_305_847_881_411, 0.716_451_513_443_998_7, 0.714_211_648_726_942_2],
    [0.287_388_559_380_280_13, 0.239_825_363_507_502_9, -0.138_697_580_673_391_58, -0.345_493_757_612_862_6, -0.430_924_450_323_499_85, -0.499_737_632_590_354_7, -0.551_793_343_032_854_9, -0.539_961_650_599_433_9, -0.449_847_619_594_144_3, -0.336_561_056_667_280_4, 0.145_692_941_848_070_86, 0.228_128_947_087_734_37, 0.246_504_720_932_939_04, 0.252_952_111_394_112_5, 0.108_874_587_885_642_46, 0.137_929_952_670_071_8, 0.148_936_690_320_286_8],
    [9.957_892_491_417_047, 9.934_757_156_622_99, 9.913_740_039_501_961, 9.902_398_736_805_894, 9.892_154_106_675_022, 9.879_706_959_481_314, 9.866_782_421_699_964, 9.855_315_813_437_324, 9.842_993_799_993_662, 9.834_670_962_627_898, 9.835_059_275_051_696, 9.840_520_223_428_184, 9.850_632_915_171_143, 9.862_376_630_647_946, 9.868_798_256_813_252, 9.872_215_452_319_395, 9.882_960_361_151_001],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_29: [(f64, f64); 11] = [
    (5.419_185_498_579_54, 10.0),
    (0.320_048_993_757_405_53, 1.38),
    (-0.278_456_376_545_300_03, 1.971_095_891_507_528_3),
    (-0.757_710_994_177_599_5, 0.359_480_989_581_144_33),
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
    0.891_438_467_812_425_1,
    0.107_750_633_528_911_41,
    0.000_810_898_658_663_473_9,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_29: [f64; 7] = [
    1.075_581_180_105_165_8e-7,
    7.346_162_878_106_792e-6,
    1.322_780_481_526_321e-6,
    1.761_893_495_412_463_2e-6,
    2.602_063_430_065_225_5e-6,
    2.716_419_732_203_517e-6,
    4.644_387_743_151_514_7e-7,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_29: [f64; 5] = [
    0.0,
    0.0,
    0.112_399_352_125_516_66,
    0.870_650_723_722_042_3,
    0.016_949_924_152_440_95,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_29: [f64; 7] = [
    3.409_600_509_233_460_4e-7,
    6.957_957_240_257_341e-5,
    0.000_110_619_647_161_577_5,
    9.389_856_220_218_231e-5,
    8.335_940_152_007_363e-5,
    8.417_731_013_447_954e-5,
    0.000_123_796_033_137_762_68,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_29: [f64; 5] = [
    0.0,
    0.0,
    0.379_864_619_031_285_4,
    0.609_119_833_631_413,
    0.011_015_547_337_301_703,
];
// @end-table binary_awd

// @begin-table binary_xrb
// The X-ray binaries' sampler and share row refitted against plan 11's engine at generator
// version 29 on the generator's own pairs (plan 15, P15.T10.b).
//
// @generated by hyperion-fit 0.1.0, task `binary_xrb` revision 0. Do not edit.
// inputs-sha256: e801734a877e2f0e848c05d848d16ea678d511436554e17b960ae9855d530766
// manifest: crates/hyperion-fit/manifests/binary_xrb.toml
// data: none
// sim-fingerprint: 5a071611475533bb2b16ef5be3305f0f52fb6d60273009b86acf833166d133fe
// since-generator-version: 30
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the total against the brainstorm's 10⁴, the black-hole transients pinned at
//   Corral-Santana et al.'s (2016, A&A 587, A61) observed 1,300
// acceptance: slab members by kind 388/9725/4/91/7/0/239/4/0; young members by kind
//   543/400/107/168/874/4/96/0/4; census kinds
//   0.0375/0.9385/0.0007/0.0070/0.0009/0.0000/0.0150/0.0003/0.0000; weights
//   0.0329/0.8223/0.0006/0.1300/0.0132/0.0002 and 0.0007/0.0000/0.0000; black-hole transients
//   1300 of 1e4 (the census's 70); high-mass hosts 8; donor mass's own nodes 14; layers C 0.000
//   D 0.147 E 0.853; rates a system 1.54e-4/2.29e-4/1.67e-4/1.30e-4/1.98e-4/1.21e-4/2.94e-5;
//   scratch marks: NeutronStarRecurrence, BlackHoleRecurrence, SupergiantPeriod,
//   SupergiantDonorMass, SupergiantRate, SupergiantRecoil, SupergiantSinceSupernova,
//   UltracompactRecurrence, SymbioticPeriod, SymbioticDonorMass, SymbioticFilling,
//   BlackHoleHighMassAccretorMass, BlackHoleHighMassPeriod, BlackHoleHighMassDonorMass,
//   BlackHoleHighMassRate; the K–S tests against the other half of the census are the sim's slow
//   `fitted_tables::the_x_ray_binary_sampler_against_the_engine`
//
// Read by `XRB_SAMPLER` and `CLASS_SHARES`, before the block. The marks are fitted to the
// engine's members of the nine kinds `XrbHostKind` holds at generator version 30's rules
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
    [1.147_638_965_107_692_8, 1.265_750_811_125_421_9, 1.282_073_957_663_440_8, 1.294_227_191_991_955_5, 1.304_341_112_997_684_3, 1.313_652_047_880_166_8, 1.322_665_960_741_281_4, 1.332_469_931_184_952_2, 1.344_012_613_482_192_1, 1.358_707_481_249_884_7, 1.377_463_755_166_355_8, 1.398_384_121_726_136_1, 1.505_047_143_382_866, 1.561_465_551_158_789_2, 1.611_414_233_922_962, 1.681_319_179_186_412_6, 1.975_628_261_510_142_4],
    [2.106_510_635_585_874, 2.475_037_411_086_844, 2.746_370_566_519_563_5, 2.903_587_089_840_107_7, 3.026_388_171_891_752_2, 3.074_705_763_439_319, 3.205_694_723_182_726_4, 3.297_770_654_348_684, 3.440_549_839_558_064_3, 3.533_948_838_522_814, 3.727_603_769_800_983_6, 3.907_593_813_488_492, 4.207_298_260_913_865, 4.699_328_480_807_889, 5.346_210_024_465_344_5, 6.037_926_841_076_648, 8.787_813_035_339_518],
    [-0.372_612_994_664_013_3, 0.043_536_595_835_613_85, 0.136_333_535_102_982_2, 0.182_590_326_442_758_85, 0.214_890_632_886_976_9, 0.240_369_434_822_138_6, 0.321_122_537_198_567_26, 0.386_845_944_983_054_75, 0.443_764_675_907_754_54, 0.489_697_793_455_240_17, 0.545_500_930_913_698_3, 0.642_024_072_574_898_1, 1.010_371_161_381_574_2, 1.207_026_286_487_487_4, 1.415_560_971_973_887, 1.642_675_796_875_287_6, 3.355_474_783_371_506],
    [-0.829_916_953_564_098_9, -0.463_085_981_259_719_84, -0.299_837_836_493_186_46, -0.173_519_207_958_975_16, -0.079_308_516_572_505_36, -0.039_313_221_988_588_685, -0.009_140_717_293_204_807, 0.010_232_415_758_440_135, 0.026_196_367_942_759_254, 0.045_546_400_138_139_036, 0.066_114_623_048_623_41, 0.086_075_237_493_620_32, 0.110_926_039_310_852_5, 0.137_835_594_134_615_44, 0.187_699_724_014_326_64, 0.319_568_096_648_143_7, 0.709_721_122_717_388_6],
    [-5.596_358_743_396_394, -2.264_169_369_416_198, -2.081_176_938_381_399_4, -1.879_024_014_366_985, -1.694_540_826_706_390_2, -1.549_568_118_303_588_5, -1.434_307_350_717_32, -1.332_590_667_246_271_7, -1.235_298_276_554_588, -1.138_369_935_118_009_2, -1.043_130_889_324_367_8, -0.951_055_813_777_872_1, -0.859_686_681_043_022_5, -0.778_035_641_201_368_9, -0.674_009_015_906_88, -0.372_199_700_972_246_2, 1.454_811_353_166_347_1],
    [1.75, 2.254_32, 2.435_993_5, 2.563_591, 2.667_624_5, 2.758_895_5, 2.842_726, 2.922_329_5, 3.0, 3.077_670_5, 3.157_274, 3.241_104_5, 3.332_375_5, 3.436_409, 3.564_006_5, 3.745_68, 4.25],
    [1.25, 1.754_319_999_999_999_9, 1.935_993_5, 2.063_591, 2.167_624_5, 2.258_895_5, 2.342_726, 2.422_329_5, 2.5, 2.577_670_5, 2.657_274, 2.741_104_5, 2.832_375_5, 2.936_409, 3.064_006_5, 3.245_68, 3.75],
    [0.355_095_299_505_206_4, 1.224_394_065_855_718_6, 1.411_788_279_891_719_6, 1.568_370_819_513_610_1, 1.660_099_829_329_538_4, 1.744_747_454_737_008_8, 1.817_667_084_926_251_5, 1.878_338_158_427_55, 1.934_138_445_528_797_8, 2.006_770_003_505_571_8, 2.054_093_044_746_226_2, 2.115_126_394_970_044, 2.180_973_994_729_009_7, 2.259_967_737_309_388, 2.361_429_251_404_064, 2.506_686_347_897_381_8, 2.900_971_840_233_724],
    [0.005_043_077_937_489_4, 0.023_108_348_512_313_755, 0.031_753_814_145_039_74, 0.039_947_703_970_367_18, 0.050_166_691_050_113_094, 0.057_849_691_871_625_565, 0.065_508_078_535_605_4, 0.074_570_061_390_614_84, 0.084_782_387_316_155_92, 0.095_101_836_279_396_76, 0.105_607_816_788_209_64, 0.116_843_867_818_028_34, 0.129_326_648_207_468_4, 0.149_353_515_156_060_93, 0.169_331_447_841_784_44, 0.204_879_500_429_589_13, 0.700_909_782_619_88],
    [8.006_095_233_936_879, 8.340_719_109_060_089, 8.751_177_336_209_983, 9.084_776_660_770_814, 9.383_352_886_326_36, 9.629_300_607_535_48, 9.831_492_475_890_968, 10.099_450_946_255_653, 10.390_837_530_986_403, 10.689_787_728_759_612, 10.997_640_113_684_794, 11.377_662_895_304_525, 11.682_340_427_694_394, 12.115_550_991_323_401, 12.865_646_165_603_911, 13.701_891_708_249_047, 21.068_498_767_664_43],
    [-0.150_000_000_000_000_02, 0.152_592, 0.261_596_1, 0.338_154_599_999_999_97, 0.400_574_699_999_999_95, 0.455_337_299_999_999_94, 0.505_635_6, 0.553_397_7, 0.6, 0.646_602_3, 0.694_364_4, 0.744_662_7, 0.799_425_3, 0.861_845_4, 0.938_403_9, 1.047_408, 1.35],
    [10.0, 14.034_559_999_999_999, 15.487_948, 16.508_728, 17.340_996, 18.071_164, 18.741_808, 19.378_636, 20.0, 20.621_364, 21.258_192, 21.928_836, 22.659_004, 23.491_272, 24.512_052, 25.965_44, 30.0],
    [-10.25, -9.745_68, -9.564_006_5, -9.436_409, -9.332_375_5, -9.241_104_5, -9.157_274, -9.077_670_5, -9.0, -8.922_329_5, -8.842_726, -8.758_895_5, -8.667_624_5, -8.563_591, -8.435_993_5, -8.254_32, -7.75],
    [0.290_321_974_951_994, 1.360_703_801_654_285_8, 1.791_539_603_668_360_4, 2.191_877_858_008_707_5, 2.554_488_011_089_232, 3.009_128_219_243_454_5, 3.481_701_605_997_549, 3.910_090_378_462_824, 4.337_814_777_660_802, 4.944_802_704_127_848, 5.444_970_985_056_721, 6.080_732_467_150_596, 6.673_462_238_939_739, 7.482_218_748_841_761, 8.495_384_723_673_219, 11.075_712_796_165_02, 26.700_840_877_730_062],
    [0.0, 21.541_471_172_88, 27.883_959_356_160_005, 32.722_835_660_352_004, 36.900_838_535_952_005, 40.732_034_499_504, 44.381_465_974_608_005, 47.957_508_026_832, 51.545_915_694_144_01, 55.228_255_120_512, 59.095_911_217_440_005, 63.268_150_174_896, 67.923_921_544_560_02, 73.375_080_106_944_01, 80.282_901_424_464, 90.636_775_035_600_01, 120.071_697_528_624_01],
    [4.176_028_263_254_182_5, 6.164_173_911_592_245, 6.409_412_463_613_848, 6.565_853_053_528_666, 6.655_441_249_789_558_5, 6.747_463_088_053_397, 6.828_646_772_791_345, 6.884_940_758_006_593, 6.946_703_657_678_007_5, 6.991_065_945_037_286_5, 7.041_286_460_321_768, 7.080_163_277_567_027, 7.126_342_616_713_086, 7.181_846_615_569_436, 7.229_125_287_561_465, 7.307_502_221_225_171, 7.477_784_022_709_876],
    [5.7, 5.901_728, 5.974_397_4, 6.025_436_4, 6.067_049_8, 6.103_558_2, 6.137_090_4, 6.168_931_8, 6.2, 6.231_068_2, 6.262_909_6, 6.296_441_8, 6.332_950_2, 6.374_563_6, 6.425_602_6, 6.498_272, 6.7],
    [0.778_677_298_083_905_8, 0.869_472_169_370_513_4, 0.892_549_818_720_358_5, 0.912_648_159_263_643_3, 0.928_013_728_766_872_9, 0.939_438_066_741_361_8, 0.955_341_976_545_370_9, 0.970_119_512_060_710_8, 0.984_373_182_403_005_2, 0.998_453_006_268_348, 1.010_984_545_478_478_7, 1.022_961_310_245_956_7, 1.033_412_951_611_085_7, 1.045_271_388_169_827_2, 1.058_731_098_372_949_2, 1.076_896_223_821_663_5, 1.446_931_504_220_512_6],
    [1.037_076_384_580_339, 1.049_629_173_644_326_8, 1.064_714_525_405_369_1, 1.070_698_323_186_928_3, 1.076_713_044_396_177, 1.081_272_812_769_883_2, 1.087_279_520_771_140_8, 1.092_750_504_721_553_8, 1.099_645_388_177_754, 1.108_129_413_826_055_3, 1.113_024_702_328_744_7, 1.127_438_896_273_824_2, 1.195_076_781_005_984_8, 1.249_123_623_104_572_1, 1.327_759_429_873_471_2, 1.364_938_342_573_612_8, 1.629_310_277_259_491_8],
    [-0.001_813_727_417_228_904_4, 0.285_798_105_652_917_4, 0.440_154_433_938_482_64, 0.655_316_762_998_526_6, 0.939_955_657_095_719_5, 1.110_454_727_542_167_7, 1.212_489_330_615_462_5, 1.292_924_049_341_905_9, 1.373_130_567_267_811_8, 1.452_622_089_294_794_7, 1.548_384_041_211_272_8, 1.671_394_902_355_113, 2.082_582_511_010_882_4, 2.269_880_812_310_468_3, 2.372_175_720_352_982_5, 2.461_398_706_767_498, 2.802_935_133_072_123_7],
    [1.028_163_506_696_164_3, 1.590_448_881_278_807_8, 1.675_199_984_166_832_1, 1.711_941_100_445_766_3, 1.741_622_610_380_917_4, 1.760_328_340_433_901_2, 1.776_487_749_537_83, 1.791_569_929_334_343_4, 1.821_705_743_467_612, 1.832_212_864_297_448_2, 1.847_947_655_084_187_6, 1.860_046_222_328_031_2, 1.868_837_019_094_957_5, 1.874_268_693_287_592_7, 1.883_245_859_049_554, 1.888_722_242_847_195_3, 1.909_120_072_658_062_2],
    [0.0, 0.403_455_999_999_999_9, 0.548_794_8, 0.650_872_799_999_999_9, 0.734_099_6, 0.807_116_4, 0.874_180_8, 0.937_863_6, 1.0, 1.062_136_4, 1.125_819_2, 1.192_883_6, 1.265_900_4, 1.349_127_2, 1.451_205_2, 1.596_544_000_000_000_2, 2.0],
    [0.105_170_738_331_146_57, 0.285_080_974_056_148_13, 0.303_657_226_927_399_9, 0.309_395_397_400_773_1, 0.314_651_012_003_535_4, 0.319_312_394_978_458_3, 0.323_522_935_998_010_2, 0.327_176_378_522_828_55, 0.333_795_161_542_274_2, 0.341_064_372_578_221_1, 0.344_611_742_101_547_5, 0.355_117_374_251_559_2, 0.362_993_088_766_211_17, 0.373_143_679_512_854_8, 0.390_515_908_410_552_6, 0.429_091_496_858_523_4, 0.836_266_432_148_279],
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
    [-0.060_978_975_715_102_246, 0.019_660_884_432_308_83, 0.043_703_189_016_988_48, 0.061_883_867_574_503_065, 0.078_344_350_027_276_94, 0.090_319_363_295_308_25, 0.097_858_403_541_992_67, 0.105_274_594_939_004_07, 0.112_563_948_988_774_32, 0.121_337_295_290_736_54, 0.133_197_258_585_239_6, 0.210_809_427_318_772_25, 0.321_509_618_995_627_7, 0.406_312_639_291_465_2, 0.499_090_122_135_269_93, 0.604_046_908_301_905_4, 0.695_326_871_860_770_2],
    [-0.687_304_919_528_244_1, -0.484_589_566_319_257_65, -0.379_195_651_762_657_07, -0.273_947_501_736_815, -0.171_612_879_636_687_87, -0.056_035_070_217_465_9, -0.014_461_596_464_010_55, 0.005_751_592_041_626_962, 0.026_229_678_306_112_097, 0.051_386_824_777_962_074, 0.073_655_740_405_696_32, 0.096_214_907_569_081_05, 0.114_523_336_962_175_66, 0.166_298_843_706_194_33, 0.431_143_739_810_653_07, 0.599_057_187_505_965_9, 0.709_721_122_717_388_6],
    [-0.687_304_919_528_244_1, -0.537_519_019_989_268_8, -0.458_826_186_786_824_3, -0.388_302_024_025_218_63, -0.305_767_677_670_471_86, -0.217_518_292_820_083_34, -0.128_569_882_376_995_96, -0.045_971_556_067_277_76, -0.015_575_008_445_851_841, 0.003_269_287_759_913_827_2, 0.025_585_027_915_293_206, 0.054_742_200_864_549_284, 0.087_215_290_774_870_46, 0.123_176_524_298_089_04, 0.236_353_384_857_500_51, 0.455_906_578_663_564_65, 0.709_721_122_717_388_6],
    [-0.687_304_919_528_244_1, -0.533_299_657_218_438_1, -0.437_306_795_408_187_4, -0.327_020_716_635_769_15, -0.201_554_919_571_217_2, -0.070_475_039_973_700_48, -0.029_845_558_163_903_645, -0.007_401_656_660_758_246, 0.015_009_873_553_763_056, 0.039_097_140_856_949_3, 0.063_253_506_113_589_06, 0.086_710_806_700_416_21, 0.110_586_873_700_836_22, 0.138_451_953_254_593_25, 0.176_579_273_750_238_7, 0.276_261_710_079_193_1, 0.709_721_122_717_388_6],
    [-0.687_304_919_528_244_1, -0.376_494_935_435_136_57, -0.140_242_559_772_994_88, -0.052_978_325_804_358_156, -0.025_386_468_787_028_316, -0.002_492_329_855_134_988, 0.019_899_741_249_467_955, 0.039_962_520_220_871_49, 0.058_808_426_758_502_036, 0.075_105_450_401_507_08, 0.091_495_789_733_720_91, 0.108_242_768_054_453_08, 0.126_011_711_975_245_22, 0.144_876_850_364_144_24, 0.167_139_533_474_372_71, 0.199_255_056_271_562_18, 0.709_721_122_717_388_6],
    [-0.527_915_693_302_112_7, -0.169_180_716_077_086_93, -0.074_160_749_057_153_18, -0.035_019_147_621_413_94, -0.010_764_087_437_006_247, 0.012_769_448_554_237_23, 0.033_055_885_978_582_18, 0.051_860_054_965_181_78, 0.069_681_265_341_679_86, 0.081_886_599_558_060_13, 0.099_394_361_978_520_67, 0.115_530_139_044_008_29, 0.130_214_703_926_098_77, 0.148_154_860_628_101_5, 0.169_876_846_633_326_47, 0.204_415_789_903_254_7, 0.475_944_355_546_581_2],
    [-0.591_113_065_302_852_8, -0.191_590_956_541_713_26, -0.147_295_532_866_617_06, -0.089_956_137_156_008_32, -0.039_334_589_653_137_25, -0.008_611_495_420_696_18, 0.017_984_360_702_772_192, 0.039_353_744_529_623_83, 0.060_180_648_985_141_61, 0.079_075_200_193_918_42, 0.097_494_426_916_089_85, 0.119_408_011_779_162_38, 0.141_035_532_604_064_07, 0.165_582_387_527_283_21, 0.219_887_876_162_867_65, 0.316_641_952_492_293_65, 0.475_944_355_546_581_2],
    [-0.689_138_064_799_683_9, -0.194_113_056_322_925_5, -0.124_002_298_019_750_52, -0.085_772_382_918_099_22, -0.061_606_408_967_378_165, -0.040_077_040_065_612_73, -0.012_876_530_133_327_18, 0.003_467_597_827_706_156_4, 0.018_845_209_658_821_603, 0.029_528_773_287_104_966, 0.040_275_456_514_152_284, 0.052_771_372_100_609_69, 0.069_077_423_695_769_97, 0.089_076_038_467_356_41, 0.148_137_381_714_801_98, 0.228_072_794_688_630_3, 0.475_944_355_546_581_2],
    [-0.726_744_675_257_158_1, -0.215_372_979_961_114_5, -0.116_004_689_727_164_63, -0.079_504_431_001_793_68, -0.062_673_594_396_711_55, -0.047_146_776_436_378_35, -0.014_445_378_014_537_302, 0.003_637_089_458_810_438_5, 0.013_517_872_447_470_465, 0.020_546_605_305_104_097, 0.028_959_531_197_394_712, 0.036_359_167_851_195_716, 0.044_852_590_491_677_1, 0.056_626_976_843_317_31, 0.074_535_586_840_629_23, 0.120_597_300_577_014_86, 0.450_687_390_873_055_8],
    [-0.829_916_953_564_098_9, -0.310_568_474_472_516_3, -0.157_037_671_680_110_1, -0.085_385_977_273_505_47, -0.058_311_507_860_054_484, -0.028_803_195_111_319_707, 0.000_868_058_828_608_640_6, 0.010_230_447_255_235_548, 0.013_516_580_174_452_14, 0.019_745_994_534_637_986, 0.027_612_045_333_659_716, 0.035_638_608_373_472_74, 0.045_164_826_880_381_96, 0.059_131_562_676_776_835, 0.079_293_673_934_807_67, 0.131_461_578_659_209_24, 0.327_613_831_416_881_16],
    [-0.829_916_953_564_098_9, -0.464_367_278_580_138_7, -0.218_683_550_900_024_64, -0.119_275_823_946_128_3, -0.049_034_250_507_247_6, -0.001_136_714_104_342_142_4, 0.007_002_430_184_469_422, 0.012_111_407_275_804_955, 0.015_171_367_272_685_095, 0.024_184_432_252_581_087, 0.037_136_582_230_731_81, 0.048_107_757_125_468_81, 0.067_337_085_570_556_8, 0.081_348_695_893_182_32, 0.130_322_168_344_806_34, 0.195_590_950_493_236, 0.548_228_941_545_350_8],
    [-0.829_916_953_564_098_9, -0.560_467_152_286_749_3, -0.383_523_297_385_488_85, -0.215_365_866_729_908_32, -0.137_589_051_562_108_35, -0.063_335_422_504_348_69, -0.001_126_580_954_449_137_4, 0.009_591_326_532_811_764, 0.019_520_144_192_135_234, 0.036_616_347_837_769_22, 0.051_388_648_744_479_98, 0.072_865_499_899_108_33, 0.081_577_489_198_552, 0.133_407_380_711_954_2, 0.191_106_227_288_188_77, 0.288_410_590_099_795_86, 0.548_228_941_545_350_8],
    [-0.829_916_953_564_098_9, -0.561_863_034_883_034_2, -0.462_360_197_715_974_64, -0.355_583_004_035_019_86, -0.249_065_833_968_956_82, -0.190_316_703_311_175_48, -0.137_210_389_450_575_68, -0.088_923_230_966_062_62, -0.053_821_545_629_455_41, 0.012_204_487_861_611_458, 0.042_843_847_512_244_286, 0.074_538_583_055_786_49, 0.095_159_681_628_687_99, 0.197_078_130_149_788_23, 0.257_852_692_060_833_4, 0.362_541_880_302_500_5, 0.548_228_941_545_350_8],
    [-0.806_586_860_905_147_9, -0.464_293_878_866_437_97, -0.376_189_522_855_203_8, -0.265_176_808_671_381_5, -0.220_169_887_588_797_5, -0.191_397_697_825_072_35, -0.159_299_493_329_351_6, -0.116_437_745_842_666_53, -0.076_943_476_883_529_33, -0.063_116_037_531_589, -0.017_595_714_688_889_08, 0.023_354_651_009_106_005, 0.089_482_459_820_320_95, 0.215_017_540_152_794_17, 0.338_367_149_702_995_4, 0.496_266_121_227_495_6, 0.548_228_941_545_350_8],
    [-0.829_916_953_564_098_9, -0.463_085_981_259_719_84, -0.299_837_836_493_186_46, -0.173_519_207_958_975_16, -0.079_308_516_572_505_36, -0.039_313_221_988_588_685, -0.009_140_717_293_204_807, 0.010_232_415_758_440_135, 0.026_196_367_942_759_254, 0.045_546_400_138_139_036, 0.066_114_623_048_623_41, 0.086_075_237_493_620_32, 0.110_926_039_310_852_5, 0.137_835_594_134_615_44, 0.187_699_724_014_326_64, 0.319_568_096_648_143_7, 0.709_721_122_717_388_6],
    [-0.829_916_953_564_098_9, -0.463_085_981_259_719_84, -0.299_837_836_493_186_46, -0.173_519_207_958_975_16, -0.079_308_516_572_505_36, -0.039_313_221_988_588_685, -0.009_140_717_293_204_807, 0.010_232_415_758_440_135, 0.026_196_367_942_759_254, 0.045_546_400_138_139_036, 0.066_114_623_048_623_41, 0.086_075_237_493_620_32, 0.110_926_039_310_852_5, 0.137_835_594_134_615_44, 0.187_699_724_014_326_64, 0.319_568_096_648_143_7, 0.709_721_122_717_388_6],
    [-0.829_916_953_564_098_9, -0.463_085_981_259_719_84, -0.299_837_836_493_186_46, -0.173_519_207_958_975_16, -0.079_308_516_572_505_36, -0.039_313_221_988_588_685, -0.009_140_717_293_204_807, 0.010_232_415_758_440_135, 0.026_196_367_942_759_254, 0.045_546_400_138_139_036, 0.066_114_623_048_623_41, 0.086_075_237_493_620_32, 0.110_926_039_310_852_5, 0.137_835_594_134_615_44, 0.187_699_724_014_326_64, 0.319_568_096_648_143_7, 0.709_721_122_717_388_6],
];

/// The X-ray binaries' marks' medians' spans of their conditions, first and last node, in
/// [`XrbMark`]'s order; only the low-mass donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIAN_SPANS_29: [(f64, f64); 30] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (-0.178_616_418_771_557_57, 2.432_128_794_428_993_8),
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
    [-1.447_002_974_944_649_1, -1.627_836_437_500_101, -1.671_578_240_202_881_4, -0.906_373_282_081_813_4, -0.720_379_855_495_411, -0.624_657_773_691_683_5, -0.516_869_679_311_561_1, -0.029_463_104_335_104_875, 0.044_355_077_502_733_26, 0.118_842_369_452_247_28, 0.190_509_407_024_422_7, 0.204_698_555_453_161_68, 0.019_636_220_044_392_652, -0.308_733_681_947_901_3, -0.469_873_826_278_093_4, -0.472_552_001_834_108_5, -0.510_943_102_137_855_7],
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
    (0.355_095_299_505_206_4, 2.900_971_840_233_724),
    (0.005_043_077_937_489_4, 0.700_909_782_619_88),
    (8.006_095_233_936_879, 21.068_498_767_664_43),
    (0.1, 1.2),
    (10.0, 35.0),
    (-10.25, -7.75),
    (0.290_321_974_951_994, 26.700_840_877_730_062),
    (0.0, 120.1),
    (4.176_028_263_254_182_5, 7.477_784_022_709_876),
    (5.5, 6.7),
    (0.778_677_298_083_905_8, 1.446_931_504_220_512_6),
    (1.037_076_384_580_339, 1.629_310_277_259_491_8),
    (-0.001_813_727_417_228_904_4, 2.802_935_133_072_123_7),
    (1.028_163_506_696_164_3, 1.909_120_072_658_062_2),
    (0.0, 2.0),
    (0.105_170_738_331_146_57, 0.836_266_432_148_279),
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
    0.032_888_269_332_923_235,
    0.822_307_253_155_159,
    0.000_644_176_453_112_762_5,
    0.13,
    0.013_179_328_232_212_82,
    0.000_224_955_633_095_129_94,
];

/// The high-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.
#[rustfmt::skip]
pub const XRB_HIGH_MASS_KINDS_29: [f64; 3] = [
    0.000_749_757_076_344_267_6,
    3.130_058_576_364_314_8e-6,
    3.130_058_576_364_314_8e-6,
];

/// The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const XRB_HOSTS_PER_FORMED_MASS_29: [f64; 7] = [
    8.501_773_212_939_689e-8,
    1.264_999_945_082_521e-7,
    9.239_418_906_957_822e-8,
    7.154_294_350_658_4e-8,
    1.090_574_741_670_784_2e-7,
    6.651_660_140_057_013e-8,
    1.624_585_724_472_513_2e-8,
];

/// The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const XRB_LAYER_SHARES_29: [f64; 5] = [
    0.0,
    0.0,
    0.0,
    0.147_341_578_098_620_83,
    0.852_658_421_901_379_2,
];
// @end-table binary_xrb

// @begin-table type_ia_yield
// The Type Ia explosion mark's yield η by delay bin and channel, fitted against plan 11's
// engine's pool at generator version 30's rules (plan 15, P15.T9.b).
//
// @generated by hyperion-fit 0.1.0, task `type_ia_yield` revision 0. Do not edit.
// inputs-sha256: 0b5130f58884e9100b6eccb687a764380233a9e38bcb6917afd3ed1f1ffb448a
// manifest: crates/hyperion-fit/manifests/type_ia_yield.toml
// data: none
// sim-fingerprint: e36ed6d9e887a88828e40a567e10e867be56c5ebf741e960e795155ac2b2d809
// since-generator-version: 28
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and pool (P11.T6, ruling 129.1's detonatable rule), over plan 06's tracks, at the
//   manifest's generator version's rules; the target Maoz and Graur 2017's 1.3 × 10⁻³ per M☉
//   formed (`type_ia_delay`), the known shortfall Claeys et al. 2014, A&A 563, A83, §4.3 and
//   §7.2; the contacts against Maoz, Hallakoun and Badenes 2018, MNRAS 476, 2584
// acceptance: 24 of 24 bins capped at η 1, every short bin among them (passes), the pool ÷
//   target by bin
//   0.005/0.110/0.431/0.390/0.510/0.648/0.761/0.587/0.135/0.012/0.025/0.110/0.225/0.423/0.079/0.094/0.098/0.106/0.104/0.108/0.105/0.094/0.093/0.099;
//   the pool 3.1053e-4 per M☉ formed, 0.239 of the yield (mergers 0.056, accretions 0.183);
//   exploded 3.1053e-4, min(pool, target) in every bin within 0.00 σ (passes); η of the even and
//   odd parts within 0.00 σ (passes; both under 4.098 σ, α = 10⁻³ over the bins); 16173
//   detonatable candidates (mergers 3802, accretions 12371) of 41655; records C/D/E
//   7924768/1625728/449568, pairs 1709088/951507/301363; recorded, not asserted: the double
//   white dwarfs' contacts over the last Gyr 4.092e-14 per M☉ formed a year, 0.567 × the
//   delay-time distribution's 7.221e-14 at the same ages (Maoz, Hallakoun and Badenes 2018:
//   4.5–7)
//
// Read by `IA_YIELD`. Each bin's pool is the engine's detonatable candidates per solar mass formed
// (hyperion-fit's `tasks::type_ia_yield`): each grid record's first attempt's pairs of layers C–E
// about the solar circle run to 1.3700e10 years under generator version 30's rules, each band's
// candidates a record weighted by its share of the systems over the mass formed a system. η is
// min(1, target ÷ pool), the target the bin's share of the observed 1.3 × 10⁻³ per solar mass
// formed, both channels of a bin thinned alike; 24 of 24 bins are capped at 1, the pool short
// there. Every bin is capped, so η is the scratch's 1 everywhere and no generated value moves. The
// census ran 10000064 records in parts 0–31 of 64.

/// η by delay bin of [`DELAY_EDGES`] and then by channel, the merger first: the share
/// of the engine's detonatable pooled candidates that explode, min(1, target ÷ pool).
#[rustfmt::skip]
pub const IA_YIELD_ETA: [[f64; 2]; 24] = [
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [1.0, 1.0],
];
// @end-table type_ia_yield
// @begin-table binary_merger
// The stellar mergers' sampler and share row fitted against plan 11's engine at generator
// version 31 on the generator's own pairs (plan 15, P15.T10.b), held until 31.
//
// @generated by hyperion-fit 0.1.0, task `binary_merger` revision 0. Do not edit.
// inputs-sha256: ab00c82f45a90665f044fca8c84efa29db5affa712bdfed13211022e15a2ae58
// manifest: crates/hyperion-fit/manifests/binary_merger.toml
// data: none
// sim-fingerprint: 40487d42606c09e8802e50176885d325c9212ab792255ffbdbf45abc59c5ca26
// since-generator-version: 29
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the total against the brainstorm's 9 × 10⁴ mergers in the source horizon (0.34 a
//   year, Kochanek et al. 2014, MNRAS 443, 1319)
// acceptance: 7175 members, Kish 2245; of the weight, uncovered 0.435, contact 0.017, evolved
//   donor 0.528, dynamical 0.020; channels 0.030/0.935/0.035; layers
//   0.000/0.000/0.006/0.003/0.991 (target 0.000/0.000/0.665/0.317/0.019, realised
//   0.000/0.000/0.665/0.317/0.019); layers 0.008/0.019/0.589/0.336/0.048 rates a system
//   3.90e-11/9.73e-13/2.68e-13/2.17e-13/3.12e-13/7.05e-13/2.37e-13; scratch marks: PrimaryRank,
//   ContactMassRatio, HeliumBurningMassRatio; the K–S tests against the other half of the census
//   are the sim's slow `merger_tables::the_stellar_merger_sampler_against_the_engine`
//
// Read by `MERGER_SAMPLER_31` and the class shares' rows from 31, after the block. The stellar
// mergers' marks are fitted to the engine's own mergers at generator version 31's rules through the
// sim's merger census of the pairs the generator draws at each record's first attempt
// (hyperion-fit's `tasks::binary_classes::mergers`), each member weighted by its age hazard at its
// merger age: a mark's weighted quantiles at the ranks k ÷ 16, a mark the class draws after the
// layer conditional on it, each layer's own law where its members' Kish size is 50 or more, and the
// layer choice at its fixed point over the members' merger ages. A mark with a Kish size under 50
// keeps the scratch sampler's law. The row is the members' weighted rate a record by population and
// band, scaled to 90000 mergers in the source horizon at Milky Way parameters (plan 11's design
// note 11). The censuses ran 5500416 records.

/// The stellar mergers' marks' residual quantiles at the ranks k ÷ 16 over all their members, in
/// [`MergerMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const MERGER_RESIDUALS_31: [[f64; 17]; 10] = [
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
];

/// The stellar mergers' marks' laws at their medians' 17 nodes: each mark's 17 rows in
/// [`MergerMark`]'s order, each its node's quantiles at the ranks k ÷ 16; node 4b is layer b's
/// for a mark conditional on the layer, and an unconditional mark's are not read.
#[rustfmt::skip]
pub static MERGER_NODE_RESIDUALS_31: [[f64; 17]; 170] = [
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [7.806_013_585_007_595e-5, 0.034_391_428_265_090_27, 0.066_109_833_705_639_22, 0.104_333_382_185_221_45, 0.144_263_322_552_112_13, 0.192_446_177_878_745_57, 0.248_325_818_052_373_41, 0.302_468_206_755_981_35, 0.364_378_717_374_923_17, 0.431_686_218_983_821_8, 0.494_866_231_468_980_7, 0.561_753_198_137_045_7, 0.634_797_685_009_544_1, 0.728_225_557_790_076_8, 0.796_266_947_376_528, 0.896_923_870_119_457_5, 0.960_891_953_344_333],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.0, 0.062_5, 0.125, 0.187_5, 0.25, 0.312_5, 0.375, 0.437_5, 0.5, 0.562_5, 0.625, 0.687_5, 0.75, 0.812_5, 0.875, 0.937_5, 1.0],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.06, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.09, 0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.15],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.023_628_441_317_916_814, 0.068_962_411_777_938_78, 0.088_334_896_946_689_68, 0.135_430_886_480_579_05, 0.240_122_727_831_408_26, 0.320_121_948_384_238_35, 0.404_255_494_441_150_83, 0.466_978_712_793_215_8, 0.558_886_186_228_954_6, 0.639_895_207_468_847_4, 0.712_435_164_234_138_9, 0.761_167_377_485_141_5, 0.790_294_607_815_558_3, 0.821_556_188_660_250_6, 0.843_019_799_473_230_7, 0.868_934_779_702_501, 1.0],
    [0.023_628_441_317_916_814, 0.068_962_411_777_938_78, 0.088_334_896_946_689_68, 0.135_430_886_480_579_05, 0.240_122_727_831_408_26, 0.320_121_948_384_238_35, 0.404_255_494_441_150_83, 0.466_978_712_793_215_8, 0.558_886_186_228_954_6, 0.639_895_207_468_847_4, 0.712_435_164_234_138_9, 0.761_167_377_485_141_5, 0.790_294_607_815_558_3, 0.821_556_188_660_250_6, 0.843_019_799_473_230_7, 0.868_934_779_702_501, 1.0],
    [0.023_628_441_317_916_814, 0.068_962_411_777_938_78, 0.088_334_896_946_689_68, 0.135_430_886_480_579_05, 0.240_122_727_831_408_26, 0.320_121_948_384_238_35, 0.404_255_494_441_150_83, 0.466_978_712_793_215_8, 0.558_886_186_228_954_6, 0.639_895_207_468_847_4, 0.712_435_164_234_138_9, 0.761_167_377_485_141_5, 0.790_294_607_815_558_3, 0.821_556_188_660_250_6, 0.843_019_799_473_230_7, 0.868_934_779_702_501, 1.0],
    [0.023_628_441_317_916_814, 0.068_962_411_777_938_78, 0.088_334_896_946_689_68, 0.135_430_886_480_579_05, 0.240_122_727_831_408_26, 0.320_121_948_384_238_35, 0.404_255_494_441_150_83, 0.466_978_712_793_215_8, 0.558_886_186_228_954_6, 0.639_895_207_468_847_4, 0.712_435_164_234_138_9, 0.761_167_377_485_141_5, 0.790_294_607_815_558_3, 0.821_556_188_660_250_6, 0.843_019_799_473_230_7, 0.868_934_779_702_501, 1.0],
    [0.003_461_791_981_956_953_3, 0.023_470_628_927_344_788, 0.028_999_344_729_091_877, 0.031_926_468_916_274_21, 0.034_520_861_406_762_374, 0.038_230_637_270_436_32, 0.043_030_924_411_576_68, 0.047_041_260_197_663_35, 0.050_484_493_306_801_756, 0.051_755_110_060_749_49, 0.055_220_520_341_593_93, 0.060_876_821_706_473_46, 0.957_939_087_978_319_3, 0.973_550_365_239_555_8, 0.978_134_221_752_613_3, 0.982_300_565_490_167_6, 0.990_757_447_585_685_7],
    [0.003_461_791_981_956_953_3, 0.023_470_628_927_344_788, 0.028_999_344_729_091_877, 0.031_926_468_916_274_21, 0.034_520_861_406_762_374, 0.038_230_637_270_436_32, 0.043_030_924_411_576_68, 0.047_041_260_197_663_35, 0.050_484_493_306_801_756, 0.051_755_110_060_749_49, 0.055_220_520_341_593_93, 0.060_876_821_706_473_46, 0.957_939_087_978_319_3, 0.973_550_365_239_555_8, 0.978_134_221_752_613_3, 0.982_300_565_490_167_6, 0.990_757_447_585_685_7],
    [0.003_461_791_981_956_953_3, 0.023_470_628_927_344_788, 0.028_999_344_729_091_877, 0.031_926_468_916_274_21, 0.034_520_861_406_762_374, 0.038_230_637_270_436_32, 0.043_030_924_411_576_68, 0.047_041_260_197_663_35, 0.050_484_493_306_801_756, 0.051_755_110_060_749_49, 0.055_220_520_341_593_93, 0.060_876_821_706_473_46, 0.957_939_087_978_319_3, 0.973_550_365_239_555_8, 0.978_134_221_752_613_3, 0.982_300_565_490_167_6, 0.990_757_447_585_685_7],
    [0.003_461_791_981_956_953_3, 0.023_470_628_927_344_788, 0.028_999_344_729_091_877, 0.031_926_468_916_274_21, 0.034_520_861_406_762_374, 0.038_230_637_270_436_32, 0.043_030_924_411_576_68, 0.047_041_260_197_663_35, 0.050_484_493_306_801_756, 0.051_755_110_060_749_49, 0.055_220_520_341_593_93, 0.060_876_821_706_473_46, 0.957_939_087_978_319_3, 0.973_550_365_239_555_8, 0.978_134_221_752_613_3, 0.982_300_565_490_167_6, 0.990_757_447_585_685_7],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.003_461_791_981_956_953_3, 0.031_650_648_414_161_646, 0.040_214_047_192_837_256, 0.050_936_999_812_412_23, 0.058_479_345_741_616_11, 0.082_867_480_783_245_28, 0.143_661_527_459_626_88, 0.303_561_218_406_320_17, 0.407_768_509_354_453_1, 0.527_247_404_527_228_6, 0.651_740_465_546_703_3, 0.745_207_740_667_232_8, 0.793_668_972_265_917_3, 0.836_217_124_670_337_8, 0.872_073_701_622_577_5, 0.981_216_786_519_732_5, 1.0],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.133_110_460_242_716_4, 0.191_751_834_205_264_84, 0.235_231_297_584_444_25, 0.281_582_086_983_154_5, 0.329_732_365_603_368_5, 0.389_243_688_229_175_7, 0.441_322_863_816_429_5, 0.515_341_733_918_198_4, 0.592_650_065_882_828, 0.639_140_746_741_557_9, 0.721_808_676_417_020_6, 0.784_622_480_367_111_1, 0.871_795_938_336_012_2, 0.950_129_097_200_639_3, 0.963_853_647_151_668_7, 0.992_307_199_725_620_1],
    [0.047_564_315_952_782_32, 0.133_110_460_242_716_4, 0.191_751_834_205_264_84, 0.235_231_297_584_444_25, 0.281_582_086_983_154_5, 0.329_732_365_603_368_5, 0.389_243_688_229_175_7, 0.441_322_863_816_429_5, 0.515_341_733_918_198_4, 0.592_650_065_882_828, 0.639_140_746_741_557_9, 0.721_808_676_417_020_6, 0.784_622_480_367_111_1, 0.871_795_938_336_012_2, 0.950_129_097_200_639_3, 0.963_853_647_151_668_7, 0.992_307_199_725_620_1],
    [0.047_564_315_952_782_32, 0.133_110_460_242_716_4, 0.191_751_834_205_264_84, 0.235_231_297_584_444_25, 0.281_582_086_983_154_5, 0.329_732_365_603_368_5, 0.389_243_688_229_175_7, 0.441_322_863_816_429_5, 0.515_341_733_918_198_4, 0.592_650_065_882_828, 0.639_140_746_741_557_9, 0.721_808_676_417_020_6, 0.784_622_480_367_111_1, 0.871_795_938_336_012_2, 0.950_129_097_200_639_3, 0.963_853_647_151_668_7, 0.992_307_199_725_620_1],
    [0.047_564_315_952_782_32, 0.133_110_460_242_716_4, 0.191_751_834_205_264_84, 0.235_231_297_584_444_25, 0.281_582_086_983_154_5, 0.329_732_365_603_368_5, 0.389_243_688_229_175_7, 0.441_322_863_816_429_5, 0.515_341_733_918_198_4, 0.592_650_065_882_828, 0.639_140_746_741_557_9, 0.721_808_676_417_020_6, 0.784_622_480_367_111_1, 0.871_795_938_336_012_2, 0.950_129_097_200_639_3, 0.963_853_647_151_668_7, 0.992_307_199_725_620_1],
    [0.054_575_686_259_341_02, 0.150_892_660_142_057_24, 0.193_758_799_672_539_12, 0.244_933_284_807_910_32, 0.303_486_252_381_578_17, 0.354_370_776_728_455_35, 0.375_873_238_690_929_53, 0.435_293_369_219_344_86, 0.435_914_444_088_555_4, 0.499_229_805_675_073_63, 0.549_552_112_122_832_7, 0.618_586_845_802_665_8, 0.714_916_601_628_393_5, 0.832_094_441_496_115_4, 0.947_307_631_240_356_2, 0.974_983_919_535_823, 0.997_054_138_888_744_1],
    [0.054_575_686_259_341_02, 0.150_892_660_142_057_24, 0.193_758_799_672_539_12, 0.244_933_284_807_910_32, 0.303_486_252_381_578_17, 0.354_370_776_728_455_35, 0.375_873_238_690_929_53, 0.435_293_369_219_344_86, 0.435_914_444_088_555_4, 0.499_229_805_675_073_63, 0.549_552_112_122_832_7, 0.618_586_845_802_665_8, 0.714_916_601_628_393_5, 0.832_094_441_496_115_4, 0.947_307_631_240_356_2, 0.974_983_919_535_823, 0.997_054_138_888_744_1],
    [0.054_575_686_259_341_02, 0.150_892_660_142_057_24, 0.193_758_799_672_539_12, 0.244_933_284_807_910_32, 0.303_486_252_381_578_17, 0.354_370_776_728_455_35, 0.375_873_238_690_929_53, 0.435_293_369_219_344_86, 0.435_914_444_088_555_4, 0.499_229_805_675_073_63, 0.549_552_112_122_832_7, 0.618_586_845_802_665_8, 0.714_916_601_628_393_5, 0.832_094_441_496_115_4, 0.947_307_631_240_356_2, 0.974_983_919_535_823, 0.997_054_138_888_744_1],
    [0.054_575_686_259_341_02, 0.150_892_660_142_057_24, 0.193_758_799_672_539_12, 0.244_933_284_807_910_32, 0.303_486_252_381_578_17, 0.354_370_776_728_455_35, 0.375_873_238_690_929_53, 0.435_293_369_219_344_86, 0.435_914_444_088_555_4, 0.499_229_805_675_073_63, 0.549_552_112_122_832_7, 0.618_586_845_802_665_8, 0.714_916_601_628_393_5, 0.832_094_441_496_115_4, 0.947_307_631_240_356_2, 0.974_983_919_535_823, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.047_564_315_952_782_32, 0.136_690_998_474_194_77, 0.191_939_704_068_347_9, 0.234_856_625_172_859_93, 0.282_889_794_269_604_87, 0.331_819_635_319_778_4, 0.383_784_773_970_874_24, 0.435_428_785_985_309_54, 0.495_829_970_528_811_34, 0.569_321_941_596_233_7, 0.624_013_634_763_115_9, 0.703_263_754_533_901_2, 0.778_701_075_127_506_2, 0.869_542_330_007_294_6, 0.947_681_042_376_486_8, 0.965_109_568_217_243_7, 0.997_054_138_888_744_1],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.215_385_033_155_245_13, 0.335_024_430_525_575_57, 0.408_329_029_719_409_04, 0.459_002_528_164_725_4, 0.486_274_732_624_940_53, 0.517_087_461_162_764, 0.571_128_216_163_067_2, 0.595_301_449_604_919_8, 0.620_152_492_425_895_4, 0.639_326_762_522_980_5, 0.660_536_879_069_795_2, 0.689_731_638_987_781_4, 0.704_757_647_053_185_7, 0.715_329_984_209_455_3, 0.774_960_891_923_275_1, 0.804_361_236_080_997_1, 1.195_393_395_859_726],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.209_346_704_859_450_76, 0.325_275_752_043_464_26, 0.347_980_264_454_254, 0.382_148_249_120_370_54, 0.414_575_967_045_869_13, 0.459_241_875_190_784_2, 0.487_446_396_639_790_06, 0.522_108_801_730_294_6, 0.614_903_617_221_757, 0.663_479_153_261_628_9, 0.683_865_901_074_699_6, 0.745_710_376_485_430_1, 0.797_245_626_044_428_7, 0.840_454_593_772_114_1, 0.889_499_490_307_462_4, 0.948_432_176_119_004_4, 0.999_845_814_885_224],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.076_442_738_662_047_01, 0.135_999_064_342_072_46, 0.162_767_848_210_727_3, 0.169_681_070_045_328_76, 0.186_635_769_332_789_14, 0.223_532_982_019_512_4, 0.245_275_162_952_179_44, 0.283_514_861_837_714_57, 0.358_259_558_147_013_8, 0.433_267_971_034_979_27, 0.524_924_329_005_230_1, 0.601_578_981_440_970_8, 0.691_766_734_539_864_6, 0.824_691_601_662_147_4, 0.917_242_366_569_039_5, 0.974_470_009_820_585_4, 0.997_785_986_181_910_8],
    [0.076_442_738_662_047_01, 0.135_999_064_342_072_46, 0.162_767_848_210_727_3, 0.169_681_070_045_328_76, 0.186_635_769_332_789_14, 0.223_532_982_019_512_4, 0.245_275_162_952_179_44, 0.283_514_861_837_714_57, 0.358_259_558_147_013_8, 0.433_267_971_034_979_27, 0.524_924_329_005_230_1, 0.601_578_981_440_970_8, 0.691_766_734_539_864_6, 0.824_691_601_662_147_4, 0.917_242_366_569_039_5, 0.974_470_009_820_585_4, 0.997_785_986_181_910_8],
    [0.076_442_738_662_047_01, 0.135_999_064_342_072_46, 0.162_767_848_210_727_3, 0.169_681_070_045_328_76, 0.186_635_769_332_789_14, 0.223_532_982_019_512_4, 0.245_275_162_952_179_44, 0.283_514_861_837_714_57, 0.358_259_558_147_013_8, 0.433_267_971_034_979_27, 0.524_924_329_005_230_1, 0.601_578_981_440_970_8, 0.691_766_734_539_864_6, 0.824_691_601_662_147_4, 0.917_242_366_569_039_5, 0.974_470_009_820_585_4, 0.997_785_986_181_910_8],
    [0.076_442_738_662_047_01, 0.135_999_064_342_072_46, 0.162_767_848_210_727_3, 0.169_681_070_045_328_76, 0.186_635_769_332_789_14, 0.223_532_982_019_512_4, 0.245_275_162_952_179_44, 0.283_514_861_837_714_57, 0.358_259_558_147_013_8, 0.433_267_971_034_979_27, 0.524_924_329_005_230_1, 0.601_578_981_440_970_8, 0.691_766_734_539_864_6, 0.824_691_601_662_147_4, 0.917_242_366_569_039_5, 0.974_470_009_820_585_4, 0.997_785_986_181_910_8],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.075_515_187_375_731_4, 0.123_356_933_815_149_55, 0.152_724_717_006_305_4, 0.169_377_004_475_600_86, 0.186_333_650_364_902_8, 0.226_251_372_352_801_43, 0.250_337_063_842_967_8, 0.291_008_778_804_420_33, 0.375_979_650_378_523_27, 0.452_650_805_585_232_1, 0.560_861_136_330_789_2, 0.641_591_686_286_106_2, 0.744_136_205_956_267_3, 0.852_228_333_768_241_8, 0.957_095_318_640_676_5, 0.978_281_761_695_789_4, 0.997_832_082_603_528_2],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.05, 0.067_5, 0.085, 0.102_5, 0.12, 0.137_5, 0.155, 0.172_5, 0.19, 0.207_5, 0.225, 0.242_5, 0.26, 0.277_5, 0.295, 0.312_5, 0.33],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.099_270_034_527_347_5, 0.119_843_978_959_166_24, 0.154_453_348_602_578_87, 0.175_739_761_937_976_27, 0.210_140_646_195_160_82, 0.236_372_732_699_655_47, 0.264_077_310_432_655_14, 0.290_229_382_356_438_7, 0.313_266_225_781_920_37, 0.362_467_022_494_949_66, 0.434_733_323_187_254_44, 0.480_652_647_712_407_73, 0.562_770_232_977_173_5, 0.671_614_998_670_314_3, 0.781_940_149_739_232_5, 0.844_808_088_554_437_3, 0.933_223_993_230_396_3],
    [0.099_270_034_527_347_5, 0.119_843_978_959_166_24, 0.154_453_348_602_578_87, 0.175_739_761_937_976_27, 0.210_140_646_195_160_82, 0.236_372_732_699_655_47, 0.264_077_310_432_655_14, 0.290_229_382_356_438_7, 0.313_266_225_781_920_37, 0.362_467_022_494_949_66, 0.434_733_323_187_254_44, 0.480_652_647_712_407_73, 0.562_770_232_977_173_5, 0.671_614_998_670_314_3, 0.781_940_149_739_232_5, 0.844_808_088_554_437_3, 0.933_223_993_230_396_3],
    [0.099_270_034_527_347_5, 0.119_843_978_959_166_24, 0.154_453_348_602_578_87, 0.175_739_761_937_976_27, 0.210_140_646_195_160_82, 0.236_372_732_699_655_47, 0.264_077_310_432_655_14, 0.290_229_382_356_438_7, 0.313_266_225_781_920_37, 0.362_467_022_494_949_66, 0.434_733_323_187_254_44, 0.480_652_647_712_407_73, 0.562_770_232_977_173_5, 0.671_614_998_670_314_3, 0.781_940_149_739_232_5, 0.844_808_088_554_437_3, 0.933_223_993_230_396_3],
    [0.099_270_034_527_347_5, 0.119_843_978_959_166_24, 0.154_453_348_602_578_87, 0.175_739_761_937_976_27, 0.210_140_646_195_160_82, 0.236_372_732_699_655_47, 0.264_077_310_432_655_14, 0.290_229_382_356_438_7, 0.313_266_225_781_920_37, 0.362_467_022_494_949_66, 0.434_733_323_187_254_44, 0.480_652_647_712_407_73, 0.562_770_232_977_173_5, 0.671_614_998_670_314_3, 0.781_940_149_739_232_5, 0.844_808_088_554_437_3, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
    [0.059_237_548_465_980_37, 0.124_777_662_644_642_96, 0.153_754_985_030_150_86, 0.181_673_599_065_648_67, 0.214_202_893_487_094_1, 0.247_036_669_833_174_57, 0.277_418_692_491_548_55, 0.305_859_287_024_961_73, 0.341_299_163_682_886_4, 0.395_873_559_260_063_46, 0.454_720_818_554_144_53, 0.501_674_524_478_524_2, 0.587_170_673_652_050_7, 0.746_160_115_979_853_5, 0.810_775_924_565_774, 0.851_149_507_073_477_4, 0.933_223_993_230_396_3],
];

/// The stellar mergers' marks' medians' spans of their conditions, first and last node, in
/// [`MergerMark`]'s order: (0, 4) of the layer's index for a mark conditional on the layer.
#[rustfmt::skip]
pub const MERGER_MEDIAN_SPANS_31: [(f64, f64); 10] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 4.0),
    (0.0, 4.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 4.0),
    (0.0, 1.0),
    (0.0, 4.0),
];

/// The stellar mergers' marks' medians at their 17 nodes, in [`MergerMark`]'s order: zero.
#[rustfmt::skip]
pub const MERGER_MEDIANS_31: [[f64; 17]; 10] = [
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

/// The stellar mergers' marks' least and greatest values, in [`MergerMark`]'s order.
#[rustfmt::skip]
pub const MERGER_LIMITS_31: [(f64, f64); 10] = [
    (7.806_013_585_007_595e-5, 0.960_891_953_344_333),
    (0.0, 1.0),
    (0.06, 0.15),
    (0.003_461_791_981_956_953_3, 1.0),
    (0.047_564_315_952_782_32, 0.997_054_138_888_744_1),
    (0.215_385_033_155_245_13, 1.195_393_395_859_726),
    (0.209_346_704_859_450_76, 0.999_845_814_885_224),
    (0.075_515_187_375_731_4, 0.997_832_082_603_528_2),
    (0.05, 0.33),
    (0.059_237_548_465_980_37, 0.933_223_993_230_396_3),
];

/// The stellar mergers' layer choice's weights, A–E, at its fixed point.
#[rustfmt::skip]
pub const MERGER_LAYERS_31: [f64; 5] = [
    0.0,
    0.0,
    0.005_620_732_138_468_386,
    0.003_082_253_384_115_856_4,
    0.991_297_014_477_415_7,
];

/// The stellar mergers' channel choice's weights, in [`MergerChannel::ALL`]'s order.
#[rustfmt::skip]
pub const MERGER_CHANNELS_31: [f64; 3] = [
    0.030_218_584_368_216_204,
    0.934_817_534_139_945_3,
    0.034_963_881_491_838_635,
];

/// The stellar mergers' mergers in the source horizon per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const MERGER_HOSTS_PER_FORMED_MASS_31: [f64; 7] = [
    5.014_616_849_198_658e-5,
    1.251_100_976_628_642_6e-6,
    3.449_976_924_914_607e-7,
    2.794_984_815_961_885e-7,
    4.007_456_154_438_342_7e-7,
    9.065_128_820_370_28e-7,
    3.044_771_096_161_315_3e-7,
];

/// The share of the stellar mergers' mergers whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const MERGER_LAYER_SHARES_31: [f64; 5] = [
    0.007_659_308_613_443_169,
    0.019_155_632_111_034_535,
    0.588_894_178_383_808,
    0.336_293_558_152_327_3,
    0.047_997_322_739_386_9,
];
// @end-table binary_merger

// @begin-table binary_nsm
// The neutron-star mergers' sampler and share row fitted against plan 11's engine at
// generator version 31 on the generator's own pairs (plan 15, P15.T10.b), held until 31.
//
// @generated by hyperion-fit 0.1.0, task `binary_nsm` revision 0. Do not edit.
// inputs-sha256: dd4bf064d56f60bb7b3bcb1f22693b3d19b75551a6594ba2686063ec67ac85c9
// manifest: crates/hyperion-fit/manifests/binary_nsm.toml
// data: none
// sim-fingerprint: 75623164a473ada74b64af316c40ad55dde418172e36ebe47156a10fb92d7555
// since-generator-version: 29
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the manifest's generator version's
//   rules; the total against the brainstorm's ten neutron-star mergers in the source horizon
// acceptance: 2910 members, Kish 579; pair kinds 0.537/0.463; of the weight, two black holes
//   0.000, a neutron star's progenitor outside 8–20 M☉ 0.640, a black hole's outside 25–60 M☉
//   0.331; layers 0.000/0.000/0.000/0.073/0.927 rates a system
//   1.51e-12/1.95e-14/9.39e-16/2.72e-15/4.52e-15/1.61e-14/1.90e-15; scratch marks: none; the K–S
//   tests against the other half of the census are the sim's slow
//   `merger_tables::the_neutron_star_merger_sampler_against_the_engine`
//
// Read by `NSM_SAMPLER_31` and the class shares' rows from 31, after the block. The neutron-star
// mergers' marks are fitted to the engine's own mergers at generator version 31's rules through the
// sim's merger census of the pairs the generator draws at each record's first attempt
// (hyperion-fit's `tasks::binary_classes::mergers`), each member weighted by its age hazard at its
// merger age: each mark's weighted quantiles at the ranks k ÷ 16, unconditional, the progenitors'
// ranks over the sim's fixed ranges held at their ends. A mark with a Kish size under 50 keeps the
// scratch sampler's law. The row is the members' weighted rate a record by population and band,
// scaled to 10 mergers in the source horizon at Milky Way parameters (plan 11's design note 11).
// The censuses ran 9500192 records.

/// The neutron-star mergers' marks' quantiles at the ranks k ÷ 16 over all their members, in
/// [`NsmMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const NSM_RESIDUALS_31: [[f64; 17]; 5] = [
    [1.144_145_717_024_521, 1.184_553_942_679_596_1, 1.193_853_979_783_817, 1.201_290_608_344_867_1, 1.208_423_996_254_099_7, 1.218_488_392_323_061_2, 1.231_272_192_050_359, 1.26, 1.271_510_556_069_683_3, 1.365_408_793_445_443_7, 1.443_860_558_649_407, 1.482_899_287_600_254_2, 1.524_211_834_327_373_6, 1.577_447_548_710_855, 1.639_375_047_032_811, 1.748_177_260_351_680_3, 1.996_227_077_284_708_8],
    [0.309_348_056_861_503_1, 0.435_327_302_997_150_06, 0.496_939_652_886_086_77, 0.545_909_754_628_509_3, 0.575_409_025_702_795_6, 0.610_371_995_027_482_5, 0.635_873_036_434_792, 0.657_606_315_971_001_2, 0.677_041_635_698_787_1, 0.695_164_433_507_983_4, 0.710_400_787_884_120_2, 0.733_478_375_022_028_7, 0.758_634_090_613_296_8, 0.790_233_464_116_350_5, 0.833_157_077_740_73, 0.888_958_310_136_671_8, 1.016_458_110_871_263_1],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.006_877_368_812_318_088, 0.078_119_210_489_339_87, 0.143_339_048_540_705_7, 0.237_688_214_556_672_6, 0.314_724_193_984_300_9, 0.423_824_948_814_407_6, 0.511_740_550_606_176_8, 0.615_568_153_139_269_5, 0.892_660_822_875_259_2, 1.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.131_292_370_133_644_25, 0.284_736_158_559_780_6, 0.530_925_528_851_500_1, 0.790_216_774_871_783_2, 1.0],
    [2.116_493_659_178_683e-5, 0.015_496_403_360_858_4, 0.034_863_105_234_060_65, 0.056_098_122_128_843_92, 0.088_746_459_444_773_76, 0.126_229_779_130_784_23, 0.167_969_963_880_996_58, 0.210_050_685_646_732_15, 0.257_215_858_531_305_24, 0.324_138_021_067_937_54, 0.398_433_864_134_207_67, 0.475_555_636_301_815_8, 0.549_233_013_478_664_5, 0.637_349_974_391_680_7, 0.780_632_969_452_254_9, 0.866_764_190_238_191_6, 0.979_255_903_395_388_6],
];

/// The neutron-star mergers' marks' least and greatest values, in [`NsmMark`]'s order.
#[rustfmt::skip]
pub const NSM_LIMITS_31: [(f64, f64); 5] = [
    (1.144_145_717_024_521, 1.996_227_077_284_708_8),
    (0.309_348_056_861_503_1, 1.016_458_110_871_263_1),
    (0.0, 1.0),
    (0.0, 1.0),
    (2.116_493_659_178_683e-5, 0.979_255_903_395_388_6),
];

/// The neutron-star mergers' pair kind choice's weights, in [`NsmPairKind::ALL`]'s order.
#[rustfmt::skip]
pub const NSM_PAIR_KINDS_31: [f64; 2] = [
    0.537_260_620_300_041_4,
    0.462_739_379_699_958_6,
];

/// The neutron-star mergers' mergers in the source horizon per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const NSM_HOSTS_PER_FORMED_MASS_31: [f64; 7] = [
    1.013_770_859_413_531_5e-8,
    1.316_294_075_765_936_8e-10,
    6.322_111_989_644_501e-12,
    1.835_319_206_621_047_7e-11,
    3.046_440_677_178_834e-11,
    1.082_388_380_584_697_3e-10,
    1.277_486_792_991_387_7e-11,
];

/// The share of the neutron-star mergers' mergers whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const NSM_LAYER_SHARES_31: [f64; 5] = [
    0.0,
    0.0,
    0.0,
    0.072_965_436_494_830_2,
    0.927_034_563_505_169_8,
];
// @end-table binary_nsm

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;

    /// P11.T6 and P15.T9.b: the fitted yield is 1 in every delay bin and for both channels, the
    /// scratch's value, so that no explosion mark moves (a refit that leaves a bin below 1 would
    /// move the grid's redraws and be held to its version, as plan 15's record says); and a delay
    /// takes the bin of `DELAY_EDGES` that holds it, the ends held.
    #[test]
    fn the_fitted_ia_yield_is_the_scratch_s_one_everywhere() {
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
