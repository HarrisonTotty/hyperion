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
//! **Scratch.** Every population makes the same hosts per solar mass formed, scaled so that the
//! Milky Way fixture's formed mass ([`MILKY_WAY_FORMED_MASS`]) gives the counts above, and each
//! class's layers are a guess from the stars that make it: an accreting white dwarf's primary is
//! its white dwarf's progenitor, 0.75–8 M☉; an X-ray binary's and a neutron-star merger's is the
//! first compact star's, 8 M☉ and up; a stellar merger's is spread over the layers with most of it
//! in C, where the contact binaries of W Ursae Majoris type lie. Plan 15's P15.T10.b fits them
//! against the engine.
//!
//! [`ClassSamplerTable`] (P11.T8) is a catalogue class's conditional laws of the marks that define
//! it now and of its history, which plan 11's `stellar::binary::sampler::ClassSampler` draws by
//! inverse transform. Each mark is a residual given by its quantiles at the 17 ranks k ÷ 16, as
//! plan 15's `PRIMARY_MASS_CDF` gives its laws, added to a median that follows a condition on a
//! 17-point grid. [`AWD_SAMPLER`] is the accreting white dwarfs' and [`XRB_SAMPLER`] the X-ray
//! binaries': hand-made scratch from the observed figures each mark's doc names (Design note 13),
//! which P15.T10.b refits against the engine's own members; [`MERGER_SAMPLER`] is the stellar
//! mergers' (P11.T8.c) and [`NSM_SAMPLER`] the neutron-star mergers' (P11.T8.d).

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

/// The scratch share rows (P11.T7; module documentation): 4.5 × 10⁶ fast and 4.5 × 10⁶ slow
/// accreting white dwarfs, 10⁴ X-ray binaries, 9 × 10⁴ stellar mergers and 10 neutron-star
/// mergers in the source horizon at Milky Way parameters.
pub const CLASS_SHARES: ClassShareTable = ClassShareTable {
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
    #[must_use]
    pub const fn with_node_residuals(self, rows: &'static [[f64; 17]; 17]) -> Self {
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
/// mark list is fixed per class: [`AwdMark`] and [`AwdPick`] for [`AWD_SAMPLER`].
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

/// The marks of [`AWD_SAMPLER`], in its order (P11.T8.a): what an accreting white dwarf is now,
/// then its history.
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
    /// Every mark, in [`AWD_SAMPLER`]'s order.
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

    /// The mark's place in [`AWD_SAMPLER`]'s marks.
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

/// The choices of [`AWD_SAMPLER`], in its order (P11.T8.a).
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

/// The donor kinds of [`AWD_SAMPLER`]'s one choice, in its order (P11.T8.a).
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

/// A median flat at `value` over the condition's span `(first, last)`.
const fn flat(first: f64, last: f64, value: f64) -> MedianCurve {
    MedianCurve {
        first,
        last,
        nodes: [value; 17],
    }
}

/// A median rising linearly from `at_first` to `at_last` over the condition's span.
const fn linear(first: f64, last: f64, at_first: f64, at_last: f64) -> MedianCurve {
    let mut nodes = [0.0; 17];
    let mut k = 0;
    while k < 17 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a node number of 0–16, exact in f64"
        )]
        let t = k as f64 / 16.0;
        nodes[k] = at_first + (at_last - at_first) * t;
        k += 1;
    }
    MedianCurve { first, last, nodes }
}

/// The scratch sampler of the two accreting white dwarf classes (P11.T8.a; Design notes 10, 12
/// and 13): [`AwdMark`]'s eleven marks and [`AwdPick`]'s one choice, the donor's kind
/// ([`AwdDonorKind`]).
///
/// Hand-made from observed figures, each recalled and not read at source in this build unless its
/// line says otherwise, and the same in every population; P15.T10.b refits it against the
/// engine's members (plan 11's `brute_force_class_members`):
///
/// - **Recurrence**, log₁₀ years: ranks 0 to ½ from 28,000 years to L (the fast hosts), ½ to 1
///   from L to 10⁸ years (the slow), so that the median is L exactly and the two classes take
///   halves, as [`CLASS_SHARES`] gives them. Classical novae recur every 10⁴–10⁵ years; at 9 × 10⁶
///   hosts the law gives 49 eruptions a year, inside the brainstorm's 30–50. It holds no recurrent
///   nova (under a century), which a 17-rank law cannot resolve at their few hundred hosts.
/// - **White dwarf mass**: 0.70 + 0.45 exp(−(x − 4.45) ÷ 0.8) M☉ at log₁₀ recurrence x, ±0.10
///   (one standard deviation, truncated at 2.5), on 0.55–1.36 M☉. Fast novae have the heavier
///   dwarfs; cataclysmic variables' white dwarfs average 0.83 ± 0.23 M☉ (Zorotovic, Schreiber and
///   Gänsicke 2011, A&A 536, A42).
/// - **Periods** by donor kind, given the transfer rate y = log₁₀ Ṁ: main-sequence donors after the
///   standard track (Knigge, Baraffe and Patterson 2011, ApJS 194, 28): 0.12 dex in hours (80
///   minutes) up to y = −11, rising to 2.1 hours at y = −9.7, across the period gap of 2.15–3.18
///   hours by y = −9.5 and to 5 hours at y = −8.5, ±0.03 dex; helium donors from an hour at
///   y = −12 to 12 minutes at y = −8.5, ±0.05 dex, held to AM Canum Venaticorum stars' 5.4–65.6
///   minutes (Ramsay et al. 2018; Solheim 2010, PASP 122, 1133); giants flat at 600 days, ±0.15
///   dex, the peak of the symbiotic stars' periods (Mikołajewska 2012, Baltic Astronomy 21, 5).
/// - **Donor masses** given log₁₀ of the period in hours, p: main-sequence donors on Warner's
///   (1995, Cataclysmic Variable Stars, equation 2.100) M₂ = 0.065 P^(5/4), ±0.03 dex; helium
///   donors 10^(−1 − 2(p + ½)) M☉, 0.1 M☉ at 19 minutes and 0.01 at an hour, ±0.05 dex; giants
///   1.3 M☉, ±0.08 dex.
/// - **The primary's initial mass** given the white dwarf's: Cummings et al.'s (2018, ApJ 866, 21)
///   initial–final mass relation (their PARSEC fit, three lines from 0.87 to 8.2 M☉) inverted,
///   held to 0.95–7.9 M☉ (layers C and D), ±0.03 dex.
/// - **The donor's initial mass** given its mass now: through 1.4 M☉ for the helium donors' 0.005
///   M☉, 0.63 M☉ for a donor of 0.1 M☉ now, 1.15 for one of 1 M☉, ±0.06 dex.
/// - **The age**: 10^(10.3 − 2.5 log₁₀ m₀) years, about twice the main-sequence lifetime of the
///   primary's m₀, at most 10^10.1, ±0.12 dex.
/// - **The donor's kind**: 0.907 main-sequence, 0.09 helium, 0.003 giant: AM Canum Venaticorum
///   stars' space density, 5 × 10⁻⁷ pc⁻³ (Carter et al. 2013, MNRAS 429, 2143, as Ramsay et al.
///   2018, A&A 620, A141, quote it), against Pala et al.'s (2020) 4.8 × 10⁻⁶ pc⁻³ for
///   cataclysmic variables, and a few times 10⁴ symbiotic stars.
pub const AWD_SAMPLER: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_MARKS,
    &[SamplerPick::new("donor_kind", &[0.907, 0.09, 0.003])],
);

/// log₁₀ L in years: 18 log₁₀ 2.
const LOG_L_YEARS: f64 = 5.418_539_921_951_662;

/// The span of the transfer rate's log₁₀ the periods' medians follow, M☉ yr⁻¹.
const RATE_SPAN: (f64, f64) = (-12.0, -8.5);

/// [`AWD_SAMPLER`]'s marks, in [`AwdMark`]'s order.
#[rustfmt::skip]
const AWD_MARKS: [SamplerMark; 11] = [
    SamplerMark {
        name: "log_recurrence_years",
        residual: [
            4.45, 4.75, 4.92, 5.05, 5.15, 5.23, 5.30, 5.36, LOG_L_YEARS,
            5.62, 5.84, 6.08, 6.34, 6.62, 6.94, 7.34, 8.0,
        ],
        median: None,
        node_residuals: None,
        limits: (4.45, 8.0),
    },
    SamplerMark {
        name: "white_dwarf_mass",
        residual: normal(0.10),
        median: Some(MedianCurve {
            first: 4.45,
            last: 8.0,
            nodes: [
                1.150_000, 1.041_007, 0.958_413, 0.895_824, 0.848_394, 0.812_452, 0.785_216,
                0.764_576, 0.748_935, 0.737_083, 0.728_101, 0.721_295, 0.716_137, 0.712_229,
                0.709_267, 0.707_022, 0.705_321,
            ],
        }),
        node_residuals: None,
        limits: (0.55, 1.36),
    },
    SamplerMark {
        name: "log_main_sequence_period_hours",
        residual: normal(0.03),
        median: Some(MedianCurve {
            first: RATE_SPAN.0,
            last: RATE_SPAN.1,
            nodes: [
                0.120_000, 0.120_000, 0.120_000, 0.120_000, 0.120_000, 0.129_375, 0.151_250,
                0.173_125, 0.195_000, 0.243_875, 0.300_750, 0.420_312, 0.525_000, 0.568_750,
                0.612_500, 0.656_250, 0.700_000,
            ],
        }),
        node_residuals: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "log_helium_period_hours",
        residual: normal(0.05),
        median: Some(linear(RATE_SPAN.0, RATE_SPAN.1, 0.0, -0.7)),
        // 5.4 and 65.6 minutes.
        node_residuals: None,
        limits: (-1.045_757, 0.038_753),
    },
    SamplerMark {
        name: "log_giant_period_hours",
        residual: normal(0.15),
        // 600 days.
        median: Some(flat(RATE_SPAN.0, RATE_SPAN.1, 4.158_362)),
        node_residuals: None,
        limits: (3.5, 5.0),
    },
    SamplerMark {
        name: "log_main_sequence_donor_mass",
        residual: normal(0.03),
        // log₁₀ 0.065 + 1.25 p.
        median: Some(linear(0.0, 0.8, -1.187_087, -0.187_087)),
        node_residuals: None,
        limits: (-1.301_030, -0.022_276),
    },
    SamplerMark {
        name: "log_helium_donor_mass",
        residual: normal(0.05),
        median: Some(linear(-1.0, 0.1, 0.0, -2.2)),
        node_residuals: None,
        limits: (-2.301_030, -0.698_970),
    },
    SamplerMark {
        name: "log_giant_donor_mass",
        residual: normal(0.08),
        // 1.3 M☉.
        median: Some(flat(3.5, 5.0, 0.113_943)),
        node_residuals: None,
        limits: (-0.096_910, 0.477_121),
    },
    SamplerMark {
        name: "log_primary_initial_mass",
        residual: normal(0.03),
        median: Some(MedianCurve {
            first: 0.55,
            last: 1.36,
            nodes: [
                -0.022_276, 0.154_591, 0.302_644, 0.412_854, 0.476_721, 0.515_468, 0.551_039,
                0.608_993, 0.669_378, 0.722_383, 0.769_616, 0.812_213, 0.851_002, 0.886_609,
                0.897_627, 0.897_627, 0.897_627,
            ],
        }),
        node_residuals: None,
        limits: (-0.022_276, 0.897_627),
    },
    SamplerMark {
        name: "log_donor_initial_mass",
        residual: normal(0.06),
        median: Some(MedianCurve {
            first: -2.3,
            last: 0.5,
            nodes: [
                0.150_000, 0.117_188, 0.084_375, 0.051_562, 0.018_750, -0.030_000, -0.100_000,
                -0.170_000, -0.176_000, -0.134_000, -0.092_000, -0.045_000, 0.004_000, 0.053_000,
                0.198_000, 0.359_000, 0.520_000,
            ],
        }),
        node_residuals: None,
        limits: (-1.096_910, 0.897_627),
    },
    SamplerMark {
        name: "log_age_years",
        residual: normal(0.12),
        median: Some(MedianCurve {
            first: -0.022_276,
            last: 0.897_627,
            nodes: [
                10.100_000, 10.100_000, 10.068_221, 9.924_486, 9.780_751, 9.637_016, 9.493_281,
                9.349_547, 9.205_812, 9.062_077, 8.918_342, 8.774_607, 8.630_872, 8.487_137,
                8.343_402, 8.199_667, 8.055_932,
            ],
        }),
        node_residuals: None,
        limits: (7.5, 10.12),
    },
];

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
    /// The mass ratio q = M₂ ÷ M₁ of the pair before the merger, unconditional: version 25's and
    /// before, every merger a contact pair.
    MassRatio,
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
    pub const ALL: [Self; 11] = [
        Self::AgeRank,
        Self::PrimaryRank,
        Self::MassRatio,
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
    /// The channel among [`MergerChannel::HELD`]: version 26's.
    Channel,
    /// The channel among [`MergerChannel::ALL`]: version 27's, with the dynamical main sequence.
    EngineChannel,
}

