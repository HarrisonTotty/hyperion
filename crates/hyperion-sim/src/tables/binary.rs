//! Plan 11's binary tables (P11.T6–T8): their shapes are plan 11's, their contents plan 15's
//! (P15.T9.b, P15.T10.b), which P11.T15 swaps in with a version bump.
//!
//! [`IA_YIELD`] (P11.T6) is the Type Ia explosion mark's yield η: the share of the binary engine's
//! pooled Type Ia candidates that explode, by delay bin and channel. **Scratch:** a sixth
//! everywhere (Design note 13), the observed double white dwarf merger rate being five to seven
//! times the Type Ia rate (Maoz, Hallakoun and Badenes 2018, MNRAS 476, 2584); P15.T9.b fits it
//! against the engine so that η times the pool's yield in each bin is the delay-time
//! distribution's.
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

/// The delay bins of [`IaYieldTable`]: those of [`DELAY_EDGES`], plan 15's.
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
    /// [`DELAY_EDGES`], a delay outside the edges in the end bin on its side.
    #[must_use]
    pub fn eta(&self, delay: Years, channel: IaPoolChannel) -> f64 {
        self.eta_in(ia_delay_bin(delay), channel)
    }
}

/// The bin of [`DELAY_EDGES`] holding `delay`, the ends held: the first bin below 40 Myr, which no
/// white dwarf reaches, and the last beyond the Hubble time.
#[must_use]
pub fn ia_delay_bin(delay: Years) -> usize {
    DELAY_EDGES
        .partition_point(|&edge| edge <= delay.value())
        .saturating_sub(1)
        .min(IA_DELAY_BINS - 1)
}

/// The scratch yield (module documentation; plan 11, Design note 13): a sixth in every delay bin
/// and for both channels.
pub const IA_YIELD: IaYieldTable = IaYieldTable::new([[1.0 / 6.0; 2]; IA_DELAY_BINS]);

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
/// 1,259; `merger/engine_channels_hold.rs`'s slow test prints them) and not fitted:
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
// P15.T10.b), held until generator version 26 (`catalogue_classes::binary`'s hold).
//
// @generated by hyperion-fit 0.1.0, task `binary_awd` revision 0. Do not edit.
// inputs-sha256: fd589d224a26cbd7ae0d30563d4917dc8409e1817efb5db5ee17a0cbbc3a1470
// manifest: crates/hyperion-fit/manifests/binary_awd.toml
// data: none
// sim-fingerprint: 37bd3b25ad361ed5a5e4cdbb791c6bd9d0a7fc0f5f06a171fa2e3b71559204d1
// since-generator-version: 24
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the generator version's rules; the total
//   against Pala et al. 2020, MNRAS 494, 3799, and the brainstorm's 6–12 × 10⁶
// acceptance: fast: 1814 members, donors 0.934/0.042/0.024, layers C 0.477 D 0.512 E 0.011,
//   rates a system 2.10e-6/3.93e-5/2.00e-5/2.03e-5/3.09e-5/3.02e-5/1.19e-5, not its own:
//   GiantPeriod Pooled, GiantDonorMass Pooled; slow: 15120 members, donors 0.315/0.682/0.003,
//   layers C 0.372 D 0.613 E 0.015, rates a system
//   7.01e-7/3.10e-4/4.89e-4/3.30e-4/3.37e-4/2.88e-4/6.57e-4, not its own: GiantPeriod Pooled,
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
    [0.805_537_852_454_358_2, 3.706_071_887_672_224_3, 4.169_148_724_122_587_5, 4.382_741_780_887_295_5, 4.491_400_402_214_596, 4.574_054_821_882_151, 4.657_362_084_246_099, 4.729_753_133_148_141, 4.783_789_255_570_764, 4.849_658_405_853_15, 4.939_419_412_930_238_5, 5.021_095_347_528_314, 5.106_420_123_607_53, 5.179_986_263_983_578_5, 5.263_571_501_043_430_5, 5.343_863_236_350_518, 5.418_038_116_770_516],
    [-0.641_639_102_939_817_2, -0.307_501_634_338_842_1, -0.242_094_126_793_940_96, -0.188_998_777_287_514_55, -0.117_896_224_995_656_6, -0.077_380_251_396_447_15, -0.056_097_465_092_851_476, -0.029_800_067_553_475_702, 0.004_794_024_251_665_696, 0.041_485_513_479_975_26, 0.070_505_835_146_468_98, 0.113_573_795_157_255_7, 0.211_562_200_385_542_55, 0.354_547_722_948_167, 0.497_650_118_825_474_16, 0.616_281_820_621_473_9, 0.833_118_525_866_526_9],
    [-1.796_670_794_010_912_3, -0.852_284_928_056_144_9, -0.674_095_516_480_887_7, -0.492_919_437_629_781_47, -0.236_927_875_862_414_37, -0.116_519_184_038_408_52, -0.062_806_337_902_844_73, -0.035_152_632_192_841_59, -0.003_597_619_450_886_186, 0.034_638_224_680_681_37, 0.087_410_370_450_488_5, 0.155_335_061_336_709, 0.227_816_634_261_563_68, 0.311_863_876_381_981_65, 0.393_563_006_699_516_5, 0.524_282_818_029_622_7, 1.207_253_255_114_521_9],
    [-0.416_460_435_652_928_3, -0.169_872_735_105_626_1, -0.071_597_691_913_676_36, -0.052_294_378_316_687_366, -0.037_663_844_222_035_42, -0.028_646_049_771_682_933, -0.012_546_954_946_227_054, -0.005_333_783_368_281_594, 0.0, 0.003_263_138_606_340_483_5, 0.005_843_590_036_822_18, 0.009_905_727_857_343_172, 0.017_118_481_398_017_138, 0.024_883_868_593_631_307, 0.057_173_859_658_346_98, 0.184_067_382_503_586_92, 1.321_251_964_718_112],
    [-0.249_700_115_204_969_38, -0.112_771_106_295_742_44, -0.082_212_413_706_034_05, -0.057_083_490_382_599_8, -0.042_910_659_439_725_274, -0.032_696_734_560_428_15, -0.015_059_263_050_974_048, -0.004_921_137_895_064_376, 0.000_433_258_384_106_238_74, 0.010_516_856_868_204_943, 0.021_735_879_975_020_134, 0.040_936_681_187_537_13, 0.048_751_332_867_626_56, 0.064_911_417_748_140_69, 0.103_854_752_718_967_38, 3.375_828_219_630_373_3, 4.160_520_440_045_394],
    [-0.841_760_828_448_155, -0.258_261_062_122_889_7, -0.175_852_442_193_208_25, -0.126_158_458_159_433_94, -0.089_945_502_107_334_15, -0.059_100_349_608_414_746, -0.041_462_492_213_100_81, -0.026_218_621_310_372_55, -0.010_613_642_886_250_24, 0.004_658_522_170_345_833, 0.022_509_618_369_409, 0.038_152_693_198_510_46, 0.062_080_396_372_989_11, 0.103_031_887_513_775_64, 0.179_395_938_919_778_87, 0.257_946_746_270_677, 0.734_790_998_543_568_8],
    [-1.273_760_096_765_299_8, -0.055_974_274_865_240_98, -0.041_973_147_433_820_86, -0.026_767_963_079_209_478, -0.021_789_933_922_667_704, -0.015_921_393_078_361_823, -0.007_316_745_589_333_373, -0.001_966_687_257_980_104_2, 0.004_768_399_585_692_196, 0.008_818_982_847_308_92, 0.013_090_466_607_441_953, 0.016_086_871_037_270_245, 0.020_164_007_324_088_518, 0.041_347_713_623_623_83, 1.222_785_281_174_057_4, 1.439_494_084_721_083, 1.756_599_534_464_769_6],
    [-0.067_544_866_706_486_18, -0.044_552_260_161_972_8, -0.032_905_359_526_587_45, -0.026_001_000_057_328_402, -0.017_356_467_330_803_815, -0.012_153_814_619_537_164, -0.009_459_563_792_777_792, -0.000_100_220_831_284_962_56, 0.011_037_276_416_031_916, 0.032_712_744_584_374_95, 0.052_715_182_789_113_29, 0.118_474_279_476_397_35, 0.169_077_038_434_093_44, 0.211_056_982_601_945_4, 0.305_756_883_881_675_05, 0.671_651_816_860_697_3, 1.231_464_482_776_076_8],
    [-0.506_720_504_732_634_2, -0.323_998_475_070_473_36, -0.146_432_484_792_771_46, -0.060_025_131_430_981_885, -0.039_579_131_436_440_054, -0.026_118_543_804_204_968, -0.014_193_654_517_829_404, -0.003_875_033_464_467_443_6, 0.004_574_867_639_417_168, 0.012_308_775_134_075_595, 0.021_167_035_822_353_547, 0.029_287_715_411_194_928, 0.037_405_159_293_849_71, 0.046_375_562_658_400_135, 0.053_432_678_666_979_25, 0.075_927_939_178_807_43, 0.240_944_875_185_698_63],
    [-1.127_267_751_001_304, -0.151_478_675_626_357_1, -0.097_645_506_123_269_44, -0.065_951_201_753_562_8, -0.044_874_504_079_368_31, -0.027_975_416_957_103_486, -0.015_450_385_620_434_654, -0.004_926_360_415_174_753, 0.003_762_065_455_712_681_3, 0.014_654_408_590_949_602, 0.024_020_945_618_133_935, 0.035_600_526_564_499_21, 0.048_671_930_473_031_776, 0.063_344_725_355_345_56, 0.085_517_949_726_133_75, 0.128_271_137_841_647_62, 0.990_870_267_245_622_4],
    [-1.458_942_479_800_01, -0.639_238_326_908_925_7, -0.490_029_645_274_250_3, -0.376_478_068_236_677_73, -0.245_139_295_828_478_15, -0.163_561_947_768_375_18, -0.096_702_052_574_372_92, -0.041_162_095_772_193_21, 0.000_816_725_488_067_326_9, 0.037_309_182_229_322_335, 0.071_352_777_186_833_2, 0.111_850_956_335_128_1, 0.158_398_108_787_201_5, 0.209_332_129_775_366_86, 0.269_383_802_367_314_5, 0.360_749_031_464_727_6, 0.977_257_734_827_594_1],
];

