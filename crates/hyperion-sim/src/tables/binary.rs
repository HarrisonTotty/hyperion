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
//! since-generator-version: 16 (the batch that lands P11.T7)
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
pub const MILKY_WAY_FORMED_MASS: f64 = 9.376_830_704_125_888e10;

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
/// The shape is narrower than one law per condition node: the residual's spread is the same at
/// every condition, which the median shifts (plan 11's Risks, "P11.T8.a as built").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplerMark {
    name: &'static str,
    residual: [f64; 17],
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
            median,
            limits,
        }
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
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "log_helium_period_hours",
        residual: normal(0.05),
        median: Some(linear(RATE_SPAN.0, RATE_SPAN.1, 0.0, -0.7)),
        // 5.4 and 65.6 minutes.
        limits: (-1.045_757, 0.038_753),
    },
    SamplerMark {
        name: "log_giant_period_hours",
        residual: normal(0.15),
        // 600 days.
        median: Some(flat(RATE_SPAN.0, RATE_SPAN.1, 4.158_362)),
        limits: (3.5, 5.0),
    },
    SamplerMark {
        name: "log_main_sequence_donor_mass",
        residual: normal(0.03),
        // log₁₀ 0.065 + 1.25 p.
        median: Some(linear(0.0, 0.8, -1.187_087, -0.187_087)),
        limits: (-1.301_030, -0.022_276),
    },
    SamplerMark {
        name: "log_helium_donor_mass",
        residual: normal(0.05),
        median: Some(linear(-1.0, 0.1, 0.0, -2.2)),
        limits: (-2.301_030, -0.698_970),
    },
    SamplerMark {
        name: "log_giant_donor_mass",
        residual: normal(0.08),
        // 1.3 M☉.
        median: Some(flat(3.5, 5.0, 0.113_943)),
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
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "primary_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "mass_ratio",
        residual: [
            0.06, 0.08, 0.10, 0.12, 0.14, 0.16, 0.19, 0.22, 0.25,
            0.29, 0.33, 0.38, 0.44, 0.52, 0.62, 0.76, 1.0,
        ],
        median: None,
        limits: (0.06, 1.0),
    },
    SamplerMark {
        name: "contact_mass_ratio",
        residual: [
            0.060, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.090,
            0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.150,
        ],
        median: None,
        limits: (0.06, 0.15),
    },
    SamplerMark {
        name: "evolved_primary_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "evolved_mass_ratio",
        residual: [
            0.05, 0.11, 0.17, 0.25, 0.33, 0.41, 0.49, 0.55, 0.60,
            0.68, 0.76, 0.83, 0.89, 0.93, 0.96, 0.98, 1.0,
        ],
        median: None,
        limits: (0.05, 1.0),
    },
    SamplerMark {
        name: "dynamical_primary_mass",
        residual: [
            0.10, 0.40, 0.42, 0.44, 0.46, 0.49, 0.52, 0.56, 0.60,
            0.61, 0.62, 0.63, 0.64, 0.66, 0.68, 0.69, 0.70,
        ],
        median: None,
        limits: (0.1, 0.7),
    },
    SamplerMark {
        name: "dynamical_mass_ratio",
        residual: [
            0.15, 0.45, 0.52, 0.58, 0.63, 0.66, 0.69, 0.71, 0.73,
            0.76, 0.79, 0.82, 0.85, 0.88, 0.91, 0.94, 1.0,
        ],
        median: None,
        limits: (0.15, 1.0),
    },
    SamplerMark {
        name: "gap_mass_ratio",
        residual: [
            0.05, 0.08, 0.10, 0.11, 0.12, 0.13, 0.14, 0.15, 0.16,
            0.17, 0.18, 0.19, 0.20, 0.21, 0.22, 0.235, 0.25,
        ],
        median: None,
        limits: (0.05, 0.25),
    },
    SamplerMark {
        name: "helium_burning_mass_ratio",
        residual: [
            0.05, 0.0675, 0.085, 0.1025, 0.12, 0.1375, 0.155, 0.1725, 0.19,
            0.2075, 0.225, 0.2425, 0.26, 0.2775, 0.295, 0.3125, 0.33,
        ],
        median: None,
        limits: (0.05, 0.33),
    },
    SamplerMark {
        name: "agb_mass_ratio",
        residual: [
            0.05, 0.09, 0.12, 0.18, 0.27, 0.33, 0.39, 0.44, 0.49,
            0.54, 0.59, 0.64, 0.69, 0.75, 0.82, 0.89, 1.0,
        ],
        median: None,
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
        limits: (1.13, 2.0),
    },
    SamplerMark {
        name: "log_black_hole_mass",
        // log₁₀ 7, ±0.15 dex, on 3–20 M☉.
        residual: normal_about(0.845_098, 0.15),
        median: None,
        limits: (0.477_121, 1.301_030),
    },
    SamplerMark {
        name: "neutron_star_progenitor_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "black_hole_progenitor_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "age_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
];

/// The kinds of the X-ray binaries' class (P11.T8.b), in [`XRB_SAMPLER`]'s two choices' order:
/// the four low-mass kinds of the old populations, then the two high-mass kinds of the young disc.
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
}

impl XrbHostKind {
    /// Every kind, in the choices' order.
    pub const ALL: [Self; 6] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
        Self::BeX,
        Self::Supergiant,
    ];

    /// The low-mass kinds, in [`XrbPick::LowMassKind`]'s order.
    pub const LOW_MASS: [Self; 4] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
    ];

    /// The high-mass kinds, in [`XrbPick::HighMassKind`]'s order.
    pub const HIGH_MASS: [Self; 2] = [Self::BeX, Self::Supergiant];

    /// Whether the kind is a high-mass X-ray binary's, whose hosts are the young disc's.
    #[must_use]
    pub const fn is_high_mass(self) -> bool {
        match self {
            Self::BeX | Self::Supergiant => true,
            Self::NeutronStarPersistent
            | Self::NeutronStarTransient
            | Self::BlackHolePersistent
            | Self::BlackHoleTransient => false,
        }
    }

    /// Whether the kind is a low-mass transient's, whose outbursts recur.
    #[must_use]
    pub const fn is_transient(self) -> bool {
        match self {
            Self::NeutronStarTransient | Self::BlackHoleTransient => true,
            Self::NeutronStarPersistent
            | Self::BlackHolePersistent
            | Self::BeX
            | Self::Supergiant => false,
        }
    }

    /// What the kind's compact star is: a neutron star, or a black hole for the black-hole kinds.
    #[must_use]
    pub const fn accretor(self) -> XrayAccretor {
        match self {
            Self::BlackHolePersistent | Self::BlackHoleTransient => XrayAccretor::BlackHole,
            Self::NeutronStarPersistent
            | Self::NeutronStarTransient
            | Self::BeX
            | Self::Supergiant => XrayAccretor::NeutronStar,
        }
    }

    /// The binary class P11.T5's `classify` gives a pair of the kind.
    #[must_use]
    pub const fn class(self) -> BinaryClass {
        match self {
            Self::NeutronStarPersistent | Self::BlackHolePersistent => {
                BinaryClass::LowMassXrayBinary(XrbKind::Persistent)
            }
            Self::NeutronStarTransient | Self::BlackHoleTransient => {
                BinaryClass::LowMassXrayBinary(XrbKind::Transient)
            }
            Self::BeX => BinaryClass::HighMassXrayBinary(HmxbKind::BeX),
            Self::Supergiant => BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant),
        }
    }
}