impl SamplerKey for MergerPick {
    fn index(self) -> usize {
        match self {
            Self::Layer => 0,
            Self::Channel => 1,
            Self::EngineChannel => 2,
        }
    }
}

/// The channels of [`MERGER_SAMPLER`]'s channel choices (P11.T8.c's fix, versions 26 and 27): what
/// a stellar merger is before its merger time.
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

    /// Version 26's channels, in [`MergerPick::Channel`]'s order. The bump to 27 deletes them with
    /// that choice.
    pub const HELD: [Self; 2] = [Self::Contact, Self::EvolvedDonor];
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
/// - **The mass ratio** of the contact pair before the merger: 0.06–1, median 0.25. W Ursae
///   Majoris pairs hold mass ratios from about 0.07 to 1 (Rucinski 1993, PASP 105, 1433), and the
///   pairs that merge are the low ones, below Rasio's (1995, ApJ 444, L41) tidal limit near 0.09
///   and just above it: V1309 Scorpii's progenitor, the contact pair whose merger was seen in
///   2008 (Tylenda et al. 2011, A&A 528, A114), held a mass ratio near 0.1 (Stępień, Pamyatnykh
///   and Różyczka 2011, A&A 531, A18).
/// - **The layer**: [`CLASS_SHARES`]'s row for the stellar mergers, A 0.1, B 0.2, C 0.5, D 0.15,
///   E 0.05, restricted to the layers with stars on their main sequence at the merger.
///
/// Version 26's channels (P11.T8.c's fix; research-leans-13 §2(c)) add three marks and a choice:
///
/// - **The channel**: 0.15 contact, 0.85 evolved donor, scratch. Luminous red novae whose
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
        SamplerPick::new("channel", &[0.15, 0.85]),
        SamplerPick::new("engine_channel", &[0.15, 0.50, 0.35]),
    ],
);

/// [`MERGER_SAMPLER`]'s marks, in [`MergerMark`]'s order.
#[rustfmt::skip]
const MERGER_MARKS: [SamplerMark; 11] = [
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
        name: "mass_ratio",
        residual: [
            0.06, 0.08, 0.10, 0.12, 0.14, 0.16, 0.19, 0.22, 0.25,
            0.29, 0.33, 0.38, 0.44, 0.52, 0.62, 0.76, 1.0,
        ],
        median: None,
        node_residuals: None,
        limits: (0.06, 1.0),
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

/// The kinds of the X-ray binaries' class (P11.T8.b), in [`XRB_SAMPLER`]'s two choices' order:
/// the four low-mass kinds of the old populations, then the two high-mass kinds of the young disc,
/// then the three the carving names (P11.T8.b's fix, drawn from 27 by plan 15's fitted tables): two
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
    /// Every kind: the six of [`XRB_SAMPLER`] and P15.T10.b's fit at 26, then the carving's three.
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

    /// The low-mass kinds [`XRB_SAMPLER`]'s choice weighs and P15.T10.b's fit at 26 fitted, the
    /// first of [`LOW_MASS`](Self::LOW_MASS).
    pub const LOW_MASS_HELD: [Self; 4] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
    ];

    /// The high-mass kinds [`XRB_SAMPLER`]'s choice weighs and P15.T10.b's fit at 26 fitted, the
    /// first of [`HIGH_MASS`](Self::HIGH_MASS).
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
    /// [`XrbHostKind::HIGH_MASS_HELD`] hold, the first twenty: those P15.T10.b's fit at 26 fitted.
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

/// The scratch sampler of the X-ray binaries' class (P11.T8.b; Design notes 10 and 13):
/// [`XrbMark`]'s thirty marks and [`XrbPick`]'s two kind choices.
///
/// Hand-made from observed figures, each recalled and not read at source in this build unless its
/// line says otherwise, and the same in every population; P15.T10.b refits it against the engine's
/// members (plan 11's `brute_force_class_members`). Each unconditional mark marked "±" is a normal
/// of the given mean and standard deviation, truncated at 2.5 and held to its limits.
///
/// - **The kinds**, as shares of all hosts: a neutron star's low-mass X-ray binaries 0.01
///   persistent and 0.699 transient, a black hole's 0.001 and 0.13, so that the black-hole
///   transients are the brainstorm's 1,300 of 10⁴ (Corral-Santana et al. 2016, A&A 587, A61) and
///   the persistent ones the Galaxy's hundred or so bright sources (Grimm, Gilfanov and Sunyaev
///   2002, A&A 391, 923); Be/X-ray binaries 0.15 and supergiant ones 0.01, some hundred against
///   the forty to fifty known, most of the high-mass systems being Be/X (Reig 2011, Ap&SS 332, 1).
///   Each choice's weights are its side's shares; the high-mass side's sum, 0.16, is the young
///   disc's share of the hosts.
/// - **Accretors**: neutron stars 1.45 ± 0.15 M☉ on 1.2–2.2 (Özel and Freire 2016, ARA&A 54,
///   401); black holes 7.8 ± 1.2 on 5–15 (Özel et al. 2010, ApJ 725, 1918).
/// - **Low-mass systems**: periods by a hand-made law from 2 hours to 10.5 days, median 8.3
///   hours, with a tail of evolved donors to days (the spread of Ritter and Kolb's 2003, A&A 404,
///   301, catalogue); donors on the Roche-lobe-filling main sequence's M₂ ≈ 0.11 P (hours) M☉
///   (Frank, King and Raine 2002, Accretion Power in Astrophysics, section 4.6) up to 0.8 M☉, at
///   7.3 hours, and evolved donors of 0.8 M☉ beyond, ±0.08 dex, on 0.1–1.5 M☉; the rate over the
///   irradiated disc's line from 3 dex below it to 1 above, its median on the line, so that each
///   side takes its own kinds by rank, and the Eddington rate held (`stellar::binary::scan`);
///   transients recurring every 10^(3.0 ± 0.5) years with a neutron star and 10^(2.5 ± 0.5)
///   with a black hole, some 14 and 8 outbursts a year over the Galaxy, against the 130 or so
///   black-hole outbursts of 1996–2015 (Tetarenko et al. 2016, ApJS 222, 15) that the sky
///   monitors saw.
/// - **Be/X-ray binaries**: periods 10^(1.8 ± 0.35) days on 12.6–355 d, eccentricities 0.35 ±
///   0.15 on 0–0.85 (Reig 2011's 12.7–330 d and 0.03–0.9), Be stars of 14 ± 2.5 M☉ on 9–19,
///   B0–B2 (Negueruela 1998, A&A 338, 505); 10^(6.8 ± 0.25) years after the supernova, on 0.3–20
///   Myr, within the Be star's main sequence.
/// - **Supergiant X-ray binaries**, marked in Roche-lobe overflow: periods 10^(0.6 ± 0.3) days on
///   1.3–16 d, donors of 20 ± 4 M☉ on 10–35, transfer 10^(−9 ± 0.5) M☉ yr⁻¹, 10³⁶–10³⁸ erg s⁻¹,
///   the Eddington rate held; 10^(6.2 ± 0.2) years after the supernova on 0.3–5 Myr.
/// - **Systemic speeds**: Maxwellians whose mean projected speed is van den Heuvel et al.'s (2000,
///   A&A 364, 563) mean tangential speed, 15 ± 6 km s⁻¹ for Be/X and 42 ± 14 for supergiant
///   systems, σ the mean tangential speed ÷ √(π ÷ 2): 12.0 km s⁻¹ (plan 09's
///   `PAIR_SYSTEMIC_SIGMA`, ruling 126.3) and 33.5, mean speeds of 19.1 and 53.5 km s⁻¹, held to
///   their 99.5% points, 43 and 120.
/// - **History**: a neutron star's progenitor of 10^(1.08 ± 0.08) M☉ on 9–17 and a black hole's
///   of 10^(1.48 ± 0.1) on 21–60, the ranges in which plan 06's single stars leave that remnant
///   at [Fe/H] −0.5 to +0.3 with median draws (measured for P11.T8.b); a low-mass donor's initial
///   mass 10^(0.15 ± 0.1) times its mass now, on 1–4 times.
///
/// The three kinds the carving names (P11.T8.b's fix) have marks of their own, `@provisional`,
/// for P15.T10.b's refit at 27; the choices weigh the first six kinds alone, so no host of this
/// table draws them, and version 27's table ([`XRB_SAMPLER_27`]) weighs all nine:
///
/// - **Ultracompact X-ray binaries**: periods 10^(1.5 ± 0.16) minutes on 10–80, median 32 (the
///   known systems lie at 11–60 minutes: in 't Zand, Jonker and Markwardt 2007, A&A 465, 953,
///   table 1); the donor a helium white dwarf filling its Roche lobe, its mass from the period
///   and the cold white dwarf's mass–radius relation, and its rate driven by gravitational
///   radiation (`stellar::binary::scan`); a transient's outbursts recurring every 10^(1.0 ± 0.4)
///   years, ours, the few seen in outburst more than once doing so after some years; and the
///   donor's progenitor of 10^(0.15 ± 0.08) M☉ on 0.9–2.3, ours, a low-mass giant whose core it
///   is.
/// - **Symbiotic X-ray binaries**, marked detached and fed by their giant's wind: periods 10^(3.0
///   ± 0.25) days on 150–5,000, the measured ones running from IGR J16194−2810's 192.7 d to 4U
///   1700+24's 4,391 d (Hinkle et al. 2019, ApJ 872, 43; research's ruling of 2026-11-02) about
///   GX 1+4's 1,161 d (Hinkle et al. 2006, ApJ 641, 479); giants of 10^(0.176 ± 0.07) M☉, 1.5
///   M☉, on 1–3; and the giant's radius 0.45 ± 0.12 of its Roche lobe's on 0.15–0.75, ours,
///   inside the carving's detached 0.8 (`stellar::binary::xrb_class`), at the accreting white
///   dwarfs' giant donors' 3,600 K (`awd`) and no larger than the red giant branch's tip there
///   (`catalogue_classes::binary`'s 129 R☉), so that its luminosity and its Reimers wind follow.
/// - **Black-hole high-mass X-ray binaries**, marked in Roche-lobe overflow as the supergiant
///   systems are, with their recoils and years since the supernova: Cyg X-1's black hole, 21.2 ±
///   2.2 M☉ on 15–30, its period, 10^(0.748 ± 0.1) days on 2–10 about its 5.6 d, and its O
///   supergiant, 40.6 ± 7.4 M☉ on 20–60 (Miller-Jones et al. 2021, Science 371, 1046), fed at
///   10^(−8.5 ± 0.4) M☉ yr⁻¹, ours, about Cyg X-1's 10³⁷ erg s⁻¹.
///
/// Every figure is recalled, not read at source in this build.
pub const XRB_SAMPLER: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    &XRB_MARKS,
    &[
        SamplerPick::new("low_mass_kind", &[0.01, 0.699, 0.001, 0.13]),
        SamplerPick::new("high_mass_kind", &[0.15, 0.01]),
    ],
);