/// The fast accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_FAST_MEDIAN_SPANS_26: [(f64, f64); 11] = [
    (0.0, 1.0),
    (2.699_035_730_183_58, 5.411_462_599_069_011_5),
    (-10.105_858_288_783_866, -8.040_448_656_535_874),
    (-11.227_802_932_131_21, -7.293_674_875_502_392),
    (-13.656_590_964_648_293, -7.100_224_314_739_383),
    (-0.356_921_321_085_588_94, 1.884_280_118_613_311_6),
    (-0.665_745_027_030_147_6, 1.042_867_636_786_300_6),
    (1.817_516_018_517_854_7, 5.734_347_611_645_019),
    (0.315_064_021_593_070_3, 1.431_415_057_084_491_2),
    (-1.783_757_259_382_188_7, 0.340_292_538_488_635_94),
    (0.028_105_846_385_071_14, 0.892_395_337_791_865_2),
];

/// The fast accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_FAST_MEDIANS_26: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.302_878_621_815_574_3, 1.266_547_487_258_008_3, 1.239_112_057_025_730_1, 1.184_488_712_221_599, 1.148_336_556_746_741_8, 1.138_676_489_010_540_3, 1.079_728_082_728_254_3, 0.931_772_574_650_848, 0.857_658_127_427_544_1, 0.794_336_407_155_805_3, 0.708_317_794_405_766_5, 0.638_930_379_185_949_7, 0.602_690_300_421_801_2, 0.604_445_320_204_891_1, 0.637_323_412_837_486_7, 0.706_688_229_283_686_5, 0.747_307_931_450_370_4],
    [1.122_454_060_952_058_8, 1.150_963_914_663_094, 1.044_008_921_741_701_7, 0.275_334_629_167_414_8, 0.270_422_359_775_857_55, 0.882_850_959_425_934_7, 1.180_811_448_401_227, 0.912_037_841_475_022_5, 0.848_199_518_052_094_1, 1.160_258_070_628_837_2, 1.329_621_940_528_602_5, 1.400_108_379_344_413_8, 1.441_143_896_166_435_1, 1.469_733_326_830_402, 1.498_437_943_328_727_6, 1.544_701_635_700_676_6, 1.563_971_800_226_211_5],
    [-0.097_533_269_772_101_32, -0.122_949_727_749_916_39, -0.156_764_915_215_465_48, -0.195_503_235_758_474_34, -0.257_927_188_542_355_63, -0.318_856_587_538_120_96, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34, -0.379_785_986_533_886_34],
    [2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.065_022_362_034_575_6, 2.054_616_243_569_972_4, 2.044_210_125_105_369_3, 2.033_804_006_640_765_7, 2.035_063_029_708_883, 2.024_863_328_329_219_7, 1.973_527_257_480_312_9, 1.969_560_002_406_848_3, 2.055_418_332_901_131_5, 2.055_418_332_901_131_5, 2.055_418_332_901_131_5],
    [-1.369_910_886_422_806_2, -1.279_494_381_183_927_4, -1.046_327_886_369_339, -0.917_035_780_777_65, -0.756_908_161_920_181_5, -0.653_902_808_919_494_5, -0.277_084_802_914_909_9, -0.107_850_281_825_080_82, -0.090_655_221_573_082_08, -0.080_623_117_581_149_47, -0.021_743_827_415_342_146, 0.055_349_740_281_213_6, 0.049_157_320_059_741_855, 0.013_132_221_773_649_793, -0.138_467_427_838_360_14, -0.354_252_106_099_835_7, -0.513_182_055_729_549],
    [-1.356_829_920_608_720_4, -1.356_829_920_608_720_4, -1.469_832_750_464_152, -1.524_023_518_484_163_5, -1.702_770_151_623_618, -1.756_654_720_883_346_7, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2, -1.783_818_693_457_131_2],
    [-0.737_150_395_036_852_5, -0.756_250_458_886_896_3, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4],
    [0.111_826_667_836_724_33, 0.157_869_177_831_897_36, 0.246_836_503_951_242_6, 0.329_099_903_663_849_06, 0.371_778_661_573_562_8, 0.441_893_111_848_146_7, 0.498_346_025_597_069_25, 0.536_140_995_744_203_3, 0.585_094_605_021_005_2, 0.714_072_854_941_628_9, 0.744_067_256_853_234_5, 0.765_520_452_131_320_8, 0.798_549_171_065_732_1, 0.830_683_323_143_650_9, 0.851_736_334_036_049_3, 0.831_915_461_739_034_4, 0.680_833_948_617_564_1],
    [0.195_403_008_721_585_25, 0.164_183_105_830_430_1, -0.354_290_000_509_756_6, -0.591_860_160_020_724, -0.488_833_310_887_916_5, -0.173_912_755_385_894_3, -0.088_917_105_804_145_27, -0.050_714_838_360_634, -0.013_503_178_757_109_489, 0.033_579_350_973_302_134, 0.043_869_153_273_788, 0.041_546_356_957_845_21, 0.030_676_642_971_513_19, 0.036_683_235_344_147_62, 0.086_268_212_556_482_68, 0.247_014_150_509_760_47, 0.308_442_598_460_031_7],
    [9.929_180_776_524_246, 9.890_898_631_119_308, 9.826_621_227_621_432, 9.783_931_632_447_771, 9.763_549_734_361_867, 9.731_730_564_361_472, 9.712_748_668_545_233, 9.673_315_532_068_223, 9.614_325_512_331_066, 9.564_402_803_365_256, 9.345_033_704_742_27, 9.064_140_403_099_573, 9.112_286_249_680_595, 9.492_323_968_360_132, 9.646_630_221_732_025, 9.642_556_944_078_125, 9.558_220_685_006_994],
];