/// The marks of [`XRB_SAMPLER`], in its order (P11.T8.b): what an X-ray binary is now, by its
/// kind, then its history.
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
}

impl XrbMark {
    /// Every mark, in [`XRB_SAMPLER`]'s order.
    pub const ALL: [Self; 20] = [
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
    ];

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
/// [`XrbMark`]'s twenty marks and [`XrbPick`]'s two kind choices.
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
const XRB_MARKS: [SamplerMark; 20] = [
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
const XRB_MARK_NAMES: [&str; 20] = [
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
];

/// The X-ray binaries' fitted marks (P15.T10.b) from task `binary_xrb`'s block, in [`XrbMark`]'s
/// order: each mark's residual quantiles, its median's span and nodes, and its limits. Only the
/// low-mass donor's mass follows a condition, the period; the other marks' median rows are not
/// read.
const fn fitted_xrb_marks(
    residuals: &[[f64; 17]; 20],
    spans: &[(f64, f64); 20],
    medians: &[[f64; 17]; 20],
    limits: &[(f64, f64); 20],
) -> [SamplerMark; 20] {
    let mut out = [SamplerMark::new("", [0.0; 17], None, (0.0, 0.0)); 20];
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
const XRB_MARKS_26: [SamplerMark; 20] = fitted_xrb_marks(
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
// sim-fingerprint: 4438b2f6cb5e4da197a31271fe5cc1681e7d4fc5fc740d828ab93da66b8ed6fe
// since-generator-version: 25
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the generator version's rules; the total
//   against Pala et al. 2020, MNRAS 494, 3799, and the brainstorm's 6–12 × 10⁶
// acceptance: fast: 1839 members, donors 0.955/0.028/0.016, layers C 0.500 D 0.488 E 0.012,
//   rates a system 2.80e-6/3.98e-5/2.00e-5/2.03e-5/3.23e-5/2.81e-5/1.33e-5, not its own:
//   GiantPeriod Pooled, GiantDonorMass Pooled; slow: 15404 members, donors 0.306/0.692/0.002,
//   layers C 0.388 D 0.598 E 0.014, rates a system
//   7.01e-7/3.17e-4/4.75e-4/3.30e-4/3.32e-4/2.99e-4/6.72e-4, not its own: GiantPeriod Pooled,
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
    [0.794_218_948_949_652, 3.933_413_891_380_183, 4.303_848_838_408_616, 4.435_199_357_180_804, 4.512_361_603_127_747, 4.581_381_084_167_433, 4.634_826_585_765_066, 4.698_637_076_098_164, 4.766_189_525_558_908, 4.818_543_322_958_52, 4.896_462_379_766_104, 4.985_467_358_109_45, 5.070_123_000_434_354, 5.159_637_255_859_209, 5.243_894_894_435_088, 5.326_740_458_861_225, 5.418_038_116_770_516],
    [-0.615_770_489_219_198_4, -0.271_778_591_973_131_1, -0.217_860_200_224_655_4, -0.194_427_215_948_825_8, -0.165_973_141_349_947_95, -0.120_688_384_942_074_07, -0.050_866_223_363_260_63, -0.020_969_916_528_536_26, 0.004_528_251_435_659_381, 0.036_709_942_154_320_33, 0.075_478_966_040_988_36, 0.121_038_470_940_539_99, 0.178_992_489_787_685_78, 0.294_702_297_581_287_63, 0.447_490_516_520_439_6, 0.618_054_610_672_994_3, 0.876_812_127_454_415_3],
    [-1.794_631_777_829_472_3, -0.786_749_571_313_777_3, -0.631_440_522_552_625, -0.334_452_554_311_626_1, -0.139_231_169_100_390_8, -0.085_473_353_690_992_21, -0.056_162_046_337_769_71, -0.026_115_579_472_089_007, -0.001_949_139_296_341_329, 0.029_052_772_809_833_594, 0.064_789_395_162_617_39, 0.120_801_093_950_582_64, 0.191_261_823_356_660_3, 0.275_817_941_027_459_6, 0.398_243_363_560_062, 0.557_993_662_880_873_6, 1.222_690_114_631_138],
    [-0.244_340_322_583_916_36, -0.059_125_502_536_475_83, -0.044_477_262_501_110_44, -0.021_089_262_786_589_888, -0.014_824_567_077_538_445, -0.010_068_870_892_022_514, -0.005_810_548_952_054_802, -0.000_482_988_542_614_398, 0.005_100_597_825_373_795_5, 0.011_077_130_732_332_805, 0.015_655_627_842_433_753, 0.030_495_013_977_356_08, 0.042_681_114_887_132_096, 0.137_211_044_453_402_24, 0.245_252_433_488_829_2, 0.352_391_899_871_441_1, 0.422_155_602_308_866_06],
    [-0.265_120_475_236_880_54, -0.095_308_138_646_977_83, -0.071_850_864_616_368_91, -0.056_124_307_683_667_9, -0.036_931_009_388_625_535, -0.017_459_734_044_010_272, -0.010_286_217_939_947_329, 0.003_792_559_443_022_883, 0.026_930_384_119_939_088, 0.033_955_482_836_556_15, 0.041_337_999_604_177_28, 0.061_295_382_992_762_965, 0.074_082_517_079_773_2, 0.081_676_949_911_627_47, 0.130_069_005_870_031_9, 3.508_117_349_027_185_7, 4.526_400_688_020_93],
    [-0.841_276_495_081_339_4, -0.241_693_158_548_127_92, -0.172_278_112_710_367_86, -0.123_644_182_109_274_64, -0.086_945_236_769_923_7, -0.053_985_164_962_159_41, -0.036_772_677_394_508_596, -0.022_377_894_066_938_967, -0.007_847_285_587_937_416, 0.008_568_488_991_096_544, 0.023_752_108_350_482_026, 0.040_403_214_560_850_866, 0.063_313_779_472_790_22, 0.118_276_522_173_223_26, 0.217_945_224_626_122_7, 0.303_081_714_124_246_17, 0.672_669_206_407_480_8],
    [-0.073_026_464_815_935_33, -0.014_181_383_786_892_127, -0.012_744_172_616_590_194, -0.010_300_379_950_983_79, -0.009_754_904_621_755_822, -0.009_610_243_626_990_991, -0.009_056_596_423_857_755, -0.007_865_418_717_130_757, -0.005_689_425_868_862_896, -0.004_248_079_888_134_484_4, 0.005_942_518_186_281_47, 0.029_501_493_699_104_93, 0.166_339_984_137_026_72, 1.203_353_396_602_116_6, 1.348_913_958_386_048, 1.611_780_301_629_023, 1.759_996_180_245_965_4],
    [-0.063_971_490_660_861_5, -0.041_826_445_323_001_82, -0.033_178_780_567_409_43, -0.024_179_902_990_029_145, -0.015_216_805_363_908_986, -0.004_522_423_558_095_893_5, -0.002_435_619_006_207_395_7, 0.007_706_137_485_891_246, 0.028_766_706_096_805_54, 0.044_307_426_268_299_77, 0.059_844_822_883_166_06, 0.096_365_059_143_324_27, 0.150_005_554_053_497_47, 0.217_537_259_989_410_48, 0.361_202_098_496_170_35, 0.710_486_171_288_787_4, 1.237_171_700_342_286_9],
    [-0.481_248_300_884_303_3, -0.072_228_425_382_546_3, -0.050_877_794_527_884_095, -0.038_309_986_664_501_83, -0.028_059_650_552_137_057, -0.018_672_724_056_260_08, -0.009_444_735_245_850_96, -0.003_705_738_771_790_763_3, 0.001_588_023_534_377_036_6, 0.008_348_784_816_215_392, 0.014_557_371_115_146_031, 0.022_811_196_684_523_354, 0.030_118_737_180_468_702, 0.037_836_102_972_439_28, 0.048_462_119_535_749_86, 0.068_910_720_019_401_17, 0.207_759_343_750_539_5],
    [-1.115_425_143_779_176_7, -0.143_792_783_544_110_51, -0.091_713_785_172_980_04, -0.064_311_714_515_760_34, -0.040_871_484_241_873_44, -0.023_783_445_898_743_4, -0.011_069_745_891_622_384, -0.001_016_738_553_323_941_5, 0.009_948_402_066_312_464, 0.020_726_037_266_302_115, 0.029_798_671_942_114_84, 0.039_938_022_375_539_95, 0.050_672_260_215_671_92, 0.065_622_650_698_263_53, 0.081_173_245_152_781_07, 0.111_967_364_060_521_88, 0.904_873_130_326_766_3],
    [-1.454_748_585_243_880_4, -0.685_384_292_350_083_7, -0.510_584_509_416_492_2, -0.368_239_194_316_720_9, -0.241_763_579_797_903_64, -0.168_817_932_684_537_6, -0.095_773_843_790_183_83, -0.044_643_857_070_973_96, 0.000_256_469_137_461_579_07, 0.033_503_062_119_339_067, 0.068_321_954_250_204_26, 0.109_974_689_197_464, 0.156_016_347_525_110_18, 0.208_314_977_046_998_75, 0.264_650_205_367_261, 0.346_703_010_636_334_7, 0.985_245_625_216_224_5],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_26: [(f64, f64); 11] = [
    (0.0, 1.0),
    (2.824_301_288_377_391, 5.411_244_615_339_646),
    (-10.083_796_112_606_139, -8.088_615_572_509_726),
    (-11.135_687_963_446_461, -7.291_534_376_035_222),
    (-15.620_177_698_528_96, -6.985_854_850_164_248),
    (-0.353_775_089_365_056_8, 1.780_259_176_407_615_4),
    (-0.765_977_273_227_754_3, -0.069_607_670_260_016_52),
    (1.796_130_567_871_092_4, 6.348_849_543_953_039),
    (0.315_064_678_903_777_76, 1.377_752_249_250_754),
    (-1.700_449_165_779_009_3, 0.449_780_646_752_014_35),
    (0.029_184_937_092_863_794, 0.892_309_125_785_286_2),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_26: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.266_547_487_258_008_3, 1.239_112_057_025_730_1, 1.164_554_384_808_319_7, 1.136_381_036_236_205_7, 1.120_597_469_469_109, 1.082_283_560_078_246_6, 0.938_206_179_863_876_3, 0.896_517_319_062_093_9, 0.786_502_926_292_434_1, 0.667_175_088_560_22, 0.579_071_095_567_937_5, 0.557_918_432_966_689_2, 0.557_979_453_256_304_4, 0.573_836_782_291_098_6, 0.616_504_696_225_717_8, 0.683_843_910_420_553_1, 0.740_217_723_478_837_4],
    [1.046_605_573_410_899_2, 1.063_796_137_201_428_8, 0.463_773_222_485_274_75, 0.252_967_144_168_567_14, 0.257_554_077_720_832_33, 0.811_148_452_235_949_2, 1.063_397_257_096_892_6, 0.898_364_054_132_272_3, 0.847_425_823_700_097_7, 0.894_116_169_557_812_3, 1.307_536_922_378_659_7, 1.381_053_266_795_848_6, 1.421_610_363_173_095, 1.441_978_519_761_277_8, 1.466_884_539_474_371_7, 1.502_571_873_099_309_4, 1.555_813_095_976_356_8],
    [-0.149_007_067_753_287_65, -0.149_007_067_753_287_65, -0.174_389_252_854_908_14, -0.209_736_138_189_710_1, -0.267_395_022_584_776_2, -0.325_053_906_979_842_27, -0.382_712_791_374_908_43, -0.440_371_675_769_974_5, -0.498_030_560_165_040_6, -0.555_689_444_560_106_7, -0.564_496_729_421_112_2, -0.564_496_729_421_112_2, -0.564_496_729_421_112_2, -0.564_496_729_421_112_2, -0.564_496_729_421_112_2, -0.564_496_729_421_112_2, -0.564_496_729_421_112_2],
    [2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.033_277_953_597_997_5, 2.002_197_249_640_828_7, 1.975_662_872_574_120_8, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4],
    [-1.345_118_590_806_519_5, -1.284_549_356_257_325, -1.051_303_235_066_680_7, -0.936_254_418_936_948, -0.786_547_446_160_343_3, -0.666_623_618_311_71, -0.602_049_527_712_247_5, -0.119_313_963_294_189_7, -0.095_235_295_181_914_29, -0.082_653_989_449_383_56, -0.062_429_642_486_663_02, 0.025_889_099_566_220_08, 0.037_037_823_142_807_666, 0.031_149_535_031_832_03, -0.001_200_927_421_223_367, -0.099_223_940_778_376_5, -0.287_315_520_639_474_7],
    [-1.319_836_604_665_465_3, -1.319_836_604_665_465_3, -1.319_836_604_665_465_3, -1.319_836_604_665_465_3, -1.319_836_604_665_465_3, -1.346_450_851_659_157_6, -1.389_461_645_671_257, -1.432_472_439_683_356_4, -1.475_483_233_695_455_5, -1.518_494_027_707_555, -1.561_504_821_719_654_3, -1.604_515_615_731_753_7, -1.647_526_409_743_853_1, -1.696_137_971_323_681, -1.739_715_674_692_677_6, -1.785_384_943_782_708, -1.785_384_943_782_708],
    [-0.734_000_280_006_429_7, -0.764_652_609_593_089_9, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2],
    [0.110_345_133_278_083_48, 0.151_330_826_100_475_35, 0.208_025_018_314_676_12, 0.318_224_252_539_520_3, 0.367_006_817_741_912_9, 0.440_364_112_748_463_3, 0.508_756_568_346_846_7, 0.557_507_036_138_266_7, 0.621_703_985_857_760_2, 0.725_496_234_972_103_5, 0.741_383_346_872_096_1, 0.755_225_637_934_590_4, 0.782_971_343_112_087_7, 0.818_954_229_349_421_6, 0.849_117_925_131_287_1, 0.863_356_019_838_611_8, 0.863_480_505_913_489_3],
    [0.111_577_204_929_774_94, -0.597_954_273_122_864_5, -0.591_860_263_087_906_3, -0.467_761_806_682_564_1, -0.202_081_516_108_676_03, -0.108_010_356_258_665_81, -0.069_069_403_666_151_43, -0.065_126_189_815_652_66, -0.035_477_628_558_157_8, 0.027_397_955_558_910_187, 0.038_243_398_511_509_51, 0.032_903_429_410_081_585, 0.038_216_522_782_603_665, 0.071_456_481_426_356_55, 0.164_126_636_002_826_73, 0.301_987_016_137_719_55, 0.346_862_686_714_287_16],
    [9.929_180_776_524_246, 9.879_698_958_739_867, 9.815_579_968_055_543, 9.758_190_015_285_905, 9.744_962_390_330_741, 9.727_536_737_342_412, 9.711_453_693_656_646, 9.673_613_176_022_899, 9.627_717_384_981_317, 9.565_386_132_280_99, 9.409_927_723_266_037, 9.064_140_403_099_573, 9.102_081_584_869_04, 9.488_846_130_430_685, 9.642_556_944_078_125, 9.636_212_991_204, 9.558_220_685_006_994],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_26: [(f64, f64); 11] = [
    (0.794_218_948_949_652, 5.418_038_116_770_516),
    (0.296_601_634_205_204_74, 1.483_786_340_347_661),
    (-0.445_071_966_194_821_8, 1.948_347_714_303_122_7),
    (-0.800_359_105_831_161_3, -0.048_793_491_161_041_785),
    (1.721_077_999_357_347_7, 6.540_111_385_157_536),
    (-1.932_978_601_950_638, 0.621_099_788_498_994_5),
    (-1.858_411_408_598_643_4, 0.005_261_985_907_671_135),
    (-0.816_682_623_392_157_5, 0.470_837_257_299_106_5),
    (-0.049_996_140_421_199_75, 0.921_778_823_803_262_7),
    (-1.044_577_772_221_719, 0.632_446_824_184_782_3),
    (8.120_636_962_353_265, 10.078_479_482_311_554),
];

/// The fast accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_DONOR_KINDS_26: [f64; 3] = [
    0.955_410_549_211_528,
    0.028_276_237_085_372_486,
    0.016_313_213_703_099_51,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16, in
/// [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_26: [[f64; 17]; 11] = [
    [5.419_163_063_092_123, 5.950_645_001_970_622_5, 6.289_009_220_400_046, 6.428_137_825_257_067, 6.636_876_341_362_369_5, 6.912_453_715_785_626_5, 7.392_152_228_295_652, 7.769_421_365_219_109, 8.033_207_565_398_52, 8.259_963_734_020_989, 8.425_422_945_587_261, 8.558_544_436_052_795, 8.667_559_618_604_9, 8.781_104_628_834_78, 8.904_976_418_579_896, 9.112_752_504_573_19, 10.0],
    [-0.911_691_512_730_985_7, -0.336_211_729_952_903_7, -0.230_813_217_691_899_97, -0.182_140_107_026_341_7, -0.138_689_137_337_104_56, -0.092_895_393_014_876_43, -0.049_396_777_279_078_444, -0.021_606_538_359_068_415, 0.004_322_646_175_318_245_5, 0.030_505_289_007_381_003, 0.058_023_332_685_720_325, 0.093_722_382_498_027_61, 0.138_193_161_947_346_65, 0.188_420_678_260_585_28, 0.263_943_986_665_123_3, 0.419_881_636_887_715_7, 0.887_952_441_689_662_7],
    [-1.274_463_584_989_433_5, -0.681_113_211_537_771_8, -0.540_475_762_700_534_4, -0.415_382_775_698_658_1, -0.298_454_029_281_579_44, -0.183_596_695_748_398_15, -0.110_389_651_744_209_17, -0.048_733_070_045_763, 0.005_217_521_790_325_003, 0.077_354_986_025_002_8, 0.182_193_756_675_858_4, 0.319_902_615_927_927_24, 0.452_004_714_244_817_4, 0.590_889_370_534_068_3, 0.732_547_060_679_284_9, 0.842_306_127_140_613_5, 1.535_327_020_493_631_1],
    [-0.294_516_041_937_964, -0.040_274_454_669_180_185, -0.033_474_147_019_363_416, -0.026_269_628_568_460_938, -0.022_495_775_114_954_555, -0.018_248_847_022_637_46, -0.014_660_030_085_287_858, -0.010_561_986_488_093_866, -0.006_647_965_718_484_076_5, -0.002_609_639_084_378_494, 0.001_819_433_715_740_878_5, 0.006_425_065_285_001_044, 0.012_392_878_584_118_416, 0.020_110_432_339_066_85, 0.030_512_414_549_844_963, 0.047_113_110_867_883_01, 1.246_328_649_809_508],
    [-0.265_120_475_236_880_54, -0.095_308_138_646_977_83, -0.071_850_864_616_368_91, -0.056_124_307_683_667_9, -0.036_931_009_388_625_535, -0.017_459_734_044_010_272, -0.010_286_217_939_947_329, 0.003_792_559_443_022_883, 0.026_930_384_119_939_088, 0.033_955_482_836_556_15, 0.041_337_999_604_177_28, 0.061_295_382_992_762_965, 0.074_082_517_079_773_2, 0.081_676_949_911_627_47, 0.130_069_005_870_031_9, 3.508_117_349_027_185_7, 4.526_400_688_020_93],
    [-1.022_512_449_540_878, -0.084_596_621_597_578_58, -0.059_791_327_003_422_62, -0.046_334_656_796_213_79, -0.036_712_729_678_222_7, -0.028_574_599_935_410_918, -0.020_271_286_791_977_25, -0.011_861_665_736_046_462, -0.001_923_826_558_799_773_8, 0.006_679_059_832_948_582, 0.014_192_425_283_753_99, 0.023_142_731_667_297_75, 0.031_651_267_587_457_55, 0.042_505_687_471_284_82, 0.058_150_097_662_487_146, 0.087_834_737_226_039_7, 0.367_119_233_087_185_2],
    [-1.238_291_622_792_304_2, -0.040_183_341_335_861_38, -0.027_400_037_403_475_297, -0.018_519_079_503_188_54, -0.012_608_394_818_367_286, -0.007_442_969_819_270_95, -0.002_403_246_845_421_702_6, 0.000_607_616_025_285_029_3, 0.004_244_696_104_719_559, 0.012_254_524_417_698_31, 0.019_288_004_805_672_61, 0.022_501_640_982_659_665, 0.025_117_318_214_122_33, 0.027_042_663_781_077_142, 0.028_574_503_145_699_648, 0.030_206_644_036_085_473, 2.279_369_628_347_346_6],
    [-0.063_971_490_660_861_5, -0.041_826_445_323_001_82, -0.033_178_780_567_409_43, -0.024_179_902_990_029_145, -0.015_216_805_363_908_986, -0.004_522_423_558_095_893_5, -0.002_435_619_006_207_395_7, 0.007_706_137_485_891_246, 0.028_766_706_096_805_54, 0.044_307_426_268_299_77, 0.059_844_822_883_166_06, 0.096_365_059_143_324_27, 0.150_005_554_053_497_47, 0.217_537_259_989_410_48, 0.361_202_098_496_170_35, 0.710_486_171_288_787_4, 1.237_171_700_342_286_9],
    [-0.448_532_698_680_862, -0.239_361_412_879_409_32, -0.124_816_245_275_997_41, -0.073_869_872_392_763_55, -0.049_981_636_906_944_28, -0.034_760_295_672_862_845, -0.022_387_487_895_398_85, -0.010_519_978_174_853_992, 0.001_403_771_804_927_872_5, 0.013_897_765_198_141_17, 0.027_515_288_098_072_792, 0.041_382_127_391_686_37, 0.057_216_564_139_888_806, 0.077_181_053_539_160_8, 0.108_195_841_864_012_53, 0.155_765_848_670_731_37, 0.334_946_027_172_905_34],
    [-1.026_343_047_480_794_4, -0.190_693_975_068_479_13, -0.157_167_200_019_624_64, -0.124_461_467_935_026_35, -0.091_161_385_442_071_13, -0.063_735_457_262_295_39, -0.040_958_056_805_956_794, -0.019_862_036_694_963_215, -0.000_730_670_890_654_178_1, 0.014_178_187_062_351_29, 0.026_578_679_105_776_527, 0.039_067_320_730_628_78, 0.052_963_177_237_221_24, 0.070_229_535_472_875_91, 0.091_781_711_303_987_9, 0.127_154_300_538_795_6, 0.716_898_944_956_348_3],
    [-1.802_522_632_911_964_4, -0.562_052_480_305_363_1, -0.400_660_676_299_846_8, -0.295_135_360_241_985_17, -0.213_978_968_027_992_43, -0.142_733_789_507_519_91, -0.085_430_296_063_839_78, -0.036_249_045_929_078_85, 0.001_663_919_687_997_506, 0.035_339_439_609_365_586, 0.068_295_131_550_548, 0.100_180_485_919_580_18, 0.128_693_298_750_899_74, 0.155_545_463_874_443, 0.182_160_443_554_129_97, 0.222_459_630_920_620_5, 0.327_369_951_738_013_75],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_26: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.524_336_774_490_389, 10.0),
    (-12.468_127_601_898_694, -9.291_557_934_188_875),
    (-15.455_358_801_002_57, -10.325_548_780_489_584),
    (-15.620_177_698_528_96, -6.985_854_850_164_248),
    (-0.224_314_937_310_198_64, 1.481_794_930_709_952_5),
    (-0.254_738_545_329_915_2, 0.888_256_616_420_659_6),
    (1.796_130_567_871_092_4, 6.348_849_543_953_039),
    (0.342_364_992_230_792_4, 1.431_111_215_967_632_5),
    (-2.753_310_661_214_415, 0.216_177_550_952_762_45),
    (0.050_251_932_886_851_69, 0.847_365_861_827_940_4),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_26: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.694_294_171_655_343_1, 0.658_323_192_821_322_3, 0.599_414_282_801_537_2, 0.566_891_051_871_618_7, 0.556_827_468_461_406, 0.606_905_647_632_003_5, 1.120_248_408_929_055_2, 1.255_037_315_925_138_7, 1.142_359_568_557_308_7, 1.043_572_631_814_877_7, 1.009_469_243_397_944_2, 0.988_081_928_490_949, 0.950_799_995_451_478_2, 0.934_471_834_132_984_3, 1.225_057_798_747_419_8, 1.279_362_136_034_356_1, 1.194_731_343_108_434_3],
    [0.992_970_842_521_452_8, 0.992_970_842_521_452_8, 0.924_524_756_168_855_1, 0.818_138_078_976_337_2, 1.004_921_070_498_419_7, 0.960_016_343_613_892_2, 0.770_951_293_660_872_2, 0.395_359_100_762_330_44, 0.426_670_397_882_924_3, 1.034_828_309_314_936_9, 1.045_462_424_779_191, 0.988_373_399_100_787, 0.360_277_767_932_211, 0.347_730_498_720_721_54, 0.905_449_509_387_353_7, 0.961_543_154_275_775_5, 0.942_510_305_024_887],
    [0.817_643_483_505_430_8, 0.783_278_698_641_093, 0.725_071_720_152_957_6, 0.654_788_750_991_021_3, 0.584_431_133_646_568_2, 0.516_618_120_691_645_5, 0.439_075_396_011_255_7, 0.306_957_856_981_684_4, 0.245_610_456_870_417_47, 0.223_525_087_625_601_4, 0.180_842_164_675_049_2, 0.115_276_207_003_544_5, 0.042_821_394_349_984_866, -0.025_064_547_858_024_255, -0.095_767_534_264_278_81, -0.161_104_527_975_167, -0.225_474_781_561_795_6],
    [2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.046_505_535_677_040_3, 2.033_277_953_597_997_5, 2.002_197_249_640_828_7, 1.975_662_872_574_120_8, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4, 2.013_710_697_136_605_4],
    [-1.116_001_547_725_507_6, -1.063_743_019_882_242_4, -1.006_100_191_230_804_5, -0.910_103_108_714_592_4, -0.800_145_273_145_809_3, -0.704_456_633_555_237_1, -0.638_618_591_003_400_6, -0.563_887_818_357_648_6, -0.148_924_213_923_022_05, -0.010_942_084_711_203_018, 0.005_583_099_452_725_039, 0.025_580_576_613_199_69, 0.053_540_512_682_099_09, 0.082_923_046_715_290_92, 0.125_997_687_530_317_65, 0.172_296_124_978_330_02, 0.202_034_459_616_314_05],
    [-1.684_468_229_863_615_8, -1.746_715_469_984_234_4, -1.817_470_045_606_601, -1.890_144_412_144_200_4, -1.963_621_902_963_406, -2.034_847_634_592_412, -2.098_462_092_956_862, -2.136_296_844_622_747, -2.159_883_148_515_475, -2.257_239_994_110_127_8, -2.364_693_824_623_04, -2.443_642_321_512_057_5, -2.516_962_766_683_524_6, -2.584_102_599_311_112, -2.657_638_607_431_98, -2.707_380_556_945_421_5, -2.747_886_442_536_864_3],
    [-0.734_000_280_006_429_7, -0.764_652_609_593_089_9, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2, -0.766_334_443_043_180_2],
    [0.136_377_832_427_560_74, 0.172_040_026_037_218_07, 0.270_244_910_527_649_3, 0.335_565_230_999_114_23, 0.382_831_899_122_741_85, 0.422_981_221_740_209_13, 0.385_557_239_866_572_4, 0.391_784_346_082_728, 0.444_269_908_287_904_86, 0.516_758_892_813_737_2, 0.558_555_760_218_386_6, 0.593_078_160_708_661_9, 0.653_623_762_081_152_1, 0.657_530_247_922_953, 0.643_178_807_055_062, 0.650_670_969_648_956_2, 0.665_887_567_780_390_8],
    [0.273_349_646_637_192_86, 0.264_227_776_865_777_53, 0.292_936_605_263_379_95, 0.293_185_547_559_715_5, 0.279_166_888_313_124_75, 0.227_098_654_066_551_25, 0.117_518_128_018_466_73, 0.078_356_658_367_854_6, -0.641_187_426_216_880_5, -0.633_755_321_518_079_6, -0.596_937_929_151_822_9, -0.552_378_586_053_681_8, -0.516_484_452_753_684_5, -0.434_998_555_763_089_93, 0.060_678_558_052_364_8, 0.078_722_814_075_254_87, 0.114_115_953_414_338_67],
    [9.951_479_356_051_13, 9.917_437_028_561_082, 9.880_188_102_178_844, 9.840_277_081_890_72, 9.813_405_771_419_49, 9.784_159_629_470_79, 9.756_507_680_551_145, 9.751_800_792_301_283, 9.751_800_792_301_283, 9.768_161_428_769_726, 9.793_927_871_010_036, 9.810_090_958_434_213, 9.817_185_109_686_537, 9.820_928_475_206_088, 9.820_902_212_454_007, 9.818_257_856_343_871, 9.814_160_771_338_585],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_26: [(f64, f64); 11] = [
    (5.419_163_063_092_123, 10.0),
    (0.305_804_134_342_104_1, 1.462_607_843_171_520_4),
    (-0.328_729_251_535_815_6, 1.967_389_092_320_799_3),
    (-0.457_399_730_514_136_4, 2.063_972_133_314_939),
    (1.721_077_999_357_347_7, 6.540_111_385_157_536),
    (-1.985_240_797_693_345_7, 0.499_081_861_582_003_35),
    (-3.986_178_065_329_168_5, 0.243_431_747_403_024_03),
    (-0.816_682_623_392_157_5, 0.470_837_257_299_106_5),
    (-0.037_923_126_073_938_634, 0.894_117_754_269_794_6),
    (-1.094_790_563_510_538_4, 0.656_502_571_728_357),
    (8.017_791_846_134_891, 10.079_170_744_039_297),
];

/// The slow accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_DONOR_KINDS_26: [f64; 3] = [
    0.306_349_000_259_672_8,
    0.691_703_453_648_403,
    0.001_947_546_091_924_175_6,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    7.301_225_490_768_368e-7,
    1.037_481_261_541_777_9e-5,
    5.211_949_739_397_424e-6,
    5.295_840_346_559_742e-6,
    8.401_119_675_552_664e-6,
    7.304_600_805_006_740_4e-6,
    3.469_386_590_989_136_7e-6,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.500_253_626_279_213_5,
    0.487_773_661_746_200_5,
    0.011_972_711_974_585_912,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    1.825_626_278_967_544_6e-7,
    8.246_279_535_890_334e-5,
    0.000_123_756_978_579_426_3,
    8.582_233_802_144_835e-5,
    8.637_141_922_704_02e-5,
    7.778_758_481_659_377e-5,
    0.000_174_933_723_657_635_8,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.387_855_559_881_581_27,
    0.598_200_091_255_173_4,
    0.013_944_348_863_245_372,
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
// sim-fingerprint: 145d86372edd869fbd79a280482938f0d67ee058bfb507479168f18fd697537e
// since-generator-version: 25
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the generator version's rules; the total
//   against the brainstorm's 10⁴ (Corral-Santana et al. 2016, A&A 587, A61)
// acceptance: slab members 344 of a kind and 238 of none; young high-mass members 110; kinds
//   0.0467/0.1367/0.0587/0.7537/0.0021/0.0021; black-hole transients 7537 of 1e4; layers C 0.000
//   D 0.561 E 0.439; rates a system 1.04e-4/1.23e-5/8.05e-6/4.91e-6/4.91e-6/1.19e-5/1.19e-5;
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
    [1.191_657_221_478_672_5, 1.260_002_057_648_469_8, 1.275_844_810_043_933, 1.444_391_151_119_811_3, 1.530_001_035_124_210_2, 1.568_583_056_702_570_5, 1.641_194_754_250_124_2, 1.679_716_121_489_074, 1.737_070_339_778_014_3, 1.792_164_102_603_499_8, 1.810_744_990_370_746_4, 1.833_432_458_299_150_6, 1.859_239_543_305_370_8, 1.895_516_711_487_082_1, 1.910_717_223_037_54, 1.964_206_995_868_375_4, 1.996_447_350_922_04],
    [2.012_615_529_935_403_4, 2.134_865_313_694_659_6, 2.217_802_076_425_051, 2.283_838_806_009_87, 2.383_607_745_593_224, 2.472_674_952_328_9, 2.543_119_299_498_449, 2.632_727_241_482_425, 2.709_818_765_942_183_7, 2.766_788_785_375_515_3, 2.820_386_112_214_815_3, 2.936_497_562_510_663, 3.030_098_721_854_726, 3.142_174_599_942_823, 3.357_150_808_600_655, 3.638_445_477_982_622_7, 7.965_248_775_045_085],
    [0.154_229_571_768_677_74, 0.828_882_575_033_836_6, 1.162_373_890_491_919_9, 1.226_656_626_275_212_8, 1.289_618_246_399_138, 1.329_504_226_177_947, 1.366_232_622_514_611_5, 1.405_326_652_397_238_3, 1.438_250_086_120_409, 1.463_441_889_184_860_8, 1.505_850_150_097_022_5, 1.533_871_432_702_669_5, 1.575_637_757_868_669_6, 1.613_820_450_922_221_6, 1.790_522_153_392_538_8, 2.124_894_988_163_875_5, 3.105_471_554_525_194_7],
    [-0.701_360_575_512_708_6, -0.266_351_676_241_227_6, -0.150_428_302_215_395_52, -0.086_893_009_584_824_93, -0.061_573_460_820_234_46, -0.042_915_223_757_617_57, -0.032_650_372_695_916_38, -0.027_299_066_956_332_1, -0.012_093_281_952_872_709, 0.005_226_620_276_801_007, 0.016_335_810_570_997_575, 0.046_743_405_872_648_074, 0.067_595_363_191_868_24, 0.095_501_309_371_249_58, 0.131_974_595_164_057_13, 0.239_508_693_899_607_1, 1.210_786_321_517_454_6],
    [-6.540_695_594_656_056, -2.433_342_928_133_272, -1.901_634_953_554_939_5, -1.798_368_655_490_865_3, -1.647_385_474_467_103_3, -1.562_740_368_582_217_9, -1.413_109_641_545_253_3, -1.323_331_465_601_130_1, -1.222_507_088_432_127_9, -1.107_546_596_256_713, -0.960_297_001_133_243_6, -0.794_928_049_424_987_3, -0.645_258_961_037_400_5, -0.405_828_238_700_134_74, -0.123_039_779_773_460_53, 0.180_293_517_780_595_69, 1.582_648_612_343_177_3],
    [1.75, 2.254_32, 2.435_993_5, 2.563_591, 2.667_624_5, 2.758_895_5, 2.842_726, 2.922_329_5, 3.0, 3.077_670_5, 3.157_274, 3.241_104_5, 3.332_375_5, 3.436_409, 3.564_006_5, 3.745_68, 4.25],
    [1.25, 1.754_319_999_999_999_9, 1.935_993_5, 2.063_591, 2.167_624_5, 2.258_895_5, 2.342_726, 2.422_329_5, 2.5, 2.577_670_5, 2.657_274, 2.741_104_5, 2.832_375_5, 2.936_409, 3.064_006_5, 3.245_68, 3.75],
    [0.627_099_185_932_020_5, 1.062_196_069_110_127_8, 1.190_622_354_512_197_7, 1.442_371_422_755_068_1, 1.595_565_773_111_117, 1.701_514_469_760_817_3, 1.828_170_493_171_980_2, 1.888_846_760_611_251, 1.908_722_909_760_124_6, 2.053_295_433_663_541_4, 2.087_055_264_591_589, 2.141_591_471_368_33, 2.196_462_375_729_221, 2.270_302_699_232_246_4, 2.288_197_071_185_112, 2.387_078_744_179_631_5, 2.863_591_070_574_552_4],
    [0.018_459_371_511_113_726, 0.038_033_098_334_583_49, 0.044_207_562_593_099_325, 0.048_484_616_137_579_12, 0.058_673_186_866_432_98, 0.064_667_142_269_533_07, 0.073_494_092_033_826_38, 0.082_473_422_919_763_2, 0.095_584_530_123_786_07, 0.114_631_769_776_454_09, 0.120_903_205_919_218_01, 0.131_651_799_572_338_2, 0.143_453_594_748_613_83, 0.158_279_694_500_251_53, 0.190_094_512_185_366_74, 0.277_678_814_848_613_8, 0.619_215_162_524_849_6],
    [8.037_509_922_784_235, 8.244_041_112_806_812, 8.426_737_957_755_1, 8.613_109_469_163_819, 8.833_769_139_124_069, 9.036_974_378_525_185, 9.447_871_476_700_087, 9.636_672_495_693_592, 9.898_703_855_342_22, 10.342_637_751_679_124, 11.316_673_347_714_778, 11.821_304_163_444_33, 12.261_696_634_581_842, 12.541_057_688_166_72, 13.629_382_015_823_618, 15.481_924_388_300_463, 16.871_065_007_533_14],
    [-0.008_944_320_735_430_495, 0.143_644_338_183_327_36, 0.218_461_128_262_994_88, 0.266_108_638_221_183_5, 0.282_709_788_715_783_3, 0.300_346_448_981_3, 0.340_122_630_608_868, 0.411_698_798_250_006_7, 0.449_333_569_272_808_93, 0.581_310_508_088_437, 1.166_515_687_406_309_4, 1.815_861_494_858_965_3, 2.070_921_054_861_136_3, 2.257_275_972_324_297_4, 2.392_302_494_184_877, 2.479_358_255_707_994, 2.672_796_268_047_545],
    [8.068_187_932_070_689, 8.251_844_731_486_507, 8.338_178_230_267_65, 8.487_631_404_279_115, 8.601_717_516_572_613, 8.658_730_311_334_994, 8.917_152_115_426_031, 9.228_770_631_784_705, 9.569_269_620_451_45, 10.310_267_881_203_565, 11.157_816_317_796_632, 12.132_300_550_984_51, 13.294_740_661_264_553, 13.702_726_692_798_064, 15.023_144_913_402_96, 16.021_882_587_489_73, 18.788_929_098_147_45],
    [-10.25, -9.745_68, -9.564_006_5, -9.436_409, -9.332_375_5, -9.241_104_5, -9.157_274, -9.077_670_5, -9.0, -8.922_329_5, -8.842_726, -8.758_895_5, -8.667_624_5, -8.563_591, -8.435_993_5, -8.254_32, -7.75],
    [1.260_676_189_258_159_5, 2.136_748_212_509_684_4, 2.529_912_866_399_051_6, 2.843_716_560_982_642, 2.979_651_176_933_16, 3.527_249_219_776_668, 3.900_111_344_784_374_7, 4.186_272_002_491_759_5, 4.623_709_444_568, 4.974_126_419_576_286, 5.228_884_099_992_722, 5.516_932_229_956_188, 6.393_087_709_790_890_5, 8.298_197_319_298_096, 10.682_501_124_134_87, 19.922_253_405_512_865, 38.844_327_206_040_15],
    [0.494_476_138_824_081_7, 1.937_649_340_897_241_3, 2.787_664_548_454_479, 3.479_511_310_808_226, 4.823_035_591_396_085, 5.991_122_861_350_22, 18.617_021_101_801_69, 19.838_017_192_581_02, 21.222_044_925_194_222, 24.431_143_304_161_093, 26.223_638_582_476_266, 27.206_707_820_285_143, 28.478_922_715_969_496, 29.287_141_765_318_55, 39.456_625_690_130_54, 53.512_246_462_728_41, 88.505_799_194_176_66],
    [5.690_529_023_627_952, 5.946_033_452_895_051, 6.238_527_388_785_847_4, 6.306_460_745_887_271_5, 6.447_485_695_268_736, 6.680_454_444_836_649, 6.824_415_720_197_724, 6.874_489_635_293_411, 6.888_957_796_725_942, 6.941_436_518_612_360_6, 6.981_163_956_817_452, 7.020_277_688_207_9, 7.103_223_958_919_845, 7.154_157_588_612_596, 7.220_800_416_641_219, 7.315_031_649_734_045, 7.444_682_532_858_668],
    [5.383_882_569_445_701, 6.268_123_073_640_488_5, 6.430_056_100_785_52, 6.517_000_292_978_555, 6.636_467_323_439_16, 6.679_542_751_210_619, 6.770_310_447_475_672, 6.796_650_699_133_451, 6.816_341_651_327_865, 6.846_956_738_514_315, 6.899_455_213_684_855, 6.937_050_647_887_347, 6.992_462_096_344_223, 7.053_028_182_194_907_5, 7.062_095_215_119_363, 7.196_215_153_468_444, 7.379_671_014_390_03],
    [0.862_987_734_426_088_9, 0.902_329_107_630_445_7, 0.918_807_016_157_137_1, 0.934_385_277_607_949_4, 0.963_168_286_657_477_4, 0.981_270_082_763_742_2, 0.993_320_255_507_781_8, 1.000_433_887_821_510_7, 1.006_612_430_479_414_2, 1.013_245_676_152_649_4, 1.020_030_795_871_094, 1.033_502_053_259_172_5, 1.056_591_346_729_577, 1.069_064_745_368_491_6, 1.086_550_350_196_680_7, 1.104_625_811_851_203, 1.184_261_519_243_101_6],
    [0.856_164_746_190_861_2, 0.883_847_988_924_506_8, 0.895_949_036_259_138_8, 0.910_164_384_290_206_5, 0.936_820_205_834_638_7, 0.955_022_796_289_224_8, 0.977_206_572_373_145_5, 0.995_519_269_241_261_6, 1.020_485_932_189_005_9, 1.038_515_154_732_043, 1.055_334_140_157_716_8, 1.075_162_371_994_948, 1.087_527_814_650_744_6, 1.104_393_953_255_551, 1.131_431_591_475_197_3, 1.212_521_330_211_696_6, 1.456_794_502_018_168_5],
    [-0.193_843_284_005_470_6, 0.014_029_603_174_454_83, 0.055_810_397_897_520_17, 0.105_635_801_881_239_85, 0.143_415_210_414_208_6, 0.173_075_387_297_896, 0.192_462_422_432_971_3, 0.214_676_230_036_947_88, 0.238_422_524_774_587_34, 0.265_470_574_369_82, 0.288_516_139_528_050_9, 0.314_345_276_004_985_9, 0.357_058_986_273_078_5, 0.440_768_748_769_694_6, 0.580_371_377_429_294_5, 0.867_118_346_808_395_6, 2.239_316_091_576_748_5],
];

/// The X-ray binaries' marks' medians' spans of their conditions, first and last node, in
/// [`XrbMark`]'s order; only the low-mass donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIAN_SPANS_26: [(f64, f64); 20] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.197_346_868_527_477_74, 2.909_464_382_879_166),
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
    [-1.846_923_837_461_032_6, -1.857_888_762_277_115_4, -1.223_073_002_156_12, -0.588_257_242_035_124_6, 0.046_558_518_085_870_73, 0.155_951_857_397_707, 0.182_323_339_251_452_25, 0.206_404_832_066_037_88, 0.248_869_446_998_193, 0.251_939_475_036_403, -0.014_958_024_856_439_914, -0.090_318_832_537_670_67, -0.130_612_404_167_790_16, -0.130_612_404_167_790_16, -0.130_612_404_167_790_16, -0.130_612_404_167_790_16, -0.130_612_404_167_790_16],
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
    (2.012_615_529_935_403_4, 7.965_248_775_045_085),
    (0.154_229_571_768_677_74, 3.105_471_554_525_194_7),
    (-1.994_386_481_051_038_6, 0.813_463_367_662_482_4),
    (-6.540_695_594_656_056, 1.582_648_612_343_177_3),
    (1.75, 4.25),
    (1.25, 3.75),
    (0.627_099_185_932_020_5, 2.863_591_070_574_552_4),
    (0.018_459_371_511_113_726, 0.619_215_162_524_849_6),
    (8.037_509_922_784_235, 16.871_065_007_533_14),
    (-0.008_944_320_735_430_495, 2.672_796_268_047_545),
    (8.068_187_932_070_689, 18.788_929_098_147_45),
    (-10.25, -7.75),
    (1.260_676_189_258_159_5, 38.844_327_206_040_15),
    (0.494_476_138_824_081_7, 88.505_799_194_176_66),
    (5.690_529_023_627_952, 7.444_682_532_858_668),
    (5.383_882_569_445_701, 7.379_671_014_390_03),
    (0.862_987_734_426_088_9, 1.184_261_519_243_101_6),
    (0.856_164_746_190_861_2, 1.456_794_502_018_168_5),
    (-0.193_843_284_005_470_6, 2.239_316_091_576_748_5),
];

/// The low-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::LOW_MASS`]' order.
#[rustfmt::skip]
pub const XRB_LOW_MASS_KINDS_26: [f64; 4] = [
    0.046_711_678_617_725_81,
    0.136_720_628_314_040_08,
    0.058_675_259_070_294_41,
    0.753_684_984_054_335_5,
];

/// The high-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.
#[rustfmt::skip]
pub const XRB_HIGH_MASS_KINDS_26: [f64; 2] = [
    0.002_141_998_100_672_604_4,
    0.002_065_451_842_931_541_7,
];

/// The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const XRB_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    1.154_846_918_413_799_4e-6,
    1.367_952_703_511_251_8e-7,
    8.966_491_714_621_258e-8,
    5.466_828_197_396_250_6e-8,
    5.466_828_197_396_250_6e-8,
    1.327_001_391_269_614_7e-7,
    1.327_439_314_763_550_5e-7,
];

/// The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const XRB_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.0,
    0.561_378_339_577_568_6,
    0.438_621_660_422_431_4,
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