/// The quantiles of a Maxwellian of σ = 1 at the ranks k ÷ 16, the last its 99.5% point: speeds
/// of three normal components (the χ distribution of three degrees of freedom).
const MAXWELL_RANKS: [f64; 17] = [
    0.0, 0.642_815, 0.832_080, 0.976_476, 1.101_151, 1.215_477, 1.324_379, 1.431_091, 1.538_172,
    1.648_056, 1.763_470, 1.887_973, 2.026_905, 2.189_572, 2.395_707, 2.704_675, 3.583_037,
];

/// [`MAXWELL_RANKS`] scaled to `sigma`.
const fn maxwell(sigma: f64) -> [f64; 17] {
    let mut out = [0.0; 17];
    let mut k = 0;
    while k < 17 {
        out[k] = MAXWELL_RANKS[k] * sigma;
        k += 1;
    }
    out
}

/// The Maxwellian σ of a Be/X-ray binary's systemic speed, km s⁻¹: 12.0, van den Heuvel et al.'s
/// mean tangential 15 ÷ √(π ÷ 2), plan 09's `PAIR_SYSTEMIC_SIGMA` (ruling 126.3).
const BEX_RECOIL_SIGMA: f64 = 12.0;

/// The Maxwellian σ of a supergiant X-ray binary's systemic speed, km s⁻¹: van den Heuvel et
/// al.'s mean tangential 42 ÷ √(π ÷ 2).
const SUPERGIANT_RECOIL_SIGMA: f64 = 33.511_152;

/// [`XRB_SAMPLER`]'s marks, in [`XrbMark`]'s order.
#[rustfmt::skip]
const XRB_MARKS: [SamplerMark; 30] = [
    SamplerMark::new("neutron_star_mass", normal_about(1.45, 0.15), None, (1.2, 2.2)),
    SamplerMark::new("black_hole_mass", normal_about(7.8, 1.2), None, (5.0, 15.0)),
    SamplerMark::new(
        "log_low_mass_period_hours",
        [
            0.3, 0.45, 0.55, 0.62, 0.68, 0.74, 0.80, 0.86, 0.92,
            0.99, 1.07, 1.16, 1.27, 1.40, 1.58, 1.85, 2.4,
        ],
        None,
        (0.3, 2.4),
    ),
    SamplerMark::new(
        "log_low_mass_donor_mass",
        normal(0.08),
        // log₁₀ 0.11 + p to the fifth node, then log₁₀ 0.8, on nodes 0.13125 apart in p.
        Some(MedianCurve::new(
            0.3,
            2.4,
            [
                -0.658_607, -0.527_357, -0.396_107, -0.264_857, -0.133_607, -0.096_910,
                -0.096_910, -0.096_910, -0.096_910, -0.096_910, -0.096_910, -0.096_910,
                -0.096_910, -0.096_910, -0.096_910, -0.096_910, -0.096_910,
            ],
        )),
        (-1.0, 0.176_091),
    ),
    SamplerMark::new(
        "low_mass_rate_excess_dex",
        [
            -3.0, -2.4, -2.0, -1.7, -1.4, -1.1, -0.8, -0.4, 0.0,
            0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8, 1.0,
        ],
        None,
        (-3.0, 1.0),
    ),
    SamplerMark::new("log_neutron_star_recurrence_years", normal_about(3.0, 0.5), None, (1.75, 4.25)),
    SamplerMark::new("log_black_hole_recurrence_years", normal_about(2.5, 0.5), None, (1.25, 3.75)),
    SamplerMark::new("log_bex_period_days", normal_about(1.8, 0.35), None, (1.1, 2.55)),
    SamplerMark::new("bex_eccentricity", normal_about(0.35, 0.15), None, (0.0, 0.85)),
    SamplerMark::new("bex_donor_mass", normal_about(14.0, 2.5), None, (9.0, 19.0)),
    SamplerMark::new("log_supergiant_period_days", normal_about(0.6, 0.3), None, (0.1, 1.2)),
    SamplerMark::new("supergiant_donor_mass", normal_about(20.0, 4.0), None, (10.0, 35.0)),
    SamplerMark::new("log_supergiant_rate", normal_about(-9.0, 0.5), None, (-10.25, -7.75)),
    SamplerMark::new("bex_recoil_km_s", maxwell(BEX_RECOIL_SIGMA), None, (0.0, 43.0)),
    SamplerMark::new(
        "supergiant_recoil_km_s",
        maxwell(SUPERGIANT_RECOIL_SIGMA),
        None,
        (0.0, 120.1),
    ),
    SamplerMark::new("log_bex_years_since_supernova", normal_about(6.8, 0.25), None, (5.5, 7.3)),
    SamplerMark::new(
        "log_supergiant_years_since_supernova",
        normal_about(6.2, 0.2),
        None,
        (5.5, 6.7),
    ),
    SamplerMark::new(
        "log_neutron_star_progenitor_mass",
        normal_about(1.08, 0.08),
        None,
        (0.954_243, 1.230_449),
    ),
    SamplerMark::new(
        "log_black_hole_progenitor_mass",
        normal_about(1.48, 0.1),
        None,
        (1.322_219, 1.778_151),
    ),
    SamplerMark::new("log_donor_initial_excess", normal_about(0.15, 0.1), None, (0.0, 0.6)),
    SamplerMark::new(
        "log_ultracompact_period_minutes",
        normal_about(1.5, 0.16),
        None,
        // 10 and 80 minutes.
        (1.0, 1.903_090),
    ),
    SamplerMark::new(
        "log_ultracompact_recurrence_years",
        normal_about(1.0, 0.4),
        None,
        (0.0, 2.0),
    ),
    SamplerMark::new(
        "log_ultracompact_donor_initial_mass",
        normal_about(0.15, 0.08),
        None,
        (-0.045_757, 0.361_728),
    ),
    SamplerMark::new(
        "log_symbiotic_period_days",
        normal_about(3.0, 0.25),
        None,
        // 150 and 5,000 days.
        (2.176_091, 3.698_970),
    ),
    SamplerMark::new(
        "log_symbiotic_donor_mass",
        normal_about(0.176_091, 0.07),
        None,
        (0.0, 0.477_121),
    ),
    SamplerMark::new("symbiotic_filling", normal_about(0.45, 0.12), None, (0.15, 0.75)),
    SamplerMark::new(
        "black_hole_high_mass_accretor_mass",
        normal_about(21.2, 2.2),
        None,
        (15.0, 30.0),
    ),
    SamplerMark::new(
        "log_black_hole_high_mass_period_days",
        normal_about(0.748_188, 0.1),
        None,
        // 2 and 10 days.
        (std::f64::consts::LOG10_2, 1.0),
    ),
    SamplerMark::new(
        "black_hole_high_mass_donor_mass",
        normal_about(40.6, 7.4),
        None,
        (20.0, 60.0),
    ),
    SamplerMark::new(
        "log_black_hole_high_mass_rate",
        normal_about(-8.5, 0.4),
        None,
        (-9.5, -7.5),
    ),
];

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

/// An accreting white dwarf sampler's marks from a fitted block's arrays, in [`AwdMark`]'s order:
/// each mark's residual quantiles, its median's span and nodes, and its limits. The recurrence is
/// unconditional, so its median's row is not read.
const fn fitted_awd_marks(
    residuals: &[[f64; 17]; 11],
    spans: &[(f64, f64); 11],
    medians: &[[f64; 17]; 11],
    limits: &[(f64, f64); 11],
) -> [SamplerMark; 11] {
    let mut out = [SamplerMark::new("", [0.0; 17], None, (0.0, 0.0)); 11];
    let mut k = 0;
    while k < 11 {
        let median = if k == AwdMark::Recurrence.index() {
            None
        } else {
            Some(MedianCurve::new(spans[k].0, spans[k].1, medians[k]))
        };
        out[k] = SamplerMark::new(AWD_MARK_NAMES[k], residuals[k], median, limits[k]);
        k += 1;
    }
    out
}

/// The fast accreting white dwarfs' fitted marks (P15.T10.b), from task `binary_awd`'s block.
const AWD_FAST_MARKS_26: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_FAST_RESIDUALS_26,
    &AWD_FAST_MEDIAN_SPANS_26,
    &AWD_FAST_MEDIANS_26,
    &AWD_FAST_LIMITS_26,
);

/// The slow accreting white dwarfs' fitted marks (P15.T10.b), from task `binary_awd`'s block.
const AWD_SLOW_MARKS_26: [SamplerMark; 11] = fitted_awd_marks(
    &AWD_SLOW_RESIDUALS_26,
    &AWD_SLOW_MEDIAN_SPANS_26,
    &AWD_SLOW_MEDIANS_26,
    &AWD_SLOW_LIMITS_26,
);

/// The fast accreting white dwarfs' sampler fitted against the engine (P15.T10.b), held until
/// generator version 27 (`catalogue_classes::binary`'s hold): [`AwdMark`]'s marks and the donor's
/// kind, each fitted to the engine's fast members alone, so that its recurrence lies all within L
/// and its every rank is the fast hosts'. Where [`AWD_SAMPLER`] serves both classes, split at L,
/// each class has its own: the engine's two sides differ in their donors and rates, and the fast
/// hosts are an eighth of the members, too few of a shared law's 17 ranks.
pub const AWD_FAST_SAMPLER_26: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_FAST_MARKS_26,
    &[SamplerPick::new("donor_kind", &AWD_FAST_DONOR_KINDS_26)],
);

/// The slow accreting white dwarfs' sampler fitted against the engine (P15.T10.b), as
/// [`AWD_FAST_SAMPLER_26`] is the fast ones', its recurrence all beyond L.
pub const AWD_SLOW_SAMPLER_26: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_SLOW_MARKS_26,
    &[SamplerPick::new("donor_kind", &AWD_SLOW_DONOR_KINDS_26)],
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