/// The fast accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_LIMITS_26: [(f64, f64); 11] = [
    (0.805_537_852_454_358_2, 5.418_038_116_770_516),
    (0.296_601_454_319_145_3, 1.480_978_608_180_117_7),
    (-0.443_095_662_618_314_8, 2.011_856_861_249_435),
    (-0.796_246_422_186_814_6, 1.070_837_910_188_447),
    (1.721_077_999_357_347_7, 6.215_938_772_946_526),
    (-1.970_675_358_311_537_5, 0.420_472_285_396_032_65),
    (-3.057_578_790_222_431, 0.005_261_985_907_671_135),
    (-0.816_627_956_194_494, 0.470_837_257_299_106_5),
    (-0.049_996_140_421_199_75, 0.921_778_823_803_262_7),
    (-1.044_577_732_629_548, 0.568_653_436_324_834_8),
    (8.150_513_772_455_156, 10.078_479_482_311_554),
];

/// The fast accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_FAST_DONOR_KINDS_26: [f64; 3] = [
    0.933_847_850_055_126_8,
    0.042_447_629_547_960_31,
    0.023_704_520_396_912_898,
];

/// The slow accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16, in
/// [`AwdMark`]'s order, each in its mark's unit.
#[rustfmt::skip]
pub const AWD_SLOW_RESIDUALS_26: [[f64; 17]; 11] = [
    [5.418_866_376_726_735, 5.896_346_612_419_761, 6.233_190_346_899_124, 6.381_758_903_109_436, 6.559_791_014_881_559, 6.714_143_091_198_391, 6.944_733_295_546_063_4, 7.396_881_391_720_243, 7.829_932_787_249_373, 8.161_591_149_474_818, 8.388_436_215_500_027, 8.561_613_325_006_263, 8.695_193_656_846_653, 8.851_810_870_276_408, 9.241_000_961_175_558, 10.0, 10.0],
    [-1.117_496_483_958_000_2, -0.458_643_688_002_038_4, -0.281_188_268_402_432_1, -0.211_007_639_974_223_73, -0.148_069_550_817_029_36, -0.079_040_724_705_703_45, -0.042_619_319_527_322_716, -0.015_820_945_671_788_6, 0.000_125_280_561_742_768_8, 0.015_837_749_768_702_954, 0.041_104_453_419_569_02, 0.074_187_347_485_953_4, 0.110_934_192_698_982_41, 0.161_232_137_189_109_42, 0.354_883_225_652_137_6, 0.705_295_295_538_231_7, 1.060_109_697_236_097_5],
    [-1.292_820_153_791_826_7, -0.688_537_023_013_011_8, -0.536_412_495_283_847_8, -0.413_237_239_390_018_2, -0.300_170_507_727_630_54, -0.186_857_883_705_707_44, -0.113_868_284_222_046_2, -0.050_805_405_880_937_53, 0.005_830_069_210_633_199, 0.085_707_036_797_905_92, 0.189_035_394_741_984_3, 0.320_705_887_926_532_43, 0.453_838_185_203_156_7, 0.594_761_648_612_021, 0.734_954_314_840_855_9, 0.843_474_707_216_183_3, 1.630_874_133_728_261_7],
    [-1.199_056_900_308_009_4, -0.093_303_863_616_185_46, -0.045_747_748_887_633_97, -0.040_580_848_978_458_48, -0.035_960_860_519_024_425, -0.030_268_895_174_843_102, -0.023_281_756_322_601_196, -0.016_743_938_453_457_667, -0.009_710_783_083_861_585, -0.001_883_493_711_670_725_8, 0.005_749_958_211_279_368_5, 0.012_499_145_677_719_78, 0.021_420_746_798_854_307, 0.031_281_569_790_830_2, 0.044_735_138_886_127_52, 0.126_250_393_707_991_3, 2.607_342_656_278_299_8],
    [-0.249_700_115_204_969_38, -0.112_771_106_295_742_44, -0.082_212_413_706_034_05, -0.057_083_490_382_599_8, -0.042_910_659_439_725_274, -0.032_696_734_560_428_15, -0.015_059_263_050_974_048, -0.004_921_137_895_064_376, 0.000_433_258_384_106_238_74, 0.010_516_856_868_204_943, 0.021_735_879_975_020_134, 0.040_936_681_187_537_13, 0.048_751_332_867_626_56, 0.064_911_417_748_140_69, 0.103_854_752_718_967_38, 3.375_828_219_630_373_3, 4.160_520_440_045_394],
    [-1.080_968_561_677_877_5, -0.088_845_722_119_463_84, -0.061_293_820_641_989_134, -0.048_566_317_401_548_265, -0.039_134_967_442_945_91, -0.031_374_330_229_428_59, -0.022_139_104_395_316_41, -0.013_316_619_476_882_008, -0.002_691_665_669_327_575_3, 0.006_482_309_432_290_667, 0.014_759_977_051_378_02, 0.024_199_479_130_685_035, 0.033_598_345_830_377_35, 0.044_395_434_028_251_884, 0.060_840_604_628_306_18, 0.091_225_524_899_126_28, 0.405_451_968_020_110_4],
    [-2.085_886_265_489_501_4, -0.065_387_354_028_410_45, -0.047_049_593_036_049_664, -0.035_904_027_896_119_77, -0.025_653_733_432_740_577, -0.016_416_593_290_914_966, -0.008_151_623_492_363_48, 0.000_121_008_859_994_303_77, 0.007_014_950_742_655_124, 0.013_019_489_577_876_464, 0.020_038_799_423_150_677, 0.028_808_790_009_308_47, 0.042_741_396_625_601_835, 0.063_027_789_381_717_57, 0.087_676_395_648_128_15, 0.105_972_870_498_525_05, 2.323_985_374_322_903],
    [-0.067_544_866_706_486_18, -0.044_552_260_161_972_8, -0.032_905_359_526_587_45, -0.026_001_000_057_328_402, -0.017_356_467_330_803_815, -0.012_153_814_619_537_164, -0.009_459_563_792_777_792, -0.000_100_220_831_284_962_56, 0.011_037_276_416_031_916, 0.032_712_744_584_374_95, 0.052_715_182_789_113_29, 0.118_474_279_476_397_35, 0.169_077_038_434_093_44, 0.211_056_982_601_945_4, 0.305_756_883_881_675_05, 0.671_651_816_860_697_3, 1.231_464_482_776_076_8],
    [-0.488_191_633_834_854_64, -0.232_837_157_477_643_9, -0.116_009_635_408_646_32, -0.075_578_290_324_990_92, -0.054_873_767_042_221_88, -0.039_739_193_050_376_626, -0.025_293_766_308_469_01, -0.012_062_187_513_281_26, 0.001_811_602_574_949_111_6, 0.015_902_831_012_214_47, 0.030_879_846_884_638_323, 0.046_215_360_477_980_29, 0.061_180_190_990_790_11, 0.078_775_356_148_980_01, 0.103_803_291_952_509_62, 0.152_317_031_376_636_7, 0.346_104_739_169_295_4],
    [-1.292_544_704_696_363_8, -0.211_443_386_776_519_03, -0.167_734_667_141_739_7, -0.123_382_972_081_543_18, -0.085_450_089_026_219_62, -0.059_584_828_595_746_916, -0.040_499_336_333_502_17, -0.021_201_596_966_107_11, -0.002_172_092_452_246_206_2, 0.012_146_258_927_733_494, 0.023_800_713_443_168_295, 0.034_956_639_746_832_274, 0.049_546_761_024_606_93, 0.068_319_696_676_380_33, 0.095_054_240_400_565_41, 0.136_743_932_040_648_72, 0.702_345_030_644_509_8],
    [-1.801_235_295_382_490_2, -0.568_357_261_721_409_8, -0.407_660_172_472_919_64, -0.299_368_735_694_693_8, -0.216_500_501_598_051_45, -0.145_428_020_810_759_9, -0.085_899_305_141_452_05, -0.037_133_786_303_516_05, 0.001_951_166_438_912_466, 0.036_024_297_941_389_95, 0.071_159_894_622_043_97, 0.102_392_711_963_129_82, 0.130_414_827_382_330_23, 0.157_221_187_158_662_07, 0.185_405_971_688_910_26, 0.227_165_314_692_469_6, 0.336_556_076_366_004_93],
];

/// The slow accreting white dwarfs' marks' medians' spans of their conditions, first and last
/// node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIAN_SPANS_26: [(f64, f64); 11] = [
    (0.0, 1.0),
    (5.511_823_662_095_153, 10.0),
    (-12.503_206_910_402_122, -9.286_098_210_519_9),
    (-16.412_246_043_652_036, -10.639_765_577_079_03),
    (-13.656_590_964_648_293, -7.100_224_314_739_383),
    (-0.220_231_361_188_203_66, 1.587_379_684_941_585_3),
    (-0.188_728_316_781_699_8, 1.986_496_097_461_852),
    (1.817_516_018_517_854_7, 5.734_347_611_645_019),
    (0.342_168_418_510_660_5, 1.569_972_768_424_638_3),
    (-3.811_998_598_596_534, 0.216_237_739_308_761_94),
    (0.050_658_027_118_156_035, 0.828_833_572_128_682_8),
];

/// The slow accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;
/// the recurrence's is zero.
#[rustfmt::skip]
pub const AWD_SLOW_MEDIANS_26: [[f64; 17]; 11] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.715_340_810_049_814_1, 0.680_644_054_366_276_6, 0.620_874_770_342_728_6, 0.583_830_033_160_131_6, 0.589_867_153_393_239_3, 0.763_230_375_287_826_6, 1.294_195_398_071_974, 1.264_308_907_936_509_7, 1.168_127_788_723_371_4, 1.076_244_407_079_019_1, 1.035_975_681_732_97, 1.007_720_403_074_312_5, 0.977_303_376_019_961_2, 0.945_534_257_919_632_2, 1.440_610_501_211_077, 1.440_840_529_664_926_3, 1.440_824_375_506_556_8],
    [0.956_345_221_347_87, 0.992_969_238_101_938_9, 0.955_175_085_899_251_6, 0.818_001_898_015_445_5, 1.008_124_720_633_872_8, 1.004_921_070_498_419_7, 0.859_083_156_805_720_9, 0.441_601_702_611_558_3, 0.923_476_319_978_330_6, 1.037_737_898_322_836_7, 1.040_247_111_089_949_4, 0.981_886_300_273_491, 0.362_790_546_061_348_64, 0.353_423_459_326_376_9, 0.911_914_301_211_827_3, 0.970_700_492_259_281_7, 0.955_258_595_779_968_4],
    [1.389_291_510_794_226, 1.277_022_914_281_126_5, 1.189_138_615_007_435_2, 1.070_351_644_886_199, 0.929_642_171_250_622_7, 0.746_722_094_046_720_9, 0.644_138_473_940_701_2, 0.536_381_919_503_147_3, 0.431_156_834_907_453_4, 0.262_899_070_314_239_7, 0.237_645_533_171_790_27, 0.211_233_043_675_501_44, 0.164_174_458_300_604_34, 0.090_071_858_692_262_32, 0.003_006_411_666_366_803_4, -0.065_467_215_238_970_52, -0.142_401_442_398_954_33],
    [2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.075_428_480_499_179, 2.065_022_362_034_575_6, 2.054_616_243_569_972_4, 2.044_210_125_105_369_3, 2.033_804_006_640_765_7, 2.035_063_029_708_883, 2.024_863_328_329_219_7, 1.973_527_257_480_312_9, 1.969_560_002_406_848_3, 2.055_418_332_901_131_5, 2.055_418_332_901_131_5, 2.055_418_332_901_131_5],
    [-1.110_881_983_759_018_5, -1.056_103_467_296_889_9, -0.990_169_274_093_04, -0.882_469_869_050_558_7, -0.771_135_342_817_400_9, -0.684_274_626_245_712_3, -0.623_743_218_349_097_1, -0.531_868_841_764_224_4, -0.021_105_050_971_070_74, -0.000_603_565_716_527_275_5, 0.018_885_852_302_791_342, 0.043_959_803_311_345_844, 0.074_719_479_075_490_54, 0.114_051_424_276_142_91, 0.164_352_822_530_253_85, 0.196_560_247_859_489_28, 0.214_292_464_657_718_28],
    [-1.833_532_047_952_258_3, -1.967_007_383_430_915_4, -2.073_515_924_771_854_6, -2.120_195_208_524_362, -2.142_837_083_536_134_6, -2.214_350_974_491_676_7, -2.545_308_234_078_28, -2.681_285_150_436_594, -2.845_354_190_478_460_5, -2.963_940_263_515_806_5, -3.099_013_153_096_511_3, -3.200_266_269_145_310_5, -3.303_869_666_263_958, -3.401_359_145_076_593_6, -3.649_957_113_317_623, -3.799_255_164_593_623, -3.914_113_734_510_498_6],
    [-0.737_150_395_036_852_5, -0.756_250_458_886_896_3, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4, -0.760_627_225_476_970_4],
    [0.139_979_512_984_610_37, 0.191_631_262_560_867_13, 0.301_448_625_690_737_16, 0.355_875_099_326_860_34, 0.401_840_147_008_288_5, 0.430_727_312_403_842_2, 0.383_886_571_644_902_86, 0.408_209_162_822_413_65, 0.448_933_860_319_296_1, 0.484_084_352_171_492_87, 0.548_243_339_346_025_8, 0.627_603_222_689_928_2, 0.680_316_388_601_559_9, 0.662_062_737_899_264_6, 0.654_056_686_894_350_4, 0.661_267_532_381_382_5, 0.680_080_417_590_675_1],
    [0.276_434_633_990_225_6, 0.261_713_839_110_141_5, 0.247_315_185_945_215_95, 0.238_156_483_661_835_06, 0.230_355_364_680_422_5, 0.263_265_560_289_643_3, 0.288_253_223_938_530_53, 0.288_855_976_086_382_47, 0.279_562_471_023_245_05, 0.203_623_998_390_136_48, -0.619_702_999_369_006_6, -0.609_879_822_121_960_9, -0.563_114_265_942_083_2, -0.521_021_410_368_464_9, 0.045_170_322_077_759_82, 0.077_362_414_253_180_05, 0.104_618_386_523_116_86],
    [9.949_008_552_725_987, 9.909_593_505_100_306, 9.869_498_200_436_237, 9.834_356_313_597_063, 9.810_782_068_579_885, 9.782_965_101_103_256, 9.752_943_167_499_49, 9.745_339_385_768_03, 9.742_347_340_310_376, 9.754_565_605_510_674, 9.785_626_303_585_543, 9.808_795_712_841_068, 9.819_469_865_446_418, 9.826_170_511_921_871, 9.824_887_693_351_608, 9.814_423_088_541_153, 9.806_269_328_071_929],
];