/// The X-ray binaries' fitted marks (P15.T10.b) from task `binary_xrb`'s block, in [`XrbMark`]'s
/// order: each of [`XrbMark::HELD`]'s marks' residual quantiles, its median's span and nodes, and
/// its limits, then the three new kinds' marks as [`XRB_SAMPLER`] has them, which the fit at 26
/// did not fit. Only the low-mass donor's mass follows a condition, the period; the other marks'
/// median rows are not read.
const fn fitted_xrb_marks(
    residuals: &[[f64; 17]; 20],
    spans: &[(f64, f64); 20],
    medians: &[[f64; 17]; 20],
    limits: &[(f64, f64); 20],
) -> [SamplerMark; 30] {
    let mut out = XRB_MARKS;
    let mut k = 0;
    while k < 20 {
        let median = if k == XrbMark::LowMassDonorMass.index() {
            Some(MedianCurve::new(spans[k].0, spans[k].1, medians[k]))
        } else {
            None
        };
        out[k] = SamplerMark::new(XRB_MARK_NAMES[k], residuals[k], median, limits[k]);
        k += 1;
    }
    out
}

/// The X-ray binaries' fitted marks.
const XRB_MARKS_26: [SamplerMark; 30] = fitted_xrb_marks(
    &XRB_RESIDUALS_26,
    &XRB_MEDIAN_SPANS_26,
    &XRB_MEDIANS_26,
    &XRB_LIMITS_26,
);

/// The X-ray binaries' sampler fitted against the engine (P15.T10.b), held until generator
/// version 27 (`catalogue_classes::binary`'s hold): [`XrbMark`]'s marks and the two kind choices,
/// fitted to the engine's members of the kinds [`XrbHostKind`] holds, each mark no member defines
/// kept as [`XRB_SAMPLER`]'s (task `binary_xrb`'s block says which).
pub const XRB_SAMPLER_26: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    &XRB_MARKS_26,
    &[
        SamplerPick::new("low_mass_kind", &XRB_LOW_MASS_KINDS_26),
        SamplerPick::new("high_mass_kind", &XRB_HIGH_MASS_KINDS_26),
    ],
);

/// The X-ray binaries' sampler from generator version 27 (`catalogue_classes::binary`'s hold):
/// [`XRB_SAMPLER_26`]'s marks, and its two kind choices over all nine kinds, each kind weighted by
/// the galaxy's share of the engine's members of it at 27 ([`XRB_LOW_MASS_KINDS_27`] and
/// [`XRB_HIGH_MASS_KINDS_27`]), so that the kinds the class draws are those its row counts
/// (P11.T8.b's fix). P15.T10.b's refit at 27 replaces it.
pub const XRB_SAMPLER_27: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    &XRB_MARKS_26,
    &[
        SamplerPick::new("low_mass_kind", &XRB_LOW_MASS_KINDS_27),
        SamplerPick::new("high_mass_kind", &XRB_HIGH_MASS_KINDS_27),
    ],
);

/// The low-mass kinds' weights from 27, in [`XrbHostKind::LOW_MASS`]' order: the galaxy's share of
/// the X-ray binaries of each kind, every population's members of the kind a record, its bands
/// weighted by their shares of the systems, times its systems, as task `binary_xrb` weighs its
/// six (`kind_shares`), over all nine kinds, in that task's censuses at 27 (measured for P11.T8.b's
/// fix, 2026-11-02, under every 27 hold with the symbiotic X-ray binaries' bounds and the helium
/// disc's line): the carving's three are 0.381 of the hosts, the high-mass side 0.0078, and black
/// holes 0.0017 of its hosts. The black-hole transients' weight alone is not the census's: plan
/// 11's design note 11 counts what is observed, so it is pinned to [`BLACK_HOLE_TRANSIENT_SHARE`]
/// and the other kinds keep the census's ratios in the rest.
pub const XRB_LOW_MASS_KINDS_27: [f64; 6] = [
    0.004_885_460_559_566_987,
    0.519_014_655_492_588_2,
    0.001_550_711_461_735_681_6,
    BLACK_HOLE_TRANSIENT_SHARE,
    0.336_742_531_699_908_75,
    0.000_911_999_675_146_566_8,
];

/// The high-mass kinds' weights from 27, in [`XrbHostKind::HIGH_MASS`]' order, as
/// [`XRB_LOW_MASS_KINDS_27`]'s: with them they sum to one.
pub const XRB_HIGH_MASS_KINDS_27: [f64; 3] = [
    0.006_834_948_420_718_55,
    4.775_415_226_834_502_5e-5,
    1.193_853_806_708_625_6e-5,
];

/// The black-hole transients' share of the X-ray binaries from 27: 0.13, the 1,300 of the row's
/// 10⁴ hosts that P11.T8.b's design takes, Corral-Santana et al.'s (2016, A&A 587, A61; recalled,
/// not read at source) 1,280 ± 120, which they call a lower limit (research's ruling of
/// 2026-11-02). The census gives 0.0196, about 190 (a finding: likely the common envelopes that
/// survive at `α_CE λ` = 0.5, Podsiadlowski et al. 2003, MNRAS 341, 385, recalled; neither
/// `α_CE λ` nor the kicks are touched, which would move every class after a common envelope).
pub const BLACK_HOLE_TRANSIENT_SHARE: f64 = 0.13;

/// The share rows fitted against the engine (P15.T10.b), held until generator version 27: the
/// accreting white dwarfs' rows from task `binary_awd`'s block and the X-ray binaries' from task
/// `binary_xrb`'s, the two merger classes' [`CLASS_SHARES`]' scratch until their own fits.
pub const CLASS_SHARES_26: ClassShareTable = ClassShareTable {
    rows: [
        ClassShareRow {
            class: CarvedClass::AccretingWdFast,
            hosts_per_formed_mass: AWD_FAST_HOSTS_PER_FORMED_MASS_26,
            layer_shares: AWD_FAST_LAYER_SHARES_26,
        },
        ClassShareRow {
            class: CarvedClass::AccretingWdSlow,
            hosts_per_formed_mass: AWD_SLOW_HOSTS_PER_FORMED_MASS_26,
            layer_shares: AWD_SLOW_LAYER_SHARES_26,
        },
        ClassShareRow {
            class: CarvedClass::XrayBinary,
            hosts_per_formed_mass: XRB_HOSTS_PER_FORMED_MASS_26,
            layer_shares: XRB_LAYER_SHARES_26,
        },
        CLASS_SHARES.rows[3],
        CLASS_SHARES.rows[4],
    ],
};

// @begin-table binary_awd
// The accreting white dwarfs' samplers and share rows fitted against plan 11's engine (plan 15,
// P15.T10.b), held until generator version 27 (`catalogue_classes::binary`'s hold).
//
// @generated by hyperion-fit 0.1.0, task `binary_awd` revision 0. Do not edit.
// inputs-sha256: fd589d224a26cbd7ae0d30563d4917dc8409e1817efb5db5ee17a0cbbc3a1470
// manifest: crates/hyperion-fit/manifests/binary_awd.toml
// data: none
// sim-fingerprint: f40a2b56ffc37e3c7ce0f8f9b6626524c07800636f73baa24dcd02db3b67074b
// since-generator-version: 26
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the generator version's rules; the total
//   against Pala et al. 2020, MNRAS 494, 3799, and the brainstorm's 6–12 × 10⁶
// acceptance: fast: 1843 members, donors 0.954/0.029/0.016, layers C 0.503 D 0.485 E 0.012,
//   rates a system 2.80e-6/3.99e-5/2.01e-5/2.03e-5/3.16e-5/2.88e-5/1.33e-5, not its own:
//   GiantPeriod Pooled, GiantDonorMass Pooled; slow: 15381 members, donors 0.305/0.693/0.002,
//   layers C 0.388 D 0.598 E 0.014, rates a system
//   7.01e-7/3.16e-4/4.76e-4/3.29e-4/3.28e-4/3.03e-4/6.69e-4, not its own: GiantPeriod Pooled,
//   GiantDonorMass Pooled; the K–S tests against the other half of the census are the sim's slow
//   `tables_hold::the_accreting_white_dwarf_samplers_against_the_engine`
//
// Read by `AWD_FAST_SAMPLER_26`, `AWD_SLOW_SAMPLER_26` and `CLASS_SHARES_26`, after the block. Each
// class's marks are fitted to its own members of the engine near the solar circle (hyperion-fit's
// `tasks::binary_classes`): a median that follows the mark's condition on 17 nodes, the weighted
// median under a triangle two node spacings wide, and the residual's quantiles about it. A mark
// with under 50 members of its class takes both classes' members, and with under 50 of
// both keeps `AWD_SAMPLER`'s law. A slow host's recurrence is held at 10^10 years, under the
// universe clock's reach; the engine's longest, beyond it, never erupt in the Galaxy's age. The
// rows' rates are each population's members a record, its bands weighted by their shares of the
// systems, scaled together to 9e6 hosts at Milky Way parameters (plan 11's design note 11).
// The slab's census ran 10000064 records and the populations' own 1500320.