/// The slow accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_LIMITS_26: [(f64, f64); 11] = [
    (5.418_866_376_726_735, 10.0),
    (0.305_803_540_975_784_7, 1.797_033_318_199_152_2),
    (-0.337_561_558_011_858_33, 2.025_285_738_777_566_5),
    (-0.414_381_389_939_817_27, 3.996_634_167_072_526),
    (1.721_077_999_357_347_7, 6.215_938_772_946_526),
    (-1.992_131_604_976_145, 0.478_406_778_667_533_84),
    (-6.0, 0.243_431_747_403_024_03),
    (-0.816_627_956_194_494, 0.470_837_257_299_106_5),
    (-0.037_923_126_073_938_634, 0.896_335_052_504_602_9),
    (-1.094_790_562_727_25, 0.695_574_415_800_093_3),
    (8.017_791_846_134_891, 10.079_170_744_039_297),
];

/// The slow accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order.
#[rustfmt::skip]
pub const AWD_SLOW_DONOR_KINDS_26: [f64; 3] = [
    0.315_410_052_910_052_9,
    0.681_878_306_878_306_9,
    0.002_711_640_211_640_212,
];

/// The fast accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_FAST_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    5.515_714_482_771_766e-7,
    1.030_515_587_394_176_9e-5,
    5.250_181_011_969_452e-6,
    5.334_734_626_711_921e-6,
    8.094_579_758_749_473e-6,
    7.909_816_034_345_367e-6,
    3.126_907_781_841_143_4e-6,
];

/// The share of the fast accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_FAST_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.476_797_209_054_990_35,
    0.511_826_020_560_099_4,
    0.011_376_770_384_910_279,
];

/// The slow accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const AWD_SLOW_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    1.839_001_160_882_683e-7,
    8.124_114_888_793_676e-5,
    0.000_128_202_579_616_058,
    8.663_444_595_226_882e-5,
    8.847_347_143_452_543e-5,
    7.559_675_969_724_225e-5,
    0.000_172_349_838_162_985_93,
];

/// The share of the slow accreting white dwarfs' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const AWD_SLOW_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.372_416_697_618_379_64,
    0.612_628_301_944_481_6,
    0.014_955_000_437_138_878,
];
// @end-table binary_awd

// @begin-table binary_xrb
// The X-ray binaries' sampler and share row fitted against plan 11's engine (plan 15, P15.T10.b),
// held until generator version 26 (`catalogue_classes::binary`'s hold).
//
// @generated by hyperion-fit 0.1.0, task `binary_xrb` revision 0. Do not edit.
// inputs-sha256: 59047b1bde352b12a6b8f4ee32a5ad35446beefa81bf7d4eb690800829746bb5
// manifest: crates/hyperion-fit/manifests/binary_xrb.toml
// data: none
// sim-fingerprint: 5906f5d3b305c569429f7e6371ebda7da9e7ed808b19aa48e156e5514395984e
// since-generator-version: 24
// source: plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 builds
//   it) and classes (P11.T5), over plan 06's tracks, at the generator version's rules; the total
//   against the brainstorm's 10⁴ (Corral-Santana et al. 2016, A&A 587, A61)
// acceptance: slab members 540 of a kind and 1673 of none; young high-mass members 132; kinds
//   0.0552/0.2397/0.0389/0.6629/0.0013/0.0019; black-hole transients 6629 of 1e4; layers C 0.000
//   D 0.550 E 0.450; rates a system 1.13e-4/4.59e-5/5.56e-5/3.43e-5/3.08e-5/3.36e-5/5.61e-5;
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
    [1.191_657_221_478_672_5, 1.282_156_327_808_284_1, 1.447_460_576_045_716_7, 1.532_938_029_143_904_3, 1.576_352_664_858_010_8, 1.649_013_209_756_917_2, 1.697_278_575_806_372_2, 1.742_265_625_014_659_8, 1.777_857_626_107_706_6, 1.800_937_814_436_021_5, 1.820_035_849_959_319_4, 1.844_355_227_436_137_2, 1.870_289_616_396_379, 1.903_917_895_969_546_4, 1.929_071_121_619_036_6, 1.964_186_598_740_191_9, 2.001_044_593_121_261],
    [2.008_176_480_393_786_7, 2.096_550_448_931_618_7, 2.162_446_104_972_964_6, 2.225_573_103_576_801, 2.272_444_923_776_233_4, 2.352_838_584_065_777_3, 2.441_075_715_158_023, 2.512_763_224_683_195_2, 2.601_435_074_689_165, 2.704_739_676_862_122, 2.761_608_311_353_647_6, 2.824_164_614_545_795_3, 2.936_495_580_665_654, 3.039_239_833_208_515_8, 3.196_096_986_011_714_3, 3.514_291_797_321_665_6, 7.965_248_775_045_085],
    [0.153_641_083_956_922_05, 1.118_713_219_205_639_5, 1.205_286_659_166_947_8, 1.268_253_136_925_135_7, 1.306_456_388_138_049_5, 1.346_442_266_289_229_5, 1.381_009_169_707_204_4, 1.409_634_772_036_584, 1.440_712_432_331_313, 1.468_601_481_740_828, 1.503_133_362_770_291_9, 1.536_466_810_838_843, 1.577_600_335_275_958_9, 1.642_660_749_107_594, 1.815_428_057_966_477_9, 2.234_286_187_752_148, 2.902_286_234_563_062],
    [-0.666_728_511_943_880_3, -0.187_919_512_871_541_46, -0.104_604_385_748_682_29, -0.077_758_983_034_704_91, -0.047_060_892_123_961_6, -0.033_004_265_307_490_48, -0.022_022_469_756_328_21, -0.012_236_177_904_606_516, -0.001_519_995_996_768_275_2, 0.010_349_196_840_632_25, 0.024_096_001_421_675_145, 0.039_908_417_633_771_55, 0.061_651_762_242_768_415, 0.084_837_978_475_376_84, 0.125_005_519_434_011_64, 0.206_616_607_275_489_8, 1.215_283_228_509_042_7],
    [-4.457_153_421_999_673, -2.379_015_116_506_759_7, -1.889_608_563_985_721_9, -1.752_697_222_588_314_4, -1.624_256_607_458_695, -1.535_807_711_293_844_9, -1.400_738_786_861_010_6, -1.300_068_663_395_176, -1.152_027_010_010_050_5, -1.068_387_603_274_513_9, -0.957_304_814_072_517_8, -0.813_351_230_288_171_8, -0.680_395_793_160_064_7, -0.471_881_255_826_604_4, -0.199_131_179_639_805_12, 0.139_072_896_048_477_14, 1.582_648_612_343_177_3],
    [1.75, 2.254_32, 2.435_993_5, 2.563_591, 2.667_624_5, 2.758_895_5, 2.842_726, 2.922_329_5, 3.0, 3.077_670_5, 3.157_274, 3.241_104_5, 3.332_375_5, 3.436_409, 3.564_006_5, 3.745_68, 4.25],
    [1.25, 1.754_319_999_999_999_9, 1.935_993_5, 2.063_591, 2.167_624_5, 2.258_895_5, 2.342_726, 2.422_329_5, 2.5, 2.577_670_5, 2.657_274, 2.741_104_5, 2.832_375_5, 2.936_409, 3.064_006_5, 3.245_68, 3.75],
    [0.569_034_759_589_777_6, 0.830_492_508_578_256, 1.156_784_294_170_731_8, 1.576_454_779_287_699_7, 1.723_474_636_660_581, 1.831_222_640_888_324_5, 1.890_061_649_811_308_8, 1.909_514_456_655_034_2, 2.050_410_372_049_656_7, 2.065_927_749_375_332, 2.110_636_434_529_898, 2.179_422_176_007_907, 2.221_705_480_235_695_7, 2.273_678_414_603_611_3, 2.288_959_380_435_318, 2.391_052_198_407_923_6, 2.863_591_070_574_552_4],
    [0.020_085_957_879_991_75, 0.046_004_675_583_994_26, 0.052_277_395_334_078_57, 0.061_897_171_159_805_78, 0.068_270_792_815_718_34, 0.079_588_917_388_263_78, 0.089_238_418_953_627_08, 0.099_053_534_117_370_04, 0.112_464_783_471_267_63, 0.120_903_205_919_218_01, 0.131_585_309_506_646_8, 0.139_603_866_234_645_1, 0.149_438_490_979_388_44, 0.184_568_677_069_151_3, 0.260_717_705_133_707_73, 0.378_204_016_250_945_85, 0.620_393_956_235_344_4],
    [8.037_509_922_784_235, 8.243_728_883_095_983, 8.418_984_836_137_43, 8.592_910_701_638_772, 8.814_105_011_911_046, 9.099_749_944_423_897, 9.486_598_241_604_703, 9.669_535_773_085_228, 9.998_597_393_556_155, 10.403_295_953_307_49, 11.204_441_003_436_04, 11.598_904_043_241_351, 12.321_423_541_335_253, 12.550_342_174_864_685, 13.633_898_441_860_79, 15.608_397_537_707_11, 16.871_065_007_533_14],
    [-0.008_804_780_701_620_758, 0.194_830_645_341_693_1, 0.263_341_231_509_416_6, 0.289_464_186_661_892_8, 0.316_022_598_981_980_54, 0.395_382_806_158_807_17, 0.469_249_746_749_796_25, 0.607_877_759_375_968_2, 0.754_382_869_313_083_1, 0.880_045_350_525_836_5, 1.634_682_712_074_604, 2.034_334_957_082_387, 2.260_674_128_379_322_6, 2.435_949_134_764_771, 3.352_727_229_177_864_5, 4.506_388_699_420_735, 5.769_365_353_046_805],
    [8.068_187_932_070_689, 8.302_891_832_121_574, 8.483_380_474_877_723, 8.592_140_668_682_859, 8.669_171_400_355_207, 8.921_874_999_655_422, 9.269_219_312_404_186, 9.626_827_699_316_822, 10.392_668_006_249_806, 11.145_516_681_448_308, 11.595_126_830_478_687, 12.102_937_683_123_042, 13.099_433_995_464_702, 13.572_542_963_821_107, 14.710_327_091_157_971, 16.001_679_293_928_98, 21.563_334_717_416_193],
    [-10.25, -9.745_68, -9.564_006_5, -9.436_409, -9.332_375_5, -9.241_104_5, -9.157_274, -9.077_670_5, -9.0, -8.922_329_5, -8.842_726, -8.758_895_5, -8.667_624_5, -8.563_591, -8.435_993_5, -8.254_32, -7.75],
    [0.381_775_076_517_603_56, 1.593_655_960_460_839, 2.032_611_602_667_470_8, 2.377_094_695_243_644_5, 2.815_448_867_491_897_4, 2.982_663_166_790_320_7, 3.622_121_066_753_728, 4.103_648_174_510_329_5, 4.406_517_008_623_304, 4.932_787_396_856_673, 5.165_561_294_323_287, 5.562_963_369_438_622, 6.692_788_408_060_023, 9.277_999_843_633_877, 18.103_770_243_191_73, 37.112_300_558_611_83, 85.483_030_322_332_9],
    [0.417_436_207_726_653_96, 1.934_841_091_145_499_6, 2.927_232_333_455_273_2, 3.726_835_764_870_239, 4.986_598_494_446_106, 19.147_374_170_682_603, 20.970_122_385_828_518, 24.506_261_012_306_38, 26.895_000_037_528_003, 27.875_042_401_231_923, 29.158_276_641_100_09, 39.474_742_628_066_025, 59.557_118_251_812_81, 83.731_707_277_964, 95.752_337_863_650_14, 103.138_663_486_757_92, 146.736_265_785_928_9],
    [5.690_529_023_627_952, 5.943_474_173_712_12, 6.227_029_085_382_988, 6.295_916_360_430_484, 6.417_459_155_919_676, 6.611_861_138_438_966, 6.771_858_435_678_364, 6.861_524_232_478_397, 6.886_126_066_634_5, 6.933_992_568_300_974, 6.986_508_694_177_974, 7.024_127_786_606_002, 7.107_132_594_537_781, 7.168_061_315_484_644, 7.241_444_867_492_735, 7.318_288_681_090_058, 7.444_682_532_858_668],
    [5.383_882_569_445_701, 6.375_839_551_430_132, 6.433_995_562_042_982, 6.539_761_249_508_62, 6.625_876_004_307_642, 6.730_074_661_923_587, 6.790_508_528_867_664, 6.813_702_820_798_288, 6.844_770_880_810_32, 6.866_351_685_743_691, 6.886_600_987_523_288_5, 6.930_433_376_974_545, 6.983_647_823_995_133, 7.050_850_864_379_985, 7.063_766_091_334_537_5, 7.200_172_778_223_321, 7.379_671_014_390_03],
    [0.542_188_014_414_711_3, 0.584_576_724_568_982_1, 0.614_582_902_104_294_6, 0.633_738_899_273_159_7, 0.672_897_134_376_132_7, 0.693_604_407_895_072_6, 0.709_010_362_095_116, 0.736_186_637_507_089, 0.783_915_656_135_625_4, 0.862_217_091_325_582_2, 0.916_540_518_008_631_9, 0.964_863_343_784_969_2, 0.998_146_790_605_179_6, 1.012_899_211_870_135_2, 1.041_017_636_406_357, 1.074_757_367_608_385_5, 1.184_261_519_243_101_6],
    [0.510_949_764_622_598, 0.675_701_716_620_431, 0.707_173_406_001_715_2, 0.745_624_822_616_199_2, 0.826_199_416_802_658_3, 0.883_985_181_072_693_1, 0.901_354_306_001_690_8, 0.930_009_107_363_148_7, 0.958_612_601_772_427_4, 0.984_585_923_330_147_8, 1.012_655_843_763_895_7, 1.041_961_595_687_218_4, 1.066_095_556_366_191_1, 1.083_615_675_747_404_3, 1.117_762_499_961_485, 1.169_469_007_797_023_3, 1.456_794_502_018_168_5],
    [-0.193_843_284_005_470_6, 0.016_087_103_996_769_36, 0.061_158_798_869_358_08, 0.116_369_614_112_760_67, 0.154_341_398_209_541_4, 0.182_712_820_472_317_18, 0.206_604_162_748_464_6, 0.232_801_753_569_077_54, 0.256_710_034_961_534_33, 0.274_098_884_608_770_8, 0.301_456_374_784_260_3, 0.320_269_705_654_683_1, 0.357_291_414_667_595_9, 0.424_523_591_977_771_7, 0.580_155_606_487_871_9, 0.762_696_343_523_659_4, 2.240_775_294_621_659],
];