/// The fast accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16, in
/// [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_FAST_RESIDUALS_26: [[f64; 17]; 11] = [
    [1.331_694_800_827_338_2, 3.932_829_683_788_521_3, 4.308_880_142_488_699, 4.434_357_504_197_2, 4.511_445_958_933_510_5, 4.581_131_152_431_687, 4.632_009_633_660_145, 4.695_195_220_494_205, 4.762_107_688_172_762, 4.817_481_119_302_665, 4.892_600_141_534_31, 4.984_311_401_257_621, 5.066_935_732_424_488, 5.158_099_067_607_736, 5.242_913_254_999_218, 5.327_818_077_503_453, 5.418_038_116_770_516],
    [-0.618_471_296_186_515_3, -0.270_405_697_215_757_13, -0.216_728_191_822_999_9, -0.193_893_247_127_448_26, -0.165_643_395_614_737_32, -0.121_555_744_976_364_26, -0.049_422_403_329_454_634, -0.020_340_735_025_448_67, 0.005_358_555_291_831_046, 0.036_425_250_040_436_05, 0.074_915_821_870_324, 0.118_027_935_467_362_48, 0.177_937_698_063_757_03, 0.283_080_450_217_273_1, 0.439_339_873_516_949_25, 0.616_809_420_328_816, 0.878_008_995_521_328_5],
    [-1.794_040_434_790_728_2, -0.786_839_630_819_834, -0.629_546_288_393_138_3, -0.344_937_249_022_556_2, -0.137_795_688_625_878_1, -0.082_551_256_100_286_57, -0.051_927_828_986_186_864, -0.024_594_484_627_409_624, -9.776_234_657_343_785e-5, 0.029_066_260_929_757_17, 0.065_654_650_336_489_4, 0.119_204_288_333_059_07, 0.190_329_506_509_799_85, 0.278_845_557_589_415_4, 0.406_078_700_093_090_44, 0.569_725_534_692_298, 1.219_076_102_127_421_7],
    [-0.244_351_034_149_241_86, -0.058_952_908_809_067_18, -0.049_417_256_320_481_11, -0.024_470_659_978_809_827, -0.015_636_274_300_414_678, -0.011_120_335_745_112_715, -0.005_988_773_170_888_57, -0.000_616_292_994_537_885_3, 0.003_805_794_467_871_34, 0.009_561_540_975_226_556, 0.014_146_002_200_771_53, 0.022_805_070_857_869, 0.034_760_497_114_758_745, 0.101_579_079_442_398_51, 0.224_280_933_972_149_3, 0.343_799_411_003_951_2, 0.422_154_370_120_465_2],
    [-0.263_315_387_914_948_13, -0.094_139_527_946_850_82, -0.074_922_736_499_690_95, -0.052_323_231_925_307_91, -0.036_930_520_948_999_4, -0.020_097_214_919_661_766, -0.010_869_827_413_611_644, 0.003_309_440_254_936_957_3, 0.028_144_352_580_083_076, 0.036_929_527_731_517_53, 0.045_255_798_419_427_06, 0.057_346_612_180_899_315, 0.076_288_297_139_223_8, 0.085_261_006_680_711_02, 0.124_786_368_298_377_53, 3.506_450_210_325_785, 4.526_400_534_420_892],
    [-0.840_347_961_357_067_1, -0.241_778_546_635_102_48, -0.171_893_747_997_523_16, -0.121_437_898_439_828_36, -0.087_294_250_042_134_37, -0.053_346_909_246_882_485, -0.036_278_270_381_743_17, -0.022_309_276_157_947_016, -0.007_682_759_136_542_969, 0.007_453_726_613_028_809, 0.024_135_201_402_662_433, 0.038_633_269_410_595_62, 0.061_087_083_604_946_78, 0.114_540_401_635_890_16, 0.221_832_708_212_385_6, 0.308_421_190_751_440_26, 0.668_452_805_846_638_9],
    [-0.073_026_464_815_935_33, -0.013_920_417_955_880_018, -0.012_269_959_837_040_767, -0.010_131_160_367_251_105, -0.009_229_527_410_245_13, -0.009_104_108_068_320_488, -0.008_327_702_906_073_875, -0.007_442_349_598_443_199, -0.005_944_231_662_898_147, -0.004_683_290_521_414_585, 0.001_508_125_640_084_512_2, 0.024_222_087_489_202_584, 0.041_287_707_389_257_02, 0.928_730_313_055_438_5, 1.317_854_735_440_816, 1.597_597_266_810_268_7, 1.760_617_375_067_639_7],
    [-0.070_721_858_249_888_41, -0.048_384_641_556_374_98, -0.038_636_538_668_807_396, -0.030_085_885_362_151_565, -0.021_156_917_334_756_553, -0.009_867_805_522_866_951, -0.008_227_903_423_746_963, 0.002_065_838_873_565_734, 0.025_695_262_798_501_872, 0.041_326_616_169_598_87, 0.061_151_818_354_968_024, 0.083_193_287_031_041_51, 0.141_253_348_779_971_1, 0.205_008_090_586_746_3, 0.344_763_180_355_313_17, 0.708_694_235_051_415_2, 1.237_171_700_342_286_9],
    [-0.481_097_897_025_249_47, -0.072_198_227_549_803_08, -0.051_058_403_335_366_6, -0.038_547_198_352_089_77, -0.027_988_318_742_794_56, -0.018_620_012_535_144_65, -0.009_739_446_303_419_289, -0.003_236_693_780_111_578_3, 0.001_766_754_376_821_166_9, 0.008_195_852_599_593_684, 0.014_589_623_645_804_267, 0.022_634_073_844_238_392, 0.029_527_661_809_850_128, 0.037_683_806_100_114_39, 0.048_375_424_216_293_73, 0.068_686_001_586_910_09, 0.206_277_240_363_225_03],
    [-1.115_425_143_262_584_1, -0.144_926_356_358_857_48, -0.093_339_917_892_454_27, -0.066_412_856_346_828_77, -0.041_419_903_668_248_56, -0.023_883_689_065_868_74, -0.011_380_523_498_838_443, -0.001_226_375_267_366_236_8, 0.009_952_781_688_706_794, 0.020_503_171_112_231_947, 0.030_303_477_217_881_493, 0.040_117_880_072_771_59, 0.050_895_203_194_178_014, 0.066_484_606_962_277_05, 0.082_131_814_748_777_5, 0.110_949_655_139_354_74, 0.866_910_217_421_142_7],
    [-1.423_580_065_629_813_9, -0.688_020_546_690_672_6, -0.507_041_825_472_574_4, -0.367_454_244_720_147_65, -0.239_886_307_810_765_6, -0.167_610_283_826_464_6, -0.095_146_617_049_138_18, -0.043_043_365_797_089_894, 0.000_291_146_976_278_966_46, 0.034_173_210_726_365_92, 0.070_305_255_996_427_01, 0.109_020_240_359_063_44, 0.156_006_965_558_095_35, 0.208_114_208_819_341_28, 0.262_489_525_061_913_5, 0.341_852_512_956_490_2, 0.980_063_812_307_175_7],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_26: [(f64, f64); 11] = [
    (0.0, 1.0),
    (2.851_889_181_205_528_3, 5.410_933_945_490_725),
    (-10.081_411_016_745_863, -8.088_743_443_538_037),
    (-11.134_797_184_918_488, -7.291_705_615_992_596),
    (-15.607_091_732_637_805, -6.990_827_454_534_971),
    (-0.356_215_450_078_690_15, 1.774_299_485_710_101),
    (-0.764_628_966_066_836_4, -0.070_423_912_577_623_51),
    (1.797_402_047_517_482_1, 6.345_607_756_390_914),
    (0.315_064_703_523_772_3, 1.374_443_469_368_853_4),
    (-1.700_171_024_201_246_2, 0.449_685_974_780_775_65),
    (0.029_357_591_606_110_63, 0.892_904_455_045_598_3),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_26: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.258_746_416_414_341_2, 1.208_993_501_802_191_3, 1.164_554_384_808_319_7, 1.136_381_036_236_205_7, 1.120_597_469_469_109, 1.079_728_082_728_254_3, 0.938_206_179_863_876_3, 0.903_396_016_185_623_4, 0.776_991_952_190_924_8, 0.658_138_307_535_400_6, 0.576_043_443_336_290_9, 0.557_385_729_247_285_5, 0.557_668_475_919_986_7, 0.572_506_508_738_875, 0.616_457_423_982_207_6, 0.689_401_945_910_775, 0.741_625_933_045_308_8],
    [1.046_605_573_410_899_2, 1.046_605_573_410_899_2, 0.487_567_123_714_336_2, 0.255_329_044_227_845_25, 0.261_296_744_993_464_17, 0.816_729_210_061_283_2, 0.951_790_853_738_399_3, 0.898_362_358_846_013_7, 0.847_425_823_700_097_7, 0.898_362_358_846_013_7, 1.307_592_415_336_286_6, 1.380_787_154_874_103_5, 1.421_610_363_173_095, 1.442_839_228_299_828_1, 1.466_884_539_474_371_7, 1.500_938_221_215_858_2, 1.546_619_204_720_16],
    [-0.149_007_067_753_287_65, -0.149_007_067_753_287_65, -0.174_389_252_854_908_14, -0.209_736_138_189_710_1, -0.267_395_022_584_776_2, -0.325_053_906_979_842_27, -0.382_712_791_374_908_43, -0.440_371_675_769_974_5, -0.498_030_560_165_040_6, -0.555_689_444_560_106_7, -0.564_495_497_232_711_3, -0.564_495_497_232_711_3, -0.564_495_497_232_711_3, -0.564_495_497_232_711_3, -0.564_495_497_232_711_3, -0.564_495_497_232_711_3, -0.564_495_497_232_711_3],
    [2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.033_277_953_597_997_5, 1.994_781_640_604_350_6, 1.972_967_831_795_460_9, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4],
    [-1.345_118_517_916_638_3, -1.284_549_356_257_325, -1.058_229_758_009_384_5, -0.936_254_418_936_948, -0.786_547_446_160_343_3, -0.657_589_795_608_545_8, -0.607_039_820_185_596_7, -0.119_545_012_868_314_75, -0.097_371_375_359_949_58, -0.083_421_687_251_380_05, -0.063_474_271_455_257_16, 0.021_933_724_602_955_607, 0.037_202_110_149_914_085, 0.031_882_132_304_553_23, -0.000_816_286_568_329_289_8, -0.095_818_565_804_078_88, -0.285_476_298_421_287_7],
    [-1.319_836_646_654_52, -1.319_836_646_654_52, -1.319_836_646_654_52, -1.319_836_646_654_52, -1.319_836_646_654_52, -1.346_448_777_286_494_6, -1.389_459_867_637_545_8, -1.432_470_957_988_597, -1.475_482_048_339_648_2, -1.518_493_138_690_699_5, -1.561_504_229_041_750_7, -1.604_515_319_392_802, -1.647_526_409_743_853_1, -1.696_137_971_323_681, -1.739_715_674_692_677_6, -1.785_384_943_782_708, -1.785_384_943_782_708],
    [-0.726_222_168_705_241_7, -0.758_764_321_922_509_3, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2],
    [0.110_345_133_278_083_48, 0.150_863_752_890_701_1, 0.207_968_625_600_848_8, 0.317_136_896_251_156_76, 0.365_588_366_816_305_3, 0.440_753_707_372_276_6, 0.506_928_174_549_283_3, 0.557_507_036_138_266_7, 0.621_703_985_857_760_2, 0.721_502_671_799_126_7, 0.739_995_684_362_479, 0.754_539_206_188_911, 0.781_703_333_345_019_4, 0.817_513_166_605_399_5, 0.848_793_639_716_063_4, 0.862_730_864_638_714_4, 0.863_769_707_504_811_1],
    [0.111_577_204_929_774_94, -0.591_860_261_451_380_5, -0.553_777_201_176_520_7, -0.429_478_634_895_642_4, -0.189_559_927_663_544_32, -0.106_850_608_422_772_1, -0.067_934_284_074_635_78, -0.060_323_509_258_546_154, -0.034_425_331_437_608_38, 0.026_979_518_551_615_202, 0.038_243_400_237_313_55, 0.032_868_928_480_947_93, 0.038_193_792_866_247_75, 0.071_018_939_978_883_27, 0.162_836_289_018_236, 0.303_107_298_865_441, 0.347_104_569_016_668_53],
    [9.929_180_776_524_246, 9.879_698_958_739_867, 9.815_579_968_055_543, 9.755_356_728_833_531, 9.744_726_334_761_2, 9.723_928_533_482_333, 9.711_453_693_656_646, 9.674_180_799_722_658, 9.630_231_281_471_21, 9.565_106_213_430_106, 9.411_655_597_413_52, 9.065_232_790_419_229, 9.109_045_265_901_827, 9.496_344_318_678_705, 9.646_630_221_732_025, 9.646_630_221_732_025, 9.566_906_362_758_1],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_26: [(f64, f64); 11] = [
    (1.331_694_800_827_338_2, 5.418_038_116_770_516),
    (0.296_601_635_211_186_04, 1.439_741_072_723_463),
    (-0.445_071_966_194_821_8, 1.948_347_714_303_122_7),
    (-0.800_359_105_831_161_3, -0.048_793_491_161_041_785),
    (1.721_077_999_357_347_7, 6.540_111_231_557_497),
    (-1.932_978_601_950_638, 0.621_099_788_498_994_5),
    (-1.858_411_408_598_643_4, 0.005_261_985_907_671_135),
    (-0.816_682_623_392_157_5, 0.470_837_257_299_106_5),
    (-0.049_996_140_421_199_75, 0.921_778_823_803_262_7),
    (-1.044_577_771_600_633_5, 0.632_446_824_184_782_3),
    (8.120_636_962_353_265, 10.078_479_482_311_554),
];