/// The X-ray binaries' marks' medians' spans of their conditions, first and last node, in
/// [`XrbMark`]'s order; only the low-mass donor's mass's is read.
#[rustfmt::skip]
pub const XRB_MEDIAN_SPANS_26: [(f64, f64); 20] = [
    (0.0, 1.0),
    (0.0, 1.0),
    (0.0, 1.0),
    (0.243_261_360_745_384_57, 2.711_986_463_617_623),
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
    [-1.857_922_827_584_143_5, -1.857_922_827_584_143_5, -1.244_469_015_639_435_6, -0.631_015_203_694_727_4, -0.017_561_391_750_019_25, 0.128_429_199_256_278_71, 0.168_581_316_523_565_14, 0.190_244_713_946_408_6, 0.210_269_172_330_622_6, 0.235_012_820_641_251_73, 0.129_033_445_899_304_38, -0.051_928_905_396_211_38, -0.175_803_946_633_005_55, -0.252_283_786_487_650_26, -0.311_141_346_269_897_7, -0.361_397_091_935_615_3, -0.420_986_590_304_136_2],
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
    (1.191_657_221_478_672_5, 2.001_044_593_121_261),
    (2.008_176_480_393_786_7, 7.965_248_775_045_085),
    (0.153_641_083_956_922_05, 2.902_286_234_563_062),
    (-1.994_364_450_422_866_1, 0.813_463_367_662_482_4),
    (-4.457_153_421_999_673, 1.582_648_612_343_177_3),
    (1.75, 4.25),
    (1.25, 3.75),
    (0.569_034_759_589_777_6, 2.863_591_070_574_552_4),
    (0.020_085_957_879_991_75, 0.620_393_956_235_344_4),
    (8.037_509_922_784_235, 16.871_065_007_533_14),
    (-0.008_804_780_701_620_758, 5.769_365_353_046_805),
    (8.068_187_932_070_689, 21.563_334_717_416_193),
    (-10.25, -7.75),
    (0.381_775_076_517_603_56, 85.483_030_322_332_9),
    (0.417_436_207_726_653_96, 146.736_265_785_928_9),
    (5.690_529_023_627_952, 7.444_682_532_858_668),
    (5.383_882_569_445_701, 7.379_671_014_390_03),
    (0.542_188_014_414_711_3, 1.184_261_519_243_101_6),
    (0.510_949_764_622_598, 1.456_794_502_018_168_5),
    (-0.193_843_284_005_470_6, 2.240_775_294_621_659),
];