/// The fast accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_DONOR_KINDS_26: [f64; 3] = [
    0.954_422_137_818_773_7,
    0.029_300_054_259_359_74,
    0.016_277_807_921_866_522,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16, in
/// [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_26: [[f64; 17]; 11] = [
    [5.419_163_063_092_123, 5.949_855_809_361_852, 6.289_874_782_445_002, 6.431_355_986_697_191, 6.639_819_153_871_911, 6.918_832_404_977_375, 7.405_955_328_944_763_5, 7.770_499_288_226_854, 8.039_271_605_945_942, 8.262_022_319_372_54, 8.425_444_302_914_517, 8.557_503_307_388_993, 8.667_495_622_253_673, 8.781_487_063_008_752, 8.905_611_513_504_745, 9.112_820_886_976_946, 10.0],
    [-0.916_519_910_686_120_7, -0.337_052_974_019_504_24, -0.232_632_684_205_171_03, -0.183_899_616_130_118_18, -0.139_937_032_182_249_18, -0.093_696_960_157_976_16, -0.050_398_698_041_823_01, -0.021_577_131_475_529_893, 0.004_135_161_994_517_689, 0.030_678_007_419_306_458, 0.058_297_959_372_430_93, 0.093_118_354_384_554_1, 0.135_627_623_256_133_53, 0.186_551_620_726_252_3, 0.263_738_139_895_716_43, 0.420_343_307_781_766_76, 0.887_254_896_121_948_9],
    [-1.273_133_097_534_774, -0.714_626_686_993_414_7, -0.587_194_877_078_799_5, -0.458_549_487_946_320_6, -0.314_138_718_254_015_6, -0.191_352_027_668_463_15, -0.105_969_773_959_239_51, -0.047_716_433_162_064_84, 0.003_502_232_122_226_500_7, 0.077_249_248_986_592_74, 0.171_362_593_140_107_1, 0.303_011_910_958_987_83, 0.428_442_020_256_394_2, 0.569_738_485_227_491_3, 0.721_145_562_685_401_2, 0.837_546_343_651_573_9, 1.555_526_987_778_847_6],
    [-0.294_867_423_116_183_75, -0.039_836_526_653_430_876, -0.033_092_889_302_442_68, -0.026_116_942_951_776_69, -0.022_210_750_102_072_99, -0.018_377_901_538_941_616, -0.014_265_229_933_299_844, -0.010_343_659_100_948_144, -0.006_687_156_446_685_105, -0.002_373_736_421_709_127_2, 0.001_910_619_842_764_305, 0.006_457_922_582_356_779, 0.012_236_284_159_831_329, 0.019_737_508_065_180_126, 0.029_705_388_826_637_258, 0.045_181_404_718_300_29, 1.245_977_268_631_288_2],
    [-0.263_315_387_914_948_13, -0.094_139_527_946_850_82, -0.074_922_736_499_690_95, -0.052_323_231_925_307_91, -0.036_930_520_948_999_4, -0.020_097_214_919_661_766, -0.010_869_827_413_611_644, 0.003_309_440_254_936_957_3, 0.028_144_352_580_083_076, 0.036_929_527_731_517_53, 0.045_255_798_419_427_06, 0.057_346_612_180_899_315, 0.076_288_297_139_223_8, 0.085_261_006_680_711_02, 0.124_786_368_298_377_53, 3.506_450_210_325_785, 4.526_400_534_420_892],
    [-1.023_045_079_822_086_3, -0.084_939_772_515_755_9, -0.059_984_213_080_972_04, -0.046_802_829_567_330_26, -0.036_914_355_905_728_56, -0.028_951_259_454_302_228, -0.020_540_190_604_775_937, -0.011_795_382_279_848_027, -0.002_068_869_336_401_713, 0.006_605_990_765_328_244, 0.014_480_550_866_149_217, 0.023_450_020_574_913_61, 0.032_167_238_153_367_49, 0.042_660_417_789_363_525, 0.058_524_935_867_337_17, 0.087_544_528_787_235_62, 0.367_180_652_469_636_94],
    [-1.235_652_946_129_061_7, -0.038_435_766_204_136_41, -0.026_727_582_251_843_685, -0.018_687_550_034_984_735, -0.013_221_782_432_139_007, -0.007_907_838_939_725_076, -0.002_791_665_582_894_342_6, 0.000_581_973_591_093_354_3, 0.004_546_744_503_175_493, 0.012_633_831_478_098_434, 0.018_270_418_589_868_997, 0.021_833_264_458_454_626, 0.024_845_476_798_328_16, 0.026_800_507_941_201_48, 0.028_404_633_571_368_63, 0.030_147_098_369_170_677, 2.278_949_379_724_64],
    [-0.070_721_858_249_888_41, -0.048_384_641_556_374_98, -0.038_636_538_668_807_396, -0.030_085_885_362_151_565, -0.021_156_917_334_756_553, -0.009_867_805_522_866_951, -0.008_227_903_423_746_963, 0.002_065_838_873_565_734, 0.025_695_262_798_501_872, 0.041_326_616_169_598_87, 0.061_151_818_354_968_024, 0.083_193_287_031_041_51, 0.141_253_348_779_971_1, 0.205_008_090_586_746_3, 0.344_763_180_355_313_17, 0.708_694_235_051_415_2, 1.237_171_700_342_286_9],
    [-0.447_468_133_757_033_54, -0.240_172_017_458_680_41, -0.125_537_308_047_606_88, -0.074_185_422_491_894_46, -0.050_188_098_653_859_87, -0.034_709_144_165_525_35, -0.022_331_964_197_186_25, -0.010_513_438_973_168_587, 0.001_282_273_902_122_377_2, 0.013_838_288_725_774_32, 0.027_652_749_073_098_698, 0.041_844_704_414_287_115, 0.058_038_378_400_230_96, 0.078_051_059_654_827_6, 0.109_638_895_817_522_2, 0.156_606_330_297_561_96, 0.335_162_384_672_501_87],
    [-1.025_708_037_653_249_5, -0.190_391_744_109_265_3, -0.157_593_573_701_295_56, -0.125_117_824_618_344_2, -0.092_526_698_171_949_86, -0.064_281_062_937_360_81, -0.041_131_191_174_500_59, -0.019_798_065_227_640_8, -0.000_696_469_801_883_314_9, 0.014_350_848_363_090_168, 0.026_963_590_169_987_478, 0.039_418_731_863_917_28, 0.053_144_575_698_921_17, 0.070_491_841_992_633, 0.091_965_680_008_924_83, 0.127_086_405_701_14, 0.716_561_799_357_486_3],
    [-1.800_575_872_516_429_1, -0.562_943_521_996_445_4, -0.400_798_658_839_602_4, -0.294_931_993_802_690_4, -0.213_398_336_594_536_89, -0.142_697_427_813_242_68, -0.085_822_035_714_174_13, -0.036_215_176_060_037_1, 0.001_564_765_650_002_186_7, 0.035_137_530_754_658_64, 0.067_765_211_185_085_05, 0.099_685_362_711_766_87, 0.127_965_666_492_787_73, 0.154_819_168_593_727_24, 0.181_170_737_192_251_96, 0.221_755_017_661_526_4, 0.325_213_814_193_132_2],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_26: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.524_771_722_187_626_5, 10.0),
    (-12.294_794_752_547_707, -9.290_107_199_089_842),
    (-15.460_946_611_435_89, -10.364_129_264_282_4),
    (-15.607_091_732_637_805, -6.990_827_454_534_971),
    (-0.223_603_909_198_159_34, 1.481_972_084_305_881_5),
    (-0.246_755_869_406_395_77, 0.890_737_224_611_108_4),
    (1.797_402_047_517_482_1, 6.345_607_756_390_914),
    (0.342_371_122_101_844_1, 1.431_253_832_147_911_4),
    (-2.757_324_063_564_704_3, 0.216_197_004_490_049_8),
    (0.050_891_510_204_642_49, 0.847_750_805_793_929_5),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_26: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.693_351_761_799_490_2, 0.660_613_939_710_433_9, 0.599_414_282_801_537_2, 0.566_465_752_455_383_4, 0.556_827_468_461_406, 0.609_030_981_865_887_6, 1.164_493_054_886_916_6, 1.256_443_620_654_596_6, 1.143_002_641_339_376_6, 1.042_053_250_407_614_8, 1.008_625_880_371_406_8, 0.987_020_593_619_949_6, 0.950_799_995_451_478_2, 0.934_471_834_132_984_3, 1.224_526_762_183_121, 1.281_552_428_068_529_8, 1.195_582_957_289_605_3],
    [1.026_289_252_150_916_6, 0.965_818_031_055_495_2, 0.818_135_829_428_525_1, 0.979_623_039_351_004_8, 1.004_921_070_498_419_7, 0.859_083_156_805_720_9, 0.434_924_162_413_743_5, 0.394_945_833_461_276_04, 0.783_480_534_059_937_8, 1.045_502_746_778_982_4, 1.040_680_569_170_813_9, 0.939_893_636_052_624_7, 0.340_972_182_062_364_8, 0.357_675_733_067_330_5, 0.920_052_992_127_213_7, 0.962_949_286_116_264, 0.938_371_506_497_090_4],
    [0.817_994_864_683_650_6, 0.783_893_431_048_380_9, 0.725_303_674_656_167, 0.657_081_270_914_339_5, 0.588_778_312_185_968_4, 0.518_886_222_235_692_7, 0.442_750_607_870_943_3, 0.327_192_644_890_664_63, 0.247_333_864_349_222_22, 0.225_920_489_118_087_38, 0.185_375_996_451_864_28, 0.120_235_231_371_469_33, 0.049_434_332_666_486_225, -0.018_232_816_959_811_45, -0.086_905_264_885_704_28, -0.153_865_526_414_073_06, -0.212_828_452_960_525_52],
    [2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.033_277_953_597_997_5, 1.994_781_640_604_350_6, 1.972_967_831_795_460_9, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4],
    [-1.115_486_847_743_225_8, -1.063_777_109_994_182_2, -1.005_387_639_787_810_8, -0.908_174_763_342_067_9, -0.799_728_317_068_581_4, -0.705_352_569_009_190_7, -0.641_668_155_356_967_6, -0.565_480_379_634_724_5, -0.144_459_603_769_214_68, -0.011_204_438_053_065_697, 0.005_543_745_197_256_17, 0.025_655_274_347_510_547, 0.053_540_480_731_351_32, 0.083_053_728_054_793_54, 0.126_097_509_727_551_96, 0.171_944_234_087_565_65, 0.202_567_089_897_522_4],
    [-1.692_702_216_405_885_8, -1.753_779_411_332_402_8, -1.823_950_448_673_336_2, -1.896_816_973_335_971_2, -1.970_661_852_715_790_4, -2.040_465_163_566_618_7, -2.102_305_389_812_355, -2.138_220_300_500_465_4, -2.162_394_804_011_352, -2.273_220_910_831_967, -2.368_577_159_966_249, -2.446_524_090_936_093, -2.522_338_520_469_707_3, -2.588_672_310_888_685, -2.659_387_229_136_226, -2.708_809_901_797_23, -2.750_525_119_200_107],
    [-0.726_222_168_705_241_7, -0.758_764_321_922_509_3, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2],
    [0.136_173_241_362_341_63, 0.171_487_335_574_583_48, 0.270_174_150_203_047_3, 0.334_914_782_858_288_5, 0.381_714_177_859_834, 0.421_973_392_121_580_4, 0.383_886_571_644_902_86, 0.391_585_305_496_803_5, 0.444_269_908_287_904_86, 0.516_846_621_784_344_3, 0.558_451_870_343_894_5, 0.592_856_098_119_244_1, 0.656_141_858_361_579_1, 0.659_549_153_410_917_1, 0.643_500_234_447_466_5, 0.650_382_144_953_188_4, 0.665_799_441_780_352_8],
    [0.276_774_714_942_474_35, 0.264_989_382_587_513_3, 0.293_046_527_628_623_1, 0.293_046_527_628_623_1, 0.278_804_593_128_999_3, 0.227_308_205_732_130_15, 0.118_236_905_311_341_79, 0.080_409_728_820_660_36, -0.645_806_293_548_760_5, -0.633_755_320_680_056, -0.596_899_403_563_703_7, -0.553_292_590_965_343_3, -0.517_080_827_606_685_5, -0.428_303_432_174_803_3, 0.060_244_765_480_223_84, 0.078_609_678_548_655_4, 0.114_317_308_277_369_23],
    [9.951_203_613_658_176, 9.917_437_028_561_082, 9.880_861_683_721_347, 9.840_820_355_741_402, 9.814_646_991_436_161, 9.783_459_728_491_962, 9.756_267_000_561_534, 9.753_692_666_107_199, 9.754_394_299_740_573, 9.768_794_233_097_442, 9.796_954_216_389_524, 9.811_011_999_391_983, 9.817_141_855_769_467, 9.818_136_531_369_412, 9.818_401_803_246_31, 9.818_242_501_343_894, 9.813_947_327_268_904],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_26: [(f64, f64); 11] = [
    (5.419_163_063_092_123, 10.0),
    (0.305_804_124_508_528_23, 1.462_607_843_171_520_4),
    (-0.328_729_247_636_373_35, 1.967_389_013_854_437_2),
    (-0.457_400_960_528_277_56, 2.063_972_133_314_939),
    (1.721_077_999_357_347_7, 6.540_111_231_557_497),
    (-1.985_240_797_693_345_7, 0.499_081_861_582_003_35),
    (-3.986_178_065_329_168_5, 0.243_431_747_403_024_03),
    (-0.816_682_623_392_157_5, 0.470_837_257_299_106_5),
    (-0.037_923_126_073_938_634, 0.894_117_754_269_794_6),
    (-1.094_790_563_498_195_4, 0.656_502_571_728_357),
    (8.017_791_846_134_891, 10.079_170_744_039_297),
];

/// The slow accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_DONOR_KINDS_26: [f64; 3] = [
    0.304_986_671_867_888_97,
    0.692_997_854_495_806_5,
    0.002_015_473_636_304_531_5,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    7.272_965_191_390_368e-7,
    1.035_705_059_583_318_5e-5,
    5.201_167_834_304_185e-6,
    5.275_342_139_260_009e-6,
    8.186_746_102_641_874e-6,
    7.458_183_437_644_700_5e-6,
    3.455_957_899_621_731e-6,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.502_646_436_285_646_8,
    0.485_377_153_768_352_83,
    0.011_976_409_946_000_324,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    1.818_559_965_886_104_2e-7,
    8.203_963_599_511_857e-5,
    0.000_123_350_801_033_892_96,
    8.530_829_601_459_712e-5,
    8.512_760_558_978_249e-5,
    7.857_772_918_517_93e-5,
    0.000_173_529_007_204_003_46,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.388_235_530_957_959_43,
    0.597_608_042_733_197_6,
    0.014_156_426_308_843_098,
];
// @end-table binary_awd

// @begin-table binary_xrb
// The X-ray binaries' sampler and share row fitted against plan 11's engine (plan 15, P15.T10.b),
// held until generator version 27 (`catalogue_classes::binary`'s hold).
//
// @generated by hyperion-fit 0.1.0, task `binary_xrb` revision 0. Do not edit.
// inputs-sha256: 59047b1bde352b12a6b8f4ee32a5ad35446beefa81bf7d4eb690800829746bb5
// manifest: crates/hyperion-fit/manifests/binary_xrb.toml
// data: none
// sim-fingerprint: 27d9a5ebf083208ff3c92f0a214ea1ca4ebe56e8cccbae31dc89124597fb8aa0
// since-generator-version: 26
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the generator version's rules; the total
//   against the brainstorm's 10⁴ (Corral-Santana et al. 2016, A&A 587, A61)
// acceptance: slab members 338 of a kind and 244 of none; young high-mass members 109; kinds
//   0.0440/0.1560/0.0588/0.7370/0.0021/0.0021; black-hole transients 7370 of 1e4; layers C 0.000
//   D 0.557 E 0.443; rates a system 1.01e-4/1.22e-5/8.92e-6/4.91e-6/5.61e-6/1.12e-5/1.33e-5;
//   scratch marks: NeutronStarRecurrence, BlackHoleRecurrence, SupergiantRate; the K–S tests
//   against the other half of the census are the sim's slow
//   `tables_hold::the_x_ray_binary_sampler_against_the_engine`
//
// Read by `XRB_SAMPLER_26` and `CLASS_SHARES_26`, before the block. The marks are fitted to the
// engine's members of the kinds `XrbHostKind` holds (hyperion-fit's `tasks::binary_classes::xrb`):
// the low-mass and history marks to the solar circle's slab, the high-mass marks to the young thin
// disc's own census of 3000000 records, each unconditional but the low-mass donor's mass, which
// follows the period. A mark with under 50 members keeps `XRB_SAMPLER`'s law. The kinds' weights
// are the galaxy's shares of its hosts of each kind; the row counts every member of the class,
// those of no kind among them, and is scaled to 1e4 hosts at Milky Way parameters (plan 11's
// design note 11).