/// The low-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::LOW_MASS`]' order.
#[rustfmt::skip]
pub const XRB_LOW_MASS_KINDS_26: [f64; 4] = [
    0.055_241_832_301_967_636,
    0.239_727_203_305_082_96,
    0.038_939_571_595_628_61,
    0.662_884_228_386_528_3,
];

/// The high-mass kinds' weights, as shares of all the class's hosts, in
/// [`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.
#[rustfmt::skip]
pub const XRB_HIGH_MASS_KINDS_26: [f64; 2] = [
    0.001_336_333_739_039_709,
    0.001_870_830_671_752_692_7,
];

/// The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order.
#[rustfmt::skip]
pub const XRB_HOSTS_PER_FORMED_MASS_26: [f64; 7] = [
    2.861_459_152_570_120_4e-7,
    1.157_904_093_115_333_1e-7,
    1.401_535_709_797_767_2e-7,
    8.664_363_283_164_526e-8,
    7.779_743_618_757_726e-8,
    8.487_612_915_920_31e-8,
    1.414_622_815_229_235_9e-7,
];

/// The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.
#[rustfmt::skip]
pub const XRB_LAYER_SHARES_26: [f64; 5] = [
    0.0,
    0.0,
    0.0,
    0.549_825_069_239_395_8,
    0.450_174_930_760_604_1,
];
// @end-table binary_xrb

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;

    /// P11.T6: the scratch yield is a sixth in every delay bin and for both channels, and a delay
    /// takes the bin of `DELAY_EDGES` that holds it, the ends held.
    #[test]
    fn the_scratch_ia_yield_is_a_sixth_everywhere() {
        for bin in 0..IA_DELAY_BINS {
            for channel in [IaPoolChannel::Merger, IaPoolChannel::Accretion] {
                assert_eq!(IA_YIELD.eta_in(bin, channel), 1.0 / 6.0);
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