/// The X-ray binaries' marks' residual quantiles at the ranks k ÷ 16, in [`XrbMark`]'s order, each
/// in its mark's unit.
#[rustfmt::skip]
pub const XRB_RESIDUALS_26: [[f64; 17]; 20] = [
    [1.191_657_221_478_672_5, 1.260_002_057_648_469_8, 1.275_844_810_043_933, 1.477_912_489_748_764, 1.526_980_740_291_465, 1.567_968_226_650_032_3, 1.640_514_512_054_783_5, 1.679_719_465_189_182_5, 1.737_070_339_778_014_3, 1.792_273_573_091_621_5, 1.810_672_394_414_601_3, 1.833_432_458_299_150_6, 1.859_339_636_739_281, 1.895_516_711_487_082_1, 1.910_717_223_037_54, 1.964_206_995_868_375_4, 1.996_447_350_922_04],
    [2.012_615_529_935_403_4, 2.133_675_407_215_666, 2.214_366_057_019_895, 2.266_619_654_027_888_7, 2.363_747_662_510_98, 2.444_175_761_059_278, 2.515_231_231_530_345, 2.595_749_039_184_804_4, 2.699_845_844_760_439_7, 2.748_676_987_158_540_4, 2.815_083_007_897_271, 2.925_700_231_910_361, 2.994_862_890_404_962_6, 3.117_796_167_648_327, 3.344_165_691_642_479_6, 3.622_743_583_029_484, 6.559_273_923_180_538],
    [0.155_137_502_146_700_13, 0.814_263_018_418_890_7, 1.154_143_236_708_282_2, 1.217_915_967_079_480_8, 1.278_696_627_718_708_6, 1.326_086_227_254_983_8, 1.355_503_990_095_197, 1.394_009_938_662_905_5, 1.437_474_864_132_209_7, 1.455_358_362_282_049, 1.494_131_826_015_079_5, 1.526_234_957_929_263_1, 1.569_875_660_461_542_7, 1.613_598_453_334_583_3, 1.787_332_943_274_336_6, 2.115_529_000_036_804_8, 3.105_471_554_525_194_7],
    [-0.500_509_670_311_661_2, -0.256_888_773_172_012_3, -0.145_447_239_467_382_75, -0.085_793_659_368_468_78, -0.058_026_718_726_666_08, -0.042_567_439_066_247_59, -0.031_596_286_387_574_7, -0.026_307_349_786_006_254, -0.012_716_087_818_835_847, 0.003_879_196_081_225_833, 0.018_870_001_671_590_96, 0.048_555_644_584_029_52, 0.070_679_225_070_257_49, 0.097_569_514_048_969_64, 0.135_717_487_648_108_37, 0.240_892_643_920_183_53, 1.212_036_542_516_502],
    [-6.540_695_594_656_056, -2.423_110_968_676_574_4, -1.904_836_282_071_467_3, -1.798_600_046_645_205_2, -1.647_630_748_890_645, -1.562_461_661_643_186_8, -1.430_262_919_134_139_8, -1.326_951_041_064_480_5, -1.216_222_008_932_912_8, -1.095_371_533_620_154, -0.950_573_128_369_458_5, -0.790_020_802_366_787_2, -0.618_904_876_352_500_9, -0.399_664_097_569_385_65, -0.155_039_711_743_071_07, 0.179_702_454_015_813_4, 1.582_648_612_343_177_3],
    [1.75, 2.254_32, 2.435_993_5, 2.563_591, 2.667_624_5, 2.758_895_5, 2.842_726, 2.922_329_5, 3.0, 3.077_670_5, 3.157_274, 3.241_104_5, 3.332_375_5, 3.436_409, 3.564_006_5, 3.745_68, 4.25],
    [1.25, 1.754_319_999_999_999_9, 1.935_993_5, 2.063_591, 2.167_624_5, 2.258_895_5, 2.342_726, 2.422_329_5, 2.5, 2.577_670_5, 2.657_274, 2.741_104_5, 2.832_375_5, 2.936_409, 3.064_006_5, 3.245_68, 3.75],
    [0.627_099_185_932_020_5, 1.054_100_957_394_124_5, 1.176_351_064_484_052_7, 1.432_340_170_896_588_6, 1.578_059_817_057_383_6, 1.680_907_755_048_110_6, 1.810_514_629_343_619_4, 1.875_770_145_888_871_8, 1.903_187_185_554_490_4, 2.040_690_891_157_337_4, 2.060_938_241_041_338_6, 2.104_142_105_634_354, 2.185_059_082_795_071, 2.241_509_239_163_966, 2.281_464_723_390_549_4, 2.339_344_367_114_658_3, 2.863_591_070_574_552_4],
    [0.018_459_371_511_113_726, 0.038_934_403_508_720_475, 0.045_164_645_372_332_61, 0.050_926_458_576_673_69, 0.060_262_092_315_548_91, 0.065_540_608_246_626_6, 0.074_087_654_726_342_3, 0.082_378_913_601_374_23, 0.090_031_614_105_331_88, 0.111_888_863_294_343_96, 0.118_949_486_587_706_77, 0.130_205_232_700_059_3, 0.137_593_244_774_831_54, 0.150_809_703_814_492_32, 0.174_551_025_021_680_3, 0.257_766_282_881_408_9, 0.618_398_684_532_080_5],
    [8.037_509_922_784_235, 8.217_768_117_065_612, 8.338_308_087_013_2, 8.499_474_719_092_497, 8.704_912_853_049_187, 8.890_843_453_295_95, 9.205_212_295_565_584, 9.517_037_945_002_881, 9.716_533_960_538_221, 10.080_643_380_694_987, 10.538_914_285_581_455, 11.383_423_601_681_498, 11.932_294_937_421_048, 12.502_889_661_053_92, 13.042_946_764_506_642, 15.023_149_301_344_953, 16.871_065_007_533_14],
    [-0.009_494_759_122_446_366, 0.146_405_026_588_488_64, 0.221_682_367_302_023, 0.260_969_168_613_616_35, 0.278_250_559_303_082_1, 0.298_035_214_683_339_77, 0.334_234_834_313_426_6, 0.389_963_826_730_539_14, 0.440_724_591_601_191_8, 0.491_093_682_228_207_65, 1.082_849_626_183_446_8, 1.748_587_764_596_959_2, 2.052_628_005_971_761_6, 2.245_087_846_480_306_6, 2.388_827_597_649_483, 2.475_417_272_285_750_6, 2.672_796_268_047_545],
    [8.068_187_932_070_689, 8.253_090_557_693_858, 8.323_671_195_840_575, 8.479_736_821_105_103, 8.594_847_730_616_058, 8.643_270_048_198_103, 8.850_987_360_987_805, 9.193_791_055_338_778, 9.561_322_490_452_264, 9.842_238_093_448_309, 11.049_723_916_463_684, 11.373_062_477_037_788, 12.919_948_594_267_474, 13.508_907_158_000_412, 14.935_433_700_698_262, 16.018_996_402_695_34, 18.788_929_098_147_45],
    [-10.25, -9.745_68, -9.564_006_5, -9.436_409, -9.332_375_5, -9.241_104_5, -9.157_274, -9.077_670_5, -9.0, -8.922_329_5, -8.842_726, -8.758_895_5, -8.667_624_5, -8.563_591, -8.435_993_5, -8.254_32, -7.75],
    [1.260_676_189_258_159_5, 2.110_687_928_085_489, 2.478_973_476_013_916, 2.825_754_753_144_546_6, 2.965_571_589_851_375, 3.373_556_161_376_898, 3.772_894_718_043_721, 4.151_555_411_783_79, 4.623_709_444_568, 5.000_020_251_829_518, 5.328_038_963_406_019, 5.869_832_394_353_026, 6.833_660_249_754_756_5, 8.557_653_859_596_808, 11.734_623_044_676_752, 20.649_646_670_441_32, 38.807_888_144_215_02],
    [0.494_476_138_824_081_7, 1.938_585_424_147_821_8, 2.814_675_136_405_594_6, 3.505_044_808_619_298, 4.839_574_119_260_202, 6.505_190_125_706_195_5, 19.343_231_452_047_8, 20.536_633_055_496_342, 23.238_865_956_511_766, 25.944_972_079_649_517, 26.896_062_785_394_513, 27.863_775_295_866_745, 28.558_366_435_991_033, 29.232_806_185_495_193, 38.818_932_560_026_17, 53.313_155_772_929_39, 88.398_848_721_343_04],
    [5.857_177_547_820_751_5, 6.186_785_023_472_979, 6.284_145_796_932_883, 6.356_994_189_070_031, 6.630_709_882_130_71, 6.787_771_850_068_362, 6.863_006_772_523_970_5, 6.882_206_067_916_074, 6.928_056_539_039_396, 6.970_259_317_985_109, 7.012_612_439_587_772, 7.060_142_941_645_255, 7.108_709_054_483_184, 7.170_926_383_941_727, 7.243_916_041_604_653, 7.371_659_004_606_053_5, 7.473_891_230_703_389],
    [5.978_121_715_039_82, 6.380_500_452_650_271_5, 6.471_477_182_905_944, 6.582_993_689_167_710_5, 6.651_576_673_045_452, 6.747_033_093_562_284, 6.777_874_944_372_318, 6.804_979_805_207_935, 6.817_629_645_894_765, 6.847_940_867_085_599, 6.919_381_808_279_473, 6.941_799_226_757_512, 7.004_737_400_203_428, 7.051_531_473_902_061, 7.061_538_256_380_971, 7.194_895_945_216_817_5, 7.379_671_014_390_03],
    [0.862_987_734_426_088_9, 0.906_943_433_248_979_4, 0.923_000_475_817_969_4, 0.944_701_015_348_758_8, 0.964_871_810_378_720_9, 0.985_247_344_242_222_4, 0.994_321_907_988_952_6, 1.001_697_160_087_088_3, 1.007_985_880_021_442, 1.015_216_723_737_412_6, 1.024_611_171_415_767, 1.034_324_315_959_661_6, 1.056_591_346_729_577, 1.069_064_745_368_491_6, 1.086_550_350_196_680_7, 1.104_625_811_851_203, 1.184_261_519_243_101_6],
    [0.856_164_746_190_861_2, 0.885_693_845_057_000_1, 0.899_232_523_273_370_4, 0.914_941_937_036_304_9, 0.938_507_166_574_387, 0.955_932_211_778_158_9, 0.977_889_635_680_082, 0.996_385_360_316_131_8, 1.020_739_245_907_277, 1.040_283_992_213_966, 1.055_508_918_921_215_7, 1.076_174_111_939_863, 1.088_167_673_079_441_2, 1.114_401_221_980_619_6, 1.134_413_618_706_183_5, 1.215_017_430_095_438_4, 1.372_297_788_261_705],
    [-0.193_843_284_005_470_6, 0.013_408_303_510_016_865, 0.054_889_046_918_123_08, 0.102_630_141_758_044_18, 0.142_442_861_209_573_83, 0.168_351_526_690_953_85, 0.191_014_058_950_267_52, 0.211_866_089_824_038_57, 0.234_936_758_088_009_62, 0.263_700_053_216_063_93, 0.284_344_153_695_557_04, 0.313_029_493_044_873_15, 0.347_062_747_975_608_9, 0.436_752_973_845_846_4, 0.546_056_646_582_763_9, 0.902_852_651_201_081, 2.237_337_706_613_712_6],
];

/// The X-ray binaries' marks' medians' spans of their conditions, first and last node, in
/// [`XrbMark`]'s order; only the low-mass donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIAN_SPANS_26: [(f64, f64); 20] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.196_854_847_478_298_46, 2.925_013_723_591_151),
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
pub const XRB_MEDIANS_26: [[f64; 17]; 20] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-1.857_797_419_006_675_3, -1.861_029_421_904_515, -1.225_073_635_673_978_9, -0.589_117_849_443_442_8, 0.046_837_936_787_093_3, 0.156_854_339_045_847_67, 0.183_547_575_936_232_17, 0.206_822_494_849_624_87, 0.250_583_769_971_459_15, 0.251_939_475_036_403, -0.014_958_024_856_439_914, -0.108_057_268_578_103_01, -0.162_794_802_479_125_2, -0.162_794_802_479_125_2, -0.162_794_802_479_125_2, -0.162_794_802_479_125_2, -0.162_794_802_479_125_2],
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
pub const XRB_LIMITS_26: [(f64, f64); 20] = [
    (1.191_657_221_478_672_5, 1.996_447_350_922_04),
    (2.012_615_529_935_403_4, 6.559_273_923_180_538),
    (0.155_137_502_146_700_13, 3.105_471_554_525_194_7),
    (-1.994_480_865_818_655_2, 0.813_463_367_662_482_4),
    (-6.540_695_594_656_056, 1.582_648_612_343_177_3),
    (1.75, 4.25),
    (1.25, 3.75),
    (0.627_099_185_932_020_5, 2.863_591_070_574_552_4),
    (0.018_459_371_511_113_726, 0.618_398_684_532_080_5),
    (8.037_509_922_784_235, 16.871_065_007_533_14),
    (-0.009_494_759_122_446_366, 2.672_796_268_047_545),
    (8.068_187_932_070_689, 18.788_929_098_147_45),
    (-10.25, -7.75),
    (1.260_676_189_258_159_5, 38.807_888_144_215_02),
    (0.494_476_138_824_081_7, 88.398_848_721_343_04),
    (5.857_177_547_820_751_5, 7.473_891_230_703_389),
    (5.978_121_715_039_82, 7.379_671_014_390_03),
    (0.862_987_734_426_088_9, 1.184_261_519_243_101_6),
    (0.856_164_746_190_861_2, 1.372_297_788_261_705),
    (-0.193_843_284_005_470_6, 2.237_337_706_613_712_6),
];

/// The low-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::LOW_MASS`]' order.
#[rustfmt::skip]
pub const XRB_LOW_MASS_KINDS_26: [f64; 4] = [
    0.044_015_378_490_305_03,
    0.155_981_138_756_617_5,
    0.058_846_715_894_840_12,
    0.736_983_246_290_492_9,
];

/// The high-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.
#[rustfmt::skip]
pub const XRB_HIGH_MASS_KINDS_26: [f64; 2] = [
    0.002_067_640_412_500_550_6,
    0.002_105_880_155_243_881,
];

/// The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const XRB_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    1.105_012_452_954_427_3e-6,
    1.339_401_714_681_616e-7,
    9.770_499_211_968_315e-8,
    5.372_689_688_740_622e-8,
    6.140_216_787_132_14e-8,
    1.227_397_783_716_581e-7,
    1.458_032_497_898_113_5e-7,
];

/// The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const XRB_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.0,
    0.556_959_681_089_303_2,
    0.443_040_318_910_696_84,
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

    /// The scratch rates are scaled to the fixture's formed mass, so that it holds the
    /// brainstorm's counts, and each class's layer shares sum to 1.
    #[test]
    fn the_scratch_rates_give_the_milky_way_counts() {
        let formed = milky_way_formed_mass();
        assert!(
            (formed / MILKY_WAY_FORMED_MASS - 1.0).abs() < 1e-9,
            "the fixture forms {formed:e} M_sun, the table is scaled to {MILKY_WAY_FORMED_MASS:e}"
        );
        let expected = [4.5e6, 4.5e6, 1.0e4, 9.0e4, 10.0];
        for (row, count) in CLASS_SHARES.rows().iter().zip(expected) {
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
            CLASS_SHARES.row(CarvedClass::XrayBinary).class(),
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
            "mass_ratio",
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
        for (pick, kinds) in [
            (MergerPick::Channel, MergerChannel::HELD.len()),
            (MergerPick::EngineChannel, MergerChannel::ALL.len()),
        ] {
            let channels = MERGER_SAMPLER.pick(pick).weights();
            assert_eq!(channels.len(), kinds, "{pick:?}");
            assert!(
                (channels.iter().sum::<f64>() - 1.0).abs() < 1e-12,
                "{pick:?}"
            );
        }
        assert_eq!(MergerChannel::ALL[..2], MergerChannel::HELD);
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
